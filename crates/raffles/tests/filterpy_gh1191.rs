//! Differential tests of the FilterPy port (gh:#1191) against FilterPy's own
//! outputs, committed in
//! `verification_and_validation/filterpy_gh1191/references/` by
//! `generate_references.py` (FilterPy `5df4e095fac0` = 2.0.0, numpy 2.4.6,
//! scipy 1.17.1, OpenBLAS SkylakeX), plus the analytic checks. Methodology,
//! predictions D1–D8 and A1–A4, and results:
//! `verification_and_validation/filterpy_gh1191.md`.

use raffles::filtering::{
    gaussian_draws, q_continuous_white_noise, q_discrete_white_noise, rts_smoother, EnsembleKalmanFilter, ExtendedKalmanFilter, KalmanFilter, Matrix,
    MerweScaledSigmaPoints, UnscentedKalmanFilter,
};
use serde_json::Value;

fn load(name: &str) -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/filterpy_gh1191/references").join(name);
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn flat(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.iter().flat_map(flat).collect(),
        Value::Number(n) => vec![n.as_f64().unwrap()],
        _ => panic!("not numeric: {v}"),
    }
}

fn mat(v: &Value) -> Matrix {
    let rows: Vec<Vec<f64>> = v.as_array().unwrap().iter().map(flat).collect();
    Matrix::from_rows(&rows).unwrap()
}

/// Per-quantity tally: how many numbers were compared, how many were
/// bit-identical, and the worst relative difference.
#[derive(Default)]
struct Tally {
    rows: Vec<(String, usize, usize, f64)>,
}

impl Tally {
    fn cmp(&mut self, name: &str, got: &[f64], want: &[f64]) {
        assert_eq!(got.len(), want.len(), "{name}: length");
        let idx = match self.rows.iter().position(|r| r.0 == name) {
            Some(i) => i,
            None => {
                self.rows.push((name.to_string(), 0, 0, 0.0));
                self.rows.len() - 1
            }
        };
        let r = &mut self.rows[idx];
        for (g, w) in got.iter().zip(want) {
            r.1 += 1;
            if g.to_bits() == w.to_bits() {
                r.2 += 1;
            } else {
                let rel = (g - w).abs() / w.abs().max(1e-300);
                r.3 = r.3.max(rel);
            }
        }
    }

    fn report_and_assert_identical(&self, what: &str) {
        println!("{what}:");
        for (n, total, same, worst) in &self.rows {
            println!("  {n:28} {same:6}/{total:6} bit-identical, worst rel {worst:.3e}");
        }
        for (n, total, same, worst) in &self.rows {
            assert_eq!(same, total, "{what} {n}: {same}/{total} bit-identical, worst {worst:.3e}");
        }
    }
}

/// **D1.** `Q_discrete_white_noise` and `Q_continuous_white_noise`: 108
/// calls (dim 2, 3, 4 × dt 0.1, 0.5, 1.3 × block 1, 2, 3 × both orders).
///
/// **Prediction:** bit-identical. **Result (2026-11-04):** bit-identical,
/// 4872 / 4872 numbers (asserted with `to_bits` equality).
#[test]
fn d1_process_noise_helpers_are_bit_identical() {
    let r = load("filterpy_filters.json");
    let mut t = Tally::default();
    for c in r["q"].as_array().unwrap() {
        let (dim, dt, var) = (c["dim"].as_u64().unwrap() as usize, c["dt"].as_f64().unwrap(), c["var"].as_f64().unwrap());
        let (bs, obd) = (c["block_size"].as_u64().unwrap() as usize, c["order_by_dim"].as_bool().unwrap());
        let q = match c["fn"].as_str().unwrap() {
            "discrete" => q_discrete_white_noise(dim, dt, var, bs, obd).unwrap(),
            _ => q_continuous_white_noise(dim, dt, var, bs, obd).unwrap(),
        };
        t.cmp(c["fn"].as_str().unwrap(), q.as_slice(), &flat(&c["Q"]));
    }
    t.report_and_assert_identical("D1 noise helpers");
}

fn kf_from_case(c: &Value) -> KalmanFilter {
    let f = mat(&c["F"]);
    let h = mat(&c["H"]);
    let mut kf = KalmanFilter::new(f.rows(), h.rows()).unwrap();
    kf.f = f;
    kf.h = h;
    kf.q = mat(&c["Q"]);
    kf.r = mat(&c["R"]);
    kf.x = mat(&c["x0"]);
    kf.p = mat(&c["P0"]);
    if !c["B"].is_null() {
        kf.b = Some(mat(&c["B"]));
    }
    kf
}

