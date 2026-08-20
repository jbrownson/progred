//! The editor's tree-projection runtime: interpret display layouts,
//! retain source provenance, and fall back to total structural display.

#[cfg(test)]
use crate::completion::{resolve_entry, resolve_label};
use crate::completion::{Entry, EntryAction, HasPopup, Popup, completion_entries};
use crate::filter;
use crate::frame::Hovered;
use crate::hover::Hover;
use crate::identity::short_id;
use crate::navigate::{Descend, HasDescends};
use crate::placed::{self, Placed, before, decorate, leaf, on_key};
use measured::{Extent, Measured, col, min_width, pad, row};
use puri::hover::Claim;
#[cfg(test)]
use crate::navigate::{projected_name_owner, step_selection};
use crate::render::{self, text};
#[cfg(test)]
use crate::sample::{sample_document, sample_vocabulary};
use crate::annotations::Annotations;
use crate::selection::{Selection, Stage, last_follow};
#[cfg(test)]
use crate::selection::{
    break_edit_run, delete_edge, from_clipboard, from_structure, pending_edge,
    pending_follow, pending_insert, pending_into, pending_rename, pending_value, rename_field,
    resolve_query, set_collapse, set_value, to_clipboard, toggle_collapse, write_through,
};
use crate::sources::Sources;
use crate::styles::Styles;
use progred_libraries::{absent, layout as layout_data, text};
mod location;
use gid::{CellId, Path, Step, Value};
#[cfg(test)]
use gid::{Cells, Document, new_cell_id};
use location::Location;
use puri::delim::{self, Delim, DelimStyle};
use puri::draw::Canvas;
use puri::edit::{
    EditCtx, LineEditDescription, LineEditPointerDown, LineEditPresentation, LineEditState,
};
use puri::geometry::Placement;
use puri::handler::HasHandler;
use puri::text::{TextCtx, TextStyle, caret_index, line_layout};
use std::collections::HashSet;
use std::rc::Rc;
#[cfg(test)]
use ui_events::keyboard::KeyboardEvent;
use ui_events::keyboard::{Key, NamedKey};
use ui_events::pointer::PointerButton;
use vello::kurbo::{Affine, Insets, Point, Rect, RoundedRect, Stroke};
use vello::peniko::{Brush, Color};

/// One ordered composition of partial value projections. The
/// structural fallback lives in this runtime and is always total.
pub struct Projection<World> {
    partials: Box<[progred_display::Partial<World, Hover>]>,
}

impl<World> Clone for Projection<World> {
    fn clone(&self) -> Self {
        Self {
            partials: self.partials.clone(),
        }
    }
}

impl<World> Default for Projection<World> {
    fn default() -> Self {
        Self {
            partials: Box::new([]),
        }
    }
}

impl<World> Projection<World> {
    pub fn new(partials: impl IntoIterator<Item = progred_display::Partial<World, Hover>>) -> Self {
        Self {
            partials: partials.into_iter().collect(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply(
        &self,
        env: &dyn progred_display::Env,
        value: &Value,
        selection: Option<&Value>,
        state: Option<&Value>,
        select: progred_display::ClickHandler<World>,
        hover: Hover,
    ) -> Option<progred_display::Layout<World, Hover>> {
        self.partials.iter().find_map(|partial| {
            partial(progred_display::ProjectionInput {
                env,
                value,
                selection,
                state,
                select: select.clone(),
                hover: hover.clone(),
            })
        })
    }

    pub fn line(&self, value: &Value) -> Option<render::LineEdit> {
        self.apply(&NoEval, value, None, None, Rc::new(|_| false), Hover::Value(Vec::new()))
        .and_then(|layout| progred_display::line_edit_of(&layout).cloned())
    }
}

struct NoEval;

impl progred_display::Env for NoEval {
    fn evaluate(&self, _: &Value) -> (Value, usize) {
        (Value::record([]), 0)
    }
}

/// Read-only projection context threaded through every view.
struct Cx<'a> {
    /// The reading context: the document read over its library.
    sources: Sources<'a>,
    /// Names and field order derive from this view bit. Value
    /// projections come from the editor's stack; `grap` is one of them.
    raw: bool,
    foreign: &'a grap::ForeignFunctions,
    annotations: &'a Annotations,
    styles: &'a Styles,
    selection: Option<&'a Selection>,
    /// The value whose other projections carry the secondary mark.
    secondary: Option<Value>,
    source: Source<'a>,
    fuel: std::cell::Cell<usize>,
}

#[derive(Clone, Copy)]
enum Source<'a> {
    Stored,
    Transient { owner: &'a [Step] },
}

impl Source<'_> {
    fn transient(self) -> bool {
        matches!(self, Self::Transient { .. })
    }
}

struct ProjectEnv<'a, 's> {
    cx: &'a Cx<'s>,
}

impl progred_display::Env for ProjectEnv<'_, '_> {
    fn evaluate(&self, expression: &Value) -> (Value, usize) {
        let fuel = if self.cx.source.transient() {
            self.cx.fuel.get()
        } else {
            grap::DEFAULT_FUEL
        };
        let evaluation = grap::evaluate(
            expression,
            |cell| self.cx.sources.value(cell).cloned(),
            self.cx.foreign,
            fuel,
        );
        self.cx.fuel.set(evaluation.remaining_fuel);
        (evaluation.result, evaluation.remaining_fuel)
    }
}

/// Lower a projection layout to measured boxes. Display leaves
/// become place-continuations; [`OnClick`] becomes a Puri handler.
#[allow(clippy::too_many_arguments)]
fn realize<
    C: 'static,
    Cv: Canvas + 'static,
>(
    cx: &Cx,
    projection: Option<&Projection<C>>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    hooks: &Hooks<C>,
    value: &Value,
    layout: progred_display::Layout<C, Hover>,
    avail: f64,
) -> Measured<Placed<C, Cv>> {
    let scale = cx.styles.scale;
    match layout {
        progred_display::Layout::Leaf(content) => {
            leaf_display(cx, tcx, path, hooks, value, content)
        }
        progred_display::Layout::OnClick { child, handler } => {
            let inner = realize(
                cx, projection, tcx, path, ancestors, hooks, value, *child, avail,
            );
            realize_click(handler, inner)
        }
        progred_display::Layout::OnPick { child, value: picked } => {
            let inner = realize(
                cx, projection, tcx, path, ancestors, hooks, value, *child, avail,
            );
            realize_pick(picked, hooks, inner)
        }
        progred_display::Layout::OnApply {
            child,
            function,
        } => {
            let inner = realize(
                cx, projection, tcx, path, ancestors, hooks, value, *child, avail,
            );
            realize_apply(path.to_vec(), function, hooks, inner)
        }
        progred_display::Layout::OnHover { child, hover } => {
            let inner = realize(
                cx, projection, tcx, path, ancestors, hooks, value, *child, avail,
            );
            realize_hover(cx, hover, inner)
        }
        progred_display::Layout::Row { gap, children } => {
            let children = children
                .into_iter()
                .map(|child| {
                    realize(
                        cx, projection, tcx, path, ancestors, hooks, value, child, avail,
                    )
                })
                .collect();
            row(gap * scale, children)
        }
        progred_display::Layout::Col {
            baseline,
            gap,
            children,
        } => {
            let children = children
                .into_iter()
                .map(|child| {
                    realize(
                        cx, projection, tcx, path, ancestors, hooks, value, child, avail,
                    )
                })
                .collect();
            col(baseline, gap * scale, children)
        }
        progred_display::Layout::Pad {
            left,
            top,
            right,
            bottom,
            child,
        } => {
            let left = left * scale;
            let right = right * scale;
            pad(
                Insets::new(left, top * scale, right, bottom * scale),
                realize(
                    cx,
                    projection,
                    tcx,
                    path,
                    ancestors,
                    hooks,
                    value,
                    *child,
                    (avail - left - right).max(0.0),
                ),
            )
        }
        progred_display::Layout::Surround {
            left,
            child,
            right,
        } => {
            let gap = 2.0 * scale;
            let reserved = side_advance(cx.styles, &left)
                + side_advance(cx.styles, &right)
                + 2.0 * gap;
            let inner = realize(
                cx,
                projection,
                tcx,
                path,
                ancestors,
                hooks,
                value,
                *child,
                (avail - reserved).max(0.0),
            );
            surround_sides(cx, tcx, path, value, hooks, left, inner, right)
        }
        progred_display::Layout::Descend { step } => descend(
            cx, projection, tcx, path, ancestors, value, step, avail, hooks,
        ),
        progred_display::Layout::At {
            steps,
            value: nested,
        } => realize_at(
            cx, projection, tcx, path, ancestors, steps, nested, avail, hooks,
        ),
        progred_display::Layout::Transient {
            value: computed,
            fuel,
        } => project_transient_root(cx, projection, tcx, path, computed, fuel, avail, hooks),
        progred_display::Layout::Alternatives(mut options) => {
            let Some(accommodating) = options.pop() else {
                return row(0.0, Vec::new());
            };
            if avail <= 0.0 {
                return realize(
                    cx,
                    projection,
                    tcx,
                    path,
                    ancestors,
                    hooks,
                    value,
                    accommodating,
                    avail,
                );
            }
            // Order is preference: options before the last realize in
            // their natural, unbounded form — nested alternatives pick
            // their own firsts — and the first whose width fits wins.
            // The last accommodates the real width. When nothing fits,
            // the narrowest wins; earlier options win ties.
            let mut tried = Vec::new();
            for option in options {
                let candidate = realize(
                    cx,
                    projection,
                    tcx,
                    path,
                    ancestors,
                    hooks,
                    value,
                    option,
                    f64::INFINITY,
                );
                if candidate.extent.width <= avail {
                    return candidate;
                }
                tried.push(candidate);
            }
            let mut best = realize(
                cx,
                projection,
                tcx,
                path,
                ancestors,
                hooks,
                value,
                accommodating,
                avail,
            );
            for candidate in tried.into_iter().rev() {
                if candidate.extent.width <= best.extent.width {
                    best = candidate;
                }
            }
            best
        }
    }
}

fn display_delim(delim: progred_display::Delim) -> Delim {
    match delim {
        progred_display::Delim::Paren => Delim::Paren,
        progred_display::Delim::Bracket => Delim::Bracket,
        progred_display::Delim::Brace => Delim::Brace,
    }
}

fn realize_at<
    C: 'static,
    Cv: Canvas + 'static,
>(
    cx: &Cx,
    projection: Option<&Projection<C>>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    steps: Vec<Step>,
    nested: Value,
    avail: f64,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    let mut path = path.to_vec();
    let mut follow_ancestors = ancestors.clone();
    for step in &steps {
        if *step == Step::Follow {
            if let Some(cell) = cx.sources.resolve(&path).and_then(Value::as_cell) {
                follow_ancestors.insert(cell);
            }
        }
        path.push(step.clone());
    }
    project_present_value(
        cx,
        projection,
        tcx,
        &path,
        &follow_ancestors,
        &nested,
        avail,
        hooks,
    )
}

fn realize_click<C: 'static, Cv: Canvas + 'static>(
    handler: progred_display::ClickHandler<C>,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, placement| {
        p.handler().on_pointer_down(move |world, event| {
            event.button == Some(PointerButton::Primary)
                && !command(&event.state.modifiers)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && handler(world)
        });
    })
}

