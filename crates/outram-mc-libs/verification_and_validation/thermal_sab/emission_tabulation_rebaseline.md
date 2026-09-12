# S(α,β) emission tabulation: the repair, and every recorded k it moved

**Date: 2026-09-12.** Tracks GitHub [#190] (bead `op-x77y`) and [#188]
(bead `op-77pu`).

This file is the re-baselining record for the change described below. It exists
because the change moves *every* recorded k-effective of a thermally-scattering
system in this crate, and a superseded number that is quietly overwritten is
indistinguishable from one that was never measured.

[#190]: https://github.com/theodoreOnzGit/outram-park-backend/issues/190
[#188]: https://github.com/theodoreOnzGit/outram-park-backend/issues/188

---

## 1. What changed

`crates/outram-mc-libs/src/material/thermal.rs`:

| constant | was | now | what it controls |
|---|---|---|---|
| `N_EMIT_GRID` | 48 | **384** | incident energies carrying an emission table |
| `N_OUTGOING` | 16 | **64** | equiprobable outgoing energies per table |

plus one non-physics change in the same file: `select_table` and `interp_linear`
now find their bracket by binary search instead of a linear scan from index 0.
That is an **exact** refactor — the same bracket, the same `r`, the same
sampled value — and it is what makes the finer grid affordable: thermal
sampling throughput went from 6.06 Msample/s at 48 points (linear scan) to
**11.22 at 48 and 10.33 at 384** (binary). The evidence that it is exact is
that the FHR ring-RPT CSG pebble reproduced its previously recorded
`k = 1.40745 ± 0.00214` bit-for-bit with the refactor in and the constants
still at 48/16.

## 2. Why — the defect, measured

`ThermalScattering::sample` picks between the two bracketing emission tables by
ACE statistical interpolation. That is unbiased in the mean but adds a variance
`r(1−r)(m₂−m₁)²` the true kernel has not got, falling as the square of the grid
spacing. At 48 points over 1e-5 … 4 eV adjacent tables were **31.6 % apart**,
and against NJOY2016 THERMR MF=6/MT=229 (`tsl-crystalline-graphite`, 600 K,
quadrature on the oracle side, 400 000 samples on ours) the sampled kernel was

- **+39.0 % too broad at 2 eV** — the grid, and
- **−11.6 % too narrow at 0.05 eV** — the 16-bin equiprobable representation,

with the sign flipping at 0.39 eV. Two errors, two dimensions, opposite signs.

Both sizes were chosen by sweeping them against that oracle
(`examples/thermal_emission_grid_convergence.rs`), not by taste:

| n_emit (at 64 bins) | worst width | rms | build [s] | Msample/s |
|---|---|---|---|---|
| 192 | −2.45 % | 1.65 % | 8.7 | 10.36 |
| **384** | **−2.50 %** | **1.78 %** | **13.3** | **10.33** |
| 768 | −2.56 % | 1.81 % | 22.7 | 10.29 |
| 1536 | −2.59 % | 1.85 % | 39.6 | 10.10 |

| n_out (at 384 points) | graphite worst | rms | H₂O ⟨E′⟩/E at 1.5 meV |
|---|---|---|---|
| 16 | −11.60 % | 6.44 % | −5.51 % |
| 32 | −5.96 % | 3.25 % | −3.26 % |
| **64** | **−2.50 %** | **1.78 %** | **−2.10 %** |
| 128 | −1.65 % | 1.10 % | −1.43 % |

384 is where the incident grid stops improving: the rms is flat across the next
two doublings (1.78 → 1.81 → 1.85 %), and 384's residual distance from the
converged 1536-point answer is ≤ 0.6 points where 192's is ≤ 1.5 — and 192 still
carries a one-signed *broad* bias (+0.17 % at 0.625 eV where the converged
answer is −1.32 %) that 384 does not.

The column-to-column differences in both tables are far more precise than the
absolute values, because every tabulation in that sweep is sampled from the same
stream — common random numbers. An individual absolute row carries up to 0.8
points of stream-to-stream scatter, measured between the sweep's 400 000 samples
and the 200 000-sample gate; the Gaussian `1/sqrt(2N)` estimate of 0.11 % is
optimistic for a width, because the variance estimator carries the error of a
fourth moment on a heavy-tailed distribution. The outgoing-bin count has **no** plateau — it
falls as ~1/n_out — so 64 is a cost cut, not a convergence point, and the
residual −2.50 % is the equiprobable representation itself. Its proper fix is a
continuous outgoing-energy law (NJOY's `iform = 1`), which is follow-up work.

## 3. #188 does **not** share #190's cause

Measured, not assumed. Water's kernel is **insensitive to the incident-energy
grid** (⟨E′⟩/E at 1.5 meV is −5.40 % at 48 grid points and −5.51 % at 1536) and
**responds only to the outgoing-bin count** (−5.51 % → −3.26 % → −2.10 % →
−1.43 % at 16 → 32 → 64 → 128). Graphite's #190 is the other dimension. They
are two dimensions of the same ACE equiprobable pre-tabulation, not one defect.

**#188 is mitigated, not closed.** ⟨E′⟩/E worst goes −5.5 % → −2.1 %, but
`ξ = ⟨ln(E/E′)⟩` — the most tail-sensitive moment there is — barely moves
(−3.24 % → −2.89 %, and only −2.79 % at 128 bins). Water still moderates ~3 %
less per collision here than in NJOY.

---

## 4. The k-effective re-baseline

**Read this before quoting any row.** Where a new number is given it was
re-run; where it is not, the old number is marked **stale** rather than
replaced, with the reason. A stale row is not a claim that the old value still
holds — it is a claim that nobody has measured the new one yet.

### 4.1 Re-run

| case | where | before (48×16) | after (384×64) | Δk | date |
|---|---|---|---|---|---|
| FHR ring-RPT CSG pebble, 4000 × [30 + 80], `OUTRAM_RINGRPT_ONLY=csg` | `examples/fhr_ring_rpt_endf.rs` | 1.40745 ± 0.00214 | **1.40546 ± 0.00234** | **−199 ± 317 pcm (0.63σ)** | 2026-09-12 |
| HTR-10 graphite pebble bed, bound arm, 300 × [8 + 15] | `tests/htr10_graphite_thermal_scattering_pebble_bed.rs` | not recorded at these statistics | **1.91699 ± 0.01115** | — | 2026-09-12 |
| …its free-gas arm (unaffected — never touches S(α,β)) | same | 1.93656 ± 0.00316 (manual, 1200 × [10 + 70]) | **1.95252 ± 0.01349** at the test's own statistics | — | 2026-09-12 |
| graphite-moderated k_inf, C/HM = 400, reflective cube | `examples/thermal_kernel_keff_worth.rs` | <!-- KINF-BEFORE --> | <!-- KINF-AFTER --> | <!-- KINF-DELTA --> | 2026-09-12 |

**The pebble number is a non-measurement and is recorded as one.** −199 ± 317 pcm
is 0.63σ; that deck's own statistics (320 000 active histories, σ ≈ 214 pcm per
side) cannot resolve an effect of this size, and the earlier grid-only
comparison that reported −63 pcm could not either. What the two together do
support is a **bound**: the repair is worth a few hundred pcm at most on this
pebble, not thousands.

Non-k quantities that moved, and were re-run:

| quantity | before | after |
|---|---|---|
| graphite kernel width, worst | +39.04 % at 2.02 eV | **−2.33 % at 0.05 eV**, one-signed narrow everywhere |
| graphite ⟨E′⟩/E, worst | −1.53 % | **−0.56 %** |
| graphite μ̄ inelastic, worst | +0.0085 at 0.0253 eV | **−0.0084 at 0.01 eV** — same magnitude, worst point relocated; both are real (≈5σ of the 400 000-sample statistics) and both are well inside the 0.02 envelope. Refining *energy* bins does not sharpen an *angular* law, and this row is the measurement that says so rather than the assumption |
| graphite ξ/ξ_fg at 3.9 eV | 1.002 | **0.987** — see below |
| H₂O ⟨E′⟩/E, worst | −5.54 % | **−1.63 %** |
| H₂O kernel width, worst | (no oracle existed) | **−4.91 %**, one-signed narrow |
| H₂O ξ, worst | −3.24 % | **−2.89 %** |

**graphite ξ/ξ_fg got *further* from 1, and that is the correct direction.** The
old 1.002 was two errors cancelling: the coarse grid made the kernel **+35 % too
broad at 3.75 eV**, which inflates `ξ = ⟨ln(E/E′)⟩`, and
`examples/graphite_energy_decrement.rs` counts the **whole** thermal channel, so
its coherent-elastic scatters contribute `ln(E/E′) = 0` and dilute ξ by the Bragg
share. At 3.9 eV that share is `0.0665/(4.6739 + 0.0665)` = **1.40 %**, so
`ξ_total ≈ 0.986·ξ_inelastic`; measured 0.987. The inelastic-only ξ agrees with
NJOY's own MF=6/MT=229 to **−0.13 % at 3.75 eV**. The defect had been flattering
that table.

### 4.2 Not re-run, and why

| case | recorded value | why not re-run |
|---|---|---|
| ICSBEP LEU-COMP-THERM-008 case 1 | k = 1.02950 ± 0.00061, 2026-09-11, 10 000 × [250 + 400] | 4 M active histories of thermal **water** transport — several CPU-hours on this four-core host, which was shared with another agent's run for the whole session. Water's kernel moves more than graphite's (⟨E′⟩/E worst −5.54 % → −1.63 %), so this number should be expected to move and **must not be quoted as current**. |
| ICSBEP HEU-SOL-THERM-009 case 1 | see `examples/hst009_keff.rs` | Same reason. Its gate is against the benchmark k = 1.0000 with a band + 4σ envelope and carries no recorded-drift arm (`recorded_pcm: None`), so nothing fails; the printed figure is nonetheless stale. |
| Thermal UO₂ pin k_inf | 1.39802 ± 0.00652, `tests/openmc_notebooks/pincell.rs` | Already documented in that file as stale for an unrelated reason; this change adds a second. |
| HTR-10 graphite pebble bed, manual 1200 × [10 + 70] bound arm | 1.94020 ± 0.00361 | That run is not what the test file executes, and re-running it was not affordable in the same pass. The free-gas arm of the same comparison, 1.93656 ± 0.00316, is unaffected. |
| `verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md` | the ring-RPT k table, incl. 1.40757 ± 0.00224 (CSG) | That document belongs to the ring-RPT residual work in flight in the same working tree. Its CSG row's new value is the first row of §4.1; updating the document itself is left to whoever owns it, rather than editing a file another agent is actively working in. |

---

## 5. Files whose recorded numbers this change touches

| file | what it records | state |
|---|---|---|
| `src/vv/njoy_golden.rs` | `GRAPHITE_KERNEL_WIDTH`, `GRAPHITE_KERNEL`, `GRAPHITE_MUBAR`, `H2O_KERNEL`, `H2O_KERNEL_WIDTH` | updated, superseded columns kept |
| `tests/thermal_laws_vs_njoy_thermr.rs` | the kernel gates | updated + a new H₂O width gate |
| `examples/graphite_sab_angle_and_width_vs_njoy_thermr.rs` | the same gates, re-measurable against a live tape | updated |
| `examples/thermal_emission_grid_convergence.rs` | the sweep that sized both constants | new |
| `examples/thermal_kernel_keff_worth.rs` | the k-worth measurement | new |
| `verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md` | the ring-RPT k table | see §4 — that document is owned by the ring-RPT work in flight |
