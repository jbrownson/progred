//! The raw projection: any document rendered with no schema, in the
//! delimiter family — `(` cell `)`, `[` list `]`, `{` record `}`.
//! A cell heads with its conventional simple name (the ordinary name
//! field projected in place) or its short id; records are field rows,
//! lists inline literals or bare element rows; atoms render as their
//! values; positions are session bookkeeping and never render at all.

use crate::styles::Styles;
use crate::completion::{
    EntryAction, HasPopup, Popup, completion_entries,
};
#[cfg(test)]
use crate::completion::{Entry, resolve_entry, resolve_label};
use crate::document::{Path, short_id};
#[cfg(test)]
use crate::document::Document;
#[cfg(test)]
use crate::document::{sample_document, sample_vocabulary};
use crate::filter;
use crate::selection::{Collapse, Selection, last_follow};
#[cfg(test)]
use crate::selection::{
    break_edit_run, delete_edge, from_clipboard, from_structure, line_edit, pending_edge,
    pending_follow, pending_insert, pending_into, pending_rename, pending_value, rename_field,
    resolve_query, set_collapse, set_value, to_clipboard, toggle_collapse, write_through,
};
use crate::hover::{HasHover, Hover, HoverClaim, Hovering, hover_value};
#[cfg(test)]
use crate::hover::resolve_hover;
use crate::navigate::{Descend, HasDescends};
#[cfg(test)]
use crate::navigate::{projected_name_owner, step_selection};
use crate::display::{self, text};
use crate::layout::{
    Extent, Measured, around, before, col, decorate, leaf, min_width, on_key, on_primary_pointer_down,
    pad,
    row,
};
use crate::sources::Sources;
use crate::projection::Location;
use parley::layout::Layout as TextLayout;
use progred_graph::{CellId, Step, Value};
#[cfg(test)]
use progred_graph::{Cells, new_cell_id};
use puri::delim::{self, Delim, DelimStyle};
use puri::draw::Canvas;
use puri::edit::{
    EditCtx, LineEditDescription, LineEditPointerDown, LineEditPresentation,
    LineEditState,
};
use puri::geometry::Placement;
use puri::handler::HasHandler;
use puri::text::{TextCtx, TextStyle, caret_index, line_layout};
use std::collections::HashSet;
use std::rc::Rc;
use ui_events::keyboard::{Key, NamedKey};
#[cfg(test)]
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::PointerButton;
use vello::kurbo::{Affine, Insets, Point, Rect, RoundedRect, Stroke};
use vello::peniko::{Brush, Color};

