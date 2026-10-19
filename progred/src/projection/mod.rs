//! The editor's projection runtime: resolve locations, supply borrowed widget inputs,
//! retain source provenance, and fall back to total structural display.

mod child;
mod fold;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
pub(crate) mod viewport;

use crate::annotations::Annotations;
#[cfg(test)]
use crate::display::widget::navigation::Landmark;
use crate::display::widget::placed::{before, decorate};
use crate::display::widget::source::{Secondary, SourceTrace};
use crate::display::widget::style::{
    Styles, highlight_outline, hover_highlight, primary_highlight, secondary_highlight,
};
use crate::frame::{Attribution, HoverPass, Hovered};
use crate::hover::Hover;
use crate::location::{Location, Start};
use crate::selection::{Selection, Stage};
use crate::sources::Sources;
use gid::{CellId, Path, Step, Value};
#[cfg(test)]
use kurbo::Point;
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
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

/// The toolkit's types over the editor, which hosts everything the runtime
/// projects. Display and the libraries stay generic over their host, through
/// [`crate::display::host`].
pub(crate) type Layout = crate::display::host::Layout<crate::Editor>;
pub(crate) type Partial = crate::display::host::Partial<crate::Editor>;
#[cfg(test)]
pub(crate) type ProjectionInput<'a, V = Value> =
    crate::display::host::ProjectionInput<'a, crate::Editor, V>;

type SharedPath = Rc<[Step]>;

/// One ordered composition of partial value projections. The
/// structural fallback lives in this runtime and is always total.
#[derive(Clone)]
pub struct Projection {
    partial: Partial,
    entry: Option<Partial>,
}

impl Default for Projection {
    fn default() -> Self {
        Self {
            partial: crate::display::runtime_partial(|_| None),
            entry: None,
        }
    }
}

impl Projection {
    pub fn new(partials: impl IntoIterator<Item = Partial>) -> Self {
        Self {
            partial: crate::display::compose_partials(partials),
            entry: None,
        }
    }

    /// The composed partials, without the entry.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn partial(&self) -> &Partial {
        &self.partial
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
    pub(crate) view: &'a crate::display::widget::view::Root,
    pub(crate) completions: Option<&'a crate::display::CompletionProvider>,
    /// The reading context: the document read over its library.
    pub(crate) sources: Sources<'a>,
    pub(crate) edits: crate::editing::Scope,
    /// Names and field order derive from this view bit.
    /// Raw opens only the cells it reaches directly: references inside a
    /// cell's definition start folded, so a call doesn't inline its callee
    /// and a shared cell isn't expanded again at every use. A cell that is
    /// only a reference to another opens through to it.
    pub(crate) raw: bool,
    pub(crate) annotations: &'a Annotations,
    pub(crate) styles: &'a Styles,
    pub(crate) selection: Option<&'a Selection>,
    /// The selected structural source, normalized across projections
    /// for execution-linked output.
    pub(crate) selected_trace: Option<SourceTrace>,
}

/// One occurrence in a view, as the widgets drawing it read the view.
pub(crate) struct Occurrence<'a> {
    pub(crate) cx: &'a Cx<'a>,
    pub(crate) path: &'a [Step],
}

impl Occurrence<'_> {
    fn below(&self, steps: &[Step]) -> Path {
        self.path.iter().chain(steps).cloned().collect()
    }
}

impl crate::display::Viewing for Occurrence<'_> {
    fn scale(&self) -> f64 {
        self.cx.styles.scale
    }

    fn writable(&self) -> bool {
        self.cx.edits.view(self.cx.sources).writable(self.path)
    }

    fn host(&self) -> &dyn grap::Host {
        &self.cx.sources
    }

    fn computations(&self) -> &crate::computations::Computations {
        self.cx.computations
    }

    fn state_below(&self, steps: &[Step]) -> Option<&Value> {
        self.cx.annotations.at(&self.below(steps))
    }

    fn selected_below(&self, steps: &[Step]) -> bool {
        let below = self.below(steps);
        self.cx
            .selection
            .is_some_and(|selection| selection.path().starts_with(&below))
    }
}

