//! The editor's projection runtime: resolve locations, supply borrowed widget inputs,
//! retain source provenance, and fall back to total structural display.

mod child;
#[cfg(test)]
#[path = "tests/projection/test_support.rs"]
pub(crate) mod test_support;
#[cfg(test)]
#[path = "tests/projection/mod.rs"]
mod tests;
pub(crate) mod viewport;

use crate::annotations::{Annotations, Node};
use crate::display::widget::placed::{before, decorate};
use crate::display::widget::source::{Secondary, SourceTrace};
use crate::display::widget::style::{
    Styles, highlight_outline, hover_highlight, primary_highlight, secondary_highlight,
};
use crate::frame::{Attribution, HoverPass, Hovered};
use crate::hover::Hover;
use crate::location::{Location, Start};
use crate::occurrence::{Step, Trail};
use crate::selection::{Selection, Stage};
use crate::sources::Sources;
use gid::{CellId, Value};
use kurbo::{Affine, RoundedRect};
use measured::Measured;
use measured::choices::{ChoiceBuild, ChoiceGraph, ChoiceLayout, resolve_choices};
use peniko::Color;
use puri::draw::Canvas;
use puri::edit::LineEditState;
use puri::handler::HasHandler;
use puri::text::TextCtx;
use std::collections::HashSet;
use std::rc::Rc;

/// The toolkit's types over the editor, which hosts everything the runtime
/// projects. Display and the libraries stay generic over their host, through
/// [`crate::display::host`].
pub(crate) type Layout = crate::display::host::Layout<crate::Editor>;
pub(crate) type Partial = crate::display::host::Partial<crate::Editor>;
pub(crate) type Provided = crate::display::host::Provided<crate::Editor>;

/// One ordered composition of partial value projections. The
/// structural fallback lives in this runtime and is always total.
#[derive(Clone)]
pub struct Projection {
    partial: Partial,
    entry: Option<Partial>,
    /// The projections cells provide, for every input drawn with this.
    provided: Provided,
}

impl Default for Projection {
    fn default() -> Self {
        Self {
            partial: crate::display::runtime_partial(|_| None),
            entry: None,
            provided: Provided::default(),
        }
    }
}

impl Projection {
    pub fn new(partials: impl IntoIterator<Item = Partial>) -> Self {
        Self {
            partial: crate::display::compose_partials(partials),
            entry: None,
            provided: Provided::default(),
        }
    }

    /// With `provided` lent to every input it draws.
    pub fn with_provided(self, provided: Provided) -> Self {
        Self { provided, ..self }
    }

    /// The projections cells provide for what this draws.
    #[cfg(test)]
    pub(crate) fn provided(&self) -> &Provided {
        &self.provided
    }

    /// The composed partials, without the entry.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn partial(&self) -> &Partial {
        &self.partial
    }

    /// `partial` tried before this projection's own partials, its entry and
    /// the projections cells provide kept.
    #[cfg(test)]
    pub(crate) fn preceded_by(&self, partial: Partial) -> Self {
        Self {
            partial: crate::display::compose_partials([partial, self.partial.clone()]),
            entry: self.entry.clone(),
            provided: self.provided.clone(),
        }
    }

    /// Prepend a partial at entry, following cells to their definitions.
    /// Its children and computed results use the ordinary projection.
    pub fn with_entry(self, partial: Partial) -> Self {
        Self {
            entry: Some(partial),
            ..self
        }
    }

    fn without_entry(&self) -> Self {
        Self {
            partial: self.partial.clone(),
            entry: None,
            provided: self.provided.clone(),
        }
    }

    fn apply(
        &self,
        input: &crate::display::ProjectionInput<'_, crate::Editor, Hovered, grap::RuntimeValue>,
    ) -> Option<Layout> {
        self.entry
            .as_ref()
            .and_then(|entry| entry(input))
            .or_else(|| (self.partial)(input))
    }
}

/// Read-only projection context threaded through every view.
#[derive(Clone)]
pub(crate) struct Cx<'a> {
    pub(crate) computations: &'a crate::computations::Computations,
    pub(crate) focused: bool,
    pub(crate) completions: Option<&'a crate::display::CompletionProvider>,
    /// The reading context: the document read over its library.
    pub(crate) sources: Sources<'a>,
    /// Whether the name library labels cells here; field order follows.
    pub(crate) names: bool,
    pub(crate) styles: &'a Styles,
    pub(crate) selection: Option<&'a Selection>,
    /// The selected structural source, normalized across projections
    /// for execution-linked output.
    pub(crate) selected_trace: Option<SourceTrace>,
}

