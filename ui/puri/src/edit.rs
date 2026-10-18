//! Bare editable text over parley, custody split at true state: the
//! caller-owned `LineEditState` holds text and interaction state;
//! font, paint, affixes, focus, and chrome belong to the ephemeral
//! `LineEditDescription`. A transient `PlainEditor` is
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
//! Chrome (frame, padding, focus ring, minimum width) is caller
//! composition; Progred uses its `pad`/`decorate` box operations.
//! Pointer-down policy stays with the wrapping layer through the
//! state's pointer methods; while focused, the widget registers
//! keyboard and IME dispatch and a drag's move, release, and cancel.

use crate::draw::CanvasSink;
use crate::geometry::Placement;
use crate::handler::{HasHandler, ImeEvent};
use crate::interact::is_primary_contact_move;
use crate::text::{TextCtx, TextMetrics, TextStyle, build_layout, draw_layout, line_metrics};
use kurbo::{Affine, Point, Rect};
use parley::Layout;
use parley::style::GenericFamily;
use parley::{FontContext, LayoutContext, PlainEditor, StyleProperty};
use peniko::Brush;
use std::rc::Rc;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};
use ui_events::pointer::PointerButtonEvent;

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

impl Preedit {
    /// An empty preedit is no composition; the cursor stays inside the text.
    fn new(text: String, cursor: Option<(usize, usize)>) -> Option<Self> {
        (!text.is_empty()).then(|| {
            let clamp = |(a, b): (usize, usize)| (a.min(text.len()), b.min(text.len()));
            Self {
                cursor: cursor.map(clamp),
                text,
            }
        })
    }
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
}

#[derive(Clone)]
pub struct EditStyle {
    pub selection: Brush,
    pub cursor: Brush,
}

/// The caller lends editing state for one operation and may interpret
/// its result after those borrows end (for example, updating a model).
pub type EditOperation<'a> = dyn Fn(EditCtx<'_>) -> bool + 'a;

/// Presentation inputs used to construct a transient Parley editor.
/// They belong to the current widget description, not durable cursor,
/// selection, drag, or IME state.
#[derive(Clone)]
pub struct LineEditPresentation {
    pub font_size: f32,
    pub brush: Brush,
    pub family: GenericFamily,
    pub prefix: String,
    pub suffix: String,
}

impl LineEditPresentation {
    pub fn new(font_size: f32, brush: Brush) -> Self {
        Self {
            font_size,
            brush,
            family: GenericFamily::SystemUi,
            prefix: String::new(),
            suffix: String::new(),
        }
    }
}

/// One frame's complete line-edit description. Only `state` survives
/// into a later frame; the other inputs are captured by this frame's
/// transient handlers.
pub struct LineEditDescription<'a> {
    pub state: &'a LineEditState,
    pub focused: bool,
    pub presentation: LineEditPresentation,
    pub style: &'a EditStyle,
    pub placeholder: Option<(&'a str, &'a TextStyle)>,
}

/// The text-only pasteboard capability line editing needs. The
/// application chooses the platform implementation and can share it
/// with richer structural clipboard policy.
pub trait TextClipboard {
    fn get_text(&mut self) -> Option<String>;
    fn set_text(&mut self, text: &str);
    /// Whether the text stands in for a richer structure, which the
    /// caller pastes whole rather than as typed text.
    fn holds_structure(&mut self) -> bool {
        false
    }
}

/// What an editing dispatch needs from the caller's context: the state
/// plus the measurement caches parley's driver requires.
pub struct EditCtx<'a> {
    pub state: &'a mut LineEditState,
    pub fonts: &'a mut FontContext,
    pub layouts: &'a mut LayoutContext<Brush>,
    pub clipboard: &'a mut dyn TextClipboard,
}

impl LineEditState {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            anchor: 0,
            focus: 0,
            preedit: None,
            drag: None,
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

    /// Land the caret at the start — the mirror, for fields entered
    /// walking backward.
    pub fn cursor_to_start(&mut self) {
        self.anchor = 0;
        self.focus = 0;
    }

