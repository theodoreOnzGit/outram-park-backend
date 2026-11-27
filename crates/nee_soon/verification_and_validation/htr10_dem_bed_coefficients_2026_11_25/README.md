# HTR-10 temperature coefficients and rod-bank worth on the poured bed (gh:#1739)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** One pour, one seed per case. Research,
> education and V&V only; not for facility operation, licensing or safety
> decisions. AI-assisted run and write-up (Claude Code), not yet reviewed by
> the maintainer.

**Status: DONE (2026-11-25).** ~~RUNNING.~~ The runs started 04:49 UTC
(`logs/start_utc.txt`) and the last case finished at 11:13 UTC (`logs/end_utc.txt`). The
container restarted twice and the 2 h task limit stopped one launch, so the
remaining cases ran one per process through `run_resume.sh` (same driver,
statistics and seed; a case interrupted mid-run was rerun from the start).
[`PREDICTION.md`](PREDICTION.md) was committed before any run (de9b762e0).

**Files:**
- `PREDICTION.md`: the predictions, written first.
- `run.sh`: the exact launch.
- `run_resume.sh`: the resumable launch, one case per process.
- `logs/poured.log` (poured BASE) and `logs/case_<bed>_<case>.log` (the
  other six): one line per case.
- `logs/*.err`: the per-job data lines.

## Methodology

- **Computed:** the HTR-10 whole-core `k_eff`, using `nee_soon`'s model of the
  2026-10-07 lattice record, with every zone at a base temperature and then
  one part hotter at a time. These are gh:#1174's cases
  (`parametric::graphite_worth_cases`), plus a rods-in case.
- **Driver:** `examples/htr10_graphite_zone_worth.rs`, unchanged in protocol
  from gh:#1174. Each case goes through `parametric::run_worth_case`: plan,
  process every (nuclide, temperature) exactly, transport.
  - **Options added for this record:** `--dem-bed` builds on the poured bed,
    and `--rods-in` adds the rod-bank case.
  - **Binary:** the poured-bed cases ran on the binary built at de9b762e0.
    The lattice cases ran on the one rebuilt at 0754f9ad1, which moved the
    per-case steps into `run_worth_case` with the same calls. Its log line
    gives one wall time for data and transport together.
