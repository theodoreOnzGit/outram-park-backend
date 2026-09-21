<!-- Moved verbatim from the workspace CLAUDE.md on 2026-09-21 (maintainer direction: physics
     rules stay in the root, everything else here behind a pointer). STILL BINDING. -->

## Android / Termux portability (HARD RULE for non-GUI code)

**Hard rule (not a default): every crate's non-GUI library code MUST compile on
Termux — native, on-device Android — with Android-hostile pieces held off behind
Android feature gates.** "Compiles on Termux" is the acceptance bar: a build run
*inside Termux* (native `aarch64-linux-android`, no NDK cross-toolchain, no system
BLAS/LAPACK, no C/Fortran toolchain) must succeed for every non-GUI library. This
does not bend for convenience — if a change cannot build on Termux, it is not done
until the offending dependency/test/example is gated off Android in the *same*
change. Tracking: the **`op-zfr` "Android support" epic**.

- **Termux builds natively on the device**, so the target is
  `aarch64-linux-android` and **`target_os = "android"`** (not `"linux"`).
  Every gate keys off that. No system package manager for BLAS/LAPACK/GUI libs
  is assumed to exist.
- Prefer an explicit **Cargo feature** (e.g. an inverted `native-blas`/`gui`
  feature that is simply *not* enabled on Termux) **plus** the
  `cfg(target_os = "android")` target gate, so a Termux user gets a working
  build from the default feature set with no manual flag-twiddling.
- **`ndarray-linalg`** — and anything needing system BLAS/LAPACK, a C/Fortran
  toolchain, or `std`-GUI/windowing — is Android-hostile. Declare it only
  under target-conditional tables, never unconditionally.