/// **D2, D3.** The linear KF (x, P, K, S, SI, y at every step, prior and
/// posterior) and its RTS smoother (x, P, K, Pp) on FilterPy's four
/// cases: scalar random walk, constant velocity, constant acceleration
/// with two correlated measurements and a control input, 2-D constant
/// velocity ordered by derivative.
///
/// **Prediction:** bit-identical. **Result (2026-11-04):** bit-identical:
/// KF 4670 / 4670 numbers, RTS 3700 / 3700 (asserted with `to_bits`).
#[test]
fn d2_d3_kalman_filter_and_rts_are_bit_identical() {
    let r = load("filterpy_filters.json");
    let mut t = Tally::default();
    let mut s = Tally::default();
    for c in r["kf"].as_array().unwrap() {
        let mut kf = kf_from_case(c);
        let u = if c["u"].is_null() { None } else { Some(mat(&c["u"])) };
        let (mut xs, mut ps) = (vec![], vec![]);
        for (z, st) in c["zs"].as_array().unwrap().iter().zip(c["steps"].as_array().unwrap()) {
            kf.predict(u.as_ref()).unwrap();
            t.cmp("x_prior", kf.x.as_slice(), &flat(&st["x_prior"]));
            t.cmp("P_prior", kf.p.as_slice(), &flat(&st["P_prior"]));
            kf.update(&flat(z)).unwrap();
            for (n, m) in [("x", &kf.x), ("P", &kf.p), ("K", &kf.k), ("S", &kf.s), ("SI", &kf.si), ("y", &kf.y)] {
                t.cmp(n, m.as_slice(), &flat(&st[n]));
            }
            xs.push(kf.x.clone());
            ps.push(kf.p.clone());
        }
        let sm = kf.rts_smoother(&xs, &ps).unwrap();
        let rr = &c["rts"];
        for (n, v) in [("x", &sm.x), ("P", &sm.p), ("K", &sm.k), ("Pp", &sm.pp)] {
            let got: Vec<f64> = v.iter().flat_map(|m| m.as_slice().to_vec()).collect();
            s.cmp(&format!("rts {n}"), &got, &flat(&rr[n]));
        }
    }
    t.report_and_assert_identical("D2 KF");
    s.report_and_assert_identical("D3 RTS");
}

/// libm `pow`, as numpy's `np.float64 ** 2` and Python's `float ** 2` are
/// (glibc's `pow(x, 2)` is not always `x * x`: measured 165 / 200 000).
fn sq(x: f64) -> f64 {
    x.powf(std::hint::black_box(2.0))
}

/// **D4.** The EKF on FilterPy's book radar (slant range to a 100 m/s,
/// 1000 m target), with the reference script's own Jacobian and hx.
///
/// **Prediction:** bit-identical (S is 1×1, where scipy's and numpy's
/// `inv` agree). **Result (2026-11-04):** bit-identical, 1200 / 1200.
#[test]
fn d4_ekf_is_bit_identical() {
    let r = load("filterpy_filters.json");
    let c = &r["ekf"][0];
    let mut f = ExtendedKalmanFilter::new(3, 1).unwrap();
    f.f = mat(&c["F"]);
    f.q = mat(&c["Q"]);
    f.r = mat(&c["R"]);
    f.x = mat(&c["x0"]);
    f.p = mat(&c["P0"]);
    let mut t = Tally::default();
    let jac = |x: &[f64]| {
        let denom = (sq(x[0]) + sq(x[2])).sqrt();
        Matrix::from_rows(&[vec![x[0] / denom, 0.0, x[2] / denom]])
    };
    let hx = |x: &[f64]| Ok(vec![(sq(x[0]) + sq(x[2])).sqrt()]);
    for (z, st) in c["zs"].as_array().unwrap().iter().zip(c["steps"].as_array().unwrap()) {
        f.predict().unwrap();
        t.cmp("x_prior", f.x.as_slice(), &flat(&st["x_prior"]));
        t.cmp("P_prior", f.p.as_slice(), &flat(&st["P_prior"]));
        f.update(&flat(z), jac, hx).unwrap();
        for (n, m) in [("x", &f.x), ("P", &f.p), ("K", &f.k), ("S", &f.s), ("SI", &f.si), ("y", &f.y)] {
            t.cmp(n, m.as_slice(), &flat(&st[n]));
        }
    }
    t.report_and_assert_identical("D4 EKF");
}

