//! Bare editable text: parley's `PlainEditor` as a caller-owned value,
//! rendered as just the text — glyphs, selection, cursor, IME preedit.
//! Chrome (frame, padding, focus ring, minimum width) is composition
//! via `pad`/`decorate`. Pointer policy stays with the wrapping layer
//! through the state's pointer methods; the widget registers keyboard
//! and IME dispatch only while focused.
//!
//! The editor's layout is lazy and refreshing needs `&mut`, so the
//! shell runs `refresh` as an explicit prep step before each pass; the
//! pass itself only reads `try_layout`.

use crate::draw::Canvas;
use crate::handler::{HasHandler, ImeEvent};
use crate::layout::{Extent, Node, leaf};
use crate::text::draw_layout;
use kurbo::{Affine, Point, Rect};
use parley::{FontContext, LayoutContext, PlainEditor};
use peniko::Brush;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};
use ui_events::pointer::PointerButton;

pub struct LineEditState {
    pub editor: PlainEditor<Brush>,
    scale: f32,
    /// Whether a drag-selection started in this field; the pure-pass
    /// translation of pointer capture.
    dragging: bool,
}

pub struct EditStyle {
    pub selection: Brush,
    pub cursor: Brush,
}

/// What an editing dispatch needs from the caller's context: the state
/// plus the measurement caches parley's driver requires.
pub struct EditCtx<'a> {
    pub state: &'a mut LineEditState,
    pub fonts: &'a mut FontContext,
    pub layouts: &'a mut LayoutContext<Brush>,
}

impl LineEditState {
    pub fn new(text: &str, font_size: f32) -> Self {
        let mut editor = PlainEditor::new(font_size);
        editor.set_text(text);
        editor.set_width(None);
        Self {
            editor,
            scale: 1.0,
            dragging: false,
        }
    }

    /// Refresh the lazy layout; the shell calls this before each pass.
    pub fn refresh(
        &mut self,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        scale: f32,
    ) {
        self.scale = scale;
        self.editor.set_scale(scale);
        let _ = self.editor.layout(fonts, layouts);
    }

    /// Keyboard editing per the vello_editor semantics. Returns whether
    /// the event was handled; unhandled keys fall through to whatever
    /// the caller composed behind this widget.
    pub fn handle_key(
        &mut self,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        event: &KeyboardEvent,
    ) -> bool {
        if !event.state.is_down() || self.editor.is_composing() {
            return false;
        }
        let action_mod = if cfg!(target_os = "macos") {
            event.modifiers.meta()
        } else {
            event.modifiers.ctrl()
        };
        let shift = event.modifiers.shift();
        let mut drv = self.editor.driver(fonts, layouts);
        match &event.key {
            #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
            Key::Character(c) if action_mod && matches!(c.to_lowercase().as_str(), "c" | "x" | "v") => {
                use clipboard_rs::{Clipboard, ClipboardContext};
                if let Ok(cb) = ClipboardContext::new() {
                    match c.to_lowercase().as_str() {
                        "c" => {
                            if let Some(text) = drv.editor.selected_text() {
                                cb.set_text(text.to_owned()).ok();
                            }
                        }
                        "x" => {
                            if let Some(text) = drv.editor.selected_text() {
                                cb.set_text(text.to_owned()).ok();
                                drv.delete_selection();
                            }
                        }
                        "v" => {
                            let text = cb.get_text().unwrap_or_default();
                            drv.insert_or_replace_selection(&text);
                        }
                        _ => {}
                    }
                }
                true
            }
            Key::Character(c) if action_mod && c.to_lowercase() == "a" => {
                if shift {
                    drv.collapse_selection();
                } else {
                    drv.select_all();
                }
                true
            }
            Key::Named(NamedKey::ArrowLeft) => {
                match (action_mod, shift) {
                    (true, true) => drv.select_word_left(),
                    (true, false) => drv.move_word_left(),
                    (false, true) => drv.select_left(),
                    (false, false) => drv.move_left(),
                }
                true
            }
            Key::Named(NamedKey::ArrowRight) => {
                match (action_mod, shift) {
                    (true, true) => drv.select_word_right(),
                    (true, false) => drv.move_word_right(),
                    (false, true) => drv.select_right(),
                    (false, false) => drv.move_right(),
                }
                true
            }
            Key::Named(NamedKey::Home) => {
                if shift {
                    drv.select_to_line_start();
                } else {
                    drv.move_to_line_start();
                }
                true
            }
            Key::Named(NamedKey::End) => {
                if shift {
                    drv.select_to_line_end();
                } else {
                    drv.move_to_line_end();
                }
                true
            }
            // Delete keys decline on an empty buffer — a no-op edit is
            // not a handled edit — so the caller can interpret them
            // (delete the element, join, whatever).
            Key::Named(NamedKey::Delete) if drv.editor.text() != "" => {
                if action_mod {
                    drv.delete_word();
                } else {
                    drv.delete();
                }
                true
            }
            Key::Named(NamedKey::Backspace) if drv.editor.text() != "" => {
                if action_mod {
                    drv.backdelete_word();
                } else {
                    drv.backdelete();
                }
                true
            }
            Key::Character(c) if !action_mod => {
                drv.insert_or_replace_selection(c);
                true
            }
            _ => false,
        }
    }

