//! Iodine in the suppression pool: SPARC-90's equilibrium chemistry and
//! the partition limit on its scrubbing, as MELCOR describes them (gh:#1047;
//! no source used; MELCOR 2.2 Vol. 2, SAND2023-10994 O, RN §2.7.1,
//! Appendix D Eq D-7 and Appendix F).
//!
//! The pool's dissolved iodine is in fast equilibrium among five reactions
//! (Eq F.6):
//!
//! ```text
//! I2(g)          <-> I2(aq)             [I2aq] = K1 [I2g]
//! I2(aq) + I-    <-> I3-                [I3-] = K2 [I2aq][I-]
//! I2(aq) + H2O   <-> H+ + I- + HIO      [H+][I-][HIO] = K3 [I2aq]
//! I2(aq) + H2O   <-> H2OI+ + I-         [H2OI+][I-] = K4 [I2aq]
//! H2O            <-> H+ + OH-           [H+][OH-] = K5
//! ```
//!
//! with the total (Eq F.3, as I2 equivalents) `S = 0.5[I-] + 1.5[I3-] +
//! [I2aq] + 0.5[HIO] + 0.5[H2OI+]` and the charge balance (Eq F.15)
//! `([I-] - [I-]_p) + [I3-] - [H2OI+] - [HIO] = 0`. Here `[I-]_p` is the
//! iodide of dissolved CsI and `[OH-]_p` the hydroxide of dissolved CsOH.
//! The pool's partition coefficient for I2 is then `H = K1 S / [I2aq]`
//! (Eq F.5).
//!
//! **The proton balance is derived here, not copied.** From F.16, `[HIO] =
//! a / [H+]` with `a = K3 [I2aq] / [I-]`; put into F.18, `[H+]^2 -
//! ([HIO] - [OH-]_p)[H+] - K5 = 0`, it gives `[H+]^2 + [OH-]_p [H+] -
//! (a + K5) = 0`. That is the content of the manual's F.19 to F.21, whose
//! printed form did not survive extraction.
//!
//! **The equilibrium constants' forms are inferred.** The manual lists
//! C7159's coefficients and their units (`moles^-1`, `K`, `K^2`) but not
//! the functions. `K_i = a_i exp(b_i / T)`, and for water `K5 = a exp(b/T +
//! c/T^2)`, reproduce the textbook values at 298 K (the test): K1 = 83.6
//! (I2's water-to-gas ratio, about 80 to 90), K2 = 769 (I3- formation,
//! about 700 to 770), K3 = 4.0e-13 (I2 hydrolysis, about 5e-13), and Kw =
//! 1.0e-14 (pKw 14.0).
//!
//! **Where it acts.** MELCOR limits the condensation scrubbing of iodine
//! vapour by the partition (Eq D-7):
//!
//! ```text
//! DF_EC,vap = 1 + min(H rho_v / rho_l, 1) (DF_EC - 1)
//! ```
//!
//! The condensate cannot hold more iodine than the partition allows. And it
//! stops vapour scrubbing altogether when the pool's own iodine, over H,
//! exceeds the entering gas's (RN §2.7.1).

use petir::mathf::RealMath;

/// Molar masses, kg/mol: I2, and Cs (as CsOH, one hydroxide per atom).
pub const M_I2: f64 = 0.25381;
pub const M_CS: f64 = 0.13291;

/// The five equilibrium constants at `t_k` (C7159, forms inferred; see the
/// module doc): K1 dimensionless, K2 L/mol, K3 mol^2/L^2, K4 mol/L, K5
/// mol^2/L^2.
pub fn constants(t_k: f64) -> [f64; 5] {
    [
        1.3882e-3 * (3279.3 / t_k).r_exp(),
        7.7606 * (1370.0 / t_k).r_exp(),
        1.0423e-2 * (-7148.0 / t_k).r_exp(),
        4.2271e-9 * (-1748.5 / t_k).r_exp(),
        1.56531e-13 * (5462.81 / t_k - 1.87376e6 / (t_k * t_k)).r_exp(),
    ]
}

/// The equilibrium species, mol/L.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Equilibrium {
    pub i2_aq: f64,
    pub iodide: f64,
    pub hio: f64,
    pub h_plus: f64,
    pub tri_iodide: f64,
    pub h2oi: f64,
}

