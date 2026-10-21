# Rationale: portability (Android/Termux and wasm32) — mechanisms and evidence

> Split out of the root `CLAUDE.md` on 2026-09-21. **Full original text,
> verbatim.** Both rules remain binding and are still stated in `CLAUDE.md`;
> what is kept here is the gate-vs-feature reasoning table, the per-crate
> patterns, the exclusion list rationale and the measured artifacts.

## Android / Termux portability (HARD RULE for non-GUI code)

**Hard rule (not a default): every crate's non-GUI library code MUST compile on
Termux — native, on-device Android — with Android-hostile pieces held off behind
Android feature gates.** "Compiles on Termux" is the acceptance bar: a build run
*inside Termux* (native `aarch64-linux-android`, no NDK cross-toolchain, no system
BLAS/LAPACK, no C/Fortran toolchain) must succeed for every non-GUI library. This
does not bend for convenience — if a change cannot build on Termux, it is not done
until the offending dependency/test/example is gated off Android in the *same*
change. Workspace-wide tracking lives in the **`op-zfr` "Android support" epic**.

Termux specifics to keep in mind:

- **Termux builds natively on the device**, so the target is `aarch64-linux-android`
  and **`target_os = "android"`** (not `"linux"`). Every gate below keys off that.
- Prefer an explicit **Cargo feature** (e.g. `android`, or an inverted
  `native-blas`/`gui` feature that is simply *not* enabled on Termux) plus the
  `cfg(target_os = "android")` target gate, so a Termux user gets a working build
  from the default feature set with no manual flag-twiddling.
- No system package manager for BLAS/LAPACK/GUI libs is assumed to exist on Termux.

**Every crate's non-GUI library code must also compile for Android**
(`aarch64-linux-android` and the armv7/x86_64 emulator targets) when cross-built
from a host. Android has no system BLAS/LAPACK and no easy C/Fortran toolchain, so
**Android-hostile dependencies must not compile on Android** — gate them off by
target rather than letting them break the build.

- **`ndarray-linalg`** (and anything needing system BLAS/LAPACK, or a C/Fortran
  toolchain, or `std`-GUI/windowing) is Android-hostile. Declare it only under
  target-conditional tables — e.g.
  `[target.'cfg(not(target_os = "android"))'.dev-dependencies]` — never as an
  unconditional dependency. (Android's `target_os` is **`"android"`, not
  `"linux"`**, so an existing `cfg(target_os = "linux")` gate already excludes
  it — but do not *rely* on a linux-only gate to mean "not Android" without
  saying so.)