impl crate::display::Toolkit for Occurrence<'_> {
    fn path(&self) -> &[Step] {
        self.path
    }

    fn view(&self) -> &crate::display::widget::view::Root {
        self.cx.view
    }

    fn scope(&self) -> &crate::editing::Scope {
        &self.cx.edits
    }

    fn site(&self) -> crate::editing::Site {
        self.cx.site(Rc::from(self.path))
    }

    fn styles(&self) -> &Styles {
        self.cx.styles
    }

    fn sources(&self) -> Sources<'_> {
        self.cx.sources
    }

    fn location(&self) -> Option<Location<'_>> {
        self.cx.edits.location(self.path)
    }

    fn focused(&self) -> bool {
        self.cx.focused
    }

    fn raw(&self) -> bool {
        self.cx.raw
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

    fn name(&self, cell: CellId) -> Option<&str> {
        crate::libraries::name::of(&self.cx.sources, cell)
    }

    fn selected_trace(&self) -> Option<&SourceTrace> {
        self.cx.selected_trace.as_ref()
    }

    fn trace_below(&self, steps: &[Step]) -> Option<SourceTrace> {
        self.cx
            .edits
            .location(&self.below(steps))
            .map(|location| SourceTrace::from_location(&self.cx.sources, &location))
    }
}

#[derive(Clone, Default)]
struct Ancestry {
    /// Cells crossed by `Follow`, used to stop projection cycles.
    cells: HashSet<CellId>,
    /// The nearest followed definition and the start of its relative path.
    enclosing: Option<(CellId, gid::Resolution, usize)>,
    ground: Ground,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Ground {
    #[default]
    Writable,
    Readonly,
}

struct ProjectEnv<'a, 's> {
    cx: &'a Cx<'s>,
    path: &'a [Step],
}

impl ProjectEnv<'_, '_> {
    /// Where what Grap evaluates `steps` below this occurrence comes from.
    fn origin(&self, steps: &[Step]) -> Option<grap::SourceOrigin> {
        crate::display::Toolkit::trace_below(
            &Occurrence {
                cx: self.cx,
                path: self.path,
            },
            steps,
        )
        .map(crate::hover::to_grap)
    }
}

impl crate::display::Env for ProjectEnv<'_, '_> {
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
        steps: &[Step],
    ) -> grap::RuntimeValue {
        grap::evaluate_runtime_at(expression, self.origin(steps), &self.cx.sources, fuel).result
    }
    fn evaluate_runtime_memo(
        &self,
        expression: &grap::RuntimeValue,
        fuel: usize,
        steps: &[Step],
    ) -> grap::RuntimeValue {
        self.cx.computations.evaluate_runtime(
            self.cx.view,
            self.path,
            expression,
            fuel,
            self.origin(steps),
        )
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
            .apply_runtime(self.cx.view, self.path, function, arguments, fuel)
    }

    fn name(&self, cell: CellId) -> Option<&str> {
        self.cx.name(cell)
    }

    fn resolve(&self, cell: CellId) -> Option<crate::display::ResolvedCell<'_>> {
        self.cx.sources.resolve(cell)
    }

    fn libraries(&self) -> Vec<crate::display::DocumentLibrary> {
        crate::workspace::libraries(&self.cx.sources)
    }

    fn occurrence(&self) -> &[Step] {
        self.path
    }
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
            },
            value: scope.value,
        },
        build,
    )
}

/// One occurrence being projected: its view's context, the projection it
/// draws with, its path, its ancestry, and its value.
#[derive(Clone, Copy)]
struct ProjectionScope<'a, 's> {
    cx: &'a Cx<'s>,
    projection: &'a Projection,
    path: &'a [Step],
    ancestors: &'a Ancestry,
    value: Option<&'a grap::RuntimeValue>,
}

