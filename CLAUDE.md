# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in this repository.

## Workflow rules (mandatory)

- **Never auto-commit or auto-push.** Do not run `git commit` or `git push` unless the user explicitly asks.
- **Never auto-bump versions** in `Cargo.toml` files. Only bump versions when explicitly requested.
- **Always build and test in release mode.** Use `--release` for all `cargo build` and `cargo test` invocations. Never run tests or builds in debug mode.


## Human interface layer (mandatory design principle)

**Every public API in this workspace must be navigable by a Rust developer using
rust-analyzer alone — no AI assistant, no prior knowledge of the codebase.**

This is a hard rule, not a goal. The human mind cannot hold large amounts of context
simultaneously. If understanding a function requires recalling three other modules at
once, the interface is wrong regardless of how correct the physics is.

### What this requires in practice

**Every public function, type, trait, and module must have a `///` or `//!` doc comment that answers:**
- What physical quantity does this compute or represent?
- What are the valid input ranges and assumptions?
- What units do parameters represent — even when `uom` enforces them, spell it out for human readers.

**Complex `uom` types must have named type aliases.** A user hovering in their editor
should see `SpecificEnthalpy`, not a raw `Quantity<ISQ<...>, SI<f64>, f64>`.

**Each module's `lib.rs` / `mod.rs` must have a `//!` module-level comment** that
explains what belongs in the module and what does not. This is the map a new user
reads first.

**Examples are the primary entry point, not the API docs.** A user must be able to
find an example, read it top-to-bottom without jumping to other files, and understand
what crate they need and how to call it.

### What AI assistants must not do

- Do not add complexity (extra type parameters, trait indirection, macro magic) in
  the name of correctness or generality if it raises the mental context load for a
  human reader.
- Do not leave public items undocumented. If you add or modify a public item, add or
  update its `///` doc comment in the same change.
- Do not write examples that require reading internal modules to understand.

## Rust design rules (mandatory)

### No trait objects — use enums for dispatch

Do not use `Box<dyn Trait>`, `&dyn Trait`, or `Arc<dyn Trait>` for dispatch.
Use enums instead. The set of physics models (EOS, turbulence models, numerical
schemes, boundary conditions) is closed and known at compile time — enums are
the right tool.

Benefits over trait objects:
- **Exhaustiveness** — adding a new variant forces every `match` site to handle it; a missing case is a compile error, not a runtime surprise
- **Zero heap allocation** — the enum lives inline in its containing struct
- **rust-analyzer navigability** — Go-to-definition works on enum variants; it often fails on `dyn Trait` implementations

Traits are still useful as a **compiler-enforced contract** on each concrete
struct — the compiler verifies every model implements the right methods. They
are just not used for runtime dispatch. The pattern:

```rust
// Trait enforces the interface — compiler checks every model satisfies it
pub trait TurbulenceKernel {
    fn div_dev_rho_reff(&self, u: &VolVectorField) -> FvVectorMatrix;
    fn correct(&mut self);
}

// Enum dispatches without Box or dyn
pub enum TurbulenceModel {
    Laminar(LaminarModel),
    KOmegaSST(KOmegaSSTModel),
    KEpsilon(KEpsilonModel),
}

impl TurbulenceModel {
    pub fn correct(&mut self) {
        match self {
            Self::Laminar(m)   => m.correct(),
            Self::KOmegaSST(m) => m.correct(),
            Self::KEpsilon(m)  => m.correct(),
        }
    }
}
```

### No `Box<T>`

Do not use `Box<T>`. Own data by value or share it with `Arc<T>`.
`Box<T>` is only justified for recursive data structures (trees, linked lists),
which do not appear in this codebase.

### No lifetime parameters

Do not add lifetime parameters (`'a`) to structs, trait definitions, or impl
blocks. Own data by value, or share it with `Arc<T>`.

| Instead of | Use |
|---|---|
| `&'a FvMesh` in a struct | `Arc<FvMesh>` |
| `&'a f64` / uom quantity in a struct | own by value — all uom types are `Copy` |
| `Box<dyn Fn(&'a T) -> U>` | newtype struct that owns its captured state |
| `&'a Cell` for graph/topology links | `CellId(usize)` — index into a `Vec` |

### Shared state: `Arc<RwLock<T>>` over channels

For shared mutable simulation state (fields, solver coefficients), use
`Arc<RwLock<T>>`. For data that is read-only after construction (mesh topology,
lookup tables, material constants), use `Arc<T>` with no lock.

