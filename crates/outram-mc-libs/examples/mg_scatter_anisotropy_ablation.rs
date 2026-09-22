// SPDX-License-Identifier: GPL-3.0

//! **Paired P0-vs-P3 ablation on the multigroup scattering kernel** — what
//! anisotropic MG scattering is worth on a leakage-dominated case. GitHub #265.
//!
//! # Methodology
//!
//! Two arms over the same seeds, the same geometry and the same 2-group
//! macroscopic set (`mg-mode-part-i`'s illustrative constants, also used by
//! `tests/openmc_notebooks/mg_mode_part_i.rs`):
//!
//! - **P3** — every transfer carries the Legendre kernel `[1, 0.3, 0.1, 0.03]`
//!   (declared and sampled `⟨μ⟩ = 0.3`; the series is positive on all of
//!   `[-1, 1]`, so there is no negativity truncation).
//! - **P0** — the same set through [`Mgxs::without_scatter_anisotropy`],
//!   i.e. the isotropic-in-lab treatment that was the *only* treatment before
//!   #265.
//!
//! Two geometries:
//!
//! - **Bare cube, vacuum, half-width 35 cm** — leakage-dominated, where the
//!   transport correction is supposed to bite.
//! - **Reflective cube, half-width 10 cm** — the **control**. Zero leakage, so
//!   a leakage-only effect must leave it alone. If this moves, the two arms
//!   differ for a reason other than transport correction and the bare-cube
//!   number is not what it appears to be.
//!
//! Each arm is an ensemble over seeds; the arms are **paired** (arm A and arm B
//! share seed `s`), and the reported worth is the mean of the per-seed
//! differences with the standard error of that mean. Pairing is what makes a
//! few tens of seeds enough: the seed-to-seed scatter largely cancels.
//!
//! # The prediction, made before this was ever run
//!
//! `verification_and_validation/mg_anisotropic_scattering/prediction_before_measuring.md`
//! — sign **negative**, magnitude **≈ −4200 pcm** (band −3000 to −5500), control
//! consistent with zero, `group_mean_cosine` exactly 0.3 against 0.
//!
//! # Results
//!
//! `verification_and_validation/mg_anisotropic_scattering/ablation_2026_09_22.md`.
//!
//! # Running it
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example mg_scatter_anisotropy_ablation
//! cargo run --release -p outram-mc-libs --example mg_scatter_anisotropy_ablation -- 48
//! ```

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::physics::physics_mg::{run_keff_mg, MgSettings, Mgxs, MgxsLibrary};
use outram_mc_libs::physics::transport_csg::SourceBox;

/// The P3 kernel attached to every transfer in the ANISO arm.
const P3: [f64; 4] = [1.0, 0.3, 0.1, 0.03];

/// The 2-group set, with or without the angular moments.
fn two_group(aniso: bool) -> Mgxs {
    let base = Mgxs::new(
        "2g-fuel",
        /* total      */ vec![0.080, 0.180],
        /* absorption */ vec![0.010, 0.080],
        /* fission    */ vec![0.0032, 0.040],
        /* nu_fission */ vec![0.008, 0.100],
        /* chi        */ vec![1.0, 0.0],
        /* scatter    */ vec![0.050, 0.020, 0.000, 0.100],
    );
    if !aniso {
        // Explicit, named ablation — never the default state.
        return base.without_scatter_anisotropy();
    }
    base.with_legendre_scattering(vec![P3.to_vec(); 4])
        .expect("the P3 kernel is positive on [-1, 1] and samplable")
}