/// One occurrence in a view, as the widgets drawing it and the projections
/// building its layout read the view.
struct Occurrence<'a> {
    cx: &'a Cx<'a>,
    path: &'a Trail,
    ancestors: &'a Ancestry,
    /// The view's state here and below.
    state: Option<&'a Node>,
}

impl Occurrence<'_> {
    fn below(&self, steps: &[gid::Step]) -> Trail {
        self.path.below(steps)
    }

    /// Where what Grap evaluates `steps` below this occurrence comes from.
    fn origin(&self, steps: &[gid::Step]) -> Option<grap::SourceOrigin> {
        crate::display::Toolkit::trace_below(self, steps).map(crate::hover::to_grap)
    }
}

impl crate::display::Viewing for Occurrence<'_> {
    fn scale(&self) -> f64 {
        self.cx.styles.scale
    }

    fn writable(&self) -> bool {
        self.ancestors.writable(&self.cx.sources)
    }

    fn host(&self) -> &dyn grap::Host {
        &self.cx.sources
    }

    fn computations(&self) -> &crate::computations::Computations {
        self.cx.computations
    }

    fn state_below(&self, steps: &[gid::Step]) -> Option<&Value> {
        steps
            .iter()
            .try_fold(self.state?, |node, step| node.child(&step.into()))?
            .value()
    }

    fn selected_below(&self, steps: &[gid::Step]) -> bool {
        let below = self.below(steps);
        self.cx
            .selection
            .is_some_and(|selection| below.leads(selection.path()))
    }

    fn inside(&self, cell: CellId) -> bool {
        self.ancestors.cells.contains(&cell)
    }

    fn inside_a_cell(&self) -> bool {
        self.ancestors
            .enclosing
            .is_some_and(|(_, start)| self.path.len() > start)
    }
}

impl crate::display::Toolkit for Occurrence<'_> {
    fn path(&self) -> &Trail {
        self.path
    }

    fn site(&self) -> crate::editing::Site {
        self.cx.site(self.path.clone())
    }

    fn styles(&self) -> &Styles {
        self.cx.styles
    }

    fn sources(&self) -> Sources<'_> {
        self.cx.sources
    }

    fn location(&self) -> Option<Location<'static>> {
        crate::occurrence::location(&self.path.to_vec())
    }

    fn focused(&self) -> bool {
        self.cx.focused
    }

    fn selection(&self) -> Option<&Selection> {
        self.cx.selection
    }

    fn selected(&self) -> bool {
        self.cx.selected(self.path)
    }

    fn label_query(&self) -> Option<&LineEditState> {
        self.cx.pending_edge_under(self.path)
    }

    fn completions(&self) -> Option<&crate::display::CompletionProvider> {
        self.cx.completions
    }

    fn name(&self, cell: CellId) -> Option<crate::display::Name<'_>> {
        crate::libraries::name::of(&self.cx.sources, cell)
    }

    fn selected_trace(&self) -> Option<&SourceTrace> {
        self.cx.selected_trace.as_ref()
    }

    fn trace_below(&self, steps: &[gid::Step]) -> Option<SourceTrace> {
        crate::occurrence::location(&self.below(steps).to_vec())
            .map(|location| SourceTrace::from_location(&self.cx.sources, &location))
    }
}

