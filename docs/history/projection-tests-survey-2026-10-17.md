# Projection tests survey, 17 October 2026

This survey asks what progred/src/projection/tests actually tests, how its tests
drive and observe the editor, what they cost, and where each belongs. The owner
asked whether these should be more targeted tests, whether they need more
internals exposed, or whether they need a system for running them. They also
said that tests belonging elsewhere should move. A subagent made the survey at
`a952ec31` and was asked to change nothing; the headline counts and the slowest
test's time were re-checked. [Deferred work](../deferred.md#code-size-and-structure)
keeps the plan, and this note keeps the evidence.

**Bottom line:**
- **Most of these tests aren't about the projection runtime.** About 87 tests
  (4.3k lines) test the runtime itself. The rest test the completion module,
  the selection model, display widgets, single libraries, or whole-editor
  flows. The completion module and selection.rs have no test modules of their
  own; their tests live here.
- **Better tests need less internal access, not more.** The six private items
  the tests reach (14 calls) are display code filed in the wrong place, plus
  one timing hook. Everything else they use is already visible across the
  crate, and targeted versions only need functions that already exist.
- **The shared driver is barely used.** Only 3 tests use it. 43 build the
  editor runner themselves, 76 fake input by calling the frame's handler
  directly, and 100 places set the selection by writing it. The handler-level
  tests partly re-implement the shell's key routing (`input::structural_keys`),
  so they can pass while the real chain behaves differently. What the driver
  lacks is a few verbs, not access to internals.
- **Run time comes from one test.** 283 of the 288 tests that run by default
  finish in 2.5 s of wall time. The shape lesson's CAM test
  (`website_shape_edits_and_playback_change_the_rendered_mesh`) takes 17 s on
  its own and is the slowest test in progred. Without it, progred's tests take
  23 s of wall time instead of 37 s. CAM code is slow in tests because progred
  builds unoptimized; the library tests in `libraries::toolpath` take another
  46% of progred's serial test time.

**Method.**
- Every test was read and classified by hand, and regex counts back the
  numbers.
- A test that mixes harnesses counts as its heaviest one.
- Times are from the default configuration on a 4-core VM.

## The files

The drive/observe classes are:
- (a) direct calls into one lower system;
- (b) the projection-level harness;
- (c) the full editor through the shared driver;
- (d) the full editor, reaching around the driver;
- (e) captures.

"Perf" means opt-in profiling. "Bench harness" means the helper that calls the
private `prepare_project`.

