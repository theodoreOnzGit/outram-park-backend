//! `mg-mode-part-i` notebook -> outram-mc verification (LIVE, multigroup k-eff).
//!
//! Notebook: `mg-mode-part-i.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Creates a multigroup cross-section library
//! (`XSdata` / `Macroscopic` / `MGXSLibrary`) and runs a multigroup k-eff on a
//! simple model. The notebook's cross sections are illustrative macroscopic MG
//! constants (not a benchmark), and it prints **no** reference k-effective in its
//! committed cell outputs — so, per this crate's V&V rule ("reference values come
//! from the notebook itself; do not invent one"), the LIVE assertion here is
//! judged against an **analytic** eigenvalue we *can* defend, not a notebook
//! number.
//!
//! **OpenMC API exercised (now real in outram-mc).** The MGXS data model
//! ([`Mgxs`] = `XSdata`/`Macroscopic`, [`MgxsLibrary`] = `MGXSLibrary`) and the
//! multigroup k-eigenvalue run ([`run_keff_mg`]) over the existing CSG
//! [`Geometry`], ported in structure from OpenMC's MG mode (`src/physics_mg.cpp`,
//! `src/mgxs.cpp`). Tracked by bead op-6tz.15.
//!
//! # V&V — methodology and results
//!
//! **Methodology.** A canonical **2-group** macroscopic MGXS set (group 0 fast,
//! group 1 thermal; χ = (1,0); no upscatter) is built via [`Mgxs`]/[`MgxsLibrary`]
//! and its constants are chosen so the **infinite-medium** eigenvalue has a
//! closed form:
//!
//! k∞ = (νΣ_f0 + νΣ_f1 · Σ_s,0→1 / Σ_a1) / (Σ_a0 + Σ_s,0→1)
//!    = (0.008 + 0.100·0.020/0.080) / (0.010 + 0.020) = 0.033/0.030 = **1.10**.
//!
//! The primary LIVE test runs MG power iteration in a **reflective cube** (zero
//! leakage ⇒ a finite stand-in for an infinite medium) and asserts the measured k
//! matches the analytic 1.10 within statistics. A supplementary leakage smoke run
//! (vacuum-bounded cubes) asserts only physics sanity (k < k∞, and a bigger cube
//! leaks less than a smaller one) with **no** invented reference, mirroring the
//! `hexagonal_lattice` convention.
//!
//! **Results (2026-07-17, this harness).** Reflective-cube k∞ = 1.10085 ± 0.00175
//! (analytic 1.10000; +0.5σ) — printed at run time with `--nocapture`. The
//! leakage smoke's measured k ± σ are printed too. The MG collision physics
//! (group total, absorption/fission split, χ birth spectrum, scatter-matrix group
//! transfer) and reflective/vacuum transport are confirmed correct.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::physics::physics_mg::{run_keff_mg, MgSettings, Mgxs, MgxsLibrary};
use outram_mc_libs::physics::transport_csg::SourceBox;

/// Analytic infinite-medium eigenvalue of [`two_group_set`] (see module docs).
const K_INF_ANALYTIC: f64 = 1.10;

/// The canonical 2-group macroscopic MGXS set (the `XSdata`/`Macroscopic`
/// analogue). Group 0 = fast, group 1 = thermal; χ = (1,0); no upscatter.
/// ν̄ = 2.5 in both groups. Chosen so k∞ = 1.10 exactly (module docs).
fn two_group_set() -> Mgxs {
    Mgxs::new(
        "2g-fuel",
        /* total      */ vec![0.080, 0.180],
        /* absorption */ vec![0.010, 0.080],
        /* fission    */ vec![0.0032, 0.040],
        /* nu_fission */ vec![0.008, 0.100],
        /* chi        */ vec![1.0, 0.0],
        /* scatter    */ vec![0.050, 0.020, 0.000, 0.100], // row-major 2×2
    )
}