impl ProjectionScope<'_, '_> {
    /// Adds GID steps to the occurrence and resolves through its conject.
    /// Current and descendant projections are independent replacements.
    /// Missing locations use the standard empty picker, without a value.
    fn descend_path(
        &self,
        tcx: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
        steps: &[Step],
        current: Option<Partial>,
        default: Option<Partial>,
    ) -> ChoiceLayout<HoverPass<crate::Editor>> {
        let cx = self.cx;
        let mut path = self.path.to_vec();
        let mut ancestors = self.ancestors.clone();
        let mut value = self.value.cloned();
        let mut edits = None;
        for step in steps {
            let followed = match step {
                Step::Follow(source) => value
                    .as_ref()
                    .and_then(grap::RuntimeValue::as_cell)
                    .map(|cell| (cell, *source)),
                Step::Key(_) | Step::Element(_) => None,
            };
            path.push(step.clone());
            if let Some((cell, source)) = followed {
                ancestors.cells.insert(cell);
                ancestors.enclosing = Some((cell, source, path.len()));
                // Following a cell starts a location at its definition, whatever
                // the route here, computed values included.
                edits = Some(edits.as_ref().unwrap_or(&cx.edits).in_cell(
                    path.clone(),
                    cell,
                    source,
                ));
            }
            let scope = edits.as_ref().unwrap_or(&cx.edits);
            value = match (!scope.descends()).then(|| scope.location(&path)).flatten() {
                Some(location) => cx.sources.read(&location).map(grap::RuntimeValue::from),
                None => value.as_ref().and_then(|value| {
                    child::of(value, step, |cell, source| cx.sources.value(cell, source))
                }),
            };
        }
        let scoped = edits.map(|edits| Cx {
            edits,
            ..cx.clone()
        });
        prepare_value(
            ProjectionScope {
                cx: scoped.as_ref().unwrap_or(cx),
                path: &path,
                ancestors: &ancestors,
                value: value.as_ref(),
                ..*self
            },
            current.as_ref(),
            default.as_ref(),
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
        steps: &[Step],
        current: Option<Partial>,
        default: Option<Partial>,
    ) -> ChoiceLayout<HoverPass<crate::Editor>> {
        self.descend_path(text, build, steps, current, default)
    }
    #[cfg(any(test, target_arch = "wasm32"))]
    fn descend_raw(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
        step: Step,
    ) -> ChoiceLayout<HoverPass<crate::Editor>> {
        let cx = Cx {
            raw: true,
            ..self.cx.clone()
        };
        let nothing = crate::display::partial(|_| None);
        ProjectionScope { cx: &cx, ..*self }.descend_path(
            text,
            build,
            std::slice::from_ref(&step),
            Some(nothing.clone()),
            Some(nothing),
        )
    }
    fn jump(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
        steps: Vec<Step>,
        document: Location<'static>,
        current: Option<Partial>,
        default: Option<Partial>,
    ) -> ChoiceLayout<HoverPass<crate::Editor>> {
        let path: Vec<_> = self.path.iter().cloned().chain(steps).collect();
        let cx = Cx {
            edits: self.cx.edits.located(path.clone(), document),
            ..self.cx.clone()
        };
        let value = cx
            .edits
            .view(cx.sources)
            .resolve_path(&path)
            .cloned()
            .map(grap::RuntimeValue::from);
        let mut ancestry = self.ancestors.clone();
        // Following the same source through jumps must not bypass cycle checks.
        if let Some(location) = cx.edits.location(&path) {
            ancestry.cells.extend(
                match location.start {
                    Start::Cell { cell, .. } => Some(cell),
                    Start::Root => None,
                }
                .into_iter()
                .chain(
                    location
                        .path
                        .iter()
                        .enumerate()
                        .filter(|(_, step)| matches!(step, Step::Follow(_)))
                        .filter_map(|(i, _)| {
                            cx.sources
                                .read_from(location.start, &location.path[..i])
                                .and_then(Value::as_cell)
                        }),
                ),
            );
        }
        prepare_value(
            ProjectionScope {
                cx: &cx,
                path: &path,
                ancestors: &ancestry,
                value: value.as_ref(),
                ..*self
            },
            current.as_ref(),
            default.as_ref(),
            text,
            build,
        )
    }
    fn at(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
        steps: Vec<Step>,
        value: grap::RuntimeValue,
        current: Option<Partial>,
        default: Option<Partial>,
    ) -> ChoiceLayout<HoverPass<crate::Editor>> {
        let path: Vec<_> = self.path.iter().cloned().chain(steps).collect();
        let cx = Cx {
            edits: self.cx.edits.detached(path.clone()),
            ..self.cx.clone()
        };
        prepare_value(
            ProjectionScope {
                cx: &cx,
                path: &path,
                ancestors: &Ancestry {
                    ground: self.ancestors.ground,
                    ..Ancestry::default()
                },
                value: Some(&value),
                ..*self
            },
            current.as_ref(),
            default.as_ref(),
            text,
            build,
        )
    }
}

/// The resolved hover's tree identity, for ink that lights its own
/// claim.
fn tree_hovered(hover: &Attribution) -> Option<&Hover> {
    match hover.hovered.as_ref() {
        Some(Hovered::Tree(hover)) => Some(hover),
        _ => None,
    }
}

/// Select `path` in this view, interpreted through this occurrence's scope.
pub(crate) fn select_handler(path: SharedPath, cx: &Cx) -> crate::display::Action {
    crate::display::widget::navigation::selecting(cx.site(path))
}

impl Cx<'_> {
    /// The occurrence at `path` in this view, as a place to act.
    pub(crate) fn site(&self, path: SharedPath) -> crate::editing::Site {
        crate::editing::Site::new(self.view.clone(), path, self.edits.clone())
    }

