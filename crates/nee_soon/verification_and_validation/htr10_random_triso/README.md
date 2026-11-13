# Random TRISO packing in DEM-bed fuel pebbles (gh:#1243, 2026-11-05)

> Research, education and V&V only; not for facility operation, licensing or
> safety decisions. AI-assisted change (Claude Code), not yet reviewed by the
> maintainer. No k_eff result is reported here.

**Request (maintainer, 2026-11-05):** the DEM pebble bed's TRISO packing is random in the browser demos; the Şeker bed stays as it is.

## What the model does

**The option.** `Htr10CoreDesign::triso_packing` is either:
- `Lattice`, the record's regular cubic lattice and the default;
- `Random { realisations, seed }`.

It is read by the explicit (DEM) bed builder only. The Şeker bed always uses the lattice.

**How Random works:**
- **Exact count:** each realisation packs exactly the lattice's particle count (8335 for the record's design), every particle wholly inside the 2.5 cm fuel zone, so the fuel mass is unchanged. It uses `outram_mc_libs::dh_universe::pack_count_in_ball`: OpenMC's RSA (`pack_spheres`, as ported), stopped at n.
- **Tracking lattice:** each packing is wrapped in a 10 × 10 × 10 tracking lattice (`pebble_beds::triso_lattice::create_triso_lattice`, the port of OpenMC's `create_triso_lattice`). The geometry's lattices are 0 the bed, 1 the record's TRISO lattice, then 2 onward the packings.
- **Which pebble gets which packing:** fuel ball number j (counted over fuel balls) takes packing j mod K. Every tile piece of a ball cut by a tile boundary takes the same packing.

**Who uses it:**
- `ParametricStudy` picks `TrisoPacking::DEM_DEFAULT` (8 packings, seed 20 261 105) for an explicit bed and the lattice for the Şeker bed.
- So does the browser demo: its pool builds through the study, and its picture through `random_bed::build_core_with`.
- Native runs outside the study (`htr10_rmc_keff` with `OUTRAM_HTR10_DEM_BED`) keep the lattice. The one recorded DEM-bed k (0.989293 ± 0.000962, 2026-10-08) was on the lattice.

## Methodology

1. **`dh_universe::tests::pack_count_in_ball_places_exactly_n_whole_particles`:** 8335 particles of 0.0455 cm in a 2.5 cm ball. It checks:
   - exactly 8335, all wholly inside;
   - no overlapping pair (every pair checked);
   - the same seed gives the same centres to the bit; another seed gives others.
2. **`explicit_bed::tests::random_triso_packing_holds_the_lattices_particles`**, on the assembled geometry: three separate fuel balls with two packings, and with the lattice. It checks:
   - the lattice count: 4 with Random, 2 with Lattice;
   - every fuel-zone piece fills a packing lattice;
   - each ball's centre lies in packing j mod 2;
   - in each fuel zone, the fraction of 40 000 uniform points that locate in UO₂ kernel equals `n (r_kernel / r_zone)³`, within 5σ of binomial noise, for both packings.
3. **Images** (`examples/htr10_random_triso_images.rs`): two fuel balls, x-y and x-z slices through each centre, rendered from the assembled geometry, lattice and random.

## Results

- **Both tests pass** (2026-11-05, release).
- **Images in this folder:** `pebble{0,1}_{lattice,random}_{xy,xz}.png`.
  - **Random:** particles scattered without order, all whole and inside the fuel zone, none cut by the tracking-lattice tiles. Ball 0 and ball 1 show different packings.
  - **Lattice:** the regular grid.
  - **Not checked in an image:** a slice through a tile boundary at higher zoom (the kernel-fraction test covers whole particles statistically).
