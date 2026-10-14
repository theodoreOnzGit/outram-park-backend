# Extended: the consumer surface

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response.

> **Review status: first draft, 2026-10-04, AI-assisted, not yet reviewed by a
> human.**

*Part II, extended deep dive. Modules: `nuclear_data`, `photon`, `interface`,
`prelude`, `units`, `common`, `modules`, `error`, `vv`, `perf_report`,
`wasm_par`, and the `njoy-tui` binary.*

## The question

The rungs follow NJOY's modules. A program that just wants σ(E, T) for U-235
does not want to know there is a module called BROADR. **What does this crate
offer a caller, and how is it held together?**

## The layers a caller meets

| module | what it is for | entry points |
|---|---|---|
| `interface` | the "builder" API: file → RECONR → BROADR → query, with `uom` units | `NuclearDataLibrary::from_file(path, mat)?.reconstruct(0.001)?.broaden(T)?`, then `total_xs(E)`, `elastic_xs`, `fission_xs`, `capture_xs`, `xs_for_reaction(mt, E)`; `write_ace`, `write_ace_type2`, `to_continuous_energy_data` |
| `prelude` | one `use` for the common pipeline, **including the multigroup collapse** (added after an API dog-food run could not find it, gh #58) | `use njoy_outram_park_fork::prelude::*;` |
| `nuclear_data` | what transport crates call per collision: `XsProvider` (WMP or MGXS), `MicroXs`; ν̄, χ, delayed-neutron data (`secondary`, `delayed`, `delayed_mgxs`), weighting spectra | `XsProvider::micro(E, T)`, `NuBar::from_endf`, `FissionSpectrum::from_endf_mf5` |
| `photon` | photon production energy (MF=12/13/15), the term HEATR's energy-balance method subtracts; ENDF MT=442 | `PhotonProduction::from_endf(tape, mat, &recon)`, `eval(E, &recon)` |
| `units` | named `uom` aliases so hover-tips read `NeutronEnergy`, `CrossSection`, `Temperature` | — |
| `common::phys` | NJOY's `phys.f90` constants (CODATA 2018), the single source for every module | `BK_EV_PER_K`, `AMASSN_AMU`, … |
| `modules` | the `NjoyModule` enum (enum dispatch, no trait objects); every card-deck `run()` is `NotPorted` and typed APIs are the entry | `NjoyModule::run` |
| `error` | `NjoyError` (`thiserror`): no process-aborting `error()` as upstream does | — |
| `vv` | the gate helpers every oracle comparison uses, re-exported by `outram_mc_libs::vv` so there is one implementation | `assert_relative`, `assert_table_relative`, `assert_reproduces_keff`, `assert_monotone` |
| `perf_report` | per-machine hardware-labelled benchmark reports, written to a git-ignored path | — |
| `wasm_par` | serial stand-ins for the `rayon` adapters on `wasm32`, bit-identical results | private |
| `reference_data`, `acquire` | where tapes are and how to fetch/cache them (rung 1) | `reference_endf`, `EndfCache` |
| `bin/njoy-tui` | a mobile-first terminal cross-section browser (Termux first): fuzzy nuclide finder, log–log braille plot, temperature in °C/K | `cargo run --release -p njoy-outram-park-fork --bin njoy-tui` |

## In the code: the builder API

> **On the [Code Review map](../../code-review/):** start at [`NuclearDataLibrary::broaden`](../../code-review/#crates/njoy-outram-park-fork/src/interface.rs::NuclearDataLibrary::broaden) and follow its callees (▸) to [`broadr_joint`](../../code-review/#crates/njoy-outram-park-fork/src/broadr/joint.rs::broadr_joint). Each card opens the function's source and shows its review stamp.

> **The physics functions:** this page is about the API and has none of its own; each physics function is listed under the rung that owns it. See [Where the physics is](./physics-functions.md#consumer-surface) (a proposal for maintainer review, 2026-10-14; the entry-point link above stays until it is accepted).

**Reading the path.** The table below is the path the Code Review map link above follows, with what each hop does:

| hop | function | what it does | resolved (kopitiam pass, 2026-10-04) |
|---|---|---|---|
| 0 | [`NuclearDataLibrary::reconstruct`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/interface.rs#@@L:crates/njoy-outram-park-fork/src/interface.rs:fn=reconstruct@@) | runs RECONR at the given tolerance | entry point |
| 1 | `reconr::reconr` | rung 2 | kopitiam |
| 0 | [`NuclearDataLibrary::broaden`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/njoy-outram-park-fork/src/interface.rs#@@L:crates/njoy-outram-park-fork/src/interface.rs:fn=broaden@@) | BROADR to T | entry point |
| 1 | `broadr::broaden_result` → `broaden_result_with_tolerance` → `joint::broadr_joint` | rung 3 | kopitiam |
| 0 | `total_xs(E)` etc. → `eval_lin_lin` on the section | lin-lin lookup, the only interpolation a PENDF needs | filled by hand |


## The checks

- **The `vv` module is the check mechanism itself.** Since 2026-09-11 the
  oracle examples *assert* their comparison instead of printing it and exiting
  0 whatever the numbers (crate `CLAUDE.md`, "Oracle examples assert their
  comparison"). `seam_stage_probe`, `endf_to_broadened_xs`,
  `tutorial_resonance_to_groups`, `temperature_thinning_study` and
  `graphite_sab_generation` are gated this way.
- **`photon`:** ~~feeds `acer::photon_blocks` and~~ feeds the HEATR
  energy-balance correction (MT=442). **CORRECTED 2026-10-05:** it does not
  feed `acer::photon_blocks`, which has its own parser (its module doc says
  why); since #535's H6a the dependency runs the other way, `photon` reusing
  `photon_blocks`'s LO=2 cascade (`Lo2Cascade`). ~~its parse is exercised by
  `tests/photon.rs` and, through ACER, by the photon blocks being
  word-identical to NJOY2016's on U-234/235/238 (rung 9). Not ported: MF=12
  `LO = 2` cascades (`hconvr`); such a reaction keeps its photon energy local,
  an over-count stated in the module doc.~~ Its parse is exercised by
  `tests/photon.rs`, and its MT=442 matches NJOY2016's HEATR at print
  precision on Fe-58 and Si-28 (`tests/heatr_mt442_vs_njoy2016.rs`), with
  LO=2 cascades, MF=6 photons and capture by energy balance all ported
  ([rung 6](./heatr.md)).
- **`interface`:** its query path is the same `reconr` + `broaden_result` the
  rungs verify; there is no separate oracle for the wrapper.
- **`njoy-tui`, `perf_report`, `wasm_par`:** tooling, not physics. `wasm_par`
  is checked by `tests/wasm_fallback.rs`; the browser demos are its
  consumer.
- **`modules`:** no physics; the `NotPorted` drivers are visible rows in the
  [coverage table](./coverage.md).

**Next:** [Coverage table](./coverage.md).