/// A command-click picks the named identity; anything else falls
/// through. Declines on a failed pick too, so the value target's own
/// pick-or-select backstop answers.
fn realize_apply<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    function: Value,
    hooks: &Hooks<C>,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    let apply = hooks.apply.clone();
    before(inner, move |p, placement| {
        p.handler().on_pointer_down(move |world, event| {
            event.button == Some(PointerButton::Primary)
                && !command(&event.state.modifiers)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && apply(world, path.clone(), function.clone())
        });
    })
}

fn realize_pick<C: 'static, Cv: Canvas + 'static>(
    picked: Value,
    hooks: &Hooks<C>,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    let pick = hooks.pick.clone();
    before(inner, move |p, placement| {
        p.handler().on_pointer_down(move |world, event| {
            event.button == Some(PointerButton::Primary)
                && command(&event.state.modifiers)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && pick(world, picked.clone())
        });
    })
}

fn realize_hover<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    hover: Option<Hover>,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    let highlight = matches!(
        hover.as_ref(),
        Some(Hover::Label(_) | Hover::Toggle(_) | Hover::Insert(_))
    );
    let scale = cx.styles.scale;
    before(inner, move |p, placement| {
        match hover {
            Some(hover) => {
                if highlight {
                    light_hover(p, placement, hover, scale);
                } else {
                    hover_claim(p, placement, hover);
                }
            }
            None => hover_block(p, placement),
        }
    })
}

/// Claim `hover` and, when it is the resolved hover, wash the box.
fn light_hover<C: 'static, Cv: Canvas + 'static>(
    p: &mut placed::Builder<C, Cv>,
    placement: Placement,
    hover: Hover,
    scale: f64,
) {
    let mine = hover.clone();
    p.ink(move |cv, ink| {
        if tree_hovered(ink) == Some(&mine) {
            hover_highlight(scale, cv, placement.rect);
        }
    });
    hover_claim(p, placement, hover);
}

/// The resolved hover's tree identity, for ink that lights its own
/// claim.
fn tree_hovered<'a>(ink: placed::Ink<'a>) -> Option<&'a Hover> {
    match ink.hovered {
        Some(Hovered::Tree(hover)) => Some(hover),
        _ => None,
    }
}

fn leaf_display<
    C: 'static,
    Cv: Canvas + 'static,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    hooks: &Hooks<C>,
    value: &Value,
    content: progred_display::Display,
) -> Measured<Placed<C, Cv>> {
    match content {
        progred_display::Display::Text { text, face } => {
            render::text(tcx, &text, face_style(cx.styles, face))
        }
        progred_display::Display::Label { key } => label_view(cx, tcx, path, key, hooks),
        progred_display::Display::Query => {
            let engaged = cx
                .pending_rename_under(path)
                .map(|(_, query, _)| query)
                .or_else(|| cx.pending_edge_under(path).map(|(query, _)| query));
            match engaged {
                Some(query) => label_query(cx, tcx, query, hooks),
                // The projection only emits this leaf where it saw a
                // matching pending; a mismatch is a malformed state,
                // shown as elision rather than hidden.
                None => render::text(tcx, "…", &cx.styles.dim),
            }
        }
        progred_display::Display::Ink { ink, face } => {
            ink_leaf(cx, tcx, ink, face, None)
        }
        progred_display::Display::LineEdit(line) => {
            let editing = cx
                .selection
                .filter(|selection| selection.path() == path)
                .and_then(Selection::edit);
            let edit = hooks.edit.clone();
            let content = render::line_edit(tcx, cx.styles, &line, editing, move |c| edit(c));
            cursor_target(
                path.to_vec(),
                value.clone(),
                cx.styles.line_presentation(&line.prefix, &line.suffix),
                hooks,
                Some(line),
                content,
            )
        }
    }
}

