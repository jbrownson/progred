//! A headless editor an agent operates the way a person does: it looks at
//! screenshots and sends the pointer and keyboard to points on them, with the
//! actions of Anthropic's computer-use tool. Each request is a GID document
//! whose root is a Grap expression, and each reply a document whose root is
//! its result.
//!
//! `make control ARGS='--document PATH'` starts a session on a local socket,
//! and `tools/control` sends it one action at a time.

mod computer;

use crate::clipboard::Clipboard;
use crate::driver::{Driver, keystroke_for};
use crate::gid_text::{self, Binders};
use crate::library::Libraries;
use crate::sources::Sources;
use gid::{CellId, Document};
use kurbo::{Point, Size, Vec2};
use puri::draw::DrawList;
use puri_vello::capture::Gpu;
use puri_vello::compositor::{Compositor, Resources};
use std::cell::RefCell;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use ui_events::keyboard::{Key, Modifiers};
use winit::keyboard::ModifiersState;

/// A laptop display's logical size, which computer use handles well. One
/// screenshot pixel is one logical unit.
const WINDOW: Size = Size::new(1280.0, 800.0);

/// Pointer moves in a drag, so gestures that follow motion see it.
const DRAG_STEPS: usize = 10;

const DEFAULT_SOCKET: &str = "target/control/progred.sock";

/// The pointer and keyboard, at window coordinates.
pub(crate) enum Action {
    Move(Point),
    Click {
        at: Point,
        count: usize,
        held: ModifiersState,
    },
    Drag {
        from: Point,
        to: Point,
        held: ModifiersState,
    },
    Scroll {
        at: Point,
        lines: Vec2,
        held: ModifiersState,
    },
    Key(Key, Modifiers),
    Type(String),
}

impl Action {
    fn points(&self) -> Vec<Point> {
        match self {
            Self::Move(at) | Self::Click { at, .. } | Self::Scroll { at, .. } => vec![*at],
            Self::Drag { from, to, .. } => vec![*from, *to],
            Self::Key(..) | Self::Type(_) => Vec::new(),
        }
    }
}

/// One editor in a window of [`WINDOW`], with in-app menus and no file
/// dialogs, and the GPU its screenshots render on once one is taken.
pub(crate) struct Session {
    driver: Driver,
    capture: Option<Capture>,
}

impl Session {
    pub(crate) fn new(doc: Document, binders: Binders) -> Self {
        let mut editor = crate::editor::new_editor(
            crate::display::widget::style::Theme::Light.palette(),
            crate::modifiers::native(),
            true,
            crate::stack::load(),
            crate::fonts::bundled_font_context(),
            doc,
            None,
            binders,
            None,
        );
        editor.focused = true;
        editor.file_dialogs = false;
        editor.text_clipboard = Clipboard::default();
        Self {
            driver: Driver::new(editor, WINDOW),
            capture: None,
        }
    }

    /// The editor's libraries, with the computer's actions on this session.
    fn libraries(session: &Rc<RefCell<Self>>) -> Libraries {
        let mut libraries = session.borrow().driver.editor().stack.libraries.clone();
        libraries.insert(
            computer::ID,
            computer::library::<crate::Editor>(session).definitions,
        );
        libraries
    }

    /// Does `action`, unless it points outside the window.
    pub(crate) fn perform(&mut self, action: Action) -> Result<(), CellId> {
        action
            .points()
            .into_iter()
            .all(|point| WINDOW.to_rect().contains(point))
            .then(|| self.act(action))
            .ok_or(computer::vocabulary::OUTSIDE_WINDOW)
    }

