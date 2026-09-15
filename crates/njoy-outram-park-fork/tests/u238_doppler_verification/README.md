<!--
SPDX-License-Identifier: GPL-3.0-only
Copyright (C) 2026 OUTRAM PARK contributors
Part of Outram Park (outram-park-backend), njoy-outram-park-fork.
Derivative work of NJOY2016 — see the crate NOTICE / LICENSE.njoy.
-->

# U-238 (n,γ) Doppler broadening — code-to-code verification vs OpenMC

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


Full-pipeline verification of the Rust NJOY port: reconstruct the U-238
radiative-capture cross section (MT=102) **directly from the ENDF/B-VIII.0 tape**
with RECONR (Reich-Moore, LRF=3), Doppler-broaden it to 900 K and 1200 K with
BROADR (SIGMA1 free-gas kernel), and compare **point-for-point** against
OpenMC's own ENDF/B-VIII.0 pointwise capture data.

- Test: `main.rs` (`capture_doppler_matches_openmc`, `capture_pipeline_is_physical`).
- Run under the crate memory cap:
  `crates/njoy-outram-park-fork/scripts/test.sh capture -- --nocapture`.

## Data provenance

| Input | File | Role |
|-------|------|------|
| ENDF/B-VIII.0 U-238 tape (MAT 9237) | `reference-data/endf/n-092_U_238.endf` | Rust RECONR+BROADR input |
| OpenMC ENDF/B-VIII.0 reference | `reference/openmc_capture_{900,1200}K.csv` | comparison target |

The OpenMC reference originates from `U238.h5` (OpenMC's HDF5 nuclear-data
library from <https://openmc.org/data/>, itself produced by the *reference* NJOY
from the same evaluation). That file is **≈115 MB — too large for GitHub**, so the
one-time extractor `examples/extract_u238_doppler_ref.rs` samples its pointwise
capture onto a **100,000-point log-spaced grid** (10⁻⁵ eV → 20 MeV) and
writes the ~2.3 MB reference CSVs committed here (100,000 points each). Once those
exist, `U238.h5` can be deleted and this test still runs. Regenerate (if the h5
is present) with:

```bash
cargo run --release --example extract_u238_doppler_ref
```

## Method

For each reference energy `E`, the OpenMC value (from the CSV) is compared to the
Rust port's broadened capture at the same `E` (`NuclearDataLibrary::capture_xs`,
lin-lin interpolation of the port's broadened grid). The comparison uses a
100,000-point log grid (~8100 points/decade) — resonance-resolved while ~4.5×
smaller than the native 448k-point 0 K grid.

Regions:

- **Resolved resonance region (RRR), E ≤ 20 keV** — where RECONR reconstructs the
  Reich-Moore resonances and where all strong, Doppler-sensitive capture lines
  sit (6.67, 20.9, 36.7, 66, 81, 90, 103.5, 117 eV, …).
- **Above 20 keV** — the port returns the MF=3 infinite-dilution average
  (unresolved reconstruction UNRESR/PURR is the next module to port; see
  `docs/porting-plan.md` §8).

The Doppler width used by SIGMA1 is the free-gas value
$\Delta_D = \sqrt{4 E\, k_B T / A}$ (≈ 0.17 eV for the 20.9 eV resonance at
900 K).

Plottable output is written to
`results/compare_capture_{900,1200}K.csv` (columns
`energy_eV, sigma_openmc_b, sigma_rust_b, rel_diff, region`).

## Results (re-baselined 2026-09-10 after `WAVE_K`; RECONR tol 0.1%, SIGMA1)

Sanity (`capture_pipeline_is_physical`): reconstructed 0 K thermal capture
~2.7 b, 6.67 eV peak ~22,000 b; Doppler broadening at 900 K lowers that peak
to ~4,900 b and raises the 6.5 eV wing — all correct in direction.

Magnitude-weighted L1 relative error $\sum|\sigma_{rust}-\sigma_{omc}| / \sum|\sigma_{omc}|$,
and the worst single point:

| Temperature | RRR L1 (E ≤ 20 keV) | worst RRR point | Above-RRR L1 (MF=3) |
|-------------|---------------------|-----------------|---------------------|
| 900 K       | **0.0002**          | 0.74 % at 80.06 eV | 0.0000 |
| 1200 K      | **0.0002**          | 0.72 % at 516.4 eV | 0.0000 |

(75,612 RRR points / 24,388 above-RRR points of the 100,000-point grid.)