/// A reported click on projected text, in text-local coordinates.
/// The shell's selection transition consumes it to seed or advance
/// the editor state — focus and caret placement are one event, as in
/// the Haskell LineEdit's focus-with-initial-selection callback. The
/// count carries double/triple clicks (word and line selection).
pub struct TextClick {
    pub point: Point,
    pub shift: bool,
    pub count: u8,
    pub presentation: LineEditPresentation,
    /// Set when the click is on an [`editable_line`]; the selection
    /// mounts that line instead of looking the value up again.
    pub line: Option<render::LineEdit>,
}

/// Dispatch-time callbacks the shell injects: what selecting a path
/// (optionally with a text click) does, what toggling a collapse
/// does, and how a dispatch reaches the selection's editor state and
/// measurement caches.
pub struct Hooks<C> {
    pub select: Rc<dyn Fn(&mut C, Path, Option<TextClick>)>,
    pub toggle: Rc<dyn Fn(&mut C, Path)>,
    /// Re-open the label of the field at `path` (a Key path) as its
    /// seeded query — the click gesture on a writable field's label.
    /// The byte index is the click hit-tested against the label's own
    /// layout; the seed shares its spelling, so the shell lands the
    /// caret there in whatever face the editor draws.
    pub rename: Rc<dyn Fn(&mut C, Path, usize)>,
    /// None when the editor is already gone — retained-frame dispatch
    /// may fire a frame late, and absent state declines.
    pub edit: Rc<dyn for<'a> Fn(&'a mut C) -> Option<EditCtx<'a>>>,
    /// Commit a pointed-at value into the open pending (value or
    /// label stage); false when nothing is pending, so the click
    /// falls through to selection.
    pub pick: Rc<dyn Fn(&mut C, Value) -> bool>,
    /// Open a pending sibling after the element at `path` — the flat
    /// list separator's click.
    pub insert: Rc<dyn Fn(&mut C, Path)>,
    /// Delete the selected edge. Installed on the selected descend
    /// so Raw and library projections share one handler.
    pub delete: Rc<dyn Fn(&mut C) -> bool>,
    /// Apply a Grap callable at `path` with the site overlay.
    pub apply: Rc<dyn Fn(&mut C, Path, Value) -> bool>,
}

/// The platform command modifier, for pointer gestures.
pub(crate) fn command(modifiers: &ui_events::keyboard::Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        modifiers.meta()
    } else {
        modifiers.ctrl()
    }
}

fn select_handler<C: 'static>(path: Path, hooks: &Hooks<C>) -> progred_display::ClickHandler<C> {
    let select = hooks.select.clone();
    Rc::new(move |world| {
        select(world, path.clone(), None);
        true
    })
}

impl Cx<'_> {
    /// The display name at this projection. Raw interprets no naming
    /// convention and therefore falls back to the short id.
    fn name(&self, cell: CellId) -> Option<&str> {
        (!self.raw).then(|| self.sources.name(cell)).flatten()
    }

    /// Whether `path` carries the primary highlight. A label-stage
    /// pending deliberately does not mark its parent — nothing is
    /// selected there, something is being authored inside; the
    /// pending row carries the highlight itself.
    fn selected(&self, path: &[Step]) -> bool {
        self.selection
            .is_some_and(|current| current.stage() != Stage::Label && current.path() == path)
    }

    /// The pending child step under `path`, when the selection is
    /// authoring one there.
    fn pending_child_of(&self, path: &[Step]) -> Option<Step> {
        let current = self.selection?;
        (current.stage() == Stage::Pending
            && current
                .path()
                .split_last()
                .is_some_and(|(_, parent)| parent == path))
        .then(|| current.path().last().cloned())
        .flatten()
    }

    /// The label query of a new field being authored on the record at
    /// `path`.
    fn pending_edge_under(&self, path: &[Step]) -> Option<(&LineEditState, usize)> {
        let current = self.selection?;
        (current.stage() == Stage::Label
            && current.replacing().is_none()
            && current.path() == path)
            .then(|| Some((current.edit()?, current.choice())))
            .flatten()
    }

    /// The re-opened label of an existing field on the record at
    /// `path`, with the key it replaces.
    fn pending_rename_under(&self, path: &[Step]) -> Option<(CellId, &LineEditState, usize)> {
        let current = self.selection?;
        if current.stage() != Stage::Label || current.path() != path {
            return None;
        }
        let replacing = current.replacing()?;
        Some((replacing, current.edit()?, current.choice()))
    }
}

fn edit_presentation(style: &TextStyle) -> LineEditPresentation {
    LineEditPresentation::new(style.size, style.brush.clone())
}

/// The delimiter metrics that marry the drawn family to the text:
/// the system font's own glyphs span -0.704..+0.171 em around the
/// baseline while its line box spans -0.929..+0.249, so a stretched
/// delimiter trims the difference at each end — it meets the glyph
/// span on its first and last lines, and a one-line span IS the
/// glyph's. Measured by `puri`'s delimiter_bench example.
const GLYPH_ASC_EM: f64 = 0.704;
const GLYPH_DESC_EM: f64 = 0.171;
const TOP_TRIM_EM: f64 = 0.929 - GLYPH_ASC_EM;
const BOTTOM_TRIM_EM: f64 = 0.249 - GLYPH_DESC_EM;
const SIDE_BEARING_EM: f64 = 0.05;

fn delim_style(styles: &Styles) -> DelimStyle {
    DelimStyle::for_text_size(14.0 * styles.scale)
}

/// A delimiter's advance: the FLAT ink plus both side bearings —
/// what layout charges at any height. A grown tall delimiter
/// OVERHANGS its advance on the outward side, the way a glyph's ink
/// may exceed its advance; layout never pays for growth.
fn delim_advance(styles: &Styles, delim: Delim) -> f64 {
    delim_style(styles).bow(delim) + 2.0 * SIDE_BEARING_EM * 14.0 * styles.scale
}

fn side_advance(styles: &Styles, display: &progred_display::Display) -> f64 {
    match display {
        progred_display::Display::Ink {
            ink: progred_display::Ink::Delim { delim, .. },
            ..
        } => delim_advance(styles, display_delim(*delim)),
        progred_display::Display::Ink {
            ink: progred_display::Ink::Frame,
            ..
        } => slot_width(styles),
        _ => delim_advance(styles, Delim::Paren),
    }
}

/// A drawn delimiter leaf: `extent` is what layout sees (the FLAT
/// advance, the span it must cover) while the ink inside spans
/// `ink_top..ink_bottom` relative to the baseline, stroked in the dim
/// brush like the text delimiters it replaces. A grown tall
/// delimiter keeps its terminals where the flat form's would be and
/// bulges OUTWARD past its advance — typographic overhang, so growth
/// costs layout nothing and nested delimiters bow into each other's
/// empty sides.
fn delim_leaf<C: 'static, Cv: Canvas + 'static>(
    styles: &Styles,
    delim: Delim,
    open: bool,
    extent: Extent,
    ink_top: f64,
    ink_bottom: f64,
    brush: Brush,
) -> Measured<Placed<C, Cv>> {
    let style = delim_style(styles);
    let bearing = SIDE_BEARING_EM * 14.0 * styles.scale;
    let path = if open {
        delim::open(delim, &style, ink_top, ink_bottom)
    } else {
        delim::close(delim, &style, ink_top, ink_bottom)
    };
    let overhang = style.bow_for(delim, ink_bottom - ink_top) - style.bow(delim);
    let ink_x = if open { bearing - overhang } else { bearing };
    leaf(
        Extent {
            width: style.bow(delim) + 2.0 * bearing,
            ..extent
        },
        move |p, placement| {
            let at = Point::new(placement.rect.x0, placement.rect.y0 + extent.ascent);
            p.fill(
                path.clone(),
                brush.clone(),
                Affine::translate((at.x + ink_x, at.y)),
            );
        },
    )
}

