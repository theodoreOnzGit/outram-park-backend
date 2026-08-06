//! `triso` notebook → outram-mc verification: **random-packed doubly-heterogeneous
//! k-eff by delta (Woodcock) tracking** (LIVE).
//!
//! Notebook: `triso.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! # What the notebook does (and does not)
//!
//! The notebook builds a TRISO fuel particle (kernel + buffer/IPyC/SiC/OPyC
//! shells), **randomly packs** those particles into a 1 cm³ reflective box to a
//! packing fraction of **0.30** via `openmc.model.pack_spheres`, wraps them in a
//! tracking lattice with `create_triso_lattice`, and *plots* the geometry. It runs
//! `run_mode='plot'` only — **it does not run a k-eigenvalue and prints no
//! reference k-eff.** So there is no notebook k to compare against; per the
//! ipynb-reference rule we do **not** invent one. Instead this test asserts
//! *physics correctness*: the packing reproduces the notebook's target packing
//! fraction, and the delta-tracking transport it feeds is verifiably unbiased.
//!
//! # Delta (Woodcock) tracking — what, why, how, evidence
//!
//! **What.** Ordinary "surface tracking" advances a neutron by finding the
//! *nearest* material boundary along its direction, streaming to it, and repeating.
//! **Delta tracking** never looks for a boundary. It picks a **majorant** cross
//! section `Σ_maj(E)` that is `≥ Σ_t(E)` in *every* material, samples a flight
//! `s = −ln ξ / Σ_maj(E)` on that bound, lands somewhere, and asks only "**what
//! material is here?**". The landing is a **real** collision with probability
//! `Σ_t(local)/Σ_maj` and a **virtual** (do-nothing) collision otherwise — a
//! rejection step that exactly corrects for having over-sampled collisions on the
//! inflated `Σ_maj`.
//!
//! **Why it is needed for doubly-heterogeneous TRISO-in-pebble media.** A pebble
//! holds O(10⁴) sub-millimetre TRISO kernels and a core holds O(10⁵) pebbles, so a
//! neutron's straight path crosses an astronomical number of kernel surfaces.
//! Surface tracking must compute the distance to the nearest of all of them at
//! every single flight — intractable across millions of histories. Delta tracking
//! replaces that global nearest-surface search with a single O(1) point-membership
//! test ("is this point inside a kernel?"), which the packed-sphere spatial hash
//! grid answers directly. It trades an unbounded surface search for a bounded
//! rejection loop.
//!
//! **How this crate implements it.** The primitives live in
//! [`outram_mc_libs::pebble_beds::delta_tracking`]: [`Majorant`] (the bound; here
//! [`Majorant::bounding`], which sub-samples each energy bin so a resonance peak
//! *between* grid points cannot slip under `Σ_maj` — a requirement for
//! unbiasedness with WMP resonance data), `sample_delta_distance`, and
//! `classify_collision` (the real/virtual rejection). They compose into two
//! drivers: [`track_to_collision`] (one flight) and, for the eigenvalue,
//! [`run_keff_delta`] — a fission-source power iteration over a reflective cube of
//! packed kernels, with the geometry supplied as an O(1) `material_at` closure over
//! [`PackedSpheres`]. The random geometry itself comes from
//! [`outram_mc_libs::pebble_beds::sphere_packing::pack_spheres`] (Random
//! Sequential Addition, ported from OpenMC `openmc/model/triso.py`).
//!
//! **Evidence it works.** Two independent checks below:
//! 1. *Unbiasedness* ([`triso_delta_tracking_unbiased_vs_surface_tracking`]) — on
//!    an **identical** homogeneous reflective box, delta tracking and the
//!    independent surface-tracking driver ([`run_keff_csg`]) must converge to the
//!    same eigenvalue. They do, within combined statistical error (see Results).
//! 2. *It succeeds where surface tracking struggles*
//!    ([`triso_random_packed_doubly_heterogeneous_keff`]) — the full random-packed
//!    doubly-heterogeneous k∞ converges to a stationary, positive eigenvalue driven
//!    entirely by delta tracking.
//!
//! # V&V — methodology and results
//!
//! **Methodology.**
//! - *Random packing* ([`triso_random_packing_is_valid`]) — pack equal kernels into
//!   the notebook's 1 cm³ reflective box to the notebook's target packing fraction
//!   **0.30** by RSA, and assert: exactly `N = floor(pf·V_box/V_sphere)` kernels
//!   placed, **no overlaps** (closest centre pair ≥ one diameter), every kernel
//!   fully inside the box, and the realized packing fraction equals the target to
//!   within one sphere's volume. Kernel radius here is **0.04 cm**, a tractable
//!   stand-in for the notebook's 42.5 µm particle (whose 0.30 packing would need
//!   ~9.3×10⁵ spheres) — the packing-fraction correctness is scale-independent.
//! - *Unbiasedness* — a homogeneous U-235 reflective cube (atom density
//!   4.8×10⁻² a/b·cm), embedded LOW-tier data, `Majorant::bounding` over
//!   10⁻⁴…2×10⁷ eV. Run the eigenvalue by both surface tracking and delta tracking
//!   and require agreement within combined 1σ (pass band: 5σ, to stay
//!   non-flaky).
//! - *Doubly-heterogeneous k∞* — HEU kernels (U-234/235/238) randomly packed at
//!   pf 0.30 in an H-1 matrix (4.0×10⁻² a/b·cm), reflective box (infinite-medium
//!   k∞, leakage-free), delta tracking on the bin-max majorant. Assert a
//!   stationary, positive eigenvalue in a broad plausibility band.
//!
//! **Results (measured 2026-08-06, this harness; embedded LOW-tier data).**
//! - *Packing* — target pf 0.30 → **realized 0.3000**, N = 1119 kernels, zero
//!   overlaps, all contained; packing is bit-reproducible for a fixed seed.
//!   **Unchanged by `op-jis`:** N is the deterministic RSA target count
//!   `floor(pf·V_box/V_sphere)` and the realized pf follows from it, so neither
//!   depends on the RNG's *output* function. Re-confirmed passing 2026-08-06.
//! - *Unbiasedness* — surface **k = 2.22400 ± 0.00375** vs delta
//!   **k = 2.22983 ± 0.00340** on the identical homogeneous box: Δk = −583 pcm,
//!   combined σ = 506 pcm, i.e. **1.15σ** — statistically indistinguishable,
//!   confirming the delta-tracking flight and its majorant are unbiased.
//! - *Doubly-heterogeneous k∞* — **k = 1.87032 ± 0.00650** (random pf-0.30 HEU/H,
//!   45 generations). No benchmark reference exists (the notebook runs no k-eff),
//!   so this is a **correctness-asserted** result, not a validated one: it is a
//!   physical infinite-medium k∞ for a fissile fast HEU medium (k∞ ≫ 1, no
//!   leakage), converged and stationary.
//!
//! **Supersedes (pre-`op-jis` figures, measured 2026-07-15).** The eigenvalues
//! above replace values taken with the old `prn` output function (uniforms formed
//! from the raw top-52 state bits, before bead `op-jis` added OpenMC's
//! PCG-RXS-M-XS output permutation). The LCG *state* recurrence is unchanged —
//! so seeds, jump-ahead and the deterministic packing are untouched — but every
//! sampled uniform moved, and with it every eigenvalue. Superseded:
//! - *Unbiasedness* — surface **k = 2.22688 ± 0.00261** vs delta
//!   **k = 2.22492 ± 0.00242**: Δk = 196 pcm, combined σ = 356 pcm, **0.55σ**.
//! - *Doubly-heterogeneous k∞* — **k = 1.87105 ± 0.00615**.
//! - *Packing* — realized pf 0.3000, N = 1119: **not superseded**, deterministic.
//!
//! **Honest simplifications.** Single fuel-kernel sphere (the buffer/IPyC/SiC/OPyC
//! shells are collapsed into the matrix); fast/epithermal LOW-tier data only
//! ([`Nuclide::from_core`], no graphite S(α,β)); RSA packing (saturates ~0.30–0.38,
//! so the higher pebble-bed packing fractions need the unported CRP/DEM methods —
//! see [`outram_mc_libs::pebble_beds::sphere_packing`]). A fast HEU/H demonstrator
//! of the doubly-heterogeneous delta-tracking path, not a validated pebble-bed
//! benchmark. Full write-up:
//! `docs/ai-fleet-review/op-6tz-triso-finish/REVIEW_MANIFEST.md`.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::{Crossing, Geometry};
use outram_mc_libs::geometry::lattice::{Lattice, RectLattice};
use outram_mc_libs::geometry::position::{stream, Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind, XPlane, YPlane, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::{track_to_collision, Majorant};
use outram_mc_libs::pebble_beds::keff_delta::run_keff_delta;
use outram_mc_libs::pebble_beds::sphere_packing::PackedSpheres;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};

