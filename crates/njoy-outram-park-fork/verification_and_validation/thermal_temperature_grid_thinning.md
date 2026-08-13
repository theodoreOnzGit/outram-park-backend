# Temperature-grid thinning cost — graphite thermal scattering (ENDF/B-VIII.0)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Generated:** 2026-08-13 (Asia/Singapore)
**Crate:** `njoy-outram-park-fork` v0.0.1, branch `develop`
**Producer:** `examples/temperature_thinning_study.rs`
**Regression gate:** `tests/thermal_temperature_thinning.rs` (8 tests, 0.49 s)
**Machinery:** `src/thermr/temperature_thinning.rs`

---

## Question being settled

`tsl-crystalline-graphite.endf` is 8 730 804 B. Measured from the tape on
2026-08-13:

| Section | records | bytes | share |
|---|---:|---:|---:|
| MF=7/MT=4 — incoherent inelastic `S(α,β)` | 114 411 | 8 695 236 | **99.59 %** |
| MF=7/MT=2 — elastic | 419 | 31 844 | 0.36 % |
| MF=1/MT=451 + tape overhead | 49 | 3 724 | 0.04 % |

(Each ENDF text record is 75 columns + newline = 76 B; 8 730 804 / 114 879 =
76.000 exactly.)

The elastic channel nevertheless carries ~90 % of graphite's thermal cross
section — 4.5514 b vs 0.4864 b at 0.0253 eV / 296 K. The bytes and the physics
sit in different places, so the obvious low-fidelity option — **keep only a few
of the ten tabulated temperatures (296, 400, 500, 600, 700, 800, 1000, 1200,
1600, 2000 K) and interpolate between them** — has to be measured.

## Methodology

The evaluation is its own oracle, so the measurement is exact and needs no
external data. For a candidate thinned grid, each **withheld** tabulated
temperature is reconstructed by interpolating from the two kept temperatures
that bracket it — through the *same* `interp_s_temperature` kernel the
production reader uses, under the evaluation's own ENDF `LI` law — and compared
against the row the evaluation actually tabulates there.

Four comparison levels:

1. `S(E)` at every Bragg edge. Because `σ_coh(E,T) = S(E,T)/E` and the edge
   energies are temperature-independent, the relative error in `S` at an edge
   **is** the relative error in `σ_coh` at that energy. Reported over the whole
   221-edge table and restricted to E ≤ 0.0253 eV.
2. `S(α,β)` over all 400 β × 150 α = 60 000 cells, unfiltered and above a
   `10⁻⁶ × S_max` significance floor.
3. `σ_inel(E)` at E ∈ {0.001, 0.005, 0.0253, 0.1, 0.5, 3.9} eV, with the
   reference and reconstructed tables run through the *same* production kernel
   at the *same* physical temperature — so the only difference is the table.
4. `σ_total(0.0253 eV) = σ_coh + σ_inel`, both channels thinned on one grid.
   **This is the decision number.**

Two extras: **leave-one-out** on the full grid (drop one interior temperature,
keep its immediate neighbours), which characterises the accuracy of the
*existing production* interpolation; and a **log-space** run (`LI = 4`,
`ln S` linear in `T`) against the elastic channel's stated `LI = 2`, testing the
prior that Debye-Waller suppression is roughly exponential in temperature.

**Candidate grids, fixed before any error was measured** (each keeps 296 K,
which structurally carries the shared `α` grid, and 2000 K, the top of the
range — dropping it would mean extrapolating):

| | kept temperatures [K] |
|---|---|
| A | 296 / 600 / 1200 / 2000 |
| B | 296 / 800 / 2000 |
| C | 296 / 500 / 1000 / 2000 |
| D | 296 / 400 / 600 / 1000 / 2000 |
| E | 296 / 400 / 500 / 600 / 1000 / 2000 |

## Data provenance

- **Evaluation:** ENDF/B-VIII.0 thermal-scattering sublibrary,
  `tsl-crystalline-graphite.endf`, MAT 30, ZA 130 — LEIP Laboratories /
  A. I. Hawari, Y. Zhu, J. L. Wormald, *Nuclear Data Sheets* **148**, 1 (2018);
  EVAL-SEP17, DIST-FEB18, rev 1449 (2018-01-17). Open, per `DATA_POLICY.md`.
- **Not committed.** The tape is read from `GRAPHITE_TSL_DIR` (env override) or
  `/home/teddy0/Documents/research/ENDF-B-VIII.0/thermal_scatt`; every test and
  the example **skip with a printed note** when it is absent, so the suite
  stays green on a machine without the data.