- **Beds:**
  - **Poured:** the gh:#216 pour
    (`reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv`). It is
    cut to the lattice's 16 681 core balls at N = 12 by
    `mc_rungs::htr10_core::dem_centres`; fuel is assigned by
    `paper_fuel_assignment` (9 508 fuel balls); the bed is built by
    `parametric::poured_bed`. This is the 2026-10-08 record's bed, drawn in
    `../htr10_dem_bed_images/`.
    - **Not the same as that record in one respect:** `ParametricStudy` gives
      an explicit bed random TRISO packing (`TrisoPacking::DEM_DEFAULT`, 8
      realisations, gh:#1243). The 2026-10-08 record used the lattice
      packing. Its −583 pcm is therefore not this record's BASE offset;
      PREDICTION.md assumed it was.
  - **Lattice:** Şeker & Çolak's N = 12 bed, gh:#1174's.
- **Settings, identical for every case:**
  - base 600 K, +600 K;
  - 3000 × [20 + 80];
  - seed 20260917, the same for every case (correlated sampling);
  - ENDF/B-VIII.0 defaults (URR, RVS, LEAPR laws at each zone's
    temperature);
  - 4 threads.
- **Cases:**
  - **On the poured bed:** BASE, FUEL-GRAPHITE (the fuel pebbles' matrix and
    shell), DUMMIES (the moderator pebbles), KERNEL (kernels and coatings),
    and RODS-IN (all ten rods fully in, at base).
  - **On the lattice:** BASE and RODS-IN.
- **Quantities:**
  - fuel coefficient: `α = (1/k_BASE − 1/k_KERNEL)/600 K`;
  - moderator coefficient: the same for FUEL-GRAPHITE plus the same for
    DUMMIES;
  - rod-bank worth: `1/k_RODS-IN − 1/k_BASE`;
  - σ from the two runs in quadrature. This ignores the correlation, so it
    is an upper bound.
  - This is the arithmetic of `twin::reactivity_coefficient` and MSRE's
    `coefficient_from_pair`.
- **Lattice pairs:** gh:#1174 (`../htr10_rz_transients/README.md` §11.1)
  for the coefficients; this record's lattice BASE and RODS-IN for the
  bank.
- **Pass criterion:** none. The record reports the numbers against
  PREDICTION.md, and the disagreement where there is one.

## Results

Read from `logs/`; every case 300 000 histories, 0 lost. σ of a
difference is the two runs' in quadrature (an upper bound; see Methodology).

**`k` per case (600 K base, +600 K):**

| Bed | Case | `k` | Log |
|---|---|---|---|
| poured | BASE | 0.94516 ± 0.00185 | `poured.log` |
| poured | FUEL-GRAPHITE | 0.89446 ± 0.00227 | `case_poured_FUEL-GRAPHITE.log` |
| poured | DUMMIES | 0.91117 ± 0.00215 | `case_poured_DUMMIES.log` |
| poured | KERNEL | 0.93624 ± 0.00205 | `case_poured_KERNEL.log` |
| poured | RODS-IN | 0.77950 ± 0.00209 | `case_poured_RODS-IN.log` |
| lattice | BASE | 0.95376 ± 0.00222 | `case_lattice_BASE.log` |
| lattice | RODS-IN | 0.79303 ± 0.00187 | `case_lattice_RODS-IN.log` |

**Against [`PREDICTION.md`](PREDICTION.md):**

| Quantity | Predicted | Poured bed (this record) | Lattice | Verdict |
|---|---|---|---|---|
| BASE `k` | 0.9480 (0.944 to 0.952) | 0.94516 ± 0.00185 | 0.95376 ± 0.00222 | inside the range; the bed sits −860 ± 289 pcm below the lattice, not the −583 assumed (the beds also differ in TRISO packing, see Beds) |
| Lattice BASE re-run | 0.95389 within 1σ | — | 0.95376 ± 0.00222 | −13 ± 319 pcm from gh:#1174's: agrees; not bit for bit (16 threads then, 4 now; not checked further) |
| Moderator (FUEL-GRAPHITE + DUMMIES) | −16 (−13 to −20) pcm/K | **−16.57 ± 0.81 pcm/K** (−10.00 ± 0.59 and −6.58 ± 0.55) | −16.73 ± 0.91 (gh:#1174) | inside; bed effect +0.16 ± 1.22, not resolved |
| Dummies' share | 0.35 to 0.50 | 0.397 | 0.405 ± 0.04 | inside |
| Fuel (KERNEL) | −2.5 (−1.0 to −4.0) pcm/K | **−1.68 ± 0.52 pcm/K** | −2.49 ± 0.64 (gh:#1174) | inside; bed effect +0.81 ± 0.82, not resolved |
| Rod bank at 600 K | 15 to 20 % Δk/k on both beds | **22.49 ± 0.40 %** | 21.25 ± 0.39 % | **MISSED on both beds**; poured minus lattice +1.24 ± 0.56 % (2.2σ), inside the predicted ±1.5 % |

**Which numbers to quote.** The poured-bed set is fuel −1.68 ± 0.52 pcm/K,
moderator −16.57 ± 0.81 pcm/K, bank 22.49 ± 0.40 % Δk/k. These are what
`htgr_sim_v1`'s "Monte Carlo, poured bed" coefficient set uses (gh:#1740,
`physics/coefficient_set.rs`, `POURED_BED`), unrounded from the logs. The
lattice numbers are for the bed comparison only.

**The miss, stated.** The rod-bank prediction was taken from gh:#1025's
17.62 ± 0.48 % at 300.15 K. Both beds give 21 to 22.5 % at 600 K. That
earlier number also used the published tapes' graphite laws and DBRC, the
defaults before 2026-11-03 / 11-10 (nee_soon `AGENTS.md`), so temperature and
data defaults both differ. Which carries the ~4 % is **not investigated
here**; the 600 K numbers are what this model gives, and they are quoted as
measured. Against the plant's cited TECDOC-1382 VSOP bank (15.24 %), the
Monte Carlo bank is about 1.5 times larger.

**What it implies for the plant.** The Monte Carlo set splits the feedback
very differently from the cited one (Chen's −14 pcm/K split by Hu's fuel
share: fuel −9.96, moderator −4.04 pcm/K). Here almost all of it is in the
graphite (−16.6) and little in the kernel (−1.7), so the plant on this set
has a much weaker prompt Doppler response and a stronger, slower moderator
response. Not validation: no measured coefficient of a poured HTR-10 bed
exists.

## Known gaps

- **One pour.** The pour-to-pour scatter (#798) is not measured.
- **The coefficients are 600 → 1200 K secants**, not operating-point
  derivatives.
- **Random TRISO packing on the poured bed, lattice packing on the Şeker
  bed.** The two beds differ in that too, so the bed effect includes it.
- **Not validation.** No measured coefficient exists for a poured HTR-10
  bed.
- **The rod-bank miss is not explained** (temperature against the data
  defaults; see Results).