    fn act(&mut self, action: Action) {
        let driver = &mut self.driver;
        match action {
            Action::Move(at) => driver.move_to(at),
            Action::Click { at, count, held } => holding(driver, held, |driver| {
                driver.clicks_at(at, count);
            }),
            Action::Drag { from, to, held } => holding(driver, held, |driver| {
                driver.drag(from, &drag_path(from, to));
            }),
            Action::Scroll { at, lines, held } => holding(driver, held, |driver| {
                driver.scroll_at(at, lines);
            }),
            Action::Key(key, modifiers) => {
                driver.keystroke(key, modifiers);
            }
            Action::Type(text) => text
                .chars()
                .map(keystroke_for)
                .for_each(|(key, modifiers)| {
                    driver.keystroke(key, modifiers);
                }),
        }
    }

    /// The window as it stands, as a PNG.
    pub(crate) fn screenshot(&mut self) -> Result<Vec<u8>, String> {
        let paper = self.driver.editor().palette.paper;
        let capture = match self.capture.take() {
            Some(capture) => capture,
            None => Capture::new()?,
        };
        self.capture
            .insert(capture)
            .picture(self.driver.painted(), paper)
    }
}

/// `act` with `held` modifiers down, as a person holds a key while they click.
fn holding(driver: &mut Driver, held: ModifiersState, act: impl FnOnce(&mut Driver)) {
    let press = !held.is_empty();
    if press {
        driver.hold(held);
    }
    act(driver);
    if press {
        driver.hold(ModifiersState::empty());
    }
}

fn drag_path(from: Point, to: Point) -> Vec<Point> {
    (1..=DRAG_STEPS)
        .map(|step| from.lerp(to, step as f64 / DRAG_STEPS as f64))
        .collect()
}

/// The GPU a session draws its screenshots with.
struct Capture {
    gpu: Gpu,
    compositor: Compositor,
    resources: Resources,
    target: vello::wgpu::Texture,
}

impl Capture {
    fn new() -> Result<Self, String> {
        let gpu = Gpu::new();
        let compositor =
            Compositor::new(&gpu.device, &gpu.queue).map_err(|error| error.to_string())?;
        let target = gpu.texture(WINDOW.width as u32, WINDOW.height as u32);
        Ok(Self {
            gpu,
            compositor,
            resources: Resources::default(),
            target,
        })
    }

    fn picture(&mut self, painted: &DrawList, paper: peniko::Color) -> Result<Vec<u8>, String> {
        crate::picture::Picture {
            drawing: painted.clone(),
            paper,
            region: WINDOW.to_rect(),
        }
        .png(
            &self.gpu,
            &mut self.compositor,
            &mut self.resources,
            &self.target,
        )
    }
}

/// The reply to one request: its root evaluated with the loaded libraries
/// and the computer's actions, or why it couldn't be read.
fn respond(libraries: &Libraries, binders: &Binders, request: &[u8]) -> String {
    let result = std::str::from_utf8(request)
        .map_err(|error| error.to_string())
        .and_then(gid_text::parse)
        .and_then(|(doc, _)| {
            doc.root
                .as_ref()
                .map(|root| {
                    ::grap::evaluate(
                        root,
                        &Sources {
                            doc: &doc,
                            libraries,
                        },
                        crate::sources::DEFAULT_FUEL,
                    )
                    .result
                    .into_value()
                })
                .ok_or_else(|| "a request's root is the expression to evaluate".to_owned())
        })
        .unwrap_or_else(computer::invalid_request);
    gid_text::print(
        &Document {
            root: Some(result),
            ..Document::default()
        },
        binders,
    )
}

/// Reading through one GID document: how deep in records it is, and whether
/// it's inside quoted text, where braces don't count.
#[derive(Clone, Copy, Default)]
struct Framing {
    depth: usize,
    opened: bool,
    quoted: bool,
    escaped: bool,
}

impl Framing {
    fn step(mut self, byte: u8) -> Self {
        match (self.quoted, self.escaped, byte) {
            (true, true, _) => self.escaped = false,
            (true, false, b'\\') => self.escaped = true,
            (true, false, b'"') => self.quoted = false,
            (true, false, _) => {}
            (false, _, b'"') => self.quoted = true,
            (false, _, b'{') => {
                self.depth += 1;
                self.opened = true;
            }
            (false, _, b'}') => self.depth = self.depth.saturating_sub(1),
            (false, _, _) => {}
        }
        self
    }
}

