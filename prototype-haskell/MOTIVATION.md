# Why This Prototype Exists

This is the Haskell/Wasm exploration for Progred.

Status: parked. The main editor prototype is currently TypeScript/Electron.

The original Haskell spike had three render targets: GTK, ImGui, and
Wasm/DOM. The native probes proved basic GUI viability but did not solve
the product-shape questions: shareability, web/VSC extension potential,
 and escaping native toolkit focus behavior. The active path inside this
prototype is the Wasm/DOM target.

The two reasons to keep investigating Haskell:

1. **Hosted language story.** Progred needs user-defined projections,
   edits, and semantics in a hosted language with a real type system.
   Haskell is the best fit, and the GHC API is the strongest self-hosting
   compiler API available.

2. **DOM distribution story.** GHC's Wasm backend plus JSFFI already lets
   Haskell code call into the browser. The newer `ghc-in-browser` work
   suggests that GHC-as-a-library in the browser may also be possible
   with a packaged runtime/filesystem, so the old assumption that the GHC
   API cannot participate in a browser build needs to be revisited.

The near-term question is narrow: can a Haskell/Wasm core drive enough
DOM interaction to handle Progred's graph/editor model, focus-sensitive
text input, and projection mechanics without recreating the problems we
hit in other GUI frameworks?

If that works, the next question is whether an in-browser GHC service can
typecheck/evaluate user projection definitions with an app-owned package
set.
