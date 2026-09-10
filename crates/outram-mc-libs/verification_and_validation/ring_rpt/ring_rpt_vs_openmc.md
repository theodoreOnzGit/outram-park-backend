# Ring-RPT FHR pebble — `outram-mc-libs` vs OpenMC

**Generated:** 2026-09-10 (UTC)
**Crate commit:** branch `op-mzvp2-reactor-physics`
**Status: INCOMPLETE.** Three blockers (below) prevent a quantitative
reactivity-equivalence comparison. This record captures the pipeline, the
numbers it *does* produce, and exactly what is blocking. Not a validated result
— an AI-assisted code-to-code check.

## Methodology

**What RPT is.** A reactivity-equivalent physical transformation replaces the
stochastic TRISO-particle distribution of a pebble's fuel zone with a single
homogeneous fuel *shell* whose inner radius is tuned so the pebble's k-eff is
unchanged. The reference (`op-mzvp.1`, GitHub #156) established that OpenMC
reproduces this to **−31 ± 92 pcm** (0.34σ) on ENDF/B-VIII.0 at
r_inner = 1.493359375 cm, at 46× less cost than the 34 224-cell explicit model.

**The two pebbles** (`openmc_inputs/`, BSD-3 © 2024 theodoreOnzGit):
19.9 % HALEU UCO kernel (215 µm r), buffer/IPyC/SiC/OPyC to 425 µm, 30 % packing
in graphite; graphite matrix + 0.1 cm shell; FLiBe at 99.995 % Li-7; 600 K;
reflective `Sphere(r = 3.0)`.

**`outram-mc-libs` path.**
- Materials + geometry: new `src/pebble_beds/fhr_pebble.rs` (data-free builders —
  `TrisoSpec`, `homogenise_by_volume`, `rpt_fuel_outer_radius`,
  `fhr_pebble_geometry`). Homogenisation is a volume-weighted atom-density mix
  (exact — `Material` is atom-density-based, unlike an OpenMC `Material`).
- Nuclear data: **HIGH-fidelity ENDF/B-VIII.0**, `Nuclide::from_endf_file`
  (RECONR + BROADR on device @ 600 K). All 13 nuclides reconstruct; U-235/U-238
  are Reich-Moore (LRF=3) — no VII.1 fallback needed (`op-mzvp.2.6`).
- Six factors + spectrum: `run_keff_reactor_physics` (`op-mzvp.2.2`).
- Reference `examples/fhr_ring_rpt_endf.rs`, feature `endf-pebble-cases`.

**Pass criterion.** Δ(RPT − explicit) within combined σ of the OpenMC Δ, and the
six factors agreeing per-quantity. **Not evaluated — see blockers.**

## Reference

```bibtex
@misc{openmc_fuel_perf_project_2024,
  author = {Ong, Theodore},
  title  = {openmc\_fuel\_perf\_project: FHR TRISO pebble / ring-RPT models},
  year   = {2024}, note = {GitLab theodore\_ong/openmc\_fuel\_perf\_project,
  base commit ff92278, BSD-3-Clause}}
```
OpenMC 0.15.3-dev (`09ee8308d`), ENDF/B-VIII.0 HDF5, 600 K, 20 000 × 150 (50
inactive). Reference k-eff / six factors: `openmc_inputs/` + GitHub #156.

## Results

### OpenMC reference (op-mzvp.1)

| | explicit TRISO | ring-RPT |
|---|---|---|
| k-eff | 1.36510 ± 0.00063 | 1.36479 ± 0.00067 |
| η / f / p / ε | 2.0158 / 0.9172 / 0.4837 / 1.5064 | 2.0073 / 0.9216 / 0.4842 / 1.5043 |
| **Δ(RPT − explicit)** | | **−31 pcm (0.34σ)** |

### `outram-mc-libs` — fuel-zone k∞ (0.7 cm reflective cube, **free-gas C**)

| case | k∞ | Δ vs explicit |
|---|---|---|
| explicit TRISO (delta tracking, 320 particles, pf 0.3000) | 1.11511 ± 0.00239 | — |
| naive volume homogenisation | 1.20463 ± 0.00209 | **+8952 pcm (28σ)** |
| ring-RPT shell (untuned, delta tracking) | 1.19066 ± 0.00196 | **+7555 pcm (25σ)** |

Six factors (naive homogenised cube): η 1.999, f 1.000, p 0.0065, ε 93.0 —
`p ≈ 0`, `ε ≈ 93` show the spectrum does **not** thermalise: free-gas carbon in
a 0.7 cm cube is a fast system, nothing like the graphite-moderated pebble.

### Interpretation

`outram-mc-libs` does **not** currently reproduce RPT equivalence. The
`naive − explicit` gap of +8952 pcm is not a clean "homogenisation-error"
measurement — it is dominated by the missing thermal treatment (blocker 1) and
the non-representative cube geometry (blocker 2), not by double-heterogeneity
self-shielding alone. A meaningful comparison needs all three blockers cleared.

## Blockers

1. **No graphite S(α,β)** (`op-mzvp.2.8`). Matrix / coating / shell carbon is
   free-gas here; the OpenMC deck binds it as `c_Graphite`. The crate *can* do
   this (`ThermalScattering::from_endf_file`,
   `tsl-crystalline-graphite.endf` is in `reference-data/`); wiring it into this
   example (with separate bound / free-gas C nuclide sets) is the next step.
   `tests/htr10_graphite_thermal_scattering_pebble_bed.rs` measures ~1700 pcm
   for this treatment in a graphite pebble bed.
2. **`run_keff_csg` leaks ~87 % on the concentric-sphere pebble geometry**
   (`op-mzvp.2.11`, P1). A near-tangent transmissive crossing of a curved
   surface plus the fixed 1e-9 nudge lands the neutron on the wrong side;
   `locate` re-picks the departing cell and the next `distance_to_boundary`
   finds no forward surface → the neutron streams to infinity. Blocks the real
   `fhr_pebble_geometry`; the fuel-zone-cube comparison above is the workaround.
3. **Charged-particle absorption under-counted** (`op-mzvp.2.10`, P1).
   `MicroXS::absorption` is capture + fission only — MT 103–117 (incl. Li-6(n,t))
   are classified as scatter. Small here (99.995 % Li-7) but real.

## Bookkeeping status

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** — blocked, and pending maintainer review.
