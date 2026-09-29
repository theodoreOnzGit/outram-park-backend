# URR bands, DBRC and equiprobable S(α,β) sampling as OpenMC (GitHub #407)

*2026-09-29. Research, education and V&V only; not for any operational,
licensing or safety use (see `RESPONSIBLE_USE.md`).*

This work follows the #407 upstream survey (comment 5887491557), which
compared outram-mc against OpenMC `d7d3284a1`. It covers three changes of
sampling scheme, all to correct physics as OpenMC implements it. Each change is
on by default, and pinned there. For each one, the predicted k shift was posted
on #407 **before** its paired A/B ran.

## 1. URR: one band per nuclide and energy

**Upstream.** OpenMC computes a nuclide's micro cross sections once per energy
(`Nuclide::calculate_xs`, `src/nuclide.cpp:826-883`). In the unresolved range
they come from a single band, drawn as
`r = future_prn(index_, seeds[STREAM_URR_PTABLE])` (`calculate_urr_xs`). That
band's total feeds the macroscopic total, and so the flight. The same cached
value is used for the choice of nuclide and for the reaction. After a
collision that changed `E`, the stream advances by the number of nuclides
(`physics.cpp:164-167`).

**Before.** outram-mc flew on, and chose the nuclide by, the infinitely-dilute
total. It drew a fresh band from the transport stream only at the collision.

**Now.**
- `material::urr_xi(nuclide_idx, urr_seed)` gives the band variate.
- `Material::macro_xs_total_urr` and `sample_nuclide_urr` use it for the flight
  and the nuclide choice. `Nuclide::band_total` is the band total.
- Each history carries its own URR stream (`future_seed(5 × stride, seed)`).
- **Kernels ported:** the CSG kernel (`transport_csg`), the CPU sphere kernel
  (`keff::transport_history`) and the pebble delta kernel (`keff_delta`).
- **Delta tracking:** majorants bound the band total through
  `Nuclide::total_upper_bound`, which is exact because
  `UrrProbabilityTables::band_representatives` visits every selectable band pair.
- **Not ported:** the GPU flight paths in `keff.rs` still fly on the smooth
  total. This is documented at both sites.

**Verification (`tests/urr_band_flight_consistency.rs`).** On NJOY2016 U-235 and
U-238:
- the bound equals the maximum sampled band total at 42 energies, to 1e-12;
- the macroscopic total equals its band-total definition, and is bit-equal to
  the smooth total outside the range;
- the nuclide choice follows the band weights, |z| ≤ 1.75 over 400 000 draws.

## 2. DBRC: OpenMC's `sample_target_velocity`

| | before | now (`physics.cpp:870-965`) |
|---|---|---|
| at-rest gate | 400 kT for every nuclide, so DBRC never acted above 10.1 eV at 293.6 K | a resonant nuclide (one with a 0 K table) is at rest only above `e_max` (1 keV); 400 kT only for the others |
| window | `E ± 4√(kT E/A)`, about half the width | `(√(AE/kT) ± 4)² kT/A` |
| candidates with `E_rel ≥ E_up` | kept | resampled |
| degenerate window (no grid point inside) | — | CXS |

**Verification.** The unit test `dbrc_target_density_is_cxs_times_sigma_0k`
checks an importance-weighting identity: the DBRC `E_rel` density must equal
the CXS density reweighted by σ⁰ᴷ. The case is a synthetic resonance at 20 eV,
A = 236, E = 20.3 eV. Worst |z| is 2.65 over 30 bins. The old half-width window
fails at z = −8.6. A companion assertion checks that the neutron can up-scatter
at 20.3 eV, which a target at rest forbids.

**Named residual difference.** `res_scat_energy_min` is OpenMC's default,
0.01 eV. Route 1's deck sets 1e-5. Below 0.01 eV the heavy nuclides' 0 K
elastic is smooth, so the two samplers coincide.

## 3. Equiprobable S(α,β) (IFENG = 0) as OpenMC