/// The root of an increasing `f` over `[lo, hi]` in the logarithm: the
/// Illinois variant of regula falsi on `asinh(f / scale)` (residuals span
/// many orders), to a log-step of 1e-14. Plain bisection took 52 halvings
/// each way and made the plant's run 40 % slower (the solve runs every
/// aerosol step); this takes about a dozen.
fn log_root<F: Fn(f64) -> f64>(f: F, lo: f64, hi: f64, scale: f64) -> f64 {
    // asinh through PETIR's logarithm (PETIR has no asinh).
    let asinh = |x: f64| x.signum() * (x.abs() + (x * x + 1.0).sqrt()).r_ln();
    let g = |u: f64| asinh(f(u.r_exp()) / scale);
    let (mut a, mut b) = (lo.r_ln(), hi.r_ln());
    let (mut fa, mut fb) = (g(a), g(b));
    if fa >= 0.0 {
        return lo;
    }
    if fb <= 0.0 {
        return hi;
    }
    let mut side = 0;
    for _ in 0..200 {
        let c = (a * fb - b * fa) / (fb - fa);
        let fc = g(c);
        if fc == 0.0 {
            return c.r_exp();
        }
        if fc < 0.0 {
            a = c;
            fa = fc;
            if side == -1 {
                fb *= 0.5;
            }
            side = -1;
        } else {
            b = c;
            fb = fc;
            if side == 1 {
                fa *= 0.5;
            }
            side = 1;
        }
        if (b - a).abs() < 1e-14 * (1.0 + a.abs().max(b.abs())) {
            return (0.5 * (a + b)).r_exp();
        }
    }
    (0.5 * (a + b)).r_exp()
}

/// The species for `[I2aq] = x` and `[I-] = y`, with `[OH-]_p` `oh_p`.
fn species(k: &[f64; 5], x: f64, y: f64, oh_p: f64) -> Equilibrium {
    let a = k[2] * x / y;
    let h = 0.5 * (-oh_p + (oh_p * oh_p + 4.0 * (a + k[4])).sqrt());
    Equilibrium { i2_aq: x, iodide: y, hio: a / h, h_plus: h, tri_iodide: k[1] * x * y, h2oi: k[3] * x / y }
}

/// The pool's equilibrium for a total dissolved iodine `s` (mol of I2
/// equivalents per litre, Eq F.3), CsOH hydroxide `oh_p` and CsI iodide
/// `i_p` (mol/L), at `t_k`. Solved as the manual does: for an assumed
/// `[I2aq]`, the charge balance gives `[I-]`; `[I2aq]` is then adjusted
/// until the total matches (both by bisection, every function monotone).
pub fn equilibrium(t_k: f64, s: f64, oh_p: f64, i_p: f64) -> Equilibrium {
    let k = constants(t_k);
    let at = |x: f64| {
        let charge = |y: f64| {
            let e = species(&k, x, y, oh_p);
            (y - i_p) + e.tri_iodide - e.h2oi - e.hio
        };
        let y = log_root(charge, 1e-30, 10.0, s.max(i_p).max(1e-30));
        species(&k, x, y, oh_p)
    };
    let total = |e: &Equilibrium| 0.5 * e.iodide + 1.5 * e.tri_iodide + e.i2_aq + 0.5 * e.hio + 0.5 * e.h2oi;
    let x = log_root(|x| total(&at(x)) - s, 1e-40, s.max(1e-40), s.max(1e-40));
    at(x)
}

/// The pool's partition coefficient for I2, `H = K1 S / [I2aq]` (Eq F.5):
/// dissolved iodine (all forms, as I2) per gaseous I2 at equilibrium. With
/// no dissolved iodine it is the hydrolysis limit at vanishing load, taken
/// at 1e-15 mol/L.
pub fn partition(t_k: f64, s: f64, oh_p: f64, i_p: f64) -> f64 {
    let s = s.max(1e-15);
    let e = equilibrium(t_k, s, oh_p, i_p);
    constants(t_k)[0] * s / e.i2_aq
}