/// A delimiter stretched over `content`'s extent, ink trimmed to meet
/// the glyph span on the first and last lines. The charged span never
/// shrinks below the glyph's own, so an empty pair still stands a
/// glyph tall — and one-line content gets exactly the flat form.
fn tall_delim<C: 'static, Cv: Canvas + 'static>(
    styles: &Styles,
    delim: Delim,
    open: bool,
    content: Extent,
    brush: Brush,
) -> Measured<Placed<C, Cv>> {
    let em = 14.0 * styles.scale;
    let content = Extent {
        width: content.width,
        ascent: content.ascent.max(GLYPH_ASC_EM * em),
        descent: content.descent.max(GLYPH_DESC_EM * em),
    };
    let ink_top = -(content.ascent - TOP_TRIM_EM * em).max(GLYPH_ASC_EM * em);
    let ink_bottom = (content.descent - BOTTOM_TRIM_EM * em).max(GLYPH_DESC_EM * em);
    delim_leaf(styles, delim, open, content, ink_top, ink_bottom, brush)
}

fn face_style(styles: &Styles, face: progred_display::Face) -> &TextStyle {
    match face {
        progred_display::Face::Name => &styles.name,
        progred_display::Face::Dim => &styles.dim,
        progred_display::Face::Label => &styles.label,
        progred_display::Face::Id => &styles.id,
    }
}

fn glyph_extent(styles: &Styles) -> Extent {
    let em = 14.0 * styles.scale;
    Extent {
        width: 0.0,
        ascent: GLYPH_ASC_EM * em,
        descent: GLYPH_DESC_EM * em,
    }
}

fn ink_leaf<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    tcx: &mut TextCtx,
    ink: progred_display::Ink,
    face: progred_display::Face,
    stretch: Option<Extent>,
) -> Measured<Placed<C, Cv>> {
    let brush = face_style(cx.styles, face).brush.clone();
    match ink {
        progred_display::Ink::Delim { delim, side } => tall_delim(
            cx.styles,
            display_delim(delim),
            matches!(side, progred_display::Side::Open),
            stretch.unwrap_or_else(|| glyph_extent(cx.styles)),
            brush,
        ),
        progred_display::Ink::Frame => {
            let extent = stretch.unwrap_or_else(|| Extent {
                width: slot_width(cx.styles),
                ..text::<C, Cv>(tcx, "", &cx.styles.name).extent
            });
            frame_leaf(cx.styles.scale, brush, extent)
        }
    }
}

fn frame_leaf<C: 'static, Cv: Canvas + 'static>(
    scale: f64,
    brush: Brush,
    extent: Extent,
) -> Measured<Placed<C, Cv>> {
    leaf(extent, move |p, placement| {
        p.stroke(
            highlight_rect(scale, placement.rect),
            Stroke::new(scale),
            brush,
            Affine::IDENTITY,
        );
    })
}

/// Place `left` and `right` in the side columns of `content`: same
/// height as the child, width the flat advance. The display nodes
/// paint; this only allocates and keeps the sides as handles.
fn surround_sides<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    target: &Value,
    hooks: &Hooks<C>,
    left: progred_display::Display,
    content: Measured<Placed<C, Cv>>,
    right: progred_display::Display,
) -> Measured<Placed<C, Cv>> {
    let extent = content.extent;
    let gap = 2.0 * cx.styles.scale;
    row(
        0.0,
        vec![
            select_target(
                path.to_vec(),
                target.clone(),
                hooks,
                pad(
                    Insets::new(0.0, 0.0, gap, 0.0),
                    paint_side(cx, tcx, path, hooks, target, left, extent),
                ),
            ),
            content,
            select_target(
                path.to_vec(),
                target.clone(),
                hooks,
                pad(
                    Insets::new(gap, 0.0, 0.0, 0.0),
                    paint_side(cx, tcx, path, hooks, target, right, extent),
                ),
            ),
        ],
    )
}

fn paint_side<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    hooks: &Hooks<C>,
    value: &Value,
    display: progred_display::Display,
    extent: Extent,
) -> Measured<Placed<C, Cv>> {
    match display {
        progred_display::Display::Ink { ink, face } => {
            ink_leaf(cx, tcx, ink, face, Some(extent))
        }
        other => leaf_display(cx, tcx, path, hooks, value, other),
    }
}

/// The one width every slot state shares: the cold box IS this wide,
/// and the engaged query's frame never lets the field get narrower —
/// the parity that keeps engagement from moving anything sideways.
fn slot_width(styles: &Styles) -> f64 {
    1.5 * 14.0 * styles.scale
}

/// The cold slot's ink: an empty rounded outline, the box marking
/// absence apart from projectional syntax (`…` is elision) — blank
/// on purpose, no ghost words. It is [`highlight_rect`] itself in
/// the dim brush — THE box, drawn the one way every box is drawn —
/// so engaging (the ring, blue over the same frame) and committing
/// (the ring over the same glyphs) redraw the same shape and only
/// the paint changes. The charge is exactly the text frame: the
/// empty line SHAPED, the same runtime metrics the engaged editor's
/// frame takes — no measured constants, one source.
fn placeholder_box<C: 'static, Cv: Canvas + 'static>(tcx: &mut TextCtx, styles: &Styles) -> Measured<Placed<C, Cv>> {
    let extent = Extent {
        width: slot_width(styles),
        ..text::<C, Cv>(tcx, "", &styles.name).extent
    };
    frame_leaf(styles.scale, styles.dim.brush.clone(), extent)
}

/// THE box: the one geometry every box around content takes — the
/// content rect plus breathing room, rounded. The selection ring
/// draws it in blue, the cold placeholder in dim; sharing the shape
/// is what keeps slot → pending → committed value from ever
/// changing the box. Sized so the QUIET wearer fits: the cold box
/// stands beside delimiters permanently, and this outset keeps its
/// hairline clear of a paren's ink where the old ring-sized box
/// overlapped.
fn highlight_rect(scale: f64, rect: Rect) -> RoundedRect {
    RoundedRect::from_rect(rect.inset(2.0 * scale), 4.0 * scale)
}

/// The pointer over this settled rect names `key`, with the visible
/// ink as its footprint. Placement order is precedence: descendants
/// and overlays contribute later and answer first.
fn hover_claim<C: 'static, Cv: 'static>(
    p: &mut placed::Builder<C, Cv>,
    placement: Placement,
    key: Hover,
) {
    p.claim(move |point| {
        placement
            .contains(point)
            .then(|| Claim::Names(Hovered::Tree(key.clone())))
    });
}

/// An occluder: takes the pointer and names nothing, so targets
/// beneath an overlay never light.
fn hover_block<C: 'static, Cv: 'static>(p: &mut placed::Builder<C, Cv>, placement: Placement) {
    p.claim(move |point| placement.contains(point).then_some(Claim::Occludes));
}

/// The pointer's preview of a click's meaning: the same box the
/// primary would ring, washed faint — hover never outranks selection.
fn hover_highlight<P: Canvas>(scale: f64, p: &mut P, rect: Rect) {
    p.fill(
        highlight_rect(scale, rect),
        Color::new([0.0, 0.48, 1.0, 0.08]),
        Affine::IDENTITY,
    );
}

/// The pane-local primary: translucent system blue, like the Swift
/// version's selection, ringed at full strength — the strongest mark
/// in the shared vocabulary.
fn primary_highlight<P: Canvas>(scale: f64, p: &mut P, rect: Rect) {
    let bg = highlight_rect(scale, rect);
    p.fill(bg, Color::new([0.0, 0.48, 1.0, 0.22]), Affine::IDENTITY);
    p.stroke(
        bg,
        Stroke::new(2.5 * scale),
        Color::new([0.0, 0.48, 1.0, 1.0]),
        Affine::IDENTITY,
    );
}