/// A cube `[-a, a]³` of material 0 with boundary `bc` on all six faces.
fn cube(a: f64, bc: BoundaryType) -> Geometry {
    let surfaces = vec![
        SurfaceKind::XPlane(XPlane { x0: -a, bc }),
        SurfaceKind::XPlane(XPlane { x0: a, bc }),
        SurfaceKind::YPlane(YPlane { y0: -a, bc }),
        SurfaceKind::YPlane(YPlane { y0: a, bc }),
        SurfaceKind::ZPlane(ZPlane { z0: -a, bc }),
        SurfaceKind::ZPlane(ZPlane { z0: a, bc }),
    ];
    let mut region = vec![
        RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Outside },
        RegionToken::HalfSpace { surface_idx: 1, sense: HalfSpaceSense::Inside },
        RegionToken::Intersection,
    ];
    for s in 2..6 {
        region.push(RegionToken::HalfSpace {
            surface_idx: s,
            sense: if s % 2 == 0 { HalfSpaceSense::Outside } else { HalfSpaceSense::Inside },
        });
        region.push(RegionToken::Intersection);
    }
    Geometry {
        surfaces,
        cells: vec![Cell::material(1, region, 0, 293.6)],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn source_box(a: f64) -> SourceBox {
    SourceBox {
        lower: Position::new(-a, -a, -a),
        upper: Position::new(a, a, a),
    }
}

/// Mean and standard error of the mean, in pcm-friendly raw units.
fn mean_sem(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    if v.len() < 2 {
        return (mean, f64::NAN);
    }
    let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    (mean, (var / n).sqrt())
}

/// Run one paired ablation on one geometry and report it.
fn ablate(label: &str, geom: &Geometry, a: f64, n_seeds: u64, settings: &MgSettings) {
    let lib_aniso = MgxsLibrary::new(vec![two_group(true)]);
    let lib_iso = MgxsLibrary::new(vec![two_group(false)]);
    let src = source_box(a);

    let mut k_aniso = Vec::with_capacity(n_seeds as usize);
    let mut k_iso = Vec::with_capacity(n_seeds as usize);
    let mut diff = Vec::with_capacity(n_seeds as usize);

    for s in 0..n_seeds {
        let mut st = settings.clone();
        st.seed = 1 + s * 7919; // distinct, shared by both arms
        let a_k = run_keff_mg(geom, &lib_aniso, src, &st).k_mean;
        let i_k = run_keff_mg(geom, &lib_iso, src, &st).k_mean;
        k_aniso.push(a_k);
        k_iso.push(i_k);
        diff.push(a_k - i_k);
    }

    let (ma, sa) = mean_sem(&k_aniso);
    let (mi, si) = mean_sem(&k_iso);
    let (md, sd) = mean_sem(&diff);

    println!("\n=== {label} ===");
    println!("  P3 (anisotropic) k = {ma:.5} +/- {sa:.5}");
    println!("  P0 (isotropic)   k = {mi:.5} +/- {si:.5}");
    println!(
        "  paired worth       = {:+.0} +/- {:.0} pcm   ({:.1} sigma)",
        1.0e5 * md,
        1.0e5 * sd,
        (md / sd).abs()
    );
}

fn main() {
    let n_seeds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(32);

    let aniso = two_group(true);
    let iso = two_group(false);
    println!("GitHub #265 — MG scattering anisotropy, paired P0/P3 ablation");
    println!("  P3 kernel {P3:?}");
    println!(
        "  group mean cosine, P3 arm: g0 = {:.6}, g1 = {:.6}",
        aniso.group_mean_cosine(0),
        aniso.group_mean_cosine(1)
    );
    println!(
        "  group mean cosine, P0 arm: g0 = {:.6}, g1 = {:.6}",
        iso.group_mean_cosine(0),
        iso.group_mean_cosine(1)
    );
    println!("  seeds per arm: {n_seeds}");

    let settings = MgSettings {
        n_particles: 2000,
        n_inactive: 30,
        n_active: 70,
        seed: 1,
    };

    ablate(
        "BARE CUBE, vacuum, a = 35 cm (leakage-dominated; the case under test)",
        &cube(35.0, BoundaryType::Vacuum),
        35.0,
        n_seeds,
        &settings,
    );
    ablate(
        "CONTROL: reflective cube, a = 10 cm (zero leakage; must NOT move)",
        &cube(10.0, BoundaryType::Reflective),
        10.0,
        n_seeds,
        &settings,
    );
}
