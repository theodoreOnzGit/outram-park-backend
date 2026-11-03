//! V&V V2 of gh:#1156: the energy equation attached to `PimpleFoam`
//! (`solvers::energy`) against the developed laminar Nusselt numbers of a
//! circular pipe.
//!
//! **Methodology** (fixed before the runs; full text and predictions in
//! `verification_and_validation/cht_double_pipe_gh1156.md`, V2):
//! `PipeOGrid::level(R = 10 mm, L = 200 mm, m)`; prescribed developed
//! Hagen–Poiseuille flux on the axial faces (exactly divergence-free on the
//! extruded mesh), `frozenFlow`, steady energy equation; Re = 50, Pr = 1
//! (Pe = 50); inlet 300 K, outlet zeroGradient; wall at constant flux
//! (q = 100 W/m², k = 0.6) or constant temperature (350 K). Per axial layer:
//! wall heat flux from the matrix's wall conduction, wall temperature,
//! upwind mixing-cup bulk temperature at the layer's downstream face,
//! `Nu = q_w D / (k (T_w − T_b))`; the developed value is the mean over
//! z/D ∈ [6, 9]. References: 48/11 (constant flux, closed form) and the
//! Graetz eigenvalue problem solved here by RK4 shooting (constant T), with
//! and without axial diffusion (physical α plus upwind's numerical u Δz/2).
//!
//! **Results (2026-11-03):** Nu on D_h at m = 3/4/5/6: constant flux 4.4435 /
//! 4.4191 / 4.4059 / 4.3978 (+1.83 → +0.78 % of 48/11), extrapolated
//! (observed order 1.41) to 4.3704, +0.15 %; constant T 3.6953 / 3.6859 /
//! 3.6804 / 3.6768 against the shooting value 3.6585 (+1.01 → +0.50 %),
//! extrapolated (order 1.11) to 3.6607, +0.06 %. The shooting gives 3.65679
//! without axial diffusion. Energy imbalance ≤ 2e-10 of the wall heat.
//! Interpretation: the energy equation reproduces the developed Nusselt
//! numbers in the grid limit, at first order on this O-grid family (as
//! gh:#1168's f·Re). `measure_gh1156_graetz` (`--ignored`) prints the study;
//! the plain test runs m = 3 and gates it.
use std::sync::Arc;

use outram_foam_appbuilder_lib::cases::laminar_pipe::axial_layers;
use outram_foam_appbuilder_lib::io::control_dict::ControlDict;
use outram_foam_appbuilder_lib::io::fv_schemes::{DdtScheme, FvSchemes};
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::solvers::energy::{EnergyEquation, IncompressibleThermo};
use outram_foam_appbuilder_lib::solvers::non_orthogonal::LaplacianCorrection;
use outram_foam_appbuilder_lib::solvers::pimple_foam::PimpleFoam;
use outram_foam_basic_lib::fv_operators::fvm::DeltaCoeff;
use outram_foam_basic_lib::prelude::*;
use outram_foam_mesh::block_primitives::PipeOGrid;
use uom::si::f64::Length;
use uom::si::length::meter;
use uom::si::power::watt;

const R: f64 = 0.01;
const L: f64 = 0.2;
const U: f64 = 2.5e-3; // Re = U D / nu = 50 with nu = 1e-6
const K: f64 = 0.6;
const ALPHA: f64 = 1.0e-6; // Pr = 1
const Q_WALL: f64 = 100.0;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Wall {
    Flux,
    Temperature,
}

struct Run {
    nu_d: f64,
    nu_dh: f64,
    dz: f64,
    cells: usize,
    balance: f64,
    divergence: f64,
}

fn u_of(r2: f64) -> f64 {
    (2.0 * U * (1.0 - r2 / (R * R))).max(0.0)
}

