# Projection interface design notes, 17 October 2026

These notes design the three larger pieces of the interface the owner wants
projections and libraries to have: a stated subset of the editor rather than
the whole editor (see [deferred work](../deferred.md#code-size-and-structure),
item 3). A subagent drafted them at `67456161` without changing code; the key
claims were spot-checked. They are proposals for the owner to veto, not
decisions; [the decisions they raise](#decisions-for-the-owner) are listed at the
end. Paths are relative to progred/src unless they start with ui/, docs/,
examples/ or website/.

Corrections to the brief they were drafted from:
- Counting reads that span lines, library widgets read the per-view bag's
  `edits` 10 times and `sources` 9, not 7 and 10. Of the 16 `styles` reads, 13
  are `scale`.
- A layout program's `canvas` operation (libraries/layout/scope.rs:328-348)
  also runs drawing programs, beside the stored drawing form.
- The CAM previews don't run drawing programs. The canaries that do are
  `iop_tree` and the two `forest` ones.

## 1. Handlers get capabilities, not the editor

### Today

- **The widget context hands over the engine's whole per-view bag.** `Context`
  (display/widget.rs:52-58) carries `inputs: &projection::Cx`
  (projection/mod.rs:112-136). Library widgets read its `view` 15 times,
  `styles` 16 (13 of them `scale`), `edits` 10, `sources` 9, `computations` 5
  and `command_modifier` 2. The presentation outline reads the whole bag
  twice, for `sources`, `selection` and `annotations`.
- **Handlers get the concrete editor.** Puri boxes `Fn(&mut C, Event, &mut P)`
  with `C` sized (ui/puri/src/handler.rs:131-134), and `C` is `Editor`, with 27
  fields (editor.rs:91-142). Display and the libraries use it at 36 sites:

| Use | Sites |
|---|---|
| 16 scoped edits, `scope.open(Access::new(world))` | per-path state, line edits, `replace`, and Grap handlers |
| 5 `editing::start_gesture` | drags in display, controls, tree ranges and number scrubbing |
| 3 `editing::commit_*` | the libraries' completion and Grap |
| 4 selection reads or writes | color, number, completion, scrubbing |
| 2 `pick_identity`, 1 `author_in`, 1 `set_collapsed` | display and the outline |
| 2 `command_modifier`, 2 `timers` | display, line edits, Fidget |

- **Offers, gestures and the model get it too.** `Completion::activate` is
  `Rc<dyn Fn(&mut crate::Editor)>` (display/mod.rs:81). Four gestures
  implement `Gesture<Editor>`. Behind `Access`, model code gets the whole
  editor, to refresh the window title.

### Design

Display and the libraries are generic over `W: Host` in their types only.
Everything a handler, gesture or offer does goes through one non-generic struct
of capability borrows.

```rust
// core: editing/session.rs. A struct, not the trait the crate split sketched.
pub struct Session<'a> {                       // fields private to core
    model: &'a mut Model,
    libraries: &'a Libraries,
    gesture: &'a mut Option<Box<dyn Gesture>>,
    timers: &'a mut Timers,
    text: TextTools<'a>,                       // fonts, layouts, &mut dyn TextClipboard
}
impl<'a> Session<'a> {
    pub fn env(&self) -> impl Env + grap::Host + '_;      // dispatch-time reads
    pub fn timers(&mut self) -> &mut dyn puri::timer::Timers;
    pub fn start_gesture(&mut self, gesture: Box<dyn Gesture>, samples: &[Point]);
    pub fn pending(&mut self) -> Option<Picking<'_, 'a>>; // the open picker (note 2)
}
/// One projected occurrence as a target: its view, occurrence path and scope. Opaque.
#[derive(Clone)] pub struct Site { root: Root, path: Rc<[Step]>, scope: Scope }
impl Site {
    pub fn below(&self, steps: &[Step]) -> Site;
    pub fn hover<H: From<Hover>>(&self) -> H;
    pub fn open<'s>(&'s self, session: &'s mut Session<'_>) -> Edit<'s>;
    pub fn edit_run(&self) -> EditRun;
}
impl Edit<'_> {                                           // bound to one site
    pub fn value(&self) -> Option<&Value>;   pub fn replace(&mut self, value: Value) -> bool;
    pub fn state(&self) -> Option<&Value>;   pub fn set_state(&mut self, state: Value) -> bool;
    pub fn fold(&mut self, default: bool, closed: Option<bool>) -> bool; // one convention of state
    pub fn select(&mut self);  pub fn select_with(&mut self, payload: Value);  pub fn pend(&mut self);
    pub fn selected(&self) -> Selected<'_>;               // Elsewhere | Here | Typing(&LineEditState)
    pub fn edit_line(&mut self, line: &LineEdit, operation: &EditOperation<'_>) -> bool;
    pub fn run_handler(&mut self, function: RuntimeValue, event: Value) -> bool;
    pub fn author(&mut self, geometry: Geometry<'_>, event: &KeyboardEvent) -> bool;
}
pub type Action = Rc<dyn Fn(&mut Session<'_>) -> bool>;  // was ActionHandler<World>
pub trait Gesture {
    fn advance(&mut self, session: &mut Session<'_>, samples: &[Point]) -> bool;
    fn finish(&mut self, _session: &mut Session<'_>) {}
    fn cursor(&self) -> Option<Cursor> { None }
}
// Completion::activate: Rc<dyn Fn(&mut Session<'_>)>, so Completion, its provider and
// offers::Entry compile once.

// core: display/host.rs
pub trait Host: 'static {
    type Hover: Clone + PartialEq + From<Hover> + 'static; // the shell's Hovered keeps its menu arm
    fn tree(hover: &Self::Hover) -> Option<&Hover>;
    fn session(&mut self) -> Session<'_>;
}
pub type View<W> = Layout<W, <W as Host>::Hover>;
```

For example, controls.rs:199-217 becomes:

```rust
Rc::new(move |session, site, value| {
    let mut edit = site.open(session);
    let state = set_state(edit.state(), key, value);
    edit.set_state(state);
})
```

The widget `Context` replaces `inputs` and `path` with `toolkit: &dyn Toolkit`
(`pub(crate)`). The runtime's `ProjectionScope` implements it, and libraries
reach only the supertrait:

```rust
pub trait Viewing {                        // what a view reads while it's built
    fn env(&self) -> &dyn Env;             // gains host(): &dyn grap::Host
    fn computations(&self) -> &Computations;  // always supplied; memo roots keyed by &Site
    fn site(&self) -> Site;                // built on demand
    fn scale(&self) -> f64;  fn writable(&self) -> bool;  fn focused(&self) -> bool;
    fn selection(&self) -> Option<Value>;  // the payload iff selected here
    fn state_below(&self, steps: &[Step]) -> Option<&Value>;  // the outline reads descendants'
    fn selection_below(&self, steps: &[Step]) -> bool;
}
pub trait Toolkit: Viewing {               // display's own widgets
    fn styles(&self) -> &Styles;  fn selected(&self) -> bool;  fn sources(&self) -> Sources<'_>;
    fn location(&self) -> Option<Location<'_>>;
    fn line(&self, text: &str) -> Option<Cow<'_, LineEditState>>;
    fn picking(&self, kind: CompletionKind) -> Option<PickerView<'_>>;
    fn selected_trace(&self) -> Option<&SourceTrace>;
    fn trace_below(&self, steps: &[Step]) -> Option<SourceTrace>;
}
```

- **The theme.** Libraries name a `Face`, as in
  `widget::label(context, text, Face::Label)`, and the toolkit takes over the
  drag outline. `Styles`' fields can become private to core once the engine's
  highlight drawing moves to display.
- **Targets.** `ProjectionInput.targets` hands out `Site`s.
- **The command convention** comes from the frame's input: `HoverInput` carries
  whether Cmd or Ctrl is the command key, read where each handler and line
  editor is installed, so neither the editor nor the per-view bag carries it
  into display or the libraries.
- **The per-view bag stays the runtime's private type**, and nothing below the
  runtime names it.

Why this shape:
- A `dyn` world needs `C: ?Sized` through Puri's handler, hover, dispatch and
  landmark types, and a downcast to `Editor` in the shell's own handlers, which
  is a partial function.
- A concrete core world inside `Editor` needs a lens that maps the frame data as
  well, because the dispatch context holds world-typed handlers and landmarks.
- Generics need neither. Behavior written over `Session` still compiles once,
  where it's written.

### Steps

Each lands on its own and passes `make test`.

1. Alias `<Editor, Hovered>`.
2. Take the command convention from `HoverInput`.
3. Introduce `Session`: `Access::new(world)` builds one, and the shell sets the
   window title after dispatch when it changes. The 16 edit sites don't change.
4. Gestures take `Session`.
5. Offers take `Session`, and the commit helpers become core functions.
6. `Site` and `Action`, with a first `Host` that only lends a `Session`.
7. `Viewing` and `Toolkit` replace `inputs`, for the libraries first and then
   the toolkit. Computations become non-optional.
8. Add `Host::Hover`; display and the libraries go generic.

### Risks and open questions

- **Constructors can't be hidden** once the runtime sits above core, but what a
  handler is given is enforced: nothing reachable from a `Session` leads to the
  window or the menus.
- **Tests move onto memoized evaluation** when computations become
  non-optional; background tasks run inline there, so results should match.
- **Source links read input state** from the editor (source_link.rs:81-83),
  which belongs in the dispatch context.
- **The outline re-roots through `Location` and `Conject`**; jumping should
  take a `Site`.
- **Who owns the handler event format.** The owner wants the core to own it;
  that can follow step 6, since `run_handler` doesn't care who encodes the
  event.

### Unblocks

The crate split's last steps, library tests without a whole test editor, and
the driver's "select a path" verb.

## 2. The picker as a widget

### Today

- **Three layers build one picker.**
  - The shell's completion.rs decides whose offers a hole lists, builds Raw's
    offers, ranks a projection's offers, names them, and prepares the commit.
  - The engine's projection/completion.rs draws it: the hole and its landmark,
    the request, the query's line editor, the card, Raw's bracket keys, Tab and
    placement. It also draws Raw's picker for any missing value no projection
    draws (projection/mod.rs:994-1003).
  - Display has only the card and the retained offers behind hover marks.
- **Grap's slots borrow Raw's offers** from the shell (grap/suggestions.rs:56,
  62).
- **The working state is the selection's.** The query is its live line editor,
  and the payload stores the stage, query, choice and card scroll as GID
  values, beside the editor's caret, preedit and drag.
- **Some keys are the shell's.** Escape, an empty query's Backspace or Delete,
  and structural paste are structural keys.
- **Choosing receives the whole editor** and commits through
  `Editor::commit_completion`.

### Design

```rust
// core: display/widget/picker.rs
/// A hole's list for a query, in the order shown, with the spans that matched.
pub type HoleOffers = Rc<dyn Fn(&CompletionRequest<'_>) -> Vec<Entry>>;
pub struct Picker {
    pub kind: CompletionKind,                  // a value, or a new field's label
    pub offers: HoleOffers,
    pub keys: Vec<(&'static str, Completion)>, // chosen at once on an empty query, like Raw's ( [ {
}
pub fn ranked(provider: CompletionProvider) -> HoleOffers;
pub fn picker<W: Host>(picker: Picker) -> View<W>;

// CompletionRequest also carries what the shell reached around it for:
pub name: &'a dyn Fn(CellId) -> Option<&'a str>,  // offers named as the document names them
pub libraries: Option<&'a CompletionProvider>,    // the loaded libraries' offers

// core: display/raw.rs, beside the structural fallback
pub fn offers() -> HoleOffers;                    // Raw's order
pub fn start() -> HoleOffers;                     // the libraries' starting points, then Raw's
pub fn keys(kind: CompletionKind) -> Vec<(&'static str, Completion)>;
pub fn constructors(kind: CompletionKind) -> Vec<Completion>;       // Grap's slots compose these
pub fn typed(request: &CompletionRequest<'_>) -> Vec<Completion>;

// core: editing, the open picker on note 1's write side
impl Picking<'_, '_> {
    pub fn stage(&self) -> Stage;
    pub fn commit(&mut self, value: Value, on_commit: Option<Continuation>) -> bool;
    pub fn commit_label(&mut self, label: CellId, definition: Option<Value>, on_commit: Option<Continuation>) -> bool;
    pub fn commit_new_cell(&mut self, definition: Value, on_commit: Option<Continuation>) -> bool;
    pub fn commit_container(&mut self, value: Value) -> bool; // then opens within it
    pub fn pick(&mut self, value: Value) -> bool;             // Cmd+click and paste
    pub fn reopen(&mut self, payload: Value);                 // color's picker
    pub fn edit_query(&mut self, operation: &EditOperation<'_>) -> bool;
    pub fn set_view(&mut self, scroll: f64, choice: usize);
}
```

- **One widget is the hole and its picker.** It draws the placeholder, landmark
  and selection, the query editor, the card, Tab and `keys`, and the placement,
  and registers entries for hover marks. It reads through `Toolkit` and writes
  only through `Picking`.
- **Raw is an ordinary projection's vocabulary.** The structural fallback draws
  `picker(Picker { kind, offers: raw::start() or raw::offers(), keys:
  raw::keys(kind) })`.
- **Keys.** The card keeps Enter and the arrows; the widget takes Tab and
  `keys`. Escape, empty-query Backspace and Delete, and structural paste stay
  structural keys, because they also act on edges and on every pending.
- **Names and libraries** reach offers through the request, filled from the
  read side.
- **Working state** stays with the selection, since a picker lasts exactly as
  long as a pending at its site, but becomes typed: the query's line editor,
  choice and scroll. Choice and scroll leave the payload; the stage and the
  query stay, as what projections and Grap see.

### Steps

Behavior stays identical; each step passes `make test`. Steps 1–3 don't need
note 1.

1. Move the card, its placement and the selection ring to display, with the 9
   card tests.
2. Move Raw's offers to display/raw.rs. The ranking becomes `ranked`, the
   request gains `name` and `libraries`, Grap imports `raw::constructors` and
   `raw::typed`, and completion.rs keeps only commit preparation.
3. Build the widget, drawing from display/widget/picker.rs. Raw's brackets
   become `keys`, and projection/completion.rs goes.
4. With note 1's step 5: activation takes `Session`, and `Picking` replaces the
   commit helpers.
5. After the owner decides: typed picker state.

### Risks and open questions

- **Raw's names.** Raw shows ids for values but names its offers, so the
  request's `name` must keep that; `Env::name` would change Raw's picker.
- **Tab's skipping of name slots** stays for now (navigate.rs:240).
- **Offers rebuild every frame** the picker is open, as now. A long list would
  need computations keyed by site and query, or scrolling hitches again.
- **Step 5 is visible to Grap**: `selection get` on a pending would no longer
  return the choice and scroll. No example reads them.

### Unblocks

Building completion back up becomes projection work: bracket keys for any hole,
an operation's projection telling its operands' holes what to offer, the record
spec's "add a field" picker, and sections for a `…` expansion that doesn't copy
offers. The completion module's tests target `ranked`, `raw::offers` and
`Picking` directly.

## 3. Drawing programs recorded by the layout library

### Today

- **Two layout forms run a drawing program**: the stored drawing form
  (layout.rs:647-661) and a layout program's `canvas` (layout/scope.rs:328-348).
  Both call `display::drawing_program` (display/mod.rs:1061-1077), which is
  concrete over `Editor` and exists only to reach the engine's `program_leaf`.
- **projection/drawing.rs interprets the layout library's vocabulary**: an
  overlay of `fill`, `path`, `move to`, `line to` and `close`, fast paths for
  `rect`, `circle`, `translate` and `rotate`, and layout's shape, transform and
  paint readers for the rest. These are the audit's 41 layout references.
- **The same file does toolkit work**: it records lazily, at most once per
  frame; hit-tests fills in reverse paint order; links Cmd+click and hover to
  sources; replays under a clip, washing selected and hovered sources; and
  resolves faces through a copy of the whole `Styles`.
- **Library-owned overlays are already the pattern**: layout runs its layout
  functions through its own overlay (layout/scope.rs:9-29, 85-118), and
  toolpath's preview records paths the same way.

### Design

```rust
// core: display/widget/recorded.rs
/// What a program drew: fills, each linked to the calls that drew it.
pub struct Recorder { commands: DrawList, hits: Vec<Hit>, input: Option<SourceTrace>, faces: [Brush; 7] }
impl Recorder {
    /// Resolves `paint` here, so the library never sees the theme, and keeps a hit
    /// only when its trace names a source.
    pub fn fill(&mut self, shape: puri::Shape, paint: Paint, transform: Affine, calls: Option<grap::CallTrace>);
}
/// Runs a program into the recorder; false, for a decline or a halt, discards what it drew.
pub type Record = Rc<dyn Fn(&dyn grap::Host, &mut Recorder) -> bool>;
/// A leaf `extent` logical units big; `program` locates the program below this occurrence.
pub fn recorded<W: Host>(extent: Extent, program: Vec<Step>, record: Record) -> View<W>;

// libraries/layout/drawing.rs: the overlay, its fast paths and INVALID_DRAWING
pub(super) fn record(program: RuntimeValue, fuel: usize) -> Record;
```

Both layout forms call `display::recorded`, and nothing in the engine names
layout's cells. The core keeps laziness shared between hit-testing and paint,
the hits, clip, replay, the two washes, and the source-link handlers.

### Steps

Each passes `make test`; compare perf after steps 1 and 2 with
`make perf ARGS="--only 'iop_tree|forest|layout_ffi'"`.

1. Carve the toolkit part out in place, still concrete over `Editor`.
2. Let the layout library record: the overlay moves to
   libraries/layout/drawing.rs, and the recording tests move to layout's tests.
3. With note 1's `Host`, the leaf goes generic and the source-link handlers
   move into the toolkit.

### Risks and open questions

- **Recording twice** if one lazy recording stops serving both hit-testing and
  paint.
- **Input traces need a location**, which `canvas` has only for stored and cell
  origins; keep that.
- **Programs can only fill.** The stored commands form also strokes, clips and
  curves but has no hit-testing. Should the recorder gain `stroke` and `clip`,
  and should stored drawings replay through it and gain hover-to-source?

## Decisions for the owner

1. Handlers, gestures and offers act through one session of the editor's
   capabilities (edit here, set state, select, start a drag, timers, the open
   picker), not through the editor or a type-erased stand-in for it.
2. A library widget refers to its place by an opaque site handle, not by view
   and path, and reads per-path state and the selection only through the read
   interface, including below its own place, which the outline needs.
3. Whether Cmd or Ctrl is the command key is decided once, where input enters a
   frame, and handlers are given it.
4. The picker's choice and scroll leave the selection payload, so projections
   and Grap's `selection get` see only the stage, the query and a projection's
   own fields.
5. Raw's offers become an ordinary projection's vocabulary in the toolkit, used
   by the structural fallback and composed by Grap, with ranking a combinator.
