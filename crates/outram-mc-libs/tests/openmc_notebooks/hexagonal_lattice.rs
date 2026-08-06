//! `hexagonal-lattice` notebook -> outram-mc verification (LIVE, geometry).
//!
//! Notebook: `hexagonal-lattice.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Builds a 4-ring hexagonal fuel-pin lattice
//! (`HexLattice`, y-orientation, pitch 1.25 cm, centred at the origin) whose
//! first element in each ring is a "big pin" (U238, r = 0.5 cm) and every other
//! element is a regular pin (U235, r = 0.25 cm), with an all-water `outer`
//! universe. The lattice is wrapped in a `main_cell` bounded by a vacuum
//! `ZCylinder` of radius 5 cm, and the geometry is **plotted**. It also
//! demonstrates switching the lattice `orientation` from `'y'` to `'x'`, and
//! bounding the model with a `model.HexagonalPrism`.
//!
//! **The notebook runs NO k-eigenvalue calculation.** Its final markdown cell
//! states explicitly that to *simulate* the model one *would* need to create
//! `openmc.Settings`, export, and run — but it never does. There is therefore
//! **no reference k-effective in the notebook's cell outputs**. Per this crate's
//! V&V rule ("reference values come from the .ipynb itself; do not invent one"),
//! the LIVE assertions here are **geometry-correctness properties**, not a
//! benchmark k comparison. A supplementary k-eigenvalue *smoke* run is included
//! (see [`hexagonal_lattice_keff_smoke`]) purely to exercise transport over the
//! hex lattice; its k is recorded, not compared to any notebook reference.
//!
//! **OpenMC API exercised (now real in outram-mc).** `HexLattice`
//! ([`outram_mc_libs::geometry::lattice::HexLattice`], ported from
//! `openmc::HexLattice`, `src/lattice.cpp:456`), its ring-based construction
//! ([`HexLattice::from_rings`], mirroring the Python `HexLattice.universes`
//! setter + `fill_lattice_y/x`), nested `Universe`/`Cell`/`ZCylinder`, and the
//! generic lattice descent in [`Geometry::locate`].
//!
//! # V&V — methodology and results
//!
//! **Methodology.** The exact notebook geometry (4 rings, pitch 1.25 cm,
//! y-orientation, big pin at ring position 0, regular pins elsewhere, water
//! outer, vacuum `ZCylinder` r = 5 cm) is built in outram-mc. The pass criteria
//! are geometric:
//!  1. Every valid lattice tile centre `locate()`s three levels deep
//!     (root -> lattice tile -> pin universe) and returns the **material the
//!     notebook placed there**: U238 (fuel2) for a big-pin tile, U235 (fuel) for
//!     a regular-pin tile.
//!  2. A point inside the r = 5 cm boundary but outside the hexagon resolves,
//!     via the lattice `outer` universe, to **water**.
//!  3. A point outside the r = 5 cm boundary is a lost particle (`None`).
//! (Lower-level index/round-trip/distance checks live in the `hex_tests` unit
//! module of `src/geometry/lattice.rs`.)
//!
//! **Results (measured 2026-08-06, this harness).** All geometry properties pass
//! — see [`hexagonal_lattice_geometry`]; they are exact index/material identities
//! that draw no random numbers, so they are RNG-independent. The k-eigenvalue
//! smoke run measured **k = 0.25347 ± 0.00714** (300 particles, 15 inactive + 25
//! active); it is printed at run time (`--nocapture`) and written up, together
//! with the "no notebook reference" caveat, in
//! `docs/ai-fleet-review/op-6tz-hexlattice/REVIEW_MANIFEST.md` and the gitignored
//! `verification_and_validation/openmc_notebook_comparisons/hexagonal_lattice.csv`.
//!
//! **Supersedes (pre-`op-jis`).** This block previously (2026-07-15) recorded no
//! k value here, deferring it to the REVIEW_MANIFEST and the gitignored CSV.
//! Whatever k was recorded there was measured with the old `prn` output function
//! (uniforms formed from the raw top-52 state bits, before bead `op-jis` added
//! OpenMC's PCG-RXS-M-XS output permutation) and is superseded by the value
//! above: the LCG *state* recurrence is unchanged, but every sampled uniform
//! moved. The geometry assertions are unaffected.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{HexLattice, HexOrientation, Lattice};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, ZCylinder};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};

