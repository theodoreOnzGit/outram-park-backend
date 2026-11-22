//! Laminar pipe flow on the O-grid: Hagen–Poiseuille, f = 64/Re, the
//! development length, and the non-orthogonal correction (gh:#1161,
//! gh:#1155).
//!
//! Methodology, predictions (written before the runs) and results:
//! `verification_and_validation/laminar_pipe_gh1155.md` and
//! `verification_and_validation/non_orthogonal_correction_gh1161.md`.
//!
//! The one test that runs by default (`pipe_5k_non_orthogonal_correction`)
//! takes ~2 s and pins the 5k-cell numbers. The `#[ignore]`d `measure_*`
//! tests print the full tables (minutes each):
//!
//! ```text
//! cargo test --release -p outram-foam-appbuilder-lib --test laminar_pipe_hagen_poiseuille \
//!     -- --ignored --nocapture measure_
//! ```
use std::sync::Arc;
use std::time::Instant;

use outram_foam_appbuilder_lib::cases::laminar_pipe::{
    centreline_profile, centreline_velocity, development_length, max_continuity_error, patch_flux,
    pressure_gradient, PipeFlow,
};
use outram_foam_appbuilder_lib::solvers::pimple_foam::PimpleFoam;
use outram_foam_mesh::block_primitives::PipeOGrid;
use uom::si::acceleration::meter_per_second_squared;
use uom::si::f64::{KinematicViscosity, Length};
use uom::si::kinematic_viscosity::square_meter_per_second;
use uom::si::length::meter;
use uom::si::velocity::meter_per_second;

/// What one run produced.
#[derive(Debug, Clone, Copy)]
struct Outcome {
    cells: usize,
    steps: usize,
    /// u_c / 2U at z = 0.15 m and 0.17 m (stage 0 stations).
    uc15: f64,
    uc17: f64,
    /// (−dp/dz) / HP in the check slab.
    dpdz: f64,
    /// Same, against the inscribed polygon's own bulk velocity (U·πR²/A).
    dpdz_poly: f64,
    /// outlet / inlet flux.
    flux: f64,
    cont: f64,
    /// u_c,dev / 2U (mean over the slab layers) and L_99 / D.
    uc_dev: f64,
    l99_d: Option<f64>,
    /// RMS of (u_z − u_HP(r)) / 2U over the slab cells.
    profile_rms: f64,
    wall_s: f64,
}

fn mesh(g: &PipeOGrid) -> Arc<outram_foam_basic_lib::mesh::FvMesh> {
    Arc::new(g.mesh().expect("O-grid").to_fv_mesh().expect("FvMesh"))
}

/// Run `flow` on `g` at axial Courant number `co` to the stage 0 steady
/// criterion (metric < 1e-3) or `cap` steps; `ablate` selects the pre-gh:#1161
/// orthogonal Laplacian. The check slab is `slab` (m).
fn run(flow: &PipeFlow, g: &PipeOGrid, co: f64, cap: usize, ablate: bool, n_non_orth: usize, slab: (f64, f64)) -> Outcome {
    let m = mesh(g);
    let dz = Length::new::<meter>(flow.length.get::<meter>() / g.nz as f64);
    let mut s: PimpleFoam = flow.build_solver(m.clone(), flow.courant_time_step(dz, co));
    if ablate {
        s = s.without_non_orthogonal_correction();
    }
    s.solution.pimple.n_non_orthogonal_correctors = n_non_orth;
    let t0 = Instant::now();
    let mut steps = 0;
    for _ in 0..cap {
        steps += 1;
        if flow.step(&mut s).expect("step") < 1e-3 {
            break;
        }
    }
    let wall_s = t0.elapsed().as_secs_f64();
    let u2 = 2.0 * flow.bulk_velocity.get::<meter_per_second>();
    let r = flow.radius.get::<meter>();
    let uc = |z: f64| centreline_velocity(&s, flow, (0.0, 0.0), z, 0.01).get::<meter_per_second>() / u2;
    let hp = flow.hagen_poiseuille_gradient().get::<meter_per_second_squared>();
    let dpdz = pressure_gradient(&s, slab.0, slab.1).get::<meter_per_second_squared>() / hp;
    let area_ratio = std::f64::consts::PI * r * r / g.polygon_area();
    let prof = centreline_profile(&s, flow, (0.0, 0.0));
    let dev: Vec<f64> = prof.iter().filter(|(z, _)| *z >= slab.0 && *z <= slab.1).map(|x| x.1).collect();
    let uc_dev = dev.iter().sum::<f64>() / dev.len().max(1) as f64;
    let l99_d = development_length(&prof, uc_dev, 0.99, 0.0).map(|l| l.get::<meter>() / (2.0 * r));
    let (mut se, mut sw) = (0.0, 0.0);
    for c in 0..m.n_cells {
        let x = m.cell_centres[c];
        if x.z >= slab.0 && x.z <= slab.1 {
            let rr = (x.x * x.x + x.y * x.y) / (r * r);
            let e = (s.u.internal[c].z - u2 * (1.0 - rr)) / u2;
            se += e * e * m.cell_volumes[c];
            sw += m.cell_volumes[c];
        }
    }
    Outcome {
        cells: m.n_cells,
        steps,
        uc15: uc(0.15),
        uc17: uc(0.17),
        dpdz,
        dpdz_poly: dpdz / area_ratio,
        flux: patch_flux(&s, "outlet") / -patch_flux(&s, "inlet"),
        cont: max_continuity_error(&s),
        uc_dev: uc_dev / u2,
        l99_d,
        profile_rms: (se / sw).sqrt(),
        wall_s,
    }
}