- **Examples/tests/benches count — they are NOT exempt.** A native Termux
  `cargo build` / `cargo test` compiles **examples, integration tests, and
  benches**, so an Android-hostile dep or a desktop-only-API reference in *any*
  of those breaks the on-device build even when the library itself is clean.
  Gate every one that touches an Android-hostile path:
  - **Tests / benches** (no `main` required): put `#![cfg(not(target_os =
    "android"))]` at the top of the file — blanking the whole file on Android is
    fine. Precedent: `outram-foam-basic-lib`'s `tests/matrix_bench.rs`.
  - **Examples / bins** (a `main` *is* required — a blanked file gives "main
    function not found"): add an **Android stub `main`** under `#[cfg(target_os
    = "android")]` that prints a "desktop-only" line, and gate every desktop
    item (`use`/`const`/`fn`/`struct`/…) with `#[cfg(not(target_os =
    "android"))]`. Precedent: `njoy-outram-park-fork`'s
    `examples/gpu_wmp_bench.rs` and `outram-mc-libs`'s
    `examples/godiva_gpu_benchmark.rs`.
- **Only windowing GUI is out of scope — terminal apps are IN scope.** Termux
  *is* a terminal, so a **CLI or a `ratatui` TUI must compile and run on
  Android** like any other non-GUI crate — do not exempt it. What is out of
  scope is **`egui`/`eframe`/`wgpu`-surface/windowing** GUI: keep that behind
  examples/optional bins/target gates, never in a library's unconditional
  build, so the lib still builds headless for Android. Concretely: the `kovan`
  crate's `kovan-cli` (CLI) and `kovan-tui` (`ratatui` TUI — genuinely
  Android/Termux-*usable*, not merely buildable; no Android stub as of
  2026-08-21, see `crates/kovan/README.md` "Android") binaries are **in
  scope and verified building and running** for `aarch64-linux-android` (its
  `kovan` binary — the GUI, renamed from `kovan-gui` the same day — is the
  crate's own GUI exemption, gated behind a `gui` feature that defaults on
  everywhere except Android, so it never affects the other two's Android
  build); only `outram-park-digital-twin-engine` (egui/eframe) is a genuine
  GUI exemption.
- **New code follows this by default.** If you add a dep or a test that can't
  build on Android, target-gate it in the same change and note it.
- **The check MUST cover all targets, not just `--lib`.** A `cargo check
  --lib --target aarch64-linux-android` checks *only the library* and silently
  misses broken examples/tests/benches — the exact gap that let the
  `godiva_gpu_benchmark` example ship un-gated (found only by an on-device
  Termux build). The proxy check is therefore **`cargo check -p <crate>
  --all-targets --target aarch64-linux-android`** (needs the Android target +
  NDK / `cargo-ndk`). The **authoritative** check is still a **native Termux
  build** (`cargo build` / `cargo test` run inside Termux on-device), which
  compiles all targets by construction. Never report Android/Termux support as
  verified from a `--lib`-only run. Workspace-wide Android/Termux build tracking
  lives in beads (the **`op-zfr` "Android support" epic**).

## WebAssembly (`wasm32-unknown-unknown`) — supported target, with a hard caveat

**Every in-scope crate's library must compile for `wasm32-unknown-unknown`, and
a gate enforces it.** Added 2026-09-04 (epic `op-okqo`); 34 of 40 members are in
scope, 6 are deliberately excluded.

```bash
scripts/check-wasm.sh          # the gate; -v shows first error lines
```

Install the target once with `rustup target add wasm32-unknown-unknown`.

### COMPILING IS NOT RUNNING — read this before claiming wasm support

The gate checks **compilation only**, and the gap between that and working in a
browser is large. `std::thread::spawn`, `std::time::Instant` and `std::fs` all
**compile** for wasm32 and fail only at **run time** — the first two panic, the
third errors. `chem-eng-real-time-process-control-simulator` is the standing
proof: it passes the gate today while containing 5 `thread::spawn` sites and 10
files using `std::fs`.

So "passes `check-wasm.sh`" means *the types line up*, never *this works in a
browser*. Making a crate genuinely run on wasm is per-crate work tracked under
epic `op-eeqw` (GH #39). Do not describe a crate as wasm-ready on the strength
of this gate.

### The gate is `--lib`, and that is a known limitation

The Android rule uses `--all-targets` because a `--lib`-only check silently
misses broken examples and tests. wasm cannot follow it: several crates carry
egui/eframe GUI examples and terminal binaries that legitimately cannot build
for wasm, so `--all-targets` would be permanently red and therefore ignored.
A broken wasm-facing *example* will **not** be caught. Stated here rather than
papered over.

### The mechanism matters: feature-gate vs target-gate vs which target

This trips people, so it is spelled out. Three different tools, three reasons:

| Dependency | Gate | Why |
|---|---|---|
| `rayon` | target-gated **off wasm only** | `rayon-core` needs OS threads and does not build for wasm at all. But rayon is pure Rust and **Android-in-scope**, so feature-gating it or gating it off Android would cost Termux its multi-core path for nothing. |
| `wgpu` | target-gated **off Android** | No system Vulkan/Metal loader there. Note it *also* needs gating off wasm in some crates for a different reason — see below. |
| `ratatui` / `crossterm` | target-gated **off wasm only** | No terminal in a browser. A TUI **is** Android-in-scope per the Android section, so Android keeps it. |
| `async-opcua`, `tokio`, `mdns-sd`, `directories` | target-gated **off wasm only** | Sockets, multicast, XDG paths. All verified Android-clean, so Android keeps them. |

**Do not reach for a Cargo feature when a target gate is correct**, and vice
versa. A feature is for "the user may not want this"; a target gate is for "this
cannot exist here".

### Patterns this workspace uses

- **`src/wasm_par.rs`** — a small in-crate module giving serial stand-ins for
  the handful of `rayon` adapters a crate actually uses (`njoy-outram-park-fork`,
  `outram-mc-libs`, `boon-lay` each have one). It is **not** a parallelism
  implementation and its docs say so. Where a crate's results are
  thread-count-independent — which the Monte Carlo drivers document — the serial
  path is *numerically exact*, not merely a degraded fallback.
- **`.cargo/config.toml`** supplies `--cfg getrandom_backend="wasm_js"` for the
  wasm target. `getrandom` 0.3 needs **both** that flag and its `wasm_js`
  feature; the feature alone is insufficient and getrandom errors if either is
  missing. Prefer **cutting** a transitive randomness dependency over satisfying
  it — gating `async-opcua` off wasm removed `getrandom` 0.2 and `uuid` from
  `outram-park-digital-twin-engine` outright.
- **`wasm32` has a 32-bit `usize`.** When a constant overflows, **widen the
  type; do not truncate the constant.** `outram-park-mpi`'s
  `COLL_CONTEXT_OFFSET = 1 << 40` became `u64` because it is a numeric namespace
  tag, not a memory size — truncating would have silently collapsed the
  isolation it provides.
- **wgpu's WebGPU backend is `!Send` on wasm** (it holds `Rc<Cell<u32>>`), so a
  `static OnceLock<Option<GpuContext>>` cache will not compile there. Reaching
  WebGPU from wasm needs a `thread_local!` instead — a real change, not a gate.

### Exclusions are deliberate and listed in one place

`scripts/check-wasm.sh` carries the exclusion list with a reason per crate.
Currently excluded: `kovan`, `kovan-discovery`, `kovan-metrics`,
`kovan-semantics` (a filesystem-walking developer CLI/TUI — "compiles for wasm"
would be meaningless), and `bedok`, `outram-blender` (not wanted as wasm
targets). Note `kovan-codegen`, `kovan-common` and `kovan-literature` are
**not** excluded: they already pass, so gating them is free.

**This does not relax the Android rule.** wasm is an additional target, not a
replacement, and nothing may break `aarch64-linux-android`.

