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
//! Pointer policy stays with the wrapping layer through the state's
//! pointer methods; the widget registers keyboard and IME dispatch
//! only while focused.

use crate::draw::Canvas;
use crate::geometry::Placement;
use crate::handler::{HasHandler, ImeEvent};
use crate::interact::is_primary_contact_move;
use crate::text::{TextCtx, TextMetrics, TextStyle, build_layout, draw_layout};
use kurbo::{Affine, Point, Rect};
use parley::Layout;
use parley::style::GenericFamily;
use parley::{FontContext, LayoutContext, PlainEditor, StyleProperty};
use peniko::Brush;
use std::rc::Rc;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

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
}

pub struct EditStyle {
    pub selection: Brush,
    pub cursor: Brush,
}

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

    pub fn with_family(mut self, family: GenericFamily) -> Self {
        self.family = family;
        self
    }

    pub fn with_affixes(mut self, prefix: &str, suffix: &str) -> Self {
        self.prefix = prefix.to_string();
        self.suffix = suffix.to_string();
        self
    }

    fn dressed(&self) -> bool {
        !self.prefix.is_empty() || !self.suffix.is_empty()
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

#[derive(Clone, Copy)]
pub struct LineEditPointerDown {
    pub point: Point,
    pub shift: bool,
    pub count: u8,
}

/// The text-only pasteboard capability line editing needs. The
/// application chooses the platform implementation and can share it
/// with richer structural clipboard policy.
pub trait TextClipboard {
    fn get_text(&mut self) -> Option<String>;
    fn set_text(&mut self, text: &str);
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
        let mut state = Self::new(text);
        state.cursor_to(anchor);
        let anchor = state.anchor;
        state.cursor_to(focus);
        state.anchor = anchor;
        state.preedit = preedit.map(|(text, cursor)| Preedit { text, cursor });
        state.drag = drag.map(|(origin, count)| Drag { origin, count });
        state
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
        let Some(inner) = composed
            .strip_prefix(presentation.prefix.as_str())
            .and_then(|t| t.strip_suffix(presentation.suffix.as_str()))
        else {
            return;
        };
        let p = presentation.prefix.len();
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
        presentation: &LineEditPresentation,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        clipboard: &mut dyn TextClipboard,
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
        let (lo, hi) = (
            presentation.prefix.len(),
            presentation.prefix.len() + self.text.len(),
        );
        let clamp = move |(a, f): (usize, usize)| (a.clamp(lo, hi), f.clamp(lo, hi));
        let mut editor = self.editor(presentation, fonts, layouts, 1.0);
        let handled = {
            let mut drv = editor.driver(fonts, layouts);
            match &event.key {
                #[cfg(any(
                    target_os = "windows",
                    target_os = "macos",
                    target_os = "linux",
                    target_arch = "wasm32"
                ))]
                // Copy and cut handle only when text is actually
                // selected: with nothing to copy they decline, so the
                // caller can interpret the chord (structural copy of
                // the edited value). Paste always lands in the text.
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
        presentation: &LineEditPresentation,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        scale: f32,
        event: LineEditPointerDown,
    ) {
        if self.is_composing() {
            return;
        }
        self.drag = Some(Drag {
            origin: event.point,
            count: event.count,
        });
        let (x, y) = (event.point.x as f32, event.point.y as f32);
        let mut editor = self.editor(presentation, fonts, layouts, scale);
        {
            let mut drv = editor.driver(fonts, layouts);
            match event.count {
                2 => drv.select_word_at_point(x, y),
                3 => drv.select_hard_line_at_point(x, y),
                _ => {
                    if event.shift {
                        drv.shift_click_extension(x, y);
                    } else {
                        drv.move_to_point(x, y);
                    }
                }
            }
        }
        self.absorb(presentation, &editor);
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
        let Some(drag) = self.drag else {
            return false;
        };
        if self.is_composing() {
            return false;
        }
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

    /// Ends a drag; returns whether one was in progress.
    pub fn pointer_up(&mut self) -> bool {
        self.drag.take().is_some()
    }
}

/// A measured, ephemeral line-edit description. Its caller chooses a
/// placement and invokes [`LineEdit::place`]; no layout strategy is
/// built into Puri.
pub struct LineEdit {
    text: String,
    metrics: TextMetrics,
    scale: f32,
    ghost: Option<Rc<Layout<Brush>>>,
    layout: Option<Layout<Brush>>,
    layout_baseline: f64,
    editor_baseline: f64,
    selection: Vec<Rect>,
    cursor: Option<Rect>,
    selection_brush: Brush,
    cursor_brush: Brush,
    focused: bool,
    presentation: LineEditPresentation,
}

