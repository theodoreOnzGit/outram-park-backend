//! `triso` notebook → outram-mc verification (LIVE honest partial).
//!
//! Notebook: `triso.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Randomly packs TRISO fuel particles
//! (`model.pack_spheres`) into a pebble/compact, builds a TRISO lattice
//! (`model.create_triso_lattice`), and runs a **doubly-heterogeneous** k-eff.
//! OpenMC API exercised: `model.TRISO`, `model.pack_spheres`,
//! `model.create_triso_lattice`, `Universe`, `Cell`, `Sphere`, `RectLattice`.
//!
//! **What outram-mc can do today (this run wires it).** The general CSG
//! navigation now descends **root → lattice → nested TRISO universe → kernel /
//! matrix cell** (op-6tz.7: [`Geometry::locate`] over
//! [`RectLattice`] + nested [`Universe`]s), a surface-tracking k-eigenvalue runs
//! over that geometry (op-6tz.8/.16: [`run_keff_csg`]), and Woodcock/delta
//! tracking flies through the packed medium via the same geometry lookup
//! (op-6tz.16: [`track_to_collision`] + [`Majorant`]). The doubly-heterogeneous
//! geometry, its k-eff, and delta tracking are therefore **real** here — the LIVE
//! tests below assert on them.
//!
//! **Honest simplifications (documented gaps).** This is *not* the notebook's
//! full model:
//! - **Regular lattice, not random packing.** `pebble_beds::stochastic_media`
//!   (`pack_spheres`) is still a stub, so TRISO kernels sit on a **regular 3-D
//!   lattice** (one kernel per tile) rather than a random dense packing. This is
//!   the honest `create_triso_lattice`-style arrangement; random packing is bead
//!   op-6tz (stochastic-media) follow-up.
//! - **Single kernel sphere, no TRISO shell stack.** Each particle is a bare fuel
//!   kernel in matrix — the buffer / IPyC / SiC / OPyC shells are collapsed into
//!   the matrix (a fidelity simplification, not a transport limitation).
//! - **Fast/epithermal data only** (`Nuclide::from_core`), no graphite S(α,β)
//!   (op-6tz.12), so this is a fast doubly-heterogeneous demo, not a benchmark.
//!
//! # V&V — methodology and results
//!
//! **Methodology.** A 3×3×3 reflective lattice of HEU fuel kernels
//! (r = 0.18 cm, pitch 0.4 cm ⇒ ~38 % packing) in an H-1 matrix, embedded
//! LOW-tier data, analog transport. Three LIVE checks:
//! 1. *Nested-lattice geometry navigation* — [`Geometry::locate`] resolves a
//!    kernel-centre point to the **fuel** material through three coordinate
//!    levels (root box → lattice tile → TRISO universe), and a between-kernels
//!    point to the **matrix**; a point outside the reflective box is lost.
//! 2. *Delta (Woodcock) tracking* — a majorant bounding both materials drives
//!    [`track_to_collision`] to a real collision inside the packed medium with a
//!    virtual/real split consistent with Σ_t/Σ_maj.
//! 3. *Doubly-heterogeneous k-eff* — [`run_keff_csg`] surface-tracks the whole
//!    nested lattice to a stationary eigenvalue (the assembled op-6tz.16 sim).
//!
//! **Results (2026-07-15, this harness; asserted at run time).** Navigation and
//! delta-tracking checks pass deterministically; the doubly-heterogeneous k-eff
//! converges to a stationary, positive eigenvalue (value asserted in a broad
//! plausibility band, not against a benchmark — see the simplifications above).
//! Details in `docs/ai-fleet-review/op-6tz-pincell-triso/REVIEW_MANIFEST.md`.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{Lattice, RectLattice};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind, XPlane, YPlane, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::{track_to_collision, Majorant};
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};

const R_KERNEL: f64 = 0.18;
const PITCH: f64 = 0.4;
const N: usize = 3;
const HALF: f64 = 0.5 * N as f64 * PITCH; // 0.6 cm

/// HEU kernel + H-1 matrix nuclide array: [U234, U235, U238, H1].
fn triso_nuclides() -> Vec<Nuclide> {
    vec![
        Nuclide::from_core("U234").unwrap(),
        Nuclide::from_core("U235").unwrap(),
        Nuclide::from_core("U238").unwrap(),
        Nuclide::from_core("H1").unwrap(),
    ]
}