| File | Lines | Tests | System under test | a/b/c/d/e/perf | Destination | Blocked by |
|---|---:|---|---|---|---|---|
| mod.rs | 312 | 0 | Shared harness, helpers, fixtures | – | shared test support | imports the completion card |
| completion.rs | 2009 | 35 | The completion module's entries, readings, ranking and preparation (20); hole offers through the projection (12); picker flows (3) | 19/16/0/0/0/0 | 20 to the completion module; offers stay; flows to the driver | – |
| display.rs | 689 | 12 | Engine partial, descend and at semantics (7); selection and sample model (3); `projection_target` (1) | 4/8/0/0/0/0 | 7 stay; 3 to selection and sample; 1 moves with its item | `projection_target` |
| editing.rs | 1777 | 34 | Selection writes, pendings and the clipboard codec (11); write-through via `line_control::commit` (9); line keys (10); paste (1) | 11/22/0/1/0/0 | 11 to selection; 9 targeted; the rest stay or go to the driver | – |
| events.rs | 438 | 2 | Handler site capture; the layout library's event encoding (one 354-line test) | 0/2/0/0/0/0 | 1 stays; encoding to the layout library | – |
| navigation.rs | 109 | 2 | `navigate::direction` and `select_all`, pure functions | 2/0/0/0/0/0 | navigate | – |
| frame/mod.rs | 529 | 3 | The bench harness (294 helper lines); widget decorator order; Plain against Raw; the absent wash | 1/1/1/0/0/0 | harness to test support; decorator to display | `prepare_project` |
| frame/completion.rs | 1212 | 12 | The completion card widget with stand-in worlds (9); picker flows (3) | 8/4/0/0/0/0 | 9 to display; 3 to the driver | the completion card, its placement, the highlight |
| frame/compositor.rs | 327 | 2, ignored | GPU compositor pixels and timing | 0/0/0/0/1/1 | opt-in GPU | – |
| frame/compositor/mesh.rs | 388 | 3, ignored | puri-vello's mesh renderer without the editor (2); orbit timing (1) | 0/0/0/0/2/1 | 2 to puri-vello | – |
| frame/conject.rs | 762 | 16 | Mapping occurrences to locations; read-only; copy, cut and fold | 0/16/0/0/0/0 | stays (runtime) | – |
| frame/custom_navigation.rs | 499 | 9 | Arrow tables through Grap, control and Fidget forms | 0/9/0/0/0/0 | stays | bench harness (8) |
| frame/declarations.rs | 801 | 14 | Grap, control and number projections of binders, names, quotes and infix | 0/14/0/0/0/0 | the grap and control libraries | bench harness (5) |
| frame/document_libraries.rs | 357 | 6 | Document-declared libraries | 1/5/0/0/0/0 | 5 stay; 1 to workspace | – |
| frame/drawing.rs and iop_tree_native.rs | 821 | 7 | Drawing programs (3); pane and Fidget previews (3); a Grap drawing equals the native one (1) | 3/4/0/0/0/0 | 5 stay; Fidget previews to fidget | bench harness (4) |
| frame/fidget_cube.rs | 187 | 3 | The cube example program as a Fidget field, with no projection at all | 3/0/0/0/0/0 | the fidget library | – |
| frame/fidget_shapes.rs | 131 | 1 | Example figures equal standalone previews | 0/1/0/0/0/0 | split | – |
| frame/fidget_source.rs | 79 | 2 | Fidget infix paths, hover and editing | 0/2/0/0/0/0 | the fidget library's projection | bench harness |
| frame/focus.rs | 210 | 5 | Runner palette and focus handling | 0/0/0/5/0/0 | end-to-end | – |
| frame/folds.rs | 168 | 4 | Fold keys and defaults at occurrences | 0/4/0/0/0/0 | stays | – |
| frame/interaction.rs | 1780 | 28 | Key and picker flows via the runner (11) or the handler (6); the driver (2); gestures (5); slots and clicks (4) | 5/10/2/11/0/0 | flows to the driver; gestures to display and number | bench harness (1) |
| frame/layout.rs | 1586 | 23 | Runtime values (2), toolkit layout and hover (6), highlights (2), list and record gaps (12), cross-pane marks (1) | 0/22/0/1/0/0 | 7 targeted; the rest stay | bench harness (16) |
| frame/logical_navigation.rs | 403 | 9 | Arrows over document-authored layouts | 0/8/0/1/0/0 | end-to-end | – |
| frame/navigation.rs | 1183 | 18 | Arrow navigation | 0/6/0/12/0/0 | end-to-end, through the driver | – |
| frame/numbers.rs | 281 | 6 | Number library projections; scrub contacts | 0/6/0/0/0/0 | the number library | bench harness (3) |
| frame/outline.rs | 821 | 13 | The presentation outline and its interaction | 0/12/0/1/0/0 | the presentation library | bench harness (1) |
| frame/profile.rs | 584 | 15, 13 ignored | Frame canaries; Fidget examples render | 0/2/0/0/0/13 | perf | `prepare_project` (phase timing) |
| frame/profile/forest.rs | 116 | 2, ignored | Forest canaries | 0/0/0/0/0/2 | perf | – |
| frame/profile/forms.rs | 135 | 4, ignored and feature-gated | Allocation profiles | 0/0/0/0/0/4 | perf | – |
| frame/profile/layout_ffi.rs | 260 | 3, 1 ignored | A layout FFI program equals the data layout | 0/2/0/0/0/1 | 2 to the layout library | `Projection::without_entry` (canary only) |
| frame/svg.rs | 1489 | 24, 8 ignored | The SVG writer and its 2 tests; captures (7 run by default); lesson drawings (5); a toolpath failure | 2/1/0/5/16/0 | writer to test support; captures to their own module; lessons targeted | bench harness (13) |
| frame/svg/refined.rs | 177 | 2, ignored | README and CAM captures | 0/0/0/0/2/0 | opt-in | – |
| frame/tutorial.rs | 211 | 5 | Tutorial slots and peeling | 0/4/0/1/0/0 | web_embed | – |

