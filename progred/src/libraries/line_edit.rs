//! The stock Rust line-edit control shared by atomic-value libraries.
//! A library supplies spelling, affixes, and a conversion callback;
//! the native widget composes Puri text, painting, and handlers directly.

use crate::libraries::{Library, name};
use gid::{Cells, Value};

pub const ID: gid::CellId = gid::CellId::from_u128(0x26b5394bf7beb5dce00140f9e03bc465);
use crate::display::{Layout, LineEdit, LineUpdate, TextFamily, line_edit};
use std::rc::Rc;

pub fn native(update: impl Fn(&str, Option<&Value>) -> Option<Value> + 'static) -> LineUpdate {
    Rc::new(move |_, spelling, current| update(spelling, current))
}

#[cfg(test)]
pub fn grap(function: Value) -> LineUpdate {
    Rc::new(move |env, spelling, current| {
        let arguments =
            std::iter::once((vocabulary::INPUT, crate::libraries::text::value(spelling)))
                .chain(
                    current
                        .cloned()
                        .map(|current| (vocabulary::CURRENT, current)),
                )
                .collect::<Vec<_>>();
        let (result, _) = env.apply(&function, &arguments);
        (!crate::libraries::absent::is_absent(&result)).then_some(result)
    })
}

pub mod vocabulary {
    use gid::CellId;

    /// Arguments passed to an atomic line editor's write-back rule.
    pub const CURRENT: CellId = CellId::from_u128(0x0e6a49d1c78325bfa9231c05e84d67fb);
    pub const INPUT: CellId = CellId::from_u128(0xd58c17f3402b96ea6f0e4a2b91c738d5);
}

pub fn layout(
    text: impl Into<String>,
    update: LineUpdate,
    prefix: impl Into<String>,
    suffix: impl Into<String>,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    layout_with_family(text, update, prefix, suffix, TextFamily::SystemUi)
}

pub fn layout_with_family(
    text: impl Into<String>,
    update: LineUpdate,
    prefix: impl Into<String>,
    suffix: impl Into<String>,
    family: TextFamily,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    line_edit(description(
        text,
        None::<String>,
        update,
        prefix,
        suffix,
        family,
    ))
}

pub fn description(
    text: impl Into<String>,
    placeholder: Option<impl Into<String>>,
    update: LineUpdate,
    prefix: impl Into<String>,
    suffix: impl Into<String>,
    family: TextFamily,
) -> LineEdit {
    LineEdit {
        text: text.into(),
        placeholder: placeholder.map(Into::into),
        update,
        prefix: prefix.into(),
        suffix: suffix.into(),
        family,
    }
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::CURRENT, name::record("current", []));
    cells.set_value(vocabulary::INPUT, name::record("input", []));
    Library::named(
        ID,
        "line edit",
        crate::libraries::Definitions::from_parts(cells, Default::default()),
        crate::display::partial(|_| None),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_handler_captures_the_current_line_description() {
        let update = native(|spelling, _| Some(crate::libraries::text::value(spelling)));
        let line = description(
            "42",
            None::<String>,
            update.clone(),
            "(",
            ")",
            TextFamily::SystemUi,
        );
        assert_eq!(line.text, "42");
        assert_eq!(line.placeholder, None);
        assert!(Rc::ptr_eq(&line.update, &update));
        assert_eq!(line.prefix, "(");
        assert_eq!(line.suffix, ")");
        assert_eq!(line.family, TextFamily::SystemUi);
    }

    #[test]
    fn layout_can_request_a_monospace_editor() {
        let line = description(
            "b4e0fe",
            None::<String>,
            native(|_, _| None),
            "#",
            "",
            TextFamily::Monospace,
        );
        assert_eq!(line.family, TextFamily::Monospace);
    }

    #[test]
    fn layout_exposes_a_placeholder() {
        let line = description(
            "",
            Some("λ"),
            native(|_, _| None),
            "",
            "",
            TextFamily::SystemUi,
        );
        assert_eq!(line.placeholder.as_deref(), Some("λ"));
    }
}