/// Marks CONTENT-SHAPED `child` as the projection of the value at
/// `path` — its bounding box is all ink (a pending's query, an
/// engaged name, the empty-document placeholder), so the whole box
/// is an honest click target. On placement it draws the highlight
/// when this is the selected path, registers a click that selects it
/// (innermost wins by handler precedence) — or, with the command
/// modifier and a pending open, picks `value` into it — and records
/// itself for keyboard navigation. Views whose boxes span structural
/// whitespace use [`descend_landmark`] plus explicit content claims
/// instead.
fn source_target<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    path: Path,
    value: Option<Value>,
    hooks: &Hooks<C>,
    child: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    let (path, transient) = match cx.source {
        Source::Transient { owner } if owner != path.as_slice() => return child,
        Source::Transient { owner } => (owner.to_vec(), true),
        Source::Stored => (path, false),
    };
    let scale = cx.styles.scale;
    let selected = cx.selected(&path);
    let select = hooks.select.clone();
    let pick = hooks.pick.clone();
    before(child, move |p, placement| {
        let rect = placement.rect;
        let highlight_path = path.clone();
        p.ink(move |cv, ink| {
            if selected {
                primary_highlight(scale, cv, rect);
            } else if matches!(tree_hovered(ink), Some(Hover::Value(hovered)) if *hovered == highlight_path)
            {
                hover_highlight(scale, cv, rect);
            }
        });
        if !transient {
            hover_claim(p, placement, Hover::Value(path.clone()));
        }
        let select = select.clone();
        let pick = pick.clone();
        let target = path.clone();
        let value = value.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && {
                    let picked = command(&event.state.modifiers)
                        && value.as_ref().is_some_and(|value| pick(ctx, value.clone()));
                    if !picked {
                        select(ctx, target.clone(), None);
                    }
                    true
                }
        });
        if !transient {
            p.descends().push(Descend { path, rect });
        }
    })
}

/// The cell marked as the secondary selection: the one at the
/// selected path. Marks mean IDENTITY — the same cell projecting in
/// many places — never equal copies, so only cell values answer.
fn secondary_of(sources: &Sources, selection: Option<&Selection>) -> Option<Value> {
    match selection? {
        current if current.stage() == Stage::Edge => sources
            .resolve(current.path())
            .filter(|value| value.as_cell().is_some())
            .cloned(),
        _ => None,
    }
}

/// The explicit-state boundary: everything a projection pass reads.
/// `width` is the space the projection may fill; containers choose
/// flat or broken forms greedily from the root down.
pub struct ProjectDescription<'a, World> {
    pub sources: Sources<'a>,
    pub selection: Option<&'a Selection>,
    pub graph_node: Option<&'a Value>,
    pub annotations: &'a Annotations,
    pub raw: bool,
    pub styles: &'a Styles,
    pub width: f64,
    pub projection: Option<&'a Projection<World>>,
    pub foreign: &'a grap::ForeignFunctions,
}

pub fn project<
    C: 'static,
    Cv: Canvas + 'static,
>(
    description: ProjectDescription<'_, C>,
    tcx: &mut TextCtx,
    hooks: Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    let ProjectDescription {
        sources,
        selection,
        graph_node,
        annotations,
        raw,
        styles,
        width,
        projection,
        foreign,
    } = description;
    let cx = Cx {
        sources,
        raw,
        foreign,
        annotations,
        styles,
        selection,
        source: Source::Stored,
        fuel: std::cell::Cell::new(grap::DEFAULT_FUEL),
        // The graph view's selected cell is a secondary here too:
        // its projections are the same value. The HOVERED value's
        // faint marks come from the render pass's Ink instead.
        secondary: secondary_of(&sources, selection).or_else(|| graph_node.cloned()),
    };
    // An empty document is a selectable placeholder at the root path.
    project_location(
        &cx,
        projection,
        tcx,
        &[],
        &HashSet::new(),
        Location::Root(sources.root()),
        width,
        &hooks,
    )
}

/// Marks `child` as the projection of `path` WITHOUT claiming any
/// clicks: the highlight, reveal rect, and keyboard reach of
/// [`descend`] over the full bounds, while pointer selection belongs
/// to the content targets the view registers — heads, delimiters,
/// rows — so clicks on structural whitespace (gutters, inter-row
/// gaps, the dead space inside a bounding box) fall through to the
/// background's deselect.
fn descend_landmark<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    path: Path,
    hooks: &Hooks<C>,
    child: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    if cx.source.transient() {
        return child;
    }
    let selected = cx.selected(&path);
    let scale = cx.styles.scale;
    let marked = decorate(child, move |p, rect| {
        let highlight_path = path.clone();
        p.ink(move |cv, ink| {
            if selected {
                primary_highlight(scale, cv, rect);
            } else if matches!(tree_hovered(ink), Some(Hover::Value(hovered)) if *hovered == highlight_path)
            {
                hover_highlight(scale, cv, rect);
            }
        });
        p.descends().push(Descend {
            path: path.clone(),
            rect,
        });
    });
    if selected {
        bind_delete(cx, hooks, marked)
    } else {
        marked
    }
}

fn bind_delete<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    hooks: &Hooks<C>,
    child: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    if cx.source.transient() {
        return child;
    }
    let delete = hooks.delete.clone();
    on_key(child, move |ctx, event| {
        crate::plain(event)
            && matches!(
                &event.key,
                Key::Named(NamedKey::Backspace | NamedKey::Delete)
            )
            && event.state.is_down()
            && delete(ctx)
    })
}

/// A record field's label: its spelling, and — when the record is
/// writable — the click that re-opens it as a rename with the caret
/// hit-tested under the pointer, in the label's own face. Command
/// declines, so the label's key pick and the value backstop answer.
fn label_view<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    key: CellId,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    let (spelling, style) = label_spelling(cx, &key);
    let content = render::text(tcx, &spelling, style);
    if !crate::selection::writable_at(&cx.sources, path) || cx.source.transient() {
        return content;
    }
    let mut target = path.to_vec();
    target.push(Step::Key(key));
    let layout = line_layout(tcx, &spelling, style);
    let rename = hooks.rename.clone();
    let scale = cx.styles.scale;
    before(content, move |p, placement| {
        light_hover(p, placement, Hover::Label(target.clone()), scale);
        let rename = rename.clone();
        let target = target.clone();
        p.handler().on_pointer_down(move |world, event| {
            event.button == Some(PointerButton::Primary)
                && !command(&event.state.modifiers)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && {
                    rename(
                        world,
                        target.clone(),
                        caret_index(
                            &layout,
                            Point::new(
                                event.state.position.x - placement.rect.x0,
                                event.state.position.y - placement.rect.y0,
                            ),
                        ),
                    );
                    true
                }
        });
    })
}

/// The spelling and face a label draws with — one truth for the view
/// and for hit-testing a click against what was actually drawn.
fn label_spelling<'a>(cx: &'a Cx, key: &CellId) -> (String, &'a TextStyle) {
    match cx.name(*key) {
        Some(name) => (name.to_string(), &cx.styles.label),
        None => (short_id(*key), &cx.styles.id),
    }
}

