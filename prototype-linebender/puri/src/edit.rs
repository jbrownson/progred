//! Bare editable text over parley, custody split at true state: the
//! caller-owned `LineEditState` holds only text, selection byte
//! offsets, and any active IME preedit. A transient `PlainEditor` is
//! constructed from that state for each pass (drawing) and each
//! dispatch (editing semantics), then discarded — parley's
//! retained-mode machinery (cached layout, dirty flag, driver-gated
//! writes) lives and dies inside those moments, while its editing
//! behavior is borrowed whole. Word- and line-anchored drag extension
//! is replayed from the gesture's origin each move rather than
//! round-tripped. Deliberately not round-tripped at all, as
//! single-line-irrelevant: cursor affinity (bidi boundaries) and the
//! vertical goal column.
//!
//! Chrome (frame, padding, focus ring, minimum width) is composition
//! via `pad`/`decorate`. Pointer policy stays with the wrapping layer
//! through the state's pointer methods; the widget registers keyboard
//! and IME dispatch only while focused.

use crate::draw::Canvas;
use crate::handler::{HasHandler, ImeEvent};
use crate::layout::{Extent, Node, leaf};
use crate::text::{TextCtx, draw_layout};
use kurbo::{Affine, Point, Rect};
use parley::style::GenericFamily;
use parley::{FontContext, LayoutContext, PlainEditor, StyleProperty};
use peniko::Brush;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};
use ui_events::pointer::PointerButton;

/// A selection as comparable byte offsets, for did-anything-move
/// checks around driver operations.
fn cursor_of(selection: &parley::Selection) -> (usize, usize) {
    (selection.anchor().index(), selection.focus().index())
}

/// An in-progress IME composition: the preedit text and the caret (or
/// highlight) the IME wants within it. Kept out of `text`, which stays
/// the base the composition will land in.
struct Preedit {
    text: String,
    cursor: Option<(usize, usize)>,
}

/// An in-progress drag-selection — the pure-pass translation of
/// pointer capture: where it started and at what click count, so each
/// move can rebuild its word or line anchor. Anchor granularity is
/// gesture state, not editor state.
#[derive(Clone, Copy)]
struct Drag {
    origin: Point,
    count: u8,
}

pub struct LineEditState {
    text: String,
    /// Selection byte offsets into `text`; equal offsets are a caret,
    /// `focus` may precede `anchor` for a backward selection.
    anchor: usize,
    focus: usize,
    preedit: Option<Preedit>,
    drag: Option<Drag>,
    font_size: f32,
    brush: Brush,
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
    pub fn new(text: &str, font_size: f32, brush: Brush) -> Self {
        Self {
            text: text.to_string(),
            anchor: 0,
            focus: 0,
            preedit: None,
            drag: None,
            font_size,
            brush,
        }
    }

    /// Start with the caret at the end, so typing appends.
    pub fn with_cursor_at_end(mut self) -> Self {
        self.cursor_to_end();
        self
    }

    /// Land the caret at the end — plain data, no contexts needed.
    pub fn cursor_to_end(&mut self) {
        self.anchor = self.text.len();
        self.focus = self.text.len();
    }

