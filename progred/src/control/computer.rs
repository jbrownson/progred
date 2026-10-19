//! What a session's agent can do, named and shaped as in Anthropic's
//! computer-use tool so a model's practice with that tool carries over.
//! Coordinates are window pixels written `[x, y]`; modifiers and keys are
//! text such as `"ctrl"` and `"ctrl+z"`. Actions answer `done`, a screenshot
//! answers `{png: …}`, and anything unreadable answers with an absent that
//! says why.

use super::{Action, Session};
use crate::gid_text::Binders;
use crate::libraries::Library;
use crate::libraries::{name, png, text};
use ::grap::{Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt};
use gid::{CellId, Value};
use kurbo::{Point, Vec2};
use std::{cell::RefCell, rc::Rc};
use ui_events::keyboard::{Key, Modifiers, NamedKey};
use winit::keyboard::ModifiersState;

pub const ID: CellId = CellId::from_u128(0x5e375174f4f2bc846a43783edbaac9f6);

pub mod vocabulary {
    use gid::CellId;

    pub const SCREENSHOT: CellId = CellId::from_u128(0xf8858be7690a76e4238875eb02d96f9b);
    pub const MOUSE_MOVE: CellId = CellId::from_u128(0x464371231311595fd8685c0e20408484);
    pub const LEFT_CLICK: CellId = CellId::from_u128(0xfa532a9b88dfe2b29a6d2353ae7336ad);
    pub const DOUBLE_CLICK: CellId = CellId::from_u128(0xed23bbbe455758d4de76b3ce46839528);
    pub const TRIPLE_CLICK: CellId = CellId::from_u128(0x842c938a27f3c34466c291287afa41e7);
    pub const LEFT_CLICK_DRAG: CellId = CellId::from_u128(0x2a75313587a6e4d2c5041b5bae427648);
    pub const SCROLL: CellId = CellId::from_u128(0x36558b1a70542010e3af36b279d53c77);
    pub const KEY: CellId = CellId::from_u128(0xa2f4a6245d388cc27258cd1fe301eb6c);
    pub const TYPE: CellId = CellId::from_u128(0x6ea61abeee0c635cbbf5ce4cc69d13fe);

    pub const COORDINATE: CellId = CellId::from_u128(0x68cbd4453d398a445385e93035321157);
    pub const START_COORDINATE: CellId = CellId::from_u128(0xdf86d8d5af86ae1d70c836f8af2cc049);
    pub const TEXT: CellId = CellId::from_u128(0x3df04f8d532e224cebbb7744b236a8b1);
    pub const SCROLL_DIRECTION: CellId = CellId::from_u128(0x25a8382b73d00eaf7a954c0f3bf1fc6b);
    pub const SCROLL_AMOUNT: CellId = CellId::from_u128(0x51d40653f2f5b8ebb25fb1b03007bc04);

    pub const DONE: CellId = CellId::from_u128(0x0c00fd40a0fdfd4e2e40a2b87753e86a);
    pub const MESSAGE: CellId = CellId::from_u128(0xa1d93183e9fcc88937bc507b29e79625);

    pub const OUTSIDE_WINDOW: CellId = CellId::from_u128(0x11ba598d7675d579c75dc82a615929a9);
    pub const NOT_A_COORDINATE: CellId = CellId::from_u128(0xf24d848b229976b65afba4bc381e0f25);
    pub const UNKNOWN_KEY: CellId = CellId::from_u128(0x49de5bfe594b718ab7bdcf904ccf9835);
    pub const NOT_TEXT: CellId = CellId::from_u128(0xdcbc45b4d32bbf167ebeea600390d57e);
    pub const NOT_A_DIRECTION: CellId = CellId::from_u128(0xc57d742a67fe25754ce1c1bc52fbdaf6);
    pub const NOT_AN_AMOUNT: CellId = CellId::from_u128(0x149ec585ecf2a0b738959a668f3bbf8b);
    pub const INVALID_REQUEST: CellId = CellId::from_u128(0x7f6e2ebbfa17da23a3a91a3dff57605e);
    pub const SCREENSHOT_FAILED: CellId = CellId::from_u128(0x3c88097f292b8995c09310116cdb9cf9);
}