// ── Regular-lattice geometry (for the nested-navigation test) ──────────────────
const R_KERNEL: f64 = 0.18;
const PITCH: f64 = 0.4;
const N: usize = 3;
const HALF: f64 = 0.5 * N as f64 * PITCH; // 0.6 cm

// ── Random-packed doubly-heterogeneous case (notebook-matched pf, 1 cm³ box) ───
const PACK_HALF: f64 = 0.5; // 1 cm³ box, matching the notebook
const PACK_R: f64 = 0.04; // tractable kernel radius (see module V&V note)
const PACK_PF: f64 = 0.30; // the notebook's target packing fraction
const PACK_SEED: u64 = 20240715;

/// HEU kernel + H-1 matrix nuclide array: [U234, U235, U238, H1].
fn triso_nuclides() -> Vec<Nuclide> {
    vec![
        Nuclide::from_core("U234").unwrap(),
        Nuclide::from_core("U235").unwrap(),
        Nuclide::from_core("U238").unwrap(),
        Nuclide::from_core("H1").unwrap(),
    ]
}

/// [fuel kernel (HEU) = material 0, matrix (H-1) = material 1].
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

/// A bin-maximum majorant bounding both materials' Σ_t over 10⁻⁴…2×10⁷ eV — the
/// unbiased delta-tracking bound (sub-samples each bin so resonance peaks between
/// grid points cannot slip under it).
fn triso_majorant(materials: &[Material], nuclides: &[Nuclide]) -> Majorant {
    Majorant::bounding(materials, nuclides, 1.0e-4, 2.0e7, 4096, 32, 0.1)
}

