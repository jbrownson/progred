//! The website's tutorials, followed step by step. Each embedded editor opens
//! as its page configures it, and the reader's input reaches it the way the
//! browser delivers it: pointer motion and buttons through Winit, keys as the
//! page hands them over, a painted frame after each.

use crate::driver::Driver;
use gid::{CellId, Step, Value};
use kurbo::{Point, Size};
use std::rc::Rc;
use ui_events::keyboard::NamedKey;
use winit::keyboard::ModifiersState;
use winit::window::CursorIcon;

mod peel;
mod story;
mod tutorial;

/// The tutorial column's frame less its border, tall enough that no lesson
/// scrolls.
const VIEWPORT: Size = Size::new(704.0, 1400.0);
const SCALE: f64 = 1.0;
const FOLLOW: Step = Step::Follow(gid::Resolution::Document);

fn website(path: &str) -> String {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../website/public")
        .join(path);
    std::fs::read_to_string(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()))
}

/// The editor address a page embeds in its section `id`, and the tasks the
/// page numbers for it.
fn section(page: &str, id: &str) -> (String, Vec<String>) {
    let html = website(page);
    let start = html
        .find(&format!("id=\"{id}\""))
        .unwrap_or_else(|| panic!("{page} has no #{id}"));
    let section = &html[start..];
    let section = &section[..section.find("</section>").unwrap_or(section.len())];
    let tag = &section[section
        .find("<iframe")
        .unwrap_or_else(|| panic!("#{id} embeds no editor"))..];
    let tag = &tag[..tag.find('>').unwrap()];
    let src = [" src=\"", " data-src=\""]
        .iter()
        .find_map(|attribute| {
            let value = &tag[tag.find(attribute)? + attribute.len()..];
            Some(value[..value.find('"')?].replace("&amp;", "&"))
        })
        .expect("the editor's address");
    let tasks = section
        .split("data-task=\"")
        .skip(1)
        .map(|rest| rest[..rest.find('"').unwrap()].to_owned())
        .collect();
    (src, tasks)
}

/// An editor embedded in a tutorial page.
pub(super) type Embed = Driver<Page>;

/// What the page says about its embedded editor.
pub(super) struct Page {
    names: crate::gid_text::Binders,
    /// The page's choice of libraries and slots, which peeling draws from.
    libraries: Option<String>,
    slots: Option<String>,
    tasks: Vec<String>,
    done: usize,
}

impl Driver<Page> {
    pub(super) fn open(page: &str, id: &str) -> Self {
        let (src, tasks) = section(page, id);
        let (_, query) = src.split_once('?').expect("the editor's options");
        let option = |name: &str| {
            query
                .split('&')
                .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
                .map(str::to_owned)
        };
        let document = option("document").expect("a document");
        let (doc, names) = crate::gid_text::parse(&website(
            document
                .strip_prefix("../")
                .expect("a document beside the editor"),
        ))
        .unwrap();
        let libraries = option("libraries");
        let slots = option("tutorial-slots");
        let mut stack = crate::web_embed::libraries(libraries.as_deref()).unwrap();
        stack.projection =
            crate::web_embed::tutorial_slots(slots.as_deref(), stack.projection, &stack.libraries)
                .unwrap();
        let mut editor = crate::test_editor_with_stack(doc, stack);
        editor.command_modifier = puri::keyboard::CommandModifier::Control;
        editor.drawn_menu = option("menu").as_deref() != Some("hidden");
        Self::embedded(
            editor,
            VIEWPORT,
            Page {
                names,
                libraries,
                slots,
                tasks,
                done: 0,
            },
        )
    }

    /// Begin the page's next numbered task, which must be `id`.
    pub(super) fn task(&mut self, id: &str) {
        assert_eq!(
            self.page.tasks.get(self.page.done).map(String::as_str),
            Some(id),
            "the page's tasks are {:?}",
            self.page.tasks
        );
        self.page.done += 1;
    }

    pub(super) fn id(&self, name: &str) -> CellId {
        *self
            .page
            .names
            .get(name)
            .unwrap_or_else(|| panic!("the document names no {name:?}"))
    }

    pub(super) fn key(&self, name: &str) -> Step {
        Step::Key(self.id(name))
    }

    /// A path by the document's names, `follow` stepping into a cell.
    pub(super) fn path(&self, steps: &[&str]) -> Vec<Step> {
        steps
            .iter()
            .map(|step| match *step {
                "follow" => FOLLOW,
                name => self.key(name),
            })
            .collect()
    }

    pub(super) fn cell(&self, name: &str) -> Value {
        self.editor()
            .model
            .doc
            .cells
            .value(self.id(name))
            .cloned()
            .unwrap_or_else(|| panic!("{name} is empty"))
    }

    /// Switch off the first `depth` of peel.js's layers, as its slider does:
    /// those libraries stop drawing, and names go with the last.
    pub(super) fn peel(&mut self, depth: usize) {
        let layers = website("peel.js")
            .split("libraries: [")
            .skip(1)
            .map(|rest| {
                rest[..rest.find(']').unwrap()]
                    .split(',')
                    .map(|id| id.trim().trim_matches('"').to_owned())
                    .filter(|id| !id.is_empty())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let drawing = layers[depth..].iter().flatten().collect::<Vec<_>>();
        let libraries = self
            .page
            .libraries
            .as_deref()
            .expect("the page's libraries");
        let projections = libraries
            .split(',')
            .filter(|id| drawing.iter().any(|drawn| drawn.as_str() == *id))
            .collect::<Vec<_>>()
            .join(",");
        let stack = crate::web_embed::peeled(
            Some(libraries),
            &projections,
            self.page.slots.as_deref(),
            depth < layers.len(),
        )
        .unwrap();
        self.runner.stack_changed(stack, SCALE, VIEWPORT);
        self.present();
    }

    pub(super) fn layers() -> usize {
        website("peel.js").matches("libraries: [").count()
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            assert_eq!(
                self.done,
                self.tasks.len(),
                "untried tasks: {:?}",
                &self.tasks[self.done..]
            );
        }
    }
}