/// The library's cells and their names. The tool's own spellings serve as
/// the names, and as the binders requests use.
const NAMES: [(CellId, &str); 24] = {
    use vocabulary::*;
    [
        (SCREENSHOT, "screenshot"),
        (MOUSE_MOVE, "mouse_move"),
        (LEFT_CLICK, "left_click"),
        (DOUBLE_CLICK, "double_click"),
        (TRIPLE_CLICK, "triple_click"),
        (LEFT_CLICK_DRAG, "left_click_drag"),
        (SCROLL, "scroll"),
        (KEY, "key"),
        (TYPE, "type"),
        (COORDINATE, "coordinate"),
        (START_COORDINATE, "start_coordinate"),
        (TEXT, "text"),
        (SCROLL_DIRECTION, "scroll_direction"),
        (SCROLL_AMOUNT, "scroll_amount"),
        (DONE, "done"),
        (MESSAGE, "message"),
        (OUTSIDE_WINDOW, "outside_the_window"),
        (NOT_A_COORDINATE, "not_a_coordinate"),
        (UNKNOWN_KEY, "unknown_key"),
        (NOT_TEXT, "not_text"),
        (NOT_A_DIRECTION, "not_a_scroll_direction"),
        (NOT_AN_AMOUNT, "not_a_scroll_amount"),
        (INVALID_REQUEST, "invalid_request"),
        (SCREENSHOT_FAILED, "screenshot_failed"),
    ]
};

pub(super) fn library<W: crate::display::Host>(session: &Rc<RefCell<Session>>) -> Library<W> {
    name::named_library(ID, "computer", NAMES, functions(session))
}

/// Binders for everything a request or reply is likely to mention: the
/// library's own cells, and the cells its values and absents are built from.
pub(super) fn binders() -> Binders {
    NAMES
        .into_iter()
        .chain([
            (::grap::vocabulary::FUNCTION, "function"),
            (png::vocabulary::PNG, "png"),
            (::grap::f64::F64, "f64"),
            (::grap::absent::ABSENT, "absent"),
            (::grap::absent::MISSING_ARGUMENT, "missing_argument"),
            (::grap::absent::MISSING_CELL, "missing_cell"),
            (::grap::absent::NOT_CALLABLE, "not_callable"),
            (::grap::absent::CELL, "cell"),
            (::grap::absent::VALUE, "value"),
        ])
        .map(|(cell, spelling)| (spelling.to_owned(), cell))
        .collect()
}

/// A document whose binders are the vocabulary, for clients to spell
/// requests with.
pub(super) fn vocabulary_document(binders: &Binders) -> String {
    crate::gid_text::print(
        &gid::Document {
            root: Some(Value::list(binders.values().map(|cell| Value::from(*cell)))),
            ..gid::Document::default()
        },
        binders,
    )
}

pub(super) fn invalid_request(message: String) -> Value {
    ::grap::absent::with_detail(
        vocabulary::INVALID_REQUEST,
        vocabulary::MESSAGE,
        text::value(message),
    )
}

fn functions(session: &Rc<RefCell<Session>>) -> ForeignFunctions {
    use vocabulary::*;
    ForeignFunctions::default()
        .register(SCREENSHOT, screenshot(session.clone()))
        .register(
            MOUSE_MOVE,
            action(session, [COORDINATE], |arguments| {
                Ok(Action::Move(arguments.coordinate(COORDINATE)?))
            }),
        )
        .register(LEFT_CLICK, action(session, [COORDINATE], click(1)))
        .register(DOUBLE_CLICK, action(session, [COORDINATE], click(2)))
        .register(TRIPLE_CLICK, action(session, [COORDINATE], click(3)))
        .register(
            LEFT_CLICK_DRAG,
            action(session, [START_COORDINATE, COORDINATE], |arguments| {
                Ok(Action::Drag {
                    from: arguments.coordinate(START_COORDINATE)?,
                    to: arguments.coordinate(COORDINATE)?,
                    held: arguments.held()?,
                })
            }),
        )
        .register(
            SCROLL,
            action(
                session,
                [COORDINATE, SCROLL_DIRECTION, SCROLL_AMOUNT],
                |arguments| {
                    Ok(Action::Scroll {
                        at: arguments.coordinate(COORDINATE)?,
                        lines: arguments.lines()?,
                        held: arguments.held()?,
                    })
                },
            ),
        )
        .register(
            KEY,
            action(session, [TEXT], |arguments| {
                let (key, modifiers) = arguments.required(
                    TEXT,
                    |value| text::read(value).and_then(key_combination),
                    UNKNOWN_KEY,
                )?;
                Ok(Action::Key(key, modifiers))
            }),
        )
        .register(
            TYPE,
            action(session, [TEXT], |arguments| {
                Ok(Action::Type(arguments.required(
                    TEXT,
                    |value| text::read(value).map(str::to_owned),
                    NOT_TEXT,
                )?))
            }),
        )
}