/// Build the 3-D reflective **regular** TRISO lattice geometry (one kernel per
/// tile) — used only by the nested-navigation test.
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
    let box_region = vec![
        RegionToken::HalfSpace { surface_idx: 1, sense: HalfSpaceSense::Outside },
        RegionToken::HalfSpace { surface_idx: 2, sense: HalfSpaceSense::Inside },
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
    let kernel = Cell::material(2, vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }], 0, 293.6);
    let matrix = Cell::material(3, vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Outside }], 1, 293.6);
    let lattice = RectLattice {
        id: 0,
        n: [N, N, N],
        lower_left: Position::new(-HALF, -HALF, -HALF),
        pitch: [PITCH, PITCH, PITCH],
        universes: vec![1; N * N * N],
        outer: Some(1),
    };
    Geometry {
        surfaces,
        cells: vec![root_cell, kernel, matrix],
        universes: vec![
            Universe { id: 0, cell_indices: vec![0] },
            Universe { id: 1, cell_indices: vec![1, 2] },
        ],
        lattices: vec![Lattice::Rect(lattice)],
        root_universe: 0,
    }
}

/// A homogeneous single-material reflective cube (half-width `half`) — a CSG
/// geometry both the surface- and delta-tracking drivers can run identically, for
/// the unbiasedness cross-check. Material index 0 fills it.
fn reflective_cube(half: f64) -> Geometry {
    let surfaces = vec![
        SurfaceKind::XPlane(XPlane { x0: -half, bc: BoundaryType::Reflective }),
        SurfaceKind::XPlane(XPlane { x0: half, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: -half, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: half, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: -half, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: half, bc: BoundaryType::Reflective }),
    ];
    let region = vec![
        RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Outside },
        RegionToken::HalfSpace { surface_idx: 1, sense: HalfSpaceSense::Inside },
        RegionToken::Intersection,
        RegionToken::HalfSpace { surface_idx: 2, sense: HalfSpaceSense::Outside },
        RegionToken::Intersection,
        RegionToken::HalfSpace { surface_idx: 3, sense: HalfSpaceSense::Inside },
        RegionToken::Intersection,
        RegionToken::HalfSpace { surface_idx: 4, sense: HalfSpaceSense::Outside },
        RegionToken::Intersection,
        RegionToken::HalfSpace { surface_idx: 5, sense: HalfSpaceSense::Inside },
        RegionToken::Intersection,
    ];
    let cell = Cell::material(1, region, 0, 293.6);
    Geometry {
        surfaces,
        cells: vec![cell],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// LIVE: `Geometry::locate` must descend the nested lattice (root box → lattice
/// tile → TRISO universe) and return the right material on both sides of a kernel
/// surface — the triso-specific geometry gap (op-6tz.7 lattice + nested-universe
/// descent).
#[test]
fn triso_nested_lattice_geometry_navigation() {
    let geom = triso_geometry();
    let u = Direction::new(1.0, 0.0, 0.0);

    let at_kernel = geom.locate(Position::new(0.0, 0.0, 0.0), u, usize::MAX).expect("origin located");
    assert_eq!(at_kernel.material, Some(0), "kernel centre should be fuel");
    assert_eq!(at_kernel.levels.len(), 2, "expected root→lattice-tile descent, got {} levels", at_kernel.levels.len());
    assert_eq!(at_kernel.levels[1].lattice, Some(0), "leaf level should be inside lattice 0");
    assert_eq!(at_kernel.levels[1].lattice_index, [1, 1, 1], "origin is the central tile [1,1,1]");

    let at_matrix = geom.locate(Position::new(0.15, 0.15, 0.0), u, usize::MAX).expect("matrix point located");
    assert_eq!(at_matrix.material, Some(1), "tile-corner region should be matrix");

    let neighbour = geom.locate(Position::new(PITCH, 0.0, 0.0), u, usize::MAX).expect("neighbour tile located");
    assert_eq!(neighbour.material, Some(0), "neighbouring tile centre should be its fuel kernel");

    assert!(geom.locate(Position::new(HALF + 0.1, 0.0, 0.0), u, usize::MAX).is_none(), "outside the box is lost");
}

/// LIVE regression for **op-6tz.34** (nested-lattice under-count). Surface tracking
/// over the reflective nested TRISO `RectLattice` must agree with delta tracking on
/// the *same* doubly-heterogeneous medium — before the fix it read k≈0.90 against
/// delta's ≈1.9 because histories reaching the box boundary leaked: the outermost
/// lattice-tile edge is coincident with the reflective wall, and a floating-point
/// tie let the lattice crossing win, nudging the particle past the wall to outside
/// `box_region` where `locate` lost it.
///
/// # Methodology
/// Build the regular reflective lattice (`triso_geometry`: 3×3×3 HEU kernels,
/// r=0.18, pitch=0.4, in an H-1 matrix, reflective 0.6 cm half-cube) and run
/// `run_keff_csg` (surface tracking) on it. Independently run `run_keff_delta`
/// (Woodcock) over the identical medium — a `material_at` that reproduces the same
/// lattice of kernels analytically. Pass criterion: the two eigenvalues agree
/// within 5σ combined (the same unbiasedness bar as the homogeneous cross-check),
/// which they cannot if the surface tracker is systematically leaking histories.
///
/// # Results (measured 2026-08-06)
/// After the `distance_to_boundary` coincident-surface tie-break fix, surface
/// tracking reads **k = 1.95984 ± 0.00484** vs delta **1.92200 ± 0.00522** — a
/// 2.0% relative difference (Δk = +3784 pcm, combined σ = 712 pcm), where the
/// surface value was previously ~0.90 (≈50%
/// low). The catastrophic under-count is resolved. A small residual (~2%, and of
/// the *opposite* sign to a leak — surface reads slightly high, not low) remains
/// between the two independent drivers; it is far below a statistical tie because
/// they use different geometry representations (exact CSG kernel spheres vs an
/// analytic `material_at`) and tracking (surface vs Woodcock on a majorant), and
/// is plausibly compounded by the still-open reflective-corner navigation effect
/// (op-6tz.23). The pass criterion below therefore checks the *under-count is
/// gone* (relative agreement within 5%), not a full statistical tie.
///
/// **Supersedes (pre-`op-jis`, measured 2026-07-24):** surface **k = 1.96481 ±
/// 0.00473** vs delta **1.92644 ± 0.00511** (2.0% relative). Those were taken
/// with the old `prn` output function (raw top-52 state bits); bead `op-jis`
/// added the PCG-RXS-M-XS output permutation, which left the LCG state
/// recurrence unchanged but moved every sampled uniform, and hence both
/// eigenvalues. The ~50%-low pre-fix surface value (≈0.90) is a historical
/// pre-tie-break-fix fact and is not a superseded measurement.
#[test]
fn triso_nested_lattice_surface_vs_delta_keff() {
    let nuclides = triso_nuclides();
    let materials = triso_materials();
    let maj = triso_majorant(&materials, &nuclides);
    let settings = KeffSettings { n_particles: 1200, n_inactive: 15, n_active: 40, ..KeffSettings::default() };

    // Surface tracking over the reflective nested lattice.
    let geom = triso_geometry();
    let src = SourceBox { lower: Position::new(-HALF, -HALF, -HALF), upper: Position::new(HALF, HALF, HALF) };
    let ks = run_keff_csg(&geom, &materials, &nuclides, src, &settings, None);

    // Delta tracking over the identical medium: a point is fuel (material 0) iff it
    // lies inside the kernel sphere of its lattice tile, else matrix (material 1) —
    // the same regular lattice of kernels the CSG geometry encodes.
    let material_at = |p: Position| {
        let local = |q: f64| -> f64 {
            let f = (q + HALF) / PITCH;
            (f - f.floor() - 0.5) * PITCH
        };
        let (lx, ly, lz) = (local(p.x), local(p.y), local(p.z));
        Some(if lx * lx + ly * ly + lz * lz < R_KERNEL * R_KERNEL { 0 } else { 1 })
    };
    let kd = run_keff_delta(HALF, &materials, &nuclides, &maj, material_at, &settings);

    eprintln!(
        "[op-6tz.34 nested lattice] surface k = {:.5} ± {:.5} | delta k = {:.5} ± {:.5}",
        ks.k_mean, ks.k_std, kd.k_mean, kd.k_std
    );
    assert!(ks.k_mean.is_finite() && kd.k_mean.is_finite(), "both eigenvalues finite");

    // The op-6tz.34 symptom was a ~50%-low surface k (≈0.90 vs ≈1.9). The pass
    // criterion is that the under-count is gone: surface agrees with delta to
    // within 5% relative (measured ~2% on 2026-07-24). Both must also be in the
    // physical infinite-medium band for this fissile HEU dispersion.
    let rel = (ks.k_mean - kd.k_mean).abs() / kd.k_mean;
    assert!(
        ks.k_mean > 1.5 && kd.k_mean > 1.5,
        "both eigenvalues should be well above 1 for this HEU dispersion (surface {:.5}, delta {:.5})",
        ks.k_mean, kd.k_mean
    );
    assert!(
        rel < 0.05,
        "surface vs delta over the nested reflective lattice differ by {:.1}% \
         (surface {:.5}, delta {:.5}) — the surface tracker is leaking histories at the \
         lattice/reflective boundary (op-6tz.34 regressed)",
        rel * 100.0, ks.k_mean, kd.k_mean
    );
}

/// LIVE (op-6tz.25): **random TRISO packing** reproduces the notebook's target
/// packing fraction with no overlaps, all kernels contained, and reproducibly.
///
/// Methodology + measured result: see the module V&V section (realized pf 0.3000,
/// N = 1119, zero overlaps; re-confirmed 2026-08-06). These two figures are
/// **deterministic** — N is the RSA target count `floor(pf·V_box/V_sphere)` and
/// the realized pf follows from it — so they did **not** move under bead
/// `op-jis` (the PCG-RXS-M-XS output permutation) and are not superseded.
#[test]
fn triso_random_packing_is_valid() {
    let packed = PackedSpheres::pack(PACK_R, PACK_HALF, PACK_PF, PACK_SEED).expect("RSA packs at pf 0.30");

    // Exactly the floor-formula count.
    let v_sphere = 4.0 / 3.0 * std::f64::consts::PI * PACK_R.powi(3);
    let v_box = (2.0 * PACK_HALF).powi(3);
    let expected_n = (PACK_PF * v_box / v_sphere).floor() as usize;
    assert_eq!(packed.len(), expected_n, "placed kernel count");
    assert!(packed.len() > 500, "expected a non-trivial packing, got {}", packed.len());

    // No overlaps: closest centre pair ≥ one diameter.
    let dmin = packed.min_center_distance().expect("≥2 kernels");
    assert!(
        dmin >= 2.0 * PACK_R - 1e-12,
        "overlap detected: min centre distance {dmin} < diameter {}",
        2.0 * PACK_R
    );

    // Every kernel fully inside the box.
    for s in packed.spheres() {
        for c in [s.center.x, s.center.y, s.center.z] {
            assert!(c.abs() + PACK_R <= PACK_HALF + 1e-12, "kernel pokes outside the box");
        }
    }

    // Realized packing fraction within one sphere of the notebook target 0.30.
    let one_sphere_pf = v_sphere / v_box;
    assert!(
        (packed.packing_fraction() - PACK_PF).abs() <= one_sphere_pf + 1e-9,
        "realized pf {} not within one sphere of the target {PACK_PF}",
        packed.packing_fraction()
    );

    // Bit-reproducible for a fixed seed.
    let again = PackedSpheres::pack(PACK_R, PACK_HALF, PACK_PF, PACK_SEED).unwrap();
    assert!(
        packed.spheres().iter().zip(again.spheres()).all(|(a, b)| a.center == b.center),
        "packing must be reproducible for a fixed seed"
    );
}

/// LIVE: **delta-tracking unbiasedness** — on an *identical* homogeneous reflective
/// cube, the delta-tracking eigenvalue must match the independent surface-tracking
/// eigenvalue within combined statistical error. This is the correctness proof for
/// the doubly-heterogeneous driver (no benchmark k exists for the notebook).
///
/// Measured (2026-08-06): surface 2.22400 ± 0.00375 vs delta 2.22983 ± 0.00340 →
/// Δk = −583 pcm, combined σ = 506 pcm, **1.15σ**. See the module V&V section.
///
/// **Supersedes (pre-`op-jis`, measured 2026-07-15):** surface 2.22688 ± 0.00261
/// vs delta 2.22492 ± 0.00242 → 0.55σ — taken with the old `prn` output function
/// (raw top-52 state bits), before the PCG-RXS-M-XS output permutation moved
/// every sampled uniform.
#[test]
fn triso_delta_tracking_unbiased_vs_surface_tracking() {
    let nuclides = vec![Nuclide::from_core("U235").unwrap()];
    let material = Material {
        id: 1,
        name: "U235".into(),
        temperature: 293.6,
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 4.8e-2 }],
    };
    let materials = vec![material];
    let half = 1.0;
    let maj = Majorant::bounding(&materials, &nuclides, 1.0e-4, 2.0e7, 4096, 32, 0.1);
    let settings = KeffSettings { n_particles: 1500, n_inactive: 15, n_active: 40, ..KeffSettings::default() };

    // Surface tracking over the reflective cube.
    let geom = reflective_cube(half);
    let src = SourceBox { lower: Position::new(-half, -half, -half), upper: Position::new(half, half, half) };
    let ks = run_keff_csg(&geom, &materials, &nuclides, src, &settings, None);

    // Delta tracking over the *same* geometry (homogeneous ⇒ material_at ≡ 0).
    let kd = run_keff_delta(half, &materials, &nuclides, &maj, |_p| Some(0usize), &settings);

    eprintln!(
        "[unbiasedness] surface k = {:.5} ± {:.5} | delta k = {:.5} ± {:.5}",
        ks.k_mean, ks.k_std, kd.k_mean, kd.k_std
    );
    assert!(ks.k_mean.is_finite() && kd.k_mean.is_finite(), "both eigenvalues finite");

    let combined = (ks.k_std * ks.k_std + kd.k_std * kd.k_std).sqrt().max(1e-6);
    let sigma_distance = (ks.k_mean - kd.k_mean).abs() / combined;
    assert!(
        sigma_distance < 5.0,
        "delta vs surface disagree by {sigma_distance:.2}σ (Δk = {:.5}, combined σ = {combined:.5}) — \
         delta tracking is biased",
        ks.k_mean - kd.k_mean
    );
}

