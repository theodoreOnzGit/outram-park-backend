# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in this repository.

## Workflow rules (mandatory)

- **Never auto-commit or auto-push.** Do not run `git commit` or `git push` unless the user explicitly asks.
- **Never auto-bump versions** in `Cargo.toml` files. Only bump versions when explicitly requested.
- **Always build and test in release mode.** Use `--release` for all `cargo build` and `cargo test` invocations. Never run tests or builds in debug mode.
- **Use rust-analyzer (the LSP tool) for all code-intelligence workflows.**
  Maximise its use whenever possible. For any symbol query — a definition,
  every reference/caller, type/hover info, or listing symbols in a file or
  across the workspace — reach for the rust-analyzer LSP tool first, **not** text
  search (`grep`). It resolves symbols semantically, so it does not confuse a
  module path with a like-named identifier the way a text match can.
  - **The LSP tool here is read-only** — `goToDefinition`, `findReferences`,
    `hover`, `documentSymbol`, `workspaceSymbol`, and call hierarchy. It does
    **not** expose rename / code-action / `applyEdit`. (Full rust-analyzer in an
    editor like Neovim/VS Code does; this harness surfaces only the query half.)
  - For a refactor an editor would drive with *rename* (e.g. renaming a module
    and rewriting every `crate::…` path to it), first use `findReferences` to
    enumerate the sites, then apply the edits yourself, and rely on the compiler
    (`cargo build`/`cargo check`) as the reference checker — every missed
    reference is a hard error pointing at the exact line. Prefer this over a
    blind `sed` rename, which can silently mangle a colliding name.

## Issue tracking & roadmap — beads (mandatory when available)

This workspace tracks issues and per-crate roadmap progress with **beads**
(`bd`). It is a dependency-aware issue tracker whose data lives in `.beads/`
(embedded Dolt DB) with a passive export at **`.beads/issues.jsonl`**.

- **Install** (Linux/macOS with bash):
  `curl -fsSL https://raw.githubusercontent.com/gastownhall/beads/main/scripts/install.sh | bash`
  (installs `bd` to `~/.local/bin`; add that to `PATH`).
- **Standing rule: if `bd` is available on this machine, you MUST use it** for
  all task/roadmap tracking and progress bookkeeping — in preference to
  TodoWrite / TaskCreate / ad-hoc markdown TODO lists. Create/close/update
  beads as work happens; file a bead for any follow-up you discover.
- **If `bd` is *not* available** — e.g. an OS or environment without bash or
  without a beads build (Android, a locked-down sandbox, etc.) — that is fine:
  beads is optional there. Do **not** block work on it; fall back to the
  harness task tools and note in your hand-off that beads wasn't updated.
- **Roadmap / progress summaries come from beads.** When the user asks "where
  are we" / "summarise progress" / "what's the roadmap", read it out of beads
  (`bd list`, `bd ready`, `bd show <id>`, `bd dep tree <id>`, or the
  `.beads/issues.jsonl` export) rather than re-deriving from scattered docs.
  One epic per member crate; child beads are that crate's workstreams.
- **Relationship to the memory system.** Beads and the per-project memory
  files (`~/.claude/projects/<slug>/memory/`) are complementary and **both
  stay in use**: beads tracks *tasks / roadmap / open work*; the memory files
  track *durable facts, user preferences, and feedback*. The auto-generated
  "Beads Issue Tracker" block lower in this file says to drop `MEMORY.md` —
  that does **not** apply here; the memory workflow is unchanged. When in
  doubt: a thing to *do or finish* → bead; a thing to *remember about how the
  user works or a settled fact* → memory.

## README / Markdown format (mandatory)

**Every `README.md` in this workspace must render correctly on GitHub
(GitHub-Flavored Markdown).** GitHub renders LaTeX math via MathJax (`$...$`
inline, `$$...$$` display), so math *is* allowed — but keep it to a conservative
subset that also survives editor previewers. **No exotic math.** Concretely:

- **No matrix/array environments** (`\begin{bmatrix}`, `pmatrix`, `array`) and
  **no `\begin{cases}`** — write a matrix system or a piecewise definition as
  separate `$$...$$` equations, one per line, labelled in prose or with a
  trailing `\quad (\text{...})`.
- **No** `\boxed`, `\underbrace`, `\displaystyle`, `\tfrac`/`\dfrac` (use
  `\frac`), or negative-space `\!`.
- **No Unicode Greek or operators inside math** — use `\gamma`, `\rho`, `\xi`,
  `-`, `\le`, `\pm`, etc. (Unicode is fine in ordinary prose and in inline
  code spans.)
- Write superscripts/subscripts with explicit braces (`(\hat{u}^*)^2`, not
  `\hat u^{*2}`).

**Check every README before finishing.** Prefer `pandoc` when available — it
validates both markdown structure *and* the LaTeX math (via its texmath engine):

```bash
pandoc -f gfm+tex_math_dollars -t html --mathml README.md > /dev/null
```

Exit 0 with **no warnings** means all math converted (any malformed equation
prints a `[WARNING] Could not convert TeX math …`). Note: without `--mathml`,
pandoc emits harmless "rendering as TeX" warnings for every equation — those are
not errors, so always pass `--mathml` when validating.

If `pandoc` is not installed, fall back to `cmark-gfm` for a structure-only
check (`cmark-gfm -e table -e strikethrough -e tagfilter README.md > /dev/null`,
exit 0, no warnings) — but `cmark-gfm` does not render math, so also eyeball the
math against the subset above.