Resonance-peak spot checks at the nearest grid point to each peak:

| Resonance | T | OpenMC [b] | Rust [b] | Δ |
|-----------|---|------------|----------|---|
| 6.673 eV | 900 K | 4530.8 | 4532.5 | +0.04 % |
| 6.673 eV | 1200 K | 3995.9 | 3997.1 | +0.03 % |
| 20.87 eV | 900 K | 4214.6 | 4218.6 | +0.09 % |
| 20.87 eV | 1200 K | 3702.9 | 3705.5 | +0.07 % |

**Gates (hard, both temperatures, `op-cjw.10`):** RRR L1 < 0.1 %, worst RRR
point < 3 %, peaks within 0.5 %, above-RRR L1 < 0.1 %, all values finite and
non-negative — each 3–5× above the measurement, tight enough that a regression
of the wing bug below, or of `WAVE_K` (every resonance σ moves 0.08 %), trips
the L1 gate. Runtime 270–280 s (the test reconstructs U-238 twice; unchanged).

Original 2026-07-06 numbers, for the record: RRR L1 0.302 / 0.315, above-RRR
L1 0.0155 / 0.0144, peaks +8.5 % — the wing pedestal resolved below.

## ✅ RESOLVED (2026-07-07) — it was the RECONR grid, not SIGMA1

The wing pedestal documented below is **fixed**. It was **not** a BROADR/SIGMA1
bug: the 0 K reconstruction grid left multi-eV gaps between resonances (for the
102.56 eV line, nothing between 103.03 eV and 111.10 eV), so the Lorentzian wing
was represented by a single straight line ~35× above the true curve — and SIGMA1
was both *fed* that over-stated wing and *sampled* on the same coarse output
grid. Adding adaptive refinement of the resonance-reconstruction grid to
tolerance (`reconr::refine_resonance_grid`) dropped the **RRR L1 from ≈0.30 →
≈0.0007** at both 900 K and 1200 K (a ~400× accuracy gain), with **no change to
the SIGMA1 kernel**. Point checks: 105 eV, 900 K now 1.845 b (OpenMC 1.8 b) vs
the old 211 b; 106 eV now 0.927 b (OpenMC 0.93 b) vs old 177 b. See
`src/reconr/README.md` and `src/broadr/README.md`.

The RRR L1 became a hard gate on 2026-09-10 (`op-cjw.10`, see the Results
section) once the `WAVE_K` correction took the residual from ≈0.0007 to
0.0002. The original finding is preserved verbatim below for the record.

## ⚠ Finding (now resolved — see above) — the port over-predicts resonance wings (BROADR/SIGMA1)

The above-RRR (smooth MF=3) band matches OpenMC to ~1.5%, and the resonance
**peaks** broaden correctly to ~10%. But in the RRR the port leaves a **spurious
wide pedestal** several eV past each resonance, where OpenMC has already decayed
to ~1 b. Fine scan around the 103.5 eV resonance at 900 K:

| E [eV] | OpenMC σ [b] | Rust σ [b] |
|--------|--------------|------------|
| 102.6 (peak) | 931 | 1007 |
| 103.2 | 89.8 | 273 |
| 103.6 | 13.0 | 259 |
| 104.0 | 5.7  | 246 |
| 105.0 | 1.8  | 211 |
| 106.0 | 0.93 | 177 |

The off-resonance baseline far from any line agrees (e.g. 19.5 eV: OpenMC 5.20 b
vs Rust 5.32 b), so this is not a global offset — it is a wing/pedestal that
tracks nearby strong resonances and decays far too slowly. It dominates the
RRR L1 (0.30–0.32) and the per-point max (up to ~300× in the deep valleys just
past a resonance).

This is a genuine fidelity bug in the Rust SIGMA1/BROADR path (or its interaction
with the RECONR wing grid), surfaced by this verification. Per the njoy port's
model division of labour ("Opus debugs/verifies"), it is **documented and left
for the BROADR debugging pass — the test gate is deliberately not loosened to
hide it**. The test therefore gates only what the port currently satisfies
(finiteness/non-negativity and the < 5% above-RRR agreement) and *reports* the
RRR L1 with a `⚠ KNOWN DISCREPANCY` line.

## Interpretation

The reconstruction (0 K peaks, thermal value), the peak Doppler broadening
(direction and ~10% magnitude), and the smooth infinite-dilution region are all
correct. The open issue is the SIGMA1 kernel's resonance-wing behaviour. When
that is fixed, tighten the RRR L1 into a hard gate here.
