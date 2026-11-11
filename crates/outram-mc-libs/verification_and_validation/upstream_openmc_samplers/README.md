# V&V: secondary distributions, S(alpha,beta) laws and collision kinematics against OpenMC with a fixed xi sequence (gh:#1386 prio-1 row 4, gh:#1444)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI draft, unreviewed.

## Methodology

- **Upstream:** OpenMC `8f79d9d0` (MIT). `run.sh` compiles, with g++ 13.3
  `-O2 -ffp-contract=off`:
  - **whole files, unmodified:** `random_lcg.cpp`, `random_dist.cpp`,
    `position.cpp`, `distribution_energy.cpp`, `distribution_angle.cpp`,
    `secondary_uncorrelated.cpp`, `secondary_kalbach.cpp`,
    `secondary_correlated.cpp`, `secondary_nbody.cpp`,
    **`secondary_thermal.cpp`** (added 2026-11-11);
  - **verbatim extracts** (`extract.sh`, commit and SHA-256 of every file
    checked; a moved function fails the script): `Distribution::sample`/
    `evaluate`, `Tabular` (ctor, `init`, `sample_unbiased`, `evaluate`) from
    `distribution.cpp`; `int2interp`, `Tabulated1D` (ctor, `operator()`),
    **`CoherentElasticXS` (ctor, `operator()`)** from `endf.cpp`; `exprel`,
    `log1prel`, `get_energy_index` from `math_functions.cpp` (that file also
    needs Faddeeva, which is not built); and, new, the **collision
    kinematics** (`physics_extract.inc`): `rotate_angle` (`math_functions.cpp:772`),
    `elastic_scatter` (`physics.cpp:792`), `sample_target_velocity` (`:874`),
    `sample_cxs_target_velocity` (`:1028`), `Nuclide::elastic_xs_0K`
    (`nuclide.cpp:589`), the 0 K CDF build (`nuclide.cpp:477-494`) and the CM to
    lab block of `inelastic_scatter` (`physics.cpp:1169-1179`).
