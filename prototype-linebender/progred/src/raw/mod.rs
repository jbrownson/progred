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
mod tests;
#[cfg(test)]
mod svg_bench;
