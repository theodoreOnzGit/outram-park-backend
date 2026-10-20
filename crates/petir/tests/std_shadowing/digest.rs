// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! The digest shared by `tests/std_shadowing_digest.rs` and
//! `scripts/check-std-shadowing.sh` (gh #1001).
//!
//! Each group hashes the exact bits PETIR returns along a route that, before
//! gh #1001, called a `real::Real` method with METHOD syntax (`x.exp()`), and
//! so silently called the platform C library instead whenever `std` was in
//! the build graph. The groups follow the converted modules: linear algebra
//! (`householder`'s `hypot` reaches every QR and eigen path), the polynomial
//! solvers, the f64 special functions, the f32 WGSL CPU mirrors, the
//! transfer-function blocks, and `deriv` / `integration` / `ode` step control.
//!
//! Only `petir`'s public API is called here, and every float operation on the
//! test side is `+ - * /` or a bit cast, so the digest measures PETIR and
//! nothing else. The file is `include!`d by path, so it must stay
//! self-contained: no `use` of anything outside `petir` and `core`/`std`.

/// FNV-1a over 64-bit words.
pub struct Fnv(u64);

impl Fnv {
    pub fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    pub fn word(&mut self, w: u64) {
        for b in w.to_le_bytes() {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    /// The raw bits, NaNs included. NaNs are NOT canonicalised here: a NaN's
    /// sign once varied with `libm`'s `arch` feature (`libm::sqrt(-1.0)` is
    /// `0x7ff8...` in software and `0xfff8...` from `sqrtsd`, measured
    /// 2026-10-20), and PETIR now canonicalises it itself (`Real::sqrt`,
    /// `Real::mul_add`), so the digest must be able to see a regression.
    pub fn f64(&mut self, x: f64) {
        self.word(x.to_bits());
    }
    pub fn f32(&mut self, x: f32) {
        self.word(x.to_bits() as u64);
    }
    pub fn finish(&self) -> u64 {
        self.0
    }
}

/// xorshift64, so the inputs need no maths at all.
pub struct Xs(u64);

impl Xs {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Uniform in `[0, 1)`, from the mantissa bits only.
    pub fn unit(&mut self) -> f64 {
        f64::from_bits((self.next() >> 12) | 0x3ff0_0000_0000_0000) - 1.0
    }
}

fn hypot_group() -> u64 {
    let mut h = Fnv::new();
    let mut r = Xs(0x9e37_79b9_7f4a_7c15);
    for _ in 0..200_000 {
        let x = r.unit() - 0.5;
        let y = (r.unit() - 0.5) * 3.0;
        h.f64(petir::real::hypot(x, y));
        h.f32(petir::real::hypotf(x as f32, y as f32));
    }
    h.finish()
}

fn random_matrix(n: usize, seed: u64) -> petir::linalg::Matrix {
    let mut r = Xs(seed);
    let data = (0..n * n).map(|_| r.unit() * 2.0 - 1.0).collect();
    petir::linalg::Matrix::from_row_major(n, n, data).expect("square")
}

fn linalg_group() -> u64 {
    let mut h = Fnv::new();
    for (n, seed) in [(6usize, 11u64), (12, 12), (25, 13), (40, 14)] {
        let a = random_matrix(n, seed);
        for balance in [false, true] {
            for z in petir::linalg::eigen_nonsymm(&a, balance).expect("eigen") {
                h.f64(z.re);
                h.f64(z.im);
            }
        }
        let qr = petir::linalg::QrDecomposition::new(a.clone()).expect("qr");
        let b: Vec<f64> = (0..n).map(|i| 1.0 + i as f64).collect();
        for x in qr.solve(&b).expect("solve") {
            h.f64(x);
        }
        h.f64(qr.diagonal_ratio());
    }
    h.finish()
}

fn poly_group() -> u64 {
    let mut h = Fnv::new();
    let mut r = Xs(0x2545_f491_4f6c_dd1d);
    for _ in 0..2_000 {
        let (a, b, c, d) = (
            r.unit() + 0.1,
            r.unit() - 0.5,
            r.unit() - 0.5,
            r.unit() - 0.5,
        );
        let roots = petir::poly::CubicEqn::new(a, b, c, d).roots();
        for i in 0..3 {
            h.f64(roots.get(i));
        }
        let roots = petir::poly::QuadraticEqn::new(a, b, c).roots();
        for i in 0..2 {
            h.f64(roots.get(i));
        }
        for x in petir::poly::quartic::roots_cubic_normalized(b, c, d).as_slice() {
            h.f64(*x);
        }
        let p = petir::poly::Polynomial::new([a, b, c, d]);
        h.f64(p.value(r.unit() + 0.5));
        h.f64(p.integral(0.5, 1.5));
    }
    for k in 0..50 {
        let c: Vec<f64> = (0..8).map(|i| 1.0 + (i * k) as f64 / 7.0).collect();
        for z in petir::poly::companion::roots_companion(&c) {
            h.f64(z.re);
            h.f64(z.im);
        }
    }
    h.finish()
}

fn specfunc_group() -> u64 {
    use petir::specfunc as s;
    let mut h = Fnv::new();
    let one: [fn(f64) -> f64; 26] = [
        s::airy::airy_ai,
        s::airy::airy_bi,
        s::atanint::atanint,
        s::bessel_j0,
        s::bessel_y1,
        s::bessel_k1,
        s::clausen::clausen,
        s::debye_3,
        s::dilog::dilog,
        s::erfc,
        s::erf_inv,
        s::expint3::expint_3,
        s::fermi_dirac::fermi_dirac_half,
        s::gamma,
        s::ln_gamma,
        s::lambert::lambert_w0,
        s::psi,
        s::psi_1,
        s::sinint::si,
        s::sinint::ci,
        s::synchrotron::synchrotron_1,
        s::transport::transport_3,
        s::zeta,
        s::eta,
        s::trig::angle_restrict_pos,
        s::shint::shi,
    ];
    let mut r = Xs(0x1234_5678_9abc_def1);
    for _ in 0..400 {
        let u = r.unit();
        let x = u * 12.0 - 2.0; // [-2, 10)
        let xp = u * 10.0 + 0.05; // (0, 10.05)
        for f in one {
            h.f64(f(x));
            h.f64(f(xp));
        }
        let a = 0.1 + r.unit() * 8.0;
        h.f64(s::inc_gamma_q(a, xp));
        h.f64(s::inc_gamma_p(a, xp));
        h.f64(s::inv_inc_gamma(a, u * 0.98 + 0.01));
        h.f64(s::hzeta(1.5 + a, xp));
        h.f64(s::beta(a, xp));
        h.f64(s::gegenbauer::gegenpoly_n(5, a, u * 2.0 - 1.0));
        h.f64(s::legendre::legendre_plm(6, 2, u * 2.0 - 1.0));
        let k = u * 0.99;
        h.f64(s::ellint::ellint_f(xp, k, s::ellint::Mode::Double));
        h.f64(s::ellint::ellint_p(xp, k, 0.3, s::ellint::Mode::Double));
        let j = s::elljac::elljac(x, k);
        h.f64(j.sn);
        h.f64(j.cn);
        h.f64(j.dn);
    }
    h.finish()
}

fn mirror_group() -> u64 {
    use petir::wgsl as w;
    let mut h = Fnv::new();
    let one: [fn(f32) -> f32; 22] = [
        w::mirror_airy::airy_ai,
        w::mirror_atanint::atanint,
        w::mirror_bessel::j0,
        w::mirror_bessel::y1,
        w::mirror_clausen::clausen,
        w::mirror_dilog::dilog,
        w::mirror_erf::erfc,
        w::mirror_expint::e1,
        w::mirror_expint::ei,
        w::mirror_expint3::expint_3,
        w::mirror_fermi_dirac::fermi_dirac_half,
        w::mirror_fermi_dirac::fermi_dirac_m1,
        w::mirror_gamma::lngamma,
        w::mirror_gamma::gamma,
        w::mirror_lambert::lambert_w0,
        w::mirror_psi_zeta::psi,
        w::mirror_psi_zeta::zeta,
        w::mirror_sinint::si,
        w::mirror_sinint::ci,
        w::mirror_synchrotron::synchrotron_1,
        w::mirror_ellint::kcomp,
        w::mirror_ellint::ecomp,
    ];
    let mut r = Xs(0x0dde_c0de_5eed_1001);
    for _ in 0..400 {
        let u = r.unit() as f32;
        let x = u * 12.0 - 2.0;
        let xp = u * 10.0 + 0.05;
        let k = u * 0.99;
        for f in one {
            h.f32(f(x));
            h.f32(f(xp));
            h.f32(f(k));
        }
        h.f32(w::mirror_psi_zeta::hzeta(2.5, xp));
        h.f32(w::mirror_transport::transport(3, xp));
        h.f32(w::mirror_debye::debye_n(3, xp));
        h.f32(w::mirror_legendre_plm::legendre_plm(6, 2, u * 2.0 - 1.0));
        h.f32(w::mirror_gegenbauer::gegenpoly_n(5, 0.5 + u, u * 2.0 - 1.0));
        h.f32(w::mirror_ellint::ellint_e(xp, k));
        let j = w::mirror_elljac::elljac(x, k);
        h.f32(j.sn);
        h.f32(j.cn);
        h.f32(j.dn);
    }
    h.finish()
}

fn transfer_fn_group() -> u64 {
    use petir::transfer_fn::{
        second_order_transfer_fn::SecondOrderStableTransferFnNoZeroes, C2dMethod,
        ContinuousTransferFn,
    };
    use uom::si::f64::{Ratio, Time};
    use uom::si::ratio::ratio;
    use uom::si::time::second;
    let mut h = Fnv::new();
    let g = ContinuousTransferFn::new(vec![1.0, 1.0], vec![6.0, 5.0, 1.0]).expect("tf");
    for method in [
        C2dMethod::Zoh,
        C2dMethod::Tustin,
        C2dMethod::MatchedPoleZero,
    ] {
        let d = g
            .to_discrete(Time::new::<second>(0.1), method)
            .expect("c2d");
        for x in d
            .numerator_z_inverse()
            .iter()
            .chain(d.denominator_z_inverse())
        {
            h.f64(*x);
        }
    }
    for zeta in [0.3, 1.0, 2.5] {
        let mut tf = SecondOrderStableTransferFnNoZeroes::new(
            Ratio::new::<ratio>(1.7),
            Time::new::<second>(2.0),
            Ratio::new::<ratio>(zeta),
            Ratio::new::<ratio>(0.0),
            Ratio::new::<ratio>(0.0),
            Time::new::<second>(0.0),
        )
        .expect("stable");
        for step in 1..200 {
            let t = Time::new::<second>(0.05 * step as f64);
            let y = tf
                .set_user_input_and_calc_output(t, Ratio::new::<ratio>(1.0))
                .expect("output");
            h.f64(y.get::<ratio>());
        }
    }
    h.finish()
}

fn steps_group() -> u64 {
    let mut h = Fnv::new();
    let f = |x: f64| x * x * x - 2.0 * x + 1.0 / (1.0 + x * x);
    for i in 0..50 {
        let x = 0.1 * i as f64;
        let d = petir::deriv::central(f, x, 1e-3);
        h.f64(d.value);
        h.f64(d.abserr);
    }
    let q = petir::integration::qag(
        petir::integration::QkRule::Qk21,
        f,
        0.0,
        3.0,
        0.0,
        1e-10,
        200,
    )
    .expect("qag");
    h.f64(q.value);
    h.f64(q.abserr);
    let rhs = |_t: f64, y: &[f64], dy: &mut [f64]| {
        dy[0] = y[1];
        dy[1] = -y[0] - 0.1 * y[1];
    };
    let step = petir::ode::rkf45_step(rhs, 0.0, &[1.0, 0.0], 0.1).expect("rkf45");
    for x in step.y.iter().chain(&step.yerr) {
        h.f64(*x);
    }
    h.finish()
}

/// Every group's digest, in a fixed order.
pub fn digests() -> Vec<(&'static str, u64)> {
    vec![
        ("hypot", hypot_group()),
        ("linalg", linalg_group()),
        ("poly", poly_group()),
        ("specfunc", specfunc_group()),
        ("wgsl_mirrors", mirror_group()),
        ("transfer_fn", transfer_fn_group()),
        ("deriv_integration_ode", steps_group()),
    ]
}
