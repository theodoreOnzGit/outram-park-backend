# outram-park-fork-offbeat

An **independent, pure-Rust fork / translation** of
**[OFFBEAT](https://gitlab.com/foam-for-nuclear/offbeat)** — the OpenFOAM Fuel
BEhaviour Analysis Tool, a multi-dimensional finite-volume **nuclear
fuel-performance** solver — rebuilt on the OUTRAM-FOAM finite-volume substrate
in `outram-foam-basic-lib` and to OUTRAM PARK's design rules: enum dispatch (no
trait objects), no `Box<T>`, no lifetime parameters, `uom`-typed API boundaries,
and an Android/Termux-buildable library.

> **Independent fork, not the official OFFBEAT.** This crate is not affiliated
> with, endorsed by, or maintained by the OFFBEAT developers, the
> `foam-for-nuclear` project, EPFL, the Paul Scherrer Institute, Texas A&M
> University, or the OpenFOAM Foundation / OpenCFD. "OFFBEAT" and "OpenFOAM" are
> used only to identify the upstream work this crate derives from. See `NOTICE`
> and the workspace `TRADEMARKS.md`.
>
> **License: GPL-3.0-only.** OFFBEAT upstream is **also GPL-3.0**, so no
> relicensing step applies — unlike some sibling fork crates, there is no
> license-compatibility caveat here. The upstream LICENSE file was read directly
> from the GitLab repository on 2026-07-29. Ported from upstream commit
> `80e84450a115b0c411e1bfa5d166379f6bf6c084` (2026-01-05).
>
> **SCAFFOLD / EARLY: the mechanics layer is VERIFICATION-ONLY and there is no
> human V&V.** 361 unit tests pass and the small-strain mechanics solver matches
> closed-form linear-elasticity solutions, but **nothing has been validated** —
> not against OFFBEAT output, not against the upstream `Cases/Verification`
> suite, not against fuel-irradiation data — and no human has reviewed any of
> it. Use at your own risk. Not for nuclear facility operation, reactor control,
> fuel licensing decisions, or safety-critical analysis — education, research,
> and V&V only. See `RESPONSIBLE_USE.md`.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## What a fuel-performance code computes

Given a fuel rod and its irradiation history — linear power against time,
coolant conditions, fast-neutron flux — it predicts the **thermal and mechanical
state of the fuel through life**: temperature distribution, fuel and cladding
deformation, closure of the fuel/cladding gap, cladding stress, released
fission-gas pressure, and ultimately whether the cladding fails.

The physics is a strongly coupled loop. Power deposits heat; heat sets the
temperature field; temperature drives thermal expansion, creep and swelling;
those deformations close the gap; gap closure changes the gap conductance, which
changes the temperature field again.

## What exists today

Measured on 2026-08-05: 63 source files, 55,716 lines, **520 passing tests**
(plus 70 doc tests), no `todo!()` or `unimplemented!()` anywhere. All statuses
below are **verification-only** — see the status warning above.