type Fx = fn(&[f64], f64) -> raffles::Result<Vec<f64>>;
type Hx = fn(&[f64]) -> raffles::Result<Vec<f64>>;

fn cv(x: &[f64], _dt: f64) -> raffles::Result<Vec<f64>> {
    Matrix::from_rows(&[vec![1.0, 0.1], vec![0.0, 1.0]])?.dot_vec(x)
}
fn first(x: &[f64]) -> raffles::Result<Vec<f64>> {
    Ok(vec![x[0]])
}
fn radar_f(x: &[f64], _dt: f64) -> raffles::Result<Vec<f64>> {
    let mut f = Matrix::identity(3);
    f.set(0, 1, 1.0 * 0.05);
    f.dot_vec(x)
}
fn radar_h(x: &[f64]) -> raffles::Result<Vec<f64>> {
    Ok(vec![(sq(x[0]) + sq(x[2])).sqrt()])
}
fn cubic_f(x: &[f64], dt: f64) -> raffles::Result<Vec<f64>> {
    Ok(vec![x[0] + dt * (0.5 * x[0] - 0.1 * x[0] * x[0] * x[0])])
}
fn cubic_h(x: &[f64]) -> raffles::Result<Vec<f64>> {
    Ok(vec![0.05 * x[0] * x[0] + x[0]])
}
fn ca(x: &[f64], _dt: f64) -> raffles::Result<Vec<f64>> {
    let dt = 0.5;
    Matrix::from_rows(&[vec![1.0, dt, 0.5 * dt * dt], vec![0.0, 1.0, dt], vec![0.0, 0.0, 1.0]])?.dot_vec(x)
}
fn first_last(x: &[f64]) -> raffles::Result<Vec<f64>> {
    Ok(vec![x[0], x[2]])
}

fn ukf_models(name: &str) -> (Fx, Hx) {
    match name {
        "ukf_constant_velocity" => (cv, first),
        "ukf_radar" => (radar_f, radar_h),
        "ukf_scalar_cubic" => (cubic_f, cubic_h),
        "ukf_constant_acceleration" => (ca, first_last),
        _ => panic!("{name}"),
    }
}

/// **D5, D6.** Merwe weights and initial sigma points, then the UKF
/// (x, P, K, S, SI, y and both sigma-point sets at every step) on four
/// cases: linear CV (α 0.1), the radar (α 0.5), a scalar cubic (α 1e-3,
/// the ddot path), linear CA with a 2×2 S (α 0.3). The fx / hx below are
/// the reference script's, operation for operation.
///
/// **Prediction:** bit-identical. **Result (2026-11-04):** bit-identical:
/// weights and initial points 99 / 99, UKF 7480 / 7480.
#[test]
fn d5_d6_sigma_points_and_ukf_are_bit_identical() {
    let r = load("filterpy_filters.json");
    let mut w = Tally::default();
    let mut t = Tally::default();
    for c in r["ukf"].as_array().unwrap() {
        let n = c["n"].as_u64().unwrap() as usize;
        let dz = c["dim_z"].as_u64().unwrap() as usize;
        let pts = MerweScaledSigmaPoints::new(n, c["alpha"].as_f64().unwrap(), c["beta"].as_f64().unwrap(), c["kappa"].as_f64().unwrap());
        w.cmp("Wm", &pts.wm, &flat(&c["Wm"]));
        w.cmp("Wc", &pts.wc, &flat(&c["Wc"]));
        let x0 = flat(&c["x0"]);
        let p0 = mat(&c["P0"]);
        w.cmp("sigmas_0", pts.sigma_points(&x0, &p0).unwrap().as_slice(), &flat(&c["sigmas_0"]));
        let mut u = UnscentedKalmanFilter::new(n, dz, c["dt"].as_f64().unwrap(), pts).unwrap();
        u.x = x0;
        u.p = p0;
        u.q = mat(&c["Q"]);
        u.r = mat(&c["R"]);
        let (fx, hx) = ukf_models(c["name"].as_str().unwrap());
        for (z, st) in c["zs"].as_array().unwrap().iter().zip(c["steps"].as_array().unwrap()) {
            u.predict(fx).unwrap();
            t.cmp("x_prior", &u.x, &flat(&st["x_prior"]));
            t.cmp("P_prior", u.p.as_slice(), &flat(&st["P_prior"]));
            t.cmp("sigmas_f", u.sigmas_f.as_slice(), &flat(&st["sigmas_f"]));
            u.update(&flat(z), hx).unwrap();
            t.cmp("x", &u.x, &flat(&st["x"]));
            t.cmp("y", &u.y, &flat(&st["y"]));
            for (nm, m) in [("P", &u.p), ("K", &u.k), ("S", &u.s), ("SI", &u.si), ("sigmas_h", &u.sigmas_h)] {
                t.cmp(nm, m.as_slice(), &flat(&st[nm]));
            }
        }
    }
    w.report_and_assert_identical("D5 sigma points");
    t.report_and_assert_identical("D6 UKF");
}

