# V&V: secondary-distribution samplers against OpenMC with a fixed xi sequence (gh:#1386, prio-1 row 4)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI draft, unreviewed.

## Methodology

- **Upstream:** OpenMC `8f79d9d0` (MIT). `run.sh` compiles, with g++ 13.3
  `-O2 -ffp-contract=off`:
  - **whole files, unmodified:** `random_lcg.cpp`, `random_dist.cpp`,
    `distribution_energy.cpp`, `distribution_angle.cpp`,
    `secondary_uncorrelated.cpp`, `secondary_kalbach.cpp`,
    `secondary_correlated.cpp`, `secondary_nbody.cpp`;
  - **verbatim extracts** (`extract.sh`, commit and SHA-256 of every file
    checked; a moved function fails the script): `Distribution::sample`/
    `evaluate`, `Tabular` (ctor, `init`, `sample_unbiased`, `evaluate`) from
    `distribution.cpp`; `int2interp`, `Tabulated1D` (ctor, `operator()`) from
    `endf.cpp`; `exprel`, `log1prel`, `get_energy_index` from
    `math_functions.cpp` (that file also needs Faddeeva, which is not built).
- **Substitutions (all in `stubs/`):** HDF5 is replaced by an **in-memory tree**
  (`stubs/openmc/hdf5_interface.h`) implementing exactly the calls the
  constructors make, so upstream's REAL constructors run on tables the driver
  builds in `openmc.data`'s layout. `fmt`, `pugixml`, the cmake-generated
  `version.h` and `error.h` are stubs; none is reached on valid data.
- **Fixed xi sequence:** both codes draw from the same LCG `prn`, so a fixed
  seed IS a fixed xi sequence. Each family is sampled at 6 incident energies x
  40 seeds (4 x 40 for the energy laws); the driver prints `E'`, `mu` and the
  **final RNG state** (so an extra, missing or reordered draw is a difference)
  as hex bits, and prints every table it built, so the Rust test constructs the
  identical data in the port's types.
- **Families:** `random_dist` (Maxwell, Watt, normal variate),
  `ContinuousTabular` (histogram, lin-lin, lin-lin with two discrete lines),
  `MaxwellEnergy`, `Evaporation`, `WattEnergy`, `LevelInelastic`,
  `NBodyPhaseSpace` (3, 4, 5 bodies), `KalbachMann` (histogram, lin-lin),
  `CorrelatedAngleEnergy` (histogram, lin-lin), `UncorrelatedAngleEnergy`
  (tabulated angle, isotropic).
- **Port side:** `tests/secondary_samplers_vs_openmc_code_to_code.rs`.
- **Pass criterion:** bit identity of `E'`, `mu` and end state.

## Prediction (written before the run)

Bit-identical: `maxwell`, `watt`, `ContinuousTabular` `E'`, `Maxwell`,
`Evaporation`, `Watt`. Expected to differ: uncorrelated draw order (OpenMC draws
`mu` first), Kalbach-Mann cosine (2 or 3 variates upstream, 1 in the port),
correlated angle-bin choice, `normal_variate` (no port function existed) and
N-body phase space (no port).

## Not covered (stated, not hidden)

Elastic scattering (`elastic_scatter` is CM kinematics on a sampled cosine, covered
by `cm_to_lab_vs_kinematics.rs` and `elastic_mubar_vs_openmc.rs`, not a
fixed-xi differential), discrete-level inelastic given a fixed xi, and the
thermal S(alpha,beta) samplers (`secondary_thermal.cpp`, `thermal.cpp`): the
port builds those tables only from ACE, and OpenMC reads only its own HDF5, so
a shared-table fixed-xi comparison needs a table writer/reader pair that this
session did not build.

## Results

2026-11-11, rustc 1.99.1, g++ 13.3 (-O2, -ffp-contract=off).

**Before the fixes** (first run): `maxwell`/`evaporation` E' 120 of 160
bit-identical (worst 3.7e-16: `Tabulated1D` against NJOY `terp1`); `uncorrelated`
E' 0 of 160 (cosine/energy draw order); `KalbachMann` mu 0 of 240 and end state 0
of 240 (one-variate closed-form inverse against upstream's two-variate mixture).

**After** (`cargo test --release -p outram-mc-libs --test secondary_samplers_vs_openmc_code_to_code`, 7 passed, 1 ignored):

| family | samples | E' bit-identical | mu bit-identical | end state |
|---|---|---|---|---|
| `maxwell_spectrum`, `watt_spectrum`, `normal_variate` | 64 x 3 | all | - | all |
| `ContinuousTabular` histogram / lin-lin / lin-lin + 2 lines | 240 each | 240 each | - | (not compared) |
| `KalbachMann` histogram / lin-lin | 240 each | 240 each | 240 each | 240 each |
| `CorrelatedAngleEnergy` histogram / lin-lin | 240 each | 240 each | 240 each | 240 each |
| `UncorrelatedAngleEnergy`, isotropic, Maxwell / Evaporation / Watt | 160 / 160 / 120 | all | all | all |
| `UncorrelatedAngleEnergy`, tabulated angle, Maxwell | 160 | **155** | 160 | 160 |
| `NBodyPhaseSpace` 3/4/5 bodies | - | - | - | **no port** (gh:#1444) |

The 5 tabulated-angle E' that differ do so by 3.5e-16 relative (1 ulp). Cause,
checked rather than assumed (`maxwell_energy_differences_are_libm_roundoff`): the
Maxwell draw recomputed with `std` (glibc) `ln`/`cos` reproduces OpenMC's bits for
all 320 draws of both Maxwell arms; the port calls PETIR's `RealMath` `ln`/`cos`
(gh:#985), which differ from glibc in the last bit for a few percent of arguments.
Accepted under the code-to-code rule (< 1e-8, explained) and flagged for the
maintainer; `mu` and the end state are bit-identical.

**Defects fixed (translated from upstream), gh:#1444:** uncorrelated draw order;
`sample_mf4_mu_cm` now draws the neighbour-pick variate outside the grid too
(`AngleDistribution::sample` always does); `Tabulated1D::operator()` for LF=7/9/11
(`tabulated1d_eval`); Kalbach-Mann cosine as upstream's two-variate mixture
(`kalbach_mann_mu_openmc`; the isotropic ablation arm draws the same number of
variates). `normal_variate` ported (the port only had RAFFLES' Box-Muller).
Draw-order changes move random paths wherever these laws run; no k pin in the
suites run here moved (see the issue).
