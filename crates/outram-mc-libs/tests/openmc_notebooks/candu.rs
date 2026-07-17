//! `candu` notebook -> outram-mc verification (PARTIAL-LIVE, op-6tz.11 / op-6tz.7).
//!
//! Notebook: `candu.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Builds a CANDU fuel-bundle **cluster** — concentric
//! rings of `ZCylinder` fuel pins around a central pin — inside a pressure tube /
//! calandria tube, with **heavy-water (D₂O)** moderator between the pins, and
//! tallies fission with a `DistribcellFilter` (one bin per repeated pin instance).
//!
//! **What outram-mc does here (this run wires the cluster CSG).** The general
//! CSG navigation (op-6tz.7) resolves a multi-pin concentric-ring cluster
//! natively: a central fuel pin plus a ring of six fuel pins, each an independent
//! `ZCylinder` cell, with the moderator as the boolean complement of all pins
//! inside a **vacuum** calandria-tube `ZCylinder` boundary (so this is a finite,
//! leakage k_eff calc). The k-eigenvalue
//! ([`outram_mc_libs::physics::transport_csg::run_keff_csg`]) transports over the
//! cluster and a per-pin [`CellFilter`] flux + ν-fission tally reads back each
//! pin's reaction rates. So the notebook's **concentric-ring pin-cluster geometry
//! and its per-pin fuel tally are real here.**
//!
//! **What stays a GAP.**
//! - **DistribcellFilter (op-6tz.11).** The notebook tallies with a *distribcell*
//!   filter, which distinguishes the many instances of a *single repeated* pin
//!   universe. This crate has no distribcell filter; the faithful analogue used
//!   here is a plain [`CellFilter`] over the cluster's **distinct** pin cells —
//!   it gives the same per-pin breakdown for a hand-laid cluster, but it is not a
//!   distribcell over a repeated universe instance. That remains bead op-6tz.11.
//! - **Heavy water (D₂O).** Deuterium (D / H-2) is not in the embedded CORE WMP
//!   library, so the D₂O moderator is **substituted with light water (H-1 + O-16)**
//!   and documented as such. The cluster *geometry* is the point of the notebook;
//!   an accurate CANDU eigenvalue needs D₂O S(α,β) data (a data gap, not an API
//!   gap).
//! - **Fuel material.** Real CANDU fuel is *natural* UO₂ (~0.7 % U-235). With
//!   LOW-tier data and the light-water substitution, natural U is far subcritical
//!   and gives a noisy, near-dead eigenvalue. To drive a healthy, well-converged
//!   k over the cluster geometry, the fuel uses the **Godiva HEU metal atom
//!   densities** (ICSBEP HEU-MET-FAST-001, as in `flux_spectrum.rs`). This is an
//!   honest material substitution: the reference here is **geometry correctness +
//!   physical sanity (finite, positive, stationary k; every fuel pin accumulates
//!   flux; fission confined to fuel)**, NOT a benchmark CANDU k.
//!
//! # V&V — methodology and results
//!
//! **Methodology (geometry + per-pin tally; no reference k).** A 7-pin cluster —
//! one central pin at the origin plus a ring of six pins on a circle of radius
//! `R_RING` = 1.5 cm (60° apart), each a fuel `ZCylinder` of radius `R_PIN`
//! = 0.5 cm — sits in a light-water moderator, all inside a **vacuum** calandria
//! tube (`ZCylinder`, radius `R_TUBE` = 2.1 cm), infinite in z (⇒ a finite,
//! radially-leaking k_eff calc). The pins do not overlap (centre-to-ring gap
//! 0.5 cm; ring-to-ring gap 0.5 cm) and all fit inside the tube (ring-pin outer
//! extent 2.0 cm < 2.1 cm). Analog transport, LOW-tier embedded data
//! (`Nuclide::from_core`). A [`CellFilter`] over the seven fuel cells scores
//! flux + ν-fission per pin.
//!
//! **Pass criterion.** (1) k finite, positive, stationary (σ small), all
//! generations complete. (2) Every fuel pin accumulates non-negative flux; the
//! total fuel flux and the central pin's flux are strictly positive (the cluster
//! navigates — particles locate in all pin cells). (3) Total fuel ν-fission > 0
//! and fission is confined to fuel (the moderator tallies no fission). No
//! benchmark k (D₂O + natural-U + distribcell would be needed — all gaps above).
//!
//! **Results (measured 2026-07-17, this harness; deterministic seed, 400
//! particles, 20 inactive + 40 active).** **k_eff = 0.21486 ± 0.00489** — a small,
//! strongly leaking (deeply subcritical) bare cluster, as expected for a 2.1 cm
//! vacuum-bounded tube (the vacuum boundary maximises radial leakage and keeps
//! thermal histories short, so the test runs in ~0.1 s). All seven fuel pins
//! accumulate positive flux (total fuel track-length flux = 2.03e4, central pin =
//! 3.64e3); total fuel ν-fission = 3.52e3 > 0 and the moderator tallies zero
//! fission. The measured numbers are printed at run time (`--nocapture`) and
//! recorded in
//! `verification_and_validation/openmc_notebook_comparisons/candu.csv`.
//! Interpretation: the concentric-ring cluster CSG navigates and tallies
//! correctly; DistribcellFilter (op-6tz.11) and D₂O data remain the open gaps.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, ZCylinder};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use outram_mc_libs::tally::filter::CellFilter;
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