/// **D7.** The EnKF with FilterPy's own draws injected (the arrays numpy
/// drew, in call order: initial ensemble, then process and measurement
/// perturbations per step), CV N 20 and CA N 25.
///
/// **Prediction:** bit-identical. **Result (2026-11-04):** the first run
/// **missed**: 1e-15 to 9e-13 relative on about a quarter of the numbers.
/// The cause was found and measured: numpy reduces a one-element-wide axis 0
/// with its pairwise sum, not sequentially. After the fix (`numpy_ops`,
/// reusing `bayesian::koh::pairwise_sum`) it is bit-identical, 7580 / 7580.
#[test]
fn d7_enkf_with_injected_draws_is_bit_identical() {
    let r = load("filterpy_filters.json");
    let mut t = Tally::default();
    for c in r["enkf"].as_array().unwrap() {
        let draws: Vec<Matrix> = c["draws"].as_array().unwrap().iter().map(mat).collect();
        let (fx, hx): (Fx, Hx) = if c["n"].as_u64() == Some(2) { (cv, first) } else { (ca, first_last) };
        let mut f = EnsembleKalmanFilter::new(&flat(&c["x0"]), &mat(&c["P0"]), c["dim_z"].as_u64().unwrap() as usize, c["dt"].as_f64().unwrap(), draws[0].clone()).unwrap();
        f.q = mat(&c["Q"]);
        f.r = mat(&c["R"]);
        for (k, (z, st)) in c["zs"].as_array().unwrap().iter().zip(c["steps"].as_array().unwrap()).enumerate() {
            f.predict_with(fx, &draws[1 + 2 * k]).unwrap();
            t.cmp("x_prior", &f.x, &flat(&st["x_prior"]));
            t.cmp("P_prior", f.p.as_slice(), &flat(&st["P_prior"]));
            t.cmp("sigmas_prior", f.sigmas.as_slice(), &flat(&st["sigmas_prior"]));
            f.update_with(&flat(z), hx, &draws[2 + 2 * k]).unwrap();
            t.cmp("x", &f.x, &flat(&st["x"]));
            for (nm, m) in [("P", &f.p), ("K", &f.k), ("S", &f.s), ("SI", &f.si), ("sigmas", &f.sigmas)] {
                t.cmp(nm, m.as_slice(), &flat(&st[nm]));
            }
        }
    }
    t.report_and_assert_identical("D7 EnKF");
}

fn mean_sd(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    (m, (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0)).sqrt())
}