/// A cell projection's ground, painted only at authority
/// TRANSITIONS: an external cell under document authority takes the
/// dark tint — no lock, just "from elsewhere" — and a
/// document-authority cell under an external one takes its light
/// ground back (opaque, since an alpha wash can't be undone by
/// another wash). Runs of the same authority draw nothing, so
/// nesting never stacks tints. The enclosing authority is the owning
/// cell at the path's last Follow, so a cell inside a list carries
/// its list's owner as context. Wraps outside the descend so the
/// cell's own selection highlight draws over its ground.
fn ground<C: 'static, Cv: Canvas + 'static>(cx: &Cx, path: &[Step], value: &Value, content: Measured<Placed<C, Cv>>) -> Measured<Placed<C, Cv>> {
    let Some(cell) = value.as_cell() else {
        return content;
    };
    let external = cx.sources.external(cell);
    let parent_external = last_follow(path)
        .and_then(|index| cx.sources.resolve(&path[..index]))
        .and_then(Value::as_cell)
        .is_some_and(|cell| cx.sources.external(cell));
    if external == parent_external {
        return content;
    }
    let scale = cx.styles.scale;
    let color = if external {
        Color::new([0.13, 0.14, 0.16, 0.05])
    } else {
        Color::new([0.965, 0.965, 0.972, 1.0])
    };
    decorate(content, move |p, rect| {
        let bg = RoundedRect::from_rect(rect.inset(3.0 * scale), 5.0 * scale);
        p.fill(bg, color, Affine::IDENTITY);
    })
}

/// The secondary selection's mark: a subtle wash over another whole
/// projection of the selected value — an expanded block, a collapsed
/// handle, or a label. The primary selection's geometry at lower
/// strength, so the two read as one family.
fn secondary_mark<C: 'static, Cv: Canvas + 'static>(cx: &Cx, value: &Value, content: Measured<Placed<C, Cv>>) -> Measured<Placed<C, Cv>> {
    let strong = cx.secondary.as_ref() == Some(value);
    let scale = cx.styles.scale;
    let value = value.clone();
    decorate(content, move |p, rect| {
        p.ink(move |cv, ink| {
            // The hover variant is the same mark at half voice.
            let faint = !strong && ink.hovered_value == Some(&value);
            if !strong && !faint {
                return;
            }
            let bg = RoundedRect::from_rect(rect.inset(3.0 * scale), 5.0 * scale);
            let (fill, line) = if strong { (0.10, 0.55) } else { (0.05, 0.25) };
            cv.fill(bg, Color::new([0.0, 0.48, 1.0, fill]), Affine::IDENTITY);
            cv.stroke(
                bg,
                Stroke::new(1.5 * scale),
                Color::new([0.0, 0.48, 1.0, line]),
                Affine::IDENTITY,
            );
        });
    })
}

/// Starts the ordinary projection at a value with no document source.
/// Interaction attributes the transient tree to `owner`, while its
/// children remain read-only and have no document paths of their own.
fn project_transient_root<
    C: 'static,
    Cv: Canvas + 'static,
>(
    cx: &Cx,
    projection: Option<&Projection<C>>,
    tcx: &mut TextCtx,
    path: &[Step],
    result: Value,
    fuel: usize,
    avail: f64,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    let origin = path.to_vec();
    let select = hooks.select.clone();
    let select_origin = origin.clone();
    let result_hooks = Hooks {
        select: Rc::new(move |ctx, _, _| select(ctx, select_origin.clone(), None)),
        toggle: Rc::new(|_, _| {}),
        rename: Rc::new(|_, _, _| {}),
        edit: Rc::new(|_| None),
        pick: hooks.pick.clone(),
        insert: Rc::new(|_, _| {}),
        delete: Rc::new(|_| false),
        apply: hooks.apply.clone(),
    };
    let result_cx = Cx {
        sources: cx.sources,
        raw: false,
        foreign: cx.foreign,
        annotations: cx.annotations,
        styles: cx.styles,
        selection: None,
        secondary: None,
        source: Source::Transient { owner: path },
        fuel: std::cell::Cell::new(fuel),
    };
    let projected = project_location(
        &result_cx,
        projection,
        tcx,
        path,
        &HashSet::new(),
        Location::Root(Some(&result)),
        avail,
        &result_hooks,
    );
    // The transient result is not another projection of the stored
    // source value. Its inner views may install ordinary hover claims while
    // rendering, so cover them across this whole arm.
    placed::after(projected, |p, placement| hover_block(p, placement))
}

/// Adds one GID step to the active source and invokes the supplied
/// projection. The projection, not the caller, resolves the child;
/// a missing child therefore reaches the same total fallback as an
/// empty root.
#[allow(clippy::too_many_arguments)]
fn descend<
    C: 'static,
    Cv: Canvas + 'static,
