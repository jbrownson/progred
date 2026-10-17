# Library vocabulary audit, 17 October 2026

This audit records where the core of progred names the built-in libraries,
meaning everything in progred/src outside libraries/. It asks why each use is
there and whether it should stay. The owner asked for it, expecting the core
libraries (text, names, numbers) to stay referenced and asking where the
library interface could remove other references. A subagent made it at
`26f48b67` and was asked to change nothing; the special cases below were
spot-checked. [Deferred work](../deferred.md#code-size-and-structure) keeps
the conclusions, and this note keeps the evidence.

**Bottom line:**
- The core names a library in 257 places. The view toolkit names none.
- 79 are the registry, the one list that loads every built-in library.
- 54 are core vocabulary to keep: names, text, numbers, bytes, absent and
  paths. 28 of these only reach Grap's own conventions through a library
  re-export, and should call Grap directly.
- 64 are vocabulary that only the core interprets: selection stages, per-site
  state, folds and panes. These should move down into the core, as f64's
  convention lives in Grap.
- 60 are the core doing a library's job, and are candidates for the library
  interface. 41 of those are one block: the projection engine interprets the
  layout library's drawing programs.

**Method.**
- Walked the module tree from lib.rs and dropped every test-only file and
  every `#[cfg(test)]` item.
- Counted every place that names a library item: a constant, a function, a
  type, or a method called on the `Libraries` registry. Re-exports
  (annotations.rs:12, workspace.rs:8, selection.rs:786) and local `use`
  aliases were resolved first.
- Left out the 18 `use` statements, plain field reads such as
  `stack.libraries`, and feature-gated tooling (counted separately below).
- The document-model counts match the [crate split
  analysis](crate-split-2026-10-16.md)'s apart from how conventions are
  counted. The verdicts are judgment.

## Uses, by purpose

| Library | What the core uses it for | Where (system) | Uses | Verdict |
|---|---|---|---:|---|
| selection | Stores the selection's stage (edge, pending or label) as the library's cells in the selection payload, and decodes it | selection.rs:86-93, 190-194, 810-831 (model) | 10 | Move down |
| selection | After a commit, says "select this" or "open a pending here" through the library's `at`/`edge`/`pending_at`, which build the core's own `site::Continuation` (libraries/selection.rs:22-40) | editing.rs:75-77 (model); commands.rs:172-174, completion.rs:77, 274 (shell) | 7 | Move down |
| selection | Implements Grap's `selection get/set` for event handlers; the library only names them | site.rs:89-90, 246-285 (model) | 7 | Move down |
| site | Implements Grap's per-site state: `get`, `set`, `site path` | site.rs:86-88, 236-298 (model) | 12 | Move down |
| site | Keeps fold overrides as `{fold: folded or expanded}` in each view's annotation store; undo restores only that field | annotations.rs:88-101, workspace.rs:347 (model) | 7 | Move down |
| workspace | Reads pane declarations and declared libraries from the root, and moves panes between columns | workspace.rs:119-286 (model) | 18 | Move down |
| workspace | Uses the `libraries` cell as the ID of the "document libraries" hide toggle | stack.rs:42, 88 (shell) | 2 | Move down |
| presentation | Reads the `projection` field of a declared document library | workspace.rs:177 (model) | 1 | Move down |
| presentation | Wires presentation into every pane, and runs document libraries through it | stack.rs:139, 144 (shell); projection/viewport.rs:27, 37 (engine) | 4 | Library interface |
| layout (+2 absent) | Interprets the layout library's drawing programs | projection/drawing.rs:158-299, 346 (engine) | 41 | Library interface |
| layout | Hands every event handler its event under layout's `event` label | site.rs:122 (model) | 1 | Library interface |
| name, text, blob, f32, f64, u64 | Decides which libraries make up Plain | stack.rs:47-75 (shell, 9); web_embed.rs:157-176 (platform, 4) | 13 | Library interface |
| name | Tab to the next empty slot skips empty name slots | navigate.rs:240 (shell) | 1 | Library interface |
| name | Names cells: `Sources::name` behind `Env::name` (the fallback view and the picker), text-file binders, and a new label's name | sources.rs:194, gid_text.rs:316, 355 (model); completion.rs:273 (shell) | 5 | Keep |
| text | Text in the selection payload (the query and the live editor text); typed queries and the clipboard spell text as `"…"` | selection.rs:664-693, 817-963 (model) | 11 | Keep |
| text | Text values never fold | selection.rs:745 (model) | 1 | Keep, through the codec |
| text | String literals in the GID text format | gid_text.rs:189, 520-524 (model) | 3 | Keep |
| text | The picker lists text readings last; label queries drop their quotes | completion.rs:238, 262 (shell) | 2 | Keep |
| blob | `0x…` queries and pastes read as bytes | selection.rs:668; completion.rs:257, 292 | 3 | Keep, through gid |
| f64 | Payload numbers: picker choice and scroll, caret offsets, drag point | selection.rs:818-970 (model) | 16 | Keep, calling Grap directly |
| absent | Results and declines of the site and selection functions | site.rs:28, 244, 254, 274, 291 (model) | 5 | Keep, calling Grap directly |
| path | The site path and the selection-set path; locations sent as JSON to the web page | site.rs:237, 262, 264 (model); web_embed.rs:269-316 (platform) | 7 | Keep, calling Grap directly |
| grap | Tutorial slots show a named cell by its name | web_embed.rs:250 (platform) | 1 | Keep |
| registry | The registry types and the built-in library list | below | 79 | Registry |

## Special cases: the core branching on one library's cells

1. **Drawing programs.**
   - projection/drawing.rs:233-297 dispatches on the layout library's `fill`,
     `path`, `move to`, `line to` and `close`.
   - Lines 160-200 take fast paths for its `rect`, `circle`, `rounded rect`,
     `translate` and `rotate`.
   - The call comes from the layout library itself: layout.rs:652-661 →
     display/mod.rs:1061-1076 → `program_leaf`. That toolkit function exists
     only for this callback.
   - The layout library already runs its other layout functions through a
     foreign overlay (layout/scope.rs:9, 107), as toolpath, tree and controls
     do for theirs.
2. **Site and selection functions.**
   - site.rs:236-299 implements them by comparing the called function with
     those libraries' cells.
   - Both libraries say they contribute names only (libraries/site.rs:1-4,
     libraries/selection.rs:1-3).
   - `selection set` also borrows site's `value` cell, both as its argument and
     as the detail label of its invalid-path absence (site.rs:265, 269-270).
3. **Handler event label.** site.rs:122 passes `{event: …}` using layout's
   cell. The call itself comes from layout (layout/events.rs:51, 81, 133 →
   editing/scope.rs:281-295).
4. **Selection stage.**
   - selection.rs:86-93 and :190-194 compare the payload's stage with the
     library's `pending` and `label`.
   - The payload's vocabulary is split: the stage cells live in the library,
     while the query, choice, caret and drag cells live in the core
     (selection.rs:786-807).
   - The library builds the edge payload (selection.rs:810-812), and the core
     builds the pending and label payloads (:814-828).
5. **Folds.** annotations.rs:89-91 and :99-100 read and write `fold`,
   `folded` and `expanded`, and undo restores only that field
   (workspace.rs:347).
6. **Panes and declared libraries.**
   - workspace.rs:241-245 matches `left` and `right`.
   - workspace.rs:150-183 reads `libraries` and `keys`, and presentation's
     `projection`.
   - stack.rs:42 and :88 reuse the `libraries` cell as a toggle ID.
7. **Pane presentation.**
   - stack.rs:141-145 makes presentation's `projected_display` the entry
     projection of every pane.
   - frame.rs:543-563 (via projection/viewport.rs:14-39) gives a pane holding
     presentation's `{value, viewport}` its assigned size and no margin.
   - stack.rs:136-140 always runs document libraries through presentation's
     `document_libraries`.
8. **Plain.** stack.rs:47-75 hard-codes name, text and blob, plus the f32, f64
   and u64 number displays. The tutorial page has a second copy,
   web_embed.rs:157-176, and it already differs: it has f64 only, and its
   caption at :98-100 says so.
9. **Tab skips names.** navigate.rs:240 leaves out slots whose last step is
   the name library's `name` cell. This runs when Tab is pressed in the picker
   (projection/completion.rs:273-287).
10. **Text never folds.** selection.rs:745-751 tests the `utf8` field on a
    runtime value, re-implementing `text::read`.
11. **Picker order.** completion.rs:234-238 sorts offers whose preview is text
    after the other readings.
12. **Literal syntax, which stays.**
    - `0x…` becomes bytes and is listed first (completion.rs:256-259, 291-292;
      selection.rs:668).
    - `"…"` is text (selection.rs:666-686; gid_text.rs:189, 519-526).
    - A typed label becomes a new cell named with `name::record`
      (completion.rs:261-276).

## Library interface candidates

These would join today's `Library { definitions, projection, completions }`
(libraries/mod.rs:168-172).

- **A plain projection.** Each library would say how its values read as
  themselves, and Plain, in the app and in the tutorials, would compose the
  libraries that provide one. This removes 13 references and the duplicated
  list.
- **A pane entry.** A projection for a pane's root that receives the pane's
  assigned size and says whether it fills the pane. It covers
  `projected_display` and viewports, so frame.rs no longer special-cases
  presentation.
- **A wrapping projection.** It is built over everything loaded and appended
  after the rest; this is exactly `document_libraries(loaded)`.

The host and toolkit would also change:

- **A recorded-drawing leaf.** The core keeps replay, hit-testing and source
  highlighting. The layout library records `fill(shape, paint, transform,
  call trace)` from its own overlay and keeps reading shapes, transforms and
  paints. This removes the engine's 41 references.
- **Handlers that take arguments.** The core would offer a call taking the
  view, path, function and arguments, and layout would supply its own
  `[(event, …)]`.
- **Optional slots.** A flag on a slot that the name library sets and Tab
  navigation honors.

## Moving ownership down

This follows K5 in the crate split analysis.
- The site, selection (stage and capability), fold and workspace cells move
  next to site.rs, selection.rs, annotations.rs and workspace.rs.
- The selection helpers `at`, `edge`, `pending` and `pending_at` move with
  them.
- The libraries re-export the cells and keep their name records; no cell ID
  changes.

Several libraries already use these as host services: fidget.rs:1243-1257;
grap.rs:88-90, 259-260, 581-583, 609-620; grap/suggestions.rs:82; color.rs:200;
and completion.rs:11-13. Together they amount to a draft of the host
interface's write side: select after a commit, per-site state, and templates
with panes.

## Keeping core vocabulary

- **f64 (16), absent (5), path (7).** These should call `::grap::f64`,
  `::grap::absent` and `::grap::path` directly. The library versions are
  re-exports or one-line wrappers (libraries/f64.rs:57-60, absent.rs:24-42,
  path.rs:7), and site.rs:263 and drawing.rs:252, 288 already call Grap
  directly next to them.
- **blob (3).** gid has a private hex parser (gid/src/value.rs:363) that
  duplicates `blob::parse`. Exposing it would let these uses go through gid.
- **name (5) and text (17).** These stay, as the owner expected. name.rs:1-2
  says "This editor assumes the library." But while the core names
  `libraries::text`, the text library can't be a plug-in. The f64 pattern
  fixes that: move the codec, about 40 lines (`value`, `read`, `record`),
  below the boundary, and let the libraries keep their names, functions and
  projections.
- **The picker.** completion.rs already gets names through `Env::name`
  (completion.rs:381-388). Of its 7 references, 2 are selection helpers that
  move down. The other 5 are Raw's literal syntax, which would go with the
  picker when it becomes a widget.

## Registry (79)

- **stack.rs, 64.** The built-in list `BUILT_INS` (29 libraries, each an ID
  and a constructor, 58 references, :151-181), plus `from_contributions`,
  `named()`, the `Library` type and `completion::combine` (6). This is the one
  place that legitimately names every library.
- **Elsewhere, 15.** These only name the registry's data type, `Libraries`,
  in sources.rs, selection.rs, computations.rs, projection/line_control.rs,
  gesture.rs and web_embed.rs. If `Definitions`, `Library` and `Libraries`
  moved into the core (libraries/mod.rs:97-362), stack.rs would be the only
  file that imports libraries.

## Summary

| Library | Model | Engine | Shell | Platform | Total | Verdict |
|---|---:|---:|---:|---:|---:|---|
| selection | 19 | | 5 | | 24 | move down |
| site | 19 | | | | 19 | move down |
| workspace | 18 | | 2 | | 20 | move down |
| presentation | 1 | 2 | 2 | | 5 | 1 move down, 4 interface |
| layout | 1 | 39 | | | 40 | interface |
| absent | 5 | 2 | | | 7 | 5 keep (Grap), 2 interface (with drawing) |
| path | 3 | | | 4 | 7 | keep (Grap) |
| f64 | 16 | | 2 | 1 | 19 | 16 keep (Grap), 3 interface (Plain) |
| f32, u64 | | | 4 | | 4 | interface (Plain) |
| text | 15 | | 3 | 1 | 19 | 17 keep, 2 interface (Plain) |
| name | 4 | | 3 | 1 | 8 | 5 keep, 3 interface |
| blob | 1 | | 3 | 1 | 5 | 3 keep, 2 interface (Plain) |
| grap | | | | 1 | 1 | keep |
| registry | 9 | 2 | 65 | 3 | 79 | registry |
| **Total** | **111** | **45** | **89** | **12** | **257** | |

**Tooling, not in the totals.** Feature-gated code adds 33 more references:
control/computer.rs 17, orbit_profile.rs 9, control.rs 5, and two cam-profile
re-exports in lib.rs:82 and 84. control/computer.rs is already a library
defined outside libraries/ and registered at run time (control.rs:102-106). It
shows what a plug-in actually uses: `Library::named`,
`Definitions::from_parts`, the name, text and png codecs,
`absent::with_reason` and `f64::read`.

**Not audited:** the other direction, libraries naming the editor (about 220
mentions of `crate::Editor`), which the crate split analysis covers.