/// **D8 (statistical, kept apart from the bit-identity cases).** The
/// port's EnKF on its **own** LCG draws against FilterPy's EnKF on
/// numpy's draws: CV model, N 50, 30 steps, 400 replicates each (the port's
/// seeds 1 … 400, FilterPy's `np.random.seed(1000 + r)`), last step.
/// Criterion fixed before the run (V&V record): (a) replicate means of x̂
/// differ by < 3 √(se₁² + se₂²); (b) replicate variance ratio in
/// [0.74, 1.35]; (c) the mean P diagonals within 10 %; (d) each code's mean
/// P diagonal in [0.85, 1.05] × the exact KF's.
///
/// **Prediction:** pass. **Result (2026-11-04):** pass. x̂ means: port
/// 3.80488 / 0.54973, FilterPy 3.79472 / 0.54159, |diff|/se 1.19 / 1.55;
/// variance ratios 1.034 / 0.981; mean P port 0.58991 / 0.20562, FilterPy
/// 0.58980 / 0.20415; both ≈ 0.955 / 0.93 × the KF's P (EnKF N 50
/// under-dispersion, the same in both codes). The first run could not
/// start: the draws helper refused FilterPy's semi-definite Q (now
/// `psd_factor`).
#[test]
fn d8_enkf_statistically_matches_filterpy() {
    let r = load("filterpy_enkf_statistical.json");
    let (n, reps) = (r["N"].as_u64().unwrap() as usize, r["replicates"].as_u64().unwrap() as usize);
    let (x0, p0, q, rr) = (flat(&r["x0"]), mat(&r["P0"]), mat(&r["Q"]), mat(&r["R"]));
    let zs: Vec<Vec<f64>> = r["zs"].as_array().unwrap().iter().map(flat).collect();
    let fp: Vec<(Vec<f64>, Vec<f64>)> = r["finals"].as_array().unwrap().iter().map(|f| (flat(&f["x"]), mat(&f["P"]).diag())).collect();
    let mut port = Vec::new();
    for rep in 0..reps {
        let mut seed = 1 + rep as u64;
        let mut f = EnsembleKalmanFilter::new_drawn(&x0, &p0, 1, 0.1, n, &mut seed).unwrap();
        f.q = q.clone();
        f.r = rr.clone();
        for z in &zs {
            f.predict(cv, &mut seed).unwrap();
            f.update(z, first, &mut seed).unwrap();
        }
        port.push((f.x.clone(), f.p.diag()));
    }
    let kf_p = mat(&r["kf_P"]).diag();
    let kf_x = flat(&r["kf_x"]);
    for j in 0..2 {
        let a: Vec<f64> = port.iter().map(|p| p.0[j]).collect();
        let b: Vec<f64> = fp.iter().map(|p| p.0[j]).collect();
        let (ma, sa) = mean_sd(&a);
        let (mb, sb) = mean_sd(&b);
        let se = ((sa * sa + sb * sb) / reps as f64).sqrt();
        let pa = port.iter().map(|p| p.1[j]).sum::<f64>() / reps as f64;
        let pb = fp.iter().map(|p| p.1[j]).sum::<f64>() / reps as f64;
        println!(
            "D8 x[{j}]: port {ma:.5} (sd {sa:.4})  filterpy {mb:.5} (sd {sb:.4})  KF {:.5}; |diff|/se {:.2}; var ratio {:.3}; mean P port {pa:.5} filterpy {pb:.5} KF {:.5} (ratios {:.3} / {:.3})",
            kf_x[j],
            (ma - mb).abs() / se,
            (sa * sa) / (sb * sb),
            kf_p[j],
            pa / kf_p[j],
            pb / kf_p[j]
        );
        assert!((ma - mb).abs() < 3.0 * se, "D8a x[{j}]");
        assert!((0.74..=1.35).contains(&((sa * sa) / (sb * sb))), "D8b x[{j}]");
        assert!((pa / pb - 1.0).abs() < 0.10, "D8c x[{j}]");
        for ratio in [pa / kf_p[j], pb / kf_p[j]] {
            assert!((0.85..=1.05).contains(&ratio), "D8d x[{j}] {ratio}");
        }
    }
}

/// **A1.** Scalar random walk (F = H = 1, q = 0.04, r = 0.25) against the
/// Riccati fixed point `P⁻ = (q + √(q² + 4qr))/2`, `K = P⁻/(P⁻ + r)`.
///
/// **Prediction:** relative difference < 1e-12 after 60 steps.
/// **Result (2026-11-04):** K 2.2e-16, P⁻ 0 (met).
#[test]
fn a1_random_walk_gain_reaches_its_steady_state() {
    let (q, r) = (0.04_f64, 0.25_f64);
    let p_minus = (q + (q * q + 4.0 * q * r).sqrt()) / 2.0;
    let k_ss = p_minus / (p_minus + r);
    let mut kf = KalmanFilter::new(1, 1).unwrap();
    kf.h = Matrix::identity(1);
    kf.q = Matrix::diagonal(&[q]);
    kf.r = Matrix::diagonal(&[r]);
    kf.p = Matrix::diagonal(&[5.0]);
    for k in 1..=60 {
        kf.predict(None).unwrap();
        kf.update(&[0.05 * k as f64]).unwrap();
    }
    let (rk, rp) = ((kf.k.get(0, 0) / k_ss - 1.0).abs(), (kf.p_prior.get(0, 0) / p_minus - 1.0).abs());
    println!("A1: K {} vs {k_ss} (rel {rk:.2e}); P- {} vs {p_minus} (rel {rp:.2e})", kf.k.get(0, 0), kf.p_prior.get(0, 0));
    assert!(rk < 1e-12 && rp < 1e-12);
}