fn click(count: usize) -> impl Fn(&mut Arguments) -> Result<Action, Stop> {
    move |arguments| {
        Ok(Action::Click {
            at: arguments.coordinate(vocabulary::COORDINATE)?,
            count,
            held: arguments.held()?,
        })
    }
}

/// A function that reads its arguments into an action and does it, answering
/// `done`, or the absent that says why it couldn't.
fn action<const N: usize>(
    session: &Rc<RefCell<Session>>,
    required: [CellId; N],
    read: impl Fn(&mut Arguments) -> Result<Action, Stop> + 'static,
) -> ForeignFunction {
    let session = session.clone();
    ForeignFunction::from_value(move |context, call, environment| {
        match read(&mut Arguments {
            context,
            call,
            environment,
        }) {
            Ok(action) => Ok(context
                .effect(|| session.borrow_mut().perform(action))
                .map_or_else(::grap::absent::value, |()| Value::from(vocabulary::DONE))),
            Err(Stop::Absent(reason)) => Ok(reason),
            Err(Stop::Halt(halt)) => Err(halt),
        }
    })
    .parameters(required)
}

fn screenshot(session: Rc<RefCell<Session>>) -> ForeignFunction {
    ForeignFunction::from_value(move |_, _, _| {
        Ok(session.borrow_mut().screenshot().map_or_else(
            |message| {
                ::grap::absent::with_detail(
                    vocabulary::SCREENSHOT_FAILED,
                    vocabulary::MESSAGE,
                    text::value(message),
                )
            },
            png::value,
        ))
    })
}

/// Why reading a call's arguments stopped: evaluation halted, or an argument
/// is missing or unreadable, which the call answers with.
enum Stop {
    Halt(Halt),
    Absent(Value),
}

impl From<Halt> for Stop {
    fn from(halt: Halt) -> Self {
        Self::Halt(halt)
    }
}

struct Arguments<'a, 'context> {
    context: &'a mut Context<'context>,
    call: &'a Expression,
    environment: &'a Environment,
}

impl Arguments<'_, '_> {
    fn value(&mut self, parameter: CellId) -> Result<Option<Value>, Halt> {
        self.context
            .field(self.call, parameter)
            .map(|expression| self.context.eval_to_value(expression, self.environment))
            .transpose()
    }

    fn required<T>(
        &mut self,
        parameter: CellId,
        read: impl FnOnce(&Value) -> Option<T>,
        invalid: CellId,
    ) -> Result<T, Stop> {
        let value = self.value(parameter)?.ok_or_else(|| {
            Stop::Absent(::grap::absent::with_detail(
                ::grap::absent::MISSING_ARGUMENT,
                ::grap::absent::CELL,
                Value::from(parameter),
            ))
        })?;
        read(&value).ok_or_else(|| Stop::Absent(::grap::absent::value(invalid)))
    }

    fn coordinate(&mut self, parameter: CellId) -> Result<Point, Stop> {
        self.required(parameter, coordinate, vocabulary::NOT_A_COORDINATE)
    }

    /// The modifiers held through a pointer action: none unless named.
    fn held(&mut self) -> Result<ModifiersState, Stop> {
        self.value(vocabulary::TEXT)?
            .map_or(Ok(ModifiersState::empty()), |value| {
                text::read(&value)
                    .and_then(held_modifiers)
                    .ok_or_else(|| Stop::Absent(::grap::absent::value(vocabulary::UNKNOWN_KEY)))
            })
    }