impl crate::display::Env for Occurrence<'_> {
    fn apply_expression_runtime(
        &self,
        function: &grap::RuntimeValue,
        arguments: &[(CellId, grap::RuntimeValue)],
    ) -> grap::RuntimeValue {
        grap::apply_expression(
            function,
            arguments.iter().cloned(),
            &self.cx.sources,
            None,
            grap::DEFAULT_FUEL,
        )
        .result
    }
    fn evaluate_runtime(
        &self,
        expression: &grap::RuntimeValue,
        fuel: usize,
        steps: &[gid::Step],
    ) -> grap::RuntimeValue {
        grap::evaluate_runtime_at(expression, self.origin(steps), &self.cx.sources, fuel).result
    }
    fn evaluate_runtime_memo(
        &self,
        expression: &grap::RuntimeValue,
        fuel: usize,
        steps: &[gid::Step],
    ) -> grap::RuntimeValue {
        self.cx
            .computations
            .evaluate_runtime(self.path, expression, fuel, self.origin(steps))
    }
    fn evaluation(
        &self,
        expression: &grap::RuntimeValue,
        fuel: usize,
        steps: &[gid::Step],
    ) -> crate::display::Evaluated {
        crate::display::Evaluated {
            expression: expression.clone(),
            origin: self.origin(steps),
            fuel,
        }
    }
    fn evaluated(&self) -> Option<&crate::display::Evaluated> {
        self.ancestors.evaluated.as_deref()
    }
    fn stack(&self, call: &grap::SourceOrigin, value: &Value) -> Option<Vec<grap::SourceOrigin>> {
        self.cx
            .computations
            .answering(self.path, self.evaluated()?, call, value)
    }
    fn apply_runtime_scoped(
        &self,
        function: &grap::RuntimeValue,
        arguments: &[(CellId, grap::RuntimeValue)],
        scope: Option<&grap::ForeignOverlay<'_>>,
    ) -> grap::Evaluation {
        self.cx
            .sources
            .apply_runtime_scoped(function, arguments, scope)
    }
    fn apply_runtime_memo(
        &self,
        function: &Value,
        arguments: &[(CellId, Value)],
        fuel: usize,
    ) -> grap::RuntimeValue {
        self.cx
            .computations
            .apply_runtime(self.path, function, arguments, fuel)
    }

    /// Unlike [`crate::display::Toolkit::name`], Raw's view names nothing.
    fn name(&self, cell: CellId) -> Option<crate::display::Name<'_>> {
        self.cx.name(cell)
    }

    fn resolve(&self, cell: CellId) -> Option<crate::display::ResolvedCell<'_>> {
        self.cx.sources.resolve(cell)
    }

    fn libraries(&self) -> Vec<crate::display::DocumentLibrary> {
        crate::workspace::libraries(&self.cx.sources)
    }

    fn occurrence(&self) -> Option<&Trail> {
        Some(self.path)
    }

    fn read_from(&self, start: crate::location::Start, path: &[gid::Step]) -> Option<&Value> {
        self.cx.sources.read_from(start, path)
    }

    fn read_computed(&self, path: &[gid::Step]) -> Option<&Value> {
        path.iter().try_fold(
            self.ancestors.computed.as_deref()?.as_value(),
            |value, step| match step {
                gid::Step::Key(field) => value.as_record()?.get(field),
                gid::Step::Element(position) => value.as_list()?.get(position),
                gid::Step::Follow => None,
            },
        )
    }
}

#[derive(Clone, Default)]
struct Ancestry {
    /// Cells crossed by `Follow`, used to stop projection cycles.
    cells: HashSet<CellId>,
    /// The nearest followed definition and the start of its relative path.
    enclosing: Option<(CellId, usize)>,
    /// Where the data here is stored starts, as
    /// [`crate::occurrence::stored_start`] finds it from the path, kept as
    /// the projection descends so no occurrence reads its route again. None
    /// for a computed value, or a route that no longer reaches a cell.
    stored: Option<Start>,
    /// The evaluation the computed value here came from, kept below it so
    /// what's drawn inside can run it again; none in the document.
    evaluated: Option<Rc<crate::display::Evaluated>>,
    /// The computed value this occurrence lies in, shared below it so it's
    /// made data once for whatever reads above itself there; none where
    /// it's stored.
    computed: Option<Rc<grap::RuntimeValue>>,
    ground: Ground,
}