    /// The display name at this projection. Raw interprets no naming
    /// convention and therefore falls back to the short id.
    fn name(&self, cell: CellId) -> Option<&str> {
        (!self.raw)
            .then(|| crate::libraries::name::of(&self.sources, cell))
            .flatten()
    }

    /// Whether `path` carries the primary highlight. A label-stage
    /// pending deliberately does not mark its parent — nothing is
    /// selected there, something is being authored inside; the
    /// pending row carries the highlight itself.
    pub(crate) fn selected(&self, path: &[Step]) -> bool {
        self.selection.is_some_and(|current| {
            current.path() == path && current.stage(&self.sources) != Stage::Label
        })
    }

    /// Whether the selection is a picker at `path` itself. Over an existing
    /// value it is replacing that value, so views draw the hole instead.
    fn pending_at(&self, path: &[Step]) -> bool {
        self.selection.is_some_and(|current| {
            current.path() == path && current.explicit_stage() == Stage::Pending
        })
    }

    /// The pending child step under `path`, when the selection is
    /// authoring one there.
    fn pending_child_of(&self, path: &[Step]) -> Option<Step> {
        let current = self.selection?;
        (current
            .path()
            .split_last()
            .is_some_and(|(_, parent)| parent == path)
            && current.stage(&self.sources) == Stage::Pending)
            .then(|| current.path().last().cloned())
            .flatten()
    }

    /// The label query of a new field being authored on the record at
    /// `path`.
    pub(crate) fn pending_edge_under(&self, path: &[Step]) -> Option<&LineEditState> {
        let current = self.selection?;
        (current.path() == path && current.stage(&self.sources) == Stage::Label)
            .then(|| current.edit())
            .flatten()
    }
}

