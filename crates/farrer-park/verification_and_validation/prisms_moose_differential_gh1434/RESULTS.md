# Results: farrer-park vs PRISMS-Fatigue, PRISMS-Plasticity and MOOSE (gh:#1434)

Measured 2026-11-13, rustc 1.99.1 release, g++ (system) `-O2 -ffp-contract=off`,
numpy 2.4.6 / pandas 3.1.1.  Predictions: `PREDICTIONS.md` (commit `b5a4b2212`,
before any run).  Pins, each verified by `git log` on the fetched tree:
PRISMS-Plasticity `ffdf4eb6` (2026-08-27), PRISMS-Fatigue `2c8fc9a2`
(2023-10-13), MOOSE `fe80d24f` (2026-09-10).  All inputs synthetic (fixed LCG)
except the FCC deck files read by upstream's own lines.  Research and V&V use
only (`RESPONSIBLE_USE.md`); nothing here is validation.

## Route taken, and what it is not

| Piece | Route | Is it a run of upstream? |
|---|---|---|
| PRISMS-Fatigue FIP, band averaging | the real `calculate_FIPs.import_PRISMS_data` and `volume_average_FIPs.Al7075_band_averaging`, imported unmodified, in a throwaway venv | **Yes** (Python, pinned tree) |
| PRISMS-Plasticity kernels | `dump_cp_kernels.cpp`: 146 `UPSTREAM` lines verbatim (checked by `check_verbatim.sh`: `checked 146 UPSTREAM lines, 0 not found`), deal.II `FullMatrix` replaced by a shim with sequential-sum `mmult`/`mTmult` | No: transcription.  deal.II's BLAS-backed `mmult` order is not reproduced |
| MOOSE J2 radial return | `dump_moose_j2.cpp`: 110 `UPSTREAM` lines (`checked 110 UPSTREAM lines, 0 not found`); `RankTwoTensor`/`RankFourTensor` shims | No: transcription.  Eigen's mat-vec and libMesh `contract` order not reproduced |
| MOOSE / deal.II builds | **not attempted**: 6.7 GB free, 5 GB floor, ~2 GB allowance; a conda build is a release, not the 2026-08/09 pins | n/a |

## Case 2: PRISMS-Fatigue (real Python)

27 elements x 12 systems = 324 FIPs; 8 grains for band averaging.

| Quantity | Before fix | After fix | Predicted |
|---|---|---|---|
| plastic shear amplitude | 324/324 bit-identical | 324/324 | 100 % (P2.1) met |
| FS FIP, all points | 289/324 identical, worst 2 ulp (2.6e-16 rel) | **324/324** | not 100 % before, 100 % after (P2.2) met |
| FS FIP, points with n > 0 | 121/156 identical (35 differ = 22.4 %) | 156/156 | 10-50 % differ (P2.2) met |
| band average, unit weights | 8/8 identical, same winning system | 8/8 | 100 % (P2.3) met |
| band average, equal weight 3.7e-18 | 6/8 identical, worst 1 ulp, same winning system 8/8 | same | <= 2 ulp (P2.4) met |
| pandas CSV parse vs correctly rounded `float()` | 0 differences of 1998 values | | "small nonzero" (P2.5) **wrong**, none |

**Defect and first divergent operation.** Upstream (`calculate_FIPs.py:97`):
`slip * (1 + k * (normal / sigma_y))`.  Port: `|amp| * (1 + k * normal /
sigma_ref)`, i.e. `(k*normal)/sigma_ref`.  The first divergent operation is
that division: same mathematics, different rounding.  Fixed in
`FatemiSocie::fip` by translating upstream's parenthesisation.  Moved number:
FIP bit-identical count 289 -> 324 of 324 (n>0: 121 -> 156 of 156).

**Pinned deviation, not a defect.** `region_fip` weights by integration
volume (`sum(w f) / sum(w)`); upstream's voxels are equal, `sum(f)/n`.  Equal
weights 1.0 give 8/8 identical; an equal non-unit weight differs by <= 1 ulp on
2/8 grains.  Kept, because unequal element volumes are the point of the
generalisation.

## Case 1 at kernel level: PRISMS-Plasticity RateDependentModel