/// Read-only projection context threaded through every view.
struct Cx<'a> {
    /// The reading context: the document read over its library.
    sources: Sources<'a>,
    /// Names and field order derive from this view bit. Value
    /// projections come from the editor's stack; `grap` is one of them.
    raw: bool,
    values: bool,
    foreign: &'a grap::ForeignFunctions,
    collapse: &'a Collapse,
    styles: &'a Styles,
    selection: Option<&'a Selection>,
    /// The pointer's current claim, previewed by the target it names.
    hover: Option<&'a Hover>,
    /// The value whose other projections carry the secondary mark.
    secondary: Option<Value>,
    /// The value the hover refers to; its projections carry the faint
    /// hover variant of the secondary mark.
    secondary_hover: Option<Value>,
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
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    hooks: &Hooks<C>,
    value: &Value,
    layout: progred_display::Layout,
    avail: f64,
) -> Measured<P> {
    let scale = cx.styles.scale;
    match layout {
        progred_display::Layout::Leaf(content) => {
            leaf_display(cx, tcx, path, hooks, content)
        }
        progred_display::Layout::OnClick { child, click } => {
            let inner = realize(cx, tcx, path, ancestors, hooks, value, *child, avail);
            realize_click(cx, tcx, path, hooks, value, click, inner)
        }
        progred_display::Layout::OnKey { child, key } => {
            let inner = realize(cx, tcx, path, ancestors, hooks, value, *child, avail);
            match key {
                progred_display::Key::Delete => bind_delete(cx, hooks, inner),
            }
        }
        progred_display::Layout::Row { gap, children } => {
            let children = children
                .into_iter()
                .map(|child| realize(cx, tcx, path, ancestors, hooks, value, child, avail))
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
                .map(|child| realize(cx, tcx, path, ancestors, hooks, value, child, avail))
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
        progred_display::Layout::Bracket { delim, child } => {
            let reserved =
                2.0 * (delim_advance(cx.styles, display_delim(delim)) + 2.0 * scale);
            let inner = realize(
                cx,
                tcx,
                path,
                ancestors,
                hooks,
                value,
                *child,
                (avail - reserved).max(0.0),
            );
            bracketed(cx, display_delim(delim), path, value, hooks, inner)
        }
        progred_display::Layout::Descend { step } => descend(
            cx,
            tcx,
            path,
            ancestors,
            value,
            step,
            avail,
            hooks,
        ),
        progred_display::Layout::At { steps, value: nested } => {
            realize_at(cx, tcx, path, ancestors, steps, nested, avail, hooks)
        }
        progred_display::Layout::Transient { value: computed, fuel } => {
            project_transient_root(cx, tcx, path, computed, fuel, avail, hooks)
        }
        progred_display::Layout::Group { flat, broken } => {
            if avail <= 0.0 {
                return realize(cx, tcx, path, ancestors, hooks, value, *broken, avail);
            }
            let candidate = realize(
                cx,
                tcx,
                path,
                ancestors,
                hooks,
                value,
                *flat,
                f64::INFINITY,
            );
            let fits = one_line(candidate.extent, scale) && candidate.extent.width <= avail;
            if fits {
                return candidate;
            }
            let broken = realize(cx, tcx, path, ancestors, hooks, value, *broken, avail);
            if one_line(candidate.extent, scale) && candidate.extent.width <= broken.extent.width {
                candidate
            } else {
                broken
            }
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
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    steps: Vec<Step>,
    nested: Value,
    avail: f64,
    hooks: &Hooks<C>,
) -> Measured<P> {
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
        tcx,
        &path,
        &follow_ancestors,
        &nested,
        avail,
        hooks,
    )
}

fn realize_click<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    hooks: &Hooks<C>,
    value: &Value,
    click: progred_display::Click,
    inner: Measured<P>,
) -> Measured<P> {
    match click {
        progred_display::Click::Select => {
            select_target(path.to_vec(), value.clone(), hooks, inner)
        }
        progred_display::Click::Quiet => {
            quiet_select_target(path.to_vec(), value.clone(), hooks, inner)
        }
        progred_display::Click::Line(line) => cursor_target(
            path.to_vec(),
            value.clone(),
            cx.styles.line_presentation(&line.prefix, &line.suffix),
            hooks,
            Some(line),
            inner,
        ),
        progred_display::Click::Toggle => toggle_target(cx, path.to_vec(), hooks, inner),
        progred_display::Click::Insert { after } => {
            let mut target = path.to_vec();
            target.push(Step::Element(after));
            insert_target(cx, target, hooks, inner)
        }
        progred_display::Click::Rename { key } => {
            let mut child = path.to_vec();
            child.push(Step::Key(key));
            let (spelling, style) = label_spelling(cx, &key);
            let layout = line_layout(tcx, &spelling, style);
            rename_target(cx, child, layout, hooks, inner)
        }
        progred_display::Click::Pick { key } => pick_target(key, hooks, inner),
        progred_display::Click::Field { key } => {
            let mut child = path.to_vec();
            child.push(Step::Key(key));
            select_target(child, Value::from(key), hooks, inner)
        }
        progred_display::Click::Absorb => {
            before(inner, move |p: &mut P, placement| {
                hover_block(p, placement);
                p.handler().on_pointer_down(move |_, event| {
                    event.button == Some(PointerButton::Primary)
                        && placement.contains(Point::new(
                            event.state.position.x,
                            event.state.position.y,
                        ))
                });
            })
        }
    }
}

fn leaf_display<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    hooks: &Hooks<C>,
    content: progred_display::Display,
) -> Measured<P> {
    match content {
        progred_display::Display::Text { text, face } => {
            let style = match face {
                progred_display::Face::Name => &cx.styles.name,
                progred_display::Face::Dim => &cx.styles.dim,
                progred_display::Face::Label => &cx.styles.label,
                progred_display::Face::Id => &cx.styles.id,
            };
            display::text(tcx, &text, style)
        }
        progred_display::Display::Delim { delim, open } => hover_target(
            path.to_vec(),
            flat_delim(cx.styles, display_delim(delim), open),
        ),
        progred_display::Display::Head { cell } => {
            head_view(cx, tcx, path, cell, cx.name(cell), hooks)
        }
        progred_display::Display::Query { labels } => {
            let engaged = if labels {
                cx.pending_rename_under(path)
                    .map(|(_, query, choice)| (query, choice))
                    .or_else(|| cx.pending_edge_under(path))
            } else {
                match cx.selection {
                    Some(Selection::Pending {
                        path: pending,
                        query,
                        choice,
                    }) if pending.as_slice() == path => Some((query, *choice)),
                    _ => None,
                }
            };
            match engaged {
                Some((query, choice)) if labels => label_query(cx, tcx, query, choice, hooks),
                Some((query, choice)) => placeholder(cx, tcx, Some((query, choice)), false, hooks),
                None => display::text(tcx, "…", &cx.styles.dim),
            }
        }
        progred_display::Display::Slot => placeholder(cx, tcx, None, false, hooks),
        progred_display::Display::LineEdit(line) => {
            let editing = cx
                .selection
                .filter(|selection| selection.path() == path)
                .and_then(Selection::edit);
            let edit = hooks.edit.clone();
            display::line_edit(tcx, cx.styles, &line, editing, move |c| edit(c))
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
    pub line: Option<display::LineEdit>,
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
}

/// The platform command modifier, for pointer gestures.
pub(crate) fn command(modifiers: &ui_events::keyboard::Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        modifiers.meta()
    } else {
        modifiers.ctrl()
    }
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
        match self.selection {
            Some(Selection::Edge { path: selected, .. })
            | Some(Selection::Pending { path: selected, .. }) => selected.as_slice() == path,
            _ => false,
        }
    }

    fn hovered_value(&self, path: &[Step]) -> bool {
        matches!(self.hover, Some(Hover::Value(hovered)) if hovered.as_slice() == path)
    }

    /// The pending child step under `path`, when the selection is
    /// authoring one there.
    fn pending_child_of(&self, path: &[Step]) -> Option<Step> {
        match self.selection {
            Some(Selection::Pending { path: pending, .. })
                if pending
                    .split_last()
                    .is_some_and(|(_, parent)| parent == path) =>
            {
                pending.last().cloned()
            }
            _ => None,
        }
    }

    /// The label query of a new field being authored on the record at
    /// `path`.
    fn pending_edge_under(&self, path: &[Step]) -> Option<(&LineEditState, usize)> {
        match self.selection {
            Some(Selection::PendingEdge {
                parent,
                query,
                choice,
                replacing: None,
            }) if parent.as_slice() == path => Some((query, *choice)),
            _ => None,
        }
    }

    /// The re-opened label of an existing field on the record at
    /// `path`, with the key it replaces.
    fn pending_rename_under(&self, path: &[Step]) -> Option<(&CellId, &LineEditState, usize)> {
        match self.selection {
            Some(Selection::PendingEdge {
                parent,
                query,
                choice,
                replacing: Some(replacing),
            }) if parent.as_slice() == path => Some((replacing, query, *choice)),
            _ => None,
        }
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

/// Whether a candidate stayed one line tall — the flat forms' second
/// gate beside width: a literal whose child broke inside is not flat,
/// however narrow it came out.
fn one_line(extent: Extent, scale: f64) -> bool {
    extent.height() <= 20.0 * scale
}

/// A drawn delimiter leaf: `extent` is what layout sees (the FLAT
/// advance, the span it must cover) while the ink inside spans
/// `ink_top..ink_bottom` relative to the baseline, stroked in the dim
/// brush like the text delimiters it replaces. A grown tall
/// delimiter keeps its terminals where the flat form's would be and
/// bulges OUTWARD past its advance — typographic overhang, so growth
/// costs layout nothing and nested delimiters bow into each other's
/// empty sides.
fn delim_leaf<P: Canvas>(
    styles: &Styles,
    delim: Delim,
    open: bool,
    extent: Extent,
    ink_top: f64,
    ink_bottom: f64,
) -> Measured<P> {
    let style = delim_style(styles);
    let bearing = SIDE_BEARING_EM * 14.0 * styles.scale;
    let brush = styles.dim.brush.clone();
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
        move |p: &mut P, placement| {
            let at = Point::new(placement.rect.x0, placement.rect.y0 + extent.ascent);
            p.fill(
                path.clone(),
                brush.clone(),
                Affine::translate((at.x + ink_x, at.y)),
            );
        },
    )
}

/// A one-line delimiter at the font's own glyph span: the drawn
/// family's flat form, sitting in a text row exactly where the glyph
/// would.
fn flat_delim<P: Canvas>(styles: &Styles, delim: Delim, open: bool) -> Measured<P> {
    let em = 14.0 * styles.scale;
    let (asc, desc) = (GLYPH_ASC_EM * em, GLYPH_DESC_EM * em);
    delim_leaf(
        styles,
        delim,
        open,
        Extent {
            width: 0.0,
            ascent: asc,
            descent: desc,
        },
        -asc,
        desc,
    )
}

/// A delimiter stretched over `content`'s extent, ink trimmed to meet
/// the glyph span on the first and last lines.
fn tall_delim<P: Canvas>(styles: &Styles, delim: Delim, open: bool, content: Extent) -> Measured<P> {
    let em = 14.0 * styles.scale;
    let ink_top = -(content.ascent - TOP_TRIM_EM * em).max(GLYPH_ASC_EM * em);
    let ink_bottom = (content.descent - BOTTOM_TRIM_EM * em).max(GLYPH_DESC_EM * em);
    delim_leaf(styles, delim, open, content, ink_top, ink_bottom)
}

/// Wraps `content` in the stretched delimiter pair claiming
/// `path`/`target`: the delimiters are the container's handles —
/// their ink selects it (command-picks it) — and they grow with the
/// content, so a tall value gets tall delimiters instead of a
/// floating closer.
fn bracketed<C: 'static, P: Canvas + HasHandler<C> + HasHover<HoverClaim>>(
    cx: &Cx,
    delim: Delim,
    path: &[Step],
    target: &Value,
    hooks: &Hooks<C>,
    content: Measured<P>,
) -> Measured<P> {
    let extent = content.extent;
    // The air between delimiter and content rides INSIDE the
    // delimiter's claim — identical pixels, and the handle's thin
    // hit zone gains the gap: hovering the air is hovering the
    // bracket.
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
                    tall_delim(cx.styles, delim, true, extent),
                ),
            ),
            content,
            select_target(
                path.to_vec(),
                target.clone(),
                hooks,
                pad(
                    Insets::new(gap, 0.0, 0.0, 0.0),
                    tall_delim(cx.styles, delim, false, extent),
                ),
            ),
        ],
    )
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
fn placeholder_box<P: Canvas>(tcx: &mut TextCtx, styles: &Styles) -> Measured<P> {
    let line = text::<P>(tcx, "", &styles.name).extent;
    let extent = Extent {
        width: slot_width(styles),
        ..line
    };
    let scale = styles.scale;
    let brush = styles.dim.brush.clone();
    leaf(extent, move |p: &mut P, placement| {
        let rect = placement.rect;
        p.stroke(
            highlight_rect(scale, rect),
            Stroke::new(scale),
            brush,
            Affine::IDENTITY,
        );
    })
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

/// Emit `claim` when this settled rect contains the frame's pointer
/// input. Placement order is precedence: descendants and overlays
/// report later and replace earlier hits in the pass's hover resolver.
fn hover_report<P: HasHover<HoverClaim>>(p: &mut P, placement: Placement, claim: HoverClaim) {
    if p.pointer().is_some_and(|point| placement.contains(point)) {
        p.claim_hover(match claim {
            HoverClaim::Direct(Some(mut hovering)) => {
                hovering.rect = placement.visible_rect();
                HoverClaim::Direct(Some(hovering))
            }
            claim => claim,
        });
    }
}

/// The pointer names `key` outright, with this ink as its footprint.
fn hover_claim<P: HasHover<HoverClaim>>(p: &mut P, placement: Placement, key: Hover) {
    hover_report(
        p,
        placement,
        HoverClaim::Direct(Some(Hovering {
            hover: key,
            rect: placement.rect,
        })),
    );
}

/// An occluder: takes the pointer and names nothing, so targets
/// beneath an overlay never light.
fn hover_block<P: HasHover<HoverClaim>>(p: &mut P, placement: Placement) {
    hover_report(p, placement, HoverClaim::Direct(None));
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
fn source_target<C: 'static, P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends>(
    cx: &Cx,
    path: Path,
    value: Option<Value>,
    hooks: &Hooks<C>,
    child: Measured<P>,
) -> Measured<P> {
    let (path, transient) = match cx.source {
        Source::Transient { owner } if owner != path.as_slice() => return child,
        Source::Transient { owner } => (owner.to_vec(), true),
        Source::Stored => (path, false),
    };
    let scale = cx.styles.scale;
    let selected = cx.selected(&path);
    let hovered = cx.hovered_value(&path);
    let select = hooks.select.clone();
    let pick = hooks.pick.clone();
    before(child, move |p, placement| {
        let rect = placement.rect;
        if selected {
            primary_highlight(scale, p, rect);
        } else if hovered {
            hover_highlight(scale, p, rect);
        }
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

/// The value marked as the secondary selection: the one at the
/// selected path. A value can project in many places — links, but
/// equally text values, blobs, and equal lists — and the marks make that
/// sameness visible. Inline records are structure, not identity: no
/// marks.
fn secondary_of(sources: &Sources, selection: Option<&Selection>) -> Option<Value> {
    match selection? {
        Selection::Edge { path, .. } => sources
            .resolve(path)
            .filter(|value| {
                !matches!(value, Value::Record(_)) || progred_text::read(value).is_some()
            })
            .cloned(),
        _ => None,
    }
}

/// The explicit-state boundary: everything a projection pass reads.
/// `width` is the space the projection may fill; containers choose
/// flat or broken forms greedily from the root down.
pub struct ProjectDescription<'a> {
    pub sources: Sources<'a>,
    pub selection: Option<&'a Selection>,
    pub graph_node: Option<&'a Value>,
    pub hover: Option<&'a Hover>,
    pub hover_node: Option<&'a Value>,
    pub collapse: &'a Collapse,
    pub raw: bool,
    pub styles: &'a Styles,
    pub width: f64,
    pub values: bool,
    pub foreign: &'a grap::ForeignFunctions,
}

pub fn project<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    description: ProjectDescription<'_>,
    tcx: &mut TextCtx,
    hooks: Hooks<C>,
) -> Measured<P> {
    let ProjectDescription {
        sources,
        selection,
        graph_node,
        hover,
        hover_node,
        collapse,
        raw,
        styles,
        width,
        values,
        foreign,
    } = description;
    let cx = Cx {
        sources,
        raw,
        values,
        foreign,
        collapse,
        styles,
        selection,
        hover,
        source: Source::Stored,
        fuel: std::cell::Cell::new(grap::DEFAULT_FUEL),
        // The graph view's selected cell is a secondary here too:
        // its projections are the same value — and the graph view's
        // HOVERED cell is a hover secondary the same way.
        secondary: secondary_of(&sources, selection).or_else(|| graph_node.cloned()),
        secondary_hover: hover
            .and_then(|hover| hover_value(&sources, raw, selection, hover))
            .or_else(|| hover_node.cloned()),
    };
    // An empty document is a selectable placeholder at the root path.
    project_location(
        &cx,
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
fn descend_landmark<C: 'static, P: Canvas + HasDescends + HasHandler<C>>(
    cx: &Cx,
    path: Path,
    hooks: &Hooks<C>,
    child: Measured<P>,
) -> Measured<P> {
    if cx.source.transient() {
        return child;
    }
    let selected = cx.selected(&path);
    let hovered = cx.hovered_value(&path);
    let scale = cx.styles.scale;
    let marked = decorate(child, move |p: &mut P, rect| {
        if selected {
            primary_highlight(scale, p, rect);
        } else if hovered {
            hover_highlight(scale, p, rect);
        }
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

fn bind_delete<C: 'static, P: HasHandler<C>>(
    cx: &Cx,
    hooks: &Hooks<C>,
    child: Measured<P>,
) -> Measured<P> {
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

/// A cell's head: an ordinary simple-name field projected as header
/// text, or the short id when no naming convention answers. A shown
/// name remains the same selectable and editable text field; the
/// header is a projection of data rather than another storage path.
///
/// The head text stands for the CELL until the cell is selected: a
/// cold click falls through to the block's own target and selects the
/// cell, and only then does the text engage as a target. The pass
/// decides from current state; single-shot dispatch means the second
/// click always sees the engaged successor. Cold, the head stays
/// keyboard-reachable (and markable, when named), just not a pointer
/// target.
fn head_view<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    cell: CellId,
    name: Option<&str>,
    hooks: &Hooks<C>,
) -> Measured<P> {
    let short = short_id(cell);
    let Some(name) = name else {
        return text(tcx, &short, &cx.styles.id);
    };
    let mut edge = path.to_vec();
    edge.push(Step::Follow);
    edge.push(Step::Key(progred_name::vocabulary::NAME));
    let editing = cx
        .selection
        .filter(|selection| selection.path() == edge.as_slice())
        .and_then(Selection::edit);
    let fallback = text(tcx, name, &cx.styles.name);
    let presentation = edit_presentation(&cx.styles.name);
    let content = atom_content(
        editing,
        fallback,
        presentation.clone(),
        None,
        tcx,
        cx.styles,
        hooks,
    );
    let mark = progred_text::value(name);
    let target = mark.clone();
    if cx.selected(path) || cx.selected(&edge) {
        let content = cursor_target(
            edge.clone(),
            target.clone(),
            presentation,
            hooks,
            None,
            content,
        );
        let content = if cx.selected(&edge) {
            content
        } else {
            secondary_mark(cx, &mark, content)
        };
        source_target(cx, edge, Some(target), hooks, content)
    } else {
        let content = secondary_mark(cx, &mark, content);
        if cx.source.transient() {
            content
        } else {
            decorate(content, move |p: &mut P, rect| {
                p.descends().push(Descend { path: edge, rect });
            })
        }
    }
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
fn ground<P: Canvas>(cx: &Cx, path: &[Step], value: &Value, content: Measured<P>) -> Measured<P> {
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
    decorate(content, move |p: &mut P, rect| {
        let bg = RoundedRect::from_rect(rect.inset(3.0 * scale), 5.0 * scale);
        p.fill(bg, color, Affine::IDENTITY);
    })
}

/// The secondary selection's mark: a subtle wash over another whole
/// projection of the selected value — an expanded block, a collapsed
/// handle, or a label. The primary selection's geometry at lower
/// strength, so the two read as one family.
fn secondary_mark<P: Canvas>(cx: &Cx, value: &Value, content: Measured<P>) -> Measured<P> {
    let strong = cx.secondary.as_ref() == Some(value);
    let faint = !strong && cx.secondary_hover.as_ref() == Some(value);
    if !strong && !faint {
        return content;
    }
    let scale = cx.styles.scale;
    decorate(content, move |p: &mut P, rect| {
        let bg = RoundedRect::from_rect(rect.inset(3.0 * scale), 5.0 * scale);
        // The hover variant is the same mark at half voice.
        let (fill, line) = if strong { (0.10, 0.55) } else { (0.05, 0.25) };
        p.fill(bg, Color::new([0.0, 0.48, 1.0, fill]), Affine::IDENTITY);
        p.stroke(
            bg,
            Stroke::new(1.5 * scale),
            Color::new([0.0, 0.48, 1.0, line]),
            Affine::IDENTITY,
        );
    })
}

/// Starts the ordinary projection at a value with no document source.
/// Interaction attributes the transient tree to `owner`, while its
/// children remain read-only and have no document paths of their own.
fn project_transient_root<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    result: Value,
    fuel: usize,
    avail: f64,
    hooks: &Hooks<C>,
) -> Measured<P> {
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
    };
    let result_cx = Cx {
        sources: cx.sources,
        raw: false,
        values: cx.values,
        foreign: cx.foreign,
        collapse: cx.collapse,
        styles: cx.styles,
        selection: None,
        hover: None,
        secondary: None,
        secondary_hover: None,
        source: Source::Transient { owner: path },
        fuel: std::cell::Cell::new(fuel),
    };
    let projected = project_location(
        &result_cx,
        tcx,
        path,
        &HashSet::new(),
        Location::Root(Some(&result)),
        avail,
        &result_hooks,
    );
    // The transient result is not another projection of the stored
    // source value. Its inner views may install ordinary hover claims while
    // rendering, so clear them after placement across this whole arm.
    around(projected, |p, placement, place_inner| {
        place_inner.place(p);
        hover_block(p, placement);
    })
}

/// Adds one graph step to the active source and invokes the supplied
/// projection. The projection, not the caller, resolves the child;
/// a missing child therefore reaches the same total fallback as an
/// empty root.
#[allow(clippy::too_many_arguments)]
fn descend<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    parent_path: &[Step],
    ancestors: &HashSet<CellId>,
    parent: &Value,
    step: Step,
    avail: f64,
    hooks: &Hooks<C>,
) -> Measured<P> {
    let mut path = parent_path.to_vec();
    path.push(step.clone());
    if step == Step::Follow {
        let mut ancestors = ancestors.clone();
        ancestors.extend(parent.as_cell());
        project_location(
            cx,
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
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    location: Location<'_>,
    avail: f64,
    hooks: &Hooks<C>,
) -> Measured<P> {
    match location.value(|cell| cx.sources.value(cell)) {
        Some(value) => project_present_value(cx, tcx, path, ancestors, value, avail, hooks),
        None => pending_view(cx, tcx, path.to_vec(), hooks),
    }
}

#[allow(clippy::too_many_arguments)]
fn project_present_value<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    value: &Value,
    avail: f64,
    hooks: &Hooks<C>,
) -> Measured<P> {
    let projected = if cx.values {
        crate::stack::project(&ProjectEnv { cx }, value)
            .map(|layout| realize(cx, tcx, path, ancestors, hooks, value, layout, avail))
    } else {
        None
    };
    let inner = match projected {
        Some(projected) => projected,
        None => realize(
            cx,
            tcx,
            path,
            ancestors,
            hooks,
            value,
            structure::of(cx, path, ancestors, value),
            avail,
        ),
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
    ground(cx, path, value, placed)
}

/// An EMPTY SLOT at `path`: the [`placeholder`] widget wired to this
/// projection — engagement derived from the selection, wrapped as an
/// ordinary descend so it highlights, clicks, and navigates like the
/// value it may become. Engaged, its placement emits the completion
/// popup for the shell to draw over the body.
fn pending_view<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: Path,
    hooks: &Hooks<C>,
) -> Measured<P> {
    let engaged = match cx.selection {
        Some(Selection::Pending {
            path: pending,
            query,
            choice,
        }) if pending.as_slice() == path.as_slice() => Some((query, *choice)),
        _ => None,
    };
    let content = placeholder(cx, tcx, engaged, false, hooks);
    // Engaged, the generic ring IS the slot's chrome: it draws
    // [`highlight_rect`] over the same frame the cold box strokes,
    // and the same ring survives the commit around the same glyphs —
    // the box never changes, only its paint.
    source_target(cx, path, None, hooks, content)
}

/// The slot widget, in the Puri idiom: its one state input is the
/// engaged pending's `(query, choice)`, and None IS the inactive
/// pending — the cold [`placeholder_box`], whose width the engaged
/// query's frame holds as its minimum, so the two forms are one
/// widget in two states and the transition between them is pure
/// chrome. The caller owns identity (descend, highlight, clicks);
/// `labels` picks the slot's role.
fn placeholder<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    engaged: Option<(&LineEditState, usize)>,
    labels: bool,
    hooks: &Hooks<C>,
) -> Measured<P> {
    match engaged {
        Some((query, choice)) => query_content(cx, tcx, query, choice, labels, hooks),
        None => placeholder_box(tcx, cx.styles),
    }
}

/// A focused completion query: the editor plus its popup, emitted at
/// placement for the shell to draw over the body. Serves both pending
/// stages — a value and a new field's label (`labels` narrows the
/// offers there).
fn query_content<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    query: &LineEditState,
    choice: usize,
    labels: bool,
    hooks: &Hooks<C>,
) -> Measured<P> {
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
    before(content, move |p: &mut P, placement| {
        let rect = placement.rect;
        *p.popup() = Some(Popup {
            anchor: rect,
            entries,
            choice,
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
pub fn popup_view<C: 'static, P: Canvas + HasHandler<C> + HasHover<HoverClaim>>(
    tcx: &mut TextCtx,
    styles: &Styles,
    popup: &Popup,
    hovered: Option<usize>,
    commit: impl Fn(&mut C, &EntryAction) + Clone + 'static,
) -> Measured<P> {
    let scale = styles.scale;
    let choice = popup.choice.min(popup.entries.len().saturating_sub(1));
    // Cells first, so rows can pad out to the widest and the chosen
    // highlight spans the card, not just its own content.
    let cells: Vec<(Measured<P>, Option<Measured<P>>)> = popup
        .entries
        .iter()
        .map(|entry| {
            let style = match &entry.action {
                EntryAction::Value(value) if progred_text::read(value).is_some() => &styles.string,
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
    let rows: Vec<Measured<P>> = cells
        .into_iter()
        .zip(widths)
        .enumerate()
        .map(|(index, ((display, detail), width))| {
            let mut cells: Vec<Measured<P>> = vec![display];
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
            let lit = hovered == Some(index) && !chosen;
            let action = popup.entries[index].action.clone();
            let commit = commit.clone();
            before(content, move |p: &mut P, placement| {
                let rect = placement.rect;
                if chosen {
                    p.fill(
                        RoundedRect::from_rect(rect, 4.0 * scale),
                        Color::new([0.0, 0.48, 1.0, 0.14]),
                        Affine::IDENTITY,
                    );
                } else if lit {
                    p.fill(
                        RoundedRect::from_rect(rect, 4.0 * scale),
                        Color::new([0.0, 0.48, 1.0, 0.08]),
                        Affine::IDENTITY,
                    );
                }
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
    before(card, move |p: &mut P, placement| {
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
fn highlighted<P: Canvas>(
    tcx: &mut TextCtx,
    s: &str,
    matches: &[filter::Match],
    style: &TextStyle,
) -> Measured<P> {
    if matches.is_empty() {
        return text(tcx, s, style);
    }
    let bold = TextStyle {
        weight: Some(700.0),
        ..style.clone()
    };
    let mut segments: Vec<Measured<P>> = Vec::new();
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
fn atom_content<C: 'static, P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends>(
    editing: Option<&LineEditState>,
    fallback: Measured<P>,
    presentation: LineEditPresentation,
    placeholder: Option<(&str, &TextStyle)>,
    tcx: &mut TextCtx,
    styles: &Styles,
    hooks: &Hooks<C>,
) -> Measured<P> {
    match editing {
        Some(line) => {
            let edit_ctx = hooks.edit.clone();
            display::text_edit(
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

/// A click that reports a collapse toggle for `path` without
/// selecting — [`disclosure`]'s click on arbitrary content, the
/// collapsed forms' way back open.
fn toggle_target<C: 'static, P: Canvas + HasHandler<C> + HasHover<HoverClaim>>(
    cx: &Cx,
    path: Path,
    hooks: &Hooks<C>,
    content: Measured<P>,
) -> Measured<P> {
    let scale = cx.styles.scale;
    let hovered =
        matches!(cx.hover, Some(Hover::Toggle(hovered)) if hovered.as_slice() == path.as_slice());
    let toggle = hooks.toggle.clone();
    let target = path.clone();
    let content = before(content, move |p, placement| {
        let rect = placement.rect;
        if hovered {
            hover_highlight(scale, p, rect);
        }
        hover_claim(p, placement, Hover::Toggle(path.clone()));
    });
    on_primary_pointer_down(
        content,
        |_| true,
        move |ctx, _| {
            toggle(ctx, target.clone());
            true
        },
    )
}

/// A flat list separator: its click opens a pending sibling between
/// the elements it separates — after the element at `path`. Only
/// offered where the insert could commit, [`pending_beside`]'s
/// affordance-lie rule.
fn insert_target<C: 'static, P: Canvas + HasHandler<C> + HasHover<HoverClaim>>(
    cx: &Cx,
    path: Path,
    hooks: &Hooks<C>,
    content: Measured<P>,
) -> Measured<P> {
    let scale = cx.styles.scale;
    let hovered =
        matches!(cx.hover, Some(Hover::Insert(hovered)) if hovered.as_slice() == path.as_slice());
    let insert = hooks.insert.clone();
    let target = path.clone();
    let content = before(content, move |p, placement| {
        let rect = placement.rect;
        if hovered {
            hover_highlight(scale, p, rect);
        }
        hover_claim(p, placement, Hover::Insert(path.clone()));
    });
    on_primary_pointer_down(
        content,
        |_| true,
        move |ctx, _| {
            insert(ctx, target.clone());
            true
        },
    )
}

/// A command-click pick target with no plain-click behavior — for
/// parts like a pending row's label, whose plain click deliberately
/// falls through.
fn pick_target<C: 'static, P: Canvas + HasHandler<C> + HasHover<HoverClaim>>(
    key: CellId,
    hooks: &Hooks<C>,
    content: Measured<P>,
) -> Measured<P> {
    let pick = hooks.pick.clone();
    on_primary_pointer_down(
        content,
        |event| command(&event.state.modifiers),
        move |ctx, _| pick(ctx, Value::Cell(key)),
    )
}

/// The label stage engaged — a rename's re-opened label or a new
/// field's — its query wearing the primary ring explicitly: a
/// pending edge has no path of its own for [`descend`] to mark, and
/// the ring spans the QUERY frame alone, the way a value pending's
/// does. Clicks inside belong to the query's own caret target;
/// clicks beside fall through like any pending's.
fn label_query<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    query: &LineEditState,
    choice: usize,
    hooks: &Hooks<C>,
) -> Measured<P> {
    let scale = cx.styles.scale;
    let content = placeholder(cx, tcx, Some((query, choice)), true, hooks);
    let ringed = decorate(content, move |p: &mut P, rect| {
        primary_highlight(scale, p, rect);
    });
    // The ring's outset rides inside the node, so glued neighbors —
    // the colon, a flat comma — clear its ink.
    pad(Insets::new(4.0 * scale, 0.0, 4.0 * scale, 0.0), ringed)
}

/// A writable field label's one pointer job: a plain click re-opens
/// it as its seeded query — selecting the field belongs to the
/// value's own ink, which claims the same path. Command-clicks
/// decline so the head's pick still wins; read-only labels never
/// register and keep the head's select.
fn rename_target<C: 'static, P: Canvas + HasHandler<C> + HasHover<HoverClaim>>(
    cx: &Cx,
    path: Path,
    layout: TextLayout<Brush>,
    hooks: &Hooks<C>,
    content: Measured<P>,
) -> Measured<P> {
    let scale = cx.styles.scale;
    let hovered =
        matches!(cx.hover, Some(Hover::Label(hovered)) if hovered.as_slice() == path.as_slice());
    let rename = hooks.rename.clone();
    before(content, move |p, placement| {
        let rect = placement.rect;
        if hovered {
            hover_highlight(scale, p, rect);
        }
        hover_claim(p, placement, Hover::Label(path.clone()));
        let rename = rename.clone();
        let target = path.clone();
        let layout = layout.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            event.button == Some(PointerButton::Primary)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && !command(&event.state.modifiers)
                && {
                    let index = caret_index(
                        &layout,
                        Point::new(
                            event.state.position.x - rect.x0,
                            event.state.position.y - rect.y0,
                        ),
                    );
                    rename(ctx, target.clone(), index);
                    true
                }
        });
    })
}

/// A plain click-to-select target for `path` — for parts like labels
/// and the cell star that select without carrying an editor click.
/// With the command modifier and a pending open, picks `value` — the
/// identity the part displays — into it instead.
fn select_target<C: 'static, P: Canvas + HasHandler<C> + HasHover<HoverClaim>>(
    path: Path,
    value: Value,
    hooks: &Hooks<C>,
    content: Measured<P>,
) -> Measured<P> {
    let claimed = hover_target(path.clone(), content);
    quiet_select_target(path, value, hooks, claimed)
}

/// Name the value at `path` for the pointer over this ink, adding no
/// click of its own — the hover half of [`select_target`], and the
/// flat literal's delimiter dress.
fn hover_target<P: Canvas + HasHover<HoverClaim>>(path: Path, content: Measured<P>) -> Measured<P> {
    before(content, move |p, placement| {
        hover_claim(p, placement, Hover::Value(path.clone()));
    })
}

/// [`select_target`] minus the pointer claim — for a container's
/// one-line literal, whose interior air belongs to the landmark's
/// hold and whose delimiter ink names the container through
/// [`hover_target`].
fn quiet_select_target<C: 'static, P: Canvas + HasHandler<C> + HasHover<HoverClaim>>(
    path: Path,
    value: Value,
    hooks: &Hooks<C>,
    content: Measured<P>,
) -> Measured<P> {
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
fn cursor_target<C: 'static, P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends>(
    path: Path,
    value: Value,
    presentation: LineEditPresentation,
    hooks: &Hooks<C>,
    line: Option<display::LineEdit>,
    content: Measured<P>,
) -> Measured<P> {
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
mod tests;
#[cfg(test)]
mod svg_bench;
