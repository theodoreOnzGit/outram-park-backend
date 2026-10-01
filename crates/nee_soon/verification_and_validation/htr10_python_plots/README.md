# HTR-10 cross sections via the ported `openmc.Model.plot`

Generated 2026-09-26, regenerated 2026-10-02 (see below), by `crates/nee_soon/examples/htr10_python_plots.rs`
(helpers: `nee_soon::htr10_rmc::plots::Htr10Plotter`). Each `<name>.py` is a
standalone matplotlib script emitted by `outram_mc_libs::geometry::plot::ModelPlot`,
the port of OpenMC's Python `Model.plot` (0 differing pixels against OpenMC
0.16.1.dev25 on 17 cases:
`crates/outram-mc-libs/verification_and_validation/python_plotting_parity/`).
`<name>.png` is what the script draws. `PLANES.md` records how each plane was
chosen and how many ball centres lie on it.

~~**Geometry drawn:** `assemble_explicit_triso(14, 25)` — critical loading
(bed 122.474 cm), the two-ball bed (PR #328)~~ **CORRECTED 2026-10-02:** the
twelve images listed in `PLANES.md` were regenerated on 2026-10-02 at
`develop` `229a70d770` from `assemble_explicit_triso(14, 12, 0)`: Şeker &
Çolak (2003)'s 13-ball bed (gh:#472), N = 12, bed 123.576 cm, 16 681 balls
eligible, 9508 fuelled, 1899 rejected at a boundary. Until then this folder
still held the 2026-09-26 two-ball images, although
`../htr10_seker_2026_10_01/README.md` said they had been regenerated on
2026-10-01. Still the explicit reflector (PR #327), **with the withdrawn rods
in place** (the geometry default).

**`htr10_triso.png` and `htr10_triso_array.png` are NOT regenerated.** The
example no longer writes them; they are the 2026-09-26 drawings. The current
TRISO zooms are `../htr10_geometry_images/htr10_xy_triso.png` and
`htr10_xy_one_pebble.png` (redrawn 2026-10-01 for the 8335-particle grid,
#430).

**What was checked by eye on the 2026-10-02 set (the assistant):**
`htr10_rz_through_pebbles.png`: bed top at +61.8 cm, conus floor at
−98.7 cm, empty helium cavity to 160 cm, chute and conus of dummy balls to
the model bottom, hot-gas duct at +x. `htr10_xy_bed_mid.png`: every ball
whole, none crosses r = 90 cm, a helium gap at the wall where wall-crossing
balls were rejected; 20 coolant channels, the larger rod/irradiation channels
and the 7 KLAK slots in the reflector. The other ten were not opened. The table
below is the 2026-09-26 check of the two-ball set and is kept as that record;
its counts (e.g. 1253 ball centres) are for that bed, not this one.
Coloured by the material the solver sees at each pixel. The legend lists only
the materials present in that slice.

**Not a validation artefact.** No transport runs here. The fast k-eff runs of
the same date were made with the rods removed (`OUTRAM_HTR10_NO_WITHDRAWN_RODS`)
as a stated modelling assumption; see the hand-off.

## What was checked by eye, and what was not

| Image | Checked |
|---|---|
| `htr10_triso.png` | kernel + buffer, IPyC, SiC, OPyC, concentric, in matrix graphite |
| `htr10_triso_array.png` | regular TRISO array; whole particles only |
| `htr10_fuel_pebble.png` | TRISO lattice fills the 2.5 cm fuel zone with whole particles; fuel-free shell; the neighbouring B balls at the corners |
| `htr10_dummy_pebble.png` | all-graphite ball |
| `htr10_rz_through_pebbles.png` | plane holds 1253 ball centres; pebbles whole, helium between; empty cavity above the bed to z_T 130; rod B4C starting at z = 171.4 cm (z_T 119.2); control-rod channel at +x down to z_T 450; KLAK channel at -x; hot-gas duct at +x near z_T 480; conus and tube of dummy balls to the model bottom |
| `htr10_rz_conus_and_chute.png` | only whole balls touch the cone and tube wall (Li's rejection); no fuel below the bed floor |
| `htr10_xy_bed_{top,mid,bottom}.png` | 20 coolant channels (r 144.6), 10 control-rod + 3 irradiation channels (r 102.1), 7 KLAK slots; fuel balls speckled with TRISO, dummy balls plain |
| `htr10_xy_conus.png`, `htr10_xy_defuel_chute.png` | whole dummy balls only; KLAK channels round below the slot range (z_T > 388.8) |
| `htr10_xy_top_reflector_rods.png` | 10 rods as steel/B4C/steel annuli in their channels; irradiation and KLAK channels empty |
| `htr10_xy_cavity.png`, `htr10_xy_hot_gas_duct.png` | empty cavity; duct |

**Not checked / open, with the issues that own them:**
- channel azimuths are a convention (#330);
- balls **clipped** at the r = 90 cm side wall, not rejected (#331);
- zones without internal geometry keep homogenised compositions (#332);
- whether the reference's height axis is offset by one ball diameter (#333).