    pub fn handle_ime(
        &mut self,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        event: &ImeEvent,
    ) -> bool {
        let mut drv = self.editor.driver(fonts, layouts);
        match event {
            ImeEvent::Commit(text) => {
                drv.insert_or_replace_selection(text);
                true
            }
            ImeEvent::Preedit(text, cursor) => {
                if text.is_empty() {
                    drv.clear_compose();
                } else {
                    drv.set_compose(text, *cursor);
                }
                true
            }
            ImeEvent::Disabled => {
                drv.clear_compose();
                true
            }
            ImeEvent::Enabled => true,
        }
    }

    /// Pointer positioning for the wrapping layer; `point` is in the
    /// text's local coordinates (layout top-left origin).
    pub fn pointer_down(
        &mut self,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        point: Point,
        shift: bool,
        count: u8,
    ) {
        if self.editor.is_composing() {
            return;
        }
        self.dragging = true;
        let (x, y) = (point.x as f32, point.y as f32);
        let mut drv = self.editor.driver(fonts, layouts);
        match count {
            2 => drv.select_word_at_point(x, y),
            3 => drv.select_hard_line_at_point(x, y),
            _ => {
                if shift {
                    drv.shift_click_extension(x, y);
                } else {
                    drv.move_to_point(x, y);
                }
            }
        }
    }

    /// Drag-extend the selection; `point` in local coordinates. Only
    /// acts while a drag started in this field.
    pub fn pointer_move(
        &mut self,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        point: Point,
    ) -> bool {
        (self.dragging && !self.editor.is_composing()) && {
            self.editor
                .driver(fonts, layouts)
                .extend_selection_to_point(point.x as f32, point.y as f32);
            true
        }
    }

    /// Ends a drag; returns whether one was in progress.
    pub fn pointer_up(&mut self) -> bool {
        std::mem::replace(&mut self.dragging, false)
    }

    /// Cursor rect in local coordinates, for drawing and IME placement.
    pub fn cursor_rect(&self) -> Option<Rect> {
        self.editor
            .cursor_geometry(1.5 * self.scale)
            .map(|bb| Rect::new(bb.x0, bb.y0, bb.x1, bb.y1))
    }
}

