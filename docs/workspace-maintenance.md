# Workspace maintenance notes

Reference material for the OUTRAM PARK workspace. This is **not** per-turn
guidance — it's consulted on demand when doing dependency upgrades, publishing,
or running GUI examples. The mandatory working rules live in the root
`CLAUDE.md`.

## Dependency policy — single source of truth

All third-party versions live in the root `[workspace.dependencies]`. Members
inherit them with `<dep>.workspace = true`, so versions **cannot drift** between
crates. When changing a shared dependency, edit the root `Cargo.toml` only.

`ndarray-linalg` is the one exception that needs per-crate attention: the BLAS
backend feature is chosen per-target by each member
(`openblas-system` on unix, `intel-mkl-static` on windows/macos).

**Future: removing `ndarray-linalg` from TUAS.** `outram-foam-basic-lib` does
**not** use `ndarray-linalg` — its `SquareMatrix` module implements LU
factorisation in pure Rust. All `ndarray-linalg` usage in TUAS bottoms out in
one call: `M.solve(&S)` in
`array_control_vol_and_fluid_component_collections/standalone_fluid_nodes/mod.rs`
(`solve_conductance_matrix_power_vector`), which performs a dense LU solve on
the per-timestep conductance matrix (typically 10–50 × 10–50, not tridiagonal
because lateral coupling fills off-band entries). Replacing it with
`outram_foam_basic_lib::matrix::SquareMatrix::lu_solve` would eliminate the
OpenBLAS system dependency from TUAS entirely. That requires changing
`solve_conductance_matrix_power_vector`'s signature and its ~13 call sites —
a moderate refactor, not urgent.

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

**Do not hand-maintain the publish order.** Derive it from `cargo metadata`,
which is the only source that cannot go stale:

```bash
cargo metadata --format-version 1 --no-deps
```

Take the internal (workspace-member) dependency edges — **including
dev-dependencies**, because `cargo publish` resolves those against crates.io
too — and topologically sort. A crate can only be published once everything it
depends on, normal *or* dev, is already live. Until then `cargo publish
--dry-run` / `cargo package` fails with "failed to select a version"; that is
expected, not a packaging error (`cargo package --list` still shows clean
contents).

Internal deps are `{ path = …, version = … }` in `[workspace.dependencies]`, so
each pin must be kept in sync with that crate's own `version`, and a downstream
pin bumped whenever an upstream is bumped.

Publish each with `cargo publish -p <crate>` from the workspace root (commit
first; `cargo publish` refuses a dirty tree without `--allow-dirty`).

### Drift: the published version is not the local version

A crate whose `version` is unchanged since it was published, but whose `src/`
has moved on, is a trap. Downstream crates build locally against the *path*
dependency but are published against the *registry* copy, so a downstream
publish fails — or worse, silently resolves to stale code. Detect it before
starting a run:

```bash
# commits touching a crate since the commit that introduced its current version
bump=$(git log --format=%H -S'version = "0.1.0"' -1 -- crates/<crate>/Cargo.toml)
git rev-list --count "$bump"..HEAD -- crates/<crate>/src
```

Anything above zero means the registry copy is stale and the crate needs a patch
bump before the chain above it can be published.

### Rate limits (measured 2026-08-03)

crates.io throttles **new crate names** far harder than new versions of existing
crates: a burst of about **5 new crates**, then roughly **one per 10 minutes**.
Exceeding it returns `429 Too Many Requests` with an explicit `Please try again
after <RFC-2822 date>` — wait for that timestamp rather than retrying blind. New
*versions* of already-published crates were not throttled at all in that run.

A separate `503` from the WAF has been seen when publishing many crates in quick
succession; on a 503, wait ~30 minutes before retrying.

### Every published crate must carry its licence text

The tarball contains only the crate directory — a `LICENSE` at the workspace
root does **not** ship. GPL-3 requires the licence be conveyed with the work, so
every crate needs its own `LICENSE` copy. Crates that are ports of a permissive
upstream additionally need that upstream's notice verbatim (MIT and BSD-3 both
require the copyright + permission notice travel with substantial portions):
`njoy-outram-park-fork` ships `LICENSE.njoy` + `NOTICE`, `outram-mc-libs` ships
`LICENSE.openmc`. Verify with:

```bash
cargo package -p <crate> --list | grep -iE 'LICENSE|NOTICE|TRADEMARK'
```

Note that `include` patterns are **gitignore-style globs**: an unanchored
`"LICENSE*"` matches at *any* depth and will drag vendored `upstream_source/`
licence files into the package, which then fails as a dirty tree. Anchor them
with a leading `/`.

**Package hygiene already applied** via `exclude` in each manifest:
- `tuas_boussinesq_solver`: `exclude = ["*.csv"]` — tests dump ~58 MB of CSVs into
  the crate root (far over the crates.io size limit).
- `teh-o-prke`: `exclude = ["pki", "docs"]` — `pki/` holds a throwaway **dummy**
  key from early experimentation (see `crates/teh-o-prke/CLAUDE.md`), excluded
  from the package as belt-and-braces; `docs/` is a large theory PDF.
- `tampines-steam-tables`: `exclude = ["docs"]` — LaTeX sources/build artifacts.

Note: `teh-o-prke/pki/` is **not a security concern** — it is a dummy key from
early playing-around, is **untracked by git** (so it is never committed or
pushed), and is `exclude`d from the package. Confirmed by the maintainer
(2026-07-16). See `crates/teh-o-prke/CLAUDE.md`.

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
