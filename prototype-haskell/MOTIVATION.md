# Why this prototype exists

Successor to `prototype-swift/`. Two triggers:

1. **Focus pain wasn't really platform-specific.** AppKit was better than
   egui or DOM, but still required a substantial refactor to take
   navigation away from its key view loop (see `project_focus_refactor_plan`
   in agent memory). Reframing: focus has been the breaking point on
   *every* prototype, suggesting general-purpose UI frameworks are
   structurally wrong for structured editors regardless of which one
   you pick.

2. **The hosted-language story.** The CAD/CAM tool wants an embedded
   scripting language with a real type system. Haskell wins that category
   cleanly — there are no better options. GHC's API gives a clean
   self-hosting embedding story.

Plus: personal preference. The user genuinely loves Haskell and had been
avoiding it only out of concern that others wouldn't.

Render targets being evaluated:

- **gtk/** — native via gi-gtk (GTK4 bindings). GHC API works.
- **wasm/** — browser via GHC's WASM backend + JSFFI for DOM access. GHC
  API does NOT work here (compiler isn't compiled to wasm32-wasi), so the
  embedded-language story would need a custom interpreter.
- **imgui/** — native via dear-imgui + SDL2. Reference point only; no
  built-in focus model to fight, ironically a strength here.

Final platform decision pending.