impl Ancestry {
    /// Whether a write can land where the data here is stored.
    fn writable(&self, sources: &Sources) -> bool {
        self.stored
            .is_some_and(|start| crate::location::owned(sources, start))
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Ground {
    #[default]
    Writable,
    Readonly,
}

/// Prepare `layout` at the occurrence `scope` describes, with its explicit
/// source and current widget inputs. Box composition itself belongs to the
/// display measurement interpreter.
fn prepare(
    scope: &ProjectionScope<'_, '_>,
    tcx: &mut TextCtx,
    layout: Layout,
    build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
) -> ChoiceLayout<HoverPass<crate::Editor>> {
    layout.measure(
        &mut crate::display::widget::Context {
            project: scope,
            text: tcx,
            toolkit: &Occurrence {
                cx: scope.cx,
                path: scope.path,
                ancestors: scope.ancestors,
                state: scope.state,
            },
            value: scope.value,
        },
        build,
    )
}

/// One occurrence being projected: its view's context, the projection it
/// draws with, its path, its ancestry, the view's state here, and its value.
#[derive(Clone, Copy)]
struct ProjectionScope<'a, 's> {
    cx: &'a Cx<'s>,
    projection: &'a Projection,
    path: &'a Trail,
    ancestors: &'a Ancestry,
    state: Option<&'a Node>,
    value: Option<&'a grap::RuntimeValue>,
}

impl ProjectionScope<'_, '_> {
    /// Adds GID steps to the occurrence, reading what's there. Current and
    /// descendant projections are independent replacements. Missing
    /// locations use the standard empty picker, without a value.
    fn descend_path(
        &self,
        tcx: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
        steps: &[gid::Step],
        current: Option<Partial>,
        default: Option<Partial>,
        part: bool,
    ) -> ChoiceLayout<HoverPass<crate::Editor>> {
        let cx = self.cx;
        let mut path = self.path.clone();
        let mut state = self.state;
        let mut ancestors = self.ancestors.clone();
        let mut value = self.value.cloned();
        for step in steps {
            let followed = match step {
                gid::Step::Follow => value.as_ref().and_then(grap::RuntimeValue::as_cell),
                gid::Step::Key(_) | gid::Step::Element(_) => None,
            };
            let step_here = match followed {
                // A cell inside a computed value has no stored route to it, so
                // following it jumps to its definition, which starts a
                // location again.
                Some(cell) if path.computed() => Step::jump(Location::in_cell(cell, Vec::new())),
                _ => step.into(),
            };
            state = state.and_then(|node| node.child(&step_here));
            path = path.push(step_here);
            match (step, followed) {
                (_, Some(cell)) => {
                    ancestors.cells.insert(cell);
                    ancestors.computed = None;
                    ancestors.enclosing = Some((cell, path.len()));
                    // Following a cell enters its definition, which stores
                    // what's below.
                    ancestors.stored = Some(Start::Cell(cell));
                }
                // A Follow from something that isn't a cell reaches nothing.
                (gid::Step::Follow, None) => ancestors.stored = None,
                _ => {}
            }
            value = value
                .as_ref()
                .and_then(|value| child::of(value, step, |cell| cx.sources.value(cell)));
        }
        prepare_value(
            ProjectionScope {
                cx,
                path: &path,
                ancestors: &ancestors,
                state,
                value: value.as_ref(),
                ..*self
            },
            current.as_ref(),
            default.as_ref(),
            part,
            tcx,
            build,
        )
    }
}

impl crate::display::widget::project::Project<crate::Editor, Hovered> for ProjectionScope<'_, '_> {
    fn descend(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
        steps: &[gid::Step],
        current: Option<Partial>,
        default: Option<Partial>,
        part: bool,
    ) -> ChoiceLayout<HoverPass<crate::Editor>> {
        self.descend_path(text, build, steps, current, default, part)
    }
    #[cfg(any(test, target_arch = "wasm32"))]
    fn descend_raw(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
        step: gid::Step,
    ) -> ChoiceLayout<HoverPass<crate::Editor>> {
        // Raw, with every library off: no names and no library's offers.
        let cx = Cx {
            names: false,
            completions: None,
            ..self.cx.clone()
        };
        let nothing = crate::display::partial(|_| None);
        ProjectionScope { cx: &cx, ..*self }.descend_path(
            text,
            build,
            std::slice::from_ref(&step),
            Some(nothing.clone()),
            Some(nothing),
            false,
        )
    }
    fn jump(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
        steps: Vec<gid::Step>,
        document: Location<'static>,
        current: Option<Partial>,
        default: Option<Partial>,
    ) -> ChoiceLayout<HoverPass<crate::Editor>> {
        let cx = self.cx;
        let value = cx
            .sources
            .read(&document)
            .cloned()
            .map(grap::RuntimeValue::from);
        let mut ancestry = self.ancestors.clone();
        // Following the same source through jumps must not bypass cycle checks.
        {
            let location = &document;
            ancestry.cells.extend(
                match location.start {
                    Start::Cell(cell) => Some(cell),
                    Start::Root => None,
                }
                .into_iter()
                .chain(
                    location
                        .path
                        .iter()
                        .enumerate()
                        .filter(|(_, step)| matches!(step, gid::Step::Follow))
                        .filter_map(|(i, _)| {
                            cx.sources
                                .read_from(location.start, &location.path[..i])
                                .and_then(Value::as_cell)
                        }),
                ),
            );
        }
        ancestry.stored = document.stored(&cx.sources).map(|stored| stored.start);
        ancestry.evaluated = None;
        ancestry.computed = None;
        let jump = Step::jump(document);
        let state = descend_state(self.state, &steps).and_then(|node| node.child(&jump));
        let path = self.path.below(&steps).push(jump);
        prepare_value(
            ProjectionScope {
                cx,
                path: &path,
                ancestors: &ancestry,
                state,
                value: value.as_ref(),
                ..*self
            },
            current.as_ref(),
            default.as_ref(),
            false,
            text,
            build,
        )
    }
    fn at(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
        steps: Vec<gid::Step>,
        value: grap::RuntimeValue,
        evaluated: Option<Rc<crate::display::Evaluated>>,
        current: Option<Partial>,
        default: Option<Partial>,
    ) -> ChoiceLayout<HoverPass<crate::Editor>> {
        let path = self.path.below(&steps).push(Step::Computed);
        prepare_value(
            ProjectionScope {
                cx: self.cx,
                path: &path,
                ancestors: &Ancestry {
                    ground: self.ancestors.ground,
                    evaluated,
                    computed: Some(Rc::new(value.clone())),
                    ..Ancestry::default()
                },
                state: descend_state(self.state, &steps)
                    .and_then(|node| node.child(&Step::Computed)),
                value: Some(&value),
                ..*self
            },
            current.as_ref(),
            default.as_ref(),
            false,
            text,
            build,
        )
    }
}

/// The view's state a document path's `steps` below `node`.
fn descend_state<'a>(node: Option<&'a Node>, steps: &[gid::Step]) -> Option<&'a Node> {
    steps
        .iter()
        .try_fold(node?, |node, step| node.child(&step.into()))
}