/// The batch least-squares posterior of the trajectory x₁ … x_N, in
/// covariance form (FilterPy's kinematic Q is singular, so the information
/// form cannot be used): the joint Gaussian prior from x₀ ~ N(m₀, P₀) and
/// x_k = F x_{k−1} + w (`C_kk = F C_{k−1,k−1} Fᵀ + Q`, `C_kj = F^{k−j} C_jj`),
/// observed through z_k = H x_k + v, conditioned densely:
/// `μ + G (z − H μ)` with `G = Σ Hᵀ (H Σ Hᵀ + R)⁻¹`, covariance in Joseph
/// form.
fn batch_posterior(kf: &KalmanFilter, m0: &[f64], p0: &Matrix, zs: &[Vec<f64>]) -> (Vec<Vec<f64>>, Vec<Matrix>, f64) {
    let (n, m, nz) = (kf.dim_x, kf.dim_z, zs.len());
    let (dim, dz) = (n * nz, m * nz);
    let mut means = vec![kf.f.dot_vec(m0).unwrap()];
    let mut diag = vec![kf.f.dot(p0).unwrap().dot(&kf.f.t()).unwrap().add(&kf.q).unwrap()];
    for k in 1..nz {
        means.push(kf.f.dot_vec(&means[k - 1]).unwrap());
        diag.push(kf.f.dot(&diag[k - 1]).unwrap().dot(&kf.f.t()).unwrap().add(&kf.q).unwrap());
    }
    let mut sigma = Matrix::zeros(dim, dim);
    for j in 0..nz {
        let mut c = diag[j].clone();
        for k in j..nz {
            if k > j {
                c = kf.f.dot(&c).unwrap();
            }
            for a in 0..n {
                for b in 0..n {
                    sigma.set(k * n + a, j * n + b, c.get(a, b));
                    sigma.set(j * n + b, k * n + a, c.get(a, b));
                }
            }
        }
    }
    let mut hb = Matrix::zeros(dz, dim);
    let mut rb = Matrix::zeros(dz, dz);
    let mut innov = vec![0.0; dz];
    for k in 0..nz {
        let hm = kf.h.dot_vec(&means[k]).unwrap();
        for a in 0..m {
            innov[k * m + a] = zs[k][a] - hm[a];
            for b in 0..n {
                hb.set(k * m + a, k * n + b, kf.h.get(a, b));
            }
            for b in 0..m {
                rb.set(k * m + a, k * m + b, kf.r.get(a, b));
            }
        }
    }
    let sht = sigma.dot(&hb.t()).unwrap();
    let gain = sht.dot(&hb.dot(&sht).unwrap().add(&rb).unwrap().inv().unwrap()).unwrap();
    let post_mean = gain.dot_vec(&innov).unwrap();
    let s_full = hb.dot(&sht).unwrap().add(&rb).unwrap();
    let alt_cov = sigma.sub(&gain.dot(&s_full).unwrap().dot(&gain.t()).unwrap()).unwrap();
    // Joseph form, (I − G H) Σ (I − G H)ᵀ + G R Gᵀ: the plain Σ − G H Σ
    // cancels catastrophically under the CV case's diffuse P₀ = 1000 I
    // (measured: 3.2e-7 relative against RTS, the instrument's own error).
    let i_gh = Matrix::identity(dim).sub(&gain.dot(&hb).unwrap()).unwrap();
    let post_cov = i_gh.dot(&sigma).unwrap().dot(&i_gh.t()).unwrap().add(&gain.dot(&rb).unwrap().dot(&gain.t()).unwrap()).unwrap();
    let out_means = (0..nz).map(|k| (0..n).map(|a| means[k][a] + post_mean[k * n + a]).collect()).collect();
    let covs = (0..nz)
        .map(|k| {
            let mut b = Matrix::zeros(n, n);
            for i in 0..n {
                for j in 0..n {
                    b.set(i, j, post_cov.get(k * n + i, k * n + j));
                }
            }
            b
        })
        .collect();
    let spread = (0..dim).fold(0.0f64, |w, i| {
        (0..dim).fold(w, |w, j| w.max((post_cov.get(i, j) - alt_cov.get(i, j)).abs() / (post_cov.get(i, i) * post_cov.get(j, j)).sqrt()))
    });
    (out_means, covs, spread)
}