/// The explicit-state boundary: everything a projection pass reads.
/// `width` is the space the projection may fill. `root` and
/// `root_path` let an editor pane begin at a value occurrence
/// while retaining ordinary document-relative interaction paths.
pub struct ProjectDescription<'a> {
    pub computations: &'a crate::computations::Computations,
    pub focused: bool,
    pub view: &'a crate::display::widget::view::Root,
    pub completions: Option<&'a crate::display::CompletionProvider>,
    pub sources: Sources<'a>,
    pub root: Option<&'a Value>,
    pub root_path: &'a [Step],
    /// Selection belonging to this editable view.
    pub selection: Option<&'a Selection>,
    /// Selection from any view, used only to link generated output
    /// back to its structural source.
    pub source_selection: Option<&'a Selection>,
    pub annotations: &'a Annotations,
    pub raw: bool,
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
        view,
        completions,
        sources,
        root,
        root_path,
        selection,
        source_selection,
        annotations,
        raw,
        styles,
        width: _,
        projection,
    } = description;
    // A view reached through a cell starts its locations at the last one it
    // follows, as following that cell in the view would.
    let followed = Location::root(root_path)
        .stored(&sources)
        .and_then(|stored| match stored.start {
            Start::Cell { cell, resolution } => {
                Some((cell, resolution, root_path.len() - stored.path.len()))
            }
            Start::Root => None,
        });
    let cx = Cx {
        computations,
        focused,
        view,
        completions,
        sources,
        edits: followed.map_or_else(Default::default, |(cell, source, start)| {
            crate::editing::Scope::default().in_cell(root_path[..start].to_vec(), cell, source)
        }),
        raw,
        annotations,
        styles,
        selection,
        selected_trace: source_selection.and_then(|selection| {
            Some(SourceTrace::from_location(&sources, &selection.location()?))
        }),
    };
    // An empty document is a selectable placeholder at the root path.
    let mut build = ChoiceBuild::default();
    let ancestry = followed.map_or_else(Ancestry::default, |(cell, source, start)| Ancestry {
        cells: HashSet::from([cell]),
        enclosing: Some((cell, source, start)),
        ..Ancestry::default()
    });
    let layout = prepare_value(
        ProjectionScope {
            cx: &cx,
            projection,
            path: root_path,
            ancestors: &ancestry,
            value: root.cloned().map(grap::RuntimeValue::from).as_ref(),
        },
        None,
        None,
        tcx,
        &mut build,
    );
    build.finish(layout)
}

/// Marks `child` as the projection of `path` WITHOUT claiming any
/// clicks: the highlight, reveal rect, and keyboard reach of
/// [`descend`] over the full bounds, while pointer selection belongs
/// to the content targets the view registers — heads, delimiters,
/// rows — so clicks on structural whitespace (gutters, inter-row
/// gaps, the dead space inside a bounding box) fall through to the
/// background's deselect.
#[allow(clippy::too_many_arguments)]
fn descend_landmark_with(
    palette: crate::display::widget::style::Palette,
    selected: bool,
    scale: f64,
    path: SharedPath,
    secondary: Option<Secondary>,
    select: crate::display::widget::navigation::Select<crate::Editor>,
    scope: crate::editing::Scope,
    child: Measured<HoverPass<crate::Editor>>,
) -> Measured<HoverPass<crate::Editor>> {
    let highlight_path = path.clone();
    let mark = secondary.clone();
    let marked = decorate(child, move |p, rect| {
        let highlight_path = highlight_path.clone();
        let outline = highlight_outline(scale, rect);
        p.render(move |cv, hover| {
            let shares = |other: &Option<Secondary>| mark.is_some() && *other == mark;
            if selected {
                primary_highlight(palette, scale, cv, outline);
            } else if shares(&hover.selected_secondary) {
                secondary_highlight(palette, scale, cv, outline, true);
            } else if matches!(
                tree_hovered(hover),
                Some(Hover::Value(hovered)) if hovered.as_ref() == highlight_path.as_ref()
            ) {
                hover_highlight(palette, scale, cv, outline);
            } else if shares(&hover.hovered_secondary) {
                secondary_highlight(palette, scale, cv, outline, false);
            }
        });
    });
    crate::display::widget::navigation::landmark(marked, path, select, scope, secondary, selected)
}