/// The resolved hover's tree identity, for ink that lights its own
/// claim.
fn tree_hovered(hover: &Attribution) -> Option<&Hover> {
    match hover.hovered.as_ref() {
        Some(Hovered::Tree(hover)) => Some(hover),
        _ => None,
    }
}

impl Cx<'_> {
    /// The occurrence at `path`, as a place to act.
    pub(crate) fn site(&self, path: Trail) -> crate::editing::Site {
        crate::editing::Site::new(path)
    }

    /// The display name at this projection; with the name library off,
    /// labels fall back to the short id.
    fn name(&self, cell: CellId) -> Option<crate::display::Name<'_>> {
        self.names
            .then(|| crate::libraries::name::of(&self.sources, cell))
            .flatten()
    }

    /// Whether `path` carries the primary highlight. A label-stage
    /// pending deliberately does not mark its parent — nothing is
    /// selected there, something is being authored inside; the
    /// pending row carries the highlight itself.
    pub(crate) fn selected(&self, path: &Trail) -> bool {
        self.selection.is_some_and(|current| {
            *path == current.path() && current.stage(&self.sources) != Stage::Label
        })
    }

    /// Whether the selection is a picker at `path` itself. Over an existing
    /// value it is replacing that value, so views draw the hole instead.
    fn pending_at(&self, path: &Trail) -> bool {
        self.selection.is_some_and(|current| {
            *path == current.path() && current.explicit_stage() == Stage::Pending
        })
    }

    /// The pending child step under `path`, when the selection is
    /// authoring one there.
    fn pending_child_of(&self, path: &Trail) -> Option<Step> {
        let current = self.selection?;
        (current
            .path()
            .split_last()
            .is_some_and(|(_, parent)| *path == parent)
            && current.stage(&self.sources) == Stage::Pending)
            .then(|| current.path().last().cloned())
            .flatten()
    }

    /// The label query of a new field being authored on the record at
    /// `path`.
    pub(crate) fn pending_edge_under(&self, path: &Trail) -> Option<&LineEditState> {
        let current = self.selection?;
        (*path == current.path() && current.stage(&self.sources) == Stage::Label)
            .then(|| current.edit())
            .flatten()
    }
}

/// The explicit-state boundary: everything a projection pass reads.
/// `width` is the space the projection may fill. `root` and
/// `root_path` let an editor pane begin at a value occurrence: the path says
/// where its data is, as any occurrence's does, a computed one included.
pub struct ProjectDescription<'a> {
    pub computations: &'a crate::computations::Computations,
    pub focused: bool,
    pub completions: Option<&'a crate::display::CompletionProvider>,
    pub sources: Sources<'a>,
    /// The view the projection draws in, which every occurrence it draws is
    /// in.
    pub view: Rc<crate::workspace::View>,
    pub root: Option<&'a Value>,
    pub root_path: &'a [Step],
    /// Selection belonging to this editable view.
    pub selection: Option<&'a Selection>,
    /// Selection from any view, used only to link generated output
    /// back to its structural source.
    pub source_selection: Option<&'a Selection>,
    pub annotations: &'a Annotations,
    pub names: bool,
    pub styles: &'a Styles,
    pub width: f64,
    pub projection: &'a Projection,
}