| Kernel | n | bit-identical | worst | Predicted |
|---|---|---|---|---|
| K1a `SlipSystem::from_miller` fed the deck's 0.577/0.707 numbers vs upstream `init.cc` normalisation | 72 | **72** | 0 ulp | 72/72 met |
| K1b port integer-Miller table vs the same deck path | 72 | 12 | 1 ulp (1.9e-16) | not identical, <= 2 ulp met |
| K1c integer table through upstream's normalisation lines vs port table | 72 | **72** | 0 | (same expression) met |
| q matrix vs `LatentHardeningRatio.txt` | 144 | **144** | 0 | met |
| K2 `odfpoint` vs `Orientation::from_rodrigues` (|r| up to 50) | 1800 | **1800** | 0 | 100 % met |
| K3 `elasticmoduli` vs `stiffness_in_sample_frame` (Cu 170/124/75) | 7200 | 1472 (20 %) | 5.1e-16 of C11 (12288 ulp only on entries that cancel to ~0); identity orientation exact | < 1e-14, < 50 % identical met |
| K4 resolved shear: `trace(T* S^T)` vs `sigma : R sym(S) R^T` | 2400 | 718 (30 %) | 5.0e-16 of max sigma | <= 1e-14 met |
| K5 power-law increment (m 0.02-0.5, 1860 cases incl. exact zero) | 1860 | **1860** | 0 | >= 90 %, <= 2 ulp met (better) |
| K6 `h_beta` | 4800 | 4794 | 2 ulp (`pow`) | >= 90 % met |
| K7 resistance update, before fix | 4800 | 1868 (39 %); 1834 of 4728 where all twelve `h` identical | 4 ulp | >= 10 % differ met (61 % differ) |
| K7 resistance update, after fix | 4800 | **4800** | 0 | equal to K6 fraction: better |

Convention checks that could have failed at O(1) and did not: Voigt shear
factor and crystal-to-sample vs sample-to-crystal in K3 and K4; slip-system
ordering in K1; the deck q pattern.

**Defect and first divergent operation (K7).** Upstream
(`RateDependentModel/calculatePlasticity.cc:698`) runs the resistance as
`s = s + h_ab |dgamma_b|`, one slipping system at a time, starting from `s_t`.
The port summed all increments first and added the sum to `s_t`.  Localised by
the split "where all twelve `h` are bit-identical": 2894 of 4728 outputs
(61 %) still differed, so `pow` was not the cause; the summation order was.
Fixed in `SaturatingHardening::advance`.  Moved number: bit-identical
resistance updates 1868 -> 4800 of 4800.

## Case 3: MOOSE J2 radial return, linear hardening

E = 200 GPa, nu = 0.3, sigma_y0 = 250 MPa, H = 1 GPa, 60 incremental steps,
yield at step 4 in both, 12 elastic steps, 48 yielding, one Newton update per
yielding step in MOOSE (linear residual).

| Metric (history-max normalised) | Measured | Gate |
|---|---|---|
| `||d stress||inf / max||stress||inf` | 5.8e-16 | < 1e-12 |
| `||d eps_p||inf / max||eps_p||inf` | 4.1e-16 | < 1e-12 |
| `|d p| / p` | 1.8e-15 | < 1e-12 |
| yield flag, every step | identical | identical |

Disclosure: normalised by the *instantaneous* plastic-strain norm instead, the
worst is 3.8e-13 at step 50, where `eps_p` passes through zero on reverse
loading (an absolute error of ~1e-18 on a 3.5e-6 remainder).  `PREDICTIONS.md`
says "max |eps_p|" and the history-max form was used for the gate; the
instantaneous figure is printed by the test and is below the 1e-12 gate either
way.  No port defect.

## Not run (and why)

* **Full single-crystal material-point driver** (issue case 1 as written).
  The issue's prediction (< 1e-10 unless sub-stepping or line search differ) is
  **wrong as stated**: the port is small-strain with start-of-step hardening and
  no backstress, upstream is finite-deformation (`Fe Fp`, exponential `Fp`,
  Ohno-Wang backstress, `numberOfCuts`, an outer fixed point on `s`).  Child
  issue filed; see the issue comment.
* **Any compiled MOOSE or deal.II run.**  Everything above that says MOOSE is a
  transcription; do not quote it as "agrees with a MOOSE run".
* **PRISMS-Fatigue `generate_microstructures.py` band construction**:
  `region_fip` is compared *given* a partition.

## Reproduce

```sh
# PRISMS-Fatigue (needs numpy, pandas; a venv is fine)
python -I fip_driver.py <PRISMS-Fatigue>/src <scratch> | grep -E "^[PIFDBN] " > reference_fip.txt
# PRISMS-Plasticity kernels
g++ -O2 -ffp-contract=off -o dump_cp dump_cp_kernels.cpp
./dump_cp <PRISMS-Plasticity>/applications/crystalPlasticity/fcc/FCC_Random_RateDependent > reference_cp_kernels.txt
sh check_verbatim.sh dump_cp_kernels.cpp <PRISMS-Plasticity>/src /tmp/norm.txt
# MOOSE (sparse checkout of framework/include/utils, modules/solid_mechanics)
g++ -O2 -ffp-contract=off -o dump_j2 dump_moose_j2.cpp && ./dump_j2 > reference_moose_j2.txt
sh check_verbatim.sh dump_moose_j2.cpp <MOOSE> /tmp/norm.txt
cargo test --release -p farrer-park --test prisms_differential --test prisms_differential_crystal --test prisms_differential_moose_j2 -- --nocapture
```
