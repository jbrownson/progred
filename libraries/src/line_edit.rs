//! The stock Rust line-edit control shared by atomic-value libraries.
//! A library supplies spelling, affixes, and a conversion callback;
//! the native widget composes Puri text, painting, and handlers directly.

use crate::{Library, name};
use gid::{Cells, Value};

pub const ID: gid::CellId = gid::CellId::from_u128(0x26b5394bf7beb5dce00140f9e03bc465);
use progred_display::{Layout, LineEdit, LineUpdate, TextFamily, line_edit};
use std::rc::Rc;

pub fn native(update: impl Fn(&str, Option<&Value>) -> Option<Value> + 'static) -> LineUpdate {
    Rc::new(move |_, spelling, current| update(spelling, current))
}

pub fn grap(function: Value) -> LineUpdate {
    Rc::new(move |env, spelling, current| {
        let arguments = std::iter::once((vocabulary::INPUT, crate::text::value(spelling)))
            .chain(
                current
                    .cloned()
                    .map(|current| (vocabulary::CURRENT, current)),
            )
            .collect::<Vec<_>>();
        let (result, _) = env.apply(&function, &arguments);
        (!crate::absent::is_absent(&result)).then_some(result)
    })
}

pub mod vocabulary {
    use gid::CellId;

    /// Arguments passed to an atomic line editor's write-back rule.
    pub const CURRENT: CellId = CellId::from_u128(0x0e6a49d1c78325bfa9231c05e84d67fb);
    pub const INPUT: CellId = CellId::from_u128(0xd58c17f3402b96ea6f0e4a2b91c738d5);
}

pub fn layout<World: 'static, Hover: Clone + PartialEq + 'static>(
    text: impl Into<String>,
    update: LineUpdate,
    prefix: impl Into<String>,
    suffix: impl Into<String>,
) -> Layout<World, Hover> {
    layout_with_family(text, update, prefix, suffix, TextFamily::SystemUi)
}

pub fn layout_with_family<World: 'static, Hover: Clone + PartialEq + 'static>(
    text: impl Into<String>,
    update: LineUpdate,
    prefix: impl Into<String>,
    suffix: impl Into<String>,
    family: TextFamily,
) -> Layout<World, Hover> {
    description(text, None::<String>, update, prefix, suffix, family)
}

pub fn layout_with_placeholder<World: 'static, Hover: Clone + PartialEq + 'static>(
    text: impl Into<String>,
    placeholder: Option<impl Into<String>>,
    update: LineUpdate,
    prefix: impl Into<String>,
    suffix: impl Into<String>,
) -> Layout<World, Hover> {
    description(
        text,
        placeholder,
        update,
        prefix,
        suffix,
        TextFamily::SystemUi,
    )
}

fn description<World: 'static, Hover: Clone + PartialEq + 'static>(
    text: impl Into<String>,
    placeholder: Option<impl Into<String>>,
    update: LineUpdate,
    prefix: impl Into<String>,
    suffix: impl Into<String>,
    family: TextFamily,
) -> Layout<World, Hover> {
    line_edit(LineEdit {
        text: text.into(),
        placeholder: placeholder.map(Into::into),
        update,
        prefix: prefix.into(),
        suffix: suffix.into(),
        family,
    })
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::CURRENT, name::record("current", []));
    cells.set_value(vocabulary::INPUT, name::record("input", []));
    Library::named(
        ID,
        "line edit",
        crate::Definitions::from_parts(cells, Default::default()),
        progred_display::partial(|_| None),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_handler_captures_the_current_line_description() {
        let update = native(|spelling, _| Some(crate::text::value(spelling)));
        let layout = layout::<(), ()>("42", update.clone(), "(", ")");
        let Some(line) = crate::test_widgets::line(&layout) else {
            panic!("stock line-edit layout")
        };
        assert_eq!(line.text, "42");
        assert_eq!(line.placeholder, None);
        assert!(Rc::ptr_eq(&line.update, &update));
        assert_eq!(line.prefix, "(");
        assert_eq!(line.suffix, ")");
        assert_eq!(line.family, TextFamily::SystemUi);
    }

    #[test]
    fn layout_can_request_a_monospace_editor() {
        let Some(line) = crate::test_widgets::line(&layout_with_family::<(), ()>(
            "b4e0fe",
            native(|_, _| None),
            "#",
            "",
            TextFamily::Monospace,
        )) else {
            panic!("stock line-edit layout")
        };
        assert_eq!(line.family, TextFamily::Monospace);
    }

    #[test]
    fn layout_exposes_a_placeholder() {
        let Some(line) = crate::test_widgets::line(&layout_with_placeholder::<(), ()>(
            "",
            Some("λ"),
            native(|_, _| None),
            "",
            "",
        )) else {
            panic!("stock line-edit layout")
        };
        assert_eq!(line.placeholder.as_deref(), Some("λ"));
    }
}