// ── Notebook constants ───────────────────────────────────────────────────────
const N_RINGS: usize = 4;
const PITCH: f64 = 1.25; // cm
const R_PIN: f64 = 0.25; // cm — regular pin (U235)
const R_BIG_PIN: f64 = 0.5; // cm — big pin (U238)
const R_OUTER: f64 = 5.0; // cm — vacuum boundary

// Material indices into the `materials()` array.
const MAT_FUEL: usize = 0; // U235 metal, 10 g/cm³
const MAT_FUEL2: usize = 1; // U238 metal, 10 g/cm³
const MAT_WATER: usize = 2; // H1 + O16, 1 g/cm³

// Universe indices into the geometry's `universes` array.
const U_ROOT: usize = 0;
const U_PIN: usize = 1; // fuel (U235) + water
const U_BIG_PIN: usize = 2; // fuel2 (U238) + water
const U_OUTER: usize = 3; // all water

/// The three notebook materials. Atom densities \[atoms/barn·cm\] from the
/// notebook densities: U235/U238 metal at 10 g/cm³, and light water at 1 g/cm³
/// with H:O = 2:1.
fn materials() -> Vec<Material> {
    // U235 metal: N = 10·N_A/235.04 = 0.025626 /b·cm.
    let fuel = Material {
        id: 1,
        name: "fuel (U235)".into(),
        temperature: 293.6,
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 0.025626 }],
    };
    // U238 metal: N = 10·N_A/238.05 = 0.025303 /b·cm.
    let fuel2 = Material {
        id: 2,
        name: "fuel2 (U238)".into(),
        temperature: 293.6,
        components: vec![NuclideComponent { nuclide_idx: 1, atom_density: 0.025303 }],
    };
    // Light water: N_H2O = 1·N_A/18.015 = 0.033427 /b·cm ⇒ H = 0.066854, O = 0.033427.
    let water = Material {
        id: 3,
        name: "water".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent { nuclide_idx: 2, atom_density: 0.066854 }, // H1
            NuclideComponent { nuclide_idx: 3, atom_density: 0.033427 }, // O16
        ],
    };
    vec![fuel, fuel2, water]
}