pub(crate) fn project(
    description: ProjectDescription<'_>,
    tcx: &mut TextCtx,
) -> Measured<HoverPass<crate::Editor>> {
    let width = description.width;
    resolve_choices(prepare_project(description, tcx), width)
}

fn prepare_project(
    description: ProjectDescription<'_>,
    tcx: &mut TextCtx,
) -> ChoiceGraph<HoverPass<crate::Editor>> {
    let ProjectDescription {
        computations,
        focused,
        completions,
        sources,
        view,
        root,
        root_path,
        selection,
        source_selection,
        annotations,
        names,
        styles,
        width: _,
        projection,
    } = description;
    // A view reached through a cell is inside that cell's definition, as
    // following the cell in the view would be.
    let followed = crate::occurrence::location(root_path)
        .and_then(|location| {
            location
                .stored(&sources)
                .map(crate::location::Location::into_owned)
        })
        .and_then(|stored| match stored.start {
            Start::Cell(cell) => root_path
                .len()
                .checked_sub(stored.path.len())
                .map(|start| (cell, start)),
            Start::Root => None,
        });
    let cx = Cx {
        computations,
        focused,
        completions,
        sources,
        names,
        styles,
        selection,
        selected_trace: source_selection.and_then(|selection| {
            Some(SourceTrace::from_location(&sources, &selection.location()?))
        }),
    };
    // An empty document is a selectable placeholder at the root path.
    let mut build = ChoiceBuild::default();
    let ancestry = Ancestry {
        stored: crate::occurrence::stored_start(&sources, root_path),
        ..followed.map_or_else(Ancestry::default, |(cell, start)| Ancestry {
            cells: HashSet::from([cell]),
            enclosing: Some((cell, start)),
            ..Ancestry::default()
        })
    };
    let layout = prepare_value(
        ProjectionScope {
            cx: &cx,
            projection,
            path: &Trail::in_view(view, root_path),
            ancestors: &ancestry,
            state: annotations.node_at(root_path),
            value: root.cloned().map(grap::RuntimeValue::from).as_ref(),
        },
        None,
        None,
        false,
        tcx,
        &mut build,
    );
    build.finish(layout)
}

/// `child` with `path`'s highlight over its full bounds: the primary one
/// when `selected`, else the secondary mark it shares with the selected or
/// hovered occurrence, or the hover's.
fn highlighted(
    palette: crate::display::widget::style::Palette,
    selected: bool,
    scale: f64,
    path: Trail,
    secondary: Option<Secondary>,
    child: Measured<HoverPass<crate::Editor>>,
) -> Measured<HoverPass<crate::Editor>> {
    decorate(child, move |p, rect| {
        let outline = highlight_outline(scale, rect);
        p.render(move |cv, hover| {
            let shares = |other: &Option<Secondary>| secondary.is_some() && *other == secondary;
            if selected {
                primary_highlight(palette, scale, cv, outline);
            } else if shares(&hover.selected_secondary) {
                secondary_highlight(palette, scale, cv, outline, true);
            } else if matches!(
                tree_hovered(hover),
                Some(Hover::Value(hovered) | Hover::Label(hovered))
                    if *hovered == path
            ) {
                hover_highlight(palette, scale, cv, outline);
            } else if shares(&hover.hovered_secondary) {
                secondary_highlight(palette, scale, cv, outline, false);
            }
        });
    })
}

/// Standard commands for this displayed value, below its own widget handlers:
/// copy and cut, and deleting it.
/// The occurrence's path identifies the selection and says where it writes.
fn bind_selection(
    child: Measured<HoverPass<crate::Editor>>,
    path: Trail,
    value: grap::RuntimeValue,
    scale: f64,
) -> Measured<HoverPass<crate::Editor>> {
    before(child, move |p, _| {
        let command = p.command_modifier();
        p.handler().on_key_with(move |editor, event, input| {
            let copy = crate::modifiers::chord(command, event, "c");
            let cut = crate::modifiers::chord(command, event, "x");
            event.state.is_down()
                && editor.model.selection.as_ref().is_some_and(|selection| {
                    path.view() == selection.view()
                        && path == selection.path()
                        && selection.stage(&editor.sources()) == Stage::Edge
                })
                && if copy || cut {
                    // A cut's occurrence is gone before a picture of it
                    // could be drawn.
                    let drawn = (!cut).then(|| path.clone());
                    editor.copy_value(value.as_value(), drawn.as_ref()) && {
                        if cut {
                            editor.delete_selected_edge(input.geometry(scale));
                        }
                        true
                    }
                } else {
                    editor.delete_key(input.geometry(scale), event)
                }
        })
    })
}