/// **A2.** KF + RTS on the CV (dim 2, 50 steps) and CA (dim 3, 40 steps,
/// no control) models equals the batch least-squares posterior of the
/// whole trajectory: marginal means and covariance blocks.
///
/// **Prediction:** max relative difference < 1e-10 (scaled by each
/// quantity's magnitude).
///
/// **Result (2026-11-04):** means met (CV 3.2e-12, CA 2.9e-11). The
/// covariance **missed** on CA: 1.9e-10 (CV 9.1e-11). The first instrument,
/// an information form, could not run at all, because FilterPy's kinematic
/// Q is singular. A plain covariance form then measured 3.2e-7, which is its
/// own cancellation under P₀ = 1000 I. The Joseph form gives the numbers
/// above. The batch's spread between two algebraically equal forms is 6.4e-7
/// (CV) and 1.6e-5 (CA), so the 1.9e-10 cannot be attributed to the RTS.
/// The 1e-9 covariance gate was set after the miss (V&V record); the
/// maintainer reviewed and accepted it on 2026-11-04 (gh:#1208).
#[test]
fn a2_rts_equals_batch_least_squares() {
    let r = load("filterpy_filters.json");
    for c in r["kf"].as_array().unwrap().iter().filter(|c| matches!(c["name"].as_str(), Some("kf_constant_velocity" | "kf_constant_acceleration"))) {
        let mut kf = kf_from_case(c);
        kf.b = None;
        let zs: Vec<Vec<f64>> = c["zs"].as_array().unwrap().iter().map(flat).collect();
        let (m0, p0) = (mat(&c["x0"]).col(0), mat(&c["P0"]));
        let out = kf.batch_filter(&zs, None).unwrap();
        let sm = rts_smoother(&out.means, &out.covariances, &vec![kf.f.clone(); zs.len()], &vec![kf.q.clone(); zs.len()]).unwrap();
        let (bm, bc, spread) = batch_posterior(&kf, &m0, &p0, &zs);
        let (mut wm, mut wc) = (0.0f64, 0.0f64);
        for k in 0..zs.len() {
            let scale_m = bm[k].iter().fold(1.0f64, |a, v| a.max(v.abs()));
            for i in 0..kf.dim_x {
                wm = wm.max((sm.x[k].get(i, 0) - bm[k][i]).abs() / scale_m);
                let scale_c = (bc[k].get(i, i)).abs();
                for j in 0..kf.dim_x {
                    wc = wc.max((sm.p[k].get(i, j) - bc[k].get(i, j)).abs() / scale_c);
                }
            }
        }
        println!("A2 {}: worst mean rel {wm:.3e}, worst covariance rel {wc:.3e}; the batch instrument's own spread (Joseph vs symmetric form) {spread:.3e}", c["name"]);
        assert!(wm < 1e-10, "means: predicted < 1e-10");
        // Covariance: predicted < 1e-10, MISSED on CA (1.9e-10). The gate
        // below was set AFTER the miss, at 1e-9, and also requires the
        // difference to sit far inside the instrument's own spread.
        assert!(wc < 1e-9 && wc < 1e-3 * spread, "covariance {wc:.3e}, instrument spread {spread:.3e}");
    }
}

