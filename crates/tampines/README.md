# TAMPINES

**T**hermal-hydraulic **A**rtificial-intelligence **M**ulti-**P**hase
**IN**tegrated **E**mulator **S**ystem — the central thermal-hydraulic
framework of the [OUTRAM PARK](https://github.com/theodoreOnzGit/outram-park-backend)
suite.

TAMPINES makes the thermal-hydraulic models of a reactor system **studyable
and challengeable**: clean-room equivalents of the RELAP-7 Theory Manual's
HEM, PCICE, seven-equation, heat-structure and control-volume models (the
manual is not public, so each equation is cited by number only, and no RELAP
source was read), the KTA and ZBS packed-bed correlations, a helical-coil once-through steam generator, and the HTR-10
primary loop, each with a V&V record that wrote its predictions down before
the run. A reader can follow an equation number from the manual to the Rust
function and then to the test that checks it.

> ⚠️ ~~**Scaffold only — version 0.0.1 is a placeholder release.** This crate is
> being built out incrementally and is **not** ready for use. It is published to
> reserve the name and to let downstream OUTRAM PARK crates resolve against
> crates.io. See the `op-dt3` epic in the workspace issue tracker for the live
> module plan and progress.~~ **CORRECTED 2026-11-27** — the crate is at
> version 0.0.5 (`Cargo.toml`) and `src/lib.rs` declares 21 public modules
> with implementations (listed below); kovan rates it maturity 1, "AI draft"
> (`[package.metadata.kovan]`). It is an unreviewed AI draft, not a scaffold.
> Work is tracked in GitHub issues (`op-*` ids are historical).

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified
> and untrusted** unless a specific verification & validation (V&V) case
> demonstrates otherwise. See the workspace `VERIFICATION_AND_VALIDATION.md` and
> `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control,
> safety-critical, or licensing decisions.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | RELAP-7 Theory Manual (not public; cited by section and equation number in each V&V record, e.g. §5.3 Eqs 257–274 for PCICE); Saurel & Abgrall (1999), *J. Comput. Phys.* 150, 425–467, for the seven-equation model (`verification_and_validation/seven_equation.md`); MELCOR surface correlations for the control volume (`verification_and_validation/control_volume.md`); KTA and ZBS packed-bed correlations (`src/gas_phase/kta_bed.rs`, `src/pebble_bed/zbs.rs`). Catalogue entries naming this crate: `wang2018coupled` (FHR pebble-bed methodology) and NUREG/CR-2671 Marviken, whose `tampines` comparison cases have not landed (`crates/kovan-literature/CATALOGUE.md`). |
| Mathematical equations | Written in the module docs and V&V records: HEM and drift-flux closures (`src/hem`, `src/closures`), the 1-D multiphase two-fluid and seven-equation systems (`src/multiphase_1d`), PCICE (`src/pcice`), a 1-D FEM against FV comparison (`src/fem1d`), lumped control volumes and their networks (`src/control_volume`, `src/cv_network`), subchannel cross-flow mixing (`src/subchannel`), helical-coil OTSG march with Mishra–Gupta and Gnielinski coil closures (`src/components/helical_coil_otsg`). Design notes in `docs/` (`six-equation-regularisation.md`, `hem-knp-stabilisation/`, `steam_generator_tube_integration.md`). |
| Source code | `src/`: `balance_of_plant`, `closures`, `components` (pipe, pump, valve, heat exchanger, steam generators, turbine, condenser, cooling tower, wet well, wall condensation, reactor power source), `compressible`, `control_volume`, `cooling_tower`, `critical_flow`, `cv_network`, `fem1d`, `flow_path`, `fluids`, `gas_phase`, `heat_transfer`, `hem`, `htr10_primary_loop`, `humid_air`, `multiphase_1d`, `pcice`, `pebble_bed`, `single_phase`, `subchannel`. |
| Interactive simulations | None in this crate. `crates/dhoby-ghaut/examples/helical_sg_web/` drives `tampines::components::helical_coil_otsg`, and `crates/outram-park-digital-twin-engine/examples/htgr_sim_v1` and `fukushima_web` use it. |
| Visualisations | `verification_and_validation/helical_coil_otsg_gh1157_geometry.png`; `verification_and_validation/htr10_primary_loop/lumped_core_layout.svg`. |
| Verification and validation evidence | `verification_and_validation/` (15 records plus `htr10_primary_loop/`) and `tests/`. Examples as recorded: helical-coil OTSG (2026-11-03) energy closure 6.7e-14 after four runs whose first three missed and exposed defects, duty 10.914 MW, steam out 554.4 °C; control volume (2026-11-01) water/N2/H2 mass closure 1.4e-14 / 1.9e-14 / 1.9e-16 relative; flow path (2026-11-01) second-order error ratio 4.000 where first order was predicted. Code-to-code and analytical checks; none of it is validation against plant data. |
| Human reviews and corrections | 0 functions carry kovan human-review stamps. `docs/human-corrections-to-ai-work.md`, 2026-09-20 "A pebble bed is a POROUS MEDIUM", cites `src/pebble_bed/` as the existing porous-media treatment. The helical-coil record strikes through and re-measures two `multiphase_1d` pins moved by a steam-table fix. |

**Status:** the components above have code-to-code and analytical V&V records
written by an AI and not yet reviewed by a human; nothing here is validated
against experiment, and every model is unverified until a human signs off.

## What it is

TAMPINES owns all fluid flow, thermal-hydraulics, thermophysical properties,
heat transfer, balance-of-plant components, humid-air psychrometrics, and
multiphase thermal-hydraulics for OUTRAM PARK.

**The one question it answers:** given a heat source as an input, how do
mass, momentum and energy move through a reactor's coolant circuit and
balance of plant?

It is distinct from **`tampines-steam-tables`**, which is only the IAPWS-IF97
property library — one of the backends TAMPINES composes.

## What it composes

| Piece | Provided by | Role |
|---|---|---|
| Single-phase liquid thermal-hydraulics | `tuas_boussinesq_solver` | Boussinesq single-phase pipe/component flow |
| Compressible / two-phase properties | `outram-park-fork-coolprop` | CoolProp-derived thermophysical properties |
| IAPWS-IF97 steam/water properties | `tampines-steam-tables` | Steam-turbine and choked-flow equations |
| Finite-volume building blocks | `outram-foam-basic-lib` | Tensor algebra, ODE/polynomial solvers, FV operators |
| Process control | `chem-eng-real-time-process-control-simulator` | PID / transfer-function control loops |
| Equipment-model correlations | `outram-park-fork-dwsim-libs` | Pipe/valve/heat-exchanger/expander/pump sizing and rating equations |

## What belongs here, and what does not

- **Belongs here:** fluid-flow and thermal-hydraulic component models (pipes,
  pumps, valves, heat exchangers, steam generators, turbines, condensers,
  cooling towers), balance-of-plant composition, humid-air psychrometrics, and
  multiphase thermal-hydraulics (HEM, drift-flux, CHF).
- **Does not belong here:** raw property-table equations (those live in
  `tampines-steam-tables` / `outram-park-fork-coolprop`), reactor physics
  (`teh-o-prke`, `outram-mc-libs`, `njoy-outram-park-fork`), or GUI /
  visualization code (`outram-park-digital-twin-engine`).

## Intended use

Education, research, capability building, and verification/validation only. This
crate is **not** for nuclear facility operation, reactor control, licensing
decisions, safety-critical decision-making, emergency response,
safeguards-sensitive analysis, or real-time plant monitoring.

## `no_std` + `alloc` (gh:#1606)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p tampines --lib --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). **Every module builds without it**,
`multiphase_1d`, `htr10_primary_loop`, `fem1d`, the pebble-bed and gas-phase
models included, because every dependency now does too (the last three were
`farrer-park` gh:#1632, `outram-foam-appbuilder-lib` gh:#1633 and `boon-lay`
gh:#1634). `std` turns on `std` in every dependency, and printing. The root
workspace entry is unchanged, so dependents keep `std`; with `std` on (the
default) nothing changes.

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

~~GPL-3.0-only~~ **AGPL-3.0-only** combined work (relicensed 2026-10-20, gh:#973; ported files keep GPL-3.0 and `LICENSE.GPL-3.0` ships beside `LICENSE`; see `docs/relicensing-agpl-2026-10-20.md`). The AGPL-3.0 text ships with this crate as `LICENSE`.

## Copyright

Copyright (C) 2026 Ong Kay Chen Theodore,
Ethan Yew Hoe, Wong,
Professor Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Lab,
Singapore Nuclear Research and Safety Institute (SNRSI),
National University of Singapore (NUS), Repository Contributors.