/// The ground under what's drawn at `path`, `writable` or not where it's
/// stored: a computed value is read-only.
fn ground(cx: &Cx, path: &Trail, value: Option<&grap::RuntimeValue>, writable: bool) -> Ground {
    match value.and_then(grap::RuntimeValue::as_cell) {
        _ if path.computed() => Ground::Readonly,
        // A reference remains replaceable; the ground describes its definition.
        Some(cell) if cx.sources.external(cell) => Ground::Readonly,
        Some(_) => Ground::Writable,
        None if writable => Ground::Writable,
        None => Ground::Readonly,
    }
}

fn ground_with(
    scale: f64,
    color: Color,
    content: Measured<HoverPass<crate::Editor>>,
) -> Measured<HoverPass<crate::Editor>> {
    decorate(content, move |p, rect| {
        let bg = RoundedRect::from_rect(rect.inset(3.0 * scale), 5.0 * scale);
        p.fill(bg, color, Affine::IDENTITY);
    })
}

/// The occurrence `scope` describes, drawn with `current_projection` when
/// given, its children with `default_projection`; as a `part`, with no stop
/// of its own unless a control drawn for it declares one, so it joins the
/// stop of the occurrence that walked to it.
fn prepare_value(
    scope: ProjectionScope<'_, '_>,
    current_projection: Option<&Partial>,
    default_projection: Option<&Partial>,
    part: bool,
    tcx: &mut TextCtx,
    build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
) -> ChoiceLayout<HoverPass<crate::Editor>> {
    let ProjectionScope {
        cx,
        projection,
        path,
        ancestors,
        ..
    } = scope;
    // A picker over a value draws its hole instead.
    let value = scope.value.filter(|_| !cx.pending_at(path));
    let writable = ancestors.writable(&cx.sources);
    let ground = ground(cx, path, value, writable);
    let changed_ground = (ground != ancestors.ground).then_some(ground);
    let changed_ancestors = changed_ground.map(|ground| Ancestry {
        ground,
        ..ancestors.clone()
    });
    let ancestors = changed_ancestors.as_ref().unwrap_or(ancestors);
    let child_projection = default_projection
        .map(|partial| Projection {
            partial: partial.clone(),
            entry: None,
            provided: projection.provided.clone(),
        })
        .or_else(|| {
            (projection.entry.is_some() && value.and_then(grap::RuntimeValue::as_cell).is_none())
                .then(|| projection.without_entry())
        });
    let child_projection = child_projection.as_ref().unwrap_or(projection);
    let layout = value_layout(
        ProjectionScope {
            value,
            ancestors,
            ..scope
        },
        current_projection,
        &child_projection.partial,
        writable,
    );
    let prepared = match layout {
        None => {
            // A document's root is where the document starts: its hole
            // leads with what the libraries on here start one as.
            let picker = if path.is_empty() {
                crate::display::raw::start_picker()
            } else {
                crate::display::raw::picker(crate::display::CompletionKind::Value)
            };
            ChoiceLayout::fixed(crate::display::widget::picker::hole(
                &Occurrence {
                    cx,
                    path,
                    ancestors,
                    state: scope.state,
                },
                tcx,
                &picker,
            ))
        }
        Some(layout) => {
            let inner = prepare(
                &ProjectionScope {
                    projection: child_projection,
                    ancestors,
                    value,
                    ..scope
                },
                tcx,
                layout,
                build,
            );
            if part {
                let selected = cx.selected(path);
                let scale = cx.styles.scale;
                let palette = cx.styles.palette;
                let path = path.clone();
                let select = crate::display::widget::navigation::arrival(cx.site(path.clone()));
                ChoiceLayout::map(inner, move |inner| {
                    crate::display::widget::navigation::part_landmark(
                        highlighted(palette, selected, scale, path.clone(), None, inner),
                        path,
                        select,
                        selected,
                    )
                })
            } else {
                let landmark_path = path.clone();
                // What this projection shares with others of the same location;
                // the frame marks those sharing the selected or hovered one's.
                // Where the path past the nearest followed cell, or from the root,
                // is the data's own, it says what's shared without reading it.
                // An empty place shares its location too, so a hole drawn in two
                // places is one hole.
                let descends = path.data_from(ancestors.enclosing.map_or(0, |(_, start)| start));
                let cell = value.and_then(grap::RuntimeValue::as_cell);
                let secondary = if descends {
                    Some(Secondary::from_context(
                        landmark_path.clone(),
                        cell,
                        ancestors.enclosing,
                    ))
                } else if let Some(cell) = cell {
                    // A reference names its cell even in a computed result.
                    Some(Secondary::Cell(cell))
                } else {
                    path.location().map(|location| {
                        Secondary::from_location(&cx.sources, &location, path.view())
                    })
                };
                // A landmark, not a target: highlight and keyboard reach span
                // the full bounds, while clicks belong to the content each arm
                // claimed above — structural whitespace deselects.
                let selected = cx.selected(path);
                let selected_value = value.filter(|_| selected).cloned();
                let scale = cx.styles.scale;
                let palette = cx.styles.palette;
                let select =
                    crate::display::widget::navigation::arrival(cx.site(landmark_path.clone()));
                let target = value.map(|value| {
                    (
                        value.clone(),
                        crate::display::widget::navigation::selecting(
                            cx.site(landmark_path.clone()),
                        ),
                    )
                });
                ChoiceLayout::map(inner, move |inner| {
                    let placed = crate::display::widget::navigation::landmark(
                        highlighted(
                            palette,
                            selected,
                            scale,
                            landmark_path.clone(),
                            secondary.clone(),
                            inner,
                        ),
                        landmark_path.clone(),
                        select,
                        secondary,
                        selected,
                    );
                    let placed = match selected_value {
                        Some(value) => bind_selection(placed, landmark_path.clone(), value, scale),
                        None => placed,
                    };
                    match target {
                        Some((value, select)) => pick_target(landmark_path, value, select, placed),
                        None => placed,
                    }
                })
            }
        }
    };
    let scale = cx.styles.scale;
    let palette = cx.styles.palette;
    match changed_ground {
        // Read-only appearance; document permissions remain in the editing
        // scope.
        Some(Ground::Readonly) => ChoiceLayout::map(prepared, move |content| {
            ground_with(scale, palette.readonly_ground, content)
        }),
        Some(Ground::Writable) => ChoiceLayout::map(prepared, move |content| {
            ground_with(scale, palette.paper, content)
        }),
        None => prepared,
    }
}