    /// How far a scroll turns the wheel, in lines.
    fn lines(&mut self) -> Result<Vec2, Stop> {
        let direction = self.required(
            vocabulary::SCROLL_DIRECTION,
            |value| text::read(value).and_then(direction),
            vocabulary::NOT_A_DIRECTION,
        )?;
        let amount = self.required(
            vocabulary::SCROLL_AMOUNT,
            |value| {
                crate::libraries::f64::read(value)
                    .filter(|amount| amount.is_finite() && *amount >= 0.0)
            },
            vocabulary::NOT_AN_AMOUNT,
        )?;
        Ok(direction * amount)
    }
}

/// `[x, y]`: two finite numbers.
fn coordinate(value: &Value) -> Option<Point> {
    match value
        .as_list()?
        .values()
        .map(crate::libraries::f64::read)
        .collect::<Option<Vec<_>>>()?
        .as_slice()
    {
        [x, y] if x.is_finite() && y.is_finite() => Some(Point::new(*x, *y)),
        _ => None,
    }
}

/// One wheel line in a direction. A positive line scrolls up or left, as
/// the wheel reports it.
fn direction(name: &str) -> Option<Vec2> {
    match name {
        "up" => Some(Vec2::new(0.0, 1.0)),
        "down" => Some(Vec2::new(0.0, -1.0)),
        "left" => Some(Vec2::new(1.0, 0.0)),
        "right" => Some(Vec2::new(-1.0, 0.0)),
        _ => None,
    }
}

/// `+`-joined modifier names, such as `ctrl+shift`.
fn held_modifiers(names: &str) -> Option<ModifiersState> {
    names
        .split('+')
        .filter(|name| !name.is_empty())
        .try_fold(ModifiersState::empty(), |held, name| {
            Some(held | modifier(name)?)
        })
}

fn modifier(name: &str) -> Option<ModifiersState> {
    match name.to_ascii_lowercase().as_str() {
        "ctrl" | "control" => Some(ModifiersState::CONTROL),
        "shift" => Some(ModifiersState::SHIFT),
        "alt" | "option" => Some(ModifiersState::ALT),
        "super" | "cmd" | "command" | "meta" => Some(ModifiersState::SUPER),
        _ => None,
    }
}

/// A key and its modifiers, written as the tool writes them: `Return`,
/// `ctrl+z`, `shift+Tab`.
fn key_combination(combination: &str) -> Option<(Key, Modifiers)> {
    let (held, key) = combination.rsplit_once('+').unwrap_or(("", combination));
    Some((
        key_named(key)?,
        ui_events_winit::keyboard::from_winit_modifier_state(held_modifiers(held)?),
    ))
}

fn key_named(name: &str) -> Option<Key> {
    use NamedKey::*;
    let named = |key| Some(Key::Named(key));
    match name.to_ascii_lowercase().as_str() {
        "return" | "enter" | "kp_enter" => named(Enter),
        "tab" => named(Tab),
        "escape" | "esc" => named(Escape),
        "backspace" => named(Backspace),
        "delete" | "del" => named(Delete),
        "insert" => named(Insert),
        "left" => named(ArrowLeft),
        "right" => named(ArrowRight),
        "up" => named(ArrowUp),
        "down" => named(ArrowDown),
        "home" => named(Home),
        "end" => named(End),
        "page_up" | "pageup" | "prior" => named(PageUp),
        "page_down" | "pagedown" | "next" => named(PageDown),
        "f1" => named(F1),
        "f2" => named(F2),
        "f3" => named(F3),
        "f4" => named(F4),
        "f5" => named(F5),
        "f6" => named(F6),
        "f7" => named(F7),
        "f8" => named(F8),
        "f9" => named(F9),
        "f10" => named(F10),
        "f11" => named(F11),
        "f12" => named(F12),
        "space" => Some(Key::Character(" ".into())),
        "plus" => Some(Key::Character("+".into())),
        "minus" => Some(Key::Character("-".into())),
        _ => (name.chars().count() == 1).then(|| Key::Character(name.into())),
    }
}

#[cfg(test)]
mod tests;
