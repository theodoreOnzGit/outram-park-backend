//! P1 radiation against the closed-form gray-slab solution (gh:#1379).
//!
//! # Method
//! A 1 m slab of gray absorbing-emitting medium at uniform temperature `T0 = 1000 K`
//! between two diffuse walls (xmin: 1500 K, emissivity 0.8; xmax: 500 K, emissivity
//! 0.5), absorption = emission coefficient `a`. With `gamma = 1/(3a)` the P1 equation
//! `gamma G'' - a G = -4 a sigma T0^4` has the solution
//! `G = 4 sigma T0^4 + C1 cosh(k (x - L/2)) + C2 sinh(k (x - L/2))`, `k = sqrt(a/gamma)`,
//! with `C1, C2` fixed by the Marshak conditions `Ep (4 sigma Tw^4 - G_w) = gamma dG/dn_out`,
//! `Ep = eps/(2 (2 - eps))` (2x2 linear system solved in `analytic`). P1 and the Marshak
//! condition: M. F. Modest, *Radiative Heat Transfer*, 3rd ed., Academic Press 2013,
//! ch. 16 (spherical-harmonics method). sigma is ESI v2506's value, the one the port
//! uses. The port is run at N = 10, 20, 40, 80, 160 uniform cells (PCG, tolerance 1e-14)
//! and the maximum cell-centre error of `G`, relative to `max |G|`, and the wall
//! fluxes `qr` are compared with the closed form; the observed order is
//! `log2(err_N / err_2N)`.
//!
//! # Criterion
//! Prediction written before the run (PREDICTIONS.md, item 5): observed order between
//! 1.8 and 2.1 for the last pair, and the N = 80 relative error below 1e-4.
//!
//! # Results
//! `verification_and_validation/upstream_radiation_gh1379/README.md`.

use outram_foam_basic_lib::fields::boundary::bc::PatchField;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::vol_field::VolScalarField;
use outram_foam_basic_lib::ldu_matrix::SolverSettings;
use outram_foam_basic_lib::mesh::fv_mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_basic_lib::radiation::{
    stefan_boltzmann_esi_v2506, ConstantAbsorptionEmission, P1Model, P1PatchModel, ROOT_VSMALL,
};
use petir::mathf::RealMath;
use std::sync::Arc;

/// Uniform 1-D slab, length 1 m, cross-section `s` m2; patch 0 = xmin, patch 1 = xmax.
fn slab(n: usize, s: f64) -> Arc<FvMesh> {
    let h = 1.0 / n as f64;
    let (mut owner, mut neighbour, mut sf, mut cf) = (vec![], vec![], vec![], vec![]);
    for f in 0..n - 1 {
        owner.push(f);
        neighbour.push(f + 1);
        sf.push(Vector3::new(s, 0.0, 0.0));
        cf.push(Vector3::new((f + 1) as f64 * h, 0.0, 0.0));
    }
    owner.push(0);
    sf.push(Vector3::new(-s, 0.0, 0.0));
    cf.push(Vector3::new(0.0, 0.0, 0.0));
    owner.push(n - 1);
    sf.push(Vector3::new(s, 0.0, 0.0));
    cf.push(Vector3::new(1.0, 0.0, 0.0));
    Arc::new(
        FvMeshBuilder::new()
            .n_cells(n)
            .n_internal_faces(n - 1)
            .owner(owner)
            .neighbour(neighbour)
            .patches(vec![
                BoundaryPatch::new("xmin", n - 1, 1, PatchKind::Patch),
                BoundaryPatch::new("xmax", n, 1, PatchKind::Patch),
            ])
            .cell_volumes(vec![s * h; n])
            .cell_centres((0..n).map(|c| Vector3::new((c as f64 + 0.5) * h, 0.0, 0.0)).collect())
            .face_area_vectors(sf)
            .face_centres(cf)
            .build()
            .unwrap(),
    )
}

fn sinh(x: f64) -> f64 {
    0.5 * (x.r_exp() - (-x).r_exp())
}
fn cosh(x: f64) -> f64 {
    0.5 * (x.r_exp() + (-x).r_exp())
}

struct Analytic {
    gp: f64,
    k: f64,
    c1: f64,
    c2: f64,
    gamma: f64,
}

