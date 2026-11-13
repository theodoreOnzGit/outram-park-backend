# HTR-10 per-zone temperatures: model, isothermal regression, drawing (2026-11-03)

> Research, education and V&V only; not for facility operation, licensing or
> safety decisions. AI-assisted change (Claude Code), not yet reviewed by the
> maintainer.

Epic gh:#1137; this record covers gh:#1138 (the model), gh:#1139
(`ParametricStudy`) and the first part of gh:#1143 (the isothermal regression).
The LEAPR validation it depends on is gh:#1140,
`crates/njoy-outram-park-fork/verification_and_validation/leapr_htr10_tsl_vs_endf8.md`.

## What the model does

**Eight zones**, each with its own temperature (maintainer, 2026-11-03):

- fuel pebbles (kernel, coatings, matrix, shell);
- moderator pebbles (graphite balls);
- helium coolant;
- top, side and bottom reflector;
- control rods (B₄C, steel, iron);
- boronated bricks.

`nee_soon::htr10_rmc::zones` says which material slot belongs to which zone.

**How a design opts in.** With `Htr10CoreDesign::temperature_zones`, a design places three kinds of extra slots:

- the graphite balls on `mat::MODERATOR_BALL`;
- one copy of each reflector composition per region;
- each Fig. 4.10 box assigned to a region by height: wholly above `z_T` = 130 cm is top, wholly below 388.764 cm is bottom, anything overlapping that band is side.

**What each zone's temperature sets:**

- its materials' temperature;
- its cells' temperature (gh:#313);
- the helium density (an ideal gas at 101.33 kPa), for the coolant;
- the temperature its nuclides are processed at, exactly: RECONR, BROADR, HEATR, PURR through `Nuclide::process_evaluation`;
- the temperature its bound thermal laws are generated at, by LEAPR (`ThermalLawSource::Leapr`, `ThermalScattering::from_leapr`).

**The nuclide array.** `zones::split_by_temperature` holds each distinct (nuclide, temperature) once. `data_jobs::tempered_jobs` lists one job per (tape or law, temperature).

**Who uses it.** `parametric::ParametricStudy` carries every setting, and both of these build through it:

- `examples/htr10_parametric.rs` (the CLI);
- `dhoby-ghaut`'s browser demo.

## Methodology

1. **Unit tests** (`cargo test --release -p nee_soon --lib`), 16 run:
   - every slot has a zone;
   - the Fig. 4.10 boxes fall into three regions (zones 1-4 top, 8-18 bottom, zone 66's two boxes in two regions);
   - the range is 20-1800 °C, with the TRISO warning above 1600 °C;
   - the split by temperature is the identity at one temperature;
   - the study's plan at one temperature is the record's slots and jobs;
   - the zoned core is the record's cell for cell, with only the zone slots' indices moved and each cell at its zone's temperature;
   - the helium density scales as 1/T;
   - the CLI parses every setting;
   - the record's own design still builds its geometry unchanged (`the_default_design_builds_the_geometry_unchanged`).
2. **Isothermal regression** (`tests/htr10_parametric_isothermal.rs`, `long-tests`):
   - **Setup:** N = 10, every zone at 300.15 K, the published tapes' graphite and SiC laws (the record's). All 36 jobs are processed once.
   - **The two runs:** the same nuclide array is transported twice by `run_keff_csg_hybrid`, 300 × [2 + 3], seed 20 260 917, one thread. One run uses the record's materials, geometry and majorant; the other the study's zoned ones.
   - **Pass:** every generation's k, the entropy and the mean, bit for bit.
3. **Drawing** (`examples/htr10_zone_images.rs`): the zoned N = 12 core rendered from the assembled geometry, coloured by zone (`zones::zone_palette`), into `images/`.

## Results

- **Unit tests:** 16 passed (2026-11-03).
- **Isothermal regression: identical.** Record path k = 0.82598314461052513 ± 0.02475, zoned path k = 0.82598314461052513 ± 0.02475. The per-generation k, entropy, histories and majorant violations are equal too.
  - This k is a regression gate, not a physics result: N = 10, only 3 active generations.
  - The run took 212 s, including processing the 36 jobs on 4 threads.
  - The slowest jobs were U-235 133 s, U-238 110 s and C-in-SiC 40 s.
  - **Hardware:** cloud container, 4 vCPU Intel Xeon (model not exposed), 15 GiB, rustc 1.99.1, release.
- **A non-uniform plan** (CLI `--plan` with fuel 900 °C, coolant 500 °C, side reflector 300 °C, every other zone 27 °C):
  - carbon and boron are processed at three temperatures each (300.15, 573.15 and 1173.15 K);
  - helium at 773.15 K;
  - the rod metals at 300.15 K;
  - the result is more jobs than the 36 at one temperature.
- **Images**, checked by eye:
  - **`images/htr10_zones_xz_full.png`:**
    - Fuel pebbles (red) and moderator pebbles (dark grey) are told apart throughout the bed. The conus and the discharge tube hold moderator balls only.
    - Top plug (blue) above the cavity, side reflector (tan) beside the bed, bottom structures (green) below the conus top.
    - Boronated bricks (purple) at the outer annulus, the top corners and the bottom band.
    - Zone 66's outer column (148.6-167.8 cm) is side all the way up, being one box over the core band.
  - **`images/htr10_zones_xz_bed_edge.png`:** ball by ball, fuel and moderator pebbles in helium against the side reflector.
  - `images/htr10_zones_xy_*.png`: radial slices at bed mid-height, through the conus and in the top reflector.
  - **Not checked in an image:** the control-rod channels (the slices shown do not cut a rod), and the legend repeats each zone's name once per slot.

## What it changes, and what is still open

- **The thermal laws.** ~~The study's default is `ThermalLawSource::Leapr`~~ **CHANGED 2026-11-03 (maintainer: "just use LEAPR by default"):**
  - `ThermalLawSource::Leapr` is now the data default everywhere: graphite and SiC by LEAPR at each zone's temperature, the native loader included.
  - The records read them from the tapes, so graphite was at the tape's 296 K for a 300.15 K run. At 300.15 K everywhere the default model therefore differs from the records by that law alone.
  - `OUTRAM_HTR10_TAPE_LAWS=1` restores the tapes. Re-measuring the records is gh:#1145.
  - gh:#1140 shows LEAPR reproduces the tapes' S(α,β) at every tabulated temperature.
  - SiC's elastic channel through `from_leapr` is coherent, from crystal data, about −3 % from the tape (`docs/leapr-sic-coherent-elastic-vv.md`).
  - The k movement from the law change is gh:#1143's to measure, with seeds.
- **Not done here** (gh:#1143):
  - temperature coefficients, one zone at a time;
  - OpenMC code-to-code at the same zone temperatures.
- **The browser demo:** see `crates/dhoby-ghaut/verification_and_validation/htr10_zone_temperatures_web/`.