/// Nuclides referenced by [`materials`], all from the embedded CORE WMP library:
/// `0 U235, 1 U238, 2 H1, 3 O16`. (No S(α,β) is attached — the k smoke run is a
/// fast/epithermal exercise, not a thermal benchmark.)
fn nuclides() -> Vec<Nuclide> {
    vec![
        Nuclide::from_core("U235").expect("U235 in CORE WMP"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP"),
        Nuclide::from_core("H1").expect("H1 in CORE WMP"),
        Nuclide::from_core("O16").expect("O16 in CORE WMP"),
    ]
}

/// The 4-ring notebook hex lattice: big pin (universe [`U_BIG_PIN`]) at the first
/// element of every ring, regular pin ([`U_PIN`]) elsewhere, outer = [`U_OUTER`].
/// Mirrors notebook cells 12–14.
fn notebook_hex_lattice() -> HexLattice {
    let outer_ring: Vec<usize> =
        std::iter::once(U_BIG_PIN).chain(std::iter::repeat(U_PIN).take(17)).collect(); // 18
    let ring_1: Vec<usize> =
        std::iter::once(U_BIG_PIN).chain(std::iter::repeat(U_PIN).take(11)).collect(); // 12
    let ring_2: Vec<usize> =
        std::iter::once(U_BIG_PIN).chain(std::iter::repeat(U_PIN).take(5)).collect(); // 6
    let inner_ring: Vec<usize> = vec![U_BIG_PIN]; // 1
    HexLattice::from_rings(
        4,
        HexOrientation::Y,
        Position::ZERO,
        PITCH,
        &[outer_ring, ring_1, ring_2, inner_ring],
        Some(U_OUTER),
    )
}

/// Build the full notebook geometry (cells 2–16): the hex lattice inside a
/// vacuum `ZCylinder` of radius 5 cm.
///
/// Surfaces: 0 = regular-pin cylinder (r = 0.25, transmissive, tile-local);
/// 1 = big-pin cylinder (r = 0.5, transmissive, tile-local); 2 = outer boundary
/// (r = 5, vacuum). The pin/outer universes tile all space with two water cells
/// so `find_cell` always resolves.
fn notebook_geometry() -> Geometry {
    const T: f64 = 293.6;
    let surfaces = vec![
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: R_PIN, bc: BoundaryType::Transmissive }),
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: R_BIG_PIN, bc: BoundaryType::Transmissive }),
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: R_OUTER, bc: BoundaryType::Vacuum }),
    ];

    // Cell 0 (root): inside the r=5 vacuum cylinder, filled by the hex lattice.
    let main_cell = Cell::fill(
        1,
        vec![RegionToken::HalfSpace { surface_idx: 2, sense: HalfSpaceSense::Inside }],
        CellFill::Lattice(0),
        Position::ZERO,
    );
    // pin_universe: fuel inside r=0.25, water outside.
    let pin_fuel = Cell::material(2, vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }], MAT_FUEL, T);
    let pin_water = Cell::material(3, vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Outside }], MAT_WATER, T);
    // big_pin_universe: fuel2 inside r=0.5, water outside.
    let big_fuel = Cell::material(4, vec![RegionToken::HalfSpace { surface_idx: 1, sense: HalfSpaceSense::Inside }], MAT_FUEL2, T);
    let big_water = Cell::material(5, vec![RegionToken::HalfSpace { surface_idx: 1, sense: HalfSpaceSense::Outside }], MAT_WATER, T);
    // outer_universe: all water — two cells (inside/outside r=0.25) tile all space.
    let outer_water_a = Cell::material(6, vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }], MAT_WATER, T);
    let outer_water_b = Cell::material(7, vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Outside }], MAT_WATER, T);

    Geometry {
        surfaces,
        cells: vec![main_cell, pin_fuel, pin_water, big_fuel, big_water, outer_water_a, outer_water_b],
        universes: vec![
            Universe { id: 0, cell_indices: vec![0] },       // U_ROOT
            Universe { id: 1, cell_indices: vec![1, 2] },    // U_PIN
            Universe { id: 2, cell_indices: vec![3, 4] },    // U_BIG_PIN
            Universe { id: 3, cell_indices: vec![5, 6] },    // U_OUTER
        ],
        lattices: vec![Lattice::Hex(notebook_hex_lattice())],
        root_universe: U_ROOT,
    }
}

/// LIVE: the `hexagonal-lattice` notebook is a **geometry demo** (no k-eff), so
/// this asserts geometry correctness — every lattice tile centre locates to the
/// material the notebook placed there, the outer region is water, and outside the
/// boundary is lost. See the module V&V note.
#[test]
fn hexagonal_lattice_geometry() {
    let geom = notebook_geometry();
    let lat = notebook_hex_lattice(); // a mirror copy for index/centre bookkeeping
    let u = Direction::new(1.0, 0.0, 0.0);
    let n = (2 * N_RINGS - 1) as i32;

    // (1) Every valid tile centre locates 3 levels deep to the right material.
    let mut n_big = 0;
    let mut n_pin = 0;
    for iy in 0..n {
        for ix in 0..n {
            let i = [ix, iy, 0];
            if !lat.are_valid_indices(i) {
                continue;
            }
            // Tile centre in the global (root) frame: lattice centre is the
            // origin with no cell translation, so it equals the centre offset.
            // get_local_position(0, i) = -center_offset(i), so the centre = -that.
            let c = lat.get_local_position(Position::ZERO, i);
            let centre = Position::new(-c.x, -c.y, 0.0);

            let path = geom
                .locate(centre, u, usize::MAX)
                .unwrap_or_else(|| panic!("tile centre {i:?} at {centre:?} located nowhere"));
            // Descent is root (level 0, the lattice-fill cell) → the tile's pin
            // universe (level 1, carrying the lattice marker), exactly as the
            // triso nested-lattice test: 2 coordinate levels.
            assert_eq!(path.levels.len(), 2, "tile {i:?}: expected root→lattice-tile descent");
            assert_eq!(path.levels[1].lattice, Some(0), "tile {i:?}: level 1 is the lattice");
            assert_eq!(path.levels[1].lattice_index, i, "tile {i:?}: located tile index mismatch");

            match lat.universe_at(i) {
                Some(U_BIG_PIN) => {
                    assert_eq!(path.material, Some(MAT_FUEL2), "big-pin tile {i:?} should be U238");
                    n_big += 1;
                }
                Some(U_PIN) => {
                    assert_eq!(path.material, Some(MAT_FUEL), "regular-pin tile {i:?} should be U235");
                    n_pin += 1;
                }
                other => panic!("valid tile {i:?} filled by unexpected universe {other:?}"),
            }
        }
    }
    assert_eq!(n_big, 4, "one big pin per ring (4 rings)");
    assert_eq!(n_pin, 33, "the other 33 tiles are regular pins");
    assert_eq!(n_big + n_pin, 3 * N_RINGS * (N_RINGS - 1) + 1, "37 tiles in a 4-ring hexagon");

    // (2) A point inside r=5 but outside the hexagon → water via the outer universe.
    //     The hexagon's outer vertices sit ≲ 3.75 cm from the centre, so 4.8 cm is
    //     safely outside it but inside the 5 cm boundary.
    let outside_hex = Position::new(0.0, 4.8, 0.0);
    let p_out = geom.locate(outside_hex, u, usize::MAX).expect("4.8 cm point is inside r=5");
    assert_eq!(p_out.material, Some(MAT_WATER), "outside the hexagon should be water");
    assert_eq!(p_out.levels[1].lattice, Some(0), "still descends through the lattice (outer tile)");

    // (3) A point outside the r=5 vacuum boundary is lost (in no cell of the root).
    assert!(
        geom.locate(Position::new(5.5, 0.0, 0.0), u, usize::MAX).is_none(),
        "beyond the vacuum boundary the particle is lost"
    );
}