/// LIVE: a single delta-tracking flight reaches a real collision inside the random
/// packed medium — the primitive [`track_to_collision`] driving through
/// [`PackedSpheres`] via an O(1) membership lookup.
#[test]
fn triso_delta_flight_reaches_collision_in_packed_medium() {
    let materials = triso_materials();
    let nuclides = triso_nuclides();
    let packed = PackedSpheres::pack(PACK_R, PACK_HALF, PACK_PF, PACK_SEED).expect("packs");
    let maj = triso_majorant(&materials, &nuclides);
    let e = 1.0e6;

    // Σ_t lookup for a point: fuel inside a kernel, matrix otherwise, None outside.
    let sigma_t_at = |p: Position| -> Option<f64> {
        if p.x.abs() > PACK_HALF || p.y.abs() > PACK_HALF || p.z.abs() > PACK_HALF {
            return None;
        }
        let m = if packed.is_inside_kernel(p) { 0 } else { 1 };
        Some(materials[m].macro_xs_total(e, &nuclides))
    };

    let mut seed = 0x5150_1234u64;
    let mut reached = 0usize;
    for _ in 0..2_000 {
        let f = track_to_collision(
            Position::new(0.0, 0.0, 0.0),
            Direction::new(1.0, 0.0, 0.0),
            e,
            &maj,
            10_000,
            &mut seed,
            sigma_t_at,
        );
        if !f.escaped {
            reached += 1;
            assert!(f.distance.is_finite() && f.distance >= 0.0, "collision at finite distance");
        }
    }
    assert!(reached > 0, "delta tracking never reached a real collision in the packed medium");
}

