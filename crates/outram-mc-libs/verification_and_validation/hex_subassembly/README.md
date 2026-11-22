# Hexagonal subassembly builder: geometry checks (gh #1327)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** AI draft, unreviewed. Research, education
> and V&V only; not for facility operation, licensing or safety decisions.

`outram_mc_libs::model::HexSubassembly` (`src/model/hex_subassembly.rs`)
builds a hex duct holding a triangular-pitch bundle of `r` rings of pins
(fuel slug, bond, cladding) with coolant between them and an optional
helical wire wrap smeared into the coolant. **All inputs below are generic
round numbers, not any reactor's design.**

## Methodology

| check | where | reference | pass criterion |
|---|---|---|---|
| pin count `3r(r-1)+1`, `6k` pins in ring `k`, nearest neighbour at one pitch, rows parallel to the flats | `model::hex_subassembly::tests::pin_count_and_bundle_structure`, r = 1..10 | closed form; ring found from geometry, independent of the lattice's order | exact (distance to 1e-12 cm) |
| every pin enclosed, no two pins overlap, centres inside the duct; too-small duct and too-small pitch refused | `pins_inside_duct_and_disjoint`, `wire_wrap_room_is_checked` | the exact `Solid::encloses` / `Solid::overlaps` relations `Model::assemble` uses | as stated |
| volume fractions | `volume_fractions_analytic_and_sampled` | hand areas `(sqrt 3/2)F^2`, `pi r^2`, wire `(pi s^2/4)/cos(theta)` | analytic to 1e-6; built shapes sampled on a 1201^2 grid to 2e-3 |
| smeared coolant | `smeared_coolant_is_the_volume_mix` | `(1-f) N_cool + f N_wire` | 1e-15 atoms/b-cm |
| through `Model::assemble`: accepted, one pair of z-planes, assembled cell lookup fractions | `tests/model_hex_subassembly.rs` (4 rings, 37 pins, wire 0.1 cm) | prediction: 2 z-planes (6 before the lowering change); analytic fractions | 2 z-planes; fractions to 2e-3; no unlocated point |

## Results (2026-11-10, release build, 4-core cloud container)

- All unit checks pass.
- `Model::assemble` accepts the 37-pin subassembly with a reflective
  boundary. **z-planes: 2.** With the `HexPrism` lowering change reverted,
  **6**, as predicted: the duct, the coolant prism and the pins each had
  their own pair of coincident end planes.
- Assembled cell-lookup fractions (901 x 901 grid at mid-height, 608 401
  points inside the duct, none unlocated) against the analytic ones:
  fuel 0.26604 vs 0.26589, bond 0.06784 vs 0.06764, clad 0.12974 vs
  0.12975, duct 0.13707 vs 0.13736, coolant + wire 0.39931 vs 0.39936;
  worst difference 2.9e-4. A first run counted 85 grid points as
  unlocated; all lay on the outer duct faces to 1e-6 cm (cell lookup on a
  face is ambiguous by construction), and the sampler now leaves out points
  within 1e-9 cm of the outer faces.
- **k-infinity: not run.** `reference-data/ace` holds ACE tables for
  U-234/235/236/238 only; sodium, iron and chromium exist only as ENDF tapes
  (`reference-data/endf/`), which would be processed at run time, and
  zirconium and nickel are absent, so a U-Zr / steel / sodium smoke run is
  neither cheap nor from that data.

## Pictures (drawn from the assembled geometry)

Regenerate with
`cargo run --release -p outram-mc-libs --example hex_subassembly_geometry`.

- `hex_subassembly_xy.png`: the section at mid-height. Checked: 37 pins in
  rows of 4-5-6-7-6-5-4 parallel to the duct's flats, the corner pins
  pointing at the duct's vertices, an even coolant gap to the duct, the
  wall uniform. The wire is smeared, so it is not drawn.
- `hex_subassembly_xz.png`: x-z through the centre row. Checked: 7 pins and
  the duct run the full 10 cm and end on the same planes.
- `hex_subassembly_pin_xy.png`: one corner pin. Checked: fuel 0.25, bond to
  0.28 and cladding to 0.33 cm, concentric.
- Not checked in pictures: the wire's volume (it is not drawn; the
  smearing is checked by the tests).