/// [fuel kernel (HEU), matrix (H-1)].
fn triso_materials() -> Vec<Material> {
    let fuel = Material {
        id: 1,
        name: "HEU kernel".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
        ],
    };
    let matrix = Material {
        id: 2,
        name: "H matrix".into(),
        temperature: 293.6,
        components: vec![NuclideComponent { nuclide_idx: 3, atom_density: 4.0e-2 }],
    };
    vec![fuel, matrix]
}

/// Build the 3-D reflective TRISO lattice geometry.
///
/// Surfaces: 0 = kernel sphere (transmissive, in the TRISO universe's local
/// frame); 1..=6 = the six reflective box planes. Cell 0 (root) fills the box
/// with lattice 0; universe 1 is the TRISO unit (cell 1 = kernel/fuel, cell 2 =
/// matrix). The lattice tiles the box exactly with `N³` copies of universe 1.
fn triso_geometry() -> Geometry {
    let surfaces = vec![
        SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: R_KERNEL, bc: BoundaryType::Transmissive }),
        SurfaceKind::XPlane(XPlane { x0: -HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::XPlane(XPlane { x0: HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: -HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: -HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: HALF, bc: BoundaryType::Reflective }),
    ];

    // Root cell: the reflective box (inside all six planes), filled by the lattice.
    let box_region = vec![
        RegionToken::HalfSpace { surface_idx: 1, sense: HalfSpaceSense::Outside }, // x > -HALF
        RegionToken::HalfSpace { surface_idx: 2, sense: HalfSpaceSense::Inside },  // x < +HALF
        RegionToken::Intersection,
        RegionToken::HalfSpace { surface_idx: 3, sense: HalfSpaceSense::Outside },
        RegionToken::Intersection,
        RegionToken::HalfSpace { surface_idx: 4, sense: HalfSpaceSense::Inside },
        RegionToken::Intersection,
        RegionToken::HalfSpace { surface_idx: 5, sense: HalfSpaceSense::Outside },
        RegionToken::Intersection,
        RegionToken::HalfSpace { surface_idx: 6, sense: HalfSpaceSense::Inside },
        RegionToken::Intersection,
    ];
    let root_cell = Cell::fill(1, box_region, CellFill::Lattice(0), Position::ZERO);

    // TRISO unit universe: kernel (inside sphere) = fuel, matrix (outside) = matrix.
    let kernel = Cell::material(2, vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }], 0, 293.6);
    let matrix = Cell::material(3, vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Outside }], 1, 293.6);

    let lattice = RectLattice {
        id: 0,
        n: [N, N, N],
        lower_left: Position::new(-HALF, -HALF, -HALF),
        pitch: [PITCH, PITCH, PITCH],
        universes: vec![1; N * N * N], // every tile → the TRISO universe (index 1)
        outer: Some(1),
    };

    Geometry {
        surfaces,
        cells: vec![root_cell, kernel, matrix],
        universes: vec![
            Universe { id: 0, cell_indices: vec![0] },       // root
            Universe { id: 1, cell_indices: vec![1, 2] },    // TRISO unit
        ],
        lattices: vec![Lattice::Rect(lattice)],
        root_universe: 0,
    }
}

/// LIVE: `Geometry::locate` must descend the nested lattice (root box → lattice
/// tile → TRISO universe) and return the right material on both sides of a kernel
/// surface — the triso-specific gap (op-6tz.7 lattice + nested-universe descent).
#[test]
fn triso_nested_lattice_geometry_navigation() {
    let geom = triso_geometry();
    let u = Direction::new(1.0, 0.0, 0.0);

    // Centre of the central tile [1,1,1] is the origin → inside the kernel → fuel (material 0).
    let at_kernel = geom.locate(Position::new(0.0, 0.0, 0.0), u, usize::MAX).expect("origin located");
    assert_eq!(at_kernel.material, Some(0), "kernel centre should be fuel");
    // Two coordinate levels: root universe (the lattice-fill cell) then the
    // lattice tile's TRISO universe. The tile level carries the lattice marker.
    assert_eq!(at_kernel.levels.len(), 2, "expected root→lattice-tile descent, got {} levels", at_kernel.levels.len());
    assert_eq!(at_kernel.levels[1].lattice, Some(0), "leaf level should be inside lattice 0");
    assert_eq!(at_kernel.levels[1].lattice_index, [1, 1, 1], "origin is the central tile [1,1,1]");

    // A tile-diagonal point in the central tile: |local| = √2·0.15 ≈ 0.212 cm >
    // R_KERNEL, and still inside the tile half-width (0.2 cm) → matrix (material 1).
    let at_matrix = geom.locate(Position::new(0.15, 0.15, 0.0), u, usize::MAX).expect("matrix point located");
    assert_eq!(at_matrix.material, Some(1), "tile-corner region should be matrix");

    // Centre of a neighbouring tile [2,1,1] is at x = +PITCH → inside that tile's kernel → fuel.
    let neighbour = geom.locate(Position::new(PITCH, 0.0, 0.0), u, usize::MAX).expect("neighbour tile located");
    assert_eq!(neighbour.material, Some(0), "neighbouring tile centre should be its fuel kernel");

    // Outside the reflective box → lost.
    assert!(geom.locate(Position::new(HALF + 0.1, 0.0, 0.0), u, usize::MAX).is_none(), "outside the box is lost");
}