fn print(label: &str, o: &Outcome) {
    println!(
        "{label:<34} cells {:>6} steps {:>4} uc15 {:.4} uc17 {:.4} dpdz/HP {:.5} (poly {:.5}) flux {:.10} cont {:.1e} uc_dev {:.4} L99/D {} rms {:.4} wall {:.1}s",
        o.cells,
        o.steps,
        o.uc15,
        o.uc17,
        o.dpdz,
        o.dpdz_poly,
        o.flux,
        o.cont,
        o.uc_dev,
        o.l99_d.map_or("-".into(), |v| format!("{v:.3}")),
        o.profile_rms,
        o.wall_s
    );
}

fn stage0_grids() -> [PipeOGrid; 3] {
    let f = PipeFlow::stage0();
    [
        PipeOGrid::new(f.radius, f.length, 6, 4, 40),
        PipeOGrid::new(f.radius, f.length, 8, 6, 80),
        PipeOGrid::new(f.radius, f.length, 10, 8, 120),
    ]
}

/// The default-run gate (gh:#1161): on the 5k stage 0 O-grid, Re = 100,
/// Co ≈ 4, the corrected Laplacian lowers dp/dz toward Hagen–Poiseuille and
/// conserves mass; the ablation reproduces the stage 0 number.
///
/// **gh:#1488 (2026-11-11), boundary `deltaCoeffs` made patch-normal as in
/// upstream `fvPatch::delta()`.** Printed values, release, this 5280-cell
/// O-grid: ablation (orthogonal) ~~`dpdz/HP` 1.03160, poly 1.01986, `uc15`
/// 0.9902, `uc17` 0.9923, `uc_dev` 0.9900~~ **1.03221, 1.02046, 0.9904,
/// 0.9926, 0.9902** (+6.1e-4 in `dpdz/HP`: the wall conductance `1/|n . d|`
/// is larger than `1/|Cf - C|` on the skewed wall cells, so more wall
/// shear for the same flow). That is 6e-4 FURTHER from the polygon's
/// Hagen-Poiseuille value, a finding recorded rather than tuned; the
/// ablation is a deliberately uncorrected solver. The default (corrected)
/// row is unchanged to the printed digits (1.01768, poly 1.00610, `uc15`
/// 0.9902, `cont` 5.7e-12): the corrected path already used the
/// patch-normal distance. The assertions below did not move.
#[test]
fn pipe_5k_non_orthogonal_correction() {
    let f = PipeFlow::stage0();
    let g = &stage0_grids()[0];
    let slab = (0.13, 0.17);
    let ab = run(&f, g, 4.0, 400, true, 0, slab);
    let co = run(&f, g, 4.0, 400, false, 0, slab);
    print("5k ablation (orthogonal)", &ab);
    print("5k corrected (default)", &co);
    // Mass: both to 1e-9.
    for o in [&ab, &co] {
        assert!((o.flux - 1.0).abs() < 1e-9, "flux ratio {}", o.flux);
        assert!(o.cont < 1e-8, "continuity {}", o.cont);
    }
    // The ablation is the pre-gh:#1161 solver: stage 0 measured 1.0316 at
    // Co ≈ 1 on the same mesh (blender route, same geometry).
    assert!((ab.dpdz - 1.0316).abs() < 1.5e-3, "ablation dp/dz {}", ab.dpdz);
    // The correction must lower dp/dz (predicted sign) ...
    assert!(co.dpdz < ab.dpdz - 5e-3, "corrected {} vs ablated {}", co.dpdz, ab.dpdz);
    // ... to within 1 % of HP against the polygon's own bulk velocity.
    assert!((co.dpdz_poly - 1.0).abs() < 0.01, "corrected/poly {}", co.dpdz_poly);
}

