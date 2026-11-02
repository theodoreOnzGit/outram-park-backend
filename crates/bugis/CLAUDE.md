# CLAUDE.md — bugis

**BUGIS — Branding, User-interface and Graphics for Interactive Simulators.**
The shared Gruvbox egui theme. The workspace root `CLAUDE.md` binds here in full.

## Rules

- **Every egui simulator in the workspace takes its theme from here**
  (maintainer, 2026-11-02): Gruvbox Dark by default, `bugis::selector` in the
  side panel. A new simulator calls `bugis::Theme::default().apply(ctx)` at
  start-up. Do not add a second copy of the palette or of `gruvbox_visuals`
  anywhere; extend this crate instead.
- **`egui` only, never `eframe`**, so any crate can depend on it without a
  windowing stack (Android and wasm32 stay clean).
- **Only Gruvbox Dark and Gruvbox Light** (GitHub issue #30: "only gruvbox
  light and dark work"). Add a theme only when the maintainer asks.