- **Structure read off the tape** (not assumed): elastic `LI = 2` on all nine
  intervals, 221 Bragg edges; inelastic `LI = 4` on all nine, `LAT = 1`,
  400 β × 150 α, of which 9.8–11.1 % of cells are exactly zero
  (LEAPR-floored); max tabulated `S` = 3.979191.

```bibtex
@article{hawari2018graphite,
  author  = {Hawari, A. I. and Zhu, Y. and Wormald, J. L.},
  title   = {Thermal Neutron Scattering Data for Crystalline Graphite
             (ENDF/B-VIII.0 thermal sublibrary, MAT 30)},
  journal = {Nuclear Data Sheets},
  volume  = {148},
  pages   = {1},
  year    = {2018}
}
```

---

## Results

### 1. Coherent elastic — the error is concentrated where it does not matter

Leave-one-out on the full grid, max relative `S(E)` error:

| Withheld | from | all 221 edges, LI=2 | E ≤ 0.0253 eV, LI=2 | all edges, LI=4 | E ≤ 0.0253 eV, LI=4 |
|---|---|---:|---:|---:|---:|
| 400 K | 296/500 | 4.51 % | 0.0375 % | 1.99 % | 0.0409 % |
| 500 K | 400/600 | 3.13 % | 0.0614 % | 1.42 % | 0.0161 % |
| 600 K | 500/700 | 2.35 % | 0.0752 % | 1.08 % | 0.0060 % |
| 700 K | 600/800 | 1.82 % | 0.0832 % | 0.8397 % | 0.0049 % |
| 800 K | 700/1000 | 2.64 % | 0.1750 % | 1.26 % | 0.0225 % |
| 1000 K | 800/1200 | 4.03 % | 0.3658 % | 1.85 % | 0.0634 % |
| 1200 K | 1000/1600 | 5.06 % | 0.7140 % | 2.43 % | 0.1563 % |
| 1600 K | 1200/2000 | 7.05 % | 1.42 % | 3.17 % | 0.3456 % |