## Totals

| Class | Tests | Lines |
|---|---:|---:|
| (a) direct call | 59 | 3,548 |
| (b) projection harness, render and inspect | 105 | 5,455 |
| (b) projection harness, input sent to the frame's handler | 76 | 4,238 |
| (c) driver | 3 | 111 |
| (d) full editor around the driver | 38 | 2,055 |
| (e) captures (8 run by default) | 21 | 873 |
| perf (all opt-in) | 22 | 622 |
| **Total** | **324**: 319 compiled by default, 288 run | 16,902 in tests, plus 3,119 in helpers and 810 in imports and constants |

## What the tests reach around the driver for

| What the tests touch | Call sites | Missing from the driver |
|---|---|---|
| Writing the selection to set a starting point | 101 | `select(path)`, through the landmark's own select |
| Reading the selection's stage, caret or text | 158 | a stage and caret query |
| The picker's entries | 6 | a picker query |
| Focus, palette and library-swap changes | 10, plus menu and tutorial tests | those inputs, and resize |
| The line editor's IME input | 10 | IME and preedit input |
| Per-path state or the document, written directly | 12 / 12 | fold and control verbs, or building the document up front |
| The frame's dispatch regions and paint preparation | 8 / 24 | view regions |
| Background computations, swapped or polled | 7 / 9 | deterministic stepping of background work |
| Editor methods such as commit, paste, undo and delete | about 60 | mostly the keys that already exist, plus a clipboard query |

The driver's other users also reach into its runner, for menus, zoom and
tutorials.

## The arrangement

- **Already targeted but misplaced: 46 tests (2,471 lines).** They go to their
  own modules' tests, including new test modules for completion and selection.
  Nothing needs exposing.
- **Targeted once the display pieces move: 10 tests (922 lines).** These are
  the completion card tests and `projection_target`'s test. They go to display
  after the card, its placement, the selection highlight and
  `projection_target` move there (item 4).
- **Library projection tests: 39.** They go next to their libraries, through
  the shared harness. Of the 78 tests that use the bench harness, the 57 that
  aren't profiling can switch to the public `project()`; only the 21 profiling
  tests need the two phases exposed.
- **Projection runtime: about 87 tests.** They stay.
- **Rightly end-to-end: about 67 tests (3.9k lines).** These cover arrow
  navigation across projections, picker and typing flows, cross-pane marks,
  focus and palette, library swaps, and lesson flows. They should go through
  the driver once it gains the verbs above.
- **Should become targeted: 30 tests (1,882 lines).** These are line
  write-through, the layout event encoding, toolkit layout and hover through
  throwaway projections, single libraries through the full stack, lessons
  observed through paint, and pure functions inside flow tests. The
  interfaces they need already exist.
- **Captures and perf** go to their own modules. The SVG writer goes to test
  support, and the 2 GPU mesh tests go to puri-vello.
- **No new test system is needed.** The harness costs about 2.5 s. A text
  format would add a parser for little gain, since the table-driven navigation
  tests are already compact. A fuller driver would remove most of the input
  and runner plumbing, about a quarter of the end-to-end lines.

**Surprises.**
- frame/fidget_source.rs repeats a fidget projection test at frame level.
- The driver finds a path's landmark by path alone, so a path drawn in two
  views is ambiguous.
- The default run includes seven SVG captures with few assertions, one of
  which takes 5.9 s.
