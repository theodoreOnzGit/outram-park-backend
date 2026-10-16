# LCT-008 on its full nuclide list, all five routes (2026-10-14)

> Research, education and V&V only.

**Data**, merged into `per_seed_keff.csv` as case `lct008-full`, 8 seeds per
route:
- `per_seed_keff_lct008_full_routes123_2026_10_14.csv`: routes 1 and 3;
  route 2's failed attempt left no rows;
- `per_seed_keff_lct008_full_routes45_2026_10_14.csv`: route 4; its route-5
  rows are superseded (see below);
- `per_seed_keff_lct008_full_routes25_fixed_2026_10_14.csv`: routes 2 and 5
  on the corrected Rust-NJOY library.

The summary (`summary_keff.{csv,md}`) and the figure are regenerated from
them.

## Methodology

- **Model:** LEU-COMP-THERM-008 case 1, the lattice, with every nuclide of
  the case-1 cards (35). This is the model since 2026-10-14; the 11-nuclide
  tier is retired to a smoke test.
- **Run size:** 10 000 × [250 + 400], seeds 1–8 on every route, 4 threads.
- **Routes:**

  | route | code | nuclear data | H in H₂O |
  |---|---|---|---|
  | 1 | OpenMC | NJOY2016 ACE | NJOY2016 table from `tsl-HinH2O.endf` (the reference) |
  | 2 | OpenMC | Rust-NJOY ACE | LEAPR (`write_ace_library --thermal-leapr`) |
  | 3 | outram-mc | NJOY2016 ACE | as route 1 |
  | 4 | outram-mc | Rust NJOY, ENDF read directly | LEAPR (`ThermalScattering::from_embedded_leapr_deck`) |
  | 5 | outram-mc | Rust-NJOY ACE | LEAPR, as route 2 |

- **Libraries:** both ACE libraries hold all 36 tapes of `nuclides.tsv`.
  NJOY2016 2016.79 and OpenMC were built in this container. **OpenMC is
  commit `ec3bc73`, not the 0.16.1-dev25 at `d7d3284a1` that ran every
  earlier route-1 and route-2 row.**
- **Host:** Intel Xeon @ 2.80 GHz, 4 logical cores, 16 GB, Linux, CPU only.
- **Speed:** about 265 s per seed on OpenMC and 400–450 s on outram-mc,
  measured on the same host.
- **Predictions, written before each run:**
  - route 1 ≈ 1.00169, the 11-nuclide route 1 less the 24 extra nuclides'
    −75 pcm (#533);
  - route 2 within 2σ of route 1;
  - route 3 ≈ 1.00151;
  - route 4 ≈ 1.00146;
  - route 5 within 2σ of route 4 + 13 pcm.
- **Pass criterion:** every route within 2σ of route 1.

## A defect found by this run, fixed before routes 2 and 5 were recorded

- **What failed:** route 2 segfaulted in OpenMC on every seed.
- **Localised:** by swapping libraries one half at a time between the
  Rust-NJOY and NJOY2016 tables, then nuclide by nuclide. The culprit was
  **Mg-24**.
- **Cause:** Mg-24 gives MT=16, 22, 28 and 91 as MF=4 + MF=5 with no MF=6.
  The Rust ACER built continuum emissions only from MF=6, so it wrote those
  reactions with TYR = 0 (no neutron out). Its NR was 11 against NJOY2016's
  15.
- **Fix:** `98cb2c850`, which follows upstream `acelod`
  (`acefc.f90:5859-5870`, `:5924`). Mg-24 is now NJOY2016's table in every
  word, and it is in `tests/ace_five_route_parity.rs`.
- **Superseded:** route 5's first 8 seeds (1.00175 ± 0.00031) read the
  defective table. They are struck through in
  `lct008_full_routes45_2026_10_14.md`.
- ~~**Still open, #875:** 12 trace nuclides (Na-23, Ti-48, Cr-50/52/53/54,
  Fe-57/58, Cu-65, Zn-64/66/67/68/70) still differ from NJOY2016 in some
  words. OpenMC and outram-mc run on all of them. Their worth in k is
  unmeasured.~~ **CORRECTED 2026-10-15 (#875):** all 74 tables are now
  NJOY2016's in every word. The rows below were taken **before** those fixes
  (routes 2 and 5 on the old Rust ACE; routes 4 and 5 also on the old
  RECONR/PURR for Ti-48, Zn and Fe-58) and have not been re-measured. The
  fixes touch clad trace nuclides only, so a shift beyond the rows' ±20-25
  pcm is not expected, but it has not been measured.

## Results

| route | k ± sem | seed sd [pcm] | prediction | vs route 1 [pcm] | vs benchmark 1.0007 [pcm] |
|---|---|---|---|---|---|
| 1 | 1.00168 ± 0.00023 | 64 | 1.00169 | reference | +98 ± 23 |
| 2 | 1.00163 ± 0.00024 | 68 | within 2σ of route 1 | −6 ± 33 (−0.2σ) | +93 ± 24 |
| 3 | 1.00134 ± 0.00019 | 54 | 1.00151 | −34 ± 30 (−1.2σ) | +64 ± 19 |
| 4 | 1.00118 ± 0.00019 | 54 | 1.00146 | −50 ± 30 (−1.7σ) | +48 ± 19 |
| 5 | 1.00137 ± 0.00025 | 70 | route 4 + 13 | −31 ± 34 (−0.9σ) | +67 ± 25 |

**Interpretation:**
- **Every route is within 2σ of route 1**, and each meets its prediction.
- **The extra nuclides' worth:** on route 1, the full model sits 76 ± 23 pcm
  below the 11-nuclide model (1.00244 ± 0.00005). That agrees with #533's
  −75 ± 14 on route 4.
- **Rust NJOY + LEAPR versus NJOY2016, same code:**
  - route 2 against route 1 (OpenMC) is −6 ± 33 pcm;
  - route 5 against route 3 (outram-mc) is +3 ± 31 pcm.

  Neither resolves a difference from the NJOY2016 data.
- **outram-mc against OpenMC** sits 31–50 pcm low on routes 3–5. With 8
  seeds that is not resolved; the 11-nuclide campaign put it at −3 to −18 pcm.
- **What 8 seeds can show:** a σ of about 30 pcm per difference, so these
  numbers bound route differences near 60 pcm at 2σ. They do not reproduce
  the 11-nuclide campaign's 96-seed resolution.