    /// The base text: what edits commit. Excludes any IME preedit and
    /// the affixes, which are display only.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The selection byte offsets — `focus` may precede `anchor` for
    /// a backward selection. Serialization surface, with
    /// [`LineEditState::from_parts`] as its inverse.
    pub fn selection_offsets(&self) -> (usize, usize) {
        (self.anchor, self.focus)
    }

    /// The in-flight IME composition: its text and cursor range.
    pub fn preedit_parts(&self) -> Option<(&str, Option<(usize, usize)>)> {
        self.preedit
            .as_ref()
            .map(|preedit| (preedit.text.as_str(), preedit.cursor))
    }

    /// The in-progress drag-selection: its origin and click count.
    pub fn drag_parts(&self) -> Option<(Point, u8)> {
        self.drag.map(|drag| (drag.origin, drag.count))
    }

    /// Rebuild editing state from serialized parts. Junk decodes to
    /// the nearest sane state: offsets clamp to char boundaries at or
    /// before themselves.
    pub fn from_parts(
        text: &str,
        anchor: usize,
        focus: usize,
        preedit: Option<(String, Option<(usize, usize)>)>,
        drag: Option<(Point, u8)>,
    ) -> Self {
        Self {
            text: text.to_string(),
            anchor: text.floor_char_boundary(anchor),
            focus: text.floor_char_boundary(focus),
            preedit: preedit.and_then(|(text, cursor)| Preedit::new(text, cursor)),
            drag: drag.map(|(origin, count)| Drag { origin, count }),
        }
    }

