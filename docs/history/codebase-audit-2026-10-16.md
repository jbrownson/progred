# Codebase audit, 16 October 2026

A survey of where the code is, what could go, what could be simpler, and where
crate boundaries could fall, made at `66779f63` by a subagent asked to change
nothing. It was prompted by agent sessions growing slow: each task read a lot
of code, and progred is one 94k-line crate. [Deferred work](../deferred.md#code-size-and-structure)
keeps the recommendations; this note keeps the evidence.

**Bottom line:** the size isn't in dead code. 60% of progred is tests (56,345 of
94,158 lines), much of it inside the source files, so the cheapest win for
reading is moving those inline tests into separate files. A crate split that
would really cut compile times is blocked: 93% of the production code is one
dependency cycle.

**Method.** Each crate's module tree was walked from `lib.rs`, following `mod`
declarations and their `cfg`s. Test code was measured by brace matching on
`#[cfg(test)]` items, with comments and strings masked. Every `.rs` file on disk
is reachable, so there are no orphan files. Dependencies are `crate::`/`super::`
paths and `use` statements in non-test code. Churn comes from `git log` on
progred/src: 256 commits since 2026-09-16 and 438 since 2026-07-16.

## Where the size is

Production code in progred is 35,392 lines (about 1.26 MB); tests are about
2.05 MB.

| progred/src area | Files | Production | Tests | Other¹ |
|---|---:|---:|---:|---:|
| libraries | 93 | 15,807 | 21,435 | 279 |
| projection (2.3k engine plus editor-level tests) | 42 | 2,286 | 21,183 | 27 |
| display | 32 | 4,902 | 3,022 | 9 |
| editor, app, lib, commands, command, completion, stack | 7 | 3,705 | 104 | 28 |
| input, placed, menu, navigate, hover, gesture | 7 | 2,186 | 3,226 | 27 |
| selection, editing, location, sources, site | 7 | 2,092 | 1,507 | 4 |
| workspace, model, annotations, computations, gid_text, text_store | 6 | 1,777 | 1,485 | 28 |
| frame | 1 | 911 | 1,685 | 0 |
| platform (web\*, macos\*, native_menu) | 10 | 1,199 | 77 | 629 |
| test/tool-only (tutorials, control, driver, sample, shell_tests…) | 15 | 0 | 2,221 | 1,350 |
| small (spine, filter, history, timers, render, clipboard…) | 10 | 527 | 400 | 40 |
| **Total** | 230 | **35,392** | **56,345** | 2,421 |

¹ Built only for tests plus a feature or the browser (`control`, `driver`,
`web_embed`, `web_scroll`), or only for profiling.

The largest libraries, as production / test lines: toolpath 3,641 / 8,081,
fidget 2,170 / 2,423, layout 1,639 / 1,313, color 1,241 / 404, controls
1,100 / 1,881, control 935 / 1,433.

| Other crates | Production | Tests |
|---|---:|---:|
| grap | 3,181 | 3,286 |
| ui/puri | 3,286 | 2,607 |
| ui/puri-vello | 1,290 | 852 |
| ui/measured | 1,229 | 798 |
| incremental | 1,127 | 1,230 |
| ui/puri-widgets | 1,118 | 816 |
| gid | 832 | 619 |
| ui/puri-web / ui/uig | 329 / 71 | 0 / 32 |

| Largest first-party files² | Production | Tests |
|---|---:|---:|
| grap/src/lib.rs | 2,825 | 1,360 |
| progred/src/frame.rs | 911 | 1,685 |
| progred/src/libraries/control.rs | 915 | 1,134 |
| progred/src/projection/tests/completion.rs | – | 2,009 |
| progred/src/libraries/fidget.rs | 1,263 | 704 |
| ui/puri/src/edit.rs | 867 | 1,032 |
| progred/src/projection/tests/frame/interaction.rs | – | 1,780 |
| progred/src/projection/tests/editing.rs | – | 1,777 |
| progred/src/libraries/controls/tests.rs | – | 1,720 |
| progred/src/libraries/layout.rs | 883 | 828 |
| progred/src/libraries/grap.rs | 669 | 1,021 |
| progred/src/projection/tests/frame/layout.rs | – | 1,586 |
| progred/src/input.rs | 501 | 1,016 |
| progred/src/projection/tests/frame/svg.rs | – | 1,489 |
| progred/src/display/mod.rs | 1,058 | 228 |

² Vendored Fidget (26k lines) is excluded.

