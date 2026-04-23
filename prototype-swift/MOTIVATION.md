# Why this prototype exists

Successor to `prototype-rust/`. Moved off egui because its focus model is
fundamentally incompatible with a structured editor:

- `Sense::click()` always sets the focusable bit — every clickable widget
  is automatically a Tab stop. A structured editor needs many
  click-without-focus regions (collapse toggles, field labels, insertion
  points). egui has no `click_without_focus` story.
- `lost_focus()` is render-order-dependent for click-driven transfers
  (egui#2142, unfixed since 2022).
- No focus hierarchy / responder chain — selection is a flat global
  state, not hierarchical.

**Why AppKit:** `acceptsFirstResponder` is per-view and orthogonal to click
handling. NSButton doesn't steal focus on click by default. The responder
chain provides hierarchical focus with innermost-wins semantics. Focus and
click are orthogonal — exactly what a structured editor needs.

See `prototype-rust/AGENTS.md` for the full egui post-mortem.
