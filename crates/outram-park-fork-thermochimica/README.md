# outram-park-fork-thermochimica

Pure-Rust molten-salt thermochemistry — an independent translation of ORNL's BSD-3-licensed [Thermochimica](https://github.com/ORNL-CEES/thermochimica).

Gibbs-energy-minimisation (CALPHAD) thermochemistry for molten fluoride salts: fission-product speciation, oxidation-state / redox (UF3/UF4) partitioning, and solubility — ~~the chemistry engine for MSRE fission-product migration~~ **CORRECTED 2026-11-27** — intended as the chemistry engine for MSRE fission-product migration; today it is the Gibbs-energy-minimisation core only (`src/gem.rs`, ideal and binary Redlich-Kister mixing, no `.dat` parser), and no other crate depends on it (`AGENTS.md`, checked 2026-10-20). Sibling to `outram-park-fork-dwsim-libs`'s `thermo::gibbs_multiphase`. Molten-fluoride thermodynamic data (e.g. MSTDB-TC) is supplied separately with documented provenance.

**Name.** `outram-park-fork-thermochimica` is an OUTRAM PARK fork, named for
its upstream, Thermochimica; the backronym is OUTRAM PARK's own, letter by
letter: **O**pen-source **U**nified **TRA**nsient **M**ulti-Physics
**A**dvanced **R**eactor simulation **K**it.

**The question it answers:** given species, their standard Gibbs energies and
an element feed at fixed temperature and pressure, which phases and species
are present at equilibrium, in what amounts?

**What this crate makes studyable and challengeable.** The element-potential
Gibbs-energy minimiser that Thermochimica uses, in one Rust file whose header
cites each Fortran routine it follows by `File.f90:line` (`GEMSolver.f90`,
`GEMNewton.f90`, `GEMLineSearch.f90`, ...), with the Redlich-Kister excess
term written out. Its differential against Thermochimica run from source is a
worked case of a comparison exposing a defect before any number could be
compared: the port solved 0 of 8 ideal cases until its line-search merit was
made offset-free, and the record names the first divergent operation. It is a
small, readable solver to study and challenge, not a thermochemistry
authority.

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`).

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Upstream: Thermochimica (ORNL-CEES, M.H.A. Piro et al., BSD-3-Clause), commit `0c35c8d`, provenance in the `src/gem.rs` header, `upstream_source/README.md` and `LICENSE.thermochimica`. In `crates/kovan-literature/CATALOGUE.md`: `robertson1965msre` (ORNL-TM-728, MSRE design), listed for the MSRE group this crate belongs to. |
| Mathematical equations | Module doc of `src/gem.rs`: minimisation of total Gibbs energy under element mass balance, species chemical potential `mu = mu0 + RT ln(x gamma)`, the element-potential saddle-point Newton system, the binary Redlich-Kister-Muggianu excess term `g_ex = x1 x2 sum_v L_v (x1 - x2)^v`, and the line-search merit. |
| Source code | `src/gem.rs` (the whole port: `GemSystem`, `SolutionModel`, `GemSystem::minimize`, `solve_linear`); `src/lib.rs` only re-exports. |
| Interactive simulations | None yet. |
| Visualisations | None yet. |
| Verification and validation evidence | Seven unit tests in `src/gem.rs` against closed-form results (pure-component limit, ideal Nernst partition, Redlich-Kister identity, LiF-BeF2 mass and charge balance). `verification_and_validation/thermochimica_differential/RESULTS.md` (2026-11-13, gh:#1407) against Thermochimica `0c35c8d` built with gfortran: 0 -> 7 of 8 ideal cases solved after the merit fix; against upstream tightened to 1e-10, worst `pi` 1.9e-10 on the eight tabulated cases except H2 at 1.8e-8 (rung 1); 1 of 50 solved quantities bit-identical; ternary and quaternary Redlich-Kister cases (W2 to W4) not supported. Pinned by `tests/thermochimica_differential.rs`. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp. No entry in `docs/human-corrections-to-ai-work.md`. `AGENTS.md` strikes through the unit-test count (6 corrected to 7, gh:#1407). |

**Status:** the GEM core has machine-run evidence against closed-form results
and against Thermochimica on gas, condensed and binary-RKMP cases; none of it
is human-reviewed, no molten-fluoride case has been compared with upstream or
with measurement, and most of Thermochimica (the data parser, sublattice and
quasichemical models, phase-assemblage management) is not ported.

> **⚠️ Scaffold — no human V&V.** Port in progress under the MSRE digital-twin
> epic (`op-6w0`). Independent OUTRAM PARK fork; not affiliated with the
> upstream project. Not for nuclear facility operation, reactor control,
> safety-critical, or licensing decisions.

## `no_std` + `alloc` (gh:#1590)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p outram-park-fork-thermochimica --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). `std` only turns on `thiserror`'s `std`. With `std` on (the default)
nothing changes.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
