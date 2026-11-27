<!--
SPDX-License-Identifier: AGPL-3.0-only
Part of Outram Park (outram-park-backend).
A Rust translation of selected OpenFOAM multiphase solvers. Independent fork,
not the official OpenFOAM software; see the workspace TRADEMARKS.md.
-->

# outram-foam-multiphase

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. For research, education and V&V only. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping pass" command). A crate is **complete** only once the maintainer has personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

**OUTRAM-FOAM Phase II — multiphase CFD** (bead epic `op-2kk`). Pure-Rust
translation of OpenFOAM's multiphase solver family on top of
[`outram-foam-basic-lib`](../outram-foam-basic-lib)'s finite-volume framework.
This is the **authoritative high-fidelity reference** from which TAMPINES' 1D
reduced-order system-code physics (epic `op-dt3`) are derived — 1D models must
trace back to a validated 3D reference here, never be invented independently.
Independent OUTRAM PARK fork, not the official OpenFOAM (see `TRADEMARKS.md`).
Nothing here is validated yet (see the status line below), so that role is
the intent, not a present fact.

**Name.** `outram` is the workspace's OUTRAM: **O**pen-source **U**nified
**TR**Ansient **M**ulti-Physics (from OUTRAM PARK, `docs/ecosystem-naming.md`);
`foam` marks the OpenFOAM lineage; `multiphase` is its scope. **The question it
answers:** how do OpenFOAM's multiphase models (drift flux, Euler-Euler two
fluid, interfacial drag and heat transfer) and the boiling closures a system
code needs (wall boiling, CHF, post-dryout) compute what they compute?

**What this crate lets you study and challenge.** Each closure is written once,
with its paper or its OpenFOAM class named beside it, so a student can set the
Zuber-Findlay drift relation, the Schiller-Naumann drag law or the Ranz-Marshall
Nusselt number against the formula in the source. The record of the
comparison with ESI OpenFOAM-v2506 keeps its missed predictions: it found that
glibc's `cbrt` is correctly rounded in only 2159 of 4096 cases, and that ESI
and the Foundation line read different conductivities in
`sphericalHeatTransfer`. The CHF and dryout correlations have no OpenFOAM
counterpart and are checked against their papers.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Upstream code: OpenFOAM, Foundation line (`incompressibleDriftFlux`, `multiphaseEuler`; OpenFOAM-dev `852527377c7e` read for the comparison) and ESI OpenFOAM-v2506 (`driftFluxFoam`, `twoPhaseEulerFoam`, the comparison build), GPL-3.0. Papers cited in the module docs: Zuber and Findlay (1965), Kurul and Podowski (RPI wall boiling), Biasi et al. (1967), W-3, Bowring, Groeneveld et al. (2007) look-up table, Dougall and Rohsenow (1963). No `crates/kovan-literature/CATALOGUE.md` entry is tied to this crate. |
| Mathematical equations | Module docs: mixture continuity and momentum with algebraic slip (`drift_flux`, `pimple`), per-phase continuity and momentum with shared pressure (`two_fluid`, `two_fluid_pimple`, `two_fluid_pimple_esi`), drag (Schiller-Naumann, Wen-Yu), interfacial heat transfer `K` (Ranz-Marshall, spherical, Gunn; `heat_transfer`), RPI heat-flux partitioning (`wall_boiling`), CHF correlations (`chf`), film-boiling HTC (`dryout`). |
| Source code | `drift_flux`, `pimple`, `two_fluid`, `two_fluid_pimple`, `two_fluid_pimple_esi`, `heat_transfer`, `wall_boiling`, `chf`, `dryout`. |
| Interactive simulations | None yet. |
| Visualisations | None yet. |
| Verification and validation evidence | [`verification_and_validation/upstream_comparison_gh1367.md`](verification_and_validation/upstream_comparison_gh1367.md) (port runs 2026-11-12, against ESI v2506): drift flux `OpenFoamSimple` 7993 / 8000 cells bit-identical (1 ulp worst), `rho_mixture` and `tauDm` 8000 / 8000; SchillerNaumann drag bit-identical, `WenYuEsi` 4096 / 4096; Ranz-Marshall `K` 2601 / 4096 bit-identical, worst 6.2e-16, every difference traced to `cbrt` / `pow`. Tests: `tests/upstream_drift_flux_gh1367.rs`, `tests/upstream_drag_gh1367.rs`, `tests/upstream_heat_transfer_gh1367.rs`. `two_fluid_pimple_esi` agrees with ESI `twoPhaseEulerFoam` to about 1e-12 of the field peak for 10 steps (record in `outram-foam-appbuilder-lib`, below). |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp (no `kind = "review"` block in any `review.md` here). No entry in `docs/human-corrections-to-ai-work.md` names this crate. |