/// Drawing inputs for composing an editor from ordinary text and
/// vector primitives. Rectangles use the line box's top-left origin.
#[derive(Clone)]
pub struct LineEditGeometry {
    pub text: String,
    pub metrics: TextMetrics,
    pub selection: Vec<Rect>,
    pub cursor: Option<Rect>,
}

impl LineEdit {
    pub fn metrics(&self) -> TextMetrics {
        self.metrics
    }

    pub fn geometry(&self) -> LineEditGeometry {
        let selection_y = self.metrics.ascent - self.layout_baseline;
        let cursor_y = self.metrics.ascent - self.editor_baseline;
        let translate = |rect: Rect, y: f64| {
            Rect::new(rect.x0, rect.y0 + y, rect.x1, rect.y1 + y)
        };
        LineEditGeometry {
            text: self.text.clone(),
            metrics: self.metrics,
            selection: self
                .selection
                .iter()
                .copied()
                .map(|rect| translate(rect, selection_y))
                .collect(),
            cursor: self.cursor.map(|rect| translate(rect, cursor_y)),
        }
    }

    /// Draw and register this description at its caller-supplied
    /// settled placement. Dispatch can outlive the editor represented
    /// by the frame, so `with` returns `None` when it has gone away.
    pub fn place<C: 'static, P: Canvas + HasHandler<C>>(
        self,
        p: &mut P,
        placement: Placement,
        with: impl for<'a> Fn(&'a mut C) -> Option<EditCtx<'a>> + Clone + 'static,
    ) {
        self.draw(p, placement);
        self.install(p, placement, with);
    }

    /// The paint half: selection, ghost, content, caret.
    pub fn draw(&self, canvas: &mut impl Canvas, placement: Placement) {
        let at = Point::new(placement.rect.x0, placement.rect.y0 + self.metrics.ascent);
        let transform = Affine::translate((at.x, at.y - self.layout_baseline));
        for rect in &self.selection {
            canvas.fill(*rect, self.selection_brush.clone(), transform);
        }
        if let Some(ghost) = &self.ghost {
            draw_layout(canvas, ghost, transform);
        }
        if let Some(layout) = &self.layout {
            draw_layout(canvas, layout, transform);
        }
        if let Some(cursor) = self.cursor {
            canvas.fill(
                cursor,
                self.cursor_brush.clone(),
                Affine::translate((at.x, at.y - self.editor_baseline)),
            );
        }
    }

    /// The dispatch half: while focused, register key, drag, release,
    /// and IME against the settled placement.
    pub fn install<C: 'static, P: HasHandler<C>>(
        &self,
        p: &mut P,
        placement: Placement,
        with: impl for<'a> Fn(&'a mut C) -> Option<EditCtx<'a>> + Clone + 'static,
    ) {
        let scale = self.scale;
        let presentation = self.presentation.clone();
        let at = Point::new(placement.rect.x0, placement.rect.y0 + self.metrics.ascent);
        if self.focused {
            let text_origin = Point::new(at.x, at.y - self.editor_baseline);
            let with_key = with.clone();
            let key_presentation = presentation.clone();
            p.handler().on_key(move |ctx, event| {
                with_key(ctx).is_some_and(
                    |EditCtx {
                         state,
                         fonts,
                         layouts,
                         clipboard,
                     }| {
                        state.handle_key(
                            &key_presentation,
                            fonts,
                            layouts,
                            clipboard,
                            event,
                        )
                    },
                )
            });
            let with_move = with.clone();
            let move_presentation = presentation.clone();
            p.handler().on_pointer_move(move |ctx, update| {
                is_primary_contact_move(update)
                    && with_move(ctx).is_some_and(
                        |EditCtx {
                             state,
                             fonts,
                             layouts,
                             ..
                         }| {
                            state.pointer_move(
                                &move_presentation,
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
            let with_cancel = with.clone();
            p.handler().on_pointer_cancel(move |ctx, _| {
                with_cancel(ctx).is_some_and(|edit| edit.state.pointer_up())
            });
            p.handler()
                .on_ime(move |ctx, event| with(ctx).is_some_and(|edit| edit.state.handle_ime(event)));
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
        .filter(|_| state.text.is_empty() && !state.is_composing() && !presentation.dressed())
        .map(|(text, style)| build_layout(tcx, text, style, None, None));
    let editor = state.editor(&presentation, tcx.fonts, tcx.layouts, scale);
    let text = match &state.preedit {
        Some(preedit) => {
            let (start, end) = (state.anchor.min(state.focus), state.anchor.max(state.focus));
            format!(
                "{}{}{}{}{}",
                presentation.prefix,
                &state.text[..start],
                preedit.text,
                &state.text[end..],
                presentation.suffix,
            )
        }
        None => format!(
            "{}{}{}",
            presentation.prefix, state.text, presentation.suffix
        ),
    };
    let layout = editor.try_layout().cloned();
    let metrics_of = |layout: &Layout<Brush>| {
        let metrics = *layout.lines().next()?.metrics();
        let baseline = metrics.baseline as f64;
        Some((
            TextMetrics {
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
    let (metrics, layout_baseline) = ghost
        .as_ref()
        .and_then(|layout| metrics_of(layout))
        .or_else(|| layout.as_ref().and_then(metrics_of))
        .unwrap_or((TextMetrics::default(), 0.0));

    let selection: Vec<Rect> = if focused {
        let mut rects = Vec::new();
        editor
            .selection_geometry_with(|bb, _| rects.push(Rect::new(bb.x0, bb.y0, bb.x1, bb.y1)));
        rects
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
    LineEdit {
        text,
        metrics,
        scale,
        ghost,
        layout,
        layout_baseline,
        editor_baseline,
        selection,
        cursor,
        selection_brush: style.selection.clone(),
        cursor_brush: style.cursor.clone(),
        focused,
        presentation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{DrawCmd, DrawList, GlyphRun, Shape};
    use crate::handler::Handler;
    use kurbo::Stroke;
    use ui_events::keyboard::{KeyState, Modifiers};

    struct DrawFrame {
        list: DrawList,
        handler: Handler<()>,
    }

    impl Canvas for DrawFrame {
        fn fill(
            &mut self,
            shape: impl Into<Shape>,
            brush: impl Into<Brush>,
            transform: Affine,
        ) {
            self.list.fill(shape, brush, transform);
        }

        fn stroke(
            &mut self,
            shape: impl Into<Shape>,
            style: Stroke,
            brush: impl Into<Brush>,
            transform: Affine,
        ) {
            self.list.stroke(shape, style, brush, transform);
        }

        fn glyph_run(&mut self, run: GlyphRun) {
            self.list.glyph_run(run);
        }

        fn clip(
            &mut self,
            shape: impl Into<Shape>,
            transform: Affine,
            content: impl FnOnce(&mut Self),
        ) {
            let outer = std::mem::take(&mut self.list.0);
            content(self);
            let children = std::mem::replace(&mut self.list.0, outer);
            self.list.0.push(DrawCmd::Clip {
                shape: shape.into(),
                transform,
                children,
            });
        }
    }

    impl HasHandler<()> for DrawFrame {
        fn handler(&mut self) -> &mut Handler<()> {
            &mut self.handler
        }
    }

    fn contexts() -> (FontContext, LayoutContext<Brush>) {
        (FontContext::new(), LayoutContext::new())
    }

    fn state(text: &str) -> LineEditState {
        LineEditState::new(text)
    }

    fn presentation() -> LineEditPresentation {
        LineEditPresentation::new(16.0, Brush::default())
    }

    #[derive(Default)]
    struct MemoryClipboard(Option<String>);

    impl TextClipboard for MemoryClipboard {
        fn get_text(&mut self) -> Option<String> {
            self.0.clone()
        }

        fn set_text(&mut self, text: &str) {
            self.0 = Some(text.to_string());
        }
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
        press_with(&presentation(), state, fonts, layouts, key, modifiers)
    }

    fn press_with(
        presentation: &LineEditPresentation,
        state: &mut LineEditState,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        key: Key,
        modifiers: Modifiers,
    ) -> bool {
        press_with_clipboard(
            presentation,
            state,
            fonts,
            layouts,
            &mut MemoryClipboard::default(),
            key,
            modifiers,
        )
    }

    fn press_with_clipboard(
        presentation: &LineEditPresentation,
        state: &mut LineEditState,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<Brush>,
        clipboard: &mut dyn TextClipboard,
        key: Key,
        modifiers: Modifiers,
    ) -> bool {
        state.handle_key(
            presentation,
            fonts,
            layouts,
            clipboard,
            &key_event(key, modifiers),
        )
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
    fn presentation_can_change_without_resetting_interaction_state() {
        let (mut fonts, mut layouts) = contexts();
        let mut state = state("abc");
        state.cursor_to(1);
        let first = presentation();
        let second = LineEditPresentation::new(28.0, Brush::default());

        assert!(press_with(
            &first,
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character("x".into()),
            Modifiers::empty(),
        ));
        assert!(press_with(
            &second,
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character("y".into()),
            Modifiers::empty(),
        ));
        assert_eq!(state.text(), "axybc");
    }

    #[test]
    fn affixes_are_armor_not_content() {
        let (mut fonts, mut layouts) = contexts();
        let action = if cfg!(target_os = "macos") {
            Modifiers::META
        } else {
            Modifiers::CONTROL
        };
        let presentation = presentation().with_affixes("\"", "\"");
        let mut state = state("hi").with_cursor_at_end();

        // Typing lands between the affixes; the text stays bare.
        assert!(press_with(
            &presentation,
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character("!".into()),
            Modifiers::empty(),
        ));
        assert_eq!(state.text(), "hi!");

        // Backspace at content start bites the prefix: swallowed
        // whole — handled, nothing changes.
        assert!(press_with(
            &presentation,
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Home),
            Modifiers::empty(),
        ));
        assert!(press_with(
            &presentation,
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Backspace),
            Modifiers::empty(),
        ));
        assert_eq!(state.text(), "hi!");

        // Motion that only wanders into an affix is no motion: the
        // boundary arrow still declines to the caller.
        assert!(!press_with(
            &presentation,
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::ArrowLeft),
            Modifiers::empty(),
        ));

        // Select-all reaches the content alone; typing replaces it
        // and the affixes stand.
        assert!(press_with(
            &presentation,
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character("a".into()),
            action,
        ));
        assert!(press_with(
            &presentation,
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Character("x".into()),
            Modifiers::empty(),
        ));
        assert_eq!(state.text(), "x");

        // Delete at content end bites the suffix: swallowed too.
        state.cursor_to_end();
        assert!(press_with(
            &presentation,
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Delete),
            Modifiers::empty(),
        ));
        assert_eq!(state.text(), "x");

        // Emptied, the delete keys decline — the caller's
        // delete-the-value idiom sees through the affixes.
        assert!(press_with(
            &presentation,
            &mut state,
            &mut fonts,
            &mut layouts,
            Key::Named(NamedKey::Backspace),
            Modifiers::empty(),
        ));
        assert_eq!(state.text(), "");
        assert!(!press_with(
            &presentation,
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
        let presentation = presentation().with_affixes("\"", "\"");
        let mut words = state("hi there");
        words.anchor = 2;
        words.focus = 2;
        assert!(press_with(
            &presentation,
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
        let mut leading = state(" hi");
        leading.anchor = 1;
        leading.focus = 1;
        assert!(press_with(
            &presentation,
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
    fn clipboard_is_a_supplied_capability() {
        let (mut fonts, mut layouts) = contexts();
        let presentation = presentation();
        let action = if cfg!(target_os = "macos") {
            Modifiers::META
        } else {
            Modifiers::CONTROL
        };
        let mut clipboard = MemoryClipboard::default();
        let mut source = state("hello").with_cursor_at_end();
        assert!(press_with_clipboard(
            &presentation,
            &mut source,
            &mut fonts,
            &mut layouts,
            &mut clipboard,
            Key::Character("a".into()),
            action,
        ));
        assert!(press_with_clipboard(
            &presentation,
            &mut source,
            &mut fonts,
            &mut layouts,
            &mut clipboard,
            Key::Character("c".into()),
            action,
        ));

        let mut target = state("");
        assert!(press_with_clipboard(
            &presentation,
            &mut target,
            &mut fonts,
            &mut layouts,
            &mut clipboard,
            Key::Character("v".into()),
            action,
        ));
        assert_eq!(target.text(), "hello");
    }

    #[test]
    fn recorder_exposes_selection_and_caret_geometry() {
        let (mut fonts, mut layouts) = contexts();
        let mut cache = crate::text::TextCache::default();
        let mut tcx = TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            scale: 1.0,
            cache: &mut cache,
        };
        let mut state = state("abc");
        state.anchor = 0;
        state.focus = 2;
        let style = EditStyle {
            selection: Brush::default(),
            cursor: Brush::default(),
        };
        let edit = text_edit(
            LineEditDescription {
                state: &state,
                focused: true,
                presentation: presentation(),
                style: &style,
                placeholder: None,
            },
            &mut tcx,
        );
        let metrics = edit.metrics();
        let mut frame = DrawFrame {
            list: DrawList::new(),
            handler: Handler::new(),
        };
        edit.place::<(), DrawFrame>(
            &mut frame,
            Placement::root(Rect::new(
                20.0,
                30.0,
                20.0 + metrics.width,
                30.0 + metrics.ascent + metrics.descent,
            )),
            |_| None,
        );
        let fills: Vec<Rect> = frame
            .list
            .0
            .iter()
            .filter_map(|command| match command {
                DrawCmd::Fill {
                    shape: Shape::Rect(rect),
                    ..
                } => Some(*rect),
                _ => None,
            })
            .collect();

        assert_eq!(fills.len(), 2);
        let selection = fills[0];
        let caret = fills[1];
        assert!(selection.width() > caret.width());
        assert_eq!(caret.width(), 1.5);
        assert!(caret.height() > 0.0);
    }

    #[test]
    fn geometry_exposes_the_composed_text_for_plain_text_drawing() {
        let (mut fonts, mut layouts) = contexts();
        let mut cache = crate::text::TextCache::default();
        let mut tcx = TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            scale: 1.0,
            cache: &mut cache,
        };
        let state = LineEditState::from_parts(
            "ab",
            1,
            1,
            Some(("XY".to_string(), Some((2, 2)))),
            None,
        );
        let style = EditStyle {
            selection: Brush::default(),
            cursor: Brush::default(),
        };
        let geometry = text_edit(
            LineEditDescription {
                state: &state,
                focused: true,
                presentation: presentation().with_affixes("[", "]"),
                style: &style,
                placeholder: None,
            },
            &mut tcx,
        )
        .geometry();
        assert_eq!(geometry.text, "[aXYb]");
        assert!(geometry.cursor.is_some());
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
        let presentation = presentation();
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
        clicked.pointer_down(
            &presentation,
            &mut fonts,
            &mut layouts,
            1.0,
            LineEditPointerDown {
                point: Point::new(0.0, 5.0),
                shift: false,
                count: 1,
            },
        );
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
        let presentation = presentation();
        let mut state = state("hello world");
        let selected = |state: &LineEditState| {
            let (start, end) = (
                state.anchor.min(state.focus),
                state.anchor.max(state.focus),
            );
            state.text()[start..end].to_string()
        };

        state.pointer_down(
            &presentation,
            &mut fonts,
            &mut layouts,
            1.0,
            LineEditPointerDown {
                point: Point::new(2.0, 5.0),
                shift: false,
                count: 2,
            },
        );
        assert_eq!(selected(&state), "hello");

        // Extending keeps the word anchor across transient editors...
        assert!(state.pointer_move(
            &presentation,
            &mut fonts,
            &mut layouts,
            1.0,
            Point::new(10_000.0, 5.0),
        ));
        assert_eq!(selected(&state), "hello world");

        // ...and dragging back re-collapses to the anchor word.
        assert!(state.pointer_move(
            &presentation,
            &mut fonts,
            &mut layouts,
            1.0,
            Point::new(2.0, 5.0),
        ));
        assert_eq!(selected(&state), "hello");
    }

    #[test]
    fn drag_extends_selection_until_released() {
        let (mut fonts, mut layouts) = contexts();
        let presentation = presentation();
        let mut state = state("hello world");

        state.pointer_down(
            &presentation,
            &mut fonts,
            &mut layouts,
            1.0,
            LineEditPointerDown {
                point: Point::new(0.0, 5.0),
                shift: false,
                count: 1,
            },
        );
        assert!(state.pointer_move(
            &presentation,
            &mut fonts,
            &mut layouts,
            1.0,
            Point::new(10_000.0, 5.0),
        ));
        assert_eq!((state.anchor, state.focus), (0, 11));

        assert!(state.pointer_up());
        assert!(!state.pointer_up());
        assert!(!state.pointer_move(
            &presentation,
            &mut fonts,
            &mut layouts,
            1.0,
            Point::new(0.0, 5.0),
        ));
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
