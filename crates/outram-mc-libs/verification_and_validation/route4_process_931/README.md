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

- **Instrument:** the campaign's own driver,
  `examples/icsbep_five_route_keff.rs --route endf`, with the new
  `--route4-data` switch:
  - `process` is route 4, and is the default;
  - `pre931` is the old chain;
  - `process-without-heatr` is the HEATR ablation.
  
  Every other setting is the campaign's (`run_all.sh` defaults): ENDF/B-VIII.0, 293.6 K, tolerance 1e-3,
  Godiva / Jemima / HST-009 at 5000 × [40 + 120], LCT-008 (35 nuclides,
  label `lct008-full`) at 10 000 × [40 + 120], DBRC ≤ 1 keV, S(α,β) H in
  H₂O from LEAPR.
- **Code:** worktree at `27213c16e6` (develop `21a93d9460` plus this
  change); the first arms ran from the same tree before the commit
  (labelled `21a93d9460+wip`, identical code).
- **Seeds:**
  - Godiva: 512 per arm.
  - Jemima: 128 per arm.
  - HST-009: 96 per arm.
  - LCT-008: 32 new, against the record's 32.

  For seeds 1–32 the old arm is the 2026-10-15 campaign record
  (`icsbep/five_route_keff/data/per_seed_keff.csv`, label `route4`), after
  checking that `pre931` reproduces it (below). Seeds above 32 were run on
  both arms here.
- **Statistics:** the sem of the per-seed k (sd/√n). The difference new −
  old is quoted **unpaired** (σ = √(sem₁² + sem₂²)). Pairs decorrelate at
  the first URR collision (P3), so the paired σ is given only alongside.
- **Control for the mechanism:** route 4's data with only the URR tables
  rebuilt at 16 / 2000 (`--urr-ladders 16 --urr-samples 2000`; same PURR
  routine, no HEATR). If everything except PURR's ladder and sample counts
  is transport-identical, this arm must give the old k to the bit.
- **Hardware:** 13th Gen Intel Core i9-13900K, 2 of 32 logical CPUs
  (`taskset -c 14-15`, 2 threads), 62.5 GiB, Linux 7.2.9, CPU only, shared
  with other agents.
- **Data:** every row is in `per_seed_keff_2026_10_18.csv`. The `route`
  column is `route4` (new), `route4-pre931`, `route4-no-heatr` or
  `route4-urr16x2000`.

## Results (2026-10-18)

**Reproduction (P4, held).** `pre931` reproduces the recorded route-4 k to
every printed digit on every seed tried: Godiva 32/32, Jemima 32/32,
HST-009 seed 1, LCT-008 seed 1. So the record is the old arm.

**HEATR (P2, held).** `process-without-heatr` gives route 4's k **bit for
bit**: Godiva seeds 1–4 and HEU-SOL-THERM-009 seeds 1–2 (k and its
internal σ identical in every printed digit). The table-level test shows
why: HEATR changes only PURR's heating column, which transport does not
read.

**Mechanism control (held).** Route 4's data with 16 / 2000 URR tables give
HEU-SOL-THERM-009 seeds 1 and 2 as 0.998204 and 1.001943. The record has
the same values, digit for digit. So the PENDF rounding, the whole-range
0 K elastic and HEATR together move no history. **Every difference below
is PURR's ladder and sample counts.**

**Route 4, old → new** (k ± sem; Δ in pcm):

| case | seeds (old / new) | old route 4 (record, 32 seeds) | old arm, all seeds | new route 4 | new − old (same seeds) | new − route 1 | new − benchmark | P1 (≪ 5 pcm) |
|---|---|---|---|---|---|---|---|---|
| Godiva | 512 / 512 | 0.99970 ± 0.00028 | 1.00001 ± 0.00008 | **1.00001 ± 0.00008** | **−0.2 ± 11.2** (paired ± 8.8) | −15 ± 22 (was −47 ± 35) | +1 ± 8 (bench ± 100) | holds: bounded \|Δ\| < 34 at 3σ |
| Jemima | 128 / 128 | 0.99609 ± 0.00027 | 0.99638 ± 0.00016 | **0.99624 ± 0.00017** | **−13.7 ± 23.3** (paired ± 19.4) | +31 ± 24 (was +15 ± 32) | −376 ± 17 (bench ± 300) | consistent (0.6σ) |
| HEU-SOL-THERM-009 | 96 / 96 | 1.00072 ± 0.00021 | 1.00116 ± 0.00015 | **1.00076 ± 0.00014** | **−39.6 ± 20.6** (paired ± 16.7) | −48 ± 32 (was −52 ± 36) | +176 ± 14 (bench ± 430) | **in tension: 1.9σ** |
| LCT-008 (35 nuclides) | 32 / 32 | 1.00159 ± 0.00021 | (the record) | **1.00135 ± 0.00023** | **−24.7 ± 31.1** | −26 ± 26 (was −1 ± 25) | +65 ± 23 (bench ± 160) | consistent (0.8σ) |