fn run(m: usize, wall: Wall) -> Run {
    let g = PipeOGrid::level(Length::new::<meter>(R), Length::new::<meter>(L), m);
    let mesh = Arc::new(g.mesh().unwrap().to_fv_mesh().unwrap());
    let mut sol = FvSolution::default();
    sol.pimple.frozen_flow = true;
    sol.pimple.n_non_orthogonal_correctors = 1;
    let mut s = PimpleFoam::new(mesh.clone(), ControlDict::default(), FvSchemes::default(), sol);
    // Prescribed Hagen–Poiseuille flux on every axial face.
    let mut phi = SurfaceScalarField::zeros("phi", mesh.clone());
    for f in 0..mesh.n_internal_faces {
        let sf = mesh.face_area_vectors[f];
        let c = mesh.face_centres[f];
        phi.internal[f] = u_of(c.x * c.x + c.y * c.y) * sf.z;
    }
    for (pi, p) in mesh.patches.iter().enumerate() {
        for fi in 0..p.size {
            let gf = p.start + fi;
            let c = mesh.face_centres[gf];
            phi.boundary[pi].values[fi] = u_of(c.x * c.x + c.y * c.y) * mesh.face_area_vectors[gf].z;
        }
    }
    s.phi = phi;
    let rho_cp = K / ALPHA;
    let mut e = EnergyEquation::uniform(mesh.clone(), IncompressibleThermo::si(1000.0, rho_cp / 1000.0, K), 300.0);
    e.ddt = DdtScheme::SteadyState;
    e.set_patch("inlet", BoundaryCondition::FixedValue(300.0));
    match wall {
        Wall::Flux => e.set_patch("walls", BoundaryCondition::FixedGradient(Q_WALL / K)),
        Wall::Temperature => e.set_patch("walls", BoundaryCondition::FixedValue(350.0)),
    }
    s.energy = Some(e);
    // Steady, with the lagged non-orthogonal correction: repeat to a fixed point.
    for _ in 0..40 {
        let t_old = s.energy.as_ref().unwrap().t.clone();
        s.step().unwrap();
        let ch = s.energy.as_ref().unwrap().t.internal.as_slice().iter().zip(t_old.internal.as_slice()).fold(0.0_f64, |a, (x, y)| a.max((x - y).abs()));
        if ch < 1e-9 {
            break;
        }
    }
    let e = s.energy.as_ref().unwrap();
    let lap = LaplacianCorrection::Corrected;
    let pi = |name: &str| mesh.patches.iter().position(|p| p.name == name).unwrap();
    let (iw, ii, io) = (pi("walls"), pi("inlet"), pi("outlet"));
    // Energy balance: heat conducted in (wall, and axially at the inlet and
    // outlet faces) = net enthalpy carried out.
    let q_in = e.conduction_in(iw, lap).get::<watt>();
    let h_out = e.enthalpy_out(io, &s.phi).get::<watt>() + e.enthalpy_out(ii, &s.phi).get::<watt>();
    let cond_ends = e.conduction_in(ii, lap).get::<watt>() + e.conduction_in(io, lap).get::<watt>();
    let balance = (q_in + cond_ends - h_out) / q_in.abs();
    let mut divergence = 0.0_f64;
    {
        let mut div = vec![0.0; mesh.n_cells];
        for f in 0..mesh.n_internal_faces {
            div[mesh.owner[f]] += s.phi.internal[f];
            div[mesh.neighbour[f]] -= s.phi.internal[f];
        }
        for (p, pf) in mesh.patches.iter().zip(&s.phi.boundary) {
            for fi in 0..p.size {
                div[mesh.owner[p.start + fi]] += pf.values[fi];
            }
        }
        let qin = -s.phi.boundary[ii].values.as_slice().iter().sum::<f64>();
        for d in div {
            divergence = divergence.max(d.abs() / qin);
        }
    }
    // Per-layer Nu over z/D in [6, 9].
    let layers = axial_layers(&mesh);
    let layer_of = |z: f64| layers.iter().position(|l| (l - z).abs() < 1e-9);
    let nl = layers.len();
    let (mut tb_num, mut tb_den) = (vec![0.0; nl], vec![0.0; nl]);
    for f in 0..mesh.n_internal_faces {
        if mesh.face_area_vectors[f].z > 0.5 * mesh.face_areas[f] {
            let o = mesh.owner[f];
            if let Some(l) = layer_of(mesh.cell_centres[o].z) {
                tb_num[l] += s.phi.internal[f] * e.t.internal[o];
                tb_den[l] += s.phi.internal[f];
            }
        }
    }
    let (mut qw, mut tw, mut aw) = (vec![0.0; nl], vec![0.0; nl], vec![0.0; nl]);
    let p = &mesh.patches[iw];
    for fi in 0..p.size {
        let gf = p.start + fi;
        let o = mesh.owner[gf];
        let Some(l) = layer_of(mesh.cell_centres[o].z) else { continue };
        let a = mesh.face_areas[gf];
        qw[l] += e.face_conduction(iw, fi, lap).get::<watt>();
        let t_face = match wall {
            Wall::Temperature => 350.0,
            Wall::Flux => e.t.internal[o] + (Q_WALL / K) * DeltaCoeff::NonOrthogonal.boundary_delta(&mesh, gf),
        };
        tw[l] += t_face * a;
        aw[l] += a;
    }
    let d = 2.0 * R;
    let n4 = 4 * g.n;
    let perimeter = n4 as f64 * 2.0 * R * (std::f64::consts::PI / n4 as f64).sin();
    let dh = 4.0 * g.polygon_area() / perimeter;
    let mut nus = Vec::new();
    for l in 0..nl {
        let zd = layers[l] / d;
        if (6.0..=9.0).contains(&zd) {
            let q = qw[l] / aw[l];
            let tb = tb_num[l] / tb_den[l];
            nus.push(q * d / (K * (tw[l] / aw[l] - tb)));
        }
    }
    let nu_d = nus.iter().sum::<f64>() / nus.len() as f64;
    Run { nu_d, nu_dh: nu_d * dh / d, dz: L / g.nz as f64, cells: mesh.n_cells, balance, divergence }
}

