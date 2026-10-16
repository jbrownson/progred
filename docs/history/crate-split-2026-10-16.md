# Crate split analysis, 16 October 2026

This note follows up the [codebase audit](codebase-audit-2026-10-16.md). It
covers what three crates would hold, how the code is coupled across their
boundaries, and how each coupling could be broken. A subagent asked to change
nothing made it at `fad47661`, and its cited sites were spot-checked.
[Deferred work](../deferred.md#code-size-and-structure) keeps the steps. This
note keeps the evidence.

**Bottom line:**
- Cut three crates, not five.
- Most of the coupling is type names. Display and the libraries write out the
  shell's `Editor` and `Hovered` 635 times, yet the libraries never build a
  `Hovered`, and display builds one in two places.
- The rest is small:
  - a few dozen handlers that take `&mut Editor` for about 8 of its 27 fields;
  - a dozen shell helpers called from below;
  - the model's use of library vocabularies.
- Each of these has a local fix that pays off on its own.
- The compile-time saving is in lines, not measured seconds. A split could also
  make `make test` slower for edits low in the stack, so measure first.

**Method.**
- Counts cover production code only: 35,852 lines, excluding test-only files
  and `#[cfg(test)]` items.
- Every `crate::` and `super::` path was resolved, along with every import,
  every later use of an imported name, and every `use super::*` glob.
- Counted the audit's way (imports plus inline paths), the tables reproduce the
  audit's: display→shell 222, libraries→shell 429.
- An "occurrence" below also counts each later use of an imported name.
- The commit simulation replays progred/src's commits from the last month (256)
  and the last 90 days. It averages the share of production lines each commit
  would not recompile.

## The crates

| Crate | Contents (production lines) | External dependencies |
|---|---|---|
| **core**, ≈9.5k | The model, ≈4.0k: selection 922, workspace 803, gid_text 528, editing/scope 290, site 282, sources 275, computations 195, filter 177, model 114, location 109, annotations 102, and smaller modules. Display, 4.9k: its mod.rs 1,058, widget/ 3,299, and structure, measure, conject, projection and builder. Moved down, ≈0.6k: hover.rs, gesture's `EditRun`, `projection::Cx` and `select_handler`, `Libraries` and `Definitions`, the vocabularies the model interprets, and the text, name and blob codecs | gid, grap, incremental, puri, puri-widgets, measured, kurbo, peniko, parley, ui-events, serde_json, web-time. It needs no winit, Vello, wgpu or Fidget, so on a clean build it compiles alongside them |
| **libraries**, ≈15.8k | Every built-in library: toolpath 3,696, fidget 2,351, layout 1,639, color 1,241, controls 1,100, control 935, grap 878, number 758, presentation 416, tree 384, list 305, and the small ones | core, Fidget (raster and mesh, plus wgpu natively), nalgebra, png, pollster |
| **top**, ≈10.6k | The projection runtime, 2,286: projection/'s mod, drawing, completion, source_link, structure, line_control, viewport and location. The shell: frame, editor, menu, commands, input, command, completion, navigate, placed, editing.rs, stack, clipboard and render. The app: app, native_menu, lib.rs, the web and macOS files, text_store and platform | winit, ui-events-winit, Vello, puri-vello, uig, clipboard-rs, rfd, the objc2 crates and raw-window-metal. On wasm it adds web-sys, wasm-bindgen, puri-web, and Fidget for its thread pool |

Share of production lines a commit would not recompile, averaged:

| Layering | Crates | Last month | 90 days |
|---|---:|---:|---:|
| The audit's split C: model < display < libraries < shell < app | 5 | 32.4% | 36.3% |
| model < display < libraries < top | 4 | 31.0% | 34.6% |
| core < libraries < shell < app | 4 | 30.1% | 34.9% |
| **core < libraries < top** | 3 | 28.8% | 33.1% |
| The same, with the projection runtime in core | 3 | 15.1% | 15.5% |

Why this cut:

- **Model and display share a crate.** Together they're 9.2k lines, tangled in
  both directions. The model refers to display 72 times, mostly to `Root` and
  `Target` (59). Those are defined in display/widget/view.rs and re-exported by
  workspace.rs:31, and `Sources` implements `display::Env`. Separating the two
  buys 1.5–2.2 points.
- **Shell and app share a crate.** The app's 3k lines rebuild with the shell
  anyway. The shell names app types in 31 places (`UserEvent`, `stack::Stack`,
  `macos_window`, `web::download`). Separating them buys 1.3–1.8 points.
- **The projection runtime stays on top.** It's the most-edited code:
  projection/mod.rs alone had 36 commits last month. Moving it into core about
  halves the saving.
- **What moves down is small and stable:**
  - hover.rs has no shell dependencies.
  - `Cx` is a struct of model and display borrows (projection/mod.rs:114-136).
  - `EditRun` touches only the model and the libraries (gesture.rs:42-98).

Three crates keep about 90% of the five-crate saving, and they make the
model–display and shell–app tangles unnecessary to break. Inside core, the
model and display stay layered by convention. The audit's split A, the CAM
geometry, is independent and can happen at any time.

## The coupling today

907 occurrences point upward, in seven kinds.

**K1. The concrete world and hover types.** There are 635 `Editor` and
`Hovered` tokens in 51 files: display has 210 in 9 files and the libraries 425
in 42. 602 of them are 301 pairs that instantiate display's generic types:
`Layout` 150 times, `ProjectionInput` 68, `Library` 30, `Partial` 25,
`RecordField` 12, and others. For example, libraries/text.rs:89-91:

```rust
pub fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered, ::grap::RuntimeValue>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
```

These are only names:
- The libraries never construct or match a `Hovered`.
- Display builds one in two places, both `Hovered::Tree(Hover::Value(path))`
  (display/widget.rs:156, display/widget/line.rs:135).
- Display is already generic almost everywhere. `Layout`, `Partial`,
  `ProjectionInput`, `Widget`, `Context`, `HoverPass`, `Gesture<World>` and
  `Project<World, Hover>` are all generic over the world and hover types, and
  so are 125 of display's 169 world-typed signatures.
- Of display's 44 concrete signatures, 10 call shell services: completion,
  drawing_program, selectable, on_state_drag, on_point, point,
  pickable_runtime, line::view, arrival and authoring. The other 34 are
  concrete only because they call those.
- The libraries are the opposite. 128 of their functions have concrete
  signatures, 2 are generic, and 490 never mention the world.
- The world-typed functions hold 4.2k lines of the libraries (26%) and 0.8k of
  display (16%).

**K2. Handlers that act on the editor.** There are 36 sites in 15 files, plus
`Completion::activate: Rc<dyn Fn(&mut crate::Editor)>` (display/mod.rs:81). The
sites are:
- 16 scoped edits through `scope.open(Access::new(world))`;
- 5 `editing::start_gesture` calls and 3 `editing::commit_*` calls;
- 4 direct reads or writes of `world.model.selection` or `world.sources()`;
- 2 `pick_identity` calls, and one each of `author_in` and `set_collapsed`;
- 2 reads each of `world.command_modifier` and `world.timers`.

Between them they use:
- 8 of `Editor`'s 27 fields: the model, the libraries, timers,
  command_modifier and gesture, plus font_cx, layout_cx and text_clipboard for
  line edits;
- five commands.rs methods: commit_completion, open_within, pick_identity,
  author_in and set_collapsed;
- refresh_title.

**K3. Shell helpers called from below.** There are 25 references to 13 items,
for example:
- Display's widget `Context` holds the shell's `&projection::Cx`
  (display/widget.rs:55).
- Display calls `projection::completion::control` (display/mod.rs:542).
- Number's scrubbing uses `gesture::scoped_value_edit` and `DRAG_THRESHOLD`
  (libraries/number/scrub.rs:48,57).
- Controls uses `hover::from_grap` and `projection::source_link::decoration`
  (libraries/controls.rs:502-503).
- Grap's suggestions call `completion::constructors` and `typed`
  (libraries/grap/suggestions.rs:56,62).

They fall into four groups:
- data: `Cx` and `Hover`;
- a helper that only touches the model: `EditRun`;
- pure functions of the completion request: `constructors` and `typed`;
- calls back into the projection runtime.

**K4. The registry.**
- `Editor.stack` is a `Stack<Editor>` (editor.rs:115) holding
  `Partial<World, Hovered>`s (stack.rs:19).
- `BUILT_INS` is a table of `fn() -> Library<crate::Editor, Hovered>`
  (stack.rs:149-181).

That is the one place that should name both the editor and every library, and
it stays on top. The only library code that reaches back into it is the
cam-profile helper (libraries/toolpath/performance.rs:11-12).

**K5. The model uses library vocabularies.** There are 123 occurrences in 8
files:

| What | Occurrences |
|---|---:|
| Vocabulary constants | 56 |
| Wrappers of grap's own conventions | 23 |
| Module imports | 14 |
| The text, name and blob codecs | 18 |
| The `Libraries` type | 9 |
| Selection helpers | 3 |

For example, annotations.rs:12 re-exports
`libraries::site::vocabulary::{EXPANDED, FOLD, FOLDED}`. And selection.rs:818
calls `f64_convention::value`, where libraries/f64.rs:60 is just
`pub use ::grap::f64::{read, value}`. All of these are cells the model
interprets itself, pure codecs, or the registry's data type.

**K6. The model reaches into the shell.** There are 20 occurrences in 3 files:
- `editing::Access` holds `&mut Editor` (editing/scope.rs:165-168). Field
  privacy keeps widgets from reaching the editor through it, as its comment
  says, but the model code behind it gets the whole editor.
- `site::apply_scoped_event` takes `app: &mut Editor` (site.rs:96).
- editing.rs has ten functions that take `&mut Editor`.

They use the model, the libraries, the font contexts (for edit_line,
editing.rs:172-205), and `refresh_title` (editing/scope.rs:206).

**K7. Tangles that end up inside one crate.** The model's 72 references to
display (9 files) stay inside core, and the shell's 31 to the app (3 files)
stay inside top.

## Breaking each kind

**K1: aliases now, a `Host` trait later.**
- Some aliases already exist: placed.rs:21-28 aliases types over `Hovered`, and
  libraries/controls.rs:52 and projection/structure.rs:15 alias the pair. One
  set of aliases would name the pair in one place.
- Later, a core trait turns the aliases generic. Roughly:
  `trait Host: Session + 'static { type Hover: Clone + PartialEq + From<Hover>; … }`,
  with aliases like `View<W> = Layout<W, W::Hover>`. The 128 library functions
  then take `<W: Host>`.
- `Hovered` itself can't move down, because its menu arm carries
  `command::Command` (frame.rs:93-102, menu.rs:130-133).

**K2: pass capabilities, not `&mut Editor`.**
- `Access` holds a `&mut dyn Session` instead: the model, the libraries and
  the text-editing context. The 16 `Access` sites don't change.
- The other 20 sites go through `Host` methods. Direct selection writes use
  `Edit::select_payload` (editing/scope.rs:235).
- A completion activates through `&mut dyn Commit`. `Completion`,
  `CompletionProvider` and `Env::completions` then stay non-generic and compile
  once.

**K3: move data down, and route calls up through `Project`.**
- `Hover`, `SourceCalls` and `from_grap` move beside `SourceTrace`. hover.rs:32
  already re-exports it from display/widget/source.rs.
- `Cx` and `select_handler` move into display.
- `EditRun` moves into core and works over `Session`.
- `constructors` and `typed` move beside `Completion`.
- Calls that need the runtime become `Project` methods: the completion picker,
  drawing programs, and source-link decorations unless those move down with
  `Cx`. Display already calls `context.project` this way for descend, jump and
  at (display/mod.rs:891, 970, 1012).

**K4: nothing extra.**
- Once the libraries are generic, stack.rs instantiates
  `name::library::<Editor>` and the rest.
- The cam-profile helper takes `&Libraries` instead of calling `stack::load`.

**K5: each vocabulary lives with the code that interprets it, as f64's already
does.** libraries/f64.rs:57-59 says: "The convention itself lives in the
evaluator, which privileges it as an accelerator; this library remains its
owner in vocabulary, functions, and projections."
- The site, selection and workspace vocabularies move into site.rs,
  selection.rs and workspace.rs, and the libraries re-export them. The `CellId`
  constants move unchanged.
- The model calls `grap::f64`, `grap::path` and `grap::absent` directly.
- The text, name and blob codecs move into core, or into grap.
- `Libraries` and `Definitions` (libraries/mod.rs:96-166 and 231-360) move into
  core.

**K6: pass what's used.**
- `apply_scoped_event` takes the model and the libraries.
- editing.rs's functions become `impl Host for Editor` on top.
- The window title is chrome derived from the document path and `dirty()`. The
  shell could set it after dispatch whenever it changes, replacing ten scattered
  `refresh_title` calls, one of them in the model layer.

A sketch, drawn from libraries/controls.rs:52-53 and 199-217, and site.rs:96:

```rust
// core, editing/scope.rs
pub trait Session {                                      // Editor implements it
    fn document(&mut self) -> (&mut Model, &Libraries);  // model, stack.libraries
    // plus one method lending the model and the text-editing context together, for edit_line
}
pub struct Access<'a> { session: &'a mut dyn Session }   // was { editor: &'a mut Editor }

// libraries/controls.rs
- type Change = Rc<dyn Fn(&mut crate::Editor, &crate::editing::Scope, &Root, &[gid::Step], Value)>;
- fn slider_widget_with(key: CellId, …) -> Widget {
+ type Change<W> = Rc<dyn Fn(&mut W, &Scope, &Root, &[gid::Step], Value)>;
+ fn slider_widget_with<W: Host>(key: CellId, …) -> Widget<W> {
      slider_control(…, Rc::new(move |editor, scope, root, path, value| {
          let mut editor = scope.open(Access::new(editor));    // unchanged
          let state = set_state(editor.annotation(root, path), key, value);
          editor.annotate(root, path, state);
      }))

// site.rs
- pub(crate) fn apply_scoped_event(app: &mut Editor, scope: Scope, root: Root, …) -> bool
+ pub fn apply_scoped_event(model: &mut Model, libraries: &Libraries, scope: Scope, root: Root, …) -> bool
```

## Steps

Each step lands on its own, changes no behaviour, and is checked by
`make test`.

| # | Step | Size | Risk | Pays off on its own? |
|---|---|---|---|---|
| 0 | Baseline timings, below | None | None | Yes: it decides 5 and 6 |
| 1 | Aliases for the concrete pair (K1) | 51 files, ~330 lines, plus 5 shell files | Very low | Yes: text.rs's signature fits on one line, and the pair is named once |
| 2 | The model owns its vocabularies, codecs and `Libraries` (K5) | ~16 files: ~300 lines moved, ~120 sites edited | Low | Yes: the model reads without the libraries, and every convention follows f64's pattern |
| 3 | `Access` over `Session`; site takes the model and the libraries; the shell sets the title; `EditRun` moves down (K6, part of K3) | ~6 files, ~200 lines | Low–medium, because of edit_line's borrows | Yes: model logic is testable without `test_editor`, and `Access` carries only what edits need |
| 4 | `Hover`, `Cx`, `select_handler`, `constructors` and `typed` move down, and calls into the runtime go through `Project` (K3) | ~12 files: ~350 lines moved, ~100 edited | Low–medium | Yes: display has one documented boundary with its host |
| 5 | The `Host` trait. Display's 10 root functions go generic, then their 34 callers. `Completion` activates through `dyn Commit`, the 128 library functions take `<W: Host>`, and stack.rs instantiates them (K1, K2, K4). Re-measure afterwards | ~55 files, ~500–700 lines | Medium | Barely: tests could use a stub host |
| 6 | Extract core, libraries and top | Large and mechanical | Medium–high: the wasm build, the sandbox wrapper, features | That is the compile-time saving |

Step 6 needs:
- three Cargo.toml files;
- updates to the Makefile's `check` and `test` feature flags and to the
  `[profile.dev.package.*]` entries;
- about 370 items and modules made `pub` across the new boundaries.
  Re-exporting core's modules at each crate root keeps `crate::display::…`
  paths working;
- a test-support feature for up to 73 `#[cfg(test)] pub` helpers, since
  `cfg(test)` doesn't cross crates;
- moving four test files that drive the real editor up to top:
  libraries/controls/tests.rs, png/tests.rs, fidget/interaction/tests.rs and
  test_widgets.rs.

## Caveats

- **The saving is in lines, not seconds.** Of last month's 256 commits, 104
  touched core and would still rebuild everything; 78 touched only top.
- **`make test` may get slower for edits low in the stack.** Today it compiles
  progred once (`cargo test --workspace --tests`, see dbc80077). After the
  split, core and the libraries each compile twice, once as a library and once
  as a test harness. A core edit would then rebuild two libraries and three
  test harnesses.
- **Generics move code generation to top.** Library code is monomorphized where
  `Editor` is known.
  - The ~5k world-typed lines are type-checked once, in their own crates, but
    their code is generated in top.
  - A `dyn Host` world would avoid that, but it needs `?Sized` through puri's
    `Handler` (ui/puri/src/handler.rs:131-134) and through display. It also
    needs a downcast in the shell's handlers, which is a partial function.
  - For any hot spot, a thin generic shim around a non-generic body is the
    cheaper fix.
- **Readability.** `<W: Host>` lands on about 170 signatures, and the aliases
  absorb most of that.
- **What resists:**
  - `Hovered`, as above.
  - edit_line needs the model and the font contexts in one borrow.
  - Features cut across the layers: a layout-profile hook sits in
    `display::completion` (display/mod.rs:540-541), and the cam-profile
    modules use `stack::load` and `command::Example`.
  - `[profile.dev.package.progred] opt-level = 0` needs a decision for each
    crate. Optimizing the libraries would speed their tests up, as optimizing
    the dependencies did (239 s to 31 s), but would slow library edits.

**What to measure**, through the wrapper, at steps 0, 5 and 6:
- `cargo build --timings`: each crate's front-end and code-generation time,
  and the clean-build critical path.
- Incremental build and `make test` wall-clock time after a one-line edit in
  each of frame.rs, libraries/color.rs, display/mod.rs and selection.rs,
  weighted by the commit mix above.

The split pays for itself in time only if that weighted time drops. Enforced
layering is the other reason to split, and it is worth weighing on its own.
