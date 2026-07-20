# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in the `nee_soon` crate.

## What this is

**NEE_SOON** — **N**eutron **E**nergy-dependent **S**imulation using
**O**pen-source **O**bject-**O**riented **N**umerics.

The **coupling / integration layer** of the OUTRAM PARK suite. It composes the
neutronics + kinetics crates behind a single object-oriented facade so users
assemble simulations without wiring the crates together by hand.

| Composes | Crate | Role |
|---|---|---|
| Nuclear data / cross sections | `njoy-outram-park-fork` | energy-dependent σ(E), ν̄, χ, WMP |
| Monte Carlo transport | `outram-mc-libs` | CSG geometry, k-eigenvalue, Woodcock tracking |
| Point reactor kinetics | `teh-o-prke` | PRKE precursor / reactivity time response |
| GeN-Foam SP3 multiphysics | `outram-foam-appbuilder-lib` | SP3 neutronics + porous-media TH + multi-region coupling (host for the Xin Wang SP3 workflow) |

See the workspace `docs/architecture.md` for the responsibility split
(nuclear data ⟂ Monte Carlo ⟂ deterministic/TH ⟂ coupling).

## Status

**Mostly scaffold; the prompt-excursion path is real.**
`NeeSoon::new_prompt_excursion_model` is real, wired code — a thin pass-through
to `teh-o-prke`'s `NordheimFuchsExactTimestepper`, backed by an integration
test (bead op-fr2.1, closed). The `xin_wang_sp3_workflow` module now exists as a
documented four-stage scaffold: its stage `run()` methods return
`WorkflowError::NotYetImplemented` (each naming its tracking bead), while
carrying real Mk1 case data. The `njoy-outram-park-fork` and `outram-mc-libs`
coupling logic is still future work. **Do not add physics kernels here** — only
orchestration / facade / cross-crate glue belongs in this crate.

## Design rules

- **One big struct.** The public surface is reached through `NeeSoon` — the
  object-oriented entry point that creates the simulation pieces. Keep the
  crate navigable by `rust-analyzer` alone; every public item needs a `///`
  doc comment (what physical quantity, valid ranges, units).
- **Expose and integrate — do not reimplement.** New cross-section code goes to
  `njoy-outram-park-fork`; new transport to `outram-mc-libs`; new kinetics to
  `teh-o-prke`. Only *new coupled* functionality belongs here.
- **Dimensioned units.** All public physical quantities use `uom`, never bare
  `f64`.
- Follow the workspace-wide architecture rules (root `CLAUDE.md`): no
  `Box<dyn Trait>` / no trait-object dispatch (use enums), no `Box<T>`, no
  lifetime parameters, `Arc<RwLock<T>>` over channels for shared state.

## Build & test

Always `--release` (workspace rule). System OpenBLAS required (pulled in via
`outram-mc-libs`).

```bash
cargo build --release -p nee_soon
cargo test  --release -p nee_soon
```