- **Examples/tests/benches count — they are NOT exempt.** A native Termux
  build compiles them, so an Android-hostile dep in *any* of them breaks the
  on-device build even when the library is clean. Tests/benches take
  `#![cfg(not(target_os = "android"))]` at the top of the file; examples/bins
  need an **Android stub `main`** (a blanked file gives "main function not
  found") with every desktop item gated. Precedents:
  `outram-foam-basic-lib`'s `tests/matrix_bench.rs`,
  `njoy-outram-park-fork`'s `examples/gpu_wmp_bench.rs`.
- **Only windowing GUI is out of scope — terminal apps are IN scope.** Termux
  *is* a terminal, so a CLI or a `ratatui` TUI must compile and run on Android
  like any other non-GUI crate. `egui`/`eframe`/`wgpu`-surface windowing stays
  behind examples/optional bins/target gates. `kovan-cli` and `kovan-tui` are
  in scope and verified; only `outram-park-digital-twin-engine` is a genuine
  GUI exemption.
- **New code follows this by default.** If you add a dep or a test that can't
  build on Android, target-gate it in the same change and note it.
- **The check MUST cover all targets, not just `--lib`.** A `--lib`-only check
  silently misses broken examples/tests/benches — the exact gap that let the
  `godiva_gpu_benchmark` example ship un-gated. The proxy check is
  **`cargo check --release -p <crate> --all-targets --target
  aarch64-linux-android`** (needs the Android target + NDK / `cargo-ndk`). The
  **authoritative** check is a **native Termux build** on-device. Never report
  Android/Termux support as verified from a `--lib`-only run.
## WebAssembly (`wasm32-unknown-unknown`) — supported target, with a hard caveat

**Every in-scope crate's library must compile for `wasm32-unknown-unknown`, and
a gate enforces it.** Added 2026-09-04. **37 of the 43 members are in scope; 6 are deliberately
excluded** — `kovan`, `kovan-discovery`, `kovan-metrics`, `kovan-semantics`,
`bedok` and `outram-blender`, each with its reason in the script.
**CORRECTED 2026-09-21** — this file had said "34 of 40", stale on both
numbers; verified against `scripts/check-wasm.sh` and the workspace member
list. Run `scripts/check-wasm.sh` (the gate;
`-v` shows first error lines); install the target once with
`rustup target add wasm32-unknown-unknown`.

**COMPILING IS NOT RUNNING.** The gate checks **compilation only**, and the
gap between that and working in a browser is large: `std::thread::spawn`,
`std::time::Instant` and `std::fs` all **compile** for wasm32 and fail only at
**run time**. `chem-eng-real-time-process-control-simulator` is the standing
proof — it passes the gate today while containing 5 `thread::spawn` sites and
10 files using `std::fs`. So "passes `check-wasm.sh`" means *the types line
up*, never *this works in a browser*. Making a crate genuinely run on wasm is
per-crate work tracked under epic `op-eeqw` (GH #39). **Do not describe a
crate as wasm-ready on the strength of this gate.**

**The gate is `--lib`, and that is a known limitation.** The Android rule uses
`--all-targets`; wasm cannot, because several crates carry GUI examples and
terminal binaries that legitimately cannot build for wasm, and a permanently
red gate is an ignored gate. A broken wasm-facing *example* will **not** be
caught. Stated here rather than papered over.

**Choose the right mechanism: a feature is for "the user may not want this", a
target gate is for "this cannot exist here".** And gate off the *right*
target — `rayon`, `ratatui`/`crossterm`, `async-opcua`/`tokio`/`mdns-sd`/
`directories` are all gated **off wasm only** and stay available on Android;
`wgpu` is gated **off Android** (no system Vulkan/Metal loader) and separately
off wasm in some crates for a different reason.

**Exclusions are deliberate and listed in one place** —
`scripts/check-wasm.sh` carries the list with a reason per crate. Note
`kovan-codegen`, `kovan-common` and `kovan-literature` are **not** excluded:
they already pass, so gating them is free.

**This does not relax the Android rule.** wasm is an additional target, not a
replacement, and nothing may break `aarch64-linux-android`.

> The gate-vs-feature reasoning table, the `wasm_par.rs` / `getrandom` /
> 32-bit-`usize` / `!Send`-wgpu patterns and the full exclusion list:
> [`docs/claude-md-rationale/portability-android-wasm.md`](../claude-md-rationale/portability-android-wasm.md).
## File path length: 170-character hard cap (HARD RULE, added 2026-08-18)

**No new file in this workspace may have a repo-relative path longer than 170
characters.** Windows' classic `MAX_PATH` is 260 characters and includes the
drive letter and every parent directory down to the repo root, not just the
part shown by `git ls-files` — a clone under a realistic path like
`C:\Users\<name>\Documents\GitHub\outram-park-backend\` already spends
45-70+ characters before the repo-relative part even starts. Long-path
support (`git config --global core.longpaths true` plus Windows'
`LongPathsEnabled`) fixes this for a user who knows to enable it, but this
workspace targets contributors who may not, so the rule is to not need it.

- **Check before adding a deeply nested file or directory**:
  `git ls-files | awk '{print length, $0}' | sort -rn | head` from the repo
  root, or scope it to one path with `git ls-files -- '<prefix>/**' | awk
  '{print length}' | sort -rn | head`.
- **170, not 260**, deliberately leaves headroom for a real clone-path prefix
  on top of the repo-relative path, and for the file to still have room to
  grow (a rename, an added test suffix) without immediately re-crossing the
  line.
- **This governs new files going forward.** It does not retroactively demand
  renaming everything already over the limit — see the history below.
- **Existing violations are grandfathered, not silently ignored** when found.
  A backlog of 53 files across `tampines-steam-tables` and
  `tuas_boussinesq_solver` was cleared on 2026-08-19 — see "Path-length
  refactor precedent" below. Re-run the scan (`git ls-files | awk '{print
  length, $0}' | sort -rn | awk '$1>=170'`) rather than trusting a count
  written here, since new violations can accumulate again over time.

### Path-length refactor precedent — this is a well-trodden, low-risk operation

**An agent session did this exact refactor workspace-wide on 2026-08-19** —
shortening directory and file names to fix path-length violations while
keeping the whole workspace compiling (including test binaries) throughout is
a routine that has already been proven out here, not a novel or risky
undertaking. It is safe to ask for again.

Two things that pass carried that are worth repeating:

- **Check every renamed identifier for public-API exposure first** — a
  workspace-wide grep for real `use`/`pub use` references, not just a
  directory listing. Four `tuas_boussinesq_solver` identifiers turned out to
  be genuinely public cross-crate API.
- **The compiler is the reference-checker.** kopitiam's `rename` could not be
  used (rust-analyzer rejected every request — filed as
  [kopitiam#30](https://github.com/theodoreOnzGit/kopitiam/issues/30)), so the
  fallback this file's "Workflow rules" already prescribes — enumerate
  references by text search, rename by hand, let `cargo check` catch the
  misses — carried the whole refactor without incident.

The compatibility re-exports used on the first push were a **same-session
bridge, not a standing policy**; they were deleted the same day once every
downstream reference had been migrated. If a future pass repeats this on a
crate with real crates.io consumers outside this workspace, keeping the alias
for at least one published version is worth raising with the maintainer
explicitly, since those consumers cannot be grepped for and fixed in-session.

> Full account — the exact renames, the four public identifiers, the
> verification commands run at each stage:
> [`docs/claude-md-rationale/path-length-refactor-precedent.md`](../claude-md-rationale/path-length-refactor-precedent.md).