/// Standard commands for this displayed value, below its own widget handlers:
/// copy and cut, folding when it folds, and deleting it.
/// The occurrence identifies the selection; only mutations need its conject.
fn bind_selection(
    child: Measured<HoverPass<crate::Editor>>,
    root: crate::display::widget::view::Root,
    path: SharedPath,
    value: grap::RuntimeValue,
    fold_default: Option<bool>,
    scale: f64,
) -> Measured<HoverPass<crate::Editor>> {
    before(child, move |p, _| {
        let command = p.command_modifier();
        p.handler().on_key_with(move |editor, event, input| {
            let copy = crate::modifiers::chord(command, event, "c");
            let cut = crate::modifiers::chord(command, event, "x");
            event.state.is_down()
                && editor.model.selection.as_ref().is_some_and(|selection| {
                    selection.root() == &root
                        && selection.path() == path.as_ref()
                        && selection.stage(&editor.sources()) == Stage::Edge
                })
                && if copy || cut {
                    editor.copy_value(value.as_value()) && {
                        if cut {
                            editor.delete_selected_edge(input.geometry(scale));
                        }
                        true
                    }
                } else {
                    match fold_default.zip(fold_key(command, event)) {
                        Some((default, closed)) => editor
                            .session()
                            .set_collapsed(&root, &path, default, closed),
                        None => editor.delete_key(input.geometry(scale), event),
                    }
                }
        })
    })
}

/// The fold a key asks of a selected value: Space toggles it, and the
/// command modifier with Up closes it and with Down opens it.
fn fold_key(
    command: puri::keyboard::CommandModifier,
    event: &KeyboardEvent,
) -> Option<Option<bool>> {
    match &event.key {
        Key::Character(key) if key.as_str() == " " => Some(None),
        Key::Named(NamedKey::ArrowUp) if command.pressed(&event.modifiers) => Some(Some(true)),
        Key::Named(NamedKey::ArrowDown) if command.pressed(&event.modifiers) => Some(Some(false)),
        _ => None,
    }
}

fn ground(cx: &Cx, source: Option<&Location>, value: Option<&grap::RuntimeValue>) -> Ground {
    match (source, value.and_then(grap::RuntimeValue::as_cell)) {
        (None, _) => Ground::Readonly,
        // A reference remains replaceable; the ground describes its definition.
        (Some(_), Some(cell)) if cx.sources.external(cell) => Ground::Readonly,
        (Some(_), Some(_)) => Ground::Writable,
        (Some(location), None) if crate::location::writable(&cx.sources, location) => {
            Ground::Writable
        }
        (Some(_), None) => Ground::Readonly,
    }
}