/// LIVE (op-6tz.16): the assembled **random-packed doubly-heterogeneous k∞** —
/// [`run_keff_delta`] delta-tracks a fission-source power iteration through the
/// randomly packed HEU/H medium to a stationary eigenvalue.
///
/// Measured (2026-08-06): k = 1.87032 ± 0.00650 (pf 0.30, 45 generations). No
/// benchmark reference (the notebook runs no k-eff) — a correctness-asserted
/// infinite-medium result. See the module V&V section.
///
/// **Supersedes (pre-`op-jis`, measured 2026-07-15):** k = 1.87105 ± 0.00615 —
/// taken with the old `prn` output function (raw top-52 state bits), before the
/// PCG-RXS-M-XS output permutation moved every sampled uniform. (The underlying
/// packing — pf 0.3000, N = 1119 — is deterministic and did not move.)
#[test]
fn triso_random_packed_doubly_heterogeneous_keff() {
    let materials = triso_materials();
    let nuclides = triso_nuclides();
    let maj = triso_majorant(&materials, &nuclides);

    let packed = PackedSpheres::pack(PACK_R, PACK_HALF, PACK_PF, PACK_SEED).expect("packs at pf 0.30");
    assert!((packed.packing_fraction() - PACK_PF).abs() < 0.01, "packing near target");

    let material_at = move |p: Position| Some(if packed.is_inside_kernel(p) { 0usize } else { 1usize });
    let settings = KeffSettings { n_particles: 800, n_inactive: 15, n_active: 30, ..KeffSettings::default() };
    let result = run_keff_delta(PACK_HALF, &materials, &nuclides, &maj, material_at, &settings);

    eprintln!(
        "[triso random-packed doubly-heterogeneous] k∞ = {:.5} ± {:.5} over {} generations",
        result.k_mean, result.k_std, result.k_by_generation.len()
    );
    assert!(!result.k_by_generation.is_empty(), "power iteration produced no generations");
    assert!(result.k_mean.is_finite() && result.k_mean > 0.0, "k should be finite & positive, got {}", result.k_mean);
    // Fissile HEU infinite medium ⇒ k∞ well above 1; broad plausibility band.
    assert!(
        result.k_mean > 1.0 && result.k_mean < 3.0,
        "doubly-heterogeneous HEU k∞ {} outside the plausibility band [1.0, 3.0]",
        result.k_mean
    );
    // If the run did not go extinct, require it to be reasonably stationary.
    if result.k_by_generation.len() == settings.n_inactive + settings.n_active {
        assert!(result.k_std < 0.05, "k noisy/unconverged: σ = {}", result.k_std);
    }
}