/// gh:#1161 measurement: the three stage 0 O-grids, Re = 100, Co ≈ 4,
/// ablation against corrected, plus one nNonOrthogonalCorrectors = 1 run.
#[test]
#[ignore = "minutes; prints the gh:#1161 table"]
fn measure_gh1161_stage0_grids() {
    let f = PipeFlow::stage0();
    let slab = (0.13, 0.17);
    for g in stage0_grids() {
        let label = format!("{}/{}/{}", g.n, g.nr, g.nz);
        print(&format!("{label} ablation"), &run(&f, &g, 4.0, 1000, true, 0, slab));
        print(&format!("{label} corrected"), &run(&f, &g, 4.0, 1000, false, 0, slab));
    }
}

/// gh:#1161 measurement, continued: corrector count, Courant number, the
/// 50k grid with one corrector, and Re = 25.
#[test]
#[ignore = "minutes; prints the rest of the gh:#1161 table"]
fn measure_gh1161_extras() {
    let f = PipeFlow::stage0();
    let slab = (0.13, 0.17);
    let g = &stage0_grids()[1];
    print("8/6/80 corrected nNonOrth=1", &run(&f, g, 4.0, 1000, false, 1, slab));
    print("8/6/80 corrected Co=1", &run(&f, g, 1.0, 3000, false, 0, slab));
    print("10/8/120 corrected nNonOrth=1", &run(&f, &stage0_grids()[2], 4.0, 1000, false, 1, slab));
    // Re = 25 (entry length ~ 0.03 m): stage 0 measured 1.0239 at 20k, at
    // Co ≈ 1. At Co ≈ 4 the viscous number ν Δt / Δr² is ~7 and the run does
    // not settle (measured 2026-11-03: 1000 steps, dp/dz / HP 0.797), the
    // large-viscous-number limit stage 0 recorded; so Co ≈ 1 here.
    let mut f25 = f;
    f25.nu = KinematicViscosity::new::<square_meter_per_second>(4.0e-6);
    print("8/6/80 Re=25 ablation Co=1", &run(&f25, g, 1.0, 3000, true, 0, slab));
    print("8/6/80 Re=25 corrected Co=1", &run(&f25, g, 1.0, 3000, false, 0, slab));
}

/// gh:#1155 V&V: L/D = 20 at Re = 100, the similar family m = 3, 4, 5
/// (`PipeOGrid::level`), developed slab z ∈ [0.25, 0.35] m.
#[test]
#[ignore = "minutes; prints the gh:#1155 grid study"]
fn measure_gh1155_grid_study() {
    let mut f = PipeFlow::stage0();
    f.length = Length::new::<meter>(0.4);
    let slab = (0.25, 0.35);
    for m in [3, 4, 5] {
        let g = PipeOGrid::level(f.radius, f.length, m);
        // Pseudo-transient at Co ≈ 4, but with the viscous number ν Δt / Δr²
        // held at 2 or below (the stage 0 stable range; ~7 diverges), and one
        // non-orthogonal corrector (the 50k Co ≈ 4 run diverged without).
        let r = f.radius.get::<meter>();
        let dr = (1.0 - g.square_frac) * r / g.nr as f64;
        let dz = f.length.get::<meter>() / g.nz as f64;
        let u = f.bulk_velocity.get::<meter_per_second>();
        let nu = f.nu.get::<square_meter_per_second>();
        let co = 4.0_f64.min(2.0 * dr * dr / nu * 2.0 * u / dz);
        println!("m={m}: Co {co:.2}, viscous number {:.2}", nu * co * dz / (2.0 * u) / (dr * dr));
        print(&format!("L/D=20 m={m} corrected"), &run(&f, &g, co, 3000, false, 1, slab));
    }
}