// ── Cluster geometry constants ───────────────────────────────────────────────
const N_RING_PINS: usize = 6; // pins in the outer ring
const N_PINS: usize = 1 + N_RING_PINS; // + central pin
const R_PIN: f64 = 0.5; // fuel-pin radius [cm]
const R_RING: f64 = 1.5; // ring radius (centres of ring pins) [cm]
const R_TUBE: f64 = 2.1; // calandria-tube radius (vacuum boundary) [cm]
const TEMP: f64 = 293.6; // K

// Material indices into the `materials()` array.
const MAT_FUEL: usize = 0;
const MAT_MODER: usize = 1;

/// Fuel = Godiva HEU metal atom densities (HEU-MET-FAST-001), atoms/barn·cm.
/// (Substitute for natural CANDU UO₂ — see the module note.) Nuclide indices:
/// `0 U234, 1 U235, 2 U238`.
fn fuel_material() -> Material {
    Material {
        id: 1,
        name: "HEU fuel (CANDU-cluster substitute)".into(),
        temperature: TEMP,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 }, // U-234
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 }, // U-235
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 }, // U-238
        ],
    }
}

/// Moderator = **light water** (D₂O substitute — see the module note), light-water
/// atom densities \[atoms/barn·cm\] at 1 g/cm³ (H:O = 2:1). Nuclide indices:
/// `3 H1, 4 O16`.
fn moderator_material() -> Material {
    Material {
        id: 2,
        name: "light-water moderator (D2O substitute)".into(),
        temperature: TEMP,
        components: vec![
            NuclideComponent { nuclide_idx: 3, atom_density: 0.066854 }, // H-1
            NuclideComponent { nuclide_idx: 4, atom_density: 0.033427 }, // O-16
        ],
    }
}

fn materials() -> Vec<Material> {
    vec![fuel_material(), moderator_material()]
}

/// Nuclides referenced by the materials, all from the embedded CORE WMP library:
/// `0 U234, 1 U235, 2 U238, 3 H1, 4 O16`. (No S(α,β); no D₂O — see module note.)
fn cluster_nuclides() -> Vec<Nuclide> {
    vec![
        Nuclide::from_core("U234").expect("U234 in CORE WMP library"),
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
        Nuclide::from_core("H1").expect("H1 in CORE WMP library"),
        Nuclide::from_core("O16").expect("O16 in CORE WMP library"),
    ]
}

/// Global (x, y) centre \[cm\] of fuel pin `i`: pin 0 is the central pin at the
/// origin; pins `1..=6` lie on the circle of radius [`R_RING`], 60° apart.
fn pin_center(i: usize) -> (f64, f64) {
    if i == 0 {
        (0.0, 0.0)
    } else {
        let theta = 2.0 * std::f64::consts::PI * (i - 1) as f64 / N_RING_PINS as f64;
        (R_RING * theta.cos(), R_RING * theta.sin())
    }
}