Prefer `RwLock<T>` over `Mutex<T>` — `RwLock` allows concurrent reads from
multiple threads; `Mutex` serialises even read-only access, which defeats
parallelism during the compute phase of a timestep.

Do not use channels (`mpsc`, `crossbeam`) for simulation state. Channels suit
pipeline patterns where data is produced, consumed, and discarded. The simulation
timestep loop is a shared-state pattern — threads compute over non-overlapping
regions of the same fields, then synchronise.

## What this is

**OUTRAM PARK backend** — the Cargo **workspace** that houses the OUTRAM PARK
(Open-source TRAnsient Multi-Phase Advanced Reactor simulator Kit) Rust suite.
Several crates that used to live as independent GitHub repositories under
`github.com/theodoreOnzGit` are now consolidated here under `crates/` and are
built, tested, and published from this single repository.

## Members

| Crate (`crates/…`) | Role | License |
|---|---|---|
| `chem-eng-real-time-process-control-simulator` | PID / transfer-function process-control library (real-time simulators) | **Apache-2.0** |
| `teh-o-prke` | Point Reactor Kinetics (PRKE) for the Teh-O transport/eigenvalue solver | GPL-3.0 |
| `tuas_boussinesq_solver` | Thermal-hydraulics (Boussinesq single-phase) solver — TUAS | GPL-3.0 |
| `tampines-steam-tables` | IAPWS-IF97 steam/water properties + steam-turbine equations — TAMPINES | GPL-3.0 |
| `openfoam-basic-lib` | Pure-Rust translation of the OpenFOAM primitive + finite-volume layer (Layers 1–4): tensor algebra, polynomial solvers, ODE solvers, interpolation, thermophysics kernels, fields, mesh, FV operators, fluid/solid thermo | GPL-3.0 |

**Planned future crates** (not yet in the workspace):

| Crate | Depends on | Solvers it targets |
|---|---|---|
| `openfoam-icof` | `openfoam-basic-lib` | **icoFoam** (incompressible laminar PISO) |
| `openfoam-cht` | `openfoam-basic-lib` | **chtMultiRegionFoam** (conjugate heat transfer, multi-region) |
| `openfoam-rho` | `openfoam-basic-lib` | **rhoPimpleFoam** / **sonicFoam** (compressible) |

**Layer 5 (solver loop logic) MUST live in these separate crates**, not in
`openfoam-basic-lib`.  `openfoam-basic-lib` provides the mathematical building
blocks (Layers 1–4) only; the PISO/PIMPLE loop, multi-region coupling logic,
and turbulence model registries belong in solver-specific crates so that
`openfoam-basic-lib` stays publishable independently and is reusable by other
projects.

Internal dependency edges (all by **path**, not crates.io):
`tampines → tuas`; `teh-o-prke → {tuas, chem-eng}` (dev); `tuas`/`tampines` dev-deps → `chem-eng`, `teh-o-prke`.
`openfoam-basic-lib` has no internal deps (pure third-party: `uom`, `ndarray`, `thiserror`).

## Dependency policy — single source of truth

All third-party versions live in the root `[workspace.dependencies]`. Members
inherit them with `<dep>.workspace = true`, so versions **cannot drift** between
crates. When changing a shared dependency, edit the root `Cargo.toml` only.

`ndarray-linalg` is the one exception that needs per-crate attention: the BLAS
backend feature is chosen per-target by each member
(`openblas-system` on unix, `intel-mkl-static` on windows/macos).

**Future: removing `ndarray-linalg` from TUAS.** `openfoam-basic-lib` does
**not** use `ndarray-linalg` — its `SquareMatrix` module implements LU
factorisation in pure Rust. All `ndarray-linalg` usage in TUAS bottoms out in
one call: `M.solve(&S)` in
`array_control_vol_and_fluid_component_collections/standalone_fluid_nodes/mod.rs`
(`solve_conductance_matrix_power_vector`), which performs a dense LU solve on
the per-timestep conductance matrix (typically 10–50 × 10–50, not tridiagonal
because lateral coupling fills off-band entries). Replacing it with
`openfoam_basic_lib::matrix::SquareMatrix::lu_solve` would eliminate the
OpenBLAS system dependency from TUAS entirely. That requires changing
`solve_conductance_matrix_power_vector`'s signature and its ~13 call sites —
a moderate refactor, not urgent.

## Build & test

Requires a system BLAS (OpenBLAS on Linux/macOS):

```bash
# Arch / EndeavourOS
sudo pacman -S openblas
# Debian / Ubuntu / Mint
sudo apt install libopenblas-dev
```