/// Bare editable text sized to its content. Registers keyboard and IME
/// dispatch (through `with`) only while focused; pointer wiring is the
/// caller's, via the state's pointer methods and its own settled rect.
pub fn text_edit<C: 'static, P: Canvas + HasHandler<C>>(
    state: &LineEditState,
    focused: bool,
    style: &EditStyle,
    with: impl for<'a> Fn(&'a mut C) -> EditCtx<'a> + Clone + 'static,
) -> Node<P> {
    let layout = state.editor.try_layout().cloned();
    let (extent, layout_baseline) = layout
        .as_ref()
        .and_then(|layout| {
            let metrics = *layout.lines().next()?.metrics();
            let baseline = metrics.baseline as f64;
            Some((
                Extent {
                    width: metrics.advance as f64,
                    ascent: baseline,
                    descent: layout.height() as f64 - baseline,
                },
                baseline,
            ))
        })
        .unwrap_or((Extent::default(), 0.0));

    let selection: Vec<Rect> = focused
        .then(|| {
            let mut rects = Vec::new();
            state
                .editor
                .selection_geometry_with(|bb, _| rects.push(Rect::new(bb.x0, bb.y0, bb.x1, bb.y1)));
            rects
        })
        .unwrap_or_default();
    let cursor = focused.then(|| state.cursor_rect()).flatten();
    let selection_brush = style.selection.clone();
    let cursor_brush = style.cursor.clone();

    leaf(extent, move |p: &mut P, at: Point| {
        let transform = Affine::translate((at.x, at.y - layout_baseline));
        for rect in &selection {
            p.fill(*rect, selection_brush.clone(), transform);
        }
        if let Some(layout) = &layout {
            draw_layout(p, layout, transform);
        }
        if let Some(cursor) = cursor {
            p.fill(cursor, cursor_brush.clone(), transform);
        }
        if focused {
            let text_origin = Point::new(at.x, at.y - layout_baseline);
            let with_key = with.clone();
            p.handler().on_key(move |ctx, event| {
                let EditCtx {
                    state,
                    fonts,
                    layouts,
                } = with_key(ctx);
                state.handle_key(fonts, layouts, event)
            });
            let with_move = with.clone();
            p.handler().on_pointer_move(move |ctx, update| {
                update.current.buttons.contains(PointerButton::Primary) && {
                    let EditCtx {
                        state,
                        fonts,
                        layouts,
                    } = with_move(ctx);
                    state.pointer_move(
                        fonts,
                        layouts,
                        Point::new(
                            update.current.position.x - text_origin.x,
                            update.current.position.y - text_origin.y,
                        ),
                    )
                }
            });
            let with_up = with.clone();
            p.handler().on_pointer_up(move |ctx, _| with_up(ctx).state.pointer_up());
            p.handler().on_ime(move |ctx, event| {
                let EditCtx {
                    state,
                    fonts,
                    layouts,
                } = with(ctx);
                state.handle_ime(fonts, layouts, event)
            });
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ui_events::keyboard::{KeyState, Modifiers};

    fn contexts() -> (FontContext, LayoutContext<Brush>) {
        (FontContext::new(), LayoutContext::new())
    }

    fn key_event(key: Key, modifiers: Modifiers) -> KeyboardEvent {
        KeyboardEvent {
            key,
            modifiers,
            state: KeyState::Down,
            ..Default::default()
        }
    }

    fn press(
        state: &mut LineEditState,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        key: Key,
        modifiers: Modifiers,
    ) -> bool {
        state.handle_key(fonts, layouts, &key_event(key, modifiers))
    }

    #[test]
    fn typing_moving_and_deleting() {
        let (mut fonts, mut layouts) = contexts();
        let mut state = LineEditState::new("", 16.0);

        for c in ["h", "i", "!"] {
            assert!(press(
                &mut state,
                &mut fonts,
                &mut layouts,
                Key::Character(c.into()),
                Modifiers::empty(),
            ));
        }
        assert!(state.editor.text() == "hi!");

        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Backspace),
            Modifiers::empty(),
        ));
        assert!(state.editor.text() == "hi");

        press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::ArrowLeft),
            Modifiers::empty(),
        );
        press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character("a".into()),
            Modifiers::empty(),
        );
        assert!(state.editor.text() == "hai");

        // Unhandled keys decline so they can fall through the handler.
        assert!(!press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Escape),
            Modifiers::empty(),
        ));
    }

    #[test]
    fn delete_keys_decline_on_an_empty_buffer() {
        let (mut fonts, mut layouts) = contexts();
        let mut state = LineEditState::new("x", 16.0);

        press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::End),
            Modifiers::empty(),
        );
        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Backspace),
            Modifiers::empty(),
        ));
        assert!(state.editor.text() == "");
        for key in [NamedKey::Backspace, NamedKey::Delete] {
            assert!(!press(
                &mut state,
                &mut fonts,
                &mut layouts,
                Key::Named(key),
                Modifiers::empty(),
            ));
        }
    }

    #[test]
    fn selection_replaces_on_insert() {
        let (mut fonts, mut layouts) = contexts();
        let mut state = LineEditState::new("abc", 16.0);

        press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::End),
            Modifiers::empty(),
        );
        press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::ArrowLeft),
            Modifiers::SHIFT,
        );
        assert_eq!(state.editor.selected_text(), Some("c"));

        press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character("z".into()),
            Modifiers::empty(),
        );
        assert!(state.editor.text() == "abz");
    }

    #[test]
    fn drag_extends_selection_until_released() {
        let (mut fonts, mut layouts) = contexts();
        let mut state = LineEditState::new("hello world", 16.0);
        state.refresh(&mut fonts, &mut layouts, 1.0);

        state.pointer_down(&mut fonts, &mut layouts, Point::new(0.0, 5.0), false, 1);
        assert!(state.pointer_move(&mut fonts, &mut layouts, Point::new(10_000.0, 5.0)));
        assert_eq!(state.editor.selected_text(), Some("hello world"));

        assert!(state.pointer_up());
        assert!(!state.pointer_up());
        assert!(!state.pointer_move(&mut fonts, &mut layouts, Point::new(0.0, 5.0)));
    }

    #[test]
    fn ime_compose_then_commit() {
        let (mut fonts, mut layouts) = contexts();
        let mut state = LineEditState::new("", 16.0);

        state.handle_ime(
            &mut fonts,
            &mut layouts,
            &ImeEvent::Preedit("ni".into(), Some((2, 2))),
        );
        assert!(state.editor.is_composing());
        // Keys decline while composing.
        assert!(!press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character("x".into()),
            Modifiers::empty(),
        ));

        state.handle_ime(&mut fonts, &mut layouts, &ImeEvent::Preedit("".into(), None));
        assert!(!state.editor.is_composing());
        state.handle_ime(&mut fonts, &mut layouts, &ImeEvent::Commit("你".into()));
        assert!(state.editor.text() == "你");
    }
}