**Upstream.** `IncoherentInelasticAEDiscrete` with `skewed_ = false`
(`secondary_thermal.cpp`):
1. `i, f` come from `get_energy_index`;
2. `j = floor(prn n)`;
3. `E' = (1−f)E(i,j) + f E(i+1,j)`;
4. `mu` is interpolated the same way.

**Before.** The #188 scheme: statistical choice of table `i` or `i+1`, with
`E'` drawn continuously within the bin. It was recorded as a maintainer
decision, and **that record was false**: CORRECTED 2026-09-29, never a
maintainer decision; replaced by OpenMC's scheme per the maintainer.

**Now.** OpenMC's scheme is the default on both routes. The #188 scheme is only
the ablation `--ablate legacy-thermal-sampling`. The default is pinned by
`correct_physics_is_default::thermal_equiprobable_sampling_is_openmc_by_default`.

**Verification (`tests/thermal_equiprobable_vs_openmc.rs`).** NJOY2016 H in H2O
(64 bins) and C in graphite (16 bins), at 11 energies, against the exact
moments and CDF of OpenMC's scheme:
- five moments, worst |z| 1.86;
- KS of `E'`, D/crit ≤ 0.92;
- the legacy scheme fails the KS at every energy (D/crit 4.1–46).

**Drift pins that moved.** `tests/thermal_kernel_stationary_distribution.rs` and
`graphite_sab_kernel_width_against_njoy_thermr` moved. With the legacy
ablation applied, each reproduces its previous record exactly. Their new
values are recorded in the tests.

## Paired k A/B, route 3 (NJOY2016 ACE), campaign settings

A = before the change, B = after it; same seeds. Predictions are as posted on
#407 before the runs.

| case | seeds | URR port | predicted | DBRC fix | predicted |
|---|---|---|---|---|---|
| Godiva | 32 | +39 ± 44 | \|Δ\| < 15 | −2 ± 39 | \|Δ\| < 10 |
| Jemima | 128 (URR) / 32 | **+38 ± 20** | 0 to +30 | −68 ± 45 | \|Δ\| < 10 |
| HST-009 | 32 | −19 ± 36 | \|Δ\| < 10 | +48 ± 37 | 0 to −15 |
| LCT-008 lattice | 32 | **+32 ± 18** | +5 to +25 | +7 ± 18 | −5 to −30 |

**Reading.**
- **URR.** Both resolved-ish shifts (lattice +32 ± 18, Jemima +38 ± 20 at 128
  seeds) are positive and sit at the top of their predicted ranges. The lattice
  shift is toward OpenMC. Jemima's 32-seed first look, +91 ± 38, was pulled in
  by the declared extension.
- **DBRC.** The fix is not resolved on any case. On the lattice, the predicted
  −5 to −30 is **not supported**: +7 ± 18 is consistent with zero. It is also
  consistent with the survey's OpenMC DBRC-cut run, −14 ± 12.

S(α,β) sampler, default − legacy ablation, same binary:

| case | route 3 | predicted | route 4 | predicted |
|---|---|---|---|---|
| HST-009 | −17 ± 47 | 0 to +60 | −7 ± 33 | \|Δ\| ≤ 30 |
| LCT-008 lattice | **+36 ± 16** | +10 to +60 | **+42 ± 14 (3.1σ)** | \|Δ\| ≤ 30 |

**Reading.**
- **Route 3.** The route-3 lattice shift (+36 ± 16) is inside its prediction and
  toward OpenMC. It matches the share the free-gas A/B had pointed at
  (+36 ± 21).
- **Route 4.** The route-4 prediction (\|Δ\| ≤ 30, "the finer bake moves
  less") is **not supported**: +42 ± 14 is resolved at 3.1σ, and larger than
  on route 3.
  - The reasoning behind the prediction was wrong. Route 4's H-in-H2O bake has
    the same 64 bins as NJOY's table (`N_OUTGOING = 64`), so it has no finer
    `E'` to protect it.
  - Route 4 sat −59 ± 14 below OpenMC before, so this moves it toward OpenMC as
    well.
- **HST-009.** Both routes are consistent with zero at ±33–47.