    /// The base text: what edits commit. Excludes any IME preedit.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replace the text wholesale — the caller's re-mint for external
    /// writes — keeping the selection clamped to char boundaries.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        let clamp = |mut i: usize| {
            i = i.min(self.text.len());
            while !self.text.is_char_boundary(i) {
                i -= 1;
            }
            i
        };
        self.anchor = clamp(self.anchor);
        self.focus = clamp(self.focus);
    }

    pub fn is_composing(&self) -> bool {
        self.preedit.is_some()
    }

    /// The transient parley editor this state denotes: constructed,
    /// used within one pass or one dispatch, dropped. Always returns
    /// with a clean layout.
    fn editor(
        &self,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        scale: f32,
    ) -> PlainEditor<Brush> {
        let mut editor = PlainEditor::new(self.font_size);
        editor.set_text(&self.text);
        editor.set_width(None);
        editor.set_scale(scale);
        editor
            .edit_styles()
            .insert(StyleProperty::Brush(self.brush.clone()));
        editor.edit_styles().insert(GenericFamily::SystemUi.into());
        let mut driver = editor.driver(fonts, layouts);
        driver.select_byte_range(self.anchor, self.focus);
        if let Some(preedit) = &self.preedit {
            driver.set_compose(&preedit.text, preedit.cursor);
        }
        editor
    }

    /// Read the mutated editor back into true state. Only called from
    /// compose-free paths: keys and pointers decline while composing,
    /// and IME events never touch an editor.
    fn absorb(&mut self, editor: &PlainEditor<Brush>) {
        self.text = editor.text().to_string();
        let selection = editor.raw_selection();
        self.anchor = selection.anchor().index();
        self.focus = selection.focus().index();
    }

    /// Splice `text` over the selection and collapse the caret after
    /// it. Selection offsets are always char boundaries, so this is
    /// pure string surgery.
    fn replace_selection(&mut self, text: &str) {
        let (start, end) = (self.anchor.min(self.focus), self.anchor.max(self.focus));
        self.text.replace_range(start..end, text);
        self.anchor = start + text.len();
        self.focus = self.anchor;
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
        if !event.state.is_down() || self.is_composing() {
            return false;
        }
        let action_mod = if cfg!(target_os = "macos") {
            event.modifiers.meta()
        } else {
            event.modifiers.ctrl()
        };
        let shift = event.modifiers.shift();
        let mut editor = self.editor(fonts, layouts, 1.0);
        let handled = {
            let mut drv = editor.driver(fonts, layouts);
            match &event.key {
                #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
                // Copy and cut handle only when text is actually
                // selected: with nothing to copy they decline, so the
                // caller can interpret the chord (structural copy of
                // the edited value). Paste always lands in the text.
                Key::Character(c)
                    if action_mod && matches!(c.to_lowercase().as_str(), "c" | "x" | "v") =>
                {
                    use clipboard_rs::{Clipboard, ClipboardContext};
                    let selected = drv.editor.selected_text().map(str::to_owned);
                    match (c.to_lowercase().as_str(), selected) {
                        ("c", Some(text)) => {
                            if let Ok(cb) = ClipboardContext::new() {
                                cb.set_text(text).ok();
                            }
                            true
                        }
                        ("x", Some(text)) => {
                            if let Ok(cb) = ClipboardContext::new() {
                                cb.set_text(text).ok();
                            }
                            drv.delete_selection();
                            true
                        }
                        ("v", _) => {
                            if let Ok(cb) = ClipboardContext::new() {
                                let text = cb.get_text().unwrap_or_default();
                                drv.insert_or_replace_selection(&text);
                            }
                            true
                        }
                        _ => false,
                    }
                }
                Key::Character(c) if action_mod && c.to_lowercase() == "a" => {
                    if shift {
                        drv.collapse_selection();
                    } else {
                        drv.select_all();
                    }
                    true
                }
                // Arrows handle only when the caret actually moves:
                // at the text's boundary they decline, so the caller
                // can interpret them (selection navigation).
                Key::Named(NamedKey::ArrowLeft) => {
                    let before = cursor_of(drv.editor.raw_selection());
                    match (action_mod, shift) {
                        (true, true) => drv.select_word_left(),
                        (true, false) => drv.move_word_left(),
                        (false, true) => drv.select_left(),
                        (false, false) => drv.move_left(),
                    }
                    cursor_of(drv.editor.raw_selection()) != before
                }
                Key::Named(NamedKey::ArrowRight) => {
                    let before = cursor_of(drv.editor.raw_selection());
                    match (action_mod, shift) {
                        (true, true) => drv.select_word_right(),
                        (true, false) => drv.move_word_right(),
                        (false, true) => drv.select_right(),
                        (false, false) => drv.move_right(),
                    }
                    cursor_of(drv.editor.raw_selection()) != before
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
                // Any ctrl or meta chord is a command somewhere —
                // never text, whichever of them is the action mod.
                Key::Character(c) if !(event.modifiers.ctrl() || event.modifiers.meta()) => {
                    drv.insert_or_replace_selection(c);
                    true
                }
                _ => false,
            }
        };
        if handled {
            self.absorb(&editor);
        }
        handled
    }

    /// IME events are pure state transitions: composition starts by
    /// consuming the selection, each preedit replaces the last whole,
    /// commit splices at the caret. No editor needed.
    pub fn handle_ime(&mut self, event: &ImeEvent) -> bool {
        match event {
            ImeEvent::Commit(text) => {
                self.replace_selection(text);
                self.preedit = None;
                true
            }
            ImeEvent::Preedit(text, cursor) => {
                if text.is_empty() {
                    self.preedit = None;
                } else {
                    if self.preedit.is_none() && self.anchor != self.focus {
                        self.replace_selection("");
                    }
                    let clamp = |(a, b): (usize, usize)| (a.min(text.len()), b.min(text.len()));
                    self.preedit = Some(Preedit {
                        text: text.clone(),
                        cursor: cursor.map(clamp),
                    });
                }
                true
            }
            ImeEvent::Disabled => {
                self.preedit = None;
                true
            }
            ImeEvent::Enabled => true,
        }
    }

    /// Pointer positioning for the wrapping layer; `point` is in the
    /// text's local coordinates (layout top-left origin), physical
    /// pixels, so hit-testing needs the display scale. A double click
    /// selects the word, a triple the line.
    pub fn pointer_down(
        &mut self,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        scale: f32,
        point: Point,
        shift: bool,
        count: u8,
    ) {
        if self.is_composing() {
            return;
        }
        self.drag = Some(Drag {
            origin: point,
            count,
        });
        let (x, y) = (point.x as f32, point.y as f32);
        let mut editor = self.editor(fonts, layouts, scale);
        {
            let mut drv = editor.driver(fonts, layouts);
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
        self.absorb(&editor);
    }

    /// Drag-extend the selection; `point` in local coordinates. Only
    /// acts while a drag started in this field. Word- and line-anchored
    /// drags rebuild their anchor from the gesture's origin each move,
    /// since only byte offsets round-trip the transient editors.
    pub fn pointer_move(
        &mut self,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        scale: f32,
        point: Point,
    ) -> bool {
        let Some(drag) = self.drag else {
            return false;
        };
        if self.is_composing() {
            return false;
        }
        let mut editor = self.editor(fonts, layouts, scale);
        {
            let mut drv = editor.driver(fonts, layouts);
            let (x, y) = (drag.origin.x as f32, drag.origin.y as f32);
            match drag.count {
                2 => drv.select_word_at_point(x, y),
                3 => drv.select_hard_line_at_point(x, y),
                _ => {}
            }
            drv.extend_selection_to_point(point.x as f32, point.y as f32);
        }
        self.absorb(&editor);
        true
    }

    /// Ends a drag; returns whether one was in progress.
    pub fn pointer_up(&mut self) -> bool {
        self.drag.take().is_some()
    }
}

/// Bare editable text sized to its content, drawn from a transient
/// editor built off the true state. Registers keyboard and IME
/// dispatch (through `with`) only while focused; pointer wiring is the
/// caller's, via the state's pointer methods and its own settled rect.
/// Dispatch targets the last rendered frame, which can outlive the
/// editor (deselect, then a move in the same gesture), so `with`
/// returns None when the editor is gone and the handlers decline.
pub fn text_edit<C: 'static, P: Canvas + HasHandler<C>>(
    state: &LineEditState,
    focused: bool,
    style: &EditStyle,
    tcx: &mut TextCtx,
    with: impl for<'a> Fn(&'a mut C) -> Option<EditCtx<'a>> + Clone + 'static,
) -> Node<P> {
    let scale = tcx.scale;
    let editor = state.editor(tcx.fonts, tcx.layouts, scale);
    let layout = editor.try_layout().cloned();
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
            editor
                .selection_geometry_with(|bb, _| rects.push(Rect::new(bb.x0, bb.y0, bb.x1, bb.y1)));
            rects
        })
        .unwrap_or_default();
    let cursor = focused
        .then(|| {
            editor
                .cursor_geometry(1.5 * scale)
                .map(|bb| Rect::new(bb.x0, bb.y0, bb.x1, bb.y1))
        })
        .flatten();
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
                with_key(ctx).is_some_and(
                    |EditCtx {
                         state,
                         fonts,
                         layouts,
                     }| state.handle_key(fonts, layouts, event),
                )
            });
            let with_move = with.clone();
            p.handler().on_pointer_move(move |ctx, update| {
                update.current.buttons.contains(PointerButton::Primary)
                    && with_move(ctx).is_some_and(
                        |EditCtx {
                             state,
                             fonts,
                             layouts,
                         }| {
                            state.pointer_move(
                                fonts,
                                layouts,
                                scale,
                                Point::new(
                                    update.current.position.x - text_origin.x,
                                    update.current.position.y - text_origin.y,
                                ),
                            )
                        },
                    )
            });
            let with_up = with.clone();
            p.handler().on_pointer_up(move |ctx, _| {
                with_up(ctx).is_some_and(|edit| edit.state.pointer_up())
            });
            p.handler()
                .on_ime(move |ctx, event| with(ctx).is_some_and(|edit| edit.state.handle_ime(event)));
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

    fn state(text: &str) -> LineEditState {
        LineEditState::new(text, 16.0, Brush::default())
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
        let mut state = state("");

        for c in ["h", "i", "!"] {
            assert!(press(
                &mut state,
                &mut fonts,
                &mut layouts,
                Key::Character(c.into()),
                Modifiers::empty(),
            ));
        }
        assert!(state.text() == "hi!");

        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Backspace),
            Modifiers::empty(),
        ));
        assert!(state.text() == "hi");

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
        assert!(state.text() == "hai");

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
        let mut state = state("x").with_cursor_at_end();

        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Backspace),
            Modifiers::empty(),
        ));
        assert!(state.text() == "");
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
    fn command_chords_never_insert_text() {
        let (mut fonts, mut layouts) = contexts();
        let mut state = state("x").with_cursor_at_end();
        for modifiers in [Modifiers::CONTROL, Modifiers::META] {
            assert!(!press(
                &mut state,
                &mut fonts,
                &mut layouts,
                Key::Character("q".into()),
                modifiers,
            ));
        }
        assert!(state.text() == "x");
    }

    #[test]
    fn selection_replaces_on_insert() {
        let (mut fonts, mut layouts) = contexts();
        let mut state = state("abc").with_cursor_at_end();

        press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::ArrowLeft),
            Modifiers::SHIFT,
        );
        assert_eq!((state.anchor, state.focus), (3, 2));

        press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character("z".into()),
            Modifiers::empty(),
        );
        assert!(state.text() == "abz");
    }

    #[test]
    fn boundary_arrows_decline_so_navigation_can_take_them() {
        let (mut fonts, mut layouts) = contexts();
        let mut state = state("ab");
        // Caret at 0: Left declines, Right moves.
        assert!(!press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::ArrowLeft),
            Modifiers::empty(),
        ));
        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::ArrowRight),
            Modifiers::empty(),
        ));
        let mut state = state.with_cursor_at_end();
        assert!(!press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::ArrowRight),
            Modifiers::empty(),
        ));
        assert!(!press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::ArrowRight),
            Modifiers::SHIFT,
        ));
        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::ArrowLeft),
            Modifiers::empty(),
        ));
    }

    #[test]
    fn cursor_end_is_immediate_and_clicks_still_place() {
        let (mut fonts, mut layouts) = contexts();
        let mut seeded = state("abc").with_cursor_at_end();
        press(
            &mut seeded,
            &mut fonts,
            &mut layouts,
            Key::Character("z".into()),
            Modifiers::empty(),
        );
        assert!(seeded.text() == "abcz");

        let mut clicked = state("abc").with_cursor_at_end();
        clicked.pointer_down(&mut fonts, &mut layouts, 1.0, Point::new(0.0, 5.0), false, 1);
        press(
            &mut clicked,
            &mut fonts,
            &mut layouts,
            Key::Character("z".into()),
            Modifiers::empty(),
        );
        assert!(clicked.text() == "zabc");

        // Landing the caret at the end again is plain data.
        clicked.cursor_to_end();
        press(
            &mut clicked,
            &mut fonts,
            &mut layouts,
            Key::Character("y".into()),
            Modifiers::empty(),
        );
        assert!(clicked.text() == "zabcy");
    }

    #[test]
    fn double_click_drag_extends_by_words() {
        let (mut fonts, mut layouts) = contexts();
        let mut state = state("hello world");
        let selected = |state: &LineEditState| {
            let (start, end) = (
                state.anchor.min(state.focus),
                state.anchor.max(state.focus),
            );
            state.text()[start..end].to_string()
        };

        state.pointer_down(&mut fonts, &mut layouts, 1.0, Point::new(2.0, 5.0), false, 2);
        assert_eq!(selected(&state), "hello");

        // Extending keeps the word anchor across transient editors...
        assert!(state.pointer_move(&mut fonts, &mut layouts, 1.0, Point::new(10_000.0, 5.0)));
        assert_eq!(selected(&state), "hello world");

        // ...and dragging back re-collapses to the anchor word.
        assert!(state.pointer_move(&mut fonts, &mut layouts, 1.0, Point::new(2.0, 5.0)));
        assert_eq!(selected(&state), "hello");
    }

    #[test]
    fn drag_extends_selection_until_released() {
        let (mut fonts, mut layouts) = contexts();
        let mut state = state("hello world");

        state.pointer_down(&mut fonts, &mut layouts, 1.0, Point::new(0.0, 5.0), false, 1);
        assert!(state.pointer_move(&mut fonts, &mut layouts, 1.0, Point::new(10_000.0, 5.0)));
        assert_eq!((state.anchor, state.focus), (0, 11));

        assert!(state.pointer_up());
        assert!(!state.pointer_up());
        assert!(!state.pointer_move(&mut fonts, &mut layouts, 1.0, Point::new(0.0, 5.0)));
    }

    #[test]
    fn ime_compose_then_commit() {
        let (mut fonts, mut layouts) = contexts();
        let mut state = state("");

        state.handle_ime(&ImeEvent::Preedit("ni".into(), Some((2, 2))));
        assert!(state.is_composing());
        // The preedit stays out of the base text.
        assert!(state.text() == "");
        // Keys decline while composing.
        assert!(!press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character("x".into()),
            Modifiers::empty(),
        ));

        state.handle_ime(&ImeEvent::Preedit("".into(), None));
        assert!(!state.is_composing());
        state.handle_ime(&ImeEvent::Commit("你".into()));
        assert!(state.text() == "你");
    }

    #[test]
    fn composing_over_a_selection_consumes_it() {
        let (mut fonts, mut layouts) = contexts();
        let mut state = state("abc").with_cursor_at_end();
        press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::ArrowLeft),
            Modifiers::SHIFT,
        );
        state.handle_ime(&ImeEvent::Preedit("n".into(), Some((1, 1))));
        assert!(state.text() == "ab");
        state.handle_ime(&ImeEvent::Commit("ñ".into()));
        assert!(state.text() == "abñ");
    }
}