>(
    cx: &Cx,
    projection: Option<&Projection<C>>,
    tcx: &mut TextCtx,
    parent_path: &[Step],
    ancestors: &HashSet<CellId>,
    parent: &Value,
    step: Step,
    avail: f64,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    let mut path = parent_path.to_vec();
    path.push(step.clone());
    if step == Step::Follow {
        let mut ancestors = ancestors.clone();
        ancestors.extend(parent.as_cell());
        project_location(
            cx,
            projection,
            tcx,
            &path,
            &ancestors,
            Location::Child { parent, step },
            avail,
            hooks,
        )
    } else {
        project_location(
            cx,
            projection,
            tcx,
            &path,
            ancestors,
            Location::Child { parent, step },
            avail,
            hooks,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn project_location<
    C: 'static,
    Cv: Canvas + 'static,
>(
    cx: &Cx,
    projection: Option<&Projection<C>>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    location: Location<'_>,
    avail: f64,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    match location.value(|cell| cx.sources.value(cell)) {
        Some(value) => {
            project_present_value(cx, projection, tcx, path, ancestors, value, avail, hooks)
        }
        None => pending_view(cx, tcx, path.to_vec(), hooks),
    }
}

#[allow(clippy::too_many_arguments)]
fn project_present_value<
    C: 'static,
    Cv: Canvas + 'static,
>(
    cx: &Cx,
    projection: Option<&Projection<C>>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    value: &Value,
    avail: f64,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    let projected = projection
        .and_then(|projection| {
            // Editor state arrives positionally: the payload only at
            // the selected path, the annotations only at this one.
            // The document's own partials answer first.
            let selection = cx
                .selection
                .filter(|current| current.path() == path)
                .map(Selection::payload);
            let state = cx.annotations.at(path);
            let select = select_handler(path.to_vec(), hooks);
            let hover = Hover::Value(path.to_vec());
            document_partial_layout(cx, value, selection, state, &select, &hover).or_else(|| {
                projection.apply(&ProjectEnv { cx }, value, selection, state, select, hover)
            })
        })
        .map(|layout| {
            realize(
                cx, projection, tcx, path, ancestors, hooks, value, layout, avail,
            )
        });
    let inner = match projected {
        Some(projected) => projected,
        None => {
            let layout = structure::of(cx, path, ancestors, value, hooks);
            realize(
                cx, projection, tcx, path, ancestors, hooks, value, layout, avail,
            )
        }
    };
    // Other projections of the selected value carry the secondary
    // mark; the selected one has the primary highlight.
    let inner = if cx.selected(path) {
        inner
    } else {
        secondary_mark(cx, value, inner)
    };
    // A landmark, not a target: highlight and keyboard reach span
    // the full bounds, while clicks belong to the content each arm
    // claimed above — structural whitespace deselects.
    let placed = descend_landmark(cx, path.to_vec(), hooks, inner);
    let grounded = ground(cx, path, value, placed);
    pick_target(path.to_vec(), value.clone(), hooks, grounded)
}

/// The document's own partials, tried before the editor's: the
/// registry is a document fact — a list of Grap callables on the
/// [`PROJECTIONS`](layout_data::vocabulary::PROJECTIONS) cell — each
/// applied to the value and its positional editor state, the result
/// decoded from the display data form. Any diagnostic or undecodable
/// result declines, falling through whole.
fn document_partial_layout<C>(
    cx: &Cx,
    value: &Value,
    selection: Option<&Value>,
    state: Option<&Value>,
    select: &progred_display::ClickHandler<C>,
    hover: &Hover,
) -> Option<progred_display::Layout<C, Hover>> {
    let registry = cx
        .sources
        .doc
        .cells
        .value(layout_data::vocabulary::PROJECTIONS)?
        .as_list()?;
    registry.values().find_map(|partial| {
        let fuel = if cx.source.transient() {
            cx.fuel.get()
        } else {
            grap::DEFAULT_FUEL
        };
        let evaluation = grap::apply(
            partial,
            [
                (layout_data::vocabulary::VALUE, value.clone()),
                (
                    layout_data::vocabulary::SELECTION,
                    selection.cloned().unwrap_or_else(absent::value),
                ),
                (
                    layout_data::vocabulary::STATE,
                    state.cloned().unwrap_or_else(absent::value),
                ),
            ],
            |cell| cx.sources.value(cell).cloned(),
            cx.foreign,
            fuel,
        );
        cx.fuel.set(evaluation.remaining_fuel);
        evaluation.diagnostics.is_empty().then_some(())?;
        layout_data::decode(&evaluation.result, select, hover)
    })
}

/// Every projected value's command-click backstop: pick the value
/// into an open pending, or — nothing pending — select it like a
/// plain click, so a stray modifier never deadens the gesture.
/// Content declines command-clicks, so inner [`realize_pick`] wrappers
/// answer first and this catches what they refused.
fn pick_target<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    value: Value,
    hooks: &Hooks<C>,
    child: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    let pick = hooks.pick.clone();
    let select = hooks.select.clone();
    before(child, move |p, placement| {
        let pick = pick.clone();
        let select = select.clone();
        let path = path.clone();
        let value = value.clone();
        p.handler().on_pointer_down(move |world, event| {
            event.button == Some(PointerButton::Primary)
                && command(&event.state.modifiers)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && {
                    if !pick(world, value.clone()) {
                        select(world, path.clone(), None);
                    }
                    true
                }
        });
    })
}

/// An EMPTY SLOT at `path`: the [`placeholder`] widget wired to this
/// projection — engagement derived from the selection, wrapped as an
/// ordinary descend so it highlights, clicks, and navigates like the
/// value it may become. Engaged, its placement emits the completion
/// popup for the shell to draw over the body.
fn pending_view<
    C: 'static,
    Cv: Canvas + 'static,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: Path,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    let engaged = cx
        .selection
        .filter(|current| {
            current.stage() == Stage::Pending && current.path() == path.as_slice()
        })
        .and_then(Selection::edit);
    let content = placeholder(cx, tcx, engaged, false, hooks);
    // Engaged, the generic ring IS the slot's chrome: it draws
    // [`highlight_rect`] over the same frame the cold box strokes,
    // and the same ring survives the commit around the same glyphs —
    // the box never changes, only its paint.
    source_target(cx, path, None, hooks, content)
}

/// The slot widget, in the Puri idiom: its one state input is the
/// engaged pending's query, and None IS the inactive
/// pending — the cold [`placeholder_box`], whose width the engaged
/// query's frame holds as its minimum, so the two forms are one
/// widget in two states and the transition between them is pure
/// chrome. The caller owns identity (descend, highlight, clicks);
/// `labels` picks the slot's role.
fn placeholder<
    C: 'static,
    Cv: Canvas + 'static,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    engaged: Option<&LineEditState>,
    labels: bool,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    match engaged {
        Some(query) => query_content(cx, tcx, query, labels, hooks),
        None => placeholder_box(tcx, cx.styles),
    }
}

/// A focused completion query: the editor plus its popup, emitted at
/// placement for the shell to draw over the body. Serves both pending
/// stages — a value and a new field's label (`labels` narrows the
/// offers there).
fn query_content<
    C: 'static,
    Cv: Canvas + 'static,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    query: &LineEditState,
    labels: bool,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    // The same inputs the shell's card view reads: the drawn rows
    // and this stash's keyboard commit must answer from one list.
    let entries = completion_entries(&cx.sources, cx.raw, labels, query.text());
    let fallback = text(tcx, "…", &cx.styles.dim);
    let presentation = edit_presentation(&cx.styles.label);
    let content = atom_content(
        Some(query),
        fallback,
        presentation.clone(),
        None,
        tcx,
        cx.styles,
        hooks,
    );
    // The FRAME holds the slot's width as a minimum — the text field
    // stays content-sized (a blank query is a bare caret), and the
    // frame around it is what never shrinks to a sliver. Framed
    // before the decorate so the popup anchor and the caret clicks
    // span it; the air around it is the caller's [`slot_insets`].
    let content = min_width(slot_width(cx.styles), content);
    let edit = hooks.edit.clone();
    let scale = cx.styles.scale;
    before(content, move |p, placement| {
        let rect = placement.rect;
        *p.popup() = Some(Popup {
            anchor: rect,
            entries,
        });
        // Clicks in the query place the caret, straight through the
        // edit hook — the selection transition is never involved, so
        // clicking what you are typing can't discard it.
        hover_block(p, placement);
        let edit = edit.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && edit(ctx).is_some_and(|edit| {
                    edit.state.pointer_down(
                        &presentation,
                        edit.fonts,
                        edit.layouts,
                        scale as f32,
                        LineEditPointerDown {
                            point: Point::new(
                                event.state.position.x - rect.x0,
                                event.state.position.y - rect.y0,
                            ),
                            shift: event.state.modifiers.shift(),
                            count: event.state.count.max(1),
                        },
                    );
                    true
                })
        });
    })
}