/// What the occurrence `scope` describes draws as, its own projection's
/// layout, else Raw's.
fn value_layout(
    scope: ProjectionScope<'_, '_>,
    current_projection: Option<&Partial>,
    default_projection: &Partial,
    writable: bool,
) -> Option<Layout> {
    let ProjectionScope {
        cx,
        projection,
        path,
        ancestors,
        state,
        value,
    } = scope;
    #[cfg(all(test, feature = "layout-profile"))]
    let _profile = crate::display::profile::enter(crate::display::profile::Kind::Projection);
    let selection = cx
        .selection
        .filter(|current| *path == current.path())
        .map(Selection::payload);
    let pending = if cx.pending_edge_under(path).is_some() {
        Some(crate::display::Pending::Field)
    } else {
        cx.pending_child_of(path)
            .and_then(|step| step.data())
            .map(crate::display::Pending::Child)
    };
    let occurrence = || cx.site(path.clone());
    let input = crate::display::ProjectionInput {
        env: &Occurrence {
            cx,
            path,
            ancestors,
            state,
        },
        default_projection: default_projection.clone(),
        provided: projection.provided.clone(),
        value,
        scale_factor: cx.styles.scale,
        writable,
        selection: selection.as_ref(),
        pending,
        state: state.and_then(Node::value),
        occurrence: &occurrence,
    };
    match current_projection {
        Some(current) => current(&input),
        None => projection.apply(&input),
    }
    .or_else(|| crate::display::raw::view(&input))
}

/// Every projected value's Pick backstop: pick the value into an open
/// pending, or — nothing pending — select it like Activate, so a stray
/// modifier never deadens the gesture. Inner Pick registrations answer
/// first and this catches what they refused.
fn pick_target(
    path: Trail,
    value: grap::RuntimeValue,
    select: crate::display::Action,
    child: Measured<HoverPass<crate::Editor>>,
) -> Measured<HoverPass<crate::Editor>> {
    before(child, move |p, _| {
        let value = value.clone();
        let select = select.clone();
        p.pick(Hovered::Tree(Hover::Value(path.clone())), move |world| {
            if !crate::display::widget::interaction::pick(world, &value) {
                select(&mut world.session());
            }
            true
        });
    })
}
