# AGENTS.md — bugis

**BUGIS — Browser-based User Graphics Interaction System.**
The workspace's home for colour theming in general (maintainer, 2026-11-02),
starting with the shared Gruvbox egui theme, and **the central GUI crate**
(maintainer, 2026-11-02: "the gui portions, eg widget library and such, should
then live in bugis too"; the plan is gh:#1112). The workspace root `CLAUDE.md`
binds here in full.

## Rules

- **Every egui simulator in the workspace takes its theme from here**
  (maintainer, 2026-11-02): Gruvbox Dark by default, `bugis::selector` in the
  side panel. A new simulator calls `bugis::Theme::default().apply(ctx)` at
  start-up. Do not add a second copy of the palette or of `gruvbox_visuals`
  anywhere; extend this crate instead.
- **`no_std` + `alloc` core, AGPL-3.0-only** (maintainer, 2026-11-02).
  Anything needing `std` or egui goes behind a feature (`egui`, on by
  default). Check with `cargo build --release -p bugis --no-default-features`.
- **`egui` only, never `eframe`**, so any crate can depend on it without a
  windowing stack (Android and wasm32 stay clean).
- ~~**Never depend on a physics crate** (tampines, nee_soon, …): bugis holds
  the physics-agnostic GUI; adapters from physics types stay with the physics
  consumer (gh:#1112).~~ **CHANGED 2026-11-02 (maintainer: "Integrate them,
  I'm ok with dependencies"):** the whole widget library moves here, including
  the widgets that read `tampines` / `nee_soon` state. Those physics crates are
  **optional dependencies behind their own features** (std), so the no_std
  core and the plain `egui` feature stay physics-free. **CORRECTED 2026-11-05
  (maintainers: "bugis should be standalone"; "widget art can live in bugis, the combination of logic and art can live inside the digital twin engine"):** the widget library is split.
  The **art** (drawing from plain numbers) stays here behind `widgets`; every
  widget that holds a physics component (`PipeVisual`, `CondenserVisual`,
  `CoolingTowerVisual`, `HeatExchangerVisual`, `PumpVisual`,
  `SteamGeneratorVisual`, `TurbineVisual`, `ValveVisual`,
  `ReactorVesselVisual`, `PipeComponent`) moved to
  `outram_park_digital_twin_engine::components` and draws through the art type
  of the same stem here (`PipeArt`, `CondenserArt`, ...). Every physics
  dependency went with them. `cargo publish` resolves optional and dev dependencies too, and
  the physics crates dev-depend on bugis for their GUI examples' theme, so
  those optional edges were publish-time cycles. **bugis depends on no
  workspace crate except `petir`** (bottom of the graph); keep it that way,
  dev-dependencies included.
- **Moved modules (gh:#1112, 2026-11-02) keep their own rules here.**
  `animation` (feature `animation`), `ascii` (feature `ascii`), the widget
  library `components` (feature `widgets`; since 2026-11-05 the art only, see
  above) and `app_scaffold` (feature `app_scaffold`) came from
  `outram-park-digital-twin-engine`, which re-exports each at its old path.
  The engine `CLAUDE.md`'s rules move with them and bind here: **animation is
  derived from physics state, never hardcoded** (HARD RULE); **tracer state
  is application-owned** (a widget never owns its `TracerTrain`); **keep
  `animation` egui-free** (it is the Android-building module:
  `cargo check --release -p bugis --no-default-features --features animation
  --target aarch64-linux-android`); **enum dispatch, never trait objects**;
  scalar-backed widgets are not placeholders. The engine's kovan review
  records for these folders stay at the old paths.
- **`web_demo` (feature `web_demo`) came from `dhoby-ghaut`** (gh:#1112),
  which re-exports it as `dhoby_ghaut::web_demo`. The mobile-first and
  no-lagging HARD RULES bind it
  (`docs/claude-md/mobile-first-tutorials-and-demos.md`), and so does
  dhoby-ghaut's "how a new track app plugs in" guide. Its wasm-bindgen family
  sits in the `cfg(target_arch = "wasm32")` table, so it is a no-op on native
  targets.
- **Every moved module needs `std`**: the crate is
  `#![cfg_attr(not(feature = "std"), no_std)]`, and each moved feature
  implies `std`. Nothing std goes into the core modules.
- **Only Gruvbox Dark and Gruvbox Light** (GitHub issue #30: "only gruvbox
  light and dark work"). Add a theme only when the maintainer asks.

## Avoid writing macros (workspace rule, 2026-11-05)

Maintainer direction: macros make code harder to read and review. Do not
write new `macro_rules!` or proc macros in this crate; use functions,
generics, enums and plain `match` arms. Using std and dependency macros
(`vec!`, `format!`, derives) is fine. Full rule: root `CLAUDE.md`, "Avoid
writing macros".