/// The drawn completion card: entry rows under the pending anchor,
/// the chosen one highlighted, styled by what each entry commits.
/// The shell places it after the body, so it overlays and its
/// handlers win: clicking a row commits it, and the card swallows
/// every other click so nothing lands on content underneath.
pub fn popup_view<C: 'static, Cv: Canvas + 'static>(
    tcx: &mut TextCtx,
    styles: &Styles,
    entries: &[Entry],
    choice: usize,
    commit: impl Fn(&mut C, &EntryAction) + Clone + 'static,
) -> Measured<Placed<C, Cv>> {
    let scale = styles.scale;
    let choice = choice.min(entries.len().saturating_sub(1));
    // Cells first, so rows can pad out to the widest and the chosen
    // highlight spans the card, not just its own content.
    let cells: Vec<(Measured<Placed<C, Cv>>, Option<Measured<Placed<C, Cv>>>)> = entries
        .iter()
        .map(|entry| {
            let style = match &entry.action {
                EntryAction::Value(value) if text::read(value).is_some() => &styles.string,
                EntryAction::Value(value) if value.as_blob().is_some() => &styles.id,
                EntryAction::Value(_) if entry.id => &styles.id,
                EntryAction::Value(_) => &styles.label,
                EntryAction::NewLabel(_)
                | EntryAction::NewCell
                | EntryAction::NewList
                | EntryAction::NewRecord => &styles.dim,
            };
            let display = highlighted(tcx, &entry.display, &entry.matches, style);
            let detail = entry
                .detail
                .as_ref()
                .map(|detail| text(tcx, detail, &styles.id));
            (display, detail)
        })
        .collect();
    let widths: Vec<f64> = cells
        .iter()
        .map(|(display, detail)| {
            display.extent.width
                + detail
                    .as_ref()
                    .map_or(0.0, |detail| 8.0 * scale + detail.extent.width)
        })
        .collect();
    let max_width = widths.iter().copied().fold(0.0, f64::max);
    let rows: Vec<Measured<Placed<C, Cv>>> = cells
        .into_iter()
        .zip(widths)
        .enumerate()
        .map(|(index, ((display, detail), width))| {
            let mut cells: Vec<Measured<Placed<C, Cv>>> = vec![display];
            if let Some(detail) = detail {
                cells.push(detail);
            }
            let content = pad(
                Insets::new(
                    8.0 * scale,
                    2.0 * scale,
                    8.0 * scale + (max_width - width),
                    2.0 * scale,
                ),
                row(8.0 * scale, cells),
            );
            let chosen = index == choice;
            let action = entries[index].action.clone();
            let commit = commit.clone();
            before(content, move |p, placement| {
                let rect = placement.rect;
                p.ink(move |cv, ink| {
                    let lit = !chosen
                        && matches!(tree_hovered(ink), Some(Hover::Entry(i)) if *i == index);
                    if chosen {
                        cv.fill(
                            RoundedRect::from_rect(rect, 4.0 * scale),
                            Color::new([0.0, 0.48, 1.0, 0.14]),
                            Affine::IDENTITY,
                        );
                    } else if lit {
                        cv.fill(
                            RoundedRect::from_rect(rect, 4.0 * scale),
                            Color::new([0.0, 0.48, 1.0, 0.08]),
                            Affine::IDENTITY,
                        );
                    }
                });
                hover_claim(p, placement, Hover::Entry(index));
                p.handler().on_pointer_down(move |ctx, event| {
                    event.button == Some(PointerButton::Primary)
                        && placement
                            .contains(Point::new(event.state.position.x, event.state.position.y))
                        && {
                            commit(ctx, &action);
                            true
                        }
                });
            })
        })
        .collect();
    let card = pad(Insets::uniform(4.0 * scale), col(0, 2.0 * scale, rows));
    before(card, move |p, placement| {
        let rect = placement.rect;
        let shape = RoundedRect::from_rect(rect, 6.0 * scale);
        p.fill(shape, Color::new([1.0, 1.0, 1.0, 1.0]), Affine::IDENTITY);
        p.stroke(
            shape,
            Stroke::new(1.0 * scale),
            Color::new([0.75, 0.77, 0.81, 1.0]),
            Affine::IDENTITY,
        );
        hover_block(p, placement);
        p.handler().on_pointer_down(move |_, event| {
            placement.contains(Point::new(event.state.position.x, event.state.position.y))
        });
    })
}

/// Entry text with the query's matched spans in bold — the fuzzy
/// filter's byte offsets drawn, not recomputed.
fn highlighted<C: 'static, Cv: Canvas + 'static>(
    tcx: &mut TextCtx,
    s: &str,
    matches: &[filter::Match],
    style: &TextStyle,
) -> Measured<Placed<C, Cv>> {
    if matches.is_empty() {
        return text(tcx, s, style);
    }
    let bold = TextStyle {
        weight: Some(700.0),
        ..style.clone()
    };
    let mut segments: Vec<Measured<Placed<C, Cv>>> = Vec::new();
    let mut at = 0;
    for span in matches {
        if span.start > at {
            segments.push(text(tcx, &s[at..span.start], style));
        }
        segments.push(text(tcx, &s[span.start..span.start + span.len], &bold));
        at = span.start + span.len;
    }
    if at < s.len() {
        segments.push(text(tcx, &s[at..], style));
    }
    row(0.0, segments)
}

/// An editable atom's content: the selection's focused editor when
/// this atom is being edited — with `placeholder` as its ghost while
/// empty — its static text otherwise.
fn atom_content<C: 'static, Cv: Canvas + 'static>(
    editing: Option<&LineEditState>,
    fallback: Measured<Placed<C, Cv>>,
    presentation: LineEditPresentation,
    placeholder: Option<(&str, &TextStyle)>,
    tcx: &mut TextCtx,
    styles: &Styles,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    match editing {
        Some(line) => {
            let edit_ctx = hooks.edit.clone();
            render::text_edit(
                LineEditDescription {
                    state: line,
                    focused: true,
                    presentation,
                    style: &styles.edit,
                    placeholder,
                },
                tcx,
                move |c| edit_ctx(c),
            )
        }
        None => fallback,
    }
}

/// The label stage engaged — a rename's re-opened label or a new
/// field's — its query wearing the primary ring explicitly: a
/// pending edge has no path of its own for [`descend`] to mark, and
/// the ring spans the QUERY frame alone, the way a value pending's
/// does. Clicks inside belong to the query's own caret target;
/// clicks beside fall through like any pending's.
fn label_query<
    C: 'static,
    Cv: Canvas + 'static,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    query: &LineEditState,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    let scale = cx.styles.scale;
    let content = placeholder(cx, tcx, Some(query), true, hooks);
    let ringed = decorate(content, move |p, rect| {
        primary_highlight(scale, p, rect);
    });
    // The ring's outset rides inside the node, so glued neighbors —
    // the colon, a flat comma — clear its ink.
    pad(Insets::new(4.0 * scale, 0.0, 4.0 * scale, 0.0), ringed)
}

/// A plain click-to-select target for `path` — for parts like labels
/// and the cell star that select without carrying an editor click.
/// With the command modifier and a pending open, picks `value` — the
/// identity the part displays — into it instead.
fn select_target<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    value: Value,
    hooks: &Hooks<C>,
    content: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    let claimed = hover_target(path.clone(), content);
    quiet_select_target(path, value, hooks, claimed)
}

/// Name the value at `path` for the pointer over this ink, adding no
/// click of its own — the hover half of [`select_target`], and the
/// flat literal's delimiter dress.
fn hover_target<C: 'static, Cv: Canvas + 'static>(path: Path, content: Measured<Placed<C, Cv>>) -> Measured<Placed<C, Cv>> {
    before(content, move |p, placement| {
        hover_claim(p, placement, Hover::Value(path.clone()));
    })
}

/// [`select_target`] minus the pointer claim — for a container's
/// one-line literal, whose interior air belongs to the landmark's
/// hold and whose delimiter ink names the container through
/// [`hover_target`].
fn quiet_select_target<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    value: Value,
    hooks: &Hooks<C>,
    content: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    let select = hooks.select.clone();
    let pick = hooks.pick.clone();
    before(content, move |p, placement| {
        let select = select.clone();
        let pick = pick.clone();
        let target = path.clone();
        let value = value.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && {
                    let picked = command(&event.state.modifiers) && pick(ctx, value.clone());
                    if !picked {
                        select(ctx, target.clone(), None);
                    }
                    true
                }
        });
    })
}

/// A click on projected text reports what happened — this path, this
/// text-local position — and nothing more; the shell's selection
/// transition decides what it means. One report serves the first
/// click and every one after. With the command modifier and a pending
/// open, picks the atom's value into it instead.
fn cursor_target<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    value: Value,
    presentation: LineEditPresentation,
    hooks: &Hooks<C>,
    line: Option<render::LineEdit>,
    content: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    let select = hooks.select.clone();
    let pick = hooks.pick.clone();
    before(content, move |p, placement| {
        let rect = placement.rect;
        hover_claim(p, placement, Hover::Value(path.clone()));
        let pick = pick.clone();
        let value = value.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && {
                    if command(&event.state.modifiers) && pick(ctx, value.clone()) {
                        return true;
                    }
                    let click = TextClick {
                        point: Point::new(
                            event.state.position.x - rect.x0,
                            event.state.position.y - rect.y0,
                        ),
                        shift: event.state.modifiers.shift(),
                        count: event.state.count.max(1),
                        presentation: presentation.clone(),
                        line: line.clone(),
                    };
                    select(ctx, path.clone(), Some(click));
                    true
                }
        });
    })
}

mod structure;

#[cfg(test)]
mod svg_bench;
#[cfg(test)]
mod tests;
