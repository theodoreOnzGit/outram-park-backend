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

## 3.1 Independent confirmation: the kernel's own fixed point

A second agent working in the same tree added
`tests/thermal_kernel_stationary_distribution.rs` on the same day — a random
walk in energy driven by the S(α,β) law alone, run from a hot start (40 kT) and
a cold one (0.2 kT), reduced to the **effective temperature of the equilibrium
spectrum** it settles on and to its shape `⟨E²⟩/⟨E⟩²` (5/3 for a Maxwellian).
That is a different question from either moment measured above: a kernel can
have the right per-collision spread and still equilibrate at the wrong
temperature, and nothing in this crate had asked it.

Measured 2026-09-12 by running that test against both tabulations, same
estimator, same streams:

| quantity | 48 × 16 | 384 × 64 | nominal |
|---|---|---|---|
| graphite `T_eff` | 614.28 K (**+2.38 %**) | **607.21 ± 0.89 K (+1.20 %)** | 600 K |
| graphite shape | 1.5652 (−6.09 %) | **1.6286 (−2.29 %)** | 1.6667 |
| H₂O `T_eff` | 291.14 K (**−0.84 %**) | **294.48 ± 0.23 K (+0.30 %)** | 293.6 K |
| H₂O shape | 1.5806 (−5.17 %) | **1.6308 (−2.15 %)** | 1.6667 |
| free-gas C-12 control | 601.28 ± 0.63 K | 601.28 ± 0.63 K | 600 K |

**The free-gas row is the control and it is identical to the digit** — that arm
never touches the S(α,β) tables, so the estimator and the random stream are
demonstrably unchanged and the only thing that moved is the tabulation.

The resize **halves** graphite's fixed-point error and cuts water's by a factor
of ~3, and takes the equilibrium spectrum from 5–6 % away from a Maxwellian
shape to ~2 %. It is a confirmation from a direction none of the oracle
comparisons above look from.

It also sizes what is left. Graphite still equilibrates **+1.20 % hot**, which
is the same residual the ξ and width numbers report from their own angles: the
equiprobable outgoing-energy representation truncating the tails, i.e. the open
half of #188. A kernel whose tails are clipped cannot transport the last bit of
up-scatter, and an over-hot fixed point is exactly what that looks like.

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
| graphite-moderated k_inf, C/HM = 400, reflective cube, 720 000 active histories | `examples/thermal_kernel_keff_worth.rs` | 1.38976 ± 0.00138 | **1.38605 ± 0.00137** | **−371 ± 194 pcm (1.9σ)** | 2026-09-12 |

### The k-worth, decomposed by dimension

The pebble cannot resolve this; a homogeneous graphite-moderated `k_inf` medium
at the same temperature can, and it can also say **which dimension** the k comes
from. 4000 particles × [20 + 180] = 720 000 active histories per tabulation,
same medium, same seeds, 2026-09-12:

| tabulation | k_inf | Δ vs 48 × 16 | σ |
|---|---|---|---|
| 48 × 16 (superseded) | 1.38976 ± 0.00138 | — | — |
| 384 × 16 (incident grid only) | 1.38351 ± 0.00146 | **−624 ± 201 pcm** | 3.1σ |
| 48 × 64 (outgoing bins only) | 1.38737 ± 0.00148 | −239 ± 203 pcm | 1.2σ |
| **384 × 64 (current)** | **1.38605 ± 0.00137** | **−371 ± 194 pcm** | 1.9σ |

**The two dimensions are not additive** — −624 and −239 separately, −371
together — which is what should be expected of two errors that pull *opposite
ways* on the kernel width. Fixing only the incident grid over-corrects; fixing
only the bins under-corrects; the physical answer needs both.

So the repair is worth a few hundred pcm in a graphite-moderated thermal
system, in the direction of **lowering** k, and the incident-grid half is
resolved at 3.1σ on its own.

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

## 4.3 What the residual fixed-point displacement is worth in k

Asked by the author of `tests/thermal_kernel_stationary_distribution.rs`
(GitHub #191, bead `op-bo02`): graphite still equilibrates **+1.20 % hot**, and
the ring-RPT record localises its whole +4004 pcm residual to the fraction of
neutrons crossing 0.625 eV — which is the kind of error a wrong-temperature
fixed point is. So: what is 1.20 % of fixed point worth?

**This is an extrapolation from one measured pair, not a measurement, and it is
labelled as such.** On the same graphite-moderated medium:

```text
  48x16  ->  384x64:   T_eff  614.28 K -> 607.21 K   (-7.07 K, -1.18 points)
                       k      1.38976  -> 1.38605    (-371 +/- 194 pcm)
```

so the local slope is **≈ 52 pcm per kelvin of fixed-point displacement**, or
≈ 315 pcm per 1 %. Carrying that linearly onto the remaining +7.21 K puts the
residual at roughly **−380 pcm** on this medium if it were removed, and — scaling
by the ratio of the two systems' measured response to the same change
(−199 pcm on the FHR ring-RPT CSG pebble against −371 here, i.e. the pebble is
about half as sensitive) — roughly **−200 pcm on that pebble**.

Three caveats, all load-bearing:

1. **The slope is not clean.** That Δk bundles the fixed-point repair with the
   *width* repair; this experiment cannot separate them, so attributing all
   371 pcm to the fixed point is an over-attribution, i.e. −380 pcm reads as an
   upper bound rather than a central value.
2. **One pair is not a slope.** Nothing here establishes linearity.
3. **Direction is right, magnitude is not.** The sign is the useful part: the
   code's fixed point is *hot*, a hot fixed point *raises* k in this medium, and
   the ring-RPT residual is this code reading **high** by +4004 pcm. So removing
   the residual moves k the right way — by something of order **5 % of that
   residual**, not by the residual. It is the right *shape* of error and the
   wrong *size*, and both halves of that sentence need saying.

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
