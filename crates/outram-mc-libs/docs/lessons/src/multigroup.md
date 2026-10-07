# Multigroup mode, and where C5G7 will go

## The problem

Continuous-energy transport carries a neutron's energy as a real number and
looks up pointwise cross sections at every collision. **Multigroup** transport
collapses the energy axis into `G` groups. The neutron carries a group index,
each cross section is a constant per group, and scattering between groups is a
matrix `Σ_s,g→g'`. It is cheaper, and it is what most deterministic codes and
many benchmarks (C5G7 among them) are written in.

## The code

[`physics::physics_mg`](../../api/outram_mc_libs/physics/physics_mg/index.html)
is a port of OpenMC's MG mode (`physics_mg.cpp`, `mgxs.cpp`; [Romano et al., 2015](#ref-romano2015openmc)):

- [`Mgxs`](../../api/outram_mc_libs/physics/physics_mg/struct.Mgxs.html) holds one
  material's group constants (the `XSdata` analogue), with its invariants checked
  on construction: fission ≤ absorption ≤ total, and χ sums to 1;
- [`MgxsLibrary`](../../api/outram_mc_libs/physics/physics_mg/struct.MgxsLibrary.html)
  is indexed by material index, so it lines up with the geometry's cells;
- [`run_keff_mg`](../../api/outram_mc_libs/physics/physics_mg/fn.run_keff_mg.html)
  is the MG twin of `run_keff_csg`: the same surface tracking and power
  iteration, with group-indexed collisions.

The crate does not *generate* group constants. Collapsing continuous-energy data
into groups belongs to `njoy-outram-park-fork`. Here the constants are only
consumed.

## Case study: anisotropic scattering, measured against OpenMC (GitHub #265)

Until 2026-09-22 every MG set was scattered **isotropically in the lab frame**.
The module doc carries the struck-through old claim and its correction
([`physics_mg.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/physics_mg.rs#L45-L68)).
[`Mgxs::with_legendre_scattering`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/physics_mg.rs#L263)
now attaches Legendre moments per transfer, and the kernel samples the outgoing
cosine from them.

The measurement in
[`ablation_2026_09_22.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/verification_and_validation/mg_anisotropic_scattering/ablation_2026_09_22.md)
is a model of how to price a physics term:

- **The prediction was written first** (one-group diffusion, −4200 pcm, ±30 %),
  unedited since.
- **Paired arms**: P3 (mean cosine 0.3) against P0 (isotropic), 128 shared seeds.
- **A control that must not move**: a reflective cube has no leakage, so the
  angular law cannot matter there.
- **Code-to-code**: OpenMC itself, in MG mode, on the same constants. MG mode needs
  no nuclear-data library, so the two codes are fed the same constants.

Recorded results (bare cube, a = 35 cm):

| | this crate | OpenMC MG | Δ |
|---|---|---|---|
| P3 | 0.63356 ± 0.00028 | 0.63348 ± 0.00012 | +8 ± 30 pcm |
| P0 | 0.67727 ± 0.00031 | 0.67771 ± 0.00012 | −44 ± 33 pcm |
| paired worth | **−4371 ± 44 pcm** | −4423 ± 17 pcm | +52 ± 47 pcm |

The reflective control moved by −1 ± 38 pcm, and both codes reproduce the set's
closed-form `k∞ = 1.10000`. The measured worth was 4.1 % from the prediction.

## What MG mode does not yet do

Stated in the module doc rather than left for a user to discover:

- **No variance reduction.** Survival biasing and weight windows are wired into the
  CSG kernel only, and [`MgSettings`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/physics_mg.rs#L457-L470)
  has no field for them.
- **No delayed-neutron separation.** Delayed neutrons are folded into ν̄.
- **Population control is still the old sampler.** The MG
  [`resample`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/physics_mg.rs#L719-L732)
  draws sites independently with replacement. The continuous-energy drivers moved
  to the uniform comb in GitHub #460 (chapter 5), and this one has not.
- **No Shannon-entropy diagnostic** in the MG driver.

## Where C5G7 will go

The planned capstone of the Monte Carlo track
([#514](https://github.com/theodoreOnzGit/outram-park-backend/issues/514)) is a
guided C5G7 2-D steady state with `run_keff_mg`: pin cell → assembly → core, then
`k` and pin powers against the reference. **None of it exists in the repository
yet.** A search of the crate's source, tests, examples and docs for "C5G7" finds
nothing. The issue records one precondition: check the NEA report's
redistribution terms before shipping the 7-group data.

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-romano2015openmc" style="padding-left: 2em; text-indent: -2em;">Romano, P. K., Horelik, N. E., Herman, B. R., Nelson, A. G., Forget, B., & Smith, K. (2015). OpenMC: A state-of-the-art Monte Carlo code for research and development. <span style="font-style: italic;">Annals of Nuclear Energy</span>, <span style="font-style: italic;">82</span>, 90–97.</p>

<!-- references:end -->