/// Closed form for medium coefficient `a` (= e), medium T0, walls `(T, eps)` at x = 0, 1.
fn analytic(a: f64, t0: f64, w0: (f64, f64), w1: (f64, f64)) -> Analytic {
    let sigma = stefan_boltzmann_esi_v2506();
    let gamma = 1.0 / (3.0 * a + ROOT_VSMALL);
    let k = (a / gamma).sqrt();
    let gp = 4.0 * sigma * t0.r_powi(4);
    let b = |t: f64| 4.0 * sigma * t.r_powi(4);
    let ep = |e: f64| e / (2.0 * (2.0 - e));
    let (e1, e2) = (ep(w0.1), ep(w1.1));
    let (shh, chh) = (sinh(k * 0.5), cosh(k * 0.5));
    let gk = gamma * k;
    // rows: C1*m11 + C2*m12 = r1 ; C1*m21 + C2*m22 = r2
    let (m11, m12, r1) = (-e1 * chh - gk * shh, e1 * shh + gk * chh, -e1 * (b(w0.0) - gp));
    let (m21, m22, r2) = (-e2 * chh - gk * shh, -e2 * shh - gk * chh, -e2 * (b(w1.0) - gp));
    let det = m11 * m22 - m12 * m21;
    Analytic { gp, k, c1: (r1 * m22 - m12 * r2) / det, c2: (m11 * r2 - r1 * m21) / det, gamma }
}

impl Analytic {
    fn g(&self, x: f64) -> f64 {
        self.gp + self.c1 * cosh(self.k * (x - 0.5)) + self.c2 * sinh(self.k * (x - 0.5))
    }
    fn dg(&self, x: f64) -> f64 {
        self.k * (self.c1 * sinh(self.k * (x - 0.5)) + self.c2 * cosh(self.k * (x - 0.5)))
    }
}

/// (error table, wall-flux errors) for one medium.
fn study(a: f64) -> Vec<(usize, f64, f64, f64)> {
    let (t0, w0, w1) = (1000.0, (1500.0, 0.8), (500.0, 0.5));
    let an = analytic(a, t0, w0, w1);
    let mut rows = vec![];
    for &n in &[10usize, 20, 40, 80, 160] {
        let mesh = slab(n, 0.1);
        let t = VolScalarField::new(
            "T",
            mesh.clone(),
            Field::new(vec![t0; n]),
            vec![PatchField::fixed_value(1, w0.0), PatchField::fixed_value(1, w1.0)],
        );
        let model = P1Model::new(
            ConstantAbsorptionEmission { absorptivity: a, emissivity: a, emission_source: 0.0 },
            vec![
                P1PatchModel::Marshak { emissivity: w0.1 },
                P1PatchModel::Marshak { emissivity: w1.1 },
            ],
        );
        let sol = model.solve(&t, SolverSettings { tolerance: 1e-14, max_iter: 4000 });
        assert!(sol.performance.converged, "N={n}");
        let gmax = (0..n).map(|c| an.g((c as f64 + 0.5) / n as f64).abs()).fold(0.0, f64::max);
        let err = (0..n)
            .map(|c| (sol.g.internal[c] - an.g((c as f64 + 0.5) / n as f64)).abs())
            .fold(0.0, f64::max)
            / gmax;
        // qr at the walls: outward flux -gamma dG/dn_out ; xmin n_out = -x, xmax n_out = +x
        let q0 = an.gamma * an.dg(0.0);
        let q1 = -an.gamma * an.dg(1.0);
        let qerr = ((sol.qr[0][0] - q0).abs() / q0.abs()).max((sol.qr[1][0] - q1).abs() / q1.abs());
        rows.push((n, err, qerr, an.g(0.5)));
    }
    rows
}

fn check(a: f64) {
    let rows = study(a);
    println!("a = {a} /m (optical thickness {:.2})", a * 1.0);
    println!("{:>5} {:>12} {:>8} {:>12} {:>8}", "N", "err_G", "order", "err_qr", "order");
    for i in 0..rows.len() {
        let (o1, o2) = if i == 0 {
            (f64::NAN, f64::NAN)
        } else {
            (
                (rows[i - 1].1 / rows[i].1).log2_nan(),
                (rows[i - 1].2 / rows[i].2).log2_nan(),
            )
        };
        println!("{:>5} {:>12.4e} {:>8.3} {:>12.4e} {:>8.3}", rows[i].0, rows[i].1, o1, rows[i].2, o2);
    }
    let n = rows.len();
    let order = (rows[n - 2].1 / rows[n - 1].1).log2_nan();
    assert!(order > 1.8 && order < 2.1, "observed order {order}");
    assert!(rows[3].1 < 1e-4, "N=80 error {}", rows[3].1);
}

/// `log2` through PETIR (`ln x / ln 2`), keeping the platform libm out of the test.
trait Log2Nan {
    fn log2_nan(self) -> f64;
}
impl Log2Nan for f64 {
    fn log2_nan(self) -> f64 {
        self.r_ln() / 2.0_f64.r_ln()
    }
}

#[test]
fn thick_slab_converges_to_the_p1_closed_form_at_second_order() {
    check(2.0);
}

#[test]
fn thin_slab_converges_to_the_p1_closed_form_at_second_order() {
    check(0.5);
}