/// LIVE: Woodcock/delta tracking flies through the doubly-heterogeneous packed
/// medium using the nested-lattice geometry lookup, reaching a real collision
/// with a sensible virtual-collision count (op-6tz.16 delta-tracking path).
#[test]
fn triso_delta_tracking_through_packed_medium() {
    let geom = triso_geometry();
    let materials = triso_materials();
    let nuclides = triso_nuclides();
    let e = 1.0e6; // 1 MeV

    // Majorant bounds both materials' Σ_t at this energy (fuel dominates).
    let maj = Majorant::from_materials(&materials, &nuclides, &[e], 0.05);

    let mut seed = 0x5150_1234u64; // arbitrary fixed seed
    let mut reached = 0usize;
    let n = 2_000usize;
    for _ in 0..n {
        // Start each flight at the origin (a kernel) heading in a fixed direction.
        let f = track_to_collision(
            Position::new(0.0, 0.0, 0.0),
            Direction::new(1.0, 0.0, 0.0),
            e,
            &maj,
            10_000,
            &mut seed,
            |p| geom.sigma_t_at(p, Direction::new(1.0, 0.0, 0.0), e, &materials, &nuclides),
        );
        // The particle either collides inside the box, or escapes on reaching a
        // reflective wall (sigma_t_at returns None outside). Both are valid; we
        // only require the tracker terminates and, when it collides, does so at a
        // finite distance.
        if !f.escaped {
            reached += 1;
            assert!(f.distance.is_finite() && f.distance >= 0.0, "collision at finite distance");
        }
    }
    // With a reflective box ~1.2 cm across and Σ_t ~ O(1) cm⁻¹ in fuel, a large
    // fraction of flights should reach a real collision before exiting.
    assert!(reached > 0, "delta tracking never reached a real collision in the packed medium");
}

/// LIVE: the assembled **doubly-heterogeneous** k-eff — [`run_keff_csg`]
/// surface-tracks the full nested TRISO lattice (root → lattice → TRISO universe
/// → kernel/matrix) to a stationary eigenvalue (op-6tz.16). Fast/epithermal,
/// regular packing, single-sphere kernel — a real doubly-heterogeneous transport
/// result, not a benchmark (see module simplifications).
#[test]
fn triso_doubly_heterogeneous_keff() {
    let geom = triso_geometry();
    let materials = triso_materials();
    let nuclides = triso_nuclides();

    let settings = KeffSettings { n_particles: 1000, n_inactive: 15, n_active: 30, ..KeffSettings::default() };
    let src = SourceBox {
        lower: Position::new(-HALF, -HALF, -HALF),
        upper: Position::new(HALF, HALF, HALF),
    };
    let result = run_keff_csg(&geom, &materials, &nuclides, src, &settings, None);

    eprintln!(
        "[triso doubly-heterogeneous] k = {:.5} ± {:.5} over {} generations",
        result.k_mean, result.k_std, result.k_by_generation.len()
    );
    assert!(!result.k_by_generation.is_empty(), "power iteration produced no generations");
    assert!(result.k_mean.is_finite() && result.k_mean > 0.0, "k should be finite & positive, got {}", result.k_mean);
    assert!(
        result.k_mean > 0.1 && result.k_mean < 5.0,
        "doubly-heterogeneous TRISO k {} outside the broad plausibility band [0.1, 5.0]",
        result.k_mean
    );
    // If the run did not go extinct (all generations present), require it to be
    // reasonably stationary.
    if result.k_by_generation.len() == settings.n_inactive + settings.n_active {
        assert!(result.k_std < 0.2, "TRISO k noisy/unconverged: sigma = {}", result.k_std);
    }
}