## Verification & validation documentation (mandatory)

**Whenever verification and validation (V&V) are concerned, the documentation
must contain both the methodology and the results of the test.** This is a hard
rule for anything that checks physics against a reference — benchmark comparisons,
cross-section reconstruction gates, convergence studies, fidelity comparisons.

Concretely, the doc comment (or `docs/` entry) for a V&V test must state:

- **Methodology** — what is being computed, the reference/benchmark it is judged
  against, the inputs (geometry, material, data source, tolerances), and the pass
  criterion.
- **Results** — the actual measured numbers *with uncertainty* (e.g. `k_eff =
  1.12451 ± 0.00202`, `+12451 pcm` from benchmark), the date/data-version they
  were taken on, and the interpretation (what the result implies about the model).

A V&V test whose documentation states only what it does, but not what it produced,
is incomplete. Record results where a reader meets the test: in the `///` doc
comment of the test/example itself, and — for iterative studies worth citing in a
paper — in the relevant `docs/` development-history entry.


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
| `njoy-outram-park-fork` | **All nuclear data** — NJOY2016 ENDF port (RECONR/BROADR/THERMR/ACER), the Faddeeva kernel, windowed-multipole evaluation, lean-ACE + WMP data blobs, ν̄/χ. Exposes the `XsProvider` surface other crates pull cross sections from. | GPL-3.0 |
| `openmc-libs` | **Monte Carlo transport** — CSG geometry, particle tracking, k-eigenvalue, delta (Woodcock) tracking for doubly heterogeneous media. **Data-free**: pulls cross sections from `njoy-outram-park-fork`. | GPL-3.0 |

> **Neutronics architecture:** the responsibility split (nuclear data ⟂ Monte
> Carlo ⟂ deterministic/TH ⟂ coupling), the dependency graph, and phasing live in
> **`docs/architecture.md`**. Rule of thumb: *all* cross-section /
> nuclear-data code belongs in `njoy-outram-park-fork`; transport crates are
> data-free and pull from it.

**Planned future crates** (not yet in the workspace):

| Crate | Depends on | Targets |
|---|---|---|
| `openfoam-icof` | `openfoam-basic-lib` | **icoFoam** (incompressible laminar PISO) |
| `openfoam-cht` | `openfoam-basic-lib` | **chtMultiRegionFoam** (conjugate heat transfer, multi-region) |
| `openfoam-rho` | `openfoam-basic-lib` | **rhoPimpleFoam** / **sonicFoam** (compressible) |
| `nee-soon` *(working name)* | `teh-o-prke`, `openmc-libs`, `njoy-outram-park-fork`, `openfoam-appbuilder-lib` | Human-readable **integration/coupling** layer: composes MC + deterministic/TH + nuclear data, exposes CFD-coupling interfaces, PRKE + surrogates. See `docs/architecture.md`. |
| **GenFOAM** (deterministic + TH) | *ported inside* `openfoam-appbuilder-lib` | Deterministic neutronics + thermal hydraulics. On hold until the MC + nuclear-data path matures. |

**Layer 5 (solver loop logic) MUST live in these separate crates**, not in
`openfoam-basic-lib`.  `openfoam-basic-lib` provides the mathematical building
blocks (Layers 1–4) only; the PISO/PIMPLE loop, multi-region coupling logic,
and turbulence model registries belong in solver-specific crates so that
`openfoam-basic-lib` stays publishable independently and is reusable by other
projects.

Internal dependency edges (all by **path**, not crates.io):
`teh-o-prke → {tuas, chem-eng}` (dev); `tuas` dev-deps → `chem-eng`, `teh-o-prke`;
`tampines` dev-deps → `{tuas, teh-o-prke, chem-eng}` (the FHR simulator examples use TUAS —
the `tampines` **library** itself is TUAS-free).
`openfoam-basic-lib` has no internal deps (pure third-party: `uom`, `ndarray`, `thiserror`).
`njoy-outram-park-fork` is lean (`thiserror`, `uom`; no BLAS) so data consumers stay light.
Neutronics edges (target): `openmc-libs → njoy-outram-park-fork` (cross sections; declared in
root workspace deps, wiring deferred); `nee-soon → {openmc-libs, njoy-outram-park-fork, teh-o-prke, openfoam-appbuilder-lib}`.

## Dependency policy — single source of truth

All third-party versions live in the root `[workspace.dependencies]`. Members
inherit them with `<dep>.workspace = true`, so versions **cannot drift**. **When
changing a shared dependency, edit the root `Cargo.toml` only.** The one
exception is `ndarray-linalg`, whose BLAS backend feature is chosen per-target by
each member (`openblas-system` on unix, `intel-mkl-static` on windows/macos).

See `docs/workspace-maintenance.md` for the rationale and the planned
`ndarray-linalg` removal from TUAS.

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

Note: a bare `cargo test --workspace` also compiles the **examples**. Use
`--lib --tests` to skip them.

## Reference material (read on demand, not per turn)

These live in `docs/` so they don't load on every turn — consult them only when
doing the relevant task:

- **`docs/workspace-maintenance.md`** — dependency-upgrade rationale, the
  2026-06 consolidation/migration history and version-bump table, the
  crates.io **publishing order and procedure**, Wayland/display notes, and the
  AI model-selection guide.

Each member crate has its own `CLAUDE.md` (crate-specific architecture and
rules) and, where relevant, a crate-level `docs/` for its reference material.


<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:6cd5cc61 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->