/// Where the first whole document in `bytes` ends: just after the brace
/// that closes its outermost record.
fn document_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .iter()
        .scan(Framing::default(), |framing, byte| {
            *framing = framing.step(*byte);
            Some(*framing)
        })
        .position(|framing| framing.opened && framing.depth == 0)
        .map(|index| index + 1)
}

/// Answers each document a client sends, in order, until it hangs up.
fn converse(
    mut stream: UnixStream,
    libraries: &Libraries,
    binders: &Binders,
) -> std::io::Result<()> {
    let mut pending = Vec::new();
    let mut buffer = [0; 1 << 16];
    while let read @ 1.. = stream.read(&mut buffer)? {
        pending.extend_from_slice(&buffer[..read]);
        while let Some(end) = document_end(&pending) {
            let request: Vec<u8> = pending.drain(..end).collect();
            stream.write_all(respond(libraries, binders, &request).as_bytes())?;
        }
    }
    Ok(())
}

/// A socket only its owner can connect to, replacing a stale one. A new
/// directory for it is private too.
fn listen(socket: &Path) -> std::io::Result<UnixListener> {
    if let Some(directory) = socket.parent().filter(|directory| !directory.exists()) {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)?;
    }
    if std::fs::symlink_metadata(socket).is_ok_and(|metadata| metadata.file_type().is_socket()) {
        std::fs::remove_file(socket)?;
    }
    let listener = UnixListener::bind(socket)?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

struct Options {
    document: Option<PathBuf>,
    socket: PathBuf,
}

fn options(arguments: &[String]) -> Result<Options, String> {
    arguments.chunks(2).try_fold(
        Options {
            document: None,
            socket: PathBuf::from(DEFAULT_SOCKET),
        },
        |options, pair| match pair {
            [flag, value] if flag == "--document" => Ok(Options {
                document: Some(PathBuf::from(value)),
                ..options
            }),
            [flag, value] if flag == "--socket" => Ok(Options {
                socket: PathBuf::from(value),
                ..options
            }),
            [flag, _] => Err(format!("unknown option {flag}")),
            _ => Err(format!("{} needs a value", pair.concat())),
        },
    )
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("control: {message}");
    std::process::exit(1)
}

pub fn main() {
    let options =
        options(&std::env::args().skip(1).collect::<Vec<_>>()).unwrap_or_else(|message| {
            fail(format!(
                "{message}\nusage: progred-control [--document PATH] [--socket PATH]"
            ))
        });
    let (doc, binders) = options
        .document
        .as_deref()
        .map_or_else(
            || Ok((Document::default(), Binders::new())),
            crate::text_store::load,
        )
        .unwrap_or_else(|message| fail(message));
    let session = Rc::new(RefCell::new(Session::new(doc, binders)));
    let libraries = Session::libraries(&session);
    let vocabulary = computer::binders();
    let listener = listen(&options.socket)
        .unwrap_or_else(|error| fail(format!("{}: {error}", options.socket.display())));
    let vocabulary_path = options.socket.with_file_name("vocabulary.gid");
    std::fs::write(&vocabulary_path, computer::vocabulary_document(&vocabulary))
        .unwrap_or_else(|error| fail(format!("{}: {error}", vocabulary_path.display())));
    eprintln!(
        "control: listening on {}, with the vocabulary's binders in {}",
        options.socket.display(),
        vocabulary_path.display()
    );
    for stream in listener.incoming() {
        if let Err(error) = stream.and_then(|stream| converse(stream, &libraries, &vocabulary)) {
            eprintln!("control: {error}");
        }
    }
}

#[cfg(test)]
#[path = "tests/control.rs"]
mod tests;
