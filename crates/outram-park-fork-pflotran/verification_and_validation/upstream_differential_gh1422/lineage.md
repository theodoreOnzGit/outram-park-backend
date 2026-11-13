# Lineage audit of `src/` against PFLOTRAN 669fbce (gh:#1422, closes bead op-2nz)

> **Research, education and V&V only.** AI-generated, not yet human-reviewed.

Upstream: PFLOTRAN, commit `669fbce204af39428020c6b9d74beae363376689`
(2026-09-09), LGPL-3.0. Clone used 2026-11-13: full history, sparse checkout of
`src/pflotran` and `regression_tests/default` (`git rev-parse HEAD` verified by
`build_and_run.sh` and `check_verbatim.sh`).

## Method

1. **Identifier overlap.** All 5242 upstream subroutine/function names of 9 or
   more characters were extracted from `src/pflotran/*.F90` and searched,
   whole-word and case-insensitively, in this crate's `src/`, `tests/` and
   `examples/`. Result in `lineage_identifier_overlap.txt`: the only
   non-generic hits are `RRadioactiveDecay`, `PMUFDDecaySolveISPDIAtCell`,
   `PMUFDDecaySetup`, `LUDecomposition1`, `LUDecomposition2` and
   `LUBackSubstitution`, all in `src/decay/upstream.rs` and its tests. (The
   other hits, `viscosity`, `Arrhenius`, `Initialise`, are generic English or
   physics words.)
