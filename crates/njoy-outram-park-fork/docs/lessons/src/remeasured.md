# Re-measurement log, 2026-10-04

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response.

The lesson philosophy asks that a record older than a change that could move
it be **re-measured before it is quoted**. The largest such change for this
crate is GitHub #340 (2026-09-26), which made RECONR's and BROADR's energy
grids NJOY2016's word for word; many records predate it. These are the runs
made for this book.

**Settings for every run below.** `develop` at `3c41d99f5b` plus this track's
doc-comment-only edits (no executable change); `--release`; two cores
(`taskset -c 10-11`, `RAYON_NUM_THREADS=2`, `-j 2`); the crate's 12 GB
address-space cap for tests. Wall times include compiling. Where a result
moved, the source doc comment now carries the new value and strikes through
the old one.

| record | what was re-run | result 2026-10-04 | moved? | wall |
|---|---|---|---|---|
| `examples/endf_to_broadened_xs.rs` (U-238, 293.6 → 900 K) | example | 52 reactions; MT=1 at 293.6 K 103 908 points; area +0.000 %; peak 13 449.77 → 8 464.64 b (−37.06 %); valley at 2.5438 keV 0.5004 → 1.9161 b (**+282.94 %**) | **yes**: valley was +253.44 % (2026-09-11), grid-sensitive, moved by the #340 grid; doc updated | 1 037 s |
| `examples/tutorial_resonance_to_groups.rs` (U-238 resonance integrals) | example | RI∞ 274.65 b vs ~~published~~ 275.7 b (−0.382 %; that reference is **not re-checked**, no source found, [#547](https://github.com/theodoreOnzGit/outram-park-backend/issues/547), so this is a coarse bound, not evidence); spread 0.0094 %; RI(σb=60) +29.0 %, RI(σb=20) +23.3 % over 0–1200 K | slightly: shielded integrals moved ≤ 0.2 % (were +28.9 %, +23.1 %); doc updated | 249 s |
| `examples/graphite_sab_generation.rs` | example | MT=4 1.002e-13 max, 9.993e-14 rms over 48 941 points; 6 645 / 60 000 bit-identical; 221 Bragg points to 1.0e-13 | no (was 1.004e-13 / 9.991e-14) | 74 s |
| `tests/reconr_lrf7_threshold_channels_vs_njoy2016.rs` (Fe-57, Mo-95) | test | Fe-57 82 sections, 17 213 / 17 213 points, **0** differing words; Mo-95 84 sections, 34 939 / 34 939, **0** | no | 76 s |
| `tests/pendf_stages_vs_njoy2016.rs` (12 PENDFs) | test | all 8 tests pass: every MF=3 word identical for RECONR Si-30, Sr-88, Ar-37, U-234 and BROADR H-2, Li-6, Be-9, C-12, F-19, Si-30, Cl-35, Ar-37 | no | 87 s |
| `tests/thermr_calcem_vs_njoy2016_golden.rs` (graphite, 600 K) | test | worst σ −4.42e-7 relative at 0.0253 eV; worst μ̄ −5.0e-6 at 0.0115 eV | no | 9 s |
| `tests/leapr_h2o_njoy_oracle.rs` (H in H₂O, 293.6 K) | test | 44 961 S(α,β) points, worst 1.002e-13; `T_eff` 1194.3410 K (NJOY 1194.341) | no | 9 s |
| `tests/dtfr_u238_claw_*_golden.rs` | 2 tests | worst 4.546e-6 to 4.927e-6 per table (gate 2e-5) | **first recorded result** (the docs stated none); docs updated | 1 s each |
| `tests/mixr_h2_be9_njoy_golden.rs` | test | 787 / 787 points per MT, worst difference 0.0 | **first recorded result**; doc updated | 1 s |
| `tests/resxsr_h2_njoy_golden.rs` | test | 2 124 / 2 124 bytes identical (and 3 508 at two temperatures) | no | 1 s |
| `tests/wimsr_u238_njoy_golden.rs` | test | 185 / 185 library lines identical | no | < 1 s |
| `tests/covr_boxer_golden.rs` | test | every tier-2 BOXER line byte-identical (e.g. Be-9 395 / 395, F-19 516 / 516) | no | 2 s |
| `tests/errorr_mf33_golden.rs` | test | tier 1 worst ≤ 4.8e-7 on every compared block; **U-234 and U-238 tier 1 skipped** (they need NJOY's 293.6 K PENDF in an environment variable) | no | 3 s |
| `tests/gaspr_vs_njoy2016.rs` | test | worst **4.6e-7** over all 17 sections (B-10 MT=207) | **yes**: was 4.9e-3 (2026-09-17), grid interpolation removed by #340; test doc and V&V record updated | < 1 s |
| `tests/heatr_vs_njoy2016.rs` | test | Fe-58 capture deficit 2.04–2.08e5 eV (3.164 % of Q); Si-28 ratio 1.000000 below 100 eV; damage mean +0.16 % (Fe-58), +0.31 % (Si-28) | no | 3 s |
| `tests/gaminr_vs_njoy2016.rs` | test | synthetic: worst 4.35e-7; real U: vectors ≤ 2.73e-7; MF=26 MT=502/504 **still disagree** (printed, not asserted) | no (#534; ~~open~~ resolved 2026-10-05, see the 2026-10-05 table below) | 5 s |
| `tests/unr_block_write_vs_njoy2016.rs` (PURR bands, ACE UNR block) | test, with the main checkout's `reference-data/ace` | U-234 / U-235 / U-238: **0** of 3 152 / 2 305 / 10 049 words differ, both read-write and generated bands; energy grids 26 / 19 / 83 points, worst 1.0e-13 | no | 236 s |
| `examples/seam_stage_probe.rs` (U-238 seam at 600 K, `--features urr-diagnostics`) | example | all 6 gates pass: bounded +0.000 % above the seam; unbounded −46.73 % at 2.000001e4 eV, MT=18 19.6 %, (n,2n) leak 1.009e-6 b | **partly**: unbounded no longer 13 % low at 23 keV (agrees from 20.5 keV); cause not established; doc updated | 420 s |

**From the demo's engine** (`crates/dhoby-ghaut/examples/nuclear_data_web`,
`--headless <rung>`, same settings): U-238 RECONR at tolerances 0.3, 0.1,
0.03, 0.01, 0.003 and 0.001 gives 54 178, 69 601, 105 735, 165 759, 277 279
and **448 168** points (MT=1), the last equal to NJOY2016's own count, in 2.8
to 20.8 s; BROADR of the 0.01 grid over 0.5–600 eV keeps the area under
capture (5–200 eV) within 0.007 % at 294, 900 and 2500 K; PURR at 8 ladders ×
2000 samples gives band factors whose probability-weighted mean is within
2.9e-4 of 1 at all 83 energies; the 30-group capture resonance integral over
1 eV–10 keV is 273.51 b infinitely dilute and 18.42 b at σ₀ = 50 b.