- **Substitutions (all in `stubs/`):** HDF5 is replaced by an **in-memory tree**
  (`stubs/openmc/hdf5_interface.h`) implementing exactly the calls the
  constructors make, so upstream's REAL constructors run on tables the driver
  builds in `openmc.data`'s layout. `fmt`, `pugixml`, the cmake-generated
  `version.h` and `error.h` are stubs; none is reached on valid data.
  `stubs/kin_stubs.h` declares exactly the members of `Particle`, `Nuclide`,
  `Reaction` and `settings::` (upstream's defaults, `settings.cpp:125-134`) that
  the extracted kinematics read.
- **Fixed xi sequence:** both codes draw from the same LCG `prn`, so a fixed
  seed IS a fixed xi sequence. Each family is sampled at 6 incident energies x
  40 seeds (4 x 40 for the energy laws, 7 x 40 for the thermal laws); the driver
  prints `E'`, `mu` and the **final RNG state** (so an extra, missing or
  reordered draw is a difference) as hex bits, and prints every table it built,
  so the Rust test constructs the identical data in the port's types.
- **Families:** `random_dist` (Maxwell, Watt, normal variate),
  `ContinuousTabular` (histogram, lin-lin, lin-lin with two discrete lines),
  `MaxwellEnergy`, `Evaporation`, `WattEnergy`, `LevelInelastic`,
  `NBodyPhaseSpace` (3, 4, 5 bodies), `KalbachMann` and `CorrelatedAngleEnergy`
  (histogram and lin-lin, **each also with two discrete lines**, the case OpenMC
  #4069 changed), `UncorrelatedAngleEnergy` (tabulated angle, isotropic);
  **thermal:** `CoherentElasticAE`, `IncoherentElasticAEDiscrete`,
  `IncoherentInelasticAEDiscrete` (equiprobable and skewed bins),
  `IncoherentInelasticAE` (continuous); **kinematics:** `rotate_angle`, the
  inelastic CM to lab block, a discrete-level inelastic collision end to end,
  `elastic_scatter` with target at rest, free gas (CXS) and the resonance window
  (RVS and DBRC) for a resonant nuclide (synthetic 0 K table with three
  resonances), hydrogen and carbon.
- **Port side:** `tests/secondary_samplers_vs_openmc_code_to_code.rs` (samplers
  and thermal) and `tests/collision_kinematics_vs_openmc_code_to_code.rs`.
- **Pass criterion:** bit identity of `E'`, `mu`, the direction and the end
  state, except where a difference is explained and its cause is itself
  measured (libm last bit, below).

## Prediction (written before the run)

Samplers (2026-11-11, first run): bit-identical `maxwell`, `watt`,
`ContinuousTabular` `E'`, `Maxwell`, `Evaporation`, `Watt`. Expected to differ:
uncorrelated draw order, Kalbach-Mann cosine, correlated angle-bin choice,
`normal_variate` and N-body (no port).

Thermal and kinematics (written before the run of 2026-11-11): the thermal laws
were ported against `secondary_thermal.cpp` in an earlier audit (gh:#365, #407)
with the same `get_energy_index` and smearing, so **predicted bit-identical**
for all five, the continuous law up to `ln`/`sqrt` last bits. For the kinematics:
`rotate_angle` bit-identical off the poles and **different at the poles** (the
port's pole branch is another parametrisation); the CM to lab block and the level
collision **different in the last bit** (another operation order, and
`two_body_scatter_with_mu` forms `E_cm` as `E (A/(A+1))^2 + Q A/(A+1)` where
OpenMC forms `mass_ratio (E - threshold)`); target-at-rest elastic **different in
the last bit** (OpenMC adds velocities, the port uses the closed form); free-gas
and resonance elastic **a different number of draws** (the port draws the CM
cosine before the target velocity, OpenMC after) so different samples of the same
distribution.

## Not covered (stated, not hidden)

- The analytic `IncoherentElasticAE` (the ENDF-HDF5 form, `exp` of a Debye-Waller
  factor): the port holds the discrete-cosine form NJOY/ACER writes for every
  incoherent elastic law, which is OpenMC's `IncoherentElasticAEDiscrete`
  (compared above). `MixedElasticAE`'s choice between its two parts is one
  `prn` against `sigma_coh / (sigma_coh + sigma_incoh)` in both codes and was not
  isolated.
- The sampling of `mu` for elastic scattering from a tabulated MF=4 law inside
  `elastic_scatter` (the isotropic `uniform_distribution` arm only is run; the
  tabulated arm is `AngleDistribution::sample`, compared through
  `UncorrelatedAngleEnergy` above).
- URR probability-table scattering (`use_ptable`) in `elastic_scatter`: covered by
  `urr_sampling_vs_openmc.rs` for the table rows, not as a kinematics case.

## Results

2026-11-11, rustc 1.99.1, g++ 13.3 (-O2, -ffp-contract=off), Intel Xeon Processor @ 2.80 GHz
(cloud container, 4 vCPUs, 15 GB, Linux 6.18, CPU only; no timings are recorded here).

### Samplers (`cargo test --release -p outram-mc-libs --test secondary_samplers_vs_openmc_code_to_code`: 14 passed)

| family | samples | E' bit-identical | mu bit-identical | end state |
|---|---|---|---|---|
| `maxwell_spectrum`, `watt_spectrum`, `normal_variate` | 64 x 3 | all | - | all |
| `ContinuousTabular` histogram / lin-lin / lin-lin + 2 lines | 240 each | 240 each | - | (not compared) |
| `KalbachMann` histogram / lin-lin | 240 each | 240 each | 240 each | 240 each |
| `KalbachMann` + 2 discrete lines, histogram / lin-lin (#4069) | 240 each | 240 each | 240 each | 240 each |
| `CorrelatedAngleEnergy` histogram / lin-lin | 240 each | 240 each | 240 each | 240 each |
| `CorrelatedAngleEnergy` + 2 discrete lines, histogram / lin-lin (#4069) | 240 each | 240 each | 240 each | 240 each |
| `UncorrelatedAngleEnergy`, isotropic, Maxwell / Evaporation / Watt | 160 / 160 / 120 | all | all | all |
| `UncorrelatedAngleEnergy`, tabulated angle, Maxwell | 160 | **155** | 160 | 160 |
| `NBodyPhaseSpace` 3 / 4 / 5 bodies (**ported 2026-11-11**, `physics::nbody`) | 120 each | **117** / 120 / 120 | 120 each | 120 each |
| `CoherentElasticAE` | 240 | 240 | 240 | 240 |
| `IncoherentElasticAEDiscrete` | 240 | 240 | 240 | 240 |
| `IncoherentInelasticAEDiscrete`, equiprobable | 280 | 280 | 280 | 280 |
| `IncoherentInelasticAEDiscrete`, skewed | 280 | 280 | 280 | 280 |
| `IncoherentInelasticAE` (continuous) | 280 | 280 | 280 | 280 |

The 5 tabulated-angle and the 3 N-body-3 `E'` that differ do so by 2e-16 to
3.5e-16 relative (1 ulp). Cause, checked rather than assumed: the Maxwell draw
(`maxwell_energy_differences_are_libm_roundoff`) and the whole N-body algorithm
(`nbody_differences_are_libm_roundoff`, **360 of 360 bit-identical**) recomputed
with `std` (glibc) `ln`/`cos` reproduce OpenMC's bits; the port calls PETIR's
`RealMath` (gh:#985), which differs from glibc in the last bit for a few percent
of arguments. Accepted under the code-to-code rule (< 1e-8, explained) and flagged
for the maintainer; `mu` and the end state are bit-identical.

**The thermal laws needed no fix**: all five were bit-identical on the first run
(the only change in the port is the new public constructor
`ThermalScattering::from_ace_thermal`, the second half of `from_ace`, so a test
can hand the sampler the arrays an OpenMC `ThermalData` is built from).

### Collision kinematics (`--test collision_kinematics_vs_openmc_code_to_code`: 8 passed)

| routine | port function | cases | bit-identical |
|---|---|---|---|
| `rotate_angle` | **`rotate_angle_openmc`** (new, translation) | 120 (40 at the poles) | **120**; end state 120 |
| | `rotate_direction` (older, used by transport) | 120 | 80; **0 of 40 at the poles** (different pole parametrisation, valid but not upstream's); end state 120 |
| CM to lab block of `inelastic_scatter` | **`inelastic_cm_to_lab_openmc`** (new) | 180 | **180** |
| | `cm_to_lab` (older) | 180 | E' 128, mu 153; worst 4.4e-15 relative (operation order) |
| discrete level, end to end | **`level_inelastic_scatter_openmc`** (new) | 156 | **156** (E', direction, end state) |
| | `two_body_scatter_with_mu` (older) | 156 | E' 84, direction 122, end state 156; worst 6.0e-16 |
| `elastic_scatter`, target at rest | **`elastic_scatter_openmc`** (new) | 192 | **192** |
| `elastic_scatter`, free gas (CXS) | `elastic_scatter_openmc` | 576 | 488; **end state 576**; worst 5.3e-16 |
| `elastic_scatter`, resonance window (RVS, DBRC) | `elastic_scatter_openmc` | 512 | 440; **end state 512**; worst 3.8e-16 |
| elastic, any regime | `free_gas_elastic_scatter_dbrc` (older, cosine drawn first) | 1280 | at rest: E' 58 of 192, direction 4, end state 192; free gas: **0** of 576 (end state 400); resonance: **0** of 512 (end state 348) |

In the free-gas and resonance rows the new function spends the same draws in the
same order as OpenMC in every collision (end state 1280 of 1280) and `E'` agrees
within 5.3e-16; the remaining non-identical `E'` are the same libm last bit (the
paths call `ln`/`cos` several times per sample, so the fraction is larger than for
a single draw). That identification is by the draw count and the size, and
by the measured cause of the same two functions above; no per-row `std`
recomputation of the elastic kernel was built.

**Port differences found, and what was done (gh:#1444):**

1. **N-body phase space had no port**: `physics::nbody::NBodyPhaseSpace`
   translated (`secondary_nbody.cpp`). The transport route for ACE LAW=66 still
   uses the tabulated shape from `ace_phase_space_chi` (NJOY ACER's own table,
   the same density, one draw per sample instead of several), so the new sampler
   is differential-tested but not on the transport path.
2. **`rotate_direction`'s pole branch** is a different parametrisation from
   `rotate_angle`'s (0 of 40 identical at `u = (0,0,+-1)` and near it). Same
   distribution (azimuth uniform), another direction for one azimuth.
3. **The CM to lab transform and `E_cm` of a level** are algebraically
   identical, different in the last bit (up to 4.4e-15 relative).
4. **Free-gas and resonance elastic draw the CM cosine before the target
   velocity** at every call site; OpenMC draws it after (inside
   `elastic_scatter`), so the two codes follow different random paths for the
   same seed in those regimes. Same distribution.
5. **Target-at-rest elastic**: OpenMC adds velocities (`v_cm`, `v_n - v_cm`,
   rotate, add back); the port uses the closed form. Last-bit differences only.

Fix: for each, the upstream routine is translated beside the older one as
`rotate_angle_openmc`, `inelastic_cm_to_lab_openmc`, `level_inelastic_e_cm_openmc`
/ `level_inelastic_scatter_openmc` and `elastic_scatter_openmc` (with
`sample_target_velocity`, `sample_cxs_target_velocity`, upstream's
`lower_bound_index` and `elastic_xs_0K` semantics, including its zeroing of
negative 0 K cross sections for every point but the last) in
`physics/scatter.rs`. **The transport loops still call the older kernels**: the
maintainer's rule that "the port is the oracle: speed work goes beside it, never
into it" and the ripple (every recorded k-effective and bit-identity pin with a
scatter in it would move, in the last bit and then chaotically) are why they were
not swapped; the new functions are the named, differential-tested alternative a
track differential needs. Whether to make them the default is a maintainer
decision (it re-pins most of the suite). Draw-order changes move random paths
wherever these laws run.

**Defects fixed earlier (translated from upstream), gh:#1444:** uncorrelated
draw order; `sample_mf4_mu_cm` now draws the neighbour-pick variate outside the
grid too (`AngleDistribution::sample` always does); `Tabulated1D::operator()` for
LF=7/9/11 (`tabulated1d_eval`); Kalbach-Mann cosine as upstream's two-variate
mixture (`kalbach_mann_mu_openmc`; the isotropic ablation arm draws the same
number of variates). `normal_variate` ported (the port only had RAFFLES'
Box-Muller).

## Fission-neutron birth (`sample_fission_neutron`, gh:#1444 d), added 2026-11-11

- **Upstream:** `physics.cpp:1081` and `Nuclide::nu` (`nuclide.cpp:532`) and
  `rotate_angle`, verbatim via `extract.sh` into `generated/fission_extract.inc`
  (SHA-checked); `fission_driver.cpp` + `stubs/fission_stubs.h` supply the
  `Particle`/`SourceSite`/`Nuclide`/`Reaction`/`ReactionProduct` members they read;
  real `UncorrelatedAngleEnergy`, `Tabulated1D`, LCG. `run.sh` writes
  `reference_fission.txt`.
- **Data:** synthetic (nu_total, inflated nu_d so beta ~ 0.12, 3 groups with yield
  nu_d*p_k, prompt Watt, delayed Maxwell with per-group theta, `energy_max` 1.5 MeV so
  resampling fires). Incident energies start at 3e4 eV: upstream's Maxwell/Watt loop
  `while E' > E_in - u` never ends for E_in < u (upstream oddity, not copied).
- **Prediction (before the run):** group and final RNG state bit-identical; E' and
  direction bit-identical except last-bit libm.
- **Result:** `tests/fission_neutron_vs_openmc_code_to_code.rs`, 2160 births (226
  delayed): group 2160/2160, RNG end state 2160/2160, E' 2137/2160 bit-identical (worst
  6.5e-16 rel), direction 2034/2160 (worst 3.3e-16 abs). No port defect found; the
  port's draw order, group selection (`nu_d*p_k` vs tabulated yields) and resampling
  match. Direction last-bit cause is inferred, not A/B-tested.


## Source distributions (gh:#1388, added 2026-11-11)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI draft, unreviewed.

**Port:** `src/source/distribution.rs` (`Distribution` enum: Discrete with Vose alias,
Uniform, PowerLaw, Maxwell, Watt, Normal incl. truncation, Tabular x4 interpolations,
Mixture, bias weights; `SpatialDistribution` enum: Cartesian, Cylindrical, Spherical,
PointCloud, Box, Point; `Legendre` evaluation only, because upstream has no sampler).

**Methodology:** `extract_distributions.sh` (SHA-256 of every file, verbatim line ranges
of `distribution.cpp` 26-772, `distribution_spatial.cpp` 43-257 and 376-492,
`math_functions.cpp` 100-105) and `run_distributions.sh` compile OpenMC's real code with
g++ 13.3 `-O2 -ffp-contract=off` against `stubs_dist/` (an in-memory XML tree,
`xml_interface.h` with the same semantics, `mesh.h` declarations). Upstream's own XML
constructors and `distribution_from_xml` run. `distributions_driver.cpp` prints value,
weight and end RNG state of 40 seeds per case as hex (`reference_distributions.txt`,
1611 lines), plus `evaluate` and `integral()`. `DecaySpectrum` is stubbed (needs the
depletion chain, gh:#975). Test: `tests/source_distributions_vs_openmc_code_to_code.rs`.

**Prediction (before the run):** libm-free families bit-identical; `PowerLaw`, `Normal`,
`Maxwell`/`Watt`, log Tabular, sin/cos spatial and truncated-normal `erfc` possibly 1 ulp
in a few percent; every end state identical.

**Results (2026-11-11, rustc 1.99.1, g++ 13.3; Intel Xeon @ 2.80 GHz cloud container,
4 vCPUs shared, CPU only; no timings):** 8 tests pass.

| family | cases x seeds | value bit-identical | weight | end state |
|---|---|---|---|---|
| 16 univariate (uniform, powerlaw x2, maxwell, watt, normal x3, discrete x3, tabular x5) | 16 x 40 | **640 / 640** | all 1 | **640 / 640** |
| `evaluate` (PDF), 13 distributions, 6-12 points | 150 | **150 / 150** | - | - |
| `integral()` (incl. tabular, discrete, nested mixture) | 18 | **18 / 18** | - | - |
| Mixture flat / nested / biased, biased Discrete / Uniform / Maxwell | 6 x 40 | 240 / 240 (worst < 1e-14 where libm) | within 1e-12 | 240 / 240 |
| Cartesian x3, Box, Point, PointCloud x3, spherical default | 40 each | all coordinates | all | all |
| Cylindrical default / tilted / diagonal / biased | 120 coords each | **116 / 119 / 117 / 119** | all | all |
| Spherical independent | 120 coords | **118** | all | all |

All libm-dependent families (pow, ln, exp, expm1, log1p, erfc) came out bit-identical
on these 40 seeds (a coincidence of the draws, not a guarantee; the first run could have
differed). The 11 spatial coordinates that differ do so by <= 4.31e-16 relative (1 ulp)
and come only from `sin`/`cos`: `spatial_differences_are_libm_roundoff` recomputes the
cylindrical default and the spherical case with `std` (glibc) and reproduces **all 240
coordinates** of upstream. Accepted under gh:#1455 (PETIR libm), flagged for the maintainer.
`powerlaw_differences_are_libm_roundoff` does the same A/B for `pow`.

**Findings (upstream, not copied):**
- `PointCloud` XML bias reads the *unbiased* `strengths` as the bias
  (`distribution_spatial.cpp:420`, same in `MeshSpatial` :326): every weight is 1. The port
  takes the bias literally (`point_cloud_biased`); pinned by
  `pointcloud_xml_bias_defect_is_upstream_not_port`.
- `Equiprobable::sample_unbiased` reads `x_[i + i]` (`distribution.cpp`): not ported.
- `PowerLaw::evaluate` truncates `n+1` to `int`: ported as upstream (wrong for non-integer n).
- `Legendre` has no sampler upstream.