/// The Graetz eigenproblem `f'' + f'/r + [β Pe_R û + a β² (1 + Pe_R û ε)] f = 0`,
/// `û = 2(1 − r²)`, `f'(0) = 0`, `f(1) = 0`; `a = 0` drops axial diffusion.
/// Returns Nu on D = 2R, `−2 f'(1)/f_b` with the mixing-cup `f_b`.
fn graetz_nu(pe_r: f64, axial: bool, eps: f64) -> f64 {
    let a = if axial { 1.0 } else { 0.0 };
    let n = 4000;
    let shoot = |beta: f64| -> (f64, f64, f64) {
        let rhs = |r: f64, f: f64, fp: f64| -> f64 {
            let u = 2.0 * (1.0 - r * r);
            -fp / r - (beta * pe_r * u + a * beta * beta * (1.0 + pe_r * u * eps)) * f
        };
        let h = 1.0 / n as f64;
        let r0 = 1e-9;
        let (mut r, mut f, mut fp) = (r0, 1.0, 0.0);
        let (mut num, mut den) = (0.0, 0.0);
        for _ in 0..n {
            let w = |r: f64, f: f64| 2.0 * (1.0 - r * r) * f * r;
            // trapezoid for the mixing cup
            let num0 = w(r, f);
            let den0 = w(r, 1.0);
            let k1 = (fp, rhs(r, f, fp));
            let k2 = (fp + 0.5 * h * k1.1, rhs(r + 0.5 * h, f + 0.5 * h * k1.0, fp + 0.5 * h * k1.1));
            let k3 = (fp + 0.5 * h * k2.1, rhs(r + 0.5 * h, f + 0.5 * h * k2.0, fp + 0.5 * h * k2.1));
            let k4 = (fp + h * k3.1, rhs(r + h, f + h * k3.0, fp + h * k3.1));
            f += h / 6.0 * (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0);
            fp += h / 6.0 * (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1);
            r += h;
            num += 0.5 * h * (num0 + w(r, f));
            den += 0.5 * h * (den0 + w(r, 1.0));
        }
        (f, fp, num / den)
    };
    // Smallest β with f(1) = 0: scan, then bisect.
    let (mut lo, mut b) = (1e-6, 1e-6);
    let f_lo = shoot(lo).0;
    loop {
        b += 0.002;
        if shoot(b).0.signum() != f_lo.signum() {
            break;
        }
        lo = b;
    }
    let mut hi = b;
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if shoot(mid).0.signum() == f_lo.signum() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let (_, fp, fb) = shoot(0.5 * (lo + hi));
    -2.0 * fp / fb
}

#[test]
fn graetz_shooting_reproduces_the_graetz_limit() {
    let nu = graetz_nu(U * R / ALPHA, false, 0.0);
    println!("Graetz shooting, no axial diffusion: Nu = {nu:.5}");
    assert!((nu - 3.657).abs() < 1e-3, "{nu}");
}

#[test]
fn developed_nusselt_numbers_m3() {
    let pe_r = U * R / ALPHA;
    for wall in [Wall::Flux, Wall::Temperature] {
        let r = run(3, wall);
        let pred = match wall {
            Wall::Flux => 48.0 / 11.0,
            Wall::Temperature => graetz_nu(pe_r, true, r.dz / (2.0 * R)),
        };
        println!(
            "m=3 {wall:?}: {} cells, Nu(D) {:.4}, Nu(D_h) {:.4}, reference {:.4}, balance {:.2e}, max div {:.1e}",
            r.cells, r.nu_d, r.nu_dh, pred, r.balance, r.divergence
        );
        assert!(r.divergence < 1e-12 && r.balance.abs() < 1e-8);
        assert!((r.nu_dh / pred - 1.0).abs() < 0.05, "{wall:?}: {} vs {pred}", r.nu_dh);
        if wall == Wall::Temperature {
            assert!(r.nu_dh > 3.657);
        }
    }
}

/// The grid study for the V&V record (m = 3, 4, 5, 6); `--ignored`.
#[test]
#[ignore]
fn measure_gh1156_graetz() {
    let pe_r = U * R / ALPHA;
    println!("Graetz limit (no axial): {:.5}", graetz_nu(pe_r, false, 0.0));
    println!("Physical axial conduction only: {:.5}", graetz_nu(pe_r, true, 0.0));
    for wall in [Wall::Flux, Wall::Temperature] {
        for m in [3, 4, 5, 6] {
            let t0 = std::time::Instant::now();
            let r = run(m, wall);
            let pred = match wall {
                Wall::Flux => 48.0 / 11.0,
                Wall::Temperature => graetz_nu(pe_r, true, r.dz / (2.0 * R)),
            };
            println!(
                "{wall:?} m={m} cells={} dz/D={:.4} Nu(D)={:.5} Nu(Dh)={:.5} ref={:.5} (Dh)/ref-1={:+.4} balance={:.1e} div={:.1e} {:.1}s",
                r.cells,
                r.dz / (2.0 * R),
                r.nu_d,
                r.nu_dh,
                pred,
                r.nu_dh / pred - 1.0,
                r.balance,
                r.divergence,
                t0.elapsed().as_secs_f64()
            );
        }
    }
}