The worst whole-table errors sit at Bragg edges of 0.37–1.28 eV (up to the
table's 5 eV top), where `σ_coh = S/E` is two decades below thermal: high-`Q`
reflections carry the largest Debye-Waller exponents and therefore the
strongest curvature in `T`. Inside the thermal window the same interpolation is
**50–100× more accurate**.

Thinned grids, `σ_coh(0.0253 eV)` relative error:

| Grid | worst over withheld points in 293–1000 K | worst anywhere |
|---|---:|---:|
| A | 0.74 % (1000 K) | 1.42 % (1600 K) |
| B | 1.65 % (1000 K) | 3.02 % (1600 K) |
| C | 0.52 % (800 K) | 2.20 % (1600 K) |
| D | 0.35 % (800 K) | 2.20 % (1600 K) |
| E | 0.35 % (800 K) | 2.20 % (1600 K) |

Elastic thinning is effectively free — and also pointless, since the elastic
section is 0.36 % of the tape.

### 2. Log space *is* materially better for the elastic channel

`LI = 4` (log-lin) beat the stated `LI = 2` on the whole table at **all eight**
leave-one-out points, by a factor of 2.08–2.27×. In the thermal window it wins
from 500 K up (0.0614 → 0.0161 % at 500 K; 1.42 → 0.3456 % at 1600 K) and is a
wash at 400 K (0.0375 → 0.0409 %, marginally worse).

This confirms the physical prior: Debye-Waller suppression is close to
exponential in `T`, so `ln S` is nearer linear in `T` than `S` is.

> **Reported, not applied.** ENDF/B-VIII.0 states `LI = 2` for this section and
> the production path keeps it. Log-space interpolation is a candidate for a
> *low-fidelity* mode only, and would need its own V&V.

### 3. Incoherent inelastic — much larger errors, small weight

`σ_inel` max relative error over E ≤ 0.1 eV, across each grid's withheld points
in 293–1000 K: **A 8.8–14.3 %, B 14.9–27.1 %, C 8.6–13.5 %, D 5.1–7.5 %,
E 6.2–7.5 %**. Over all six test energies (to 3.9 eV) the maxima roughly
double, and the worst point is 3.9 eV for **every** grid and **every** withheld
temperature.

Leave-one-out on the *full* grid still gives 4.60–18.52 % RMS over the six
energies, so most of this is the intrinsic difficulty of interpolating
`S(α,β)` in temperature — not the thinning.

`S(α,β)` table-level error, grid A at 400 K: **68.45 % max / 26.12 % RMS** over
the 34 098 cells above the `10⁻⁶ × S_max` floor (25 902 skipped). Unfiltered,
the max is meaningless (`10¹³ %` at `α = 11.8, β = 188`, where the reference is
3.06 × 10⁻⁶⁰) — which is exactly why the floored figure is the one to read.

**Reconstruction consistency check.** Holding the physical temperature at 400 K
and swapping only the table, `σ_inel(3.9 eV)` is 0.7228 b with the 296 K table,
858.63 b with the 600 K table, 2.0459 b with the interpolated table, and
4.6367 b with the true 400 K table. The interpolated value lies between the two
endpoints at **every** test energy, so the machinery is monotone and the large
errors are a property of the data: at high incident energy the integral samples
the steep far tail of `S(α,β)`, where a modest table error swings `σ` by orders
of magnitude.

A `ZeroPolicy::PreserveZeros` variant (keeping LEAPR-floored cells at zero
through the interpolation, so the short-collision-time fall-through is
preserved) produced **bit-identical** statistics, ruling out zero-endpoint
handling as the mechanism.

### 4. Combined `σ_total(0.0253 eV)` — the decision number

| Grid | withheld in 293–1000 K | worst σ_total error there | worst anywhere | MT=4 bytes | tape saved |
|---|---|---:|---:|---:|---:|
| A 296/600/1200/2000 | 400, 500, 700, 800, 1000 | **3.13 %** (1000 K) | 3.13 % | 3 952 684 | 54.3 % |
| B 296/800/2000 | 400, 500, 600, 700, 1000 | **4.27 %** (600 K) | 7.05 % (1600 K) | 3 162 208 | 63.4 % |
| C 296/500/1000/2000 | 400, 600, 700, 800 | **2.88 %** (800 K) | 4.66 % (1600 K) | 3 952 684 | 54.3 % |
| D 296/400/600/1000/2000 | 500, 700, 800 | **1.72 %** (800 K) | 4.66 % (1600 K) | 4 743 084 | 45.3 % |
| E 296/400/500/600/1000/2000 | 700, 800 | **1.72 %** (800 K) | 4.66 % (1600 K) | 5 533 484 | 36.2 % |

Per-point detail for the two extremes:

```csv
grid,T_K,sigma_coh_ref_b,sigma_coh_thin_b,coh_err_pct,sigma_inel_ref_b,sigma_inel_thin_b,inel_err_pct,sigma_total_ref_b,sigma_total_thin_b,total_err_pct
A,400,4.3731,4.3771,0.0907,0.6960,0.6107,12.26,5.0691,4.9878,1.60
A,500,4.2049,4.2095,0.1086,0.9028,0.8069,10.62,5.1077,5.0164,1.79
A,700,3.8849,3.9010,0.4152,1.3173,1.2122,7.98,5.2022,5.1132,1.71
A,800,3.7344,3.7601,0.6908,1.5208,1.3455,11.53,5.2552,5.1056,2.85
A,1000,3.4529,3.4785,0.7393,1.9152,1.7216,10.11,5.3682,5.2000,3.13
A,1600,2.7529,2.7919,1.42,2.9795,2.7809,6.66,5.7324,5.5729,2.78
D,500,4.2049,4.2075,0.0614,0.9028,0.8630,4.40,5.1077,5.0705,0.7276
D,700,3.8849,3.8946,0.2509,1.3173,1.2440,5.56,5.2022,5.1386,1.22
D,800,3.7344,3.7474,0.3490,1.5208,1.4176,6.79,5.2552,5.1650,1.72
D,1200,3.1968,3.2398,1.35,2.2901,2.0873,8.86,5.4869,5.3271,2.91
D,1600,2.7529,2.8134,2.20,2.9795,2.6520,10.99,5.7324,5.4655,4.66
```

The dilution is the point: grid A at 400 K has a **12.26 %** inelastic error
that becomes a **1.60 %** total error, because inelastic is only ~14 % of the
thermal total there.

**Every candidate grid meets a 5 % criterion across 293–1000 K. None meets
1 %** — the best (D and E) reach 1.72 %. Under a 2 % criterion, D (45.3 %
saved) and E (36.2 % saved) pass and A, B, C do not.

### 5. A pre-existing defect found on the way — NOT a thinning result

`σ_inel(E, T)` is monotone in `T` across the tabulated grid (4.60966, 4.63671,
4.65439, … b at 3.9 eV), so an interpolated temperature whose `σ` falls outside
its tabulated bracket is a defect, not an approximation error. The
**production** path — `parse_mf7_at_temperature`, adjacent tabulated
temperatures, the evaluation's stated `LI = 4` — leaves the bracket above
~0.5 eV:

| T | 0.001–0.1 eV | 0.5 eV | 3.9 eV |
|---|---|---|---|
| 393.15 K | in bracket | 3.9488 ∈ [3.7915, 3.9978] ok | **4.4175 ∉ [4.6097, 4.6367]** |
| 523.15 K | in bracket | **4.1217 ∉ [4.1328, 4.2303]** | **4.4784 ∉ [4.6544, 4.6672]** |
| 900 K | in bracket | **4.3471 ∉ [4.3614, 4.4459]** | **4.4549 ∉ [4.6823, 4.6913]** |
| 1400 K | in bracket | **4.4635 ∉ [4.5057, 4.5866]** | **4.4279 ∉ [4.6927, 4.7006]** |

About 4–5 % low at 3.9 eV, ≲ 1 % low at 0.5 eV, correctly bracketed at and
below 0.1 eV. This affects **every non-tabulated temperature request today**,
independent of any thinning decision — including the HTR-10 benchmark points
393.15 K and 523.15 K that `tests/thermal_graphite_coherent.rs` already
exercises (that test checks 0.0253 eV only, where the behaviour is correct).

Reported here, not fixed: the fix is a separate change needing its own V&V.

---

## Interpretation and recommendation

- **Thinning is viable for HTR-10's 293–523 K range**, at a cost of 1.6–1.8 %
  in `σ_total(0.0253 eV)` for the aggressive 4-temperature grid A, and under
  1 % for grids that keep a point in the low 400s.
- **The best trade is to thin only above 600 K.** Grid E
  (296/400/500/600/1000/2000) keeps every tabulated point HTR-10 operates
  between, so its reconstruction over 296–600 K is *identical to the production
  path* — zero added error in the band that matters — while still removing
  **36.2 %** of the tape. Grid D (296/400/600/1000/2000) removes **45.3 %** for
  a worst-case 1.72 % below 1000 K.
- **Keep the elastic section in full.** It is 0.36 % of the tape, carries 90 %
  of the thermal cross section, and thinning it saves nothing.
- **The error is concentrated, not spread out.** Elastic: at high-`Q` Bragg
  edges above ~0.4 eV. Inelastic: at high incident energy — 3.9 eV is the worst
  point for every grid at every withheld temperature — and, in the table, at
  large `α` and `β`. Both regions are outside the thermal window that drives
  HTR-10, which is why the thermal-window numbers are an order of magnitude
  better than the whole-table numbers.
- **A 1 % criterion is not reachable by thinning at all** on this evaluation,
  because even the *full* grid's own interpolation is only good to ~5–19 % on
  `σ_inel` at leave-one-out spacing.

## What was NOT measured

- **No independent oracle.** Every figure here is the evaluation against
  itself. Nothing is compared against NJOY2016's own output, an ACE file, or a
  criticality benchmark, so this **does not validate** the underlying THERMR
  port — it quantifies one approximation within it.
- **No `k_eff` impact.** A 1.7–3 % `σ_total` error has not been propagated
  through a transport calculation. The reactivity worth of these errors is
  unknown, and 1 % in a moderator cross section is not automatically 1 % in
  reactivity.
- **293.15 / 393.15 / 523.15 K have no ground truth.** They are not tabulated,
  so the withheld 400 K and 500 K points are *proxies*; the true error at
  393 K on a thinned grid is bounded by, not equal to, the 400 K figure.
- **MAT 30 only.** `tsl-reactor-graphite-10P` (MAT 31) and `-30P` (MAT 32) have
  the same ten temperatures and an identical 114 411-record MT=4 section, but
  were not run.
- **Integrated cross sections only.** Angular distributions, the equiprobable
  emission tables, and the ACE ITXE/ITCA blocks a thinned library would
  ultimately feed were not compared.
- **Byte figures are ENDF text bytes** from an exact record model (validated to
  the byte against the real MT=4 section). A baked binary blob would have a
  different, smaller absolute size; the *fractions* carry over.

## Reproducing

```bash
cargo run --release -p njoy-outram-park-fork --example temperature_thinning_study
crates/njoy-outram-park-fork/scripts/test.sh thinning -- --nocapture
```
