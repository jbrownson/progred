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
use crate::text::{TextCtx, TextStyle, build_layout, draw_layout};
use kurbo::{Affine, Point, Rect};
use parley::Layout;
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
    /// Display armor around `text`: shaped and measured as one run
    /// with it — a string literal's quotes ride the field — but never
    /// editable. An edit that would bite an affix declines whole, and
    /// the selection lives strictly between them.
    prefix: String,
    suffix: String,
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
            prefix: String::new(),
            suffix: String::new(),
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

    /// Dress the field in uneditable affixes — a string literal's
    /// quotes, a blob's `0x` — laid out as one shaped run with the
    /// text.
    pub fn with_affixes(mut self, prefix: &str, suffix: &str) -> Self {
        self.prefix = prefix.to_string();
        self.suffix = suffix.to_string();
        self
    }

    /// Land the caret at the end — plain data, no contexts needed.
    pub fn cursor_to_end(&mut self) {
        self.anchor = self.text.len();
        self.focus = self.text.len();
    }

    /// Land the caret at the start — the mirror, for fields entered
    /// walking backward.
    pub fn cursor_to_start(&mut self) {
        self.anchor = 0;
        self.focus = 0;
    }

    /// Land the caret at a byte index, clamped to the nearest char
    /// boundary at or before it — for mounts whose caret was
    /// hit-tested against another projection of the same spelling.
    pub fn cursor_to(&mut self, index: usize) {
        let mut index = index.min(self.text.len());
        while !self.text.is_char_boundary(index) {
            index -= 1;
        }
        self.anchor = index;
        self.focus = index;
    }

    /// The base text: what edits commit. Excludes any IME preedit and
    /// the affixes, which are display only.
    pub fn text(&self) -> &str {
        &self.text
    }

    fn dressed(&self) -> bool {
        !self.prefix.is_empty() || !self.suffix.is_empty()
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
        editor.set_text(&format!("{}{}{}", self.prefix, self.text, self.suffix));
        editor.set_width(None);
        editor.set_scale(scale);
        editor
            .edit_styles()
            .insert(StyleProperty::Brush(self.brush.clone()));
        editor.edit_styles().insert(GenericFamily::SystemUi.into());
        let mut driver = editor.driver(fonts, layouts);
        let p = self.prefix.len();
        driver.select_byte_range(p + self.anchor, p + self.focus);
        if let Some(preedit) = &self.preedit {
            driver.set_compose(&preedit.text, preedit.cursor);
        }
        editor
    }

    /// Read the mutated editor back into true state. Only called from
    /// compose-free paths: keys and pointers decline while composing,
    /// and IME events never touch an editor. The affixes are not the
    /// editor's to change: an edit that bit one declines WHOLE (the
    /// state simply doesn't absorb it), and the selection clamps to
    /// the span between them.
    fn absorb(&mut self, editor: &PlainEditor<Brush>) {
        let composed = editor.text().to_string();
        let Some(inner) = composed
            .strip_prefix(self.prefix.as_str())
            .and_then(|t| t.strip_suffix(self.suffix.as_str()))
        else {
            return;
        };
        let p = self.prefix.len();
        let n = inner.len();
        self.text = inner.to_string();
        let selection = editor.raw_selection();
        self.anchor = selection.anchor().index().clamp(p, p + n) - p;
        self.focus = selection.focus().index().clamp(p, p + n) - p;
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
        // The selection's reachable span: between the affixes. Motion
        // that only wanders into an affix is no motion — clamped, it
        // reads as the boundary it started at, so boundary arrows
        // still decline to the caller.
        let (lo, hi) = (self.prefix.len(), self.prefix.len() + self.text.len());
        let clamp = move |(a, f): (usize, usize)| (a.clamp(lo, hi), f.clamp(lo, hi));
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
                    let before = clamp(cursor_of(drv.editor.raw_selection()));
                    match (action_mod, shift) {
                        (true, true) => drv.select_word_left(),
                        (true, false) => drv.move_word_left(),
                        (false, true) => drv.select_left(),
                        (false, false) => drv.move_left(),
                    }
                    clamp(cursor_of(drv.editor.raw_selection())) != before
                }
                Key::Named(NamedKey::ArrowRight) => {
                    let before = clamp(cursor_of(drv.editor.raw_selection()));
                    match (action_mod, shift) {
                        (true, true) => drv.select_word_right(),
                        (true, false) => drv.move_word_right(),
                        (false, true) => drv.select_right(),
                        (false, false) => drv.move_right(),
                    }
                    clamp(cursor_of(drv.editor.raw_selection())) != before
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
                // Delete keys decline on empty CONTENT — a no-op edit is
                // not a handled edit — so the caller can interpret them
                // (delete the element, join, whatever). With content, a
                // delete that only bites an affix is swallowed instead:
                // absorb declines it, and handled stays true.
                Key::Named(NamedKey::Delete) if !self.text.is_empty() => {
                    if action_mod {
                        drv.delete_word();
                    } else {
                        drv.delete();
                    }
                    true
                }
                Key::Named(NamedKey::Backspace) if !self.text.is_empty() => {
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
/// editor built off the true state. While the text is empty, an
/// optional `placeholder` shows as ghost content — sized and drawn in
/// its own style, the field keeping the ghost's width instead of
/// collapsing — and the first typed character replaces it, the field
/// snapping to fit. Registers keyboard and IME dispatch (through
/// `with`) only while focused; pointer wiring is the caller's, via
/// the state's pointer methods and its own settled rect. Dispatch
/// targets the last rendered frame, which can outlive the editor
/// (deselect, then a move in the same gesture), so `with` returns
/// None when the editor is gone and the handlers decline.
pub fn text_edit<C: 'static, P: Canvas + HasHandler<C>>(
    state: &LineEditState,
    focused: bool,
    style: &EditStyle,
    placeholder: Option<(&str, &TextStyle)>,
    tcx: &mut TextCtx,
    with: impl for<'a> Fn(&'a mut C) -> Option<EditCtx<'a>> + Clone + 'static,
) -> Node<P> {
    let scale = tcx.scale;
    let ghost = placeholder
        .filter(|_| state.text.is_empty() && !state.is_composing() && !state.dressed())
        .map(|(text, style)| build_layout(tcx, text, style, None, None));
    let editor = state.editor(tcx.fonts, tcx.layouts, scale);
    let layout = editor.try_layout().cloned();
    let metrics_of = |layout: &Layout<Brush>| {
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
    };
    let editor_baseline = layout
        .as_ref()
        .and_then(metrics_of)
        .map(|(_, baseline)| baseline)
        .unwrap_or(0.0);
    // The ghost's metrics size the field while it shows; both it and
    // the cursor hang from the shared visual baseline.
    let (extent, layout_baseline) = ghost
        .as_ref()
        .and_then(metrics_of)
        .or_else(|| layout.as_ref().and_then(metrics_of))
        .unwrap_or((Extent::default(), 0.0));

    let selection: Vec<Rect> = focused
        .then(|| {
            let mut rects = Vec::new();
            editor
                .selection_geometry_with(|bb, _| rects.push(Rect::new(bb.x0, bb.y0, bb.x1, bb.y1)));
            rects
        })
        .unwrap_or_default();
    // Parley's caret spans the leaded line box; a native-feeling
    // caret spans the ascent and a taste of the descent.
    let caret_span = layout.as_ref().and_then(|l| l.lines().next()).map(|line| {
        let m = *line.metrics();
        let baseline = m.baseline as f64;
        (
            baseline - m.ascent as f64,
            baseline + 0.5 * m.descent as f64,
        )
    });
    let cursor = focused
        .then(|| {
            editor.cursor_geometry(1.5 * scale).map(|bb| {
                let (top, bottom) = caret_span.unwrap_or((bb.y0, bb.y1));
                Rect::new(bb.x0, top, bb.x1, bottom)
            })
        })
        .flatten();
    let selection_brush = style.selection.clone();
    let cursor_brush = style.cursor.clone();

    leaf(extent, move |p: &mut P, at: Point| {
        let transform = Affine::translate((at.x, at.y - layout_baseline));
        for rect in &selection {
            p.fill(*rect, selection_brush.clone(), transform);
        }
        if let Some(ghost) = &ghost {
            draw_layout(p, ghost, transform);
        }
        if let Some(layout) = &layout {
            draw_layout(p, layout, transform);
        }
        if let Some(cursor) = cursor {
            p.fill(
                cursor,
                cursor_brush.clone(),
                Affine::translate((at.x, at.y - editor_baseline)),
            );
        }
        if focused {
            let text_origin = Point::new(at.x, at.y - editor_baseline);
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
    fn cursor_to_floors_to_a_char_boundary() {
        let (mut fonts, mut layouts) = contexts();
        let mut state = state("héllo");
        state.cursor_to(2);
        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character("z".into()),
            Modifiers::empty(),
        ));
        assert!(state.text() == "hzéllo");
    }

    #[test]
    fn boundary_arrows_decline_from_either_seeded_end() {
        let (mut fonts, mut layouts) = contexts();
        let mut state = state("abc").with_cursor_at_end();
        // At the end, right declines and left grinds inward.
        assert!(!press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::ArrowRight),
            Modifiers::empty(),
        ));
        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::ArrowLeft),
            Modifiers::empty(),
        ));
        // Re-seeded at the start, left declines immediately and
        // typing prepends.
        state.cursor_to_start();
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
            Key::Character("z".into()),
            Modifiers::empty(),
        ));
        assert_eq!(state.text(), "zabc");
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
    fn affixes_are_armor_not_content() {
        let (mut fonts, mut layouts) = contexts();
        let action = if cfg!(target_os = "macos") {
            Modifiers::META
        } else {
            Modifiers::CONTROL
        };
        let mut state = state("hi").with_affixes("\"", "\"").with_cursor_at_end();

        // Typing lands between the affixes; the text stays bare.
        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character("!".into()),
            Modifiers::empty(),
        ));
        assert_eq!(state.text(), "hi!");

        // Backspace at content start bites the prefix: swallowed
        // whole — handled, nothing changes.
        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Home),
            Modifiers::empty(),
        ));
        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Backspace),
            Modifiers::empty(),
        ));
        assert_eq!(state.text(), "hi!");

        // Motion that only wanders into an affix is no motion: the
        // boundary arrow still declines to the caller.
        assert!(!press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::ArrowLeft),
            Modifiers::empty(),
        ));

        // Select-all reaches the content alone; typing replaces it
        // and the affixes stand.
        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character("a".into()),
            action,
        ));
        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character("x".into()),
            Modifiers::empty(),
        ));
        assert_eq!(state.text(), "x");

        // Delete at content end bites the suffix: swallowed too.
        state.cursor_to_end();
        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Delete),
            Modifiers::empty(),
        ));
        assert_eq!(state.text(), "x");

        // Emptied, the delete keys decline — the caller's
        // delete-the-value idiom sees through the affixes.
        assert!(press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Backspace),
            Modifiers::empty(),
        ));
        assert_eq!(state.text(), "");
        assert!(!press(
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Backspace),
            Modifiers::empty(),
        ));
    }

    #[test]
    fn word_delete_stays_interior_or_declines_whole() {
        let (mut fonts, mut layouts) = contexts();
        let action = if cfg!(target_os = "macos") {
            Modifiers::META
        } else {
            Modifiers::CONTROL
        };
        // Word-delete whose boundary lands in the interior works.
        let mut words = state("hi there").with_affixes("\"", "\"");
        words.anchor = 2;
        words.focus = 2;
        assert!(press(
            &mut words,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Backspace),
            action,
        ));
        assert_eq!(words.text(), " there");
        // KNOWN COARSENESS: leading whitespace lets the word boundary
        // reach through it into the prefix, and the bite declines
        // WHOLE — a swallowed no-op where trimming to the interior
        // was arguable. Parley owns the range; the decline is the
        // affix contract.
        let mut leading = state(" hi").with_affixes("\"", "\"");
        leading.anchor = 1;
        leading.focus = 1;
        assert!(press(
            &mut leading,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Backspace),
            action,
        ));
        assert_eq!(leading.text(), " hi");
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
