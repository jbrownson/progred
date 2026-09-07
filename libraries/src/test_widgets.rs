//! Inspect the description captured by a real placed text handler, not a layout opcode.
use progred_display::{Layout, LineEdit, widget};
use puri::text::{FontContext, LayoutContext, TextCache, TextCtx};
use std::{cell::RefCell, rc::Rc};

pub fn line<Hover: Default>(layout: &Layout<(), Hover>) -> Option<LineEdit> {
    let Layout::Widget(widget) = layout else {
        return None;
    };
    let mut fonts = FontContext::new();
    let mut layouts = LayoutContext::new();
    let mut cache = TextCache::default();
    let captured = Rc::new(RefCell::new(None));
    let output = captured.clone();
    let measured = widget(&mut widget::Context {
        text: &mut TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            cache: &mut cache,
            scale: 1.0,
        },
        styles: &widget::style::editor(1.0),
        writable: true,
        selected: true,
        editing: None,
        spelling: None,
        initial_text: &|spelling| puri::LineEditState::new(spelling).with_cursor_at_end(),
        target: Hover::default(),
        select: Rc::new(|_| true),
        edit: Rc::new(move |_, description, _| {
            output.replace(Some(description.clone()));
            true
        }),
        primary_edit: |_| true,
    });
    let placement = puri::Placement::root(measured.extent.rect_at(puri::Point::ZERO));
    let placed = widget::place(measured, placement);
    placed
        .handler?
        .dispatch_key(&mut (), &puri::handler::KeyboardEvent::default());
    captured.take()
}