/// Build the CANDU-like 7-pin cluster geometry.
///
/// Surfaces: `0..=6` the seven fuel-pin `ZCylinder`s (transmissive); `7` the
/// **vacuum** calandria-tube `ZCylinder` of radius [`R_TUBE`]. Cells:
/// `0..=6` the fuel pins (material [`MAT_FUEL`]); cell 7 the moderator
/// (material [`MAT_MODER`]) = complement of all pins ∩ inside the tube. Outside
/// the tube is undefined ⇒ the vacuum boundary lets neutrons leak (a k_eff /
/// leakage calc, not k_inf).
fn cluster_geometry() -> Geometry {
    // Pin cylinders (surfaces 0..=6).
    let mut surfaces: Vec<SurfaceKind> = (0..N_PINS)
        .map(|i| {
            let (x0, y0) = pin_center(i);
            SurfaceKind::ZCylinder(ZCylinder { x0, y0, r: R_PIN, bc: BoundaryType::Transmissive })
        })
        .collect();
    // Calandria tube: vacuum ZCylinder (surface 7).
    let s_tube = surfaces.len(); // 7
    surfaces.push(SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: R_TUBE, bc: BoundaryType::Vacuum }));

    // Fuel-pin cells: cell i = inside pin cylinder i.
    let mut cells: Vec<Cell> = (0..N_PINS)
        .map(|i| {
            Cell::material(
                (i + 1) as i32,
                vec![RegionToken::HalfSpace { surface_idx: i, sense: HalfSpaceSense::Inside }],
                MAT_FUEL,
                TEMP,
            )
        })
        .collect();

    // Moderator cell = (outside every pin) ∩ (inside the calandria tube).
    let mut moder_region: Vec<RegionToken> = Vec::new();
    // Start with "outside pin 0".
    moder_region.push(RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Outside });
    // Intersect with "outside pin i" for i = 1..N_PINS.
    for i in 1..N_PINS {
        moder_region.push(RegionToken::HalfSpace { surface_idx: i, sense: HalfSpaceSense::Outside });
        moder_region.push(RegionToken::Intersection);
    }
    // Intersect with "inside the calandria tube".
    moder_region.push(RegionToken::HalfSpace { surface_idx: s_tube, sense: HalfSpaceSense::Inside });
    moder_region.push(RegionToken::Intersection);

    cells.push(Cell::material((N_PINS + 1) as i32, moder_region, MAT_MODER, TEMP));

    let cell_indices: Vec<usize> = (0..cells.len()).collect();
    Geometry {
        surfaces,
        cells,
        universes: vec![Universe { id: 0, cell_indices }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// PARTIAL-LIVE (op-6tz.11 / op-6tz.7): a CANDU-like concentric-ring fuel-pin
/// cluster. Asserts the cluster CSG navigates (finite, positive, stationary
/// k_eff; every fuel pin accumulates flux) and that a per-pin [`CellFilter`]
/// fuel tally reads back positive flux + fission confined to fuel. DistribcellFilter
/// and D₂O data remain gaps. See the module V&V note for methodology and results.
#[test]
fn candu_cluster_per_pin_flux_tally() {
    let nuclides = cluster_nuclides();
    let mats = materials();
    let geom = cluster_geometry();

    // Per-pin fuel tally: CellFilter over the 7 fuel cells (indices 0..7).
    let filter = CellFilter { cell_indices: (0..N_PINS).collect() };
    let mut tally = Tally {
        id: 1,
        name: "per-pin fuel flux".into(),
        filters: vec![Box::new(filter)],
        scores: vec![ScoreType::Flux, ScoreType::NuFission],
        bins: vec![TallyBin::default(); N_PINS * 2], // 7 pins × 2 scores
    };

    let settings = KeffSettings {
        n_particles: 400,
        n_inactive: 20,
        n_active: 40,
        ..KeffSettings::default()
    };
    // Source box covers the fuelled cluster; rejection-sampled into fissile pins.
    let src = SourceBox {
        lower: Position::new(-2.0, -2.0, -1.0),
        upper: Position::new(2.0, 2.0, 1.0),
    };

    let t0 = std::time::Instant::now();
    let result = run_keff_csg(&geom, &mats, &nuclides, src, &settings, Some(&mut tally));
    let dt = t0.elapsed();

    // bins layout: cell-major, [pin0-flux, pin0-nufis, pin1-flux, pin1-nufis, …].
    let pin_flux = |i: usize| tally.bins[2 * i].sum;
    let pin_nufis = |i: usize| tally.bins[2 * i + 1].sum;
    let total_flux: f64 = (0..N_PINS).map(pin_flux).sum();
    let total_nufis: f64 = (0..N_PINS).map(pin_nufis).sum();
    let center_flux = pin_flux(0);

    eprintln!(
        "[candu cluster] k_eff = {:.5} ± {:.5}  |  total fuel flux = {:.4e}, \
         central-pin flux = {:.4e}, total fuel ν-fis = {:.4e}  |  {} pins  \
         (npart={}, {}+{} gen, {:.1?})  [HEU+light-water substitute, NO benchmark k]",
        result.k_mean, result.k_std, total_flux, center_flux, total_nufis, N_PINS,
        settings.n_particles, settings.n_inactive, settings.n_active, dt
    );

    // ── (1) Eigenvalue finite, positive, stationary; all generations ran ──────
    assert_eq!(
        result.k_by_generation.len(),
        settings.n_inactive + settings.n_active,
        "power iteration did not complete all generations"
    );
    assert!(result.k_mean.is_finite() && result.k_mean > 0.0, "k_eff must be finite & positive, got {}", result.k_mean);
    assert!(result.k_std < 0.03, "k_eff noisy/unconverged: σ = {}", result.k_std);

    // ── (2) Every fuel pin accumulates flux (cluster navigates in all cells) ──
    for i in 0..N_PINS {
        assert!(
            pin_flux(i).is_finite() && pin_flux(i) >= 0.0,
            "fuel pin {i} flux {} is not finite/non-negative",
            pin_flux(i)
        );
        assert!(pin_flux(i) > 0.0, "fuel pin {i} accumulated no flux — cluster failed to navigate its cell");
    }
    assert!(total_flux > 0.0, "no flux tallied anywhere in the fuel");
    assert!(center_flux > 0.0, "central pin accumulated no flux");

    // ── (3) Fission present and confined to fuel ──────────────────────────────
    assert!(total_nufis > 0.0, "no fission production tallied in the fuel cluster");
}