/// **A3.** The UKF on the linear CV and CA models equals the KF (the
/// unscented transform is exact for a linear map).
///
/// **Prediction:** max relative difference < 1e-9 on x and P.
/// **Result (2026-11-04):** CV x 5.5e-14, P 6.6e-14; CA x 6.1e-14, P 6.8e-14
/// (met).
#[test]
fn a3_ukf_on_a_linear_model_is_the_kf() {
    let r = load("filterpy_filters.json");
    for (kname, uname) in [("kf_constant_velocity", "ukf_constant_velocity"), ("kf_constant_acceleration", "ukf_constant_acceleration")] {
        let kc = r["kf"].as_array().unwrap().iter().find(|c| c["name"] == kname).unwrap();
        let uc = r["ukf"].as_array().unwrap().iter().find(|c| c["name"] == uname).unwrap();
        let mut kf = kf_from_case(kc);
        kf.b = None;
        let n = kf.dim_x;
        let pts = MerweScaledSigmaPoints::new(n, uc["alpha"].as_f64().unwrap(), 2.0, uc["kappa"].as_f64().unwrap());
        let mut u = UnscentedKalmanFilter::new(n, kf.dim_z, 0.1, pts).unwrap();
        u.x = flat(&uc["x0"]);
        u.p = mat(&uc["P0"]);
        u.q = kf.q.clone();
        u.r = kf.r.clone();
        kf.x = Matrix::column(&u.x);
        kf.p = u.p.clone();
        let (fx, hx) = ukf_models(uname);
        let (mut wx, mut wp) = (0.0f64, 0.0f64);
        for z in kc["zs"].as_array().unwrap() {
            kf.predict(None).unwrap();
            kf.update(&flat(z)).unwrap();
            u.predict(fx).unwrap();
            u.update(&flat(z), hx).unwrap();
            for i in 0..n {
                wx = wx.max((u.x[i] - kf.x.get(i, 0)).abs() / (kf.p.get(i, i).sqrt()).max(kf.x.get(i, 0).abs()));
                for j in 0..n {
                    wp = wp.max((u.p.get(i, j) - kf.p.get(i, j)).abs() / (kf.p.get(i, i) * kf.p.get(j, j)).sqrt());
                }
            }
        }
        println!("A3 {uname}: worst x {wx:.3e}, worst P {wp:.3e}");
        assert!(wx < 1e-9 && wp < 1e-9);
    }
}

/// **A4.** The EnKF's mean against the exact KF as N grows (CV model, 30
/// steps, N 50, 200, 800, 3200, one seed per N): the RMS over steps of the
/// error in position, in units of the KF's posterior σ.
///
/// **Prediction:** the error falls roughly as 1/√N: each 4× of N within a
/// factor 2 of halving it (ratio in [0.25, 1.0]). **Result (2026-11-04):**
/// 0.110, 0.075, 0.034, 0.017 σ (ratios 0.69, 0.45, 0.49; met, the first
/// step slower than 1/√N).
#[test]
fn a4_enkf_converges_to_the_kf() {
    let r = load("filterpy_enkf_statistical.json");
    let (x0, p0, q, rr) = (flat(&r["x0"]), mat(&r["P0"]), mat(&r["Q"]), mat(&r["R"]));
    let zs: Vec<Vec<f64>> = r["zs"].as_array().unwrap().iter().map(flat).collect();
    let mut errs = Vec::new();
    for n in [50usize, 200, 800, 3200] {
        let mut kf = KalmanFilter::new(2, 1).unwrap();
        kf.f = Matrix::from_rows(&[vec![1.0, 0.1], vec![0.0, 1.0]]).unwrap();
        kf.h = Matrix::from_rows(&[vec![1.0, 0.0]]).unwrap();
        kf.q = q.clone();
        kf.r = rr.clone();
        kf.x = Matrix::column(&x0);
        kf.p = p0.clone();
        let mut seed = 20_261_104;
        let mut f = EnsembleKalmanFilter::new_drawn(&x0, &p0, 1, 0.1, n, &mut seed).unwrap();
        f.q = q.clone();
        f.r = rr.clone();
        let mut sum = 0.0;
        for z in &zs {
            kf.predict(None).unwrap();
            kf.update(z).unwrap();
            f.predict(cv, &mut seed).unwrap();
            f.update(z, first, &mut seed).unwrap();
            sum += ((f.x[0] - kf.x.get(0, 0)) / kf.p.get(0, 0).sqrt()).powi(2);
        }
        errs.push((sum / zs.len() as f64).sqrt());
    }
    println!("A4 RMS error / sigma at N 50, 200, 800, 3200: {errs:?}");
    for w in errs.windows(2) {
        assert!((0.25..=1.0).contains(&(w[1] / w[0])), "{errs:?}");
    }
    // The draws helper is exercised by every EnKF above; one direct call
    // keeps its error path reached too.
    assert!(gaussian_draws(&[0.0, 0.0], &Matrix::identity(1), 1, &mut 1).is_err());
}
