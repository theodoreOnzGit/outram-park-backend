//! probe scaffold — iterated in place, replaced by the real V&V file
use outram_foam_appbuilder_lib::io::control_dict::{ControlDict, StartControl, StopControl};
use outram_foam_appbuilder_lib::io::fv_schemes::FvSchemes;
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::solvers::melt_foam::MeltFoam;
use outram_foam_basic_lib::prelude::*;
use std::sync::Arc;

fn line_mesh(n: usize, length: f64, area: f64) -> Arc<FvMesh> {
    let h = length / n as f64;
    let ax = |x: f64| Vector3::new(x, 0.0, 0.0);
    let mut owner: Vec<usize> = (0..n - 1).collect();
    owner.push(0);
    owner.push(n - 1);
    let neighbour: Vec<usize> = (1..n).collect();
    let mut fav: Vec<Vector3> = (0..n - 1).map(|_| ax(area)).collect();
    fav.push(ax(-area));
    fav.push(ax(area));
    let mut fc: Vec<Vector3> = (0..n - 1).map(|i| ax((i + 1) as f64 * h)).collect();
    fc.push(ax(0.0));
    fc.push(ax(length));
    Arc::new(
        FvMeshBuilder::new()
            .n_cells(n)
            .n_internal_faces(n - 1)
            .owner(owner)
            .neighbour(neighbour)
            .patches(vec![
                BoundaryPatch::new("hot", n - 1, 1, PatchKind::Wall),
                BoundaryPatch::new("far", n, 1, PatchKind::Wall),
            ])
            .cell_volumes(vec![area * h; n])
            .cell_centres((0..n).map(|i| ax((i as f64 + 0.5) * h)).collect())
            .face_area_vectors(fav)
            .face_centres(fc)
            .build()
            .expect("mesh"),
    )
}

/// Solve `lambda * exp(lambda^2) * erf(lambda) = St / sqrt(pi)` by bisection.
fn stefan_lambda(stefan: f64) -> f64 {
    let target = stefan / std::f64::consts::PI.sqrt();
    let f = |l: f64| l * (l * l).exp() * erf(l) - target;
    let (mut lo, mut hi) = (1e-12, 10.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Abramowitz & Stegun 7.1.26 rational approximation, |eps| < 1.5e-7.
fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let y = 1.0
        - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t
            + 0.254829592)
            * t
            * (-x * x).exp();
    sign * y
}

/// Run the 1-D Stefan problem. Returns (alpha=0.5 front, integral melt, exact).
/// Run the 1-D Stefan problem on `n` cells with timestep `dt` and mushy
/// interval `mushy`, to `t_end`. Returns (front position [m], exact [m]).
fn run_stefan(n: usize, dt: f64, mushy: f64, t_end: f64) -> (f64, f64, f64) {
    let cp = 1000.0;
    let latent = 100_000.0;
    let alpha_th = 1e-5;
    let t_melt = 300.0;
    let t_wall = 320.0;
    let stefan = cp * (t_wall - t_melt) / latent;
    let lambda = stefan_lambda(stefan);
    let length = 0.1;
    let mesh = line_mesh(n, length, 1.0);
    let mut control = ControlDict::default();
    control.delta_t = dt;
    let mut s = MeltFoam::new(
        mesh.clone(),
        control,
        FvSchemes::default(),
        FvSolution::default(),
    );
    s.t = VolScalarField::uniform("T", mesh.clone(), t_melt);
    s.alpha_thermal = VolScalarField::uniform("alphat", mesh.clone(), alpha_th);
    s.nu = VolScalarField::uniform("nu", mesh.clone(), 1e-6);
    s.t.boundary[0].bc = BoundaryCondition::FixedValue(t_wall);
    for v in s.t.boundary[0].values.iter_mut() {
        *v = t_wall;
    }
    s.t.boundary[1].bc = BoundaryCondition::ZeroGradient;
    let coeffs = MeltFoam::boussinesq_coefficients(
        t_melt,
        t_melt + mushy,
        latent,
        cp,
        1.0,
        0.0,
        1.0e8,
    );
    s.fv_models
        .push(FvModel::SolidificationMelting(SolidificationMelting::new(
            "melt",
            "U",
            "T",
            true,
            CellSelection::All,
            coeffs,
            Vector3::new(0.0, 0.0, 0.0),
            n,
        )));
    let steps = (t_end / dt).round() as usize;
    for _ in 0..steps {
        s.step().expect("step");
    }
    let h = length / n as f64;
    let a = s.liquid_fraction().unwrap();
    let mut front = 0.0;
    for i in 0..n - 1 {
        if a[i] >= 0.5 && a[i + 1] < 0.5 {
            let f = (a[i] - 0.5) / (a[i] - a[i + 1]);
            front = (i as f64 + 0.5 + f) * h;
            break;
        }
    }
    let integral: f64 = a.iter().sum::<f64>() * h;
    (front, integral, 2.0 * lambda * (alpha_th * t_end).sqrt())
}