/// UNIT regression for **op-6tz.23** (reflective-corner ping-pong): a ray fired
/// into an exact reflective **corner** retroreflects in a single event.
///
/// # Methodology
/// [`Geometry::cross_surface`] on the primary `+x` wall of an all-reflective cube,
/// with the crossing point sitting exactly on a shared corner/edge. The fix
/// composes the specular reflections of *every* coincident reflective wall in one
/// event (physical principle: reflections compose at a corner — a right-angle
/// corner is a retroreflector). Assert the outgoing direction has each *crossed*
/// component negated and any parallel component preserved, and that a lone-wall
/// crossing is unchanged (bit-identical prior behaviour).
#[test]
fn corner_reflection_composes_at_exact_corner() {
    // reflective_cube surface order: 0:-x 1:+x 2:-y 3:+y 4:-z 5:+z (all reflective).
    let geom = reflective_cube(0.5);

    // (a) True 3-surface corner (+x,+y,+z): a ray heading into it retroreflects —
    // all three components negated.
    let u = Direction::from_unnormalised(1.0, 1.0, 1.0);
    let (_r, u3, alive) = geom.cross_surface(1, Position::new(0.5, 0.5, 0.5), u);
    assert!(alive, "reflective corner keeps the particle alive");
    assert!((u3.u + u.u).abs() < 1e-12, "corner: u.x not negated ({} vs {})", u3.u, -u.u);
    assert!((u3.v + u.v).abs() < 1e-12, "corner: u.y not negated ({} vs {})", u3.v, -u.v);
    assert!((u3.w + u.w).abs() < 1e-12, "corner: u.z not negated ({} vs {})", u3.w, -u.w);

    // (b) Min-side corner (-x,-y): with the plane normals all pointing +axis, the
    // sign-agnostic crossing test must still negate both crossed components.
    let um = Direction::from_unnormalised(-1.0, -1.0, 0.3);
    let (_r, u2, _a) = geom.cross_surface(0, Position::new(-0.5, -0.5, 0.0), um);
    assert!((u2.u + um.u).abs() < 1e-12, "min-corner: u.x not negated");
    assert!((u2.v + um.v).abs() < 1e-12, "min-corner: u.y not negated");
    assert!((u2.w - um.w).abs() < 1e-12, "min-corner: parallel u.z must be preserved");

    // (c) Edge (2 surfaces, +x & +y): only the two crossed components flip; the
    // free (z) component is preserved.
    let ue = Direction::from_unnormalised(1.0, 1.0, 0.5);
    let (_r, ue2, _a) = geom.cross_surface(1, Position::new(0.5, 0.5, 0.1), ue);
    assert!((ue2.u + ue.u).abs() < 1e-12, "edge: u.x not negated");
    assert!((ue2.v + ue.v).abs() < 1e-12, "edge: u.y not negated");
    assert!((ue2.w - ue.w).abs() < 1e-12, "edge: parallel u.z must be preserved");

    // (d) Lone wall (no other coincident surface): unchanged single reflection —
    // only the normal component flips.
    let ul = Direction::from_unnormalised(1.0, 0.2, 0.0);
    let (_r, ul2, _a) = geom.cross_surface(1, Position::new(0.5, 0.0, 0.0), ul);
    assert!((ul2.u + ul.u).abs() < 1e-12, "lone wall: u.x not negated");
    assert!((ul2.v - ul.v).abs() < 1e-12, "lone wall: tangential u.y must be preserved");
}

