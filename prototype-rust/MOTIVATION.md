# Why this prototype exists

Successor to `prototype-ts/`. Goals:

- **Modern stack** — TS/Electron felt aged.
- **Demo the raw graph format from the start.** The TS prototype required a
  bootstrapped semantics graph to function at all, which meant explaining
  the project always began with a chunk of hand-waving about preconditions.
  This prototype was built to show the raw graph directly — with a
  graphical node/edge representation of the graph itself and identicons
  to visually represent UUIDs without needing names in play.
- **Escape the DOM.** Focus management on the DOM was the final straw —
  every workaround fought the browser's global `activeElement` model.

**Why Rust + egui:** Rust as the modern systems language. egui was reportedly
among the more mature Rust GUI frameworks at the time, was cross-platform
(like most), and immediate-mode seemed like a good fit for a structured
editor where the display is derived from the underlying graph each frame.

The egui choice didn't pan out — see `AGENTS.md` for the focus-model
post-mortem that drove the move to Swift/AppKit.
