# OUTRAM PARK backend — workspace TODO

## Android target support (assessed 2026-07-04)

Feasibility probe for compiling the eframe/egui simulators to Android
(`aarch64-linux-android`). Driven by `fhr_sim_v2` in `tampines-steam-tables`.

**Verdict: feasible.** `cargo check --example fhr_sim_v2 -p tampines-steam-tables
--target aarch64-linux-android` type-checks nearly the entire tree —
`android-activity 0.6.1`, `wgpu`, `winit`, `egui`/`egui_plot`/`egui_extras`, and
`local-ip-address` all compile for Android. No native BLAS is in the GUI chain
(`tampines → tuas` is `peroxide` pure-Rust + `ndarray`; OpenBLAS is confined to
the neutronics crates). The sole failure is `ring` (C/asm crypto) for lack of an
NDK C compiler, and `ring` is droppable (HTTP image loader only).

Full findings and per-crate notes:
- `crates/tampines-steam-tables/TODO.md` — canonical work-item list for the
  `fhr_sim_v2` port.
- `crates/tuas_boussinesq_solver/TODO.md` — why the TH backend is Android-clean
  (no `ndarray-linalg`; keep it that way).

### Workspace-level items

- [ ] **Decide feature gating for Android.** The `egui_extras` `all_loaders`
  feature (root `Cargo.toml`) pulls `ehttp → ureq → rustls → ring`. For Android,
  build with `["image"]` only. Choose whether this is a per-target feature, a
  Cargo feature flag, or a dedicated Android app crate. *(Affects any eframe
  example in the workspace, not just `fhr_sim_v2`.)*
- [ ] **Provide the Android toolchain path.** NDK + SDK install (needs sudo on
  the dev machine), `rustup target add aarch64-linux-android` (done), and a
  packager (`cargo-apk` or `xbuild`). Document the exact commands in
  `docs/workspace-maintenance.md` once proven end-to-end.
- [ ] **Confirm `local-ip-address` runtime behaviour on Android** (needs
  INTERNET permission in the manifest; compiles fine, runtime unverified).
