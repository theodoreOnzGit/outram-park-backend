# Ring-RPT FHR pebble — `outram-mc-libs` vs OpenMC

**Generated:** 2026-09-10 (UTC)
**Crate commit:** branch `op-mzvp2-pebble-wiring` (off `develop`)
**Status: first full-pebble comparison.** The two P1 transport bugs that blocked
this (GH #168 concentric-sphere leak, GH #169 charged-particle absorption) are
fixed, and graphite S(α,β) is wired. Absolute k is now within ~2.7 % of OpenMC
and the six factors within a few %. Not a validated result — an AI-assisted
code-to-code check, no human V&V.

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
coating / matrix / shell carbon (fuel-kernel carbon free-gas, as in the deck):

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

### `outram-mc-libs` — ENDF/B-VIII.0, `c_Graphite` S(α,β)

| run | k-eff | Δ(vs explicit) |
|---|---|---|
| explicit TRISO (delta, cube) | 1.33600 ± 0.00214 | — |
| ring-RPT (delta, **same cube**) | 1.32863 ± 0.00202 | **−736 pcm (2.5σ)** |
| naive homogenised (delta, same cube) | 1.35046 ± 0.00211 | **+1447 pcm (4.8σ)** |
| ring-RPT (**CSG reflective sphere**) | 1.39228 ± 0.00232 | — |

**Ring-RPT CSG six factors:** η 2.0156, f 0.9142, p 0.5285, ε 1.4103,
P_FNL 1.0000, P_TNL 0.9998; product 1.37328, consistency gap +1.36 % (in band).
Leakage 1.1e-4 (reflective — was ~0.87 before the GH #168 fix).

vs OpenMC ring-RPT: η **2.0073** (−0.4 %), f **0.9216** (+0.8 %),
p **0.4842** (−8 %), ε **1.5043** (+7 %).

### Interpretation

1. **Absolute k is now close.** The CSG-sphere ring-RPT k = 1.39228 is **+2749
   pcm** (~2.7 %) above OpenMC's 1.36479. Adding graphite S(α,β) moved it down
   from the free-gas regime (`tests/htr10_graphite_thermal_scattering…` measures
   ~1700 pcm for that treatment alone); the residual is a mix of the fuel-kernel
   free-gas carbon, the reconstruction differences RECONR-on-device vs the
   pre-built NNDC ACE library, and the delta-cube corner FLiBe on the
   delta-tracked rows (the CSG row has none).
2. **The six factors track OpenMC to a few percent** — η and f within 1 %, p and
   ε within ~8 %. The consistency check passes. This is the first evidence the
   3-group decomposition in `run_keff_reactor_physics` is physically sound on a
   real thermal system.
3. **RPT has the right effect but the wrong magnitude.** Naive homogenisation
   over-predicts by **+1447 pcm**; the ring-RPT shell brings that to **−736
   pcm** — the correct direction and roughly halved. It does not reach OpenMC's
   −31 pcm because **the RPT inner radius (1.493359375 cm) was fitted by the
   deck author against OpenMC**; the equivalence radius is code-dependent (the
   deck's own notes say it is even library-dependent). A radius search in
   `outram-mc-libs` (`physics::search::search_for_keff` against the CSG pebble)
   would be the next step to close this.
4. **Both P1 transport bugs are fixed** — GH #168 (concentric-sphere leak: k
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
3. **Residual absolute-k bias (~2.7 %)** — quantify the split between the
   free-gas fuel-kernel carbon, RECONR-on-device vs NNDC ACE, and unresolved
   resonance treatment. Cross-check one nuclide's reconstructed σ(E) against the
   NNDC HDF5 the OpenMC deck used.

## Bookkeeping status

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** — blocked, and pending maintainer review.
