# Ring-RPT FHR pebble — `outram-mc-libs` vs OpenMC

**Generated:** 2026-09-10 (UTC)
**Crate commit:** branch `op-mzvp2-pebble-wiring` (off `develop`)
**Status: first full-pebble comparison.** The two P1 transport bugs that blocked
this (GH #168 concentric-sphere leak, GH #169 charged-particle absorption) are
fixed, and graphite S(α,β) is wired. Absolute k is ~2.9 % above OpenMC, and the
discrepancy is concentrated in the resonance-escape (p, +9 %) and fast-fission
(ε, −6 %) factors — pointing at U-238 epithermal/fast cross sections
(leading suspect: URR self-shielding not reconstructed). η and f match within
1 %. Not a validated result — an AI-assisted code-to-code check, no human V&V.

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

**How `outram-mc-libs` runs it.** Four Monte-Carlo runs, 4000 histories ×
[30 + 80] generations, ENDF/B-VIII.0 @ 600 K, crystalline-graphite S(α,β) on the
buffer / PyC / matrix / shell carbon. Fuel-kernel carbon **and SiC-coating
carbon** are free-gas, matching the deck — the deck applies `c_Graphite` to
neither, and a dedicated `tsl-CinSiC` / `tsl-SiinSiC` law (both present in
`reference-data/endf/`) is used by neither code. Switching the SiC carbon from
`c_Graphite` to free-gas moved the CSG-sphere k by +105 pcm (1.39228 → 1.39333,
within 1σ) and the six factors not at all — confirming the 35 µm coating's
thermal treatment is negligible here.

| run | driver | domain |
|---|---|---|
| explicit TRISO | `run_keff_delta` (Woodcock) | reflective **cube** r→3; 54 706 packed particles in r < 1.9; 5 layers resolved by nearest-centre + radius |
| ring-RPT | `run_keff_delta` | **same cube**; homogenised fuel shell 1.4934–1.7531 cm |
| naive homogenised | `run_keff_delta` | **same cube**; homogenised fuel fills r < 1.9 |
| ring-RPT (CSG) | `run_keff_reactor_physics` | real **reflective sphere** r=3 (`fhr_pebble_geometry`); + six factors + spectrum |

The cube runs share a boundary, so `RPT − explicit` and `naive − explicit` are
clean; the CSG-sphere run is the one directly comparable to the OpenMC absolute k.

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

### OpenMC reference (op-mzvp.1) — full pebble, reflective sphere r=3

| | explicit TRISO | ring-RPT |
|---|---|---|
| k-eff | 1.36510 ± 0.00063 | 1.36479 ± 0.00067 |
| η / f / p / ε | 2.0158 / 0.9172 / 0.4837 / 1.5064 | 2.0073 / 0.9216 / 0.4842 / 1.5043 |
| **Δ(RPT − explicit)** | | **−31 pcm (0.34σ)** |

### `outram-mc-libs` — ENDF/B-VIII.0, `c_Graphite` S(α,β) (free-gas fuel + SiC C)

| run | k-eff | Δ(vs explicit) |
|---|---|---|
| explicit TRISO (delta, cube) | 1.33445 ± 0.00215 | — |
| ring-RPT (delta, **same cube**) | 1.33094 ± 0.00204 | **−351 pcm (1.2σ)** |
| naive homogenised (delta, same cube) | 1.35044 ± 0.00205 | **+1599 pcm (5.4σ)** |
| ring-RPT (**CSG reflective sphere**) | 1.39333 ± 0.00219 | — |

**Ring-RPT CSG six factors:** η 2.0158, f 0.9140, p 0.5279, ε 1.4109,
P_FNL 1.0000, P_TNL 0.9999; product (k_4f) 1.37209, consistency gap +1.52 % (in
band). Leakage 1.1e-4 (reflective — was ~0.87 before the GH #168 fix).

vs OpenMC ring-RPT: η **2.0073** (+0.4 %), f **0.9216** (−0.8 %),
p **0.4842** (**+9 %**), ε **1.5043** (**−6 %**).

### Interpretation

1. **Absolute k: CSG-sphere ring-RPT k = 1.39333 is +2854 pcm (~2.9 %) above
   OpenMC's 1.36479.** Adding graphite S(α,β) moved it down from the free-gas
   regime (`tests/htr10_graphite_thermal_scattering…` measures ~1700 pcm for
   that treatment alone). SiC-carbon free-gas vs `c_Graphite` is worth only
   ~+105 pcm (1σ) — negligible. The residual splits as below.
2. **The miss is entirely in p and ε.** η and f match OpenMC within 1 %, but
   resonance escape **p is +9 % (0.528 vs 0.484)** and fast fission **ε is −6 %
   (1.411 vs 1.504)** — both governed by the U-238 epithermal / fast cross
   sections. The most likely cause is **U-238 unresolved-resonance-region (URR,
   ~20–150 keV) self-shielding not being reconstructed**: OpenMC's pre-built
   NNDC ACE carries PURR probability-table self-shielding through the U-238 URR;
   `njoy-outram-park-fork` reconstructs RECONR + BROADR on device but the URR
   probability-table path (UNRESR / PURR) is ported yet unverified and not wired
   into this path, so U-238 there comes out as a smooth infinite-dilution
   background with no resonance-integral enhancement → too little epithermal
   capture → p too high → k too high. A secondary contributor is
   RECONR-on-device vs NNDC ACE processing differences generally (grid density,
   broadening kernel, thinning). Next diagnostic: cross-check reconstructed
   U-238 σ_γ(E) against the NNDC HDF5 at the 6.7 eV resonance (should match) and
   across 20–100 keV (expect ours low and flat).
3. **The consistency check passes** (+1.52 % gap, in the `(−1 %, +5 %)` band) —
   first evidence the 3-group decomposition in `run_keff_reactor_physics` is
   physically sound on a real thermal system.
4. **RPT has the right effect; magnitude is code-dependent.** On the matched
   cube, naive homogenisation over-predicts by **+1599 pcm**; the ring-RPT shell
   brings that to **−351 pcm** (1.2σ — now statistically marginal), the correct
   direction. It is not expected to reach OpenMC's −31 pcm because **the RPT
   inner radius (1.493359375 cm) was fitted by the deck author against OpenMC**;
   the equivalence radius is code- and library-dependent. A radius search in
   `outram-mc-libs` (`physics::search::search_for_keff` against the CSG pebble)
   is the next step.
5. **The delta-cube rows carry ~2.3× the FLiBe of the real sphere** (cube
   corners outside r = 2). Post-#169 FLiBe absorbs correctly (Li-6 at 938 b), so
   the cube k's sit low (explicit-cube −3000 pcm vs OpenMC) while the CSG-sphere
   sits high (+2900) — they bracket OpenMC in opposite directions, so this is
   **not one systematic data bias**. Only the CSG-sphere row is apples-to-apples.
6. **Both P1 transport bugs are fixed** — GH #168 (concentric-sphere leak: k
   0.24 → 1.39, leakage 0.87 → 1e-4) and GH #169 (Li-6(n,t) absorption
   0.04 → 938 b). The `fhr_pebble_geometry` CSG path transports correctly.

## Remaining work


1. **RPT radius search** — fit the ring-RPT inner radius in `outram-mc-libs`
   itself (`search_for_keff` against `fhr_pebble_geometry`) rather than using
   the OpenMC-tuned 1.4934 cm. The −736 pcm residual is expected to close.
2. **Explicit-TRISO pebble in CSG** — the delta-cube explicit run has two
   approximations (cube-corner FLiBe, layer-by-radius). An explicit-TRISO CSG
   build (now that GH #168 is fixed) would make the explicit row directly
   comparable to OpenMC's 1.36510 too.
3. **Residual absolute-k bias (~2.9 %), concentrated in p and ε** — cross-check
   reconstructed U-238 σ_γ(E) against the NNDC HDF5 the OpenMC deck used: at the
   6.7 eV / 20.9 eV resolved resonances (expect agreement) and across the
   20–150 keV URR band (expect ours low and structureless if URR self-shielding
   is absent). If confirmed, the fix is wiring UNRESR/PURR into the
   `Nuclide::from_endf_file` reconstruction path (`njoy-outram-park-fork`).

## Bookkeeping status

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** — blocked, and pending maintainer review.