2. **Bit-level differential** where a routine corresponds: characteristic
   curves, water EOS, isotherms, decay (gh:#1000, this folder's README).
3. **Reading both sides**: the module header of each port file states how it
   was derived; upstream's routine was located by domain noun.

**What this proves and what it does not.** Method 1 shows no port file other
than `decay/upstream.rs` reuses upstream identifiers or comments. It cannot
show that nobody read the Fortran. Method 2 shows, for four modules, that the
port is an independent implementation of the same published formula: the
operation order, parameter conventions and guards differ from upstream in ways
a line-by-line translation would not produce (examples in the table).

## Per-module findings

`T` = translation of an upstream routine (provenance header required); `I` =
independent implementation of a published model; `O` = original, no PFLOTRAN
counterpart; `?` = counterpart located, comparison not done.

| port module | upstream counterpart | class | evidence |
|---|---|---|---|
| `decay/upstream.rs`, `decay/upstream_tests.rs` | `pm_ufd_decay.F90` (`PMUFDDecaySolveISPDIAtCell`, `PMUFDDecaySetup`), `utility.F90` (LU), `reaction.F90` (`RRadioactiveDecay`) | **T** | cites file, routine and line; 165/165 3-generation and 115/115 implicit masses bit-identical to the extracted upstream (gh:#1000). **New 2026-11-13:** `pm_ufd_decay.F90` and `utility.F90` are byte-identical between the extraction commit `2af572b8` and the pin `669fbce` (`git diff` empty), so the record holds at the pin |
| `decay/mod.rs` (CRAM48 default) | none (upstream uses its own ODE/analytic paths) | **O** | CRAM is not in upstream; the three upstream algorithms are the `DecaySolver` variants above |
| `properties/curves.rs` van Genuchten, Brooks-Corey | `characteristic_curves_common.F90` `SFVG*`, `SFBC*`, `RPFMualemVGLiq*`, `RPFBurdineBCLiq*` | **I** | 255 VG and 650 BC saturation values bit-identical; differs in input convention (`n` vs `m`), structure (enum vs class hierarchy) and, before 2026-11-13, in two guards (below) |
| `properties/curves.rs` Haverkamp | none | **O** | `grep -il haverkamp src/pflotran/*.F90` finds no file |
| `properties/eos.rs` | `eos_water.F90` `EXPONENTIAL*`/`LINEAR` options | **I** | constant compressibility, no temperature dependence; not compared as a model (P11 shows the gap to the default IFC67) |
| `properties/thermal.rs`, `energy`, `thermal_convection` | `thc_eos_utils.F90`, `th_aux.F90`, `pm_th.F90` | `?` | headers cite no upstream; not compared |
| `eos_real` | `eos_water.F90` `EOSWaterDensityIF97`, `EnthalpyIF97`, `SaturationPressureIF97` | **I** | wraps `tampines-steam-tables`; density worst 4.6e-16, enthalpy 3.7e-16, psat 4.7e-15 relative |
| `eos_co2_brine` | `co2eos.F90`, `EOSWaterDensityBatzleAndWangExt` | `?` | brine uses Batzle and Wang, which upstream also offers; not compared |
| `activity` | `reaction.F90` activity coefficients, `reaction_aux.F90` constants | **I** | port constants 0.5085 / 0.3281 vs upstream 0.5114 / 0.3288 / 0.0410 (measured, README) |
| `pitzer` | none | **O** | `grep -il pitzer src/pflotran/*.F90` finds only `co2eos.F90`; PFLOTRAN has no Pitzer activity model at this commit |
| `sorption` (isotherms) | `reaction_isotherm.F90` `ReactionIsothermTotalSorbKD` | **I** | linear and Langmuir bit-identical; Freundlich exponent convention inverted (below) |
| `sorption` (Gaines-Thomas exchange), `surface_complexation` | `reaction_sorption`/`reaction_surf_complex.F90` | `?` | not compared |
| `kinetics`, `microbial`, `geochemistry` | `reaction_mineral.F90`, `reaction_microbial.F90`, `reaction.F90` | `?` | not compared |
| `reactive_transport` | `reactive_transport.F90`, `pm_rt.F90` | **I** | the port's header says operator-split (SNIA); upstream's default is global implicit (GIA) with an operator-split option, so even matched models take different steps |
| `flow/richards.rs`, `flow` | `pm_richards.F90`, `richards.F90`, `richards_common.F90` | **I** (one case) | ~~not compared; upstream 1-D case in `regression_tests/default/infiltration1d` needs `SATURATION_FUNCTION LINEAR`, which the port lacks~~ **CORRECTED 2026-11-13 (gh:#1537)**: `Linear` curves added and `infiltration1d` compared against its gold: 20/20 steps, 62/62 Newton iterations, all pressures within 5.4e-9 Pa (README, "Gold-file results"). Only that one homogeneous 1-D Dirichlet case; heterogeneous, gravity, Neumann and hydrostatic cases are not compared |
| `general_mode`, `multiphase` | `general.F90`, `general_aux.F90`, `pm_general.F90` | **I** | headers say "deliberately simplified rendering", air-water-energy, quadratic Corey gas rel-perm |
| `transport` | `transport.F90` | `?` | not compared |
| `wells` | `pm_well.F90`, `well_grid` | `?` | not compared |
| `grid`, `unstructured` | `grid_structured.F90`, `grid_unstructured*.F90` | `?` | not compared |
| `deck`, `io` | `input_aux.F90`, `pflotran_input` | **I** / **O** | `deck` parses a subset of public syntax (~~cannot read any `regression_tests/default` deck: REGION, FLOW_CONDITION, BOUNDARY_CONDITION unsupported~~ **CORRECTED 2026-11-13**: `deck::regression::parse_regression_deck` reads `default/infiltration1d/infiltration1d.in` completely, REGION/FLOW_CONDITION/BOUNDARY_CONDITION/INITIAL_CONDITION/STRATA/EOS WATER/OUTPUT included; the older `parse_pflotran_deck` still reads none of them; every other regression deck is untested); `io` is stated in its own header to be an invented format |
| `hdf5_io` | `hdf5_aux.F90`, `output_hdf5.F90` | **I** | pure-Rust `hdf5-pure`, not libhdf5 |
| `solver`, `decomposition` | PETSc SNES/KSP/DMDA | **O** | PETSc is replaced; `decomposition` runs over `outram-park-mpi` |
| `units`, `error`, `gpu` | none | **O** | |

## Conclusion on lineage

- **One translation:** `decay/upstream.rs` (already headed, now confirmed
  identical at the pin).
- **Four modules shown independent by measurement** (curves, EOS, isotherms,
  activity constants); **`pitzer`, Haverkamp, `io`, CRAM, `gpu` have no
  PFLOTRAN counterpart**; the rest are classed independent on their headers and
  the identifier search, with no differential yet (`?`).
- **No provenance header needs adding** to any file other than those already
  carrying one: the evidence does not show a translation anywhere else.
  `NOTICE` and `upstream_source/README.md` are corrected accordingly (the
  audit-open statement struck through, the finding recorded).
- This is evidence, not proof of clean-room authorship; a human reviewer
  should decide whether to claim it. That decision is **deferred** (gh:#1422).

## Port defects found by the differential and fixed by translating upstream

| # | defect | upstream | fix |
|---|---|---|---|
| D1 | Brooks-Corey slope at exactly `pc = 1/alpha` returned 0 (port tested `alpha*pc <= 1`); upstream returns `-lambda*alpha*(alpha pc)^(-lambda-1)` there (`SFBCSaturation`: `capillary_pressure < 1.d0/this%alpha` saturated, strict) | `characteristic_curves_common.F90` | `pc < 1/alpha` test in `effective_saturation` and `d_se_d_pc`; doc strike-through; test `bc_slope_at_exact_air_entry_matches_upstream_gh1422`. Found as relative difference 1 at set 1, `pc = 1e4` |
| D2 | van Genuchten had no near-saturation guard; upstream `SFVGSaturation` returns exactly `Sl = 1`, slope 0 when `(alpha pc)^n < 1e-15`. The port returned `1 - m x` and a slope of order 1e-12 | same file | `VG_PC_ALPHA_N_EPSILON` guard in both functions; test `vg_near_saturation_guard_matches_upstream_gh1422`; 65 + 13 VG guard points now agree exactly |
| D3 | Freundlich convention inverted relative to PFLOTRAN's `FREUNDLICH_N` | `reaction_isotherm.F90` | not a bug in the port's own model (documented as an exponent); added `new_freundlich_pflotran` (extension, no existing signature changed) and a doc warning |

## Deferred for the maintainer (not decided autonomously)

1. Whether `ActivityModel` should offer PFLOTRAN's constants
   (0.5114 / 0.3288 / 0.0410) as a named variant. Both sets are legitimate 25 C
   values; choosing one is a modelling decision.
2. Whether to claim in `NOTICE` that the non-`decay` modules are independent
   works (this audit supports it for the compared modules only).
3. The pinned `upstream_source/PFLOTRAN` shallow clone cannot reach
   `2af572b8`; this audit used a full-history blobless clone. Whether to
   vendor that instead.