```bash
cargo build --workspace --release                  # all libraries
cargo check --workspace --lib --tests              # type-check (mode-independent)
cargo test  --workspace --lib --tests --release    # run the test suites
```

Note: a bare `cargo test --workspace` also compiles the **examples**, some of
which are still mid-migration to egui 0.34 (see status below). Use
`--lib --tests` until the examples are done.

## Migration status (OUTRAM PARK consolidation, 2026-06)

Everything below was done while moving these crates into the workspace and
bumping to the latest stable dependencies.

**Toolchain:** requires Rust ≥ 1.92 (egui 0.34). Developed on rustc 1.96.

**Version bumps (all crates, via `[workspace.dependencies]`):**

| Dep | Was | Now | Notes |
|---|---|---|---|
| `uom` | 0.36 | 0.38 | unifying this to a single version removed ~150 type-mismatch errors |
| `ndarray` | 0.15 | 0.17 | |
| `ndarray-linalg` | 0.16 | 0.18 | |
| `peroxide` | 0.37 | 0.41 | TUAS only |
| `thiserror` | 1 | 2 | |
| `csv` | 1.3 | 1.4 | |
| `env_logger` | 0.10 | 0.11 | |
| `egui` / `eframe` | 0.29 | 0.34 | breaking GUI API changes (see below) |
| `egui_plot` | 0.29 | **0.35** | egui_plot's numbering is decoupled: **0.35 pairs with egui 0.34**, whereas egui_plot **0.34.x pairs with egui 0.33**. Mispinning this produces "multiple versions of crate `egui`" errors. |
| `approx` | 0.5.1 | 0.5.1 | left as-is; 0.6 is only a pre-release |

**Structural:**
- Each crate moved to `crates/<name>`; standalone `.git`, `target/`, and
  `Cargo.lock` were dropped (histories intentionally not preserved).
- `chem-eng…` keeps its own **Apache-2.0** license (not the GPL-3.0 default),
  and its crate-level `[profile.*]` sections were lifted to the workspace root
  (Cargo only honors profiles at the root). Its `release` opt-level=2 override
  was dropped so the solvers get default `-O3`; `dev.package."*"` opt-level=2 is
  kept at the root so unoptimized deps don't make tests painfully slow.

**egui 0.29 → 0.34 example migration:**
- `eframe::App` now requires `fn ui(&mut self, ui: &mut egui::Ui, frame)`; the
  old `fn update(&mut self, ctx, frame)` is deprecated. Migration pattern: rename
  to `ui`, then `let ctx = ui.ctx();` at the top so existing panel code keeps
  working (panel `.show(ctx, …)` is deprecated in favor of `.show_inside(ui, …)`
  but still compiles).
- `egui_plot::Line::new` now takes `(name, series)` instead of `(series)` +
  `.name()`. Migration pattern: `Line::new("label", PlotPoints::from(vec))`.

**Per-example progress — all migrated; `cargo build --workspace --all-targets` is green:**
- ✅ `teh-o-prke` / `fhr_sim_v1`
- ✅ `tampines-steam-tables` / `fhr_sim_v1`, `fhr_sim_v2` (`depressurisation` and
  `transient_rankine_cycle` needed no changes — they don't use the changed
  egui/egui_plot APIs)
- ✅ `tuas_boussinesq_solver` / `ciet_educational_simulator` — 20 `Line::new`
  call sites updated (most via a scripted reorder; two HTC plots had no `.name()`
  and were given `"CTAH HTC"` / `"TCHX HTC"` labels)

All four GUI examples were additionally migrated **off the deprecated egui APIs**
(zero deprecation warnings in `cargo build --workspace --all-targets`):
`TopBottomPanel`/`SidePanel` → `Panel::top/bottom/left/right`; panel `.show(ctx,…)`
→ `.show_inside(ui,…)` with the `CentralPanel` moved **last** (and any trailing
`ctx.request_repaint_after` switched to `ui.ctx()`); `egui::menu::bar` →
`egui::MenuBar::new().ui`; `ScrollArea::drag_to_scroll(true)` →
`scroll_source(egui::scroll_area::ScrollSource::ALL)`; `Image::rounding` →
`corner_radius`.

Each member crate has its own `CLAUDE.md` with crate-specific architecture and
migration notes.

## Publishing to crates.io

Current versions (bumped for the dependency migration — breaking, since `uom`
etc. appear in public APIs):

