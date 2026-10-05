# HTR-10 control-rod insertion in the explicit geometry (gh:#580)

**Research, education and V&V only.** Not for facility operation, licensing
or safety decisions. Every number here is **TENTATIVE**: a short,
single-seed run on the AI-drafted HTR-10 model of the RMC code-to-code record,
not compared against a measured rod worth.

## What changed

The ten control rods of `nee_soon::htr10_rmc` were explicit geometry (B4C
rings, steel sleeves, iron joints and ends) but could only sit at their
withdrawn position. Since 2026-10-05 the rod universe is built at any
position of the published travel, all ten together:
`core_design::Htr10CoreDesign::rod_insertion`, 0 = withdrawn (lower end at
`z_T` = 119.2 cm, the benchmark's state) to 1 = fully inserted (lower end at
`z_T` = 394.2 cm), linear in the lower-end position, both ends from
IAEA-TECDOC-1382 § 4.1.1.5 (`control_rod::LOWER_END_*_CM`). `z_T` runs
downward from the model top. Above the rod's top end, once it is inside the
model, the channel is helium. The workbench's Step 5 slider sets it, and a
change re-assembles the core and re-arms the review gate.

## The drawings (drawing rule)

Rendered from the **assembled** geometry the solver transports
(`outram_mc_libs::geometry::plot`, material lookup per pixel), by

```text
cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --render-review DIR --rod-insertion F
```

at F = 0 (`rods_out/`), 0.5 (`rods_half/`) and 1 (`rods_in/`), 14 rings x 12
Şeker layers (bed 123.6 cm). Each folder is the review gate's full minimum
set, with two views added for the rods:

- `review_x_z_through_a_control_rod.png`: the plane through one rod's axis,
  whole model height;
- `review_x_z_zoom_on_the_lower_end_of_a_control_rod.png`: 60 cm around the
  lower end, showing the iron end cap, the first B4C section and the steel
  sleeves.

**Checked by eye in them (2026-10-05):** withdrawn, the rod occupies the top
reflector only (lower end at z = 170.8 cm, above the bed top at 61.8 cm);
half in, the lower end is at z = 33.3 cm, inside the bed height; fully in,
it is at z = -104.2 cm (below the bed floor at -61.8 cm) and the rod top at
z = 160.5 cm, with helium above it in the channel. The X-Y slice at bed
mid-height shows the B4C ring (3.00-5.25 cm), both steel sleeves and the
helium gaps in the rod channel with rods in, and an empty channel with rods
out. **Not checked:** the other nine rods individually (the channel layout is
pinned by `reflector_geometry`'s own tests); a radial slice at every joint.

**Gated in code:** `nee_soon` `core_design::tests::inserted_rods_are_in_the_assembled_geometry`
locates 600 points up a rod channel on the B4C ring: withdrawn, no B4C below
`z_T` 119.2 cm; inserted, B4C spans the whole bed height, none above
`z_T` 129.5 cm, and the sampled absorber length is 243.5 cm (the published
five 48.7 cm sections) to the sampling step.
`the_default_design_builds_the_geometry_unchanged` pins the withdrawn core at
43 445 cells, 22 974 tiles, 1 502 universes (the workbench fixture's counts).
Inserting the rods adds 14 cells (43 459): the joints and end that were above
the model top, and the helium above the rod.

## k_eff, rods out against rods in (TENTATIVE)

**Methodology.** `--headless-keff` on the HTR-10 preset (ENDF/B-VIII.0 from
the workspace, 300.15 K, every bound thermal law on, full rod metal), 14 x 12
bed, 1000 neutrons x [10 inactive + 30 active] generations, seed 1, 3
threads, the same seed and statistics for both states. σ is the within-run
standard deviation of the mean over the 30 active generations; a 30-generation
single-seed run underestimates the true uncertainty (generation correlation),
so these are previews, not record statistics.

**Results (2026-10-05).**

| rods | lower end `z_T` | k_eff | histories | lost |
|---|---|---|---|---|
| withdrawn (0) | 119.2 cm | 0.99291 ± 0.00622 | 40 000 | 0 |
| fully inserted (1) | 394.2 cm | 0.85707 ± 0.00535 | 40 000 | 0 |

Δk = −0.1358 ± 0.0082; worth `1/k_in − 1/k_out` = **0.160 ± 0.010**
(about 16 000 pcm), sign and magnitude as expected for ten B4C rods over the
whole core height (predicted before the run: strongly negative, of order
10^4 pcm). The withdrawn k was not compared with the record's runs here.

**What this is not.** Not a comparison with the HTR-10 measured rod worth
(TECDOC-1382's control-rod problems); the model's open −2726 pcm residual at
the critical loading (gh:#333) is unresolved, and the one-rod problems are not
representable (the rods move together). Re-measure with record statistics
before quoting.

## Same run, tapes from an extracted library (gh:#581)

The rods-out case was repeated with `--endf-dir ~/Desktop/ENDF-B-VIII.0`
(the official ENDF/B-VIII.0 archive, extracted: `neutrons/`,
`thermal_scatt/`, the library's own file names), 5 threads instead of 3.
Every tape was found by MAT and NSUB (`--scan-endf` lists each path), and
the result is **k = 0.99291 ± 0.00622, identical to every printed digit**
to the workspace run. Expected: of the tapes compared byte for byte (U-235,
U-238, C-12, O-16, He-4, Si-28, graphite 30P) all are identical files; B-10
differs in its header lines (an E4-Web retrieval with sequence numbers
against the archive's copy), not, on this evidence, in data.
