# DHOBY GHAUT

**D**igital **H**igh-fidelity **O**rchestration **by** **G**UI for a
**H**uman-friendly **A**utomated **U**nified **T**oolkit.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`. The studios are offline
> demonstrations: never connect one to a live operational, plant,
> safety-critical or restricted system.

The home for OUTRAM PARK's graphical studios, the ones that drive physics
rather than only display it. Windowing GUIs (`egui`/`eframe`) cannot build for
Android/Termux, which every non-GUI library in the workspace must. This crate
is where the windowing stack lives, so that `nee_soon`, `outram-blender` and
the solver crates never have to carry it. It adds no physics of its own: a
studio that needs a quantity the solver crates do not expose gets it added
there.

## What is here

**The library is a placeholder.** Its only public item is the `EXPANSION`
constant, and it has no `[dependencies]`. The two studios are **examples**,
moved here from `outram-blender` on 2026-09-17:

| Example | What it does |
|---|---|
| `mc_studio` | Author a geometry, set a material and run settings, run a basic `outram-mc` k-eigenvalue calculation through `outram_blender::sim`, and read `k_eff ± σ` with the per-generation plot |
| `mesh_studio` | Author a surface in `outram-blender`, volume-mesh it through `outram-park-fork-cfmesh`'s tet → dual → boundary-layer pipeline, show the mesh statistics, and export an OpenFOAM `polyMesh` |

```bash
cargo run -p dhoby-ghaut --example mc_studio --release
cargo run -p dhoby-ghaut --example mesh_studio --release
```

Each studio has a `--headless` mode that prints one CSV row per case with no
window. Four `#[test]`s per studio check it against a committed fixture under
`tests/fixtures/`:

```bash
cargo run  -p dhoby-ghaut --example mc_studio --release -- --headless
cargo test -p dhoby-ghaut --examples --release
```

The GUI dependencies (`eframe`, `egui`, `egui_plot`, `env_logger`, and
`outram-blender` with its `mc-export` and `foam-mesh` features) are
dev-dependencies gated off Android. On Android each example compiles to an
empty `main`.

The non-GUI halves of both bridges still live in `outram-blender`
(`src/sim.rs`, `src/foam_mesh.rs`, `src/export.rs`). `src/lib.rs` records the
intent to move the headless Monte Carlo bridge into `nee_soon` as ungated code;
that has not happened.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## License

GPL-3.0. Part of the [OUTRAM PARK](../../README.md) workspace.