#[test]
fn probe_stefan_refinement() {
    let show = |tag: String, num: f64, integ: f64, exact: f64| {
        println!(
            "{tag}  front50={num:.6} ({:+.3}%)   integral={integ:.6} ({:+.3}%)   exact={exact:.6}",
            100.0 * (num - exact) / exact,
            100.0 * (integ - exact) / exact
        );
    };
    println!("--- mesh refinement (dt=0.01, mushy=0.2 K, t=100 s) ---");
    for n in [100, 200, 400, 800] {
        let (f, i, e) = run_stefan(n, 0.01, 0.2, 100.0);
        show(format!("n={n:5}"), f, i, e);
    }
    println!("--- timestep refinement (n=400, mushy=0.2 K, t=100 s) ---");
    for dt in [0.08, 0.04, 0.02, 0.01, 0.005] {
        let (f, i, e) = run_stefan(400, dt, 0.2, 100.0);
        show(format!("dt={dt:6.3}"), f, i, e);
    }
    println!("--- mushy interval (n=400, dt=0.01, t=100 s) ---");
    for m in [1.0, 0.5, 0.2, 0.1, 0.05] {
        let (f, i, e) = run_stefan(400, 0.01, m, 100.0);
        show(format!("mushy={m:5.2}K"), f, i, e);
    }
}

/// Energy conservation: does `d/dt integral(Cp*T + L*alpha) dV` equal the heat
/// flux in through the hot wall? Independent of the analytical solution.
#[test]
fn probe_energy_conservation() {
    let cp = 1000.0;
    let latent = 100_000.0;
    let alpha_th = 1e-5;
    let t_melt = 300.0;
    let t_wall = 320.0;
    let n = 400;
    let length = 0.1;
    let h = length / n as f64;
    let dt = 0.01;
    let mesh = line_mesh(n, length, 1.0);
    let mut control = ControlDict::default();
    control.delta_t = dt;
    let mut s = MeltFoam::new(
        mesh.clone(),
        control,
        FvSchemes::default(),
        FvSolution::default(),
    );
    s.t = VolScalarField::uniform("T", mesh.clone(), t_melt);
    s.alpha_thermal = VolScalarField::uniform("alphat", mesh.clone(), alpha_th);
    s.nu = VolScalarField::uniform("nu", mesh.clone(), 1e-6);
    s.t.boundary[0].bc = BoundaryCondition::FixedValue(t_wall);
    for v in s.t.boundary[0].values.iter_mut() {
        *v = t_wall;
    }
    s.t.boundary[1].bc = BoundaryCondition::ZeroGradient;
    let coeffs =
        MeltFoam::boussinesq_coefficients(t_melt, t_melt + 0.2, latent, cp, 1.0, 0.0, 1.0e8);
    s.fv_models
        .push(FvModel::SolidificationMelting(SolidificationMelting::new(
            "melt",
            "U",
            "T",
            true,
            CellSelection::All,
            coeffs,
            Vector3::new(0.0, 0.0, 0.0),
            n,
        )));

    let enthalpy = |s: &MeltFoam| -> f64 {
        let a = s.liquid_fraction().unwrap();
        (0..n).map(|i| (cp * s.t.internal[i] + latent * a[i]) * h).sum()
    };
    // What diffusivity does the solver actually see at the hot wall?
    let af = fvc::interpolate(&s.alpha_thermal);
    println!(
        "alpha_thermal internal={:.6e}  boundary[0][0]={:.6e}  (assumed {:.6e})",
        s.alpha_thermal.internal[0], af.boundary[0].values[0], alpha_th
    );
    let d_wall = (mesh.face_centres[mesh.patches[0].start] - mesh.cell_centres[0]).mag();
    println!("wall delta = {d_wall:.6e}  (h/2 = {:.6e})", 0.5 * h);

    let h0 = enthalpy(&s);
    let mut q_in = 0.0;
    let mut q_in_solver = 0.0;
    for _ in 0..10_000 {
        // Implicit Euler: the flux the step transports uses the NEW-time T[0].
        s.step().expect("step");
        q_in += cp * alpha_th * (t_wall - s.t.internal[0]) / (0.5 * h) * dt;
        q_in_solver += cp * af.boundary[0].values[0] * (t_wall - s.t.internal[0]) / d_wall * dt;
    }
    let h1 = enthalpy(&s);
    let d_h = h1 - h0;
    println!("integral d(enthalpy)     = {d_h:.6} J/m^2");
    println!("integral wall flux       = {q_in:.6} J/m^2");
    println!("integral wall flux (slv) = {q_in_solver:.6} J/m^2");
    println!(
        "imbalance vs assumed = {:+.4e}  ({:+.4}%)",
        d_h - q_in,
        100.0 * (d_h - q_in) / q_in
    );
    println!(
        "imbalance vs solver  = {:+.4e}  ({:+.4}%)",
        d_h - q_in_solver,
        100.0 * (d_h - q_in_solver) / q_in_solver
    );
}