| Crate | Version | License |
|---|---|---|
| `chem-eng-real-time-process-control-simulator` | 0.1.0 | Apache-2.0 |
| `tuas_boussinesq_solver` | 0.1.0 | GPL-3.0-only |
| `teh-o-prke` | 0.1.0 | GPL-3.0-only |
| `tampines-steam-tables` | 0.2.0 | GPL-3.0-only |
| `openfoam-basic-lib` | 0.1.2 | GPL-3.0-only |

Internal deps are `{ path = …, version = … }` in `[workspace.dependencies]`, so
the version pins above must be kept in sync with each crate's `version` (and a
downstream crate's pin bumped whenever an upstream crate is bumped).

**Publish order is mandatory** — `cargo publish` resolves *all* dependencies,
including dev-dependencies, against crates.io, so each crate can only be packaged
once everything it depends on (normal **or** dev) is already live:

1. `chem-eng-real-time-process-control-simulator` (no internal deps)
1. `openfoam-basic-lib` (no internal deps — can publish in parallel with chem-eng)
2. `tuas_boussinesq_solver` (dev-dep: chem-eng)
3. `teh-o-prke` (dev-deps: tuas, chem-eng)
4. `tampines-steam-tables` (dep: tuas; dev-deps: teh-o-prke, chem-eng)

Because of this, `cargo publish --dry-run` / `cargo package` for crates 2–4 will
fail with "failed to select a version" until their upstreams are published —
that's expected, not a packaging error (`cargo package --list` still shows clean
contents). chem-eng's dry-run passes standalone.

Publish each with `cargo publish -p <crate>` from the workspace root (commit
first; `cargo publish` refuses a dirty tree without `--allow-dirty`).

**Package hygiene already applied** via `exclude` in each manifest:
- `tuas_boussinesq_solver`: `exclude = ["*.csv"]` — tests dump ~58 MB of CSVs into
  the crate root (far over the crates.io size limit).
- `teh-o-prke`: `exclude = ["pki", "docs"]` — `pki/` contained a **private key**
  (`private.pem`) that must never be published; `docs/` is a large theory PDF.
- `tampines-steam-tables`: `exclude = ["docs"]` — LaTeX sources/build artifacts.

⚠ The private key under `teh-o-prke/pki/` is excluded from the package, but it
still exists in the working tree — consider removing it and rotating the key.

## Wayland / display notes

The GUI examples (`fhr_sim_v2`, `ciet_educational_simulator`) use the **wgpu**
renderer (Vulkan-backed, native Wayland). The old `glow` (OpenGL/glutin) backend
raised `glutin error: provided native window is not supported` on Wayland and was
replaced in the workspace `Cargo.toml` (2026-06-21).

If you ever need to run without Vulkan (e.g. in a VM), force XWayland instead:

```bash
WINIT_UNIX_BACKEND=x11 cargo run --release --example fhr_sim_v2 -p tampines-steam-tables
```

## Model selection guide (for AI assistants)

When working on debugging tasks in this workspace, choose the Claude model based on task complexity:

### Haiku 4.5 — fastest, cheapest ($1/$5 per 1M tokens)
- **Good for**: Quick lookups, simple one-file questions, iterative file reading
- **Avoid for**: Multi-file reasoning, concurrency bugs, subtle state-flow issues
- **Verdict for fhr_sim_v2 debugging**: Too weak — the UI state-sync bug spans thread boundaries and mutex discipline across multiple files; Haiku will likely miss it or suggest plausible-sounding wrong fixes

### Sonnet 4.6 — balanced ($3/$15 per 1M tokens)
- **Good for**: Multi-file code tracing, Rust ownership/mutex reasoning, interactive debugging sessions
- **Avoid for**: Very deep invariants that require holding the entire codebase in mind simultaneously
- **Verdict for fhr_sim_v2 debugging**: Good default — the bug is a logic/data-binding issue, not a novel algorithm, so Sonnet's reasoning depth is sufficient for most passes

### Opus 4.8 — most capable ($5/$25 per 1M tokens)
- **Good for**: Holding a large mental model across many interacting files simultaneously (e.g., all three simulation threads + the egui render loop at once), catching subtle concurrency bugs like "mutex held during repaint starves the UI thread"
- **Avoid for**: Routine iterative work — 5× cost and noticeably slower responses make it sluggish for back-and-forth file reading
- **Verdict for fhr_sim_v2 debugging**: Use if Sonnet gets stuck after reading `app/mod.rs`, `app/graph_data/update.rs`, and `simulator_trait.rs` and the root cause is still unclear
