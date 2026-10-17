# Route 4 on njoy's `process`: what moved (GitHub #931)

> Research, education and V&V only. AI-generated (Claude Opus 5.5 in Claude
> Code), not yet human-reviewed.

**Class: verification** (a data path replaced; the old one kept as a named
ablation and measured against the new one) plus the benchmark comparison
that goes with route 4.

## What changed

`Nuclide::process_evaluation`, route 4 of the five-route ICSBEP study and the
first half of every `Nuclide::from_tape` / `from_endf_file`, is now a thin
call into `process::reconstruct` + `process::cross_sections_at` at
`process`'s defaults (maintainer decision 2026-10-17 on #929/#931, option
(a)):

| | before (`process_evaluation_pre_931`, kept as an ablation) | now (`process_evaluation`) |
|---|---|---|
| PURR | 20 bands / 16 ladders / 2 000 samples | 20 / 64 / 10 000 |
| HEATR | not run; PURR's heating column zero | run before PURR, as the NJOY deck |
| broadened sections | BROADR, PENDF-rounded inside BROADR | the same, plus `process`'s own PENDF rounding (idempotent: #929 P1, 0 of 5 067 756 points differ) |
| 0 K elastic for DBRC | RECONR's raw MT=2 to 25 keV | the whole range, PENDF-rounded (the nuclide keeps ≤ 25 keV; DBRC uses ≤ 1 keV) |

The browser demos (`dhoby-ghaut`'s `processed_cache.rs`, the HTR-10 pool
bake) stay on the pre-#931 chain by name until #932 moves them; their cache
keys and code version are unchanged.

## Predictions (written 2026-10-17 before any k run)

**Data first.** `examples/route4_purr_table_shift.rs` compares the two
products' PURR tables at every URR energy (293.6 K, ENDF/B-VIII.0, i9-13900K,
2 cores via `taskset -c 14-15`, shared machine). New/old − 1, mean over the
energies / RMS:

| nuclide | energies | load pre-931 [s] | load now [s] | inf.-dilution capture | fully shielded capture | inf.-dilution fission | fully shielded fission |
|---|---|---|---|---|---|---|---|
| U-234 | 26 | 2.7 | 14.7 | −2.9e-8 / 1.0e-7 | −1.5e-3 / 5.9e-3 | −2.1e-8 / 1.0e-7 | −1.6e-3 / 8.3e-3 |
| U-235 | 19 | 42.5 | 89.6 | +4.6e-8 / 1.7e-7 | −4.9e-4 / 1.3e-3 | +5.9e-9 / 1.3e-7 | −6.0e-4 / 1.7e-3 |
| U-238 | 83 | 38.5 | 69.1 | −2.2e-8 / 1.1e-7 | +3.0e-4 / 1.7e-3 | 0 / 0 | 0 / 0 |

("Fully shielded" is the σ0 → 0 Bondarenko limit, `Σ p c/t / Σ p/t`: the most
a table change can matter. Infinite dilution is preserved to 1e-7, as PURR
normalises the bands to it.) The PURR change moves the shielded U-235 and
U-238 factors by ≤ 0.06 % on average, 0.13–0.17 % per energy.

From that, before transport:

- **P1 (PURR, all four cases):** |Δk| well below 5 pcm on every case; sign
  not predictable (the shifts are table sampling noise, mean ≲ 1σ of the
  per-energy scatter). Reasoning: URR self-shielding is worth of order
  10²  pcm in these systems (Jemima: −142 pcm, root `CLAUDE.md`; LCT-008
  URR+DBRC bounded below 154 pcm), and the tables' shielded factors move by
  ~1e-3 of themselves, a few per cent at most of the shielding effect. So
  **not resolvable** at the campaign's 32 seeds (±30 pcm) or at any seed
  count affordable here; the measurement is a bound. Jemima (U-238
  reflector, softest metal spectrum) is where it would show first; Godiva
  least. Against the caller's expectation that LCT-008 and HST-009 matter
  most: their URR flux fraction is small (thermal), and the measured table
  shift gives them no lever, so I predict them as small as the metal cases.
- **P2 (HEATR):** bit-identical k. HEATR fills only PURR's heating column
  (`purr::tables::from_endf_with_mt301`) and draws no random number;
  transport never reads heating. Tested at the table level by
  `tests/route4_is_process.rs` (only heating words differ).
- **P3 (pairing):** a same-seed pair (old, new) decorrelates at the first
  URR collision, so pairs carry no correlation; the comparison is unpaired.
- **P4 (reproduction):** the pre-#931 ablation reproduces the 2026-10-15
  campaign's route-4 per-seed k to every printed digit.

**Honesty note:** P2, P3 and P4 were written after a 4-seed Godiva smoke
run (seeds 1–4, all three arms) had already shown them; they are
predictions for the other cases and seeds only. P1 was written before any
transport run of the new data's *mean*.

## Methodology

(Filled in with the runs.)

## Results

(Filled in with the runs.)
