//! The raw projection: any document rendered with no schema, in the
//! delimiter family — `(` cell `)`, `[` list `]`, `{` record `}`.
//! A cell heads with its conventional simple name (the ordinary name
//! field projected in place) or its short id; records are field rows,
//! lists inline literals or bare element rows; atoms render as their
//! values; positions are session bookkeeping and never render at all.

use crate::conventions::Names;
use crate::display::{Language, NodeLanguage, Styles, TextRole};
use crate::completion::{
    Entry, EntryAction, HasPopup, Popup, completion_entries,
};
#[cfg(test)]
use crate::completion::{resolve_entry, resolve_label};
use crate::document::{Document, Path, short_id};
#[cfg(test)]
use crate::document::{sample_document, sample_vocabulary};
use crate::filter;
use crate::selection::{Collapse, Selection, last_follow, writable_at};
#[cfg(test)]
use crate::selection::{
    break_edit_run, delete_edge, from_clipboard, from_structure, line_edit, pending_edge,
    pending_follow, pending_insert, pending_into, pending_rename, pending_value, rename_field,
    resolve_query, set_collapse, set_value, to_clipboard, toggle_collapse, write_through,
};
use crate::hover::{HasHover, Hover, HoverClaim, Hovering, hover_value, resolve_hover};
use crate::navigate::{Descend, HasDescends};
#[cfg(test)]
use crate::navigate::{projected_name_owner, step_selection};
use crate::layout::{
    Extent, Node, around, before, col, decorate, leaf, min_width, on_primary_pointer_down, pad,
    row, text, text_edit,
};
use crate::sources::Sources;
use crate::projection::{self, Location};
use im::OrdMap;
use parley::layout::Layout;
use progred_graph::{CellId, Cells, Position, Step, Value, hex_string, new_cell_id};
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
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};
use ui_events::pointer::PointerButton;
use vello::kurbo::{Affine, Insets, Point, Rect, RoundedRect, Stroke};
use vello::peniko::{Brush, Color};

/// Read-only projection context threaded through every view.
struct Cx<'a> {
    /// The reading context: the document read over its library.
    sources: Sources<'a>,
    /// The editor's name policy; every display-name check asks it,
    /// through [`Cx::name`], which derives from the raw bit.
    names: &'a Names,
    /// The Raw view, ONE bit of view state: convention layers derive
    /// from it — names answer None through [`Cx::name`]; domain
    /// projections, when they arrive, stand down through the same
    /// bit. Nothing else is swapped anywhere.
    raw: bool,
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
    fn name(&self, cell: CellId) -> Option<String> {
        crate::conventions::display_name(&self.sources, self.names, self.raw, cell)
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
) -> Node<P> {
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
fn flat_delim<P: Canvas>(styles: &Styles, delim: Delim, open: bool) -> Node<P> {
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
fn tall_delim<P: Canvas>(styles: &Styles, delim: Delim, open: bool, content: Extent) -> Node<P> {
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
    content: Node<P>,
) -> Node<P> {
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
fn placeholder_box<P: Canvas>(tcx: &mut TextCtx, styles: &Styles) -> Node<P> {
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
    child: Node<P>,
) -> Node<P> {
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
                !matches!(value, Value::Record(_)) || projection::whole_text(value).is_some()
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
    pub names: &'a Names,
    pub raw: bool,
    pub styles: &'a Styles,
    pub width: f64,
    pub projection: projection::Projection<'a>,
}

pub fn project<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    description: ProjectDescription<'_>,
    tcx: &mut TextCtx,
    hooks: Hooks<C>,
) -> Node<P> {
    let ProjectDescription {
        sources,
        selection,
        graph_node,
        hover,
        hover_node,
        collapse,
        names,
        raw,
        styles,
        width,
        projection,
    } = description;
    let cx = Cx {
        sources,
        names,
        raw,
        collapse,
        styles,
        selection,
        hover,
        source: Source::Stored,
        // The graph view's selected cell is a secondary here too:
        // its projections are the same value — and the graph view's
        // HOVERED cell is a hover secondary the same way.
        secondary: secondary_of(&sources, selection).or_else(|| graph_node.cloned()),
        secondary_hover: hover
            .and_then(|hover| hover_value(&sources, names, raw, selection, hover))
            .or_else(|| hover_node.cloned()),
    };
    // The Raw view derives from the one bit: names answer None and
    // nothing else changes — lists and records render as themselves
    // there too, since kind is data, not convention. An empty
    // document is a selectable placeholder at the root path.
    (if raw { projection.raw() } else { projection }).project::<C, P>(
        &cx,
        tcx,
        &[],
        &HashSet::new(),
        Location::Root(sources.root()),
        width,
        &hooks,
    )
}

/// A link rendered as its cell: PARENS are the cell's syntax — `(`
/// name-or-short-id value `)` — completing the delimiter family
/// (brackets say list, braces say record). A conventional simple-name
/// field, when present, is projected as the head while retaining its
/// ordinary `Follow, Key(name)` path — selectable, editable,
/// two-stage. The value after the head is an
/// ordinary [`descend`] at the Follow step, whatever its kind:
/// the drawn parens stretch over whatever height it takes, and when
/// head-beside-value overflows the width remaining here the cell
/// BREAKS like a field row — head on its own line, value dropped
/// below at the tab, parens spanning both. A WRITABLE valueless cell
/// — a bare cell — renders the [`placeholder`] box in
/// the value's place (the empty-slot rule in [`Selection::edge`]
/// makes selecting it begin the first value); an external valueless
/// cell renders head-only, complete. Cells COLLAPSE like containers
/// — Space toggles the override at the cell's path — but the
/// collapsed form is `( … )`: pure elision, never a summary, a cell
/// does not introspect its value. CYCLE RE-ENTRY is the same
/// machinery with the DEFAULT flipped: the repeated cell defaults
/// collapsed, and expanding — Space, or clicking the ellipsis —
/// opens one more turn, as deep as you care to follow. The parens
/// and the head claim cell-selection; gaps between claims fall
/// through.
#[allow(clippy::too_many_arguments)]
fn cell_view<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    cell: CellId,
    avail: f64,
    hooks: &Hooks<C>,
    projection: projection::Projection<'_>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let name = cx.name(cell);
    let target = Value::from(cell);
    let mut followed = path.to_vec();
    followed.push(Step::Follow);
    let value = cx.sources.value(cell).cloned();
    // A pending inside the value forces the cell open.
    let pending_inside = cx.pending_child_of(&followed).is_some()
        || cx.pending_edge_under(&followed).is_some()
        || cx.pending_rename_under(&followed).is_some();
    let elided = value.is_some()
        && !pending_inside
        && cx.collapse.collapsed(path, ancestors.contains(&cell));
    if elided {
        // The ellipsis is the way back open: clicking it expands one
        // turn (the parens still select the cell).
        return bracketed(
            cx,
            Delim::Paren,
            path,
            &target,
            hooks,
            toggle_target(cx, path.to_vec(), hooks, text(tcx, "…", &cx.styles.dim)),
        );
    }
    // The head claims cell-selection — the gap beside the name
    // included; an engaged name inside still wins its own clicks.
    let head = select_target(
        path.to_vec(),
        target.clone(),
        hooks,
        head_view(cx, tcx, path, cell, &name, hooks),
    );
    let content = match &value {
        // A writable bare cell's slot invites its first value; an
        // EXTERNAL bare cell is complete as it stands — no hole, no
        // invitation, the affordance-lie rule in notation.
        None if cx.sources.writable(cell) => row(
            4.0 * scale,
            vec![
                head,
                descend(
                    cx,
                    tcx,
                    path,
                    ancestors,
                    &target,
                    Step::Follow,
                    avail,
                    hooks,
                    projection,
                ),
            ],
        ),
        None => head,
        Some(_) => {
            let mut inner = ancestors.clone();
            inner.insert(cell);
            // The field-row discipline inside the parens: hug only
            // where the value stays WHOLE beside the head, probed
            // with a CLOSED unbounded build; else drop at the tab
            // with its wider budget. A head narrower than the tab
            // hugs whatever the value does — dropping there buys no
            // room — and that guard, or the short-circuit at no room
            // beside, decides the unbounded and crushed budgets
            // without probing.
            let inside = avail - 2.0 * (delim_advance(cx.styles, Delim::Paren) + 2.0 * scale);
            let beside = inside - head.extent.width - 4.0 * scale;
            let tab = 20.0 * scale;
            let hug = beside >= inside - tab
                || (beside > 0.0
                    && descend::<C, P>(
                        cx,
                        tcx,
                        path,
                        &inner,
                        &target,
                        Step::Follow,
                        f64::INFINITY,
                        hooks,
                        projection,
                    )
                        .extent
                        .width
                        <= beside);
            let value_node = descend(
                cx,
                tcx,
                path,
                &inner,
                &target,
                Step::Follow,
                if hug { beside } else { inside - tab }.max(0.0),
                hooks,
                projection,
            );
            if hug {
                row(4.0 * scale, vec![head, value_node])
            } else {
                col(
                    0,
                    2.0 * scale,
                    vec![head, pad(Insets::new(tab, 0.0, 0.0, 0.0), value_node)],
                )
            }
        }
    };
    bracketed(cx, Delim::Paren, path, &target, hooks, content)
}

/// Marks `child` as the projection of `path` WITHOUT claiming any
/// clicks: the highlight, reveal rect, and keyboard reach of
/// [`descend`] over the full bounds, while pointer selection belongs
/// to the content targets the view registers — heads, delimiters,
/// rows — so clicks on structural whitespace (gutters, inter-row
/// gaps, the dead space inside a bounding box) fall through to the
/// background's deselect.
fn descend_landmark<P: Canvas + HasDescends>(cx: &Cx, path: Path, child: Node<P>) -> Node<P> {
    if cx.source.transient() {
        return child;
    }
    let selected = cx.selected(&path);
    let hovered = cx.hovered_value(&path);
    let scale = cx.styles.scale;
    decorate(child, move |p: &mut P, rect| {
        if selected {
            primary_highlight(scale, p, rect);
        } else if hovered {
            hover_highlight(scale, p, rect);
        }
        p.descends().push(Descend {
            path: path.clone(),
            rect,
        });
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
    name: &Option<String>,
    hooks: &Hooks<C>,
) -> Node<P> {
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
        let content = cursor_target(edge.clone(), target.clone(), presentation, hooks, content);
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

/// The projection-level account of evaluation: the stored expression
/// remains an ordinary editable projection, followed by projection
/// chrome and the read-only transient result. Keeping the expression
/// arm ordinary also preserves hover/secondary links to its other cell
/// projections. Prefer one line; when it cannot fit, keep the arrow
/// attached to the result on the following row.
#[allow(clippy::too_many_arguments)]
fn evaluation_projection<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    expression: &Value,
    result: Value,
    avail: f64,
    hooks: &Hooks<C>,
    projection: projection::Projection<'_>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let gap = 6.0 * scale;
    let data_projection = projection.without_evaluation();
    let flat = (avail > 0.0)
        .then(|| {
            let expression = project_present_value(
                cx,
                tcx,
                path,
                ancestors,
                expression,
                f64::INFINITY,
                hooks,
                data_projection,
            );
            let arrow = NodeLanguage::<C, P>::new(
                tcx,
                cx.styles,
                None,
                hooks.edit.clone(),
            )
            .text("→", TextRole::Dim);
            let result = project_transient_root(
                cx,
                tcx,
                path,
                result.clone(),
                f64::INFINITY,
                hooks,
                data_projection,
            );
            NodeLanguage::<C, P>::new(tcx, cx.styles, None, hooks.edit.clone())
                .row(6.0, vec![expression, arrow, result])
        })
        .filter(|candidate| one_line(candidate.extent, scale));
    if let Some(candidate) = flat.filter(|candidate| candidate.extent.width <= avail) {
        return candidate;
    }

    let arrow = NodeLanguage::<C, P>::new(tcx, cx.styles, None, hooks.edit.clone())
        .text("→", TextRole::Dim);
    let result_avail = (avail - arrow.extent.width - gap).max(0.0);
    let expression = project_present_value(
        cx,
        tcx,
        path,
        ancestors,
        expression,
        avail,
        hooks,
        data_projection,
    );
    let result = project_transient_root(
        cx,
        tcx,
        path,
        result,
        result_avail,
        hooks,
        data_projection,
    );
    let result_row = NodeLanguage::<C, P>::new(tcx, cx.styles, None, hooks.edit.clone())
        .row(6.0, vec![arrow, result]);
    NodeLanguage::<C, P>::new(tcx, cx.styles, None, hooks.edit.clone())
        .col(0, 2.0, vec![expression, result_row])
}

/// One record field row: the label-and-colon head, then the value (or
/// its pending query). `parent` is the record's own path — a cell's
/// followed path or an inline record's. A real field's label and
/// colon select the field, like its value — grouped so one target
/// spans both and the gap between. A pending row's plain click falls
/// through (the not-yet-field can't be selected), but command still
/// picks its label's identity. Three alternatives, the outermost
/// level degrading first: the value HUGS the label while it stays
/// whole beside it; else it DROPS below at a fixed tab with the drop
/// position's wider budget — never aligned under the label's own
/// width, which is the indentation that drifts. A head narrower than
/// the tab hugs whatever the value does — dropping there buys no
/// room — which is where the lisp-flavored broken-beside form
/// survives, and the overflow answer when nothing fits anywhere.
#[allow(clippy::too_many_arguments)]
fn field_row<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    parent: &[Step],
    ancestors: &HashSet<CellId>,
    parent_value: &Value,
    key: CellId,
    value: Option<Value>,
    avail: f64,
    hooks: &Hooks<C>,
    projection: projection::Projection<'_>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let mut child = parent.to_vec();
    child.push(Step::Key(key));
    // A re-opened label renders as its seeded query; cold, a
    // writable label's one click is its own edit — selecting the
    // field belongs to the value's ink (and the head's colon), which
    // already claims the same path.
    let label = match cx.pending_rename_under(parent) {
        Some((replacing, query, choice)) if replacing == &key => {
            label_query(cx, tcx, query, choice, hooks)
        }
        _ => field_label(cx, tcx, parent, child.clone(), &key, hooks),
    };
    let head = row(0.0, vec![label, text(tcx, ":", &cx.styles.dim)]);
    let head = match &value {
        Some(_) => select_target(child.clone(), Value::Cell(key), hooks, head),
        None => pick_target(key, hooks, head),
    };
    let Some(_) = value else {
        return row(
            6.0 * scale,
            vec![
                head,
                descend(
                    cx,
                    tcx,
                    parent,
                    ancestors,
                    parent_value,
                    Step::Key(key),
                    avail,
                    hooks,
                    projection,
                ),
            ],
        );
    };
    // The hug decision probes the value's FLAT form: hug only where
    // the value stays WHOLE beside the label, so the first break
    // lands at the outermost level that cannot stay flat — the
    // literal gate's ordering carried into the hug seam. The
    // unbounded probe is CLOSED — every nested fit test passes, so
    // nothing branches inside (the literal candidates' own build) —
    // and ONE real build follows at the chosen position; building
    // both positions recursed probes-within-probes and went
    // exponential exactly at narrow widths.
    let beside = avail - head.extent.width - 6.0 * scale;
    let tab = 20.0 * scale;
    // A head narrower than the tab hugs whatever the value does: the
    // drop would offer LESS room and overflow wider, so the guard is
    // both the lisp-flavored form's remaining home and the overflow
    // tie-break. That guard at unbounded budgets, and the short-
    // circuit at no room beside, keep forced builds probe-free — a
    // probe that probed would recurse the exponential right back.
    let hug = beside >= avail - tab
        || (beside > 0.0
            && descend::<C, P>(
                cx,
                tcx,
                parent,
                ancestors,
                parent_value,
                Step::Key(key),
                f64::INFINITY,
                hooks,
                projection,
            )
            .extent
            .width
                <= beside);
    let content = descend(
        cx,
        tcx,
        parent,
        ancestors,
        parent_value,
        Step::Key(key),
        if hug { beside } else { avail - tab }.max(0.0),
        hooks,
        projection,
    );
    if hug {
        row(6.0 * scale, vec![head, content])
    } else {
        col(
            0,
            2.0 * scale,
            vec![head, pad(Insets::new(tab, 0.0, 0.0, 0.0), content)],
        )
    }
}

/// The label-query row of a new field being authored on a record. The
/// authoring locus carries the primary itself; its parent is
/// deliberately unmarked.
fn pending_edge_row<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    query: &LineEditState,
    choice: usize,
    hooks: &Hooks<C>,
) -> Node<P> {
    // Both stages through the slot widget: the label engaged and
    // wearing the ring — ONLY the label, as a re-opened rename wears
    // it, so the cold value slot's box stands clear instead of
    // colliding with a row-wide outline — the value to come cold.
    let pending_row = row(
        0.0,
        vec![
            label_query(cx, tcx, query, choice, hooks),
            text(tcx, ": ", &cx.styles.dim),
            placeholder(cx, tcx, None, false, hooks),
        ],
    );
    before(pending_row, move |p: &mut P, placement| {
        // The row owns its clicks: nothing here means "select the
        // parent", so nothing may fall through to it. (The query's
        // caret target, registered after, still wins inside itself.)
        hover_block(p, placement);
        p.handler().on_pointer_down(move |_, event| {
            event.button == Some(PointerButton::Primary)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
        });
    })
}

/// A list value: its elements as bare ordered rows — the position is
/// session identity, not information; order carries it. Collapsed —
/// override-only; a value has no identity to recur through — it
/// elides to `[ … ]`; a list whose literal `["a", "b"]` fits
/// the width and stays one line reads as that literal; anything else
/// takes the block form, the drawn brackets spanning the element
/// rows as a column. Lists have no identity, so there is no head of
/// their own and no cycle through them — only linked cells can
/// recurse; a cell holding one wraps this same view in its stretched
/// parens.
#[allow(clippy::too_many_arguments)]
fn list_view<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    elements: &OrdMap<Position, Value>,
    avail: f64,
    hooks: &Hooks<C>,
    projection: projection::Projection<'_>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let mut items: Vec<(Position, Option<Value>)> = elements
        .iter()
        .map(|(position, value)| (position.clone(), Some(value.clone())))
        .collect();
    if let Some(Step::Element(position)) = cx.pending_child_of(path) {
        items.push((position, None));
        items.sort_by(|a, b| a.0.cmp(&b.0));
    }
    let target = Value::List(elements.clone());

    // A pending child forces the list open; the collapse override
    // outranks the layout the content would pick. Collapsed is pure
    // elision — no summary — and the ellipsis is the way back open.
    let collapsed = !items.is_empty()
        && items.iter().all(|(_, value)| value.is_some())
        && cx.collapse.collapsed(path, false);
    if collapsed {
        return select_target(
            path.to_vec(),
            target,
            hooks,
            row(
                4.0 * scale,
                vec![
                    flat_delim(cx.styles, Delim::Bracket, true),
                    toggle_target(cx, path.to_vec(), hooks, text(tcx, "…", &cx.styles.dim)),
                    flat_delim(cx.styles, Delim::Bracket, false),
                ],
            ),
        );
    }

    // The literal candidate, kept when it FITS: within the width
    // remaining here and one line tall (a pending inside can force a
    // child open, and a broken child disqualifies the literal,
    // however narrow). Children build against an UNBOUNDED budget, so
    // every nested fit test passes and the candidate materializes in
    // one all-flat construction — no branching inside; this enclosing
    // test is the one gate (Wadler's fits test, operationally). On
    // rejection the block form rebuilds them against its own columns.
    // At zero budget no literal can be accepted; skipping the
    // candidate keeps zero-budget probe builds closed and cheap. An
    // EMPTY list is the exception both ways: `[]` is its one form —
    // a block of zero rows is not a representation — so it takes the
    // literal whatever the width says.
    let bare = items.is_empty();
    let writable = writable_at(&cx.sources, path);
    let mut flat = (avail > 0.0 || bare)
        .then(|| {
            let mut cells: Vec<Node<P>> = vec![hover_target(
                path.to_vec(),
                flat_delim(cx.styles, Delim::Bracket, true),
            )];
            for (index, (position, _)) in items.iter().enumerate() {
                if index > 0 {
                    // The separator is the between: writable, its click
                    // opens a pending right here.
                    let separator = text(tcx, ", ", &cx.styles.dim);
                    cells.push(if writable {
                        let mut previous = path.to_vec();
                        previous.push(Step::Element(items[index - 1].0.clone()));
                        insert_target(cx, previous, hooks, separator)
                    } else {
                        separator
                    });
                }
                cells.push(descend(
                    cx,
                    tcx,
                    path,
                    ancestors,
                    &target,
                    Step::Element(position.clone()),
                    f64::INFINITY,
                    hooks,
                    projection,
                ));
            }
            cells.push(hover_target(
                path.to_vec(),
                flat_delim(cx.styles, Delim::Bracket, false),
            ));
            row(0.0, cells)
        })
        .filter(|candidate| one_line(candidate.extent, scale));
    if let Some(candidate) = flat.take_if(|candidate| candidate.extent.width <= avail || bare) {
        // The one-line literal is all content: it selects the list
        // whole, elements winning their own spans — and stays QUIET
        // for the pointer, so its gaps hold whatever the hover was.
        return quiet_select_target(path.to_vec(), target, hooks, candidate);
    }

    let inside = (avail - 2.0 * (delim_advance(cx.styles, Delim::Bracket) + 2.0 * scale)).max(0.0);
    // Element rows are bare values: the spanning brackets already
    // say "list", every multi-line element carries its own
    // delimiter, and each value's ink selects its element — a
    // leading dash would restate all three.
    let rows: Vec<Node<P>> = items
        .into_iter()
        .map(|(position, _)| {
            descend(
                cx,
                tcx,
                path,
                ancestors,
                &target,
                Step::Element(position),
                inside,
                hooks,
                projection,
            )
        })
        .collect();
    // The block form: the brackets span the element column and are
    // the list's click claims; everything between the rows falls
    // through. Collapsing is Space on the selection — no button.
    let block = bracketed(
        cx,
        Delim::Bracket,
        path,
        &target,
        hooks,
        col(0, 4.0 * scale, rows),
    );
    // The general rule: first alternative that FITS, in priority
    // order; when none fits, the NARROWEST attempted, priority
    // breaking ties. A small list's block form can be WIDER than its
    // literal (the dash overhead), and kicking to it would overflow
    // more.
    match flat {
        Some(candidate) if candidate.extent.width <= block.extent.width => {
            quiet_select_target(path.to_vec(), target, hooks, candidate)
        }
        _ => block,
    }
}