/// Read-only appearance; document permissions remain in the editing scope.
fn readonly(
    scale: f64,
    palette: crate::display::widget::style::Palette,
    content: Measured<HoverPass<crate::Editor>>,
) -> Measured<HoverPass<crate::Editor>> {
    ground_with(scale, palette.readonly_ground, content)
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
/// given, its children with `default_projection`.
fn prepare_value(
    scope: ProjectionScope<'_, '_>,
    current_projection: Option<&Partial>,
    default_projection: Option<&Partial>,
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
    let source = cx.edits.location(path);
    let ground = ground(cx, source.as_ref(), value);
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
        })
        .or_else(|| {
            (projection.entry.is_some() && value.and_then(grap::RuntimeValue::as_cell).is_none())
                .then(|| projection.without_entry())
        });
    let child_projection = child_projection.as_ref().unwrap_or(projection);
    // Ancestry is already available from projection; folds must not re-read an
    // occurrence as a document path (and computed values have no such path).
    let fold_default = value.and_then(|value| {
        let folded = value.as_cell().is_some_and(|cell| {
            ancestors.cells.contains(&cell)
                || cx.raw
                    && ancestors
                        .enclosing
                        .is_some_and(|(_, _, start)| path.len() > start)
        });
        crate::selection::collapse_default_for_value(&cx.sources, value, folded)
    });
    let layout = value_layout(
        ProjectionScope { value, ..scope },
        current_projection,
        &child_projection.partial,
        source.as_ref(),
        fold_default,
    );
    let prepared = match layout {
        None => {
            // A document's root is where the document starts: outside Raw,
            // its hole leads with what the libraries start one as.
            let picker = if path.is_empty() && !cx.raw {
                crate::display::raw::start_picker()
            } else {
                crate::display::raw::picker(crate::display::CompletionKind::Value)
            };
            ChoiceLayout::fixed(crate::display::widget::picker::hole(
                &Occurrence { cx, path },
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
            let landmark_path: SharedPath = Rc::from(path);
            // What this projection shares with others of the same location;
            // the frame marks those sharing the selected or hovered one's.
            let secondary = value.and_then(|value| {
                Some(if cx.edits.descends() {
                    Secondary::from_context(
                        landmark_path.clone(),
                        value.as_cell(),
                        ancestors.enclosing,
                    )
                } else if let Some(cell) = value.as_cell() {
                    // A reference names its cell even in a computed result.
                    Secondary::Cell(cell)
                } else {
                    Secondary::from_location(&cx.sources, source.as_ref()?)
                })
            });
            // A landmark, not a target: highlight and keyboard reach span
            // the full bounds, while clicks belong to the content each arm
            // claimed above — structural whitespace deselects.
            let selected = cx.selected(path);
            let selected_value = value.filter(|_| selected).cloned();
            let root = cx.view.clone();
            let scale = cx.styles.scale;
            let palette = cx.styles.palette;
            let select =
                crate::display::widget::navigation::arrival(cx.site(landmark_path.clone()));
            let target =
                value.map(|value| (value.clone(), select_handler(landmark_path.clone(), cx)));
            let edits = cx.edits.clone();
            ChoiceLayout::map(inner, move |inner| {
                let placed = descend_landmark_with(
                    palette,
                    selected,
                    scale,
                    landmark_path.clone(),
                    secondary,
                    select,
                    edits.clone(),
                    inner,
                );
                let placed = match selected_value {
                    Some(value) => bind_selection(
                        placed,
                        root,
                        landmark_path.clone(),
                        value,
                        fold_default,
                        scale,
                    ),
                    None => placed,
                };
                match target {
                    Some((value, select)) => pick_target_with(landmark_path, value, select, placed),
                    None => placed,
                }
            })
        }
    };
    let scale = cx.styles.scale;
    let palette = cx.styles.palette;
    match changed_ground {
        Some(Ground::Readonly) => {
            ChoiceLayout::map(prepared, move |content| readonly(scale, palette, content))
        }
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
    source: Option<&Location>,
    fold_default: Option<bool>,
) -> Option<Layout> {
    let ProjectionScope {
        cx,
        projection,
        path,
        value,
        ..
    } = scope;
    #[cfg(all(test, feature = "layout-profile"))]
    let _profile = crate::display::profile::enter(crate::display::profile::Kind::Projection);
    if let Some(value) = value
        && let Some(default) = fold_default
        && crate::annotations::folded(cx.annotations.at(path), default)
        && let Some(collapsed) = fold::collapsed_layout(cx, path, value, default)
    {
        return Some(collapsed);
    }
    let selection = cx
        .selection
        .filter(|current| current.path() == path)
        .map(Selection::payload);
    let pending = if cx.pending_edge_under(path).is_some() {
        Some(crate::display::Pending::Field)
    } else {
        cx.pending_child_of(path)
            .map(crate::display::Pending::Child)
    };
    let state = cx.annotations.at(path);
    let occurrence = || cx.site(Rc::from(path));
    let input = crate::display::ProjectionInput {
        env: &ProjectEnv { cx, path },
        default_projection: default_projection.clone(),
        value,
        scale_factor: cx.styles.scale,
        writable: source.is_some_and(|source| crate::location::writable(&cx.sources, source)),
        selection: selection.as_ref(),
        pending,
        state,
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
fn pick_target_with(
    path: SharedPath,
    value: grap::RuntimeValue,
    select: crate::display::Action,
    child: Measured<HoverPass<crate::Editor>>,
) -> Measured<HoverPass<crate::Editor>> {
    before(child, move |p, _| {
        let value = value.clone();
        let select = select.clone();
        p.pick(Hovered::Tree(Hover::Value(path.clone())), move |world| {
            let mut session = world.session();
            if !session
                .pending()
                .is_some_and(|picking| picking.pick(value.to_value()))
            {
                select(&mut session);
            }
            true
        });
    })
}