Benchmarks: Godiva 1.0000 ± 0.0010, Jemima 1.0000 ± 0.0030,
HEU-SOL-THERM-009 case 1 0.9990 ± 0.0043, LEU-COMP-THERM-008 case 1
1.0007 ± 0.0016 (cited in the five-route record). Route 1 is the
2026-10-15 OpenMC-on-NJOY2016 row, 32 seeds.

**Which numbers to quote.**
- **Route 4's k:** use the "new route 4" column. Its seed counts are 512,
  128, 96 and 32. The record's 32-seed route-4 values are now the
  `pre931` ablation's.
- **The size of the move:** use "new − old (same seeds)". "Old route 4
  (record)" against "new route 4" mixes seed sets, so it is not a
  measurement of the change.
  - Godiva: the 32-seed record sat −31 pcm below its own 512-seed mean.
    That was a low draw, as #546 found before.
  - HST-009: the record sat −44 pcm below its own 96-seed mean.

**Interpretation.**
- **Godiva:** the move onto `process` changes route 4 by −0.2 ± 11 pcm,
  bounded below 34 pcm at 3σ. P1 holds. Route 4 is now +1 ± 8 pcm from
  the benchmark and −15 ± 22 pcm from route 1.
- **Jemima and LCT-008:** consistent with zero (0.6σ and 0.8σ), bounded
  below ~84 and ~118 pcm at 3σ.
- **HEU-SOL-THERM-009: −40 ± 21 pcm, 1.9σ (paired 2.4σ), against a
  prediction of ≪ 5 pcm.** This is not resolved, and it is the
  unflattering cell.
  - The three non-Godiva cases all moved down. Weighted together they give
    −28 ± 14 pcm (2.0σ).
  - With four cases tested, one 2σ cell is not unusual. But all three
    pointing the same way is a hint, and it would refute P1's magnitude if
    it held.
  - The table data argue against an effect this size. The measured table
    shift is ≤ 0.06 % in the shielded factors. To give −40 pcm, HEU-SOL-THERM-009
    (a thermal solution, URR flux small) would need a sensitivity tens of
    times larger than any metal case shows.
  - The mechanism control rules out everything except the PURR counts.
  - **Not resolved here.** Settling it needs about 4× the HEU-SOL-THERM-009 seeds
    (~400 per arm, ~8 h at 2 cores), or a per-nuclide split with
    `--urr-ladders` on one nuclide at a time. Neither was run; filed as
    an open question on #931.
- **Against route 1 (the five-route criterion, |Δ| within 2σ):** every new
  route-4 cell is inside it. Godiva −0.7σ, Jemima +1.3σ, HEU-SOL-THERM-009 −1.5σ,
  LCT-008 −1.0σ.
- **Load time** (PURR at 64 × 10 000 plus HEATR). Each pair is old → new
  for the same case, 2 cores, shared machine:
  - Godiva: 76–99 s → 151–169 s.
  - Jemima: 71 s → 375 s.
  - HEU-SOL-THERM-009: 90–152 s → 191–303 s.
  - LCT-008: 102 s → 260 s.

  Per nuclide: U-235 42 → 90 s, U-238 39 → 69 s. The ranges are two runs
  of one case under different machine load. Transport time per seed is
  unchanged.

## Tests that pin this

- `tests/route4_is_process.rs`: route 4 equals `process::cross_sections`
  (20 / 64 / 10 000, HEATR) bit for bit on U-234. That covers 285 041
  section words, 45 977 0 K elastic pairs and 3180 PURR words. Without
  HEATR only the 520 heating words change. The pre-931 tables differ in
  2574 of 2660 non-heating words. 42 s.
- `tests/wasm_route4_runtime.rs`: U-234 route 4 plus `from_processed`
  fingerprint `0x871cdd4d51d206f2` natively, and the **same** under
  wasm32-wasip1 (Node 26.11.1 WASI, 12.6 s). Headless Chromium was not run.
- `examples/route4_purr_table_shift.rs`: the data table above, with a
  hand-checked unit test.