| Module | LOC | Tests | Content |
|---|---|---|---|
| `mechanics` | 2,515 | 14 | Small-strain quasi-static solver — displacement field, linear-elastic constitutive law, eigenstrain loading, momentum assembly on foam's `ldu_matrix`/`krylov`, and the **creep/plasticity coupling** that drives `rheology` (mechanical-strain subtraction, inelastic strain fed back as an additional eigenstrain, once-per-step state advance, creep timestep control) |
| `rheology` | 27,429 | 187 | Constitutive laws — plasticity (yield stress, hardening), creep, per-material law selection; **includes the `rheology::aster` port below** |
| ↳ `rheology::aster` | 23,733 | 159 | Port of code_aster's constitutive-law layer (EDF, GPL-3.0-or-later) — see the table below |
| `materials` | 13,430 | 189 | Property correlations (conductivity, density, heat capacity, thermal expansion, Young's modulus, Poisson ratio, emissivity) and behavioural models (swelling, densification, relocation, phase transition, failure) |
| `gap` | 5,477 | 68 | Fuel/cladding gap — conductance, gas mixture properties, free volume, contact, axial slice mapping |
| `corrosion` | 5,044 | 44 | Cladding corrosion kinetics, hydrogen pickup, thermal feedback, Anderson-mixing acceleration |
| `burnup` | 1,437 | 12 | Burnup accumulation and fast-neutron flux |
| `fgr` | 1,553 | 12 | Fission-gas release models and OFFBEAT's SCIANTIX coupling shim |
| `error` | — | — | `OffbeatError` enum; unfinished paths return `NotImplemented` |

### The code_aster port (`rheology::aster`)

code_aster is EDF's nonlinear structural solver, built to justify the integrity
and remaining life of its own reactor fleet — so its constitutive laws are the
*nuclear* ones (irradiation creep, Zircaloy anisotropy, vessel steels) rather
than generic mechanical-engineering fare. Ported from a read-only clone of
upstream's `src` repository at commit `b504ea08`; never vendored. Tracked as
epic `op-a7p`.

| Module | Tests | Laws |
|---|---|---|
| `catalogue` | — | The 229 behaviours upstream declares, generated from its Python catalogue |
| `kinematics` | — | Mandel convention (`√2` on shears) and the deformation gradient |
| `integration` | — | The scalar local solvers every law shares — Newton, secant, Brent |
| `log_strain` | — | The `GDEF_LOG` finite-strain wrapper |
| `viscoplastic` | 17 | `NORTON`, `LEMAITRE`, `LEMAITRE_IRRA` |
| `isotropic` | 16 | `VMIS_ISOT_LINE`/`_PUIS` hardening, `NORTON_HOFF` |
| `chaboche` | 19 | `VMIS_CIN1/2_CHAB`, `VISC_CIN1/2_CHAB`, `VMIS/VISC_CIN2_MEMO` |
| `damage` | 29 | `VENDOCHAB`, `VISC_ENDO_LEMA`, `ROUSS_PR`, `ROUSS_VISC`, `GTN`, `VISC_GTN`, `CRIT_RUPT` |
| `metallurgy` | 25 | `VISC_IRRA_LOG`, `GRAN_IRRA_LOG`, `IRRAD3M`, `META_LEMA_ANI` |
| `fracture` | 24 | Linear-elastic fracture post-processing only — see the limitation below |

**Two limitations that change results**, documented here so they are not
discovered late:

- **`fracture` is roughly 80 % blocked.** The G-theta domain integral needs
  element shape functions, Gauss quadrature and crack-front ring topology, none
  of which this crate has. What is implemented is the closed-form subset —
  Irwin mode split, Westergaard near-tip fields, kink-angle criteria, and the
  front-smoothing bases.
- **`damage`'s `GTN` is the local form only.** Without `GRADVARI` nonlocal
  regularisation, a structural run will localise into a single element band and
  give mesh-dependent answers.

Porting the upstream laws also surfaced several apparent defects in them. These
are **reproduced and documented, never silently corrected** — each has a test
that pins the discrepancy so an upstream fix breaks loudly. See the module docs
for the full list; the sharpest is `nmvpir.F90`'s growth tensor, whose `yy`
component makes the tensor identically zero for growth along `y`.

**Not present yet**, and needed before this is a fuel-performance solver rather
than a library of fuel-performance pieces:

- **A thermal sub-solver.** The heat-conduction equation on the fuel/gap/cladding
  stack. The correlations that feed it exist; the solve does not.
- **Multi-region coupling** — the outer fuel ↔ gap ↔ cladding iteration that
  closes the loop described above.
- **Validation of any kind.** The upstream `Cases/Verification` and
  `Cases/testCases` suites (~1500 entries) are the intended oracle and none of
  them has been run against this port.

## Units

Public constructors and results are typed with `uom` where a caller supplies or
consumes a physical quantity. The inner numerical loops carry **raw `f64` in
strict SI** — kelvin, pascal, metre, and MWd/kgHM for burnup unless an item
documents otherwise — because the correlations are dense and per-cell, and `uom`
round-trips inside a cell loop cost more than they buy. Every raw-`f64` boundary
says so in its doc comment.

## Verification status, honestly

The mechanics solver is checked against closed-form elasticity in
`src/mechanics/solver/tests.rs`, and each test records **methodology, pass
criterion, and the measured result** per the workspace V&V rule. For example,
`fully_constrained_eigenstrain_gives_hydrostatic_compression` asserts
`σ = −3K ε*` and measured `σ_xx = −1.500 GPa` against the closed-form
`−500 GPa × 3e-3 = −1.500 GPa`, agreeing to better than `1e-10` relative
(`E = 200 GPa`, `ν = 0.3`, measured 2026-07-29).

The rheology-coupled solve is checked in `src/mechanics/solver/rheology_tests.rs`
against the closed-form relaxation of a linear viscoelastic solid: a bar held at
a fixed strain of `1e-4` relaxes its von Mises stress as `q(t) = q_0 e^{-t/tau}`
with `tau = 1040.00 s`, and the solver reproduces `q(tau)` to `6.25e-4` relative
at 800 steps, with the error falling 4x per 4x step reduction (measured
2026-08-05). A freely expanding body with an aggressive creep law present
carries `1.37e-6 Pa` of residual stress and accumulates `5.49e-18` of creep
strain against a `3e-3` eigenstrain — i.e. none.

That is **verification** in this workspace's sense — "is the equation
implemented correctly?" It is **not validation** — "does it represent physical
reality well enough for its intended purpose?" No test in this crate may be
cited as evidence that the port reproduces experiment or reproduces OFFBEAT.
See `VERIFICATION_AND_VALIDATION.md`.

## Android / Termux

Buildable for `aarch64-linux-android`. Dependencies are `uom`, `thiserror`, and
`outram-foam-basic-lib` (itself pure Rust, no system BLAS), with `approx` as the
only dev-dependency — nothing needing a C/Fortran toolchain or system
BLAS/LAPACK. Verified 2026-07-30 with:

```bash
cargo check -p outram-park-fork-offbeat --all-targets --target aarch64-linux-android
```

The authoritative check remains a native build inside Termux on-device.

## Build and test

```bash
cargo build -p outram-park-fork-offbeat --release
cargo test  -p outram-park-fork-offbeat --release --lib
```

## Provenance

The upstream tree is **not** vendored here — porting is done from a read-only
clone kept outside this working tree, and no OFFBEAT or OpenFOAM C++ source is
copied into the repository. Every derived Rust file carries an attribution
header naming its upstream source file, project, commit, copyright holder and
license. Full detail, including the SCIANTIX license lineage, is in `NOTICE`.

Porting plan and module inventory: `docs/offbeat-port-scoping.md` at the
workspace root.
