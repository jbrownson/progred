# prototype-haskell

Haskell/Wasm prototype for Progred.

Status: this is a Haskell/Wasm/Tauri spike for testing whether Haskell
can own UI state and event interpretation while a thin JavaScript host
only forwards browser events and provides drawing primitives. The current
demo renders directly to a `<canvas>` through JSFFI. Haskell owns the
model, focus state, hit testing, keyboard handling, and draw-command
generation.

This prototype's direction is Haskell compiled to Wasm via GHC's Wasm
backend, talking to Canvas/Web APIs through JSFFI. The Progred HTML host
(`progred/web/index.html`) loads the `.wasm` output, resizes the canvas, forwards
pointer/key events, and exposes a small `window.puriCanvas` drawing
surface.

The reusable UI component is named `puri`. Its Haskell modules live under
the `Puri.*` namespace, while app-specific prototype modules still live
under `Progred.*`.

Requires a native GHC for editor/typechecking and the WASM-targeted GHC
cross-compiler for the app bundle:

```sh
ghcup install ghc 9.12.2
ghcup config add-release-channel cross
source ~/.ghc-wasm/env
ghcup install ghc wasm32-wasi-9.12.2.20250327 -- --host=aarch64-apple-darwin --target=wasm32-wasi --with-intree-gmp --with-system-libffi
```

The default `cabal.project` selects native `ghc-9.12.2` so editor
tooling can typecheck the Haskell code. `cabal-wasm.project` selects
`wasm32-wasi-ghc` for the real app bundle, and the Makefile uses that
project file when building the Wasm executable. The Makefile invokes the
cross compiler through `ghcup run` instead of changing the global active
GHC, so native editor tooling can keep using the native compiler.
It still expects the ghc-wasm-meta environment at `~/.ghc-wasm/env` for
the WASI SDK, Node, Binaryen, and related tools.

Run in a browser:

```sh
make run
```

Run in Tauri:

```sh
cargo install tauri-cli --locked
make tauri-dev
```

Build a Tauri app bundle:

```sh
make tauri-build
```

The Makefile builds the Haskell/Wasm executable, generates
`ghc_wasm_jsffi.js`, installs the web host's npm dependencies, copies the
runtime files into `progred/dist/`, and then serves or wraps that static
directory.

`ghc_wasm_jsffi.js` is the generated JSFFI import object for the current
Haskell source. It is generated as part of `make dist`.

Editor note: each Haskell component has its own source tree. Puri lives
under `puri/src`; Progred lives under `progred/src`; the executable shell
lives under `progred/app`; and the browser host page lives under
`progred/web`. Target-specific code lives with the component that owns it:
`Puri.Platform` is selected by Cabal from either `puri/platform/stub` or
`puri/platform/wasm`, and Wasm-only JS exports live in
`progred/platform/wasm/Progred/Wasm/Exports.hs`. There is no native
app-platform stub because that export module is only listed in the
`os(wasi)` Cabal branch.

The native GTK and ImGui probes were removed. They proved basic native
Haskell GUI viability, but the active question is now whether Haskell can
own the UI model/projection/event logic while the browser/webview remains
the rendering and distribution substrate.

## Layout Notes

`halay` is the Clay-inspired layout library used by the prototype. It is
kept as its own Cabal library so it can eventually move out of this repo
if the direction holds up. `puri` re-exports its geometry types, and
Progred uses Halay to place the current raw graph projection.

Halay's public API describes one concrete layout tree. `measureHalay`
first builds and measures that tree in `measureM`, returning a `Measured`
value with a preferred `measuredSize`. The caller then chooses a final
rectangle and runs `placeMeasured`, which invokes placement callbacks in
`placeM`. In Progred those monads are currently the same rendering monad,
but Halay keeps them separate so a standalone backend can allow text
measurement without also allowing drawing.

`leaf` is intentionally opaque: it supplies one intrinsic `Size` and a
placement callback that receives the final `Rect`. `text` is not just a
leaf helper. Text nodes measure words and natural line height up front,
then choose line breaks after horizontal sizing has produced an available
width, and finally place each line with `textPlaceLine`.

Use `sized` to change a node's width/height sizing, `decorate` to attach
extra placement output to a node, and `aspectRatio` to constrain layout
boxes. Clay applies aspect ratio to layout elements, while text is
width-sensitive content inside those elements; wrap text in a `box` when
the containing text box should be aspect-constrained.

Run the Halay conformance tests with:

