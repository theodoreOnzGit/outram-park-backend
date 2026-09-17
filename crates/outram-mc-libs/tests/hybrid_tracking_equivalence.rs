//! **THE GATE for hybrid delta/surface tracking** — `bn:op-867c.7`, gh #214.
//!
//! Nothing downstream may rely on hybrid tracking until this passes. It is the
//! maintainer-chosen acceptance for the whole Phase 1 API.
//!
//! # What is required, and why this shape
//!
//! Run a geometry where **pure surface** and **hybrid** are both valid, and
//! require them to agree within statistics. Delta tracking is unbiased under
//! any valid majorant, so a correct hybrid must give the *same eigenvalue* as
//! surface tracking on the same model — only the cost may differ.
//!
//! That is a stronger check than it looks, because a hybrid can go wrong in
//! ways that do not crash and do not obviously bias:
//!
//! - collide in the material the flight STARTED in rather than where it ended
//!   (exactly the defect found in `c6d9b5717a` and fixed in `9557686e99`),
//! - truncate at the wrong boundary, stopping at internal surfaces the tracker
//!   exists to cross,
//! - lose or double-count path length at the handoff.
//!
//! Each of those shifts `k` by an amount that reads as statistics until the
//! histories pile up.
//!
//! # The second gate: the absorber-isolation property
//!
//! The reason the API exists at all is that a strong absorber must not raise
//! the tracking cost inside a region that does not contain it. That is
//! measurable directly: the virtual-collision count inside a delta region must
//! be **insensitive to adding an absorber outside it**. A global majorant
//! fails this by construction.
//!
//! # Results (2026-09-17) — PASSED
//!
//! ## Gate 1: hybrid equals surface tracking
//!
//! | histories | surface k | hybrid k | difference | sigma |
//! |---|---|---|---|---|
//! | 3,000 | 0.999180 +/- 0.003623 | 0.986457 +/- 0.004122 | -1272.3 pcm | 2.32 |
//! | **12,000** | **0.993201 +/- 0.001410** | **0.991873 +/- 0.001617** | **-132.9 pcm** | **0.62** |
//!
//! **Both rows are reported, and the first one is why.** At 3,000 histories the
//! difference was -1272 pcm at 2.32 sigma -- inside the 4-sigma gate, but
//! equally consistent with zero and with a real ~1200 pcm bias. That is not
//! equivalence established, it is a gate too wide to resolve the claim resting
//! on it, and this crate has made exactly that mistake before (see the Godiva
//! bar history in `CLAUDE.md`).
//!
//! Quadrupling the histories shrank the difference ~10x while sigma halved. A
//! real bias does not do that -- it stays put and becomes MORE significant. So
//! the low-statistics result was noise, and the 12,000-history row is the
//! measurement: **-133 +/- 215 pcm, 0.62 sigma**.
//!
//! ## Gate 2: an absorber outside the region does not raise its cost
//!
//! | majorant | virtual collisions |
//! |---|---|
//! | region-local (bounds only the fuel) | **191,944** |
//! | global-style (also bounds an absent absorber) | **86,070,909** |
//!
//! **448x**, with `k` unchanged at 1.32 sigma. That is the isolation property
//! the whole API exists for, measured directly: an over-bound majorant costs
//! time and never accuracy, which is exactly why the cost comparison is the
//! meaningful one.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, run_keff_csg_hybrid, SourceBox};

const R_FUEL: f64 = 8.7407; // Godiva's critical radius, a known-multiplying body
const TEMP: f64 = 293.6;

fn heu() -> Option<Vec<Nuclide>> {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let load = |name: &str, f: &str| -> Option<Nuclide> {
        let p = base.join(f);
        p.exists().then_some(())?;
        Nuclide::from_endf_file(&p, name, TEMP, 1.0e-3).ok()
    };
    Some(vec![
        load("U235", "n-092_U_235-ENDF8.0.endf")?,
        load("U238", "n-092_U_238.endf")?,
    ])
}

fn fuel() -> Material {
    Material {
        id: 1,
        name: "HEU".into(),
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.4994e-2 },
            NuclideComponent { nuclide_idx: 1, atom_density: 2.4984e-3 },
        ],
        temperature: TEMP,
    }
}

