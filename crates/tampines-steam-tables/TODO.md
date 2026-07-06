# tampines-steam-tables — TODO


## Android build for `fhr_sim_v2` (feasibility confirmed 2026-07-04)

Goal: compile the `fhr_sim_v2` eframe/egui simulator (`examples/fhr_sim_v2/`)
for Android (`aarch64-linux-android`) and package it as an installable APK.

**Verdict: feasible.** A `cargo check --example fhr_sim_v2 -p tampines-steam-tables
--target aarch64-linux-android` type-checks almost the whole tree, including the
parts that usually break Android ports:

- `android-activity v0.6.1` compiles — eframe 0.34.3 pulls in its Android winit
  backend for this target.
- `wgpu` / `winit` / `egui` / `egui_plot` / `egui_extras` / `local-ip-address`
  all check clean.
- **No native BLAS in the chain.** `tampines → tuas_boussinesq_solver` uses
  `peroxide` (pure-Rust, default features) + `ndarray`, *not*
  `ndarray-linalg`/OpenBLAS. (The workspace's OpenBLAS dep lives only in the
  neutronics crates, which `fhr_sim_v2` never touches.) See
  `../tuas_boussinesq_solver/TODO.md`.

The **only** compile failure was `ring` (C/assembly crypto), and purely because
no Android NDK C compiler (`aarch64-linux-android-clang`) is installed — a
toolchain gap, not unportable code. `ring` is not load-bearing here; it enters
via the HTTP image loader only:

```
ring ← rustls ← ureq ← ehttp ← egui_extras (all_loaders feature)
```

### Work items (effort order)

- [ ] **Drop the HTTP image loader for the Android build.** Change
  `egui_extras` from the `all_loaders` feature to `["image"]` (the app only
  loads embedded/local images via `install_image_loaders`, no HTTP). This
  removes `ehttp → ureq → rustls → ring` entirely, so no C crypto needs
  cross-compiling. *(Trivial; root `Cargo.toml` `egui_extras` line.)*
- [ ] **Restructure the entry point: example → `cdylib`.** Cargo *examples*
  build as executables, but Android needs a `cdylib` (`.so`) with a
  `#[no_mangle] fn android_main(app: AndroidApp)` that feeds
  `eframe::NativeOptions.android_app` — not `fn main()` + `run_native`. So
  `fhr_sim_v2` cannot stay an `[[example]]`; it needs a small dedicated crate
  (or lib target) with `crate-type = ["cdylib"]` that reuses the existing sim
  logic unchanged. *(Moderate.)*
- [ ] **Install toolchain + packager.** Android NDK + SDK, `rustup target add
  aarch64-linux-android` (already added), and `cargo-apk` or `xbuild` to link
  and produce the APK. Set `CC_aarch64_linux_android` at the NDK clang (only
  needed if any C `-sys` crate survives step 1). *(Needs `pacman`/sudo on the
  dev machine — cannot be done headless.)*
- [ ] **Touch/responsive UI (UX, not compile).** `main.rs` hardcodes a
  1920×1080 viewport and mouse-oriented layout; it will *run* on a phone but be
  unusable until the layout is made responsive. Separate follow-up.

Consider `wgpu` vs `glow`: `wgpu` (current) targets Vulkan on Android and
compiles fine; `glow` (GLES) is sometimes more robust on older devices. Revisit
only if runtime surface/resume issues appear.