#[test]
fn probe_stefan() {
    // Material — chosen for clean numbers, not a real substance.
    let cp = 1000.0; // J/(kg K)
    let latent = 100_000.0; // J/kg
    let alpha_th = 1e-5; // m^2/s
    let t_melt = 300.0;
    let t_wall = 320.0;
    let stefan = cp * (t_wall - t_melt) / latent;
    let lambda = stefan_lambda(stefan);
    println!("Stefan number St = {stefan:.6}");
    println!("lambda           = {lambda:.6}");

    let n = 400;
    let length = 0.1;
    let mesh = line_mesh(n, length, 1.0);

    let mut control = ControlDict::default();
    control.delta_t = 0.02;
    control.start = StartControl::StartTime(0.0);
    control.stop = StopControl::EndTime(100.0);

    let mut s = MeltFoam::new(
        mesh.clone(),
        control,
        FvSchemes::default(),
        FvSolution::default(),
    );
    s.t = VolScalarField::uniform("T", mesh.clone(), t_melt);
    s.alpha_thermal = VolScalarField::uniform("alphat", mesh.clone(), alpha_th);
    s.nu = VolScalarField::uniform("nu", mesh.clone(), 1e-6);

    // hot wall at x=0 (patch 0), zero-gradient far wall (patch 1)
    s.t.boundary[0].bc = BoundaryCondition::FixedValue(t_wall);
    for v in s.t.boundary[0].values.iter_mut() {
        *v = t_wall;
    }
    s.t.boundary[1].bc = BoundaryCondition::ZeroGradient;

    // no buoyancy, no gravity: pure conduction Stefan problem
    let coeffs = MeltFoam::boussinesq_coefficients(
        t_melt,       // solidus
        t_melt + 0.2, // liquidus (small mushy interval)
        latent,
        cp,
        1.0,   // density (only scales the Darcy coefficient)
        0.0,   // beta = 0 -> no buoyancy
        1.0e8, // Cu
    );
    s.fv_models
        .push(FvModel::SolidificationMelting(SolidificationMelting::new(
            "melt",
            "U",
            "T",
            true, // energy equation is in temperature
            CellSelection::All,
            coeffs,
            Vector3::new(0.0, 0.0, 0.0),
            n,
        )));

    let dt = s.control.delta_t;
    let mut time = 0.0;
    let h = length / n as f64;
    for step in 1..=5000 {
        s.step().expect("step");
        time += dt;
        if step % 1000 == 0 {
            let a = s.liquid_fraction().unwrap();
            // front = last x where alpha >= 0.5, linearly interpolated
            let mut front = 0.0;
            for i in 0..n - 1 {
                if a[i] >= 0.5 && a[i + 1] < 0.5 {
                    let f = (a[i] - 0.5) / (a[i] - a[i + 1]);
                    front = (i as f64 + 0.5 + f) * h;
                    break;
                }
            }
            let exact = 2.0 * lambda * (alpha_th * time).sqrt();
            let melted: f64 = a.iter().sum::<f64>() * h;
            println!(
                "t={time:7.2}s  front_num={front:.6} m  front_exact={exact:.6} m  \
                 err={:+.2}%  melted_thickness={melted:.6} m  T[0]={:.3}",
                100.0 * (front - exact) / exact,
                s.t.internal[0]
            );
        }
    }
}