**Status:** the drift-flux, drag and heat-transfer closures and the ESI
two-fluid step have AI-run code-to-code evidence against ESI v2506; the
dev-form `pimple` and `two_fluid_pimple` loops, wall boiling, CHF and dryout
have unit tests only; nothing is validated against experiment or reviewed by a
human.

## `no_std` + `alloc` (gh:#1594)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p outram-foam-multiphase --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). `std` turns on `outram-foam-basic-lib`'s and `thiserror`'s `std`. With `std` on (the default)
nothing changes.

## Roadmap

| Stage | Module | OpenFOAM ref | Bead | Status |
|---|---|---|---|---|
| 1 — Drift Flux | `drift_flux` | `incompressibleDriftFlux` | `op-2kk.1` | Foundation done |
| 2 — Euler-Euler two-fluid | `two_fluid` | `multiphaseEuler` | `op-2kk.2` | Foundation done |
| 3 — Wall boiling framework | `wall_boiling` | OF wall boiling (RPI) | `op-2kk.3` | Foundation done |
| 4 — CHF models | `chf` | Biasi/W-3/Bowring/Groeneveld | `op-2kk.4` | Foundation done |
| 5 — Dryout / post-dryout | `dryout` | — | `op-2kk.5` | Reserved framework |
| — Interfacial heat transfer | `heat_transfer` | `multiphaseEuler` heat-transfer models (RanzMarshall, spherical, Gunn) | — | Ported (gh:#1367); row added 2026-11-27 |

**Pressure-velocity coupling.** Two segregated PISO/PIMPLE loops close the
momentum + pressure solve the drift-flux and two-fluid foundations leave out:
`pimple` (drift-flux mixture Rhie-Chow pressure correction, advancing
`U_m`–`p`–`α`) and `two_fluid_pimple` (shared-pressure Euler-Euler PISO,
advancing `U_d`–`U_c`–`p`–`α_d`). Both are tested foundations (hydrostatic
balance, at-rest stability), not validated CFD. A third module,
`two_fluid_pimple_esi` (gh:#1363 row 5), is a translation of ESI v2506
`twoPhaseEulerFoam`'s step (drag and constant virtual mass, laminar, constant
density, wall boundaries) that agrees with the real solver to about 1e-12 of
the field peak for 10 steps on a closed bubble cloud; the dev-form
`two_fluid_pimple` differs from it by 23 to 75 % there (record:
`outram-foam-appbuilder-lib/verification_and_validation/reacting_two_phase_euler_foam_vs_openfoam.md`).

All foundations are **unit-tested translations, not benchmark-validated**
(verification checks only; ~~drag-only interfacial force~~ **CORRECTED
2026-11-27** — interfacial momentum is drag only, except constant virtual mass
in `two_fluid_pimple_esi`, and interfacial heat transfer is in `heat_transfer`
(checked in `src/`); validation is a later human step).

Definition of done for every solver: theory documentation + verification tests
+ reference-benchmark comparison + unit-safe (`uom`) implementation. Humans own
physics, verification, validation, benchmarking, and engineering judgement; AI
only accelerates translation.

## License

**AGPL-3.0-only** combined work (`license.workspace = true` in `Cargo.toml`);
ported files keep their GPL-3.0 headers and `LICENSE.GPL-3.0` ships beside
`LICENSE` (see `docs/relicensing-agpl-2026-10-20.md`).

## Copyright

Copyright (C) 2026 Ong Kay Chen Theodore, Professor Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Lab,
Singapore Nuclear Research and Safety Institute (SNRSI),
National University of Singapore (NUS), Repository Contributors.