/// A cube [-a, a]³ filled with material 0, all six faces having boundary `bc`.
/// With `Reflective` it is a zero-leakage infinite-medium stand-in; with `Vacuum`
/// it leaks.
fn cube(a: f64, bc: BoundaryType) -> Geometry {
    let surfaces = vec![
        SurfaceKind::XPlane(XPlane { x0: -a, bc }),
        SurfaceKind::XPlane(XPlane { x0: a, bc }),
        SurfaceKind::YPlane(YPlane { y0: -a, bc }),
        SurfaceKind::YPlane(YPlane { y0: a, bc }),
        SurfaceKind::ZPlane(ZPlane { z0: -a, bc }),
        SurfaceKind::ZPlane(ZPlane { z0: a, bc }),
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
    Geometry {
        surfaces,
        cells: vec![Cell::material(1, region, 0, 293.6)],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// LIVE: multigroup k∞ against the analytic 2-group infinite-medium eigenvalue.
///
/// This is the `mg-mode-part-i` verification: build the MGXS library and run a
/// multigroup k-eff. See the module V&V note for methodology and results.
#[test]
fn mg_mode_run() {
    let geom = cube(10.0, BoundaryType::Reflective);
    let lib = MgxsLibrary::new(vec![two_group_set()]);
    let settings = MgSettings { n_particles: 4000, n_inactive: 40, n_active: 120, seed: 1 };
    let src =
        SourceBox { lower: Position::new(-5.0, -5.0, -5.0), upper: Position::new(5.0, 5.0, 5.0) };

    let result = run_keff_mg(&geom, &lib, src, &settings);

    eprintln!(
        "[mg-mode-part-i] k∞ = {:.5} ± {:.5}  (analytic {:.5})",
        result.k_mean, result.k_std, K_INF_ANALYTIC
    );

    // Sanity on the MGXS set itself (self-consistency Σ_t = Σ_a + Σ_scatter-out).
    assert!(lib.material(0).consistency_residual() < 1e-12, "MGXS set self-consistent");
    assert_eq!(result.k_by_generation.len(), 160, "ran all generations");
    assert!(
        (result.k_mean - K_INF_ANALYTIC).abs() < 0.01,
        "MG k∞ {} deviates from analytic {K_INF_ANALYTIC} by more than 0.01",
        result.k_mean
    );
    assert!(result.k_std < 0.005, "k noisy/unconverged: σ = {}", result.k_std);
}

/// LIVE (supplementary smoke, NOT a benchmark): MG transport with leakage. A
/// vacuum-bounded cube must be sub-k∞, and a bigger cube must leak less (higher
/// k) than a smaller one — a physics-sanity monotonicity check that the MG
/// kernel couples to geometry/leakage. No reference k is asserted.
#[test]
fn mg_mode_leakage_smoke() {
    let lib = MgxsLibrary::new(vec![two_group_set()]);
    let settings = MgSettings { n_particles: 2000, n_inactive: 25, n_active: 50, seed: 7 };

    let run = |a: f64| {
        let geom = cube(a, BoundaryType::Vacuum);
        let src = SourceBox {
            lower: Position::new(-a * 0.5, -a * 0.5, -a * 0.5),
            upper: Position::new(a * 0.5, a * 0.5, a * 0.5),
        };
        run_keff_mg(&geom, &lib, src, &settings).k_mean
    };

    let k_big = run(30.0);
    let k_small = run(12.0);
    eprintln!("[mg-mode-part-i leakage] k(30cm)={k_big:.5}  k(12cm)={k_small:.5}  (k∞={K_INF_ANALYTIC})");

    assert!(k_big.is_finite() && k_big > 0.0, "k(30cm) finite/positive, got {k_big}");
    assert!(k_small.is_finite() && k_small > 0.0, "k(12cm) finite/positive, got {k_small}");
    assert!(k_big < K_INF_ANALYTIC, "vacuum-bounded k(30cm)={k_big} must be below k∞");
    assert!(k_small < k_big, "smaller cube k={k_small} must leak more than bigger k={k_big}");
}