/// LIVE (supplementary smoke, NOT a benchmark): run fission-source power
/// iteration over the hex lattice to exercise transport through hex-tile
/// crossings. The `hexagonal-lattice` notebook prints **no** reference k, so
/// this asserts only that the eigenvalue is finite, positive and stationary
/// (small σ) and records the measured k ± σ — it is deliberately **not** compared
/// to any reference number (that would be inventing one).
///
/// # V&V — methodology and results
///
/// **Methodology.** The full notebook geometry (U235/U238 metal pins in light
/// water, vacuum r = 5 cm), CORE-WMP fast/epithermal data, no S(α,β). Small run
/// (seeded, deterministic). **Pass criterion:** k finite & positive, σ bounded,
/// all generations completed. **No reference** (geometry-only notebook).
///
/// **Results (measured 2026-08-06).** **k = 0.25347 ± 0.00714** (npart = 300,
/// 15 inactive + 25 active) — printed at run time and recorded in
/// `verification_and_validation/openmc_notebook_comparisons/hexagonal_lattice.csv`
/// (gitignored) and `docs/ai-fleet-review/op-6tz-hexlattice/REVIEW_MANIFEST.md`.
///
/// **Supersedes (pre-`op-jis`):** the k ± σ recorded on 2026-07-15 (in the CSV /
/// REVIEW_MANIFEST, never quoted in this doc comment) was measured with the old
/// `prn` output function — uniforms from the raw top-52 state bits, before the
/// PCG-RXS-M-XS output permutation — and is superseded by the value above.
#[test]
fn hexagonal_lattice_keff_smoke() {
    let geom = notebook_geometry();
    let mats = materials();
    let nucs = nuclides();

    let settings = KeffSettings { n_particles: 300, n_inactive: 15, n_active: 25, ..KeffSettings::default() };
    // Seed the source across the fuelled hexagon (rejection-sampled into fissile cells).
    let src = SourceBox {
        lower: Position::new(-3.75, -3.75, -1.0),
        upper: Position::new(3.75, 3.75, 1.0),
    };

    let t = std::time::Instant::now();
    let result = run_keff_csg(&geom, &mats, &nucs, src, &settings, None);
    let dt = t.elapsed();

    eprintln!(
        "[hex lattice] k = {:.5} ± {:.5}  (npart={}, {}+{} gen, {:.1?})  [NO notebook reference — smoke only]",
        result.k_mean, result.k_std, settings.n_particles, settings.n_inactive, settings.n_active, dt
    );

    assert_eq!(
        result.k_by_generation.len(),
        settings.n_inactive + settings.n_active,
        "ran all generations"
    );
    assert!(result.k_mean.is_finite() && result.k_mean > 0.0, "k must be finite & positive, got {}", result.k_mean);
    assert!(result.k_std < 0.05, "k noisy/unconverged: σ = {}", result.k_std);
}