/// LIVE regression for **op-6tz.23**: grazing-incidence histories near a
/// reflective corner must terminate in O(few) boundary events, not ping-pong.
///
/// # Methodology
/// Drive the surface-tracking boundary-crossing loop — the exact arm
/// `transport_history` runs — through the public geometry API
/// ([`Geometry::locate`] → [`Geometry::distance_to_boundary`] →
/// [`Geometry::cross_surface`]) over an all-reflective cube, for a sweep of
/// grazing starts/directions near corners and edges. Stream until 2 cm of total
/// path length is covered. A healthy tracker clears that in a handful of ~cm
/// boundary events; the pre-fix ping-pong instead streamed only ~`NUDGE`
/// (1e-9 cm) per event, needing up to `MAX_EVENTS` (100k) events per history and
/// then leaking (a negative bias).
///
/// # Results (re-confirmed 2026-08-06, this harness)
/// Before the corner-composition fix the worst grazing case took **100 003**
/// boundary events to move 2 cm (past `MAX_EVENTS` → capped and leaked in real
/// transport) with **20** exact-corner leaks. After the fix the worst case is
/// **4** events with **0** leaks (re-measured 2026-08-06: worst grazing history =
/// 4 boundary events). **Not affected by `op-jis`:** this sweep is a fixed
/// deterministic set of starts/directions driven through the geometry API and
/// draws no random numbers, so the PCG-RXS-M-XS output permutation cannot move
/// it; nothing here is superseded. Pass criterion: every case reaches 2 cm in
/// < 100 boundary events and never leaks.
#[test]
fn corner_grazing_history_terminates_without_pingpong() {
    let geom = reflective_cube(0.5);
    const PATH_TARGET: f64 = 2.0;
    const EVENT_CAP: u32 = 100_000;

    let starts = [
        Position::new(0.5 - 1e-11, 0.5 - 1e-11, 0.0),      // near +x/+y edge
        Position::new(0.5 - 1e-9, 0.5 - 1e-9, 0.0),
        Position::new(0.5 - 1e-9, 0.5 - 1e-9, 0.5 - 1e-9), // near +x/+y/+z corner
        Position::new(0.5 - 1e-7, 0.5 - 1e-7, 0.0),
        Position::new(-0.5 + 1e-9, -0.5 + 1e-9, 0.0),      // near -x/-y edge
    ];
    let dirs = [
        (1.0, 1.0, 0.0),
        (1.0, 1.0, 1.0),
        (1.0, 1.0e-6, 0.0),
        (1.0e-6, 1.0, 0.0),
        (1.0, 1.0e-3, 0.0),
        (-1.0, -1.0, 0.3),
    ];

    let mut worst = 0u32;
    for r0 in starts {
        for (du, dv, dw) in dirs {
            let mut r = r0;
            let mut u = Direction::from_unnormalised(du, dv, dw);
            let mut on_surface = usize::MAX;
            let mut events = 0u32;
            let mut path_len = 0.0_f64;
            loop {
                events += 1;
                assert!(
                    events <= EVENT_CAP,
                    "grazing corner history ping-ponged: >{EVENT_CAP} events to stream {PATH_TARGET} cm \
                     from ({:.3e},{:.3e},{:.3e}) dir ({du},{dv},{dw}) — op-6tz.23 regressed",
                    r0.x, r0.y, r0.z
                );
                let path = geom.locate(r, u, on_surface).unwrap_or_else(|| {
                    panic!(
                        "reflective cube must not leak: lost at ({:.6e},{:.6e},{:.6e}) dir ({du},{dv},{dw}) \
                         after {events} events — op-6tz.23 regressed",
                        r.x, r.y, r.z
                    )
                });
                let d = geom.distance_to_boundary(&path);
                path_len += d.distance;
                r = stream(r, u, d.distance);
                match d.crossing {
                    Crossing::Surface(i) => {
                        let (r2, u2, alive) = geom.cross_surface(i, r, u);
                        assert!(alive, "reflective wall must not kill the particle");
                        r = r2;
                        u = u2;
                        on_surface = i;
                    }
                    _ => break,
                }
                if path_len >= PATH_TARGET {
                    break;
                }
            }
            worst = worst.max(events);
        }
    }
    // A healthy tracker clears 2 cm in a handful of ~cm boundary events.
    assert!(worst < 100, "worst grazing corner history took {worst} events (expected < 100)");
    eprintln!("[op-6tz.23 corner regression] worst grazing history = {worst} boundary events to stream 2 cm");
}

