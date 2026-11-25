# HTR-10 temperature coefficients and rod-bank worth on the poured bed (gh:#1739)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** One pour, one seed per case. Research,
> education and V&V only; not for facility operation, licensing or safety
> decisions. AI-assisted run and write-up (Claude Code), not yet reviewed by
> the maintainer.

**Status: RUNNING.** The runs started 2026-11-25 04:49 UTC
(`logs/start_utc.txt`). This README is written while they run. The
**Methodology** below is final. **Results** are filled in only from the logs,
when all seven cases are done. [`PREDICTION.md`](PREDICTION.md) was committed
before any run (de9b762e0).

**Files:**
- `PREDICTION.md`: the predictions, written first.
- `run.sh`: the exact launch.
- `logs/poured.log`, `logs/lattice.log`: one line per case.
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

*Pending: filled in from `logs/` when all seven cases finish.*

## Known gaps

- **One pour.** The pour-to-pour scatter (#798) is not measured.
- **The coefficients are 600 → 1200 K secants**, not operating-point
  derivatives.
- **Random TRISO packing on the poured bed, lattice packing on the Şeker
  bed.** The two beds differ in that too, so the bed effect includes it.
- **Not validation.** No measured coefficient exists for a poured HTR-10
  bed.
