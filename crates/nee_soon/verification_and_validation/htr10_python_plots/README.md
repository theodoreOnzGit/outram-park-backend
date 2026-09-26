# HTR-10 cross sections via the ported `openmc.Model.plot`

Generated 2026-09-26 by `crates/nee_soon/examples/htr10_python_plots.rs`
(helpers: `nee_soon::htr10_rmc::plots::Htr10Plotter`). Each `<name>.py` is a
standalone matplotlib script emitted by `outram_mc_libs::geometry::plot::ModelPlot`,
the port of OpenMC's Python `Model.plot` (0 differing pixels against OpenMC
0.16.1.dev25 on 17 cases:
`crates/outram-mc-libs/verification_and_validation/python_plotting_parity/`).
`<name>.png` is what the script draws. `PLANES.md` records how each plane was
chosen and how many ball centres lie on it.

**Geometry drawn:** `assemble_explicit_triso(14, 25)` — critical loading
(bed 122.474 cm), the two-ball bed (PR #328) and the explicit reflector
(PR #327), **with the withdrawn rods in place** (the geometry default).
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
