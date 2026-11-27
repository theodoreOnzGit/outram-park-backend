# outram-park-fork-pflotran

An **independent, pure-Rust fork / translation** of
**[PFLOTRAN](https://www.pflotran.org)** — the US-DOE national-lab subsurface
**flow and reactive-transport** simulator — rebuilt to OUTRAM PARK's design
rules: enum dispatch (no trait objects), `uom`-typed API boundaries, a
pure-Rust solver (no PETSc FFI, no MPI in v1), and an Android-buildable library.

**Name.** `outram-park-fork-pflotran` is an OUTRAM PARK fork, named for its
upstream, PFLOTRAN; the backronym is OUTRAM PARK's own, letter by letter:
**O**pen-source **U**nified **TRA**nsient **M**ulti-Physics **A**dvanced
**R**eactor simulation **K**it. The REDHILL domain (subsurface radionuclide
transport) is to depend on it (`docs/ecosystem-naming.md`).

**The question it answers:** how do water, heat and dissolved (including
radioactive) species move and react through variably saturated porous rock and
soil?

**What this crate makes studyable and challengeable.** Richards' equation, solute
and heat transport, aqueous speciation, sorption, mineral kinetics and decay
chains as Rust modules a reader can set beside PFLOTRAN's Fortran. The
differential against PFLOTRAN `669fbce` compiles upstream's own routines and
compares bits, and `lineage.md` audits which parts of `src/` were translated
from upstream and which were designed independently, so a reader knows which
code can be checked against PFLOTRAN and which cannot. Every AI-made design
choice is listed in `docs/ai-directed-decisions.md`. It is verification
material to study and challenge, not a validated subsurface code.

> **Independent fork, not the official PFLOTRAN.** This crate is not affiliated
> with, endorsed by, or maintained by the PFLOTRAN development team or the
> national laboratories (LANL, PNNL, ORNL, LBNL, SNL). "PFLOTRAN" is used only
> to identify the upstream work this crate derives from. See `NOTICE` and the
> workspace `TRADEMARKS.md`.
>
> ~~**License: GPL-3.0-only.** PFLOTRAN upstream is LGPL-2.1-or-later; LGPL-2.1
> section 3 lets a licensee relicense a copy under the ordinary GPL, so this
> crate is distributed GPL-3.0-only, consistent with the rest of the suite. The
> exact upstream license must be re-verified byte-for-byte before publish — see
> `NOTICE` and `upstream_source/README.md` (bead op-v6s.1).~~
> **CORRECTED 2026-11-27** — **License: AGPL-3.0-only combined work** (see
> "License" at the end; gh:#973). The upstream licence was re-verified on
> 2026-09-10 from the upstream files themselves: current PFLOTRAN is
> **LGPL-3.0**, and LGPL-2.1-or-later described only the v2.X era (recorded in
> `upstream_source/README.md`, "License determination (2026-09-10) —
> VERIFIED").
>
> **Early v1: RICHARDS solves, but is VERIFICATION-ONLY and has no human V&V.**
> The variably-saturated flow mode runs end-to-end and passes closed-form and
> manufactured-solution verification (2nd-order convergence), but it has **not**
> been validated against published PFLOTRAN reference cases (**CORRECTED
> 2026-11-27**: it has since been compared code-to-code with PFLOTRAN's own
> `default/infiltration1d` gold file, gh:#1537; that is verification, not
> validation against measurement), and no human has
> reviewed it. Use at your own risk. Not for nuclear facility operation, reactor
> control, safety-critical analysis, or licensing decisions — education,
> research, and V&V only.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Upstream: PFLOTRAN (bitbucket.org/pflotran/pflotran, US DOE national laboratories, LGPL-3.0), commit `669fbce204af39428020c6b9d74beae363376689` (2026-09-09); decay routines from commit `2af572b8759c34aacc2216f56a906637f90af03e`; provenance and licence determination in `upstream_source/README.md`. Celia, Bouloutas and Zarba (1990), *Water Resources Research* 26(7), for the Haverkamp infiltration benchmark (`docs/celia-1990-validation-beads.md`). No literature in `crates/kovan-literature/CATALOGUE.md` is tied to this crate. |
| Mathematical equations | Richards' equation and Darcy flux ("Governing equation" below); van Genuchten-Mualem, Brooks-Corey-Burdine and Haverkamp curves (`src/properties/`); module docs of `src/transport/` (advection-dispersion, TVD), `src/energy/`, `src/thermal_convection/` (buoyancy-coupled flow), `src/geochemistry/` (mass action and mass balance), `src/activity/` and `src/pitzer/`, `src/sorption/`, `src/surface_complexation/`, `src/kinetics/` (transition-state theory), `src/decay/` (decay chains with ingrowth), `src/general_mode/`; `docs/general-multiphase-design.md`. |
| Source code | `src/flow/` (RICHARDS), `src/general_mode/` and `src/multiphase/`, `src/transport/`, `src/energy/`, `src/thermal_convection/`, `src/geochemistry/`, `src/kinetics/`, `src/microbial/`, `src/reactive_transport/`, `src/decay/`, `src/sorption/`, `src/surface_complexation/`, `src/activity/`, `src/pitzer/`, `src/properties/`, `src/eos_real/`, `src/eos_co2_brine/`, `src/grid/`, `src/unstructured/`, `src/solver/`, `src/wells/`, `src/deck/` (genuine PFLOTRAN deck subset), `src/io/`, `src/hdf5_io/`, `src/decomposition/`, `src/par.rs`, `src/gpu.rs`, `src/units.rs`. |
| Interactive simulations | CLI example only: `examples/infiltration_column.rs`. |
| Visualisations | None yet (the `io` module writes CSV and legacy VTK for external viewers). |
| Verification and validation evidence | `verification_and_validation/upstream_differential_gh1422/README.md` (2026-11-13) against PFLOTRAN `669fbce` compiled from source: characteristic curves, isotherms and decay bit-identical on most points (decay 18 of 18 constants, 165 of 165 three-generation masses); `default/infiltration1d` gold file matched "to 5.4e-9 Pa absolute, 6e-14 relative, same 20 steps and 62 Newton iterations"; `lineage.md` audits translation versus independent design. `verification_and_validation/upstream_decay/`. MMS convergence of observed order 2.000 (`tests/verification.rs`). Tests: `tests/upstream_differential_gh1422.rs`, `tests/regression_decks_gh1537.rs`, `tests/mpi_sensitivity_gh1435.rs`. No validation against field or laboratory data. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp. No entry in `docs/human-corrections-to-ai-work.md`. Struck-through corrections in this README (v1 scope, test counts, roadmap) and in `upstream_source/README.md` (the licence, LGPL-2.1-or-later corrected to LGPL-3.0). |

**Status:** characteristic curves, isotherms, decay and the RICHARDS solve on
one regression deck have machine-run code-to-code evidence against PFLOTRAN;
the other modes are verified only against closed-form and manufactured
solutions; none of it is human-reviewed or validated against measurement.

## What exists today

Many PFLOTRAN modes are implemented and tested — **verification-only, no
validation, no human V&V**:

| Piece | Module | Status |
|---|---|---|
| Physical-quantity type aliases | `units` | **real** — named `uom` aliases (`FluidPressure`, `Saturation`, `Permeability`, ...) |
| Crate error type | `error` | **real** — `PflotranError` enum; unfinished paths return `NotImplemented` |
| Structured Cartesian FV grid | `grid` | **real** — cells, two-point-flux transmissibility, LDU addressing |
| Fluid EOS + characteristic curves | `properties` | **real** — slightly-compressible water EOS; van Genuchten–Mualem, Brooks–Corey–Burdine, **Haverkamp (Celia)** curves; thermal `rho(p,T)`/`mu(T)`; all FD-checked |
| Input deck + output | `io` | **real (AI-designed subset)** — card-deck parser + CSV/legacy-VTK writers. **Not** real PFLOTRAN deck syntax |
| Newton–Krylov solver | `solver` | **real** — scalar + **block multi-DOF** ([`solver::block`]) Newton driver, Armijo line search, over foam-basic-lib `krylov` |
| RICHARDS flow mode | `flow` | **real (verif-only)** — residual/Jacobian, adaptive timestepping, deck-driven; exports a Darcy flow field |
| Conservative solute transport | `transport` | **real (verif-only)** — advection (upwind + **TVD** via foam `FluxLimiter`) + dispersion; couples to a RICHARDS flow field |
| TH heat transport | `energy` | **real (verif-only)** — advection–conduction of temperature; one-way coupled to flow (buoyancy deferred) |
| Aqueous geochemistry | `geochemistry` | **real (verif-only)** — equilibrium speciation (mass-action + mass-balance Newton); ideal activities |
| Mineral kinetics | `kinetics` | **real (verif-only)** — TST precipitation/dissolution on foam's Rosenbrock/RKF45 ODE solver |
| Reactive transport | `reactive_transport` | **real (verif-only)** — SNIA transport↔geochemistry coupling |
| GENERAL multiphase | `multiphase` | **real (verif-only)** — two-phase air–water on the block solver (2 unknowns/cell) |

**CORRECTED 2026-11-27** — the table above is not the whole crate. `src/lib.rs`
also declares, each with a module doc naming its bead (checked 2026-11-27):
`general_mode` (non-isothermal air-water-energy GENERAL mode, op-v6s.15.5),
`thermal_convection` (two-way buoyancy-coupled TH, op-v6s.15.6), `activity`
(ideal, Debye-Hückel, Davies, op-v6s.15.1) and `pitzer`, `sorption` and
`surface_complexation`, `microbial`, `decay` (decay chains with ingrowth),
`eos_real` (IAPWS-IF97 water) and `eos_co2_brine`, `unstructured` (polyhedral
FV grid), `wells`, `deck` (genuine PFLOTRAN deck syntax, a subset), `hdf5_io`,
`decomposition` (MPI-style domain decomposition on `outram-park-mpi` threads,
op-v6s.15.9), `par` and `gpu` (wgpu demonstrator).

**Test suite:** ~~105 unit + 5 integration + 1 regression + 3 verification (114
total; MMS 2nd-order, hydrostatic gravity, closed-form advection–diffusion, TVD
error reduction), all green in release mode;~~ **CORRECTED 2026-11-13** (counted
from `cargo test --release -p outram-park-fork-pflotran --lib --tests`): 287
unit + 5 integration + 1 regression (+1 ignored) + 3 verification + 11 upstream
differential (gh:#1422) + 1 MPI sensitivity (gh:#1435) = 308 passed, all green
in release mode; both crates cross-compile to
`aarch64-linux-android`. The `outram-foam-basic-lib` `krylov`, `limiters` (OpenFOAM
TVD, translated from upstream), and `solver::block` machinery back all of this.

**Not yet implemented / deferred:** ~~air–water–**energy** 3-phase GENERAL (only
two-phase isothermal so far); two-way (buoyancy) TH coupling; energy-transport
TVD; mineral surface-area evolution / Debye–Hückel activities; **parallelism
(op-v6s.14)**.~~ **CORRECTED 2026-11-27** — still deferred: energy-transport
TVD (the `src/energy/` module doc says the higher-order / TVD scheme is
deferred) and mineral surface-area evolution (`kinetics` keeps one constant
surface area per mineral, its module doc says so). The rest now exist:
`general_mode` (air-water-energy), `thermal_convection` (two-way buoyancy),
`activity` (Debye-Hückel), and `decomposition`, `par` and `gpu`
(parallelism). All validation
work is deferred (open beads op-v6s.9.x/.10.1/.11.1/.12.1/.13.1).

## Verification results (methodology + measured numbers)

- **MMS spatial convergence** (`tests/verification.rs`): saturated 1D operator
  with a manufactured sinusoidal source; Linf pressure error 40.8 → 10.3 → 2.57
  → 0.64 → 0.16 Pa over 10→160 cells, **observed order 2.000** (2nd-order design
  met). Verification of the two-point-flux discretisation.
- **Closed-form steady state**: gravity-free saturated column reproduces the
  exact linear pressure profile to machine zero.
- These are **verification** (implemented correctly?), not **validation**
  (matches reality?). ~~No comparison to published PFLOTRAN gold-files exists yet
  (bead op-v6s.9).~~ **CORRECTED 2026-11-27** — one exists: PFLOTRAN's
  `default/infiltration1d` gold file, matched to 5.4e-9 Pa absolute with the
  same 20 steps and 62 Newton iterations (gh:#1537,
  `verification_and_validation/upstream_differential_gh1422/README.md`). See `docs/ai-directed-decisions.md` for every AI-made choice.

## v1 scope — the vertical slice

The first end-to-end target is deliberately narrow, so a real physics result can
be validated before breadth is added (bead op-v6s.2):

- **Flow mode:** RICHARDS — variably-saturated single-phase groundwater flow.
- **Grid:** structured Cartesian finite volume, two-point flux.
- **Solver:** serial pure-Rust Newton-Krylov (no PETSc, no MPI).
- **I/O:** a minimal card-based ASCII input-deck subset; CSV / VTK output.

~~Explicitly **out of v1**: unstructured grids, MPI / distributed solves, HDF5,
multiphase (GENERAL) flow, energy transport (TH), solute transport, and
reactive geochemistry (GIRT). Those are later beads (op-v6s.10 .. op-v6s.14).~~
**CORRECTED 2026-10-18:** this was the scope of the *first vertical slice*, and
it no longer describes the crate. Checked: `src/lib.rs` declares `transport`,
`energy`, `geochemistry`, `kinetics`, `reactive_transport`, `multiphase`,
`general_mode` and `unstructured`; each is a non-empty module (about 640 to
1600 lines) with `#[test]` functions (transport 8, energy 4, geochemistry 5,
kinetics 5, reactive_transport 4, multiphase 5, general_mode 6,
unstructured 18). The tests were counted, not re-run for this edit. Still out:
MPI / distributed solves (the "What exists today" section lists parallelism as
deferred).

## Governing equation (RICHARDS, v1)

Liquid-phase mass conservation:

$$ \frac{\partial}{\partial t}\left(\phi\, S_l\, \rho_l\right) + \nabla \cdot \left(\rho_l\, \mathbf{q}_l\right) = Q_l $$

with the Darcy flux

$$ \mathbf{q}_l = -\frac{k\, k_{rl}}{\mu_l}\left(\nabla p_l - \rho_l\, \mathbf{g}\right) $$

where `phi` is porosity, `S_l` liquid saturation, `rho_l` liquid density, `k`
intrinsic permeability, `k_rl` relative permeability, `mu_l` viscosity, `p_l`
liquid pressure, `g` gravity, and `Q_l` a source/sink term. Saturation and
relative permeability follow characteristic (retention) curves of capillary
pressure.

## Roadmap (beads)

Tracked under epic **op-v6s** (`outram-park-fork-pflotran`). Status below is
AI-assessed from the code; bead closure is the maintainer's decision.

- **op-v6s.1** — license + provenance — *done (verify upstream before publish)*
- **op-v6s.2** — scope decision (the v1 slice above) — *done*
- **op-v6s.3** — architecture: enum dispatch, `uom`-typed, no-FFI / no-MPI — *done*
- **op-v6s.4** — pure-Rust Newton-Krylov solver (PETSc replacement, keystone) — *implemented*
- **op-v6s.5** — structured Cartesian finite-volume grid — *implemented*
- **op-v6s.6** — input-deck I/O + gated HDF5 / output — *implemented* ~~*(HDF5 still deferred)*~~ **CORRECTED 2026-11-27**: `hdf5_io` exists (op-v6s.15.11, `std` only)
- **op-v6s.7** — fluid & material properties (EOS + characteristic curves) — *implemented*
- **op-v6s.8** — RICHARDS flow mode — first end-to-end solve — *implemented (verification-only)*
- **op-v6s.9** — V&V strategy + first RICHARDS benchmark (vs PFLOTRAN gold-files) — *partial: MMS + analytical verification done; validation vs published gold-files still open* (**CORRECTED 2026-11-27**: one gold file, `default/infiltration1d`, is now matched code-to-code, gh:#1537; validation against measurement is still open)
- **op-v6s.10 .. op-v6s.14** — TH, solute transport, reactive geochemistry, GENERAL multiphase, parallelism — ~~*not started*~~ **CORRECTED 2026-10-18:** TH, solute transport, reactive geochemistry and GENERAL multiphase are implemented as verification-only modules (see "What exists today"; module and `#[test]` counts checked, tests not re-run); ~~only parallelism is deferred~~ **CORRECTED 2026-11-27**: parallelism exists too (`decomposition`, `par`, `gpu`; `tests/mpi_sensitivity_gh1435.rs`)

## `no_std` + `alloc` (gh:#1603)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p outram-park-fork-pflotran --lib --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). The flow modes, transport, geochemistry,
EOS, solvers and deck parsing build without it. What `std` adds:

- `rayon`. Without it the per-cell loops run serially through `crate::par`;
  each is an ordered `map`/`collect` or writes only its own element, so the
  values are the same.
- `decomposition`, the MPI-style domain decomposition: `outram-park-mpi` runs
  its ranks as OS threads.
- `hdf5_io` (files) and the `gpu` module (`wgpu`).

With `std` on (the default) nothing changes.

## Design rules (workspace mandate)

- **Enum dispatch, no trait objects** — flow modes / EOS forms / solver kinds
  are enums matched exhaustively.
- **`uom` at API boundaries** — every physical quantity crossing a public
  boundary is a named `units` alias.
- **Pure Rust, Android-safe** — no PETSc, no MPI, no system BLAS, no C/Fortran
  toolchain in the library build.

## License

~~GPL-3.0-only~~ **AGPL-3.0-only** combined work (relicensed 2026-10-20, gh:#973; ported files keep GPL-3.0 and `LICENSE.GPL-3.0` ships beside `LICENSE`; see `docs/relicensing-agpl-2026-10-20.md`). See `LICENSE`, `NOTICE`, and the workspace `TRADEMARKS.md`.