/// MELCOR's Eq D-7: the condensation factor for iodine vapour, from the
/// gas's `df_ec`, the partition `h` and the vapour-to-liquid density
/// ratio.
pub fn vapour_condensation_df(df_ec: f64, h: f64, rho_v_over_rho_l: f64) -> f64 {
    1.0 + (h * rho_v_over_rho_l).min(1.0) * (df_ec - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The constants against textbook values at 298.15 K (2026-10-29):
    /// K1 83.6, K2 769, K3 4.0e-13, K4 1.2e-11, Kw 1.0e-14. And the
    /// equilibrium against its own balances: at 375 K, from neutral to
    /// pH 10, from trace to high loads, the total and the charge close to
    /// 1e-10 and every product matches its constant.
    #[test]
    fn the_iodine_equilibrium_closes() {
        let k = constants(298.15);
        let want = [83.6, 769.0, 4.0e-13, 1.2e-11, 1.0e-14];
        for (got, w) in k.iter().zip(want) {
            assert!((got / w - 1.0).abs() < 0.05, "{k:?}");
        }
        let k = constants(375.0);
        for &oh in &[0.0, 1e-6, 1e-4] {
            for &s in &[1e-9, 1e-6, 1e-4, 1e-2] {
                let e = equilibrium(375.0, s, oh, 0.0);
                let total = 0.5 * e.iodide + 1.5 * e.tri_iodide + e.i2_aq + 0.5 * e.hio + 0.5 * e.h2oi;
                assert!((total / s - 1.0).abs() < 1e-10, "{s} {oh} {e:?}");
                let charge = e.iodide + e.tri_iodide - e.h2oi - e.hio;
                assert!(charge.abs() < 1e-10 * s, "{s} {oh} {e:?}");
                assert!((e.h_plus * e.iodide * e.hio / (k[2] * e.i2_aq) - 1.0).abs() < 1e-9, "{e:?}");
                assert!((e.h_plus * (e.h_plus + oh - e.hio) / k[4] - 1.0).abs() < 1e-9, "{e:?}");
            }
        }
    }

    /// The partition's physics (2026-10-29). Predicted: at a trace load
    /// hydrolysis takes nearly all the iodine, so H is far above K1; at a
    /// high load in neutral water it falls toward K1 (8.72 at 375 K); CsOH
    /// keeps it high. Measured at 375 K:
    /// - trace (1e-9 M) in neutral water: 6.49e5, held;
    /// - 1e-2 M in neutral water: 8.91, held;
    /// - 1e-2 M with 1e-3 M CsOH: 10.5. The prediction "keeps it high"
    ///   **missed** here: that hydroxide is not an excess, and the
    ///   hydrolysis's protons use it up;
    /// - the plant's pool at 26 h (1.75e-5 M iodine, 4.3e-4 M CsOH, about
    ///   25:1): 2.08e4. Against `rho_v / rho_l` about 6.6e-4, the product
    ///   is about 14, so Eq D-7 does not bind. The same load in neutral
    ///   water gives 16.1 (product 0.011), which would bind hard.
    ///
    /// And Eq D-7: no partition, no scrubbing; a large one, the full DF_EC.
    #[test]
    fn hydrolysis_and_cs_oh_raise_the_partition() {
        let k1 = constants(375.0)[0];
        assert!((k1 - 8.715).abs() < 0.01, "{k1}");
        let near = |got: f64, want: f64| (got / want - 1.0).abs() < 0.01;
        let trace = partition(375.0, 1e-9, 0.0, 0.0);
        let loaded = partition(375.0, 1e-2, 0.0, 0.0);
        let short = partition(375.0, 1e-2, 1e-3, 0.0);
        let plant = partition(375.0, 1.75e-5, 4.3e-4, 0.0);
        let neutral = partition(375.0, 1.75e-5, 0.0, 0.0);
        assert!(near(trace, 6.494e5) && near(loaded, 8.915) && near(short, 10.52), "{trace} {loaded} {short}");
        assert!(near(plant, 2.081e4) && near(neutral, 16.13), "{plant} {neutral}");
        assert_eq!(vapour_condensation_df(5.0, 0.0, 1e-3), 1.0);
        assert_eq!(vapour_condensation_df(5.0, 1e4, 1e-3), 5.0);
        assert!((vapour_condensation_df(5.0, 100.0, 1e-3) - 1.4).abs() < 1e-12);
    }
}