```sh
cabal --config-file=.cache/cabal/config test halay-tests
```

For a larger randomized sweep against the vendored Clay oracle:

```sh
HALAY_QUICKCHECK_TESTS=5000 HALAY_TEXT_QUICKCHECK_TESTS=5000 HALAY_TREE_QUICKCHECK_TESTS=5000 cabal --config-file=.cache/cabal/config test halay-tests
```

A fuzz failure prints a `(seed, size)` pair; pass it back through the
matching `HALAY_*_QUICKCHECK_REPLAY` variable to make that case the first
one run. Deep-fuzz mismatches have shown up at roughly one in fifty
thousand tree cases even on known-good states, so treat rare failures as
repro material (capture the replay seed) rather than as immediate
regressions.

The oracle compiles `halay/test/clay_oracle.c`, which includes the
vendored Clay header, and QuickCheck compares Halay placements against
Clay placements while shrinking failures to small repros. Randomized
coverage includes flat box layouts, basic text wrapping, and recursive
layout trees. The recursive generator intentionally avoids degenerate
negative-inner-size cases that can make the Clay oracle nonterminate;
in practice, that means recursive random padding is only generated on
fixed-size axes.

Halay is intended to track Clay's layout model, not Clay's full UI
framework surface. Current-scope Clay features are layout-affecting
configuration: sizing, padding, child gaps, direction, child alignment,
aspect ratio, text measurement/wrapping as input to layout, and clipping
with `childOffset`. `clip.childOffset` is the current Clay mechanism for
scroll-like child positioning; the deprecated `.scroll` config should
not be ported.

Aspect ratio is covered for the direct cases we currently rely on, but
recursive fuzzing still exposes mismatches in Clay's deeper
aspect/percent/grow interactions around compressed or collapsed parents.
Do not paper over those with generator filters; the next step there is a
more faithful port of Clay's aspect passes.

Some Clay behavior that Halay mirrors comes from C memory layout rather
than layout intent: Clay's sizing union overlays `percent` on
`minMax.min`, so percent-sized parents clamp oddly during height
propagation (Clay can collapse them to zero), and the aspect pass writes
heights through that union as `minMax.max`. These puns are load-bearing
for oracle conformance; diverging from them is a deliberate decision to
make against the fuzzer, not a cleanup.

Ignore Clay features that only generate render commands or manage
retained interaction state when asking what remains for Halay. That
includes background colors, overlay colors, corner radius, borders,
images, custom render commands, user data, hover/click helpers,
pointer-over APIs, and Clay-managed scroll helpers. Puri should draw
borders/backgrounds/etc. from Halay's placed rectangles instead of
encoding those as layout concepts.

Likewise, do not port Clay's ID-attached floating element system into
Halay just to support popups or overlays. Floating UI can be modeled as
independent Halay layouts placed into overlay rectangles by the caller;
those overlays do not affect parent layout and do not need Clay's
retained ID machinery.

Ignore Clay transitions for now as well. Clay transitions are a retained
animation system keyed by element IDs: they compare old and new bounding
boxes across frames, keep exiting elements alive, and interpolate
positions, sizes, colors, and border widths. Halay/Puri do not rely on
Clay-style IDs to connect rectangles to views, so animation should be a
separate layer later if needed, not part of the layout port.

If this prototype does grow a layout layer, keep three concerns separate:

1. Box layout: Nic Barker's C Clay layout library is still a good source
   of inspiration for a small, fast, flex-like row/column model with
   fixed/grow/fit sizing, padding, gaps, and measured leaves. There does
   not appear to be a mature Haskell binding for that Clay. The Haskell
   package named `clay` is a CSS EDSL, not that layout engine.

2. Text flow: Clay includes text wrapping, but its core value here is box
   layout rather than text layout. For real text flow, look separately at
   projects such as Pretext, Parley, or Cosmic Text. Progred is unlikely
   to need giant blocks of prose early, so do not make this a dependency
   until a real use case appears.

3. Structured pretty layout: Most of Progred's editor body is likely
   closer to Wadler/Leijen pretty-printing than to flexbox. Pretty
   layout decides whether graph/projection structures render flat or
   multiline, where separators go, and how multiline children affect
   their parents. Box layout then assigns rectangles to the chosen
   leaves. These layers should compose, but they are distinct concerns.

The initial Progred foundation does not need that full pretty-printer
layer. Start with a raw projection that always renders graph structure on
new lines. Revisit interactive pretty-docs when domain-specific
projections need flat-vs-multiline choices.