/// A record value: an anonymous content-compared value, BRACED —
/// braces mark records the way parens mark cells. Field rows at the
/// record's own path. Collapsed — override-only, since a value has
/// no identity to recur through — it elides to `{ … }`; a
/// record whose literal `{x: "1", y: "2"}` fits the width and stays
/// one line reads as that literal; anything else takes the block
/// form, the drawn braces spanning the field rows as a column. A
/// cell holding one wraps this same view in its stretched parens.
#[allow(clippy::too_many_arguments)]
fn record_view<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    fields: &OrdMap<CellId, Value>,
    avail: f64,
    hooks: &Hooks<C>,
    projection: projection::Projection<'_>,
) -> Node<P> {
    let scale = cx.styles.scale;
    let consumes_simple_name = !cx.raw
        && path
            .split_last()
            .filter(|(step, _)| matches!(step, Step::Follow))
            .and_then(|(_, parent)| cx.sources.resolve(parent))
            .and_then(Value::as_cell)
            .and_then(|cell| cx.name(cell))
            .is_some()
        && fields
            .get(&progred_name::vocabulary::NAME)
            .and_then(projection::whole_text)
            .is_some_and(|name| !name.is_empty());
    let mut items: Vec<(CellId, Option<Value>)> = fields
        .iter()
        .filter(|(key, _)| {
            !consumes_simple_name || **key != progred_name::vocabulary::NAME
        })
        .map(|(key, value)| (*key, Some(value.clone())))
        .collect();
    if let Some(Step::Key(key)) = cx.pending_child_of(path) {
        items.push((key, None));
    }
    items.sort_by(|(left, _), (right, _)| match (cx.name(*left), cx.name(*right)) {
        (Some(left_name), Some(right_name)) => {
            left_name.cmp(&right_name).then(left.cmp(right))
        }
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => left.cmp(right),
    });
    let pending_edge = cx.pending_edge_under(path).is_some();
    let renaming = cx.pending_rename_under(path);
    let target = Value::Record(fields.clone());

    // A pending inside forces the record open; the collapse override
    // outranks the layout the content would pick. Collapsed is pure
    // elision — no summary — and the ellipsis is the way back open.
    let collapsed = !items.is_empty()
        && !pending_edge
        && renaming.is_none()
        && items.iter().all(|(_, value)| value.is_some())
        && cx.collapse.collapsed(path, false);
    if collapsed {
        return select_target(
            path.to_vec(),
            target,
            hooks,
            row(
                4.0 * scale,
                vec![
                    flat_delim(cx.styles, Delim::Brace, true),
                    toggle_target(cx, path.to_vec(), hooks, text(tcx, "…", &cx.styles.dim)),
                    flat_delim(cx.styles, Delim::Brace, false),
                ],
            ),
        );
    }

    // The literal candidate, kept when it FITS: within the width
    // remaining here and one line tall (a pending inside can force a
    // child open). A new field's label query rides the literal like
    // any other fragment — authoring alone never forces the block
    // form. Children build against an UNBOUNDED budget, so every
    // nested fit test passes and the candidate materializes in one
    // all-flat construction — no branching inside; this enclosing
    // test is the one gate (Wadler's fits test, operationally). On
    // rejection the block form rebuilds them against its own columns.
    // At zero budget no literal can be accepted; skipping the
    // candidate keeps zero-budget probe builds closed and cheap. An
    // EMPTY record is the exception both ways: `{}` is its one form —
    // a block of zero rows is not a representation — so it takes the
    // literal whatever the width says. An active label query counts
    // as content and layouts normally.
    let bare = items.is_empty() && !pending_edge;
    let mut flat = (avail > 0.0 || bare)
        .then(|| {
            let mut cells: Vec<Node<P>> = vec![hover_target(
                path.to_vec(),
                flat_delim(cx.styles, Delim::Brace, true),
            )];
            for (index, (key, _)) in items.iter().enumerate() {
                if index > 0 {
                    cells.push(text(tcx, ", ", &cx.styles.dim));
                }
                let mut child = path.to_vec();
                child.push(Step::Key(*key));
                cells.push(match renaming {
                    Some((replacing, query, choice)) if replacing == key => {
                        label_query(cx, tcx, query, choice, hooks)
                    }
                    _ => field_label(cx, tcx, path, child.clone(), key, hooks),
                });
                cells.push(text(tcx, ": ", &cx.styles.dim));
                cells.push(descend(
                    cx,
                    tcx,
                    path,
                    ancestors,
                    &target,
                    Step::Key(*key),
                    f64::INFINITY,
                    hooks,
                    projection,
                ));
            }
            if let Some((query, choice)) = cx.pending_edge_under(path) {
                if !items.is_empty() {
                    cells.push(text(tcx, ", ", &cx.styles.dim));
                }
                cells.push(pending_edge_row(cx, tcx, query, choice, hooks));
            }
            cells.push(hover_target(
                path.to_vec(),
                flat_delim(cx.styles, Delim::Brace, false),
            ));
            row(0.0, cells)
        })
        .filter(|candidate| one_line(candidate.extent, scale));
    if let Some(candidate) = flat.take_if(|candidate| candidate.extent.width <= avail || bare) {
        // The one-line literal is all content: it selects the record
        // whole, fields winning their own spans — and stays QUIET
        // for the pointer, so its gaps hold whatever the hover was.
        return quiet_select_target(path.to_vec(), target, hooks, candidate);
    }

    let inside = (avail - 2.0 * (delim_advance(cx.styles, Delim::Brace) + 2.0 * scale)).max(0.0);
    let mut rows: Vec<Node<P>> = items
        .into_iter()
        .map(|(key, value)| {
            field_row(
                cx,
                tcx,
                path,
                ancestors,
                &target,
                key,
                value,
                inside,
                hooks,
                projection,
            )
        })
        .collect();
    // A new field being authored: the label query, unsorted until it
    // has a label to sort by.
    if let Some((query, choice)) = cx.pending_edge_under(path) {
        rows.push(pending_edge_row(cx, tcx, query, choice, hooks));
    }
    // The block form: the braces span the field column and are the
    // record's click claims; everything between the rows falls
    // through. Collapsing is Space on the selection — no button.
    let block = bracketed(
        cx,
        Delim::Brace,
        path,
        &target,
        hooks,
        col(0, 4.0 * scale, rows),
    );
    // The general rule: first alternative that FITS, in priority
    // order; when none fits, the NARROWEST attempted, priority
    // breaking ties. A small record's block form can be WIDER than
    // its literal, and kicking to it would overflow more.
    match flat {
        Some(candidate) if candidate.extent.width <= block.extent.width => {
            quiet_select_target(path.to_vec(), target, hooks, candidate)
        }
        _ => block,
    }
}