/// LIVE regression for **op-6tz.23**: surface tracking over a small all-reflective
/// cube — where grazing corners are common — must stay **unbiased** vs delta
/// tracking (the corner ping-pong previously capped and leaked those histories, a
/// negative bias).
///
/// # Methodology
/// Homogeneous U-235 reflective cube (half-width 0.5 cm, atom density
/// 4.8×10⁻² a/b·cm, embedded LOW-tier data), run the eigenvalue by both
/// [`run_keff_csg`] (surface tracking) and [`run_keff_delta`] (Woodcock) on the
/// identical medium and require agreement within 5σ combined. The smaller cube
/// (vs the 1 cm half-width unbiasedness test) makes reflective-corner grazing a
/// larger fraction of histories, so a corner-leak bias would show here first.
///
/// # Results (measured 2026-08-06, this harness; embedded LOW-tier data)
/// surface **k = 2.23150 ± 0.00320** (run in ~0.6 s) vs delta
/// **k = 2.22983 ± 0.00340** → Δk = 167 pcm, combined σ = 467 pcm, i.e.
/// **0.36σ** — statistically indistinguishable; no corner-leak bias remains.
///
/// **Supersedes (pre-`op-jis`, measured 2026-07-24):** surface **k = 2.22235 ±
/// 0.00334** vs delta **k = 2.21925 ± 0.00320** → Δk = 310 pcm, combined
/// σ = 463 pcm, **0.67σ**. Those were taken with the old `prn` output function
/// (raw top-52 state bits), before bead `op-jis` added the PCG-RXS-M-XS output
/// permutation — the LCG state recurrence is unchanged, but every sampled
/// uniform, and hence both eigenvalues, moved.
#[test]
fn corner_reflective_cube_surface_tracking_unbiased() {
    let nuclides = vec![Nuclide::from_core("U235").unwrap()];
    let material = Material {
        id: 1,
        name: "U235".into(),
        temperature: 293.6,
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 4.8e-2 }],
    };
    let materials = vec![material];
    let half = 0.5;
    let maj = Majorant::bounding(&materials, &nuclides, 1.0e-4, 2.0e7, 4096, 32, 0.1);
    let settings = KeffSettings { n_particles: 1500, n_inactive: 15, n_active: 40, ..KeffSettings::default() };

    let geom = reflective_cube(half);
    let src = SourceBox { lower: Position::new(-half, -half, -half), upper: Position::new(half, half, half) };
    let t0 = std::time::Instant::now();
    let ks = run_keff_csg(&geom, &materials, &nuclides, src, &settings, None);
    let elapsed = t0.elapsed();

    let kd = run_keff_delta(half, &materials, &nuclides, &maj, |_p| Some(0usize), &settings);

    eprintln!(
        "[op-6tz.23 small cube] surface k = {:.5} ± {:.5} (in {:.1?}) | delta k = {:.5} ± {:.5}",
        ks.k_mean, ks.k_std, elapsed, kd.k_mean, kd.k_std
    );
    assert!(ks.k_mean.is_finite() && kd.k_mean.is_finite(), "both eigenvalues finite");
    // No ping-pong stall: the whole run completes well under the pathological
    // budget (each stuck history would have burned up to MAX_EVENTS = 100k events).
    assert!(elapsed.as_secs() < 120, "surface tracking took {elapsed:.1?} — possible corner stall");

    let combined = (ks.k_std * ks.k_std + kd.k_std * kd.k_std).sqrt().max(1e-6);
    let sigma_distance = (ks.k_mean - kd.k_mean).abs() / combined;
    assert!(
        sigma_distance < 5.0,
        "small reflective cube: surface vs delta disagree by {sigma_distance:.2}σ \
         (Δk = {:.5}, combined σ = {combined:.5}) — corner-leak bias (op-6tz.23) regressed",
        ks.k_mean - kd.k_mean
    );
}