| Docs | Lines | KB |
|---|---:|---:|
| docs/projections.md | 1,009 | 65 |
| docs/puri.md | 686 | 43 |
| docs/deferred.md | 742 | 42 |
| docs/toolpaths.md | 609 | 40 |
| docs/model.md | 591 | 38 |
| docs/incremental.md | 335 | 21 |
| README.md | 255 | 13 |
| AGENTS.md | 160 | 8 |
| All current docs (docs/*.md, AGENTS, README, MOTIVATION, CLAUDE, vendor/README) | 6,357 | 383 (~95k tokens) |
| AGENTS.md, docs/README.md and the 7 core references | – | 198 (~50k tokens) |
| docs/history (not instructions, but grep still searches it) | 10,672 | 606 |

## What may not need to be there

- **There is essentially no dead code.** Of 3,082 non-test item definitions,
  leaving out trait-impl members, only 14 names occur nowhere else, and all are
  FFI entry points: `#[wasm_bindgen]` exports in web.rs, web_worker.rs,
  web_input.rs and web_render.rs, and an Objective-C selector at
  native_menu.rs:34. The 10 `dead_code` allowances are all `cfg_attr`s for
  browser or test builds. This matches rustc's dead_code lint, which
  `make check` runs, and earlier cleanups such as dafc4549 and cb39174f.
- **Some public API is used only by tests,** which rustc doesn't flag in library
  crates: puri's `handler::dispatch_*` (7 functions), `delim::open_fitted` and
  `close_fitted`, and `text::line_layout`; grap's `evaluate_at`,
  `evaluate_value_scoped`, `call_prepared_value`, `eval_value` and `merge_all`;
  measured's `place_at` and `place_top_left`; puri-widgets'
  `range_slider::item_at`; puri-vello's `uploaded_images` and
  `scratch_textures`. A small amount of code.
- **Test scaffolding is duplicated.** There are 19 ad-hoc `display::Env` test
  doubles in 12 files, six named `NoEval`, most only panicking (for example
  libraries/color.rs:809, libraries/text.rs:218, libraries/control.rs:930,
  libraries/fidget.rs:1951, libraries/mod.rs:406, display/mod.rs:1138). Test code
  defines `fn evaluate` 29 times and `fn apply_scoped` 19 times. Six 13-line
  projection-input builders are near-identical: libraries/control.rs:967 and
  :983, f64.rs:273, grap.rs:852 and :869, fidget/projection/tests.rs:44.
- **Some code may be retired; that's the owner's call.** The September
  profiling scaffolding is about 0.9k lines plus 37 `cfg` hooks in 14 files: the
  `cam-profile` and `layout-profile` features, orbit_profile.rs, three
  `performance.rs` modules, display/profile.rs and two examples. It is still
  documented, and `make check` still builds it. sample.rs (364 test-only lines)
  builds a sample document in Rust whose 15 vocabulary IDs also appear in
  examples/sample.gid; whether the tests could parse the `.gid` file instead
  wasn't checked.
- **Comments are not the problem.** They are 5.1% of production lines; only
  seven comment blocks run 8 or more lines, and there is no commented-out code.
- **Duplication in production code is small:** 10 identical function bodies
  (46 extra lines). Most near-duplicates are the `library()` constructor each
  library repeats, or parallel pairs such as `convention()` in f32.rs:68 and
  u64.rs:67.

## What could be simpler

- **Tests sit inside the source files.** The 102 progred files with inline tests
  total 49,821 lines, 42% of them tests; frame.rs is 911 lines of code and 1,685
  of tests. Another 111 test-only items (751 lines) are mixed in among the code
  in 37 files, 32 of them in libraries/layout.rs. The repo already keeps tests in
  separate files (`tests.rs` or a `tests/` directory) in about 60 places.
  projection/tests holds 20.8k lines of editor-level tests, which makes a
  2.3k-line projection engine look like 23.5k lines.
- **Which stored location an occurrence edits is answered across ten files:**
  location.rs, editing/scope.rs, display/conject.rs, display/widget/project.rs,
  display/widget/source.rs, projection/mod.rs (which builds the scopes),
  projection/source_link.rs, selection.rs (which carries the scope and writes by
  `Location`), sources.rs and hover.rs. `Location` appears in 12 production files
  across 10 top-level modules, and `Scope` in 11 files across 8. The generic
  display layer exposes it in its own API (`Project::jump(…, document: Location,
  conject: Conject)`). That is why 0e98ca2b changed production code in 20 files
  across 14 modules, plus 16 test files; 586 of its 1,012 added lines were
  tests.
- **One concept is split across same-named files.** There are five `completion`
  modules (1,130 production lines), three `projection` modules, and two each of
  `structure`, `frame`, `gesture`, `hover`, `selection`, `site`, `workspace`,
  `location` and `scope`. "Source" names at least six different things:
  `Sources`, `DefinitionSource`, `Scope::source()` (which returns a `Location`),
  `SourceTrace`, `source_link` and `SourceCalls`.
- **Some files mix several concerns.** display/mod.rs holds the completion API,
  the projection API, layout combinators, and record helpers specific to the
  editor. selection.rs holds the `Selection` type, about a dozen `pending_*`
  navigators, document writes, the clipboard codec and payload encoding.
  grap/src/lib.rs holds runtime values, a ~1,300-line evaluator, and 13
  evaluate/apply entry points. app.rs has 81 platform `cfg`s in 1,071 lines.
- **The concrete type pair is written out in full:**
  `<crate::Editor, crate::frame::Hovered>` appears 332 times in 69 files, and
  only three type aliases exist.

## Crate boundaries

Every production module except a few leaves belongs to one dependency cycle:
30 top-level modules holding 32,773 of the 35,392 production lines (93%). The
table counts non-test references between candidate layers; bold cells point
upward, against the layering, and are the cycles.

| from \ to | model | display | libraries | shell | app |
|---|---:|---:|---:|---:|---:|
| model (4.2k lines) | – | **15** | **26** | **3** | 0 |
| display (4.9k lines) | 22 | – | 0 | **222** | 0 |
| libraries (16.0k lines) | 56 | 355 | – | **430** | 1 |
| shell (7.7k lines) | 94 | 143 | 15 | – | 18³ |
| app (3.0k lines) | 10 | 14 | 34 | 36 | – |

The layers: **model** is location, sources, selection, editing, site,
workspace, model, annotations, computations, gid_text and text_store, among
others; **shell** is editor, frame, projection, input, menu, placed, navigate,
hover, gesture, completion and the command modules; **app** is the platform
files, lib.rs and the stack.rs library registry.

³ Mostly lib.rs helpers such as `UserEvent` and `content_viewport`.

What forms the cycles:

- **libraries→shell and display→shell:** about 635 uses of the concrete
  `Editor` and `frame::Hovered` types across 51 files, plus about a dozen shell
  helpers such as `gesture::scoped_value_edit`, `projection::Cx` and
  `completion::typed`. `Editor` also stores a `Stack<Editor>` holding the
  libraries' projections, so the cycle is built into the types.
- **model→libraries:** vocabulary constants and the `Libraries` type.
- **model→display:** `Env`, `ResolvedCell`, `Conject`, `LineEdit` and
  `DocumentLibrary`.
- **model→shell:** `editing::Access` and `site` take `&mut Editor`.

In this table, "most-edited crate" means progred's core, or the shell in split
C, and "lines not recompiled" is the average share of production lines that
would not rebuild per commit, over the last month.

| Split | Moved out of the most-edited crate | Lines not recompiled | Blockers |
|---|---|---|---|
| A. CAM geometry crate: toolpath cutter, profile, stock and paths; fidget raster, partial and interaction | ~1.6k production lines with no references to editor code | ~4%. Only 21 of 256 commits touched it, and it could build optimized like the other dependencies | Few. Some of its tests are editor-level and would stay behind |
| B. Leaf libraries above the core: toolpath, fidget, controls, tree, list, sequence, png, random, geometry, conversion | 8.4k production + 13.8k test lines | 14% (27% per edited file). But 84% of commits touch the core and still rebuild everything | The stack.rs registry, ~88 core paths to make `pub`, ~25 test files or modules to move |
| C. model, then display, libraries, shell, app | 25k production lines below the shell | 31% (36% over 90 days). The 31–41% of commits that touch only the shell or app would skip 25k lines | The whole cycle above |

Split A is cheap but small; split B costs more than it saves; split C is the
one that would pay off, and the `Editor`/`Hovered` coupling blocks it. Line
counts also overstate the time saved: rustc's incremental compilation already
reuses unchanged items within a crate, downstream crates rebuild whenever an
upstream one changes, and generic code instantiated in the shell is compiled
there anyway. Measure with `cargo build --timings` before committing to a split.

## Recommendations, ranked

| # | Change | Payoff | Effort / risk | Evidence |
|---|---|---|---|---|
| 1 | Move the inline test modules and interleaved test-only items into sibling `tests.rs` files across the workspace, and move projection/tests' editor-level suites out from under projection/ | High for reading: 42% of the lines in those 102 files are tests, and there are 27k inline test lines across the workspace | Low / very low; a mechanical move the repo already uses | Size, simplicity |
| 2 | Keep docs/history out of default search with a ripgrep `.ignore` file, and point agents to the one doc section they need, with sizes | Medium: the AGENTS.md reading path is ~50k tokens, and history adds 606 KB of grep noise | Trivial / low | Docs table |
| 3 | Share one `Env` test double and the common evaluate/apply/projection-input helpers | Medium: dozens of local copies become one place to learn | Low–medium / low (tests only) | Scaffolding |
| 4 | Add type aliases for the concrete widget types, and give the six "source" concepts distinct names | Easier reading, and a later generic split becomes a local change | Low / very low | 332 written-out type pairs |
| 5 | Give the occurrence-to-location mapping one owning module, and pass an opaque handle through display and libraries instead of `Location` and `Conject` | Medium–high for future changes in the most cross-cutting area | Medium–high / medium; must stay behaviour-neutral | 0e98ca2b: 20 production files, 14 modules |
| 6 | Move the model-level types out of display and the vocabularies out of libraries, and make the write path take the model rather than `&mut Editor` | Removes the 44 upward references from the model layer; needed before any split | Medium / low | Crate boundaries |
| 7 | Split A | Small; might also speed up CAM tests (unmeasured) | Low / low | Crate boundaries |
| 8 | Split C, only after 4–6 and a timings baseline | ~31% of production lines not recompiled per commit (lines, not measured time) | High / medium–high | Crate boundaries |

Not recommended: split B, or a dead-code hunt.