    /// Replace the text wholesale — the caller's re-mint for external
    /// writes — keeping the selection clamped to char boundaries.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.anchor = self.text.floor_char_boundary(self.anchor);
        self.focus = self.text.floor_char_boundary(self.focus);
    }

    /// The transient parley editor this state denotes: constructed,
    /// used within one pass or one dispatch, dropped. Always returns
    /// with a clean layout.
    fn editor(
        &self,
        presentation: &LineEditPresentation,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        scale: f32,
    ) -> PlainEditor<Brush> {
        let mut editor = PlainEditor::new(presentation.font_size);
        editor.set_text(&format!(
            "{}{}{}",
            presentation.prefix, self.text, presentation.suffix
        ));
        editor.set_width(None);
        editor.set_scale(scale);
        editor
            .edit_styles()
            .insert(StyleProperty::Brush(presentation.brush.clone()));
        editor.edit_styles().insert(presentation.family.into());
        let mut driver = editor.driver(fonts, layouts);
        let p = presentation.prefix.len();
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
    fn absorb(&mut self, presentation: &LineEditPresentation, editor: &PlainEditor<Brush>) {
        let composed = editor.text().to_string();
        if let Some(inner) = composed
            .strip_prefix(presentation.prefix.as_str())
            .and_then(|t| t.strip_suffix(presentation.suffix.as_str()))
        {
            let p = presentation.prefix.len();
            let n = inner.len();
            self.text = inner.to_string();
            let selection = editor.raw_selection();
            self.anchor = selection.anchor().index().clamp(p, p + n) - p;
            self.focus = selection.focus().index().clamp(p, p + n) - p;
        }
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
        presentation: &LineEditPresentation,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        clipboard: &mut dyn TextClipboard,
        command_modifier: crate::keyboard::CommandModifier,
        event: &KeyboardEvent,
    ) -> bool {
        event.state.is_down()
            && self.preedit.is_none()
            && self.edit_key(
                presentation,
                fonts,
                layouts,
                clipboard,
                command_modifier,
                event,
            )
    }

    /// [`Self::handle_key`] for a key press outside a composition.
    fn edit_key(
        &mut self,
        presentation: &LineEditPresentation,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        clipboard: &mut dyn TextClipboard,
        command_modifier: crate::keyboard::CommandModifier,
        event: &KeyboardEvent,
    ) -> bool {
        let action_mod = command_modifier.pressed(&event.modifiers);
        let (line_mod, word_mod) = match command_modifier {
            crate::keyboard::CommandModifier::Meta => {
                (event.modifiers.meta(), event.modifiers.alt())
            }
            crate::keyboard::CommandModifier::Control => (false, event.modifiers.ctrl()),
        };
        let shift = event.modifiers.shift();
        // The selection's reachable span: between the affixes. Motion
        // that only wanders into an affix is no motion — clamped, it
        // reads as the boundary it started at, so boundary arrows
        // still decline to the caller.
        let (lo, hi) = (
            presentation.prefix.len(),
            presentation.prefix.len() + self.text.len(),
        );
        let clamp = move |(a, f): (usize, usize)| (a.clamp(lo, hi), f.clamp(lo, hi));
        let mut editor = self.editor(presentation, fonts, layouts, 1.0);
        let handled = {
            let mut drv = editor.driver(fonts, layouts);
            match &event.key {
                // Copy and cut handle only when text is actually
                // selected: with nothing to copy they decline, so the
                // caller can interpret the chord (structural copy of
                // the edited value). Paste lands in the text unless the
                // clipboard holds a structure, which the caller places.
                Key::Character(c)
                    if action_mod && matches!(c.to_lowercase().as_str(), "c" | "x" | "v") =>
                {
                    let selected = drv.editor.selected_text().map(str::to_owned);
                    match (c.to_lowercase().as_str(), selected) {
                        ("c", Some(text)) => {
                            clipboard.set_text(&text);
                            true
                        }
                        ("x", Some(text)) => {
                            clipboard.set_text(&text);
                            drv.delete_selection();
                            true
                        }
                        ("v", _) if clipboard.holds_structure() => false,
                        ("v", _) => {
                            if let Some(text) = clipboard.get_text() {
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
                    match (line_mod, word_mod, shift) {
                        (true, _, true) => drv.select_to_line_start(),
                        (true, _, false) => drv.move_to_line_start(),
                        (false, true, true) => drv.select_word_left(),
                        (false, true, false) => drv.move_word_left(),
                        (false, false, true) => drv.select_left(),
                        (false, false, false) => drv.move_left(),
                    }
                    clamp(cursor_of(drv.editor.raw_selection())) != before
                }
                Key::Named(NamedKey::ArrowRight) => {
                    let before = clamp(cursor_of(drv.editor.raw_selection()));
                    match (line_mod, word_mod, shift) {
                        (true, _, true) => drv.select_to_line_end(),
                        (true, _, false) => drv.move_to_line_end(),
                        (false, true, true) => drv.select_word_right(),
                        (false, true, false) => drv.move_word_right(),
                        (false, false, true) => drv.select_right(),
                        (false, false, false) => drv.move_right(),
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
                    match (line_mod, word_mod) {
                        (true, _) => {
                            if drv.editor.raw_selection().is_collapsed() {
                                drv.extend_selection_to_byte(hi);
                            }
                            drv.delete_selection();
                        }
                        (false, true) => drv.delete_word(),
                        (false, false) => drv.delete(),
                    }
                    true
                }
                Key::Named(NamedKey::Backspace) if !self.text.is_empty() => {
                    match (line_mod, word_mod) {
                        (true, _) => {
                            if drv.editor.raw_selection().is_collapsed() {
                                drv.extend_selection_to_byte(lo);
                            }
                            drv.delete_selection();
                        }
                        (false, true) => drv.backdelete_word(),
                        (false, false) => drv.backdelete(),
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
            self.absorb(presentation, &editor);
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
                let preedit = Preedit::new(text.clone(), *cursor);
                if preedit.is_some() && self.preedit.is_none() && self.anchor != self.focus {
                    self.replace_selection("");
                }
                self.preedit = preedit;
                true
            }
            ImeEvent::Disabled => {
                self.preedit = None;
                true
            }
            ImeEvent::Enabled => true,
        }
    }

    /// Pointer positioning for the wrapping layer: `origin` is where the
    /// text's layout starts, in the event's coordinates, physical pixels,
    /// so hit-testing needs the display scale. A double click selects the
    /// word, a triple the line.
    pub fn pointer_down(
        &mut self,
        presentation: &LineEditPresentation,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        scale: f32,
        event: &PointerButtonEvent,
        origin: Point,
    ) {
        if self.preedit.is_none() {
            let point = Point::new(event.state.position.x, event.state.position.y) - origin;
            let count = event.state.count.max(1);
            self.drag = Some(Drag {
                origin: point.to_point(),
                count,
            });
            let (x, y) = (point.x as f32, point.y as f32);
            let mut editor = self.editor(presentation, fonts, layouts, scale);
            {
                let mut drv = editor.driver(fonts, layouts);
                match count {
                    2 => drv.select_word_at_point(x, y),
                    3 => drv.select_hard_line_at_point(x, y),
                    _ => {
                        if event.state.modifiers.shift() {
                            drv.shift_click_extension(x, y);
                        } else {
                            drv.move_to_point(x, y);
                        }
                    }
                }
            }
            self.absorb(presentation, &editor);
        }
    }

    /// Drag-extend the selection; `point` in local coordinates. Only
    /// acts while a drag started in this field. Word- and line-anchored
    /// drags rebuild their anchor from the gesture's origin each move,
    /// since only byte offsets round-trip the transient editors.
    pub fn pointer_move(
        &mut self,
        presentation: &LineEditPresentation,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        scale: f32,
        point: Point,
    ) -> bool {
        match self.drag.filter(|_| self.preedit.is_none()) {
            Some(drag) => {
                let mut editor = self.editor(presentation, fonts, layouts, scale);
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
                self.absorb(presentation, &editor);
                true
            }
            None => false,
        }
    }

    /// Ends a drag; returns whether one was in progress.
    pub fn pointer_up(&mut self) -> bool {
        self.drag.take().is_some()
    }
}

/// A measured, ephemeral line-edit description. Its caller chooses a
/// placement, then draws it and installs its handlers there; no layout
/// strategy is built into Puri.
pub struct LineEdit {
    metrics: TextMetrics,
    scale: f32,
    ghost: Option<Rc<Layout<Brush>>>,
    layout: Option<Layout<Brush>>,
    editor_baseline: f64,
    selection: Vec<Rect>,
    cursor: Option<Rect>,
    composition: Option<Rect>,
    selection_brush: Brush,
    cursor_brush: Brush,
    focused: bool,
    presentation: LineEditPresentation,
}

impl LineEdit {
    pub fn metrics(&self) -> TextMetrics {
        self.metrics
    }

    /// Where the editor's layout starts at `placement`: its baseline on
    /// the field's. The ghost's metrics may size the field while it shows.
    fn editor_origin(&self, placement: Placement) -> Point {
        Point::new(
            placement.rect.x0,
            placement.rect.y0 + self.metrics.ascent - self.editor_baseline,
        )
    }

    /// While focused, the area at `placement`, in its coordinates, that
    /// platform composition candidates should keep clear: the preedit, or
    /// the caret with some context. It exists even while a preedit hides
    /// the caret.
    pub fn input_area(&self, placement: Placement) -> Option<Rect> {
        self.composition
            .map(|area| area + self.editor_origin(placement).to_vec2())
    }

    /// The paint half: selection, ghost, content, caret.
    pub fn draw(&self, canvas: &mut (impl CanvasSink + ?Sized), placement: Placement) {
        let transform = Affine::translate(placement.rect.origin().to_vec2());
        for rect in &self.selection {
            canvas.fill_shape((*rect).into(), self.selection_brush.clone(), transform);
        }
        if let Some(ghost) = &self.ghost {
            draw_layout(canvas, ghost, transform);
        }
        if let Some(layout) = &self.layout {
            draw_layout(canvas, layout, transform);
        }
        if let Some(cursor) = self.cursor {
            canvas.fill_shape(
                cursor.into(),
                self.cursor_brush.clone(),
                Affine::translate(self.editor_origin(placement).to_vec2()),
            );
        }
    }

    /// The dispatch half: while focused, register key, drag, release,
    /// and IME against the settled placement. `command_modifier` is the
    /// platform's command key, for the editing shortcuts. Dispatch can
    /// outlive the editor represented by the frame, so the caller
    /// declines when it has gone away. `with` runs the editing operation
    /// and can process its result after the state and platform-service
    /// borrows end.
    pub fn install<C: 'static, P: HasHandler<C>>(
        &self,
        p: &mut P,
        placement: Placement,
        command_modifier: crate::keyboard::CommandModifier,
        with: impl Fn(&mut C, &EditOperation<'_>) -> bool + Clone + 'static,
    ) {
        let scale = self.scale;
        let presentation = self.presentation.clone();
        if self.focused {
            let text_origin = self.editor_origin(placement);
            let with_key = with.clone();
            let key_presentation = presentation.clone();
            p.handler().on_key(move |ctx, event| {
                with_key(ctx, &|edit| {
                    edit.state.handle_key(
                        &key_presentation,
                        edit.fonts,
                        edit.layouts,
                        edit.clipboard,
                        command_modifier,
                        event,
                    )
                })
            });
            let with_move = with.clone();
            let move_presentation = presentation.clone();
            p.handler().on_pointer_move(move |ctx, update| {
                is_primary_contact_move(update)
                    && with_move(ctx, &|edit| {
                        edit.state.pointer_move(
                            &move_presentation,
                            edit.fonts,
                            edit.layouts,
                            scale,
                            Point::new(
                                update.current.position.x - text_origin.x,
                                update.current.position.y - text_origin.y,
                            ),
                        )
                    })
            });
            let with_up = with.clone();
            p.handler()
                .on_pointer_up(move |ctx, _| with_up(ctx, &|edit| edit.state.pointer_up()));
            let with_cancel = with.clone();
            p.handler()
                .on_pointer_cancel(move |ctx, _| with_cancel(ctx, &|edit| edit.state.pointer_up()));
            p.handler()
                .on_ime(move |ctx, event| with(ctx, &|edit| edit.state.handle_ime(event)));
        }
    }
}

/// Build bare editable text from current state and presentation. While
/// empty, an optional placeholder supplies the measured and drawn ghost
/// content. Pointer-down policy remains with the caller.
pub fn text_edit(description: LineEditDescription<'_>, tcx: &mut TextCtx) -> LineEdit {
    let LineEditDescription {
        state,
        focused,
        presentation,
        style,
        placeholder,
    } = description;
    let scale = tcx.scale;
    let ghost = placeholder
        .filter(|_| {
            state.text.is_empty()
                && state.preedit.is_none()
                && presentation.prefix.is_empty()
                && presentation.suffix.is_empty()
        })
        .map(|(text, style)| build_layout(tcx, text, style));
    let editor = state.editor(&presentation, tcx.fonts, tcx.layouts, scale);
    let layout = editor.try_layout().cloned();
    let editor_metrics = layout.as_ref().and_then(line_metrics);
    // The ghost's metrics size the field while it shows; both it and
    // the cursor hang from the shared visual baseline.
    let metrics = ghost
        .as_deref()
        .and_then(line_metrics)
        .or(editor_metrics)
        .unwrap_or_default();
    let selection = if focused {
        editor
            .selection_geometry()
            .into_iter()
            .map(|(bb, _)| Rect::new(bb.x0, bb.y0, bb.x1, bb.y1))
            .collect()
    } else {
        Vec::new()
    };
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
    let composition = focused.then(|| {
        let area = editor.ime_cursor_area();
        Rect::new(area.x0, area.y0, area.x1, area.y1)
    });
    LineEdit {
        metrics,
        scale,
        ghost,
        layout,
        editor_baseline: editor_metrics.map_or(0.0, |metrics| metrics.ascent),
        selection,
        cursor,
        composition,
        selection_brush: style.selection.clone(),
        cursor_brush: style.cursor.clone(),
        focused,
        presentation,
    }
}

#[cfg(test)]
mod tests;