/// A bare sphere of fuel. `delta` decides whether the fuel region declares
/// delta tracking; the geometry is otherwise IDENTICAL, so any difference in
/// `k` is the tracking method and nothing else.
fn sphere_geometry(delta: bool) -> Geometry {
    let surfaces = vec![SurfaceKind::Sphere(Sphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r: R_FUEL,
        bc: BoundaryType::Vacuum,
    })];
    let inside = vec![RegionToken::HalfSpace {
        surface_idx: 0,
        sense: HalfSpaceSense::Inside,
    }];
    let core = if delta {
        Cell::material(1, inside, 0, TEMP).delta_tracked(0)
    } else {
        Cell::material(1, inside, 0, TEMP)
    };
    Geometry {
        surfaces,
        cells: vec![core],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn settings(seed: u64) -> KeffSettings {
    KeffSettings {
        n_particles: 12_000,
        n_inactive: 20,
        n_active: 60,
        temperature_k: TEMP,
        seed,
        compute: ComputeType::CpuSingleThread,
        ..KeffSettings::default()
    }
}

fn source() -> SourceBox {
    SourceBox {
        lower: Position::new(-R_FUEL, -R_FUEL, -R_FUEL),
        upper: Position::new(R_FUEL, R_FUEL, R_FUEL),
    }
}

fn energy_grid() -> Vec<f64> {
    let (lo, hi) = (1.0e-4_f64, 2.0e7_f64);
    (0..4096)
        .map(|i| (lo.ln() + (hi.ln() - lo.ln()) * i as f64 / 4095.0).exp())
        .collect()
}

/// **THE GATE.** Pure surface tracking and hybrid (delta inside the fuel) must
/// give the same eigenvalue within combined statistics.
#[test]
fn hybrid_and_surface_tracking_agree() {
    let Some(nucs) = heu() else {
        eprintln!("SKIP: ENDF tapes not in this checkout");
        return;
    };
    let mats = vec![fuel()];
    let maj = Majorant::over_indices(&mats, &[0], &nucs, &energy_grid(), 0.3);

    let surf = run_keff_csg(
        &sphere_geometry(false), &mats, &nucs, source(), &settings(1), None,
    );
    let hyb = run_keff_csg_hybrid(
        &sphere_geometry(true), &mats, &nucs, &[maj], source(), &settings(1), None,
    );

    let dk = (hyb.k_mean - surf.k_mean) * 1.0e5;
    let sigma = ((surf.k_std.powi(2) + hyb.k_std.powi(2)).sqrt()) * 1.0e5;
    let z = dk.abs() / sigma.max(1.0e-12);

    println!("surface  k = {:.6} +/- {:.6}", surf.k_mean, surf.k_std);
    println!("hybrid   k = {:.6} +/- {:.6}", hyb.k_mean, hyb.k_std);
    println!("difference {dk:+.1} pcm, combined sigma {sigma:.1} pcm  ({z:.2} sigma)");
    println!("hybrid virtual collisions: {}", hyb.virtual_collisions);

    assert!(
        hyb.virtual_collisions > 0,
        "the hybrid arm reported ZERO virtual collisions, so its delta region \
         was never entered -- the test is not exercising what it claims"
    );
    assert!(
        z < 4.0,
        "hybrid and surface tracking must agree within statistics: {dk:+.1} pcm \
         apart, {z:.2} sigma. Delta tracking is unbiased under any valid \
         majorant, so a real difference is a defect in the handoff, the \
         truncation, or which material the collision used -- not physics."
    );
}

/// **The absorber-isolation property**, which is the entire reason the API
/// exists: a strong absorber OUTSIDE a delta region must not raise the tracking
/// cost inside it.
///
/// Both arms use the identical geometry and the identical region. Only the
/// majorant differs: one bounded over the region's own material, one bounded
/// over that plus a strong absorber the region does not contain. A global
/// majorant cannot pass this.
#[test]
fn an_absorber_outside_the_region_does_not_raise_its_cost() {
    let Some(nucs) = heu() else {
        eprintln!("SKIP: ENDF tapes not in this checkout");
        return;
    };
    let mats = vec![fuel()];
    let grid = energy_grid();

    // Region-local: bounds only the fuel.
    let local = Majorant::over_indices(&mats, &[0], &nucs, &grid, 0.3);
    // Global-style: also bounds a dense absorber that is NOWHERE in the model.
    let absorber = Material {
        id: 2,
        name: "absorber outside the region".into(),
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 5.0 }],
        temperature: TEMP,
    };
    let global = Majorant::from_materials(&[mats[0].clone(), absorber], &nucs, &grid, 0.3);

    let geom = sphere_geometry(true);
    let a = run_keff_csg_hybrid(&geom, &mats, &nucs, &[local], source(), &settings(2), None);
    let b = run_keff_csg_hybrid(&geom, &mats, &nucs, &[global], source(), &settings(2), None);

    println!("region-local majorant : {:>12} virtual collisions", a.virtual_collisions);
    println!("global-style majorant : {:>12} virtual collisions", b.virtual_collisions);
    let ratio = b.virtual_collisions as f64 / a.virtual_collisions.max(1) as f64;
    println!("cost ratio {ratio:.1}x");

    // Both must give the SAME eigenvalue -- an over-bound majorant costs time,
    // never accuracy. That is what makes the cost comparison meaningful.
    let dk = (b.k_mean - a.k_mean) * 1.0e5;
    let sigma = ((a.k_std.powi(2) + b.k_std.powi(2)).sqrt()) * 1.0e5;
    println!("k unchanged: {dk:+.1} pcm ({:.2} sigma)", dk.abs() / sigma.max(1e-12));
    assert!(
        dk.abs() / sigma.max(1.0e-12) < 4.0,
        "an over-bound majorant must not change k, only cost: {dk:+.1} pcm apart"
    );
    assert!(
        ratio > 2.0,
        "bounding an absorber the region does not contain should cost \
         materially more tracking steps; got {ratio:.2}x. If this is ~1, the \
         majorant is not actually being consulted and the isolation property is \
         untested."
    );
}