/// A blob's display: `0x` and its bytes, truncated past sixteen —
/// the mini hex editor is a later projection; this is the floor.
fn blob_text(bytes: &[u8]) -> String {
    if bytes.len() <= 16 {
        format!("0x{}", hex_string(bytes))
    } else {
        format!("0x{}… ({} bytes)", hex_string(&bytes[..8]), bytes.len())
    }
}

/// The spelling and face a label draws with — one truth for the view
/// and for hit-testing a click against what was actually drawn.
fn label_spelling<'a>(cx: &'a Cx, key: &CellId) -> (String, &'a TextStyle) {
    match cx.name(*key) {
        Some(name) => (name, &cx.styles.label),
        None => (short_id(*key), &cx.styles.id),
    }
}

fn label_view<P: Canvas>(cx: &Cx, tcx: &mut TextCtx, key: &CellId) -> Node<P> {
    let (spelling, style) = label_spelling(cx, key);
    let inner = text(tcx, &spelling, style);
    secondary_mark(cx, &Value::Cell(*key), inner)
}

/// A cold field label; writable, its one click re-opens it as the
/// seeded rename, the caret hit-tested against this very layout.
fn field_label<C: 'static, P: Canvas + HasHandler<C> + HasHover<HoverClaim>>(
    cx: &Cx,
    tcx: &mut TextCtx,
    parent: &[Step],
    child: Path,
    key: &CellId,
    hooks: &Hooks<C>,
) -> Node<P> {
    let cold = label_view(cx, tcx, key);
    if writable_at(&cx.sources, parent) {
        let (spelling, style) = label_spelling(cx, key);
        let layout = line_layout(tcx, &spelling, style);
        rename_target(cx, child, layout, hooks, cold)
    } else {
        cold
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
fn ground<P: Canvas>(cx: &Cx, path: &[Step], value: &Value, content: Node<P>) -> Node<P> {
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
fn secondary_mark<P: Canvas>(cx: &Cx, value: &Value, content: Node<P>) -> Node<P> {
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

fn projected_value_view<
    C: 'static,
    P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    value: &Value,
    hooks: &Hooks<C>,
    editing: Option<&LineEditState>,
    projection: projection::Projection<'_>,
) -> Option<Node<P>> {
    let mut display = NodeLanguage::<C, P>::new(tcx, cx.styles, editing, hooks.edit.clone());
    let projected = projection.try_project(&mut display, value)?;
    Some(match projected.editor {
        Some(presentation) => cursor_target(
            path.to_vec(),
            value.clone(),
            cx.styles.edit_presentation(&presentation),
            hooks,
            projected.view,
        ),
        None => select_target(path.to_vec(), value.clone(), hooks, projected.view),
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
    avail: f64,
    hooks: &Hooks<C>,
    projection: projection::Projection<'_>,
) -> Node<P> {
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
    };
    let result_cx = Cx {
        sources: cx.sources,
        names: cx.names,
        raw: false,
        collapse: cx.collapse,
        styles: cx.styles,
        selection: None,
        hover: None,
        secondary: None,
        secondary_hover: None,
        source: Source::Transient { owner: path },
    };
    let projected = projection.project(
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
    projection: projection::Projection<'_>,
) -> Node<P> {
    let mut path = parent_path.to_vec();
    path.push(step.clone());
    if step == Step::Follow {
        let mut ancestors = ancestors.clone();
        ancestors.extend(parent.as_cell());
        projection.project(
            cx,
            tcx,
            &path,
            &ancestors,
            Location::Child { parent, step },
            avail,
            hooks,
        )
    } else {
        projection.project(
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

impl projection::Projection<'_> {
    #[allow(clippy::too_many_arguments)]
    fn project<
        C: 'static,
        P: Canvas + HasHandler<C> + HasHover<HoverClaim> + HasDescends + HasPopup,
    >(
        self,
        cx: &Cx,
        tcx: &mut TextCtx,
        path: &[Step],
        ancestors: &HashSet<CellId>,
        location: Location<'_>,
        avail: f64,
        hooks: &Hooks<C>,
    ) -> Node<P> {
        match location.value(|cell| cx.sources.value(cell)) {
            Some(value) => match location.field().and_then(|field| {
                self.try_evaluate(field, value, |cell| cx.sources.value(cell).cloned())
            }) {
                Some(result) => evaluation_projection(
                    cx,
                    tcx,
                    path,
                    ancestors,
                    value,
                    result,
                    avail,
                    hooks,
                    self,
                ),
                None => project_present_value(
                    cx,
                    tcx,
                    path,
                    ancestors,
                    value,
                    avail,
                    hooks,
                    self,
                ),
            },
            None => pending_view(cx, tcx, path.to_vec(), hooks),
        }
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
    projection: projection::Projection<'_>,
) -> Node<P> {
    let editing = cx
        .selection
        .filter(|selection| selection.path() == path)
        .and_then(Selection::edit);
    let projected = projected_value_view(
        cx,
        tcx,
        path,
        value,
        hooks,
        editing,
        projection,
    );
    let inner = match projected {
        Some(projected) => projected,
        None => raw_value_view(cx, tcx, path, ancestors, value, avail, hooks, projection),
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
    let placed = descend_landmark(cx, path.to_vec(), inner);
    ground(cx, path, value, placed)
}

/// The total fallback: structural projection for every graph value.
/// Its children re-enter [`descend`], so partial projections are
/// considered again at every descent.
#[allow(clippy::too_many_arguments)]
fn raw_value_view<
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
    projection: projection::Projection<'_>,
) -> Node<P> {
    match value {
        Value::Blob(bytes) => select_target(
            path.to_vec(),
            value.clone(),
            hooks,
            text(tcx, &blob_text(bytes), &cx.styles.id),
        ),
        Value::Cell(cell) => cell_view(cx, tcx, path, ancestors, *cell, avail, hooks, projection),
        Value::List(elements) => {
            list_view(cx, tcx, path, ancestors, elements, avail, hooks, projection)
        }
        Value::Record(fields) => {
            record_view(cx, tcx, path, ancestors, fields, avail, hooks, projection)
        }
    }
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
) -> Node<P> {
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
) -> Node<P> {
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
) -> Node<P> {
    let entries = completion_entries(&cx.sources, cx.names, cx.raw, labels, query.text());
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
) -> Node<P> {
    let scale = styles.scale;
    let choice = popup.choice.min(popup.entries.len().saturating_sub(1));
    // Cells first, so rows can pad out to the widest and the chosen
    // highlight spans the card, not just its own content.
    let cells: Vec<(Node<P>, Option<Node<P>>)> = popup
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
    let rows: Vec<Node<P>> = cells
        .into_iter()
        .zip(widths)
        .enumerate()
        .map(|(index, ((display, detail), width))| {
            let mut cells: Vec<Node<P>> = vec![display];
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
) -> Node<P> {
    if matches.is_empty() {
        return text(tcx, s, style);
    }
    let bold = TextStyle {
        weight: Some(700.0),
        ..style.clone()
    };
    let mut segments: Vec<Node<P>> = Vec::new();
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
    fallback: Node<P>,
    presentation: LineEditPresentation,
    placeholder: Option<(&str, &TextStyle)>,
    tcx: &mut TextCtx,
    styles: &Styles,
    hooks: &Hooks<C>,
) -> Node<P> {
    match editing {
        Some(line) => {
            let edit_ctx = hooks.edit.clone();
            text_edit(
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
    content: Node<P>,
) -> Node<P> {
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
    content: Node<P>,
) -> Node<P> {
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
    content: Node<P>,
) -> Node<P> {
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
) -> Node<P> {
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
    layout: Layout<Brush>,
    hooks: &Hooks<C>,
    content: Node<P>,
) -> Node<P> {
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
    content: Node<P>,
) -> Node<P> {
    let claimed = hover_target(path.clone(), content);
    quiet_select_target(path, value, hooks, claimed)
}

/// Name the value at `path` for the pointer over this ink, adding no
/// click of its own — the hover half of [`select_target`], and the
/// flat literal's delimiter dress.
fn hover_target<P: Canvas + HasHover<HoverClaim>>(path: Path, content: Node<P>) -> Node<P> {
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
    content: Node<P>,
) -> Node<P> {
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
    content: Node<P>,
) -> Node<P> {
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
                    };
                    select(ctx, path.clone(), Some(click));
                    true
                }
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ui_events::keyboard::{KeyState, Modifiers};

    struct EmptyClipboard;

    impl puri::edit::TextClipboard for EmptyClipboard {
        fn get_text(&mut self) -> Option<String> {
            None
        }

        fn set_text(&mut self, _: &str) {}
    }

    fn src<'a>(doc: &'a Document, library: &'a Cells) -> Sources<'a> {
        Sources { doc, library }
    }

    fn key(s: &str) -> Step {
        Step::Key(crate::test_values::label(s))
    }

    /// A one-cell document: the root links a cell holding `fields`.
    fn doc_of(fields: Vec<(CellId, Value)>) -> (Document, CellId) {
        let mut cells = Cells::new();
        let cell = new_cell_id();
        cells.set_value(cell, Value::record(fields));
        (
            Document {
                root: Some(Value::from(cell)),
                cells,
            },
            cell,
        )
    }

    /// The ordered positions of a list value's elements.
    fn positions(value: &Value) -> Vec<Position> {
        value.as_list().unwrap().keys().cloned().collect()
    }

    const LINE: f64 = 16.0;

    fn stop(path: Vec<Step>, x0: f64, y0: f64, x1: f64, y1: f64) -> Descend {
        Descend {
            path,
            rect: Rect::new(x0, y0, x1, y1),
        }
    }

    fn arrow(named: NamedKey) -> KeyboardEvent {
        KeyboardEvent {
            key: Key::Named(named),
            state: KeyState::Down,
            modifiers: Modifiers::empty(),
            ..Default::default()
        }
    }

    fn stepped(ds: &[Descend], from: Option<Vec<Step>>, named: NamedKey) -> Option<Path> {
        let selection = from.map(|path| Selection::Edge {
            path,
            edit: None,
            recorded: false,
        });
        step_selection(ds, selection.as_ref(), LINE, &arrow(named))
    }

    #[test]
    fn arrows_walk_rows_down_and_lines_across() {
        // A block record: field `a` hugs a flat record on line one,
        // field `b` drops a two-row block, field `e` closes. Settled
        // in placement order — children before parents.
        let a = || vec![key("a")];
        let a1 = || vec![key("a"), key("p")];
        let a2 = || vec![key("a"), key("q")];
        let b = || vec![key("b")];
        let c = || vec![key("b"), key("c")];
        let d = || vec![key("b"), key("d")];
        let e = || vec![key("e")];
        let ds = vec![
            stop(a1(), 60.0, 2.0, 80.0, 18.0),
            stop(a2(), 100.0, 2.0, 120.0, 18.0),
            stop(a(), 40.0, 2.0, 140.0, 18.0),
            stop(c(), 60.0, 24.0, 80.0, 40.0),
            stop(d(), 60.0, 44.0, 80.0, 60.0),
            stop(b(), 20.0, 22.0, 280.0, 62.0),
            stop(e(), 40.0, 66.0, 80.0, 82.0),
            stop(vec![], 0.0, 0.0, 300.0, 84.0),
        ];
        // Nothing selected: any arrow lands on the root.
        assert_eq!(stepped(&ds, None, NamedKey::ArrowDown), Some(vec![]));
        // Down walks every row in reading order, entering the open
        // block; up reverses it exactly.
        let rows = [vec![], a(), b(), c(), d(), e()];
        for pair in rows.windows(2) {
            let (above, below) = (&pair[0], &pair[1]);
            assert_eq!(
                stepped(&ds, Some(above.clone()), NamedKey::ArrowDown),
                Some(below.clone())
            );
            assert_eq!(
                stepped(&ds, Some(below.clone()), NamedKey::ArrowUp),
                Some(above.clone())
            );
        }
        assert_eq!(stepped(&ds, Some(e()), NamedKey::ArrowDown), None);
        assert_eq!(stepped(&ds, Some(vec![]), NamedKey::ArrowUp), None);
        // Right walks the hugged line's content; the walk ends with
        // the line, and left retraces it back out to the row.
        assert_eq!(stepped(&ds, Some(a()), NamedKey::ArrowRight), Some(a1()));
        assert_eq!(stepped(&ds, Some(a1()), NamedKey::ArrowRight), Some(a2()));
        assert_eq!(stepped(&ds, Some(a2()), NamedKey::ArrowRight), None);
        assert_eq!(stepped(&ds, Some(a2()), NamedKey::ArrowLeft), Some(a1()));
        assert_eq!(stepped(&ds, Some(a1()), NamedKey::ArrowLeft), Some(a()));
        // Left from a row widens to the parent; the root has none.
        assert_eq!(stepped(&ds, Some(a()), NamedKey::ArrowLeft), Some(vec![]));
        assert_eq!(stepped(&ds, Some(vec![]), NamedKey::ArrowLeft), None);
        // Mid-line, down exits to the next row and up collects to the
        // line's own stop.
        assert_eq!(stepped(&ds, Some(a2()), NamedKey::ArrowDown), Some(b()));
        assert_eq!(stepped(&ds, Some(a1()), NamedKey::ArrowUp), Some(a()));
        // A dropped block is entered by down, never right.
        assert_eq!(stepped(&ds, Some(b()), NamedKey::ArrowRight), None);
    }

    #[test]
    fn the_cell_head_rides_its_first_line() {
        let name = || {
            vec![
                Step::Follow,
                Step::Key(progred_name::vocabulary::NAME),
            ]
        };
        // Dropped: the projected name shares the cell's head line while the
        // value opens a row below it.
        let f = || vec![Step::Follow, key("f")];
        let ds = vec![
            stop(name(), 0.0, 2.0, 60.0, 18.0),
            stop(f(), 30.0, 24.0, 100.0, 40.0),
            stop(vec![Step::Follow], 20.0, 22.0, 180.0, 48.0),
            stop(vec![], 0.0, 0.0, 200.0, 50.0),
        ];
        assert_eq!(
            stepped(&ds, Some(vec![]), NamedKey::ArrowRight),
            Some(name())
        );
        assert_eq!(
            stepped(&ds, Some(vec![]), NamedKey::ArrowDown),
            Some(vec![Step::Follow])
        );
        assert_eq!(
            stepped(&ds, Some(vec![Step::Follow]), NamedKey::ArrowDown),
            Some(f())
        );
        assert_eq!(
            stepped(&ds, Some(name()), NamedKey::ArrowLeft),
            Some(vec![])
        );
        // Hugged: head and value share the one line; there is no row
        // below, only the line to walk.
        let ds = vec![
            stop(name(), 0.0, 2.0, 60.0, 18.0),
            stop(vec![Step::Follow], 70.0, 2.0, 150.0, 18.0),
            stop(vec![], 0.0, 0.0, 160.0, 20.0),
        ];
        assert_eq!(stepped(&ds, Some(vec![]), NamedKey::ArrowDown), None);
        assert_eq!(
            stepped(&ds, Some(vec![]), NamedKey::ArrowRight),
            Some(name())
        );
        assert_eq!(
            stepped(&ds, Some(name()), NamedKey::ArrowRight),
            Some(vec![Step::Follow])
        );
        assert_eq!(
            stepped(&ds, Some(vec![Step::Follow]), NamedKey::ArrowRight),
            None
        );
    }

    #[test]
    fn navigation_declines_modified_keys_releases_and_other_keys() {
        let ds = vec![
            stop(vec![key("a")], 0.0, 2.0, 60.0, 18.0),
            stop(vec![], 0.0, 0.0, 300.0, 40.0),
        ];
        let shifted = KeyboardEvent {
            modifiers: Modifiers::SHIFT,
            ..arrow(NamedKey::ArrowDown)
        };
        assert!(step_selection(&ds, None, LINE, &shifted).is_none());
        let released = KeyboardEvent {
            state: KeyState::Up,
            ..arrow(NamedKey::ArrowDown)
        };
        assert!(step_selection(&ds, None, LINE, &released).is_none());
        assert!(step_selection(&ds, None, LINE, &arrow(NamedKey::Escape)).is_none());
    }

    #[test]
    fn set_collapse_is_directional_and_stays_sparse() {
        let lib = Cells::new();
        let (doc, _) = doc_of(vec![(
            crate::test_values::label("a"),
            crate::test_values::text("1"),
        )]);
        let sources = src(&doc, &lib);
        let mut collapse = Collapse::default();
        assert!(set_collapse(&sources, &mut collapse, &[], true));
        assert!(!set_collapse(&sources, &mut collapse, &[], true));
        assert!(set_collapse(&sources, &mut collapse, &[], false));
        assert!(!set_collapse(&sources, &mut collapse, &[], false));
        // Matching the default stores nothing.
        assert!(collapse.overrides.is_empty());
        // A leaf has nothing to fold.
        let leaf = vec![Step::Follow, key("a")];
        assert!(!set_collapse(&sources, &mut collapse, &leaf, true));
    }

    #[test]
    fn selecting_text_brings_an_editor() {
        let lib = Cells::new();
        let (mut doc, cell) = doc_of(vec![
            (
                crate::test_values::label("name"),
                crate::test_values::text("old"),
            ),
            (
                crate::test_values::label("x"),
                crate::test_values::text("1.5"),
            ),
        ]);
        let at = |doc: &Document, path: Vec<Step>| Selection::edge(&src(doc, &lib), path);
        assert!(at(&doc, vec![Step::Follow, key("name")]).edit().is_some());
        assert!(at(&doc, vec![Step::Follow, key("x")]).edit().is_some());
        // Missing fields, links, and blobs carry no editor.
        assert!(
            at(&doc, vec![Step::Follow, key("missing")])
                .edit()
                .is_none()
        );
        assert!(at(&doc, vec![]).edit().is_none());
        doc.cells.set_value(
            cell,
            Value::record([(crate::test_values::label("b"), Value::from(vec![0xff_u8]))]),
        );
        assert!(at(&doc, vec![Step::Follow, key("b")]).edit().is_none());
        // A cell holding text edits at its Follow path.
        doc.cells.set_value(cell, crate::test_values::text("held"));
        assert!(at(&doc, vec![Step::Follow]).edit().is_some());
        // A simple name convention is just another text field.
        doc.cells
            .set_value(cell, progred_name::record("roof", []));
        assert!(
            at(
                &doc,
                vec![
                    Step::Follow,
                    Step::Key(progred_name::vocabulary::NAME),
                ],
            )
            .edit()
            .is_some()
        );
    }

    #[test]
    fn edits_write_through_to_the_field() {
        let lib = Cells::new();
        let (mut doc, _) = doc_of(vec![(
            crate::test_values::label("name"),
            crate::test_values::text("old"),
        )]);
        let path = vec![Step::Follow, key("name")];
        let mut selection = Selection::edge(&src(&doc, &lib), path.clone());
        selection.edit_mut().unwrap().set_text("new");
        write_through(&mut doc, &lib, &mut selection);
        assert_eq!(
            src(&doc, &lib).resolve(&path),
            Some(&crate::test_values::text("new"))
        );
        // A selection without an editor writes nothing.
        let mut plain = Selection::edge(&src(&doc, &lib), vec![Step::Follow, key("missing")]);
        assert!(!write_through(&mut doc, &lib, &mut plain));
        assert_eq!(
            src(&doc, &lib).resolve(&path),
            Some(&crate::test_values::text("new"))
        );
    }

    #[test]
    fn compact_f64_values_edit_as_decimal_text() {
        let lib = Cells::new();
        let cell = new_cell_id();
        let mut cells = Cells::new();
        cells.set_value(cell, grap_f64::value(2.5));
        let mut doc = Document {
            root: Some(Value::from(cell)),
            cells,
        };
        let path = vec![Step::Follow];
        let mut selection = Selection::edge(&src(&doc, &lib), path.clone());
        assert_eq!(selection.edit().map(LineEditState::text), Some("2.5"));
        selection.edit_mut().unwrap().set_text("7.25");
        assert!(write_through(&mut doc, &lib, &mut selection));
        assert_eq!(
            src(&doc, &lib).resolve(&path).and_then(grap_f64::read),
            Some(7.25)
        );

        selection.edit_mut().unwrap().set_text("not a number");
        assert!(!write_through(&mut doc, &lib, &mut selection));
        assert_eq!(
            src(&doc, &lib).resolve(&path).and_then(grap_f64::read),
            Some(7.25)
        );
    }

    #[test]
    fn element_edits_rebuild_the_list_at_the_owning_cell() {
        let lib = Cells::new();
        let (mut doc, _) = doc_of(vec![(
            crate::test_values::label("dash"),
            Value::list([crate::test_values::text("2"), crate::test_values::text("3")]),
        )]);
        let list_path = vec![Step::Follow, key("dash")];
        let ps = positions(src(&doc, &lib).resolve(&list_path).unwrap());
        let element = vec![Step::Follow, key("dash"), Step::Element(ps[1].clone())];

        // Editing an element writes the whole rebuilt list at the
        // owning cell; the sibling keeps its position and value.
        let mut selection = Selection::edge(&src(&doc, &lib), element.clone());
        selection.edit_mut().unwrap().set_text("9");
        assert!(write_through(&mut doc, &lib, &mut selection));
        assert_eq!(
            src(&doc, &lib).resolve(&element),
            Some(&crate::test_values::text("9"))
        );
        assert_eq!(
            src(&doc, &lib).resolve(&list_path),
            Some(&Value::list([
                crate::test_values::text("2"),
                crate::test_values::text("9")
            ]))
        );
        assert_eq!(positions(src(&doc, &lib).resolve(&list_path).unwrap()), ps);
    }

    #[test]
    fn set_value_writes_fields_elements_roots_and_bare_cells() {
        let lib = Cells::new();
        let (mut doc, cell) = doc_of(vec![(
            crate::test_values::label("x"),
            crate::test_values::text("1"),
        )]);
        assert!(set_value(
            &mut doc,
            &lib,
            &[Step::Follow, key("x")],
            crate::test_values::text("2")
        ));
        assert_eq!(
            src(&doc, &lib).resolve(&[Step::Follow, key("x")]),
            Some(&crate::test_values::text("2"))
        );

        // A fresh Key step INSERTS a field; a deep spine rebuilds
        // through nested records and lists.
        assert!(set_value(
            &mut doc,
            &lib,
            &[Step::Follow, key("at")],
            Value::record([(
                crate::test_values::label("row"),
                crate::test_values::text("top")
            )]),
        ));
        assert!(set_value(
            &mut doc,
            &lib,
            &[Step::Follow, key("at"), key("row")],
            crate::test_values::text("bottom")
        ));
        assert_eq!(
            src(&doc, &lib).resolve(&[Step::Follow, key("at"), key("row")]),
            Some(&crate::test_values::text("bottom"))
        );

        // The whole cell value is addressable at Follow: conversion
        // is one set.
        assert!(set_value(
            &mut doc,
            &lib,
            &[Step::Follow],
            Value::list([crate::test_values::text("a")])
        ));
        assert_eq!(
            src(&doc, &lib).resolve(&[Step::Follow]),
            Some(&Value::list([crate::test_values::text("a")]))
        );

        // A bare cell takes its first value through the empty spine;
        // deeper steps into nothing decline.
        let bare = new_cell_id();
        doc.root = Some(Value::from(bare));
        assert!(!set_value(
            &mut doc,
            &lib,
            &[Step::Follow, key("x")],
            crate::test_values::text("v")
        ));
        assert!(set_value(
            &mut doc,
            &lib,
            &[Step::Follow],
            Value::record([(
                crate::test_values::label("x"),
                crate::test_values::text("v")
            )])
        ));
        assert_eq!(
            src(&doc, &lib).resolve(&[Step::Follow, key("x")]),
            Some(&crate::test_values::text("v"))
        );

        // An inline record at the root writes on the root spine — no
        // cell involved.
        doc.root = Some(Value::record([(
            crate::test_values::label("shape"),
            Value::from(cell),
        )]));
        assert!(set_value(
            &mut doc,
            &lib,
            &[key("title")],
            crate::test_values::text("scene")
        ));
        assert_eq!(
            src(&doc, &lib).resolve(&[key("title")]),
            Some(&crate::test_values::text("scene"))
        );
        assert!(set_value(
            &mut doc,
            &lib,
            &[],
            crate::test_values::text("root")
        ));
        assert_eq!(doc.root, Some(crate::test_values::text("root")));
        // Text is a record convention, so a structural write can
        // enrich it. The extra field then keeps the compact text
        // projection from hiding structure.
        assert!(set_value(
            &mut doc,
            &lib,
            &[key("x")],
            crate::test_values::text("0")
        ));
        assert_eq!(
            doc.root
                .as_ref()
                .and_then(Value::as_record)
                .and_then(|fields| fields.get(&crate::test_values::label("x"))),
            Some(&crate::test_values::text("0"))
        );
        assert!(projection::whole_text(doc.root.as_ref().unwrap()).is_none());
    }

    #[test]
    fn external_cells_decline_writes_and_bare_cells_accept() {
        let mut lib = Cells::new();
        let lib_cell = new_cell_id();
        lib.set_value(
            lib_cell,
            progred_name::record(
                "convention",
                [(
                    crate::test_values::label("a"),
                    crate::test_values::text("1"),
                )],
            ),
        );
        let mut doc = Document {
            root: Some(Value::from(lib_cell)),
            cells: Cells::new(),
        };
        // The library's cell declines writes wholesale.
        assert!(!set_value(
            &mut doc,
            &lib,
            &[Step::Follow, key("a")],
            crate::test_values::text("2")
        ));
        assert!(!set_value(
            &mut doc,
            &lib,
            &[
                Step::Follow,
                Step::Key(progred_name::vocabulary::NAME),
            ],
            crate::test_values::text("mine")
        ));
        assert!(!delete_edge(&mut doc, &lib, &[Step::Follow, key("a")]));
        assert!(!delete_edge(&mut doc, &lib, &[Step::Follow]));
        // Forking — the document taking the cell over — writes.
        doc.cells.set_value(
            lib_cell,
            Value::record([(
                crate::test_values::label("a"),
                crate::test_values::text("1"),
            )]),
        );
        assert!(set_value(
            &mut doc,
            &lib,
            &[Step::Follow, key("a")],
            crate::test_values::text("2")
        ));
    }

    #[test]
    fn write_through_opens_one_step_per_editor_life() {
        let lib = Cells::new();
        let (mut doc, _) = doc_of(vec![(
            crate::test_values::label("name"),
            crate::test_values::text("a"),
        )]);
        let path = vec![Step::Follow, key("name")];
        let mut selection = Selection::edge(&src(&doc, &lib), path);

        // First write opens the step; the rest of the run is silent,
        // as are no-op rewrites.
        selection.edit_mut().unwrap().set_text("ab");
        assert!(write_through(&mut doc, &lib, &mut selection));
        selection.edit_mut().unwrap().set_text("abc");
        assert!(!write_through(&mut doc, &lib, &mut selection));
        assert!(!write_through(&mut doc, &lib, &mut selection));

        // Breaking the run (a save) makes the next write a new step.
        break_edit_run(Some(&mut selection));
        selection.edit_mut().unwrap().set_text("abcd");
        assert!(write_through(&mut doc, &lib, &mut selection));

        // A re-minted editor is a new run by construction.
        let mut fresh = Selection::edge(&src(&doc, &lib), vec![Step::Follow, key("name")]);
        fresh.edit_mut().unwrap().set_text("x");
        assert!(write_through(&mut doc, &lib, &mut fresh));
    }

    #[test]
    fn delete_unlinks_fields_and_elements_and_bares_cells() {
        let lib = Cells::new();
        let child = new_cell_id();
        let (mut doc, cell) = doc_of(vec![
            (crate::test_values::label("child"), Value::from(child)),
            (
                crate::test_values::label("dash"),
                Value::list([crate::test_values::text("2"), crate::test_values::text("3")]),
            ),
        ]);
        doc.cells
            .set_value(child, progred_name::record("c", []));

        assert!(!delete_edge(
            &mut doc,
            &lib,
            &[Step::Follow, key("missing")]
        ));

        // Unlinking a field drops the link; the linked cell floats in
        // the table for the orphan pool.
        assert!(delete_edge(&mut doc, &lib, &[Step::Follow, key("child")]));
        assert_eq!(src(&doc, &lib).resolve(&[Step::Follow, key("child")]), None);
        assert!(doc.cells.value(child).is_some());

        // An element step rebuilds the list without it.
        let dash = vec![Step::Follow, key("dash")];
        let ps = positions(src(&doc, &lib).resolve(&dash).unwrap());
        assert!(delete_edge(
            &mut doc,
            &lib,
            &[Step::Follow, key("dash"), Step::Element(ps[0].clone())]
        ));
        assert_eq!(
            src(&doc, &lib).resolve(&dash),
            Some(&Value::list([crate::test_values::text("3")]))
        );

        // A trailing Follow removes the cell's value: valueless
        // again, and a second delete declines.
        assert!(delete_edge(&mut doc, &lib, &[Step::Follow]));
        assert!(doc.cells.value(cell).is_none());
        assert!(src(&doc, &lib).resolve(&[]).is_some());
        assert!(!delete_edge(&mut doc, &lib, &[Step::Follow]));

        // The empty path empties the document.
        assert!(delete_edge(&mut doc, &lib, &[]));
        assert!(doc.root.is_none());
        assert!(!delete_edge(&mut doc, &lib, &[]));
    }

    #[test]
    fn pendings_normalize_through_links_and_gate_on_authority() {
        let mut lib = Cells::new();
        let lib_cell = new_cell_id();
        lib.set_value(
            lib_cell,
            Value::record([(
                crate::test_values::label("a"),
                crate::test_values::text("1"),
            )]),
        );
        let bare = new_cell_id();
        let (mut doc, _) = doc_of(vec![
            (crate::test_values::label("at"), Value::record([])),
            (
                crate::test_values::label("tags"),
                Value::list([crate::test_values::text("x")]),
            ),
            (crate::test_values::label("lib"), Value::from(lib_cell)),
            (crate::test_values::label("material"), Value::from(bare)),
            (
                crate::test_values::label("s"),
                crate::test_values::text("leaf"),
            ),
        ]);
        doc.root = doc.root.clone();
        let sources = src(&doc, &lib);

        // A link to a record cell pends its field under Follow; an
        // inline record pends at its own path.
        let on_cell = pending_edge(&sources, vec![]).unwrap();
        assert_eq!(on_cell.path(), &[Step::Follow]);
        let inline = pending_edge(&sources, vec![Step::Follow, key("at")]).unwrap();
        assert_eq!(inline.path(), &[Step::Follow, key("at")]);

        // Lists, external cells, and bare cells decline fields. Text
        // is a record convention, so adding a field enriches it and
        // makes its structure visible.
        assert!(pending_edge(&sources, vec![Step::Follow, key("tags")]).is_none());
        assert!(pending_edge(&sources, vec![Step::Follow, key("s")]).is_some());
        assert!(pending_edge(&sources, vec![Step::Follow, key("lib")]).is_none());
        assert!(pending_edge(&sources, vec![Step::Follow, key("material")]).is_none());

        // A bare cell pends its first value at Follow — the
        // within-gesture's meaning there.
        let filling = pending_follow(&sources, &[Step::Follow, key("material")]).unwrap();
        assert_eq!(
            filling.path(),
            &[Step::Follow, key("material"), Step::Follow]
        );
        assert!(pending_follow(&sources, &[Step::Follow, key("lib")]).is_none());
        assert!(pending_follow(&sources, &[Step::Follow, key("at")]).is_none());

        // Into a list through its link, appended at the end.
        let into = pending_into(&sources, &[Step::Follow, key("tags")]).unwrap();
        assert!(matches!(into.path().last(), Some(Step::Element(_))));
        assert_eq!(into.path().len(), 3);

        // The within chord: fields on records, elements into lists,
        // first values into bare cells.
        assert!(pending_insert(&sources, &[], false).is_some());
        assert!(pending_insert(&sources, &[Step::Follow, key("tags")], false).is_some());
        assert!(pending_insert(&sources, &[Step::Follow, key("material")], false).is_some());
        assert!(pending_insert(&sources, &[Step::Follow, key("s")], false).is_some());
    }

    #[test]
    fn queries_resolve_text_and_blobs() {
        assert_eq!(resolve_query("hello"), crate::test_values::text("hello"));
        assert_eq!(
            resolve_query("\"quoted\""),
            crate::test_values::text("quoted")
        );
        assert_eq!(resolve_query("\"open"), crate::test_values::text("open"));
        assert_eq!(resolve_query("\"0xff\""), crate::test_values::text("0xff"));
        assert_eq!(resolve_query("0xff00"), Value::from(vec![0xff, 0x00]));
        // Input is case-tolerant — the value is the bytes, lowercase
        // just the canonical spelling — and whole bytes only.
        assert_eq!(resolve_query("0xDEad"), Value::from(vec![0xde, 0xad]));
        assert_eq!(resolve_query("0xf"), crate::test_values::text("0xf"));
        assert_eq!(resolve_query("0x"), Value::from(vec![]));
    }

    #[test]
    fn clipboard_spellings_round_trip() {
        let cell = new_cell_id();
        // Atoms are text and round-trip through it; structure rides
        // the private format and round-trips through its bytes.
        let atoms = [
            crate::test_values::text("plain"),
            crate::test_values::text("\"tricky\""),
            Value::from(vec![0xde, 0xad]),
        ];
        for value in atoms {
            let (text, structural) = to_clipboard(&value);
            assert!(!structural);
            assert_eq!(from_clipboard(&text), value);
        }
        let structures = [
            Value::from(cell),
            Value::list([crate::test_values::text("a"), Value::from(cell)]),
            Value::record([(
                crate::test_values::label("x"),
                crate::test_values::text("1"),
            )]),
            Value::record([(cell, Value::from(vec![0x00_u8]))]),
        ];
        for value in structures {
            let (text, structural) = to_clipboard(&value);
            assert!(structural);
            assert_eq!(from_structure(text.as_bytes()), Some(value));
        }
        // Atoms read in other apps; alien text pastes sensibly — and
        // TEXT IS NEVER STRUCTURE: characters that happen to spell
        // Value JSON read as the string they are.
        assert_eq!(to_clipboard(&crate::test_values::text("hi")).0, "\"hi\"");
        assert_eq!(to_clipboard(&Value::from(vec![0xff_u8])).0, "0xff");
        assert_eq!(
            from_clipboard("loose text"),
            crate::test_values::text("loose text")
        );
        let spelled = to_clipboard(&Value::record([])).0;
        assert_eq!(from_clipboard(&spelled), crate::test_values::text(&spelled));
    }

    #[test]
    fn completion_offers_follow_the_stage() {
        let lib = crate::conventions::library();
        let (mut doc, cell) = doc_of(vec![
            progred_name::field("roof"),
            (
                crate::test_values::label("kind"),
                crate::test_values::text("building"),
            ),
        ]);
        let sources = src(&doc, &lib);
        let names = Names::convention();
        let displays = |labels: bool, query: &str| -> Vec<String> {
            completion_entries(&sources, &names, false, labels, query)
                .into_iter()
                .map(|entry| entry.display)
                .collect()
        };

        // The value stage offers the string, references, the value
        // constructors, and the mint.
        let value_stage = displays(false, "");
        assert!(value_stage.iter().any(|d| d == "roof"));
        assert!(value_stage.iter().any(|d| d == "new list"));
        assert!(value_stage.iter().any(|d| d == "new record"));
        assert!(value_stage.iter().any(|d| d == "new cell"));

        // The label stage narrows to what can label: no list, record,
        // or blob offers.
        let label_stage = displays(true, "");
        assert!(label_stage.iter().all(|d| d != "new list"));
        assert!(label_stage.iter().all(|d| d != "new record"));
        assert!(label_stage.iter().any(|d| d == "new cell"));
        let label_blob = completion_entries(&sources, &names, false, true, "0xff");
        assert_eq!(label_blob[0].display, "0xff");
        assert_eq!(label_blob[0].detail.as_deref(), Some("new label"));
        assert!(matches!(
            &label_blob[0].action,
            EntryAction::NewLabel(name) if name == "0xff"
        ));

        // A blob query leads with the blob, its string form below.
        let value_blob = displays(false, "0xff");
        assert_eq!(value_blob[0], "0xff");
        assert_eq!(value_blob[1], "\"0xff\"");

        // Reference commits are links.
        let roof = completion_entries(&sources, &names, false, false, "roof");
        assert!(matches!(
            &roof[0].action,
            EntryAction::Value(value) if value.as_cell() == Some(cell)
        ));
        let circle = completion_entries(&sources, &names, false, false, "circle");
        assert!(matches!(
            &circle[0].action,
            EntryAction::Value(value)
                if value.as_cell() == Some(grap_geometry::vocabulary::CIRCLE)
        ));

        // A bare id never outranks the typed text: the string the
        // query spells comes before every unnamed reference, however
        // exactly the id matches — ids are for reading; want it
        // reachable, name it.
        let unnamed = new_cell_id();
        doc.cells.set_value(unnamed, crate::test_values::text("x"));
        let sources = src(&doc, &lib);
        let entries = completion_entries(&sources, &names, false, false, &short_id(unnamed));
        let atom = entries
            .iter()
            .position(
                |e| matches!(&e.action, EntryAction::Value(v) if progred_text::read(v).is_some()),
            )
            .unwrap();
        let reference = entries
            .iter()
            .position(
                |e| matches!(&e.action, EntryAction::Value(v) if v.as_cell() == Some(unnamed)),
            )
            .unwrap();
        assert!(atom < reference);
    }

    #[test]
    fn cycles_collapse_by_default_and_expand_turn_by_turn() {
        // A: { next: A } — the re-entry at [Follow, next] repeats the
        // root value.
        let a = new_cell_id();
        let mut cells = Cells::new();
        cells.set_value(
            a,
            Value::record([(crate::test_values::label("next"), Value::from(a))]),
        );
        let doc = Document {
            root: Some(Value::from(a)),
            cells,
        };
        let lib = Cells::new();
        let sources = src(&doc, &lib);
        let mut collapse = Collapse::default();
        let reentry = vec![Step::Follow, key("next")];
        // Space's toggle expands the default-collapsed re-entry.
        assert!(toggle_collapse(&sources, &mut collapse, &reentry));
        assert!(!collapse.collapsed(&reentry, true));
        // The next turn defaults collapsed at its own deeper path and
        // expands the same way — follow the cycle as far as wanted.
        let deeper: Vec<Step> = reentry.iter().chain(reentry.iter()).cloned().collect();
        assert!(collapse.collapsed(&deeper, true));
        assert!(toggle_collapse(&sources, &mut collapse, &deeper));
        assert!(!collapse.collapsed(&deeper, true));
        // Toggling back restores the default (the override is sparse).
        assert!(toggle_collapse(&sources, &mut collapse, &deeper));
        assert!(collapse.collapsed(&deeper, true));
        assert!(collapse.overrides.is_empty() || !collapse.overrides.contains_key(&deeper));
    }

    #[test]
    fn any_valued_cell_and_any_container_collapse() {
        let lib = Cells::new();
        let (doc, _) = doc_of(vec![(
            crate::test_values::label("kind"),
            crate::test_values::text("building"),
        )]);
        let sources = src(&doc, &lib);
        let mut collapse = Collapse::default();
        // A plain (non-cycle) cell collapses to ( … ) via the same
        // toggle.
        assert!(toggle_collapse(&sources, &mut collapse, &[]));
        assert!(collapse.collapsed(&[], false));
        // Its record collapses too — layout never enters into it, so
        // inline literals toggle exactly like block forms.
        assert!(toggle_collapse(&sources, &mut collapse, &[Step::Follow]));
        assert!(collapse.collapsed(&[Step::Follow], false));
        // A valueless location declines.
        let empty = Document {
            root: None,
            cells: Cells::new(),
        };
        assert!(!toggle_collapse(
            &src(&empty, &lib),
            &mut collapse,
            &[] as &[Step]
        ));
    }

    #[test]
    fn minting_seeds_bare_and_named_cells() {
        // A mint is fully bare: a link with nothing said at all —
        // naming happens on the head afterward.
        let bare = resolve_entry(&EntryAction::NewCell);
        assert!(bare.unwrap().as_cell().is_some());
        // The value constructors commit pure values — nothing minted.
        assert_eq!(resolve_entry(&EntryAction::NewList), Some(Value::list([])));
        assert_eq!(
            resolve_entry(&EntryAction::NewRecord),
            Some(Value::record([]))
        );

        let (label, created) = resolve_label(&EntryAction::NewLabel("asdf".to_string())).unwrap();
        let (cell, value) = created.unwrap();
        assert_eq!(label, cell);
        assert_eq!(progred_name::read(&value), Some("asdf"));
        assert!(resolve_label(&EntryAction::Value(crate::test_values::text("no"))).is_none());
    }

    #[test]
    fn pending_rename_seeds_the_current_spelling() {
        let doc = sample_document();
        let lib = crate::conventions::library();
        let sources = src(&doc, &lib);
        // A cell label seeds its ordinary name — the spelling whose
        // first choice resolves back to the same identity, so committing
        // untouched is a no-op rename.
        let tags = vec![key("shape"), Step::Follow, key("tags")];
        let pending = pending_rename(&sources, &tags).unwrap();
        assert_eq!(pending.edit().unwrap().text(), "tags");
        let Selection::PendingEdge {
            parent, replacing, ..
        } = &pending
        else {
            panic!("a rename pends the edge");
        };
        assert_eq!(parent.as_slice(), &tags[..2]);
        assert_eq!(replacing.as_ref(), Some(&crate::test_values::label("tags")));
        // A cell label seeds by NAME — a spelling, not the identity;
        // another cell sharing the name may rank first, accepted.
        let roof = sources.resolve(&[key("shape")]).unwrap().as_cell().unwrap();
        let stroke = sources
            .value(roof)
            .unwrap()
            .as_record()
            .unwrap()
            .keys()
            .copied()
            .find(|cell| sources.value(*cell).and_then(progred_name::read) == Some("stroke"))
            .unwrap();
        let path = vec![key("shape"), Step::Follow, Step::Key(stroke)];
        assert_eq!(
            pending_rename(&sources, &path)
                .unwrap()
                .edit()
                .unwrap()
                .text(),
            "stroke"
        );
        // Missing fields have no label to re-open.
        assert!(pending_rename(&sources, &[key("gone")]).is_none());
    }

    #[test]
    fn entry_hover_marks_follow_the_live_query() {
        // The reported bug: hover an entry, keep the mouse still,
        // type — the mark must follow what the entry NOW is, not
        // what it was when the pointer arrived.
        let doc = sample_document();
        let lib = crate::conventions::library();
        let sources = src(&doc, &lib);
        let names = Names::convention();
        let pending = |text: &str| Selection::Pending {
            path: Vec::new(),
            query: line_edit(text),
            choice: 0,
        };
        // A quoted query leads with its atom, so entry zero IS the
        // typed string — and re-derives as the query grows.
        assert_eq!(
            hover_value(
                &sources,
                &names,
                false,
                Some(&pending("\"a\"")),
                &Hover::Entry(0)
            ),
            Some(crate::test_values::text("a"))
        );
        assert_eq!(
            hover_value(
                &sources,
                &names,
                false,
                Some(&pending("\"ab\"")),
                &Hover::Entry(0)
            ),
            Some(crate::test_values::text("ab"))
        );
        // Dead addresses answer nothing: a closed pending, a label
        // no longer in the document.
        assert_eq!(
            hover_value(&sources, &names, false, None, &Hover::Entry(0)),
            None
        );
        assert_eq!(
            hover_value(
                &sources,
                &names,
                false,
                None,
                &Hover::Label(vec![key("gone")])
            ),
            None
        );
        assert_eq!(
            hover_value(
                &sources,
                &names,
                false,
                None,
                &Hover::Label(vec![key("shape"), Step::Follow, key("tags")])
            ),
            Some(Value::from(sample_vocabulary::TAGS))
        );
    }

    #[test]
    fn a_mounting_click_can_place_the_rename_caret() {
        // The caret index is hit-tested against the label's OWN
        // layout — the text that was clicked, in its face — and lands
        // in the seed by byte index, so nothing depends on the
        // editor's font agreeing with the label's (short ids draw
        // monospace; the editor draws system-ui).
        let doc = sample_document();
        let lib = crate::conventions::library();
        let sources = src(&doc, &lib);
        let styles = Styles::new(1.0);
        let mut fonts = parley::FontContext::new();
        let mut layouts = parley::LayoutContext::new();
        let mut cache = puri::text::TextCache::default();
        let layout = line_layout(
            &mut TextCtx {
                fonts: &mut fonts,
                layouts: &mut layouts,
                scale: 1.0,
                cache: &mut cache,
            },
            "tags",
            &styles.label,
        );
        let z = KeyboardEvent {
            key: Key::Character("z".into()),
            modifiers: Modifiers::empty(),
            state: KeyState::Down,
            ..Default::default()
        };
        let tags = vec![key("shape"), Step::Follow, key("tags")];
        let presentation = edit_presentation(&styles.label);
        let mut clipboard = EmptyClipboard;
        // A click at the label's left edge prepends, where an
        // unclicked mount appends...
        let mut pending = pending_rename(&sources, &tags).unwrap();
        let edit = pending.edit_mut().unwrap();
        edit.cursor_to(caret_index(&layout, Point::ZERO));
        edit.handle_key(&presentation, &mut fonts, &mut layouts, &mut clipboard, &z);
        assert_eq!(edit.text(), "ztags");
        // ...and one past the right edge still appends.
        let mut pending = pending_rename(&sources, &tags).unwrap();
        let edit = pending.edit_mut().unwrap();
        edit.cursor_to(caret_index(&layout, Point::new(10_000.0, 7.0)));
        edit.handle_key(&presentation, &mut fonts, &mut layouts, &mut clipboard, &z);
        assert_eq!(edit.text(), "tagsz");
    }

    #[test]
    fn rename_carries_the_value_and_never_a_sibling() {
        let lib = Cells::new();
        let (mut doc, _cell) = doc_of(vec![
            (
                crate::test_values::label("a"),
                crate::test_values::text("1"),
            ),
            (
                crate::test_values::label("b"),
                crate::test_values::text("2"),
            ),
        ]);
        let parent = vec![Step::Follow];
        // A taken label declines whole: the sibling keeps its value.
        assert!(!rename_field(
            &mut doc,
            &lib,
            &parent,
            &crate::test_values::label("a"),
            crate::test_values::label("b")
        ));
        // A fresh label re-keys in one write, the value carried.
        assert!(rename_field(
            &mut doc,
            &lib,
            &parent,
            &crate::test_values::label("a"),
            crate::test_values::label("c")
        ));
        {
            let sources = src(&doc, &lib);
            assert_eq!(
                sources.resolve(&[Step::Follow, key("c")]).cloned(),
                Some(crate::test_values::text("1"))
            );
            assert!(sources.resolve(&[Step::Follow, key("a")]).is_none());
            assert_eq!(
                sources.resolve(&[Step::Follow, key("b")]).cloned(),
                Some(crate::test_values::text("2"))
            );
        }
        // A missing field has nothing to carry.
        assert!(!rename_field(
            &mut doc,
            &lib,
            &parent,
            &crate::test_values::label("gone"),
            crate::test_values::label("d")
        ));
    }

    #[test]
    fn the_sample_document_shows_the_constructs() {
        let doc = sample_document();
        let lib = crate::conventions::library();
        let sources = src(&doc, &lib);
        // The root is an inline record of roles.
        assert!(doc.root.as_ref().unwrap().as_record().is_some());
        let roof = sources.resolve(&[key("shape")]).unwrap().as_cell().unwrap();
        assert_eq!(
            sources.value(roof).and_then(progred_name::read),
            Some("roof")
        );
        // The material cell is referenced and fully bare.
        let material = sources
            .resolve(&[key("shape"), Step::Follow, key("material")])
            .unwrap()
            .as_cell()
            .unwrap();
        assert!(sources.value(material).is_none());
        // The stroke cell is a name-only ordinary record, referenced
        // as a label.
        let stroke = sources
            .value(roof)
            .unwrap()
            .as_record()
            .unwrap()
            .keys()
            .copied()
            .find(|cell| sources.value(*cell).and_then(progred_name::read) == Some("stroke"))
            .unwrap();
        assert_eq!(
            sources.value(stroke).and_then(progred_name::read),
            Some("stroke")
        );
        // The style cell is shared by the root and the roof.
        assert_eq!(
            sources.resolve(&[key("style")]),
            sources.resolve(&[key("shape"), Step::Follow, key("style")])
        );
        // Points hold inline records; the swatch is a blob.
        let points = sources
            .resolve(&[key("shape"), Step::Follow, key("points")])
            .unwrap();
        let origin = points
            .as_list()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .as_cell()
            .unwrap();
        assert!(matches!(
            sources
                .value(origin)
                .and_then(|value| value.as_record())
                .and_then(|fields| fields.get(&crate::test_values::label("at"))),
            Some(Value::Record(_))
        ));
        assert!(
            sources
                .resolve(&[key("style"), Step::Follow, key("swatch")])
                .unwrap()
                .as_blob()
                .is_some()
        );
        // Documents round trip with convention data unchanged.
        let json = serde_json::to_string(&doc).unwrap();
        let loaded: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.root, doc.root);
        assert_eq!(
            loaded.cells.value(roof).and_then(progred_name::read),
            Some("roof")
        );
        assert_eq!(serde_json::to_string(&loaded).unwrap(), json);
    }

    #[test]
    fn selecting_an_empty_value_slot_pends() {
        let mut lib = Cells::new();
        let bare = new_cell_id();
        let mut doc = Document {
            root: Some(Value::from(bare)),
            cells: Cells::new(),
        };
        // A writable valueless cell's Follow slot is already
        // authoring: selecting it (the rendered placeholder) pends.
        assert!(matches!(
            Selection::edge(&src(&doc, &lib), vec![Step::Follow]),
            Selection::Pending { .. }
        ));
        // Valued, it selects normally.
        doc.cells.set_value(bare, crate::test_values::text("v"));
        assert!(matches!(
            Selection::edge(&src(&doc, &lib), vec![Step::Follow]),
            Selection::Edge { .. }
        ));
        // An EXTERNAL cell has an ordinary value, so its Follow slot
        // selects normally and remains unwritable.
        let lib_cell = new_cell_id();
        lib.set_value(lib_cell, progred_name::record("convention", []));
        doc.root = Some(Value::from(lib_cell));
        assert!(matches!(
            Selection::edge(&src(&doc, &lib), vec![Step::Follow]),
            Selection::Edge { edit: None, .. }
        ));
        // The empty document's root is the same rule.
        let empty = Document {
            root: None,
            cells: Cells::new(),
        };
        assert!(matches!(
            Selection::edge(&src(&empty, &lib), vec![]),
            Selection::Pending { .. }
        ));
    }

    #[test]
    fn a_simple_name_is_an_ordinary_editable_field() {
        let lib = Cells::new();
        let (mut doc, cell) = doc_of(vec![
            progred_name::field("old"),
            (
                crate::test_values::label("x"),
                crate::test_values::text("1"),
            ),
        ]);
        let path = vec![
            Step::Follow,
            Step::Key(progred_name::vocabulary::NAME),
        ];

        let mut selection = Selection::edge(&src(&doc, &lib), path.clone());
        selection.edit_mut().unwrap().set_text("new");
        assert!(write_through(&mut doc, &lib, &mut selection));
        assert_eq!(
            doc.cells.value(cell).and_then(progred_name::read),
            Some("new")
        );
        assert!(!write_through(&mut doc, &lib, &mut selection));

        // Empty is an ordinary text value, not a hidden spelling of
        // field absence.
        selection.edit_mut().unwrap().set_text("");
        write_through(&mut doc, &lib, &mut selection);
        assert_eq!(doc.cells.value(cell).and_then(progred_name::read), None);
        assert_eq!(
            doc.cells
                .value(cell)
                .and_then(Value::as_record)
                .and_then(|fields| { fields.get(&progred_name::vocabulary::NAME) })
                .and_then(progred_text::read),
            Some("")
        );

        // Removing the field uses the same structural deletion as any
        // other record field; the rest of the value remains.
        assert!(delete_edge(&mut doc, &lib, &path));
        assert_eq!(doc.cells.value(cell).and_then(progred_name::read), None);
        assert!(
            doc.cells
                .value(cell)
                .and_then(Value::as_record)
                .is_some_and(|fields| fields.contains_key(&crate::test_values::label("x")))
        );
        assert!(Selection::edge(&src(&doc, &lib), path).edit().is_none());
    }
}

/// Headless visual bench: the sample document through the real
/// projection, written as an SVG — the qlmanage trick for the whole
/// editor frame, no window needed. `cargo test -p progred svg_bench`
/// writes target/raw_projection.svg.
#[cfg(test)]
mod svg_bench {
    use super::*;
    use puri::draw::{DrawCmd, DrawList, GlyphRun, Shape};
    use puri::handler::Handler;
    use skrifa::instance::{LocationRef, NormalizedCoord, Size};
    use skrifa::outline::{DrawSettings, OutlinePen};
    use skrifa::{FontRef, GlyphId, MetadataProvider};
    use std::fmt::Write as _;
    use vello::kurbo::{BezPath, Shape as KurboShape};

    type Claims = Vec<HoverClaim>;

    struct Bench {
        list: DrawList,
        handler: Handler<Claims>,
        descends: Vec<Descend>,
        popup: Option<Popup>,
        pointer: Option<Point>,
        hover_claims: Vec<HoverClaim>,
    }

    impl Canvas for Bench {
        fn fill(&mut self, shape: impl Into<Shape>, brush: impl Into<Brush>, transform: Affine) {
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
            let _ = (shape.into(), transform);
            content(self);
        }
    }

    impl HasHandler<Claims> for Bench {
        fn handler(&mut self) -> &mut Handler<Claims> {
            &mut self.handler
        }
    }

    impl HasDescends for Bench {
        fn descends(&mut self) -> &mut Vec<Descend> {
            &mut self.descends
        }
    }

    impl HasPopup for Bench {
        fn popup(&mut self) -> &mut Option<Popup> {
            &mut self.popup
        }
    }

    impl HasHover<HoverClaim> for Bench {
        fn pointer(&self) -> Option<Point> {
            self.pointer
        }

        fn claim_hover(&mut self, claim: HoverClaim) {
            self.hover_claims.push(claim);
        }
    }

    fn css(brush: &Brush) -> String {
        match brush {
            Brush::Solid(color) => {
                let [r, g, b, a] = color.components;
                format!(
                    "rgba({},{},{},{:.3})",
                    (r * 255.0).round(),
                    (g * 255.0).round(),
                    (b * 255.0).round(),
                    a
                )
            }
            _ => "magenta".to_string(),
        }
    }

    struct BezPen {
        path: BezPath,
        offset: Point,
    }

    impl OutlinePen for BezPen {
        fn move_to(&mut self, x: f32, y: f32) {
            self.path
                .move_to((self.offset.x + x as f64, self.offset.y - y as f64));
        }
        fn line_to(&mut self, x: f32, y: f32) {
            self.path
                .line_to((self.offset.x + x as f64, self.offset.y - y as f64));
        }
        fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
            self.path.quad_to(
                (self.offset.x + cx0 as f64, self.offset.y - cy0 as f64),
                (self.offset.x + x as f64, self.offset.y - y as f64),
            );
        }
        fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
            self.path.curve_to(
                (self.offset.x + cx0 as f64, self.offset.y - cy0 as f64),
                (self.offset.x + cx1 as f64, self.offset.y - cy1 as f64),
                (self.offset.x + x as f64, self.offset.y - y as f64),
            );
        }
        fn close(&mut self) {
            self.path.close_path();
        }
    }

    fn svg_shape(shape: &Shape, transform: Affine) -> String {
        let mut path = match shape {
            Shape::Rect(rect) => rect.to_path(0.05),
            Shape::RoundedRect(rect) => rect.to_path(0.05),
            Shape::Circle(circle) => circle.to_path(0.05),
            Shape::Line(line) => {
                let mut p = BezPath::new();
                p.move_to(line.p0);
                p.line_to(line.p1);
                p
            }
            Shape::Path(path) => path.clone(),
        };
        path.apply_affine(transform);
        path.to_svg()
    }

    fn write_cmds(out: &mut String, cmds: &[DrawCmd]) {
        for cmd in cmds {
            match cmd {
                DrawCmd::Fill {
                    shape,
                    brush,
                    transform,
                } => writeln!(
                    out,
                    r#"<path d="{}" fill="{}"/>"#,
                    svg_shape(shape, *transform),
                    css(brush)
                )
                .unwrap(),
                DrawCmd::Stroke {
                    shape,
                    style,
                    brush,
                    transform,
                } => writeln!(
                    out,
                    r#"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"#,
                    svg_shape(shape, *transform),
                    css(brush),
                    style.width
                )
                .unwrap(),
                DrawCmd::GlyphRun(run) => {
                    let Ok(font_ref) = FontRef::from_index(run.font.data.as_ref(), run.font.index)
                    else {
                        continue;
                    };
                    let outlines = font_ref.outline_glyphs();
                    let coords: Vec<NormalizedCoord> = run
                        .normalized_coords
                        .iter()
                        .map(|bits| NormalizedCoord::from_bits(*bits))
                        .collect();
                    let size = Size::new(run.size);
                    let mut path = BezPath::new();
                    for glyph in &run.glyphs {
                        let mut pen = BezPen {
                            path: std::mem::take(&mut path),
                            offset: run.transform * Point::new(glyph.x as f64, glyph.y as f64),
                        };
                        if let Some(outline) = outlines.get(GlyphId::new(glyph.id)) {
                            let settings = DrawSettings::unhinted(size, LocationRef::new(&coords));
                            let _ = outline.draw(settings, &mut pen);
                        }
                        path = pen.path;
                    }
                    writeln!(out, r#"<path d="{}" fill="{}"/>"#, path.to_svg(), css(&run.brush))
                        .unwrap();
                }
                DrawCmd::Clip { children, .. } => write_cmds(out, children),
            }
        }
    }

    fn place_with_pointer(
        doc: &Document,
        selection: Option<&Selection>,
        width: f64,
        pointer: Option<Point>,
    ) -> (Bench, Extent) {
        place_with_inputs(doc, selection, width, pointer, None)
    }

    fn place_with_inputs(
        doc: &Document,
        selection: Option<&Selection>,
        width: f64,
        pointer: Option<Point>,
        viewport: Option<Rect>,
    ) -> (Bench, Extent) {
        let library = crate::conventions::library();
        let sources = Sources {
            doc,
            library: &library,
        };
        let names = Names::convention();
        let styles = Styles::new(1.0);
        let collapse = Collapse::default();
        let mut fonts = parley::FontContext::new();
        let mut layouts = parley::LayoutContext::new();
        let mut cache = puri::text::TextCache::default();
        let mut tcx = TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            scale: 1.0,
            cache: &mut cache,
        };
        let hooks = Hooks::<Claims> {
            select: Rc::new(|_, _, _| {}),
            toggle: Rc::new(|_, _| {}),
            rename: Rc::new(|_, _, _| {}),
            edit: Rc::new(|_| None),
            pick: Rc::new(|_, _| false),
            insert: Rc::new(|_, _| {}),
        };
        // Timed as the layout perf canary: a projection is a
        // per-keystroke cost, and the fallback-heavy narrow widths
        // are where accidental exponentials have surfaced twice.
        // Numbers only, no assert (user call) — read them when the
        // bench runs; single-digit milliseconds is healthy.
        let start = std::time::Instant::now();
        let foreign = crate::conventions::foreign_functions();
        let node = project::<Claims, Bench>(
            ProjectDescription {
                sources,
                selection,
                graph_node: None,
                hover: None,
                hover_node: None,
                collapse: &collapse,
                names: &names,
                raw: false,
                styles: &styles,
                width: width - 48.0,
                projection: projection::Projection::new(&foreign),
            },
            &mut tcx,
            hooks,
        );
        let elapsed = start.elapsed();
        eprintln!("project at {width:.0}px: {elapsed:.1?}");
        let extent = node.extent;
        let mut bench = Bench {
            list: DrawList::new(),
            handler: Handler::default(),
            descends: Vec::new(),
            popup: None,
            pointer,
            hover_claims: Vec::new(),
        };
        let rect = node.extent.rect_at(Point::new(24.0, 24.0));
        crate::layout::place(
            node,
            &mut bench,
            match viewport {
                Some(clip_rect) => Placement::new(rect, clip_rect),
                None => Placement::root(rect),
            },
        );
        (bench, extent)
    }

    fn place(doc: &Document, selection: Option<&Selection>, width: f64) -> (Bench, Extent) {
        place_with_pointer(doc, selection, width, None)
    }

    fn render(doc: &Document, selection: Option<&Selection>, width: f64, out_path: &str) {
        let (bench, extent) = place(doc, selection, width);
        let (width, height) = (width.max(extent.width + 48.0), extent.height() + 48.0);
        let mut out = String::new();
        writeln!(
            out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}">"#
        )
        .unwrap();
        writeln!(
            out,
            r##"<rect width="{width:.0}" height="{height:.0}" fill="#FFFFFF"/>"##
        )
        .unwrap();
        write_cmds(&mut out, &bench.list.0);
        writeln!(out, "</svg>").unwrap();
        std::fs::write(out_path, out).unwrap();
    }

    fn contains_circle(commands: &[DrawCmd]) -> bool {
        commands.iter().any(|command| match command {
            DrawCmd::Fill { shape, .. } | DrawCmd::Stroke { shape, .. } => {
                matches!(shape, Shape::Circle(_))
            }
            DrawCmd::Clip { children, .. } => contains_circle(children),
            DrawCmd::GlyphRun(_) => false,
        })
    }

    #[test]
    fn grap_geometry_reaches_the_canvas() {
        let (bench, _) = place(&sample_document(), None, 900.0);
        assert!(contains_circle(&bench.list.0));
    }

    #[test]
    fn svg_bench_renders_the_sample_projection() {
        let doc = sample_document();
        render(&doc, None, 900.0, "../target/raw_projection.svg");
        render(&doc, None, 560.0, "../target/raw_projection_narrow.svg");
        // The deep-fallback regime: hugging fails at most levels, so
        // this render is also the canary against layout cost blowing
        // up when width is scarce.
        render(&doc, None, 320.0, "../target/raw_projection_tight.svg");
    }

    #[test]
    fn svg_bench_renders_the_grap_demo() {
        let (doc, _) = crate::gid::parse(include_str!("../../grap-demo.gid"))
            .expect("the Grap demo parses");
        render(&doc, None, 900.0, "../target/grap_demo.svg");
        render(&doc, None, 560.0, "../target/grap_demo_narrow.svg");
    }

    #[test]
    fn named_fields_display_alphabetically_before_unnamed_fields() {
        let alpha = CellId::from_u128(0xf1);
        let beta = CellId::from_u128(0x01);
        let unnamed_low = CellId::from_u128(0x02);
        let unnamed_high = CellId::from_u128(0xe1);
        let mut cells = Cells::new();
        cells.set_value(alpha, progred_name::record("alpha", []));
        cells.set_value(beta, progred_name::record("beta", []));
        let doc = Document {
            root: Some(Value::record([
                (alpha, Value::from(vec![1])),
                (beta, Value::from(vec![2])),
                (unnamed_low, Value::from(vec![3])),
                (unnamed_high, Value::from(vec![4])),
            ])),
            cells,
        };
        let (bench, _) = place(&doc, None, 320.0);
        let y = |field| {
            bench
                .descends
                .iter()
                .filter(|descend| descend.path == [Step::Key(field)])
                .map(|descend| descend.rect.y0)
                .reduce(f64::min)
                .expect("field descend")
        };
        assert!(y(alpha) < y(beta));
        assert!(y(beta) < y(unnamed_low));
        assert!(y(unnamed_low) < y(unnamed_high));
    }

    #[test]
    fn expression_children_are_real() {
        let (doc, binders) = crate::gid::parse(include_str!("../../grap-demo.gid"))
            .expect("the Grap demo parses");
        let label = binders["inert_data"];
        let position = doc
            .root
            .as_ref()
            .and_then(Value::as_list)
            .and_then(|entries| {
                entries.iter().find_map(|(position, value)| {
                    value
                        .as_record()
                        .is_some_and(|entry| entry.contains_key(&label))
                        .then(|| position.clone())
                })
            })
            .expect("inert-data demo entry");
        let record = vec![Step::Element(position), Step::Key(label)];
        let mut result = record.clone();
        result.push(Step::Key(crate::conventions::vocabulary::GRAP));
        let mut source_note = result.clone();
        source_note.push(Step::Key(binders["note"]));
        let (bench, _) = place(&doc, None, 560.0);
        assert!(bench
            .descends
            .iter()
            .any(|descend| descend.path == record));
        assert!(bench
            .descends
            .iter()
            .any(|descend| descend.path == result));
        assert!(bench
            .descends
            .iter()
            .any(|descend| descend.path == source_note));
    }

    /// The keyboard walk against real settled geometry: down visits
    /// rows in screen order — never climbing back up — and up
    /// retraces the same stops exactly.
    #[test]
    fn the_row_walk_descends_the_sample_projection_in_screen_order() {
        use ui_events::keyboard::{KeyState, Modifiers};
        let doc = sample_document();
        let (bench, _) = place(&doc, None, 560.0);
        let line = 14.0;
        let press = |named: NamedKey| KeyboardEvent {
            key: Key::Named(named),
            state: KeyState::Down,
            modifiers: Modifiers::empty(),
            ..Default::default()
        };
        let rect_of = |path: &Path| {
            bench
                .descends
                .iter()
                .find(|descend| &descend.path == path)
                .expect("walk stops on placed descends")
                .rect
        };
        let select = |path: &Path| Selection::Edge {
            path: path.clone(),
            edit: None,
            recorded: false,
        };
        let mut selection: Option<Selection> = None;
        let mut walk: Vec<Path> = Vec::new();
        while walk.len() < 200 {
            match step_selection(
                &bench.descends,
                selection.as_ref(),
                line,
                &press(NamedKey::ArrowDown),
            ) {
                Some(path) => {
                    selection = Some(select(&path));
                    walk.push(path);
                }
                None => break,
            }
        }
        assert!(walk.len() >= 5 && walk.len() < 200, "walked {}", walk.len());
        assert!(
            walk.iter().any(|path| path.len() >= 2),
            "walk enters open blocks"
        );
        for pair in walk.windows(2) {
            assert!(
                rect_of(&pair[1]).y0 >= rect_of(&pair[0]).y0,
                "down never climbs: {:?} -> {:?}",
                pair[0],
                pair[1]
            );
        }
        for expect in walk.iter().rev().skip(1) {
            let up = step_selection(
                &bench.descends,
                selection.as_ref(),
                line,
                &press(NamedKey::ArrowUp),
            )
            .expect("up retraces the walk");
            assert_eq!(&up, expect);
            selection = Some(select(&up));
        }
        // A projected simple-name field is the cell's editable head.
        let head = bench
            .descends
            .iter()
            .map(|descend| descend.path.clone())
            .find(|path| {
                matches!(
                    path.last(),
                    Some(Step::Key(label))
                        if *label == progred_name::vocabulary::NAME
                )
            })
            .expect("the sample has a cell head");
        let cell = head[..head.len() - 2].to_vec();
        assert_eq!(
            step_selection(
                &bench.descends,
                Some(&select(&cell)),
                line,
                &press(NamedKey::ArrowRight),
            ),
            Some(head)
        );
    }

    fn key(s: &str) -> Step {
        Step::Key(crate::test_values::label(s))
    }

    #[test]
    fn air_holds_only_within_a_little_gap_of_the_hover() {
        let current = Hovering {
            hover: Hover::Value(vec![key("shape")]),
            rect: Rect::new(10.0, 10.0, 30.0, 20.0),
        };
        // Direct switches unconditionally, however close.
        assert_eq!(
            resolve_hover(
                HoverClaim::Direct(None),
                Some(&current),
                Point::new(11.0, 11.0),
                8.0
            ),
            Some(None)
        );
        // Air just past the footprint holds; air beyond the reach
        // clears — open space keeps no distant focus.
        assert_eq!(
            resolve_hover(HoverClaim::Air, Some(&current), Point::new(36.0, 15.0), 8.0),
            None
        );
        assert_eq!(
            resolve_hover(HoverClaim::Air, Some(&current), Point::new(25.0, 26.0), 8.0),
            None
        );
        assert_eq!(
            resolve_hover(HoverClaim::Air, Some(&current), Point::new(60.0, 15.0), 8.0),
            Some(None)
        );
        // With nothing held, air is just air.
        assert_eq!(
            resolve_hover(HoverClaim::Air, None, Point::new(11.0, 11.0), 8.0),
            Some(None)
        );
    }

    #[test]
    fn placement_claims_the_hover_innermost_last() {
        let doc = sample_document();
        let (bench, _) = place(&doc, None, 560.0);
        let library = crate::conventions::library();
        let sources = Sources {
            doc: &doc,
            library: &library,
        };
        // Over a string leaf every containing claim reports in
        // placement order. The innermost reports last — the string
        // itself, not its containers — and replaces the earlier
        // candidates in the real pass resolver.
        let string = bench
            .descends
            .iter()
            .find(|descend| {
                sources
                    .resolve(&descend.path)
                    .is_some_and(|value| projection::whole_text(value).is_some())
                    && projected_name_owner(&descend.path).is_none()
            })
            .expect("the sample has a string leaf");
        let string_rect = string.rect;
        let string_path = string.path.clone();
        let (bench, _) = place_with_pointer(&doc, None, 560.0, Some(string_rect.center()));
        assert!(matches!(
            bench.hover_claims.last(),
            Some(HoverClaim::Direct(Some(Hovering {
                hover: Hover::Value(path),
                ..
            }))) if *path == string_path
        ));
        let (bench, _) = place_with_pointer(&doc, None, 560.0, Some(Point::new(-10.0, -10.0)));
        assert!(bench.hover_claims.is_empty());

        let center = string_rect.center();
        let clipped = place_with_inputs(
            &doc,
            None,
            560.0,
            Some(center),
            Some(Rect::new(
                string_rect.x0,
                string_rect.y0,
                center.x - 1.0,
                string_rect.y1,
            )),
        )
        .0;
        assert!(clipped.hover_claims.is_empty());
    }

    /// Two flat elements and two block rows, deterministically: the
    /// short list stays a literal, the long strings force the block.
    fn gap_document() -> Document {
        Document {
            root: Some(Value::record([
                (
                    crate::test_values::label("tags"),
                    Value::list([crate::test_values::text("a"), crate::test_values::text("b")]),
                ),
                (
                    crate::test_values::label("body"),
                    Value::list([
                        crate::test_values::text(
                            "a long enough string that the flat literal cannot fit",
                        ),
                        crate::test_values::text(
                            "and another beside it overflowing any width we render",
                        ),
                    ]),
                ),
            ])),
            cells: Cells::new(),
        }
    }

    /// The two descends under `field`, in the given axis order.
    fn elements_of(bench: &Bench, field: &str, by_y: bool) -> (Descend, Descend) {
        let mut found: Vec<&Descend> = bench
            .descends
            .iter()
            .filter(|descend| descend.path.len() == 2 && descend.path.first() == Some(&key(field)))
            .collect();
        found.sort_by(|a, b| {
            let (a, b) = if by_y {
                (a.rect.y0, b.rect.y0)
            } else {
                (a.rect.x0, b.rect.x0)
            };
            a.total_cmp(&b)
        });
        assert_eq!(found.len(), 2);
        let clone = |d: &Descend| Descend {
            path: d.path.clone(),
            rect: d.rect,
        };
        (clone(found[0]), clone(found[1]))
    }

    #[test]
    fn flat_separators_claim_the_insert_between() {
        let doc = gap_document();
        let (bench, _) = place(&doc, None, 560.0);
        let (first, second) = elements_of(&bench, "tags", false);
        let mid = Point::new(
            (first.rect.x1 + second.rect.x0) / 2.0,
            first.rect.center().y,
        );
        let (bench, _) = place_with_pointer(&doc, None, 560.0, Some(mid));
        assert!(matches!(
            bench.hover_claims.last(),
            Some(HoverClaim::Direct(Some(Hovering {
                hover: Hover::Insert(path),
                ..
            }))) if *path == first.path
        ));
    }

    #[test]
    fn block_gaps_are_unclaimed_air_and_brackets_widen() {
        let doc = gap_document();
        let (bench, _) = place(&doc, None, 560.0);
        let (upper, lower) = elements_of(&bench, "body", true);
        let parent = vec![key("body")];
        let gap_y = (upper.rect.y1 + lower.rect.y0) / 2.0;
        // Between the rows nothing claims: the gap is air, and air is
        // the SHELL's backstop — hold-or-clear by reach, never the
        // container outright.
        let (air, _) = place_with_pointer(
            &doc,
            None,
            560.0,
            Some(Point::new(upper.rect.center().x, gap_y)),
        );
        assert!(air.hover_claims.is_empty());
        // Just inside the bracket's absorbed gap, the bracket claims
        // the container outright — the widened handle.
        let styles = Styles::new(1.0);
        let list = bench
            .descends
            .iter()
            .find(|descend| descend.path == parent)
            .expect("the list has a landmark");
        let (claimed, _) = place_with_pointer(
            &doc,
            None,
            560.0,
            Some(Point::new(
                list.rect.x0 + delim_advance(&styles, Delim::Bracket) + 1.0,
                gap_y,
            )),
        );
        assert!(matches!(
            claimed.hover_claims.last(),
            Some(HoverClaim::Direct(Some(Hovering {
                hover: Hover::Value(path),
                ..
            }))) if *path == parent
        ));
    }

    #[test]
    fn popup_rows_claim_their_entries_and_the_card_occludes() {
        let popup = Popup {
            anchor: Rect::new(0.0, 0.0, 10.0, 10.0),
            entries: vec![
                Entry {
                    display: "\"x\"".to_string(),
                    detail: None,
                    matches: Vec::new(),
                    id: false,
                    action: EntryAction::Value(crate::test_values::text("x")),
                },
                Entry {
                    display: "new list".to_string(),
                    detail: None,
                    matches: Vec::new(),
                    id: false,
                    action: EntryAction::NewList,
                },
            ],
            choice: 0,
        };
        let place_card = |pointer| {
            let mut fonts = parley::FontContext::new();
            let mut layouts = parley::LayoutContext::new();
            let mut cache = puri::text::TextCache::default();
            let mut tcx = TextCtx {
                fonts: &mut fonts,
                layouts: &mut layouts,
                scale: 1.0,
                cache: &mut cache,
            };
            let card = popup_view::<Claims, Bench>(
                &mut tcx,
                &Styles::new(1.0),
                &popup,
                None,
                |_, _| {},
            );
            let extent = card.extent;
            let mut bench = Bench {
                list: DrawList::new(),
                handler: Handler::default(),
                descends: Vec::new(),
                popup: None,
                pointer: Some(pointer),
                hover_claims: Vec::new(),
            };
            crate::layout::place_top_left(card, &mut bench, Point::ZERO);
            (bench, extent)
        };
        // The card's own padding claims-and-clears: an overlay's
        // pointer never falls through to what sits beneath it.
        let (padding, extent) = place_card(Point::new(1.0, 1.0));
        assert_eq!(padding.hover_claims.last(), Some(&HoverClaim::Direct(None)));
        // Scanning down the card crosses both rows, each claiming its
        // index — an address into the live entries, never a snapshot.
        let winners: Vec<Hover> = (0..extent.height() as usize)
            .filter_map(|y| {
                let (bench, _) = place_card(Point::new(extent.width / 2.0, y as f64 + 0.5));
                match bench.hover_claims.into_iter().last() {
                    Some(HoverClaim::Direct(Some(hovering))) => Some(hovering.hover),
                    _ => None,
                }
            })
            .collect();
        assert!(winners.contains(&Hover::Entry(0)));
        assert!(winners.contains(&Hover::Entry(1)));
    }

    #[test]
    fn svg_bench_renders_the_placeholder_notation() {
        let empty = Document {
            root: None,
            cells: Cells::new(),
        };
        render(&empty, None, 320.0, "../target/raw_placeholder_root.svg");
        // The engaged twin: same slot, same rect, selection blue.
        render(
            &empty,
            Some(&pending_value(Vec::new())),
            320.0,
            "../target/raw_placeholder_engaged.svg",
        );
        let cells = Cells::new();
        let bare = new_cell_id();
        render(
            &Document {
                root: Some(Value::from(bare)),
                cells,
            },
            None,
            320.0,
            "../target/raw_placeholder_cell.svg",
        );
        // The commit transition pair: the same spelling typed in the
        // slot and committed as the string — glyphs should not move.
        render(
            &empty,
            Some(&Selection::Pending {
                path: Vec::new(),
                query: line_edit("\"asdf\""),
                choice: 0,
            }),
            320.0,
            "../target/raw_placeholder_typed.svg",
        );
        render(
            &Document {
                root: Some(crate::test_values::text("asdf")),
                cells: Cells::new(),
            },
            Some(&Selection::Edge {
                path: Vec::new(),
                edit: None,
                recorded: false,
            }),
            320.0,
            "../target/raw_placeholder_committed.svg",
        );
        // The empty string under its write-through editor: quotes
        // stay snug, no slot minimum applies to string literals.
        let empty_string = Document {
            root: Some(crate::test_values::text("")),
            cells: Cells::new(),
        };
        let library = crate::conventions::library();
        let sel = Selection::edge(
            &Sources {
                doc: &empty_string,
                library: &library,
            },
            Vec::new(),
        );
        render(
            &empty_string,
            Some(&sel),
            320.0,
            "../target/raw_empty_string_editing.svg",
        );
    }

    // The re-opened label: the tags field's query seeded with its
    // quoted spelling, ringed in place, the value staying put.
    #[test]
    fn svg_bench_renders_a_label_rename() {
        let doc = sample_document();
        let library = crate::conventions::library();
        let path = vec![
            Step::Key(crate::test_values::label("shape")),
            Step::Follow,
            Step::Key(crate::test_values::label("tags")),
        ];
        let rename = pending_rename(
            &Sources {
                doc: &doc,
                library: &library,
            },
            &path,
        )
        .unwrap();
        render(&doc, Some(&rename), 560.0, "../target/raw_label_rename.svg");
    }

    #[test]
    fn svg_bench_renders_a_pending_edge() {
        let doc = sample_document();
        let library = crate::conventions::library();
        let edge = pending_edge(
            &Sources {
                doc: &doc,
                library: &library,
            },
            vec![Step::Key(crate::test_values::label("shape"))],
        )
        .unwrap();
        let Selection::PendingEdge {
            parent, replacing, ..
        } = edge
        else {
            panic!("a pending edge pends");
        };
        let typing = Selection::PendingEdge {
            parent,
            query: line_edit("na"),
            choice: 0,
            replacing,
        };
        render(&doc, Some(&typing), 560.0, "../target/raw_pending_edge.svg");
    }
}
