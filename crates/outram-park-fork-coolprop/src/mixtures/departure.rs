//! Binary **departure functions** `Δα_r(δ, τ, x)` — the excess reduced
//! Helmholtz energy beyond the mole-fraction-weighted pure-component sum.
//!
//! `Δα_r = Σ_i Σ_{j>i} x_i x_j F_{ij} · α_{r,ij}(δ, τ)`, where `F_{ij}` is the
//! binary scaling factor and `α_{r,ij}` is a departure term list (both from
//! [`super::binary_pairs`]). Ported from CoolProp's
//! `ResidualHelmholtzGeneralizedExponential::add_{Power,Gaussian,GERG2008Gaussian}`
//! (`include/CoolProp/fluids/Helmholtz.h`) — the three term forms its own 28
//! departure functions (`upstream_source/CoolProp/dev/mixtures/mixture_departure_functions.json`)
//! use (`type` field `Exponential`/`GERG-2008`/`Gaussian+Exponential`, split
//! per-term below rather than per-function, since the "power-only" leading
//! terms of the latter two types are exactly the `eta=beta=0` special case of
//! [`DepartureTerm::Gaussian`]/[`DepartureTerm::GergExponential`] — no special
//! casing needed).

use crate::eos::HelmholtzDerivs;

/// One term of a binary departure function.
#[derive(Debug, Clone, Copy)]
pub enum DepartureTerm {
    /// `n·δ^d·τ^t·exp(-δ^l)` (`l = 0` ⇒ plain `n·δ^d·τ^t`, matching
    /// [`crate::eos::ResidualTerm::Power`]'s per-term form). CoolProp
    /// `add_Power`/`add_Exponential` (departure `type: "Exponential"`).
    Power { n: f64, d: f64, t: f64, l: f64 },
    /// `n·δ^d·τ^t·exp(-η(δ-ε)² - β(τ-γ)²)` — the same form as
    /// [`crate::eos::ResidualTerm::Gaussian`]. CoolProp `add_Gaussian`
    /// (the tail of departure `type: "Gaussian+Exponential"`; `η=β=0` for
    /// that type's leading power-only terms).
    Gaussian {
        n: f64,
        d: f64,
        t: f64,
        eta: f64,
        epsilon: f64,
        beta: f64,
        gamma: f64,
    },
    /// `n·δ^d·τ^t·exp(-η(δ-ε)² - β(δ-γ))` — the GERG-2008 natural-gas-model
    /// Gaussian, where **both** the `η` and `β` corrections act on `δ`
    /// (unlike [`Self::Gaussian`], whose `β` term acts on `τ`). CoolProp
    /// `add_GERG2008Gaussian` (the tail of departure `type: "GERG-2008"`;
    /// `η=β=0` for that type's leading power-only terms).
    GergExponential {
        n: f64,
        d: f64,
        t: f64,
        eta: f64,
        epsilon: f64,
        beta: f64,
        gamma: f64,
    },
}

impl DepartureTerm {
    /// Accumulate this departure term's contribution to `acc` at reduced state
    /// `(δ, τ)` (before the `x_i x_j F_{ij}` binary weighting — see
    /// [`super::Mixture::residual_derivs`]).
    pub fn accumulate(&self, delta: f64, tau: f64, acc: &mut HelmholtzDerivs) {
        match *self {
            DepartureTerm::Power { n, d, t, l } => {
                // Identical to ResidualTerm::Power's per-term math (eos.rs).
                let (e, dl) = if l == 0.0 {
                    (1.0, 0.0)
                } else {
                    let dpow_l = delta.powf(l);
                    ((-dpow_l).exp(), l * dpow_l)
                };
                let a = n * delta.powf(d) * tau.powf(t) * e;
                let fd = (d - dl) / delta;
                let ft = t / tau;
                let fdd = ((d - dl) * (d - 1.0 - dl) - l * dl) / (delta * delta);
                add(acc, a, fd, ft, fdd, t * (t - 1.0) / (tau * tau), fd * ft);
            }
            DepartureTerm::Gaussian {
                n,
                d,
                t,
                eta,
                epsilon,
                beta,
                gamma,
            } => {
                // Identical to ResidualTerm::Gaussian's per-term math (eos.rs).
                let g = (-eta * (delta - epsilon).powi(2) - beta * (tau - gamma).powi(2)).exp();
                let a = n * delta.powf(d) * tau.powf(t) * g;
                let fd = d / delta - 2.0 * eta * (delta - epsilon);
                let ft = t / tau - 2.0 * beta * (tau - gamma);
                add(
                    acc,
                    a,
                    fd,
                    ft,
                    fd * fd - d / (delta * delta) - 2.0 * eta,
                    ft * ft - t / (tau * tau) - 2.0 * beta,
                    fd * ft,
                );
            }
            DepartureTerm::GergExponential {
                n,
                d,
                t,
                eta,
                epsilon,
                beta,
                gamma,
            } => {
                // u = -eta*(delta-epsilon)^2 - beta*(delta-gamma); both
                // corrections act on delta (not tau) -- see the module doc.
                let dm = delta - epsilon;
                let u = -eta * dm * dm - beta * (delta - gamma);
                let du_ddelta = -2.0 * eta * dm - beta;
                let a = n * delta.powf(d) * tau.powf(t) * u.exp();
                let fd = d / delta + du_ddelta;
                let ft = t / tau;
                let dfd_ddelta = -d / (delta * delta) - 2.0 * eta;
                add(
                    acc,
                    a,
                    fd,
                    ft,
                    fd * fd + dfd_ddelta,
                    ft * ft - t / (tau * tau),
                    fd * ft,
                );
            }
        }
    }
}

/// Accumulate `(a, a*fd, a*ft, a*fdd, a*ftt, a*fdt)` into `acc`.
#[allow(clippy::too_many_arguments)]
fn add(acc: &mut HelmholtzDerivs, a: f64, fd: f64, ft: f64, fdd: f64, ftt: f64, fdt: f64) {
    acc.a += a;
    acc.ad += a * fd;
    acc.at += a * ft;
    acc.add += a * fdd;
    acc.att += a * ftt;
    acc.adt += a * fdt;
}

#[cfg(test)]
mod self_consistency_tests {
    use super::*;

    fn eval_a(term: &DepartureTerm, delta: f64, tau: f64) -> f64 {
        let mut acc = HelmholtzDerivs::default();
        term.accumulate(delta, tau, &mut acc);
        acc.a
    }

    fn fd_derivs(term: &DepartureTerm, delta: f64, tau: f64) -> HelmholtzDerivs {
        let h = 1e-5;
        let a = eval_a(term, delta, tau);
        let ad = (eval_a(term, delta + h, tau) - eval_a(term, delta - h, tau)) / (2.0 * h);
        let at = (eval_a(term, delta, tau + h) - eval_a(term, delta, tau - h)) / (2.0 * h);
        let add = (eval_a(term, delta + h, tau) - 2.0 * a + eval_a(term, delta - h, tau)) / (h * h);
        let att = (eval_a(term, delta, tau + h) - 2.0 * a + eval_a(term, delta, tau - h)) / (h * h);
        let adt = (eval_a(term, delta + h, tau + h)
            - eval_a(term, delta + h, tau - h)
            - eval_a(term, delta - h, tau + h)
            + eval_a(term, delta - h, tau - h))
            / (4.0 * h * h);
        HelmholtzDerivs {
            a,
            ad,
            at,
            add,
            att,
            adt,
        }
    }

    fn check(name: &str, term: DepartureTerm, delta: f64, tau: f64) {
        let mut acc = HelmholtzDerivs::default();
        term.accumulate(delta, tau, &mut acc);
        let fd = fd_derivs(&term, delta, tau);
        eprintln!("{name}: analytic={acc:?}\n{name}: fd      ={fd:?}");
        let rel = |a: f64, b: f64| (a - b).abs() / b.abs().max(1e-10);
        assert!(
            rel(acc.ad, fd.ad) < 1e-4,
            "{name} ad: {} vs fd {}",
            acc.ad,
            fd.ad
        );
        assert!(
            rel(acc.at, fd.at) < 1e-4,
            "{name} at: {} vs fd {}",
            acc.at,
            fd.at
        );
        assert!(
            rel(acc.add, fd.add) < 1e-3,
            "{name} add: {} vs fd {}",
            acc.add,
            fd.add
        );
        assert!(
            rel(acc.att, fd.att) < 1e-3,
            "{name} att: {} vs fd {}",
            acc.att,
            fd.att
        );
        assert!(
            rel(acc.adt, fd.adt) < 1e-3,
            "{name} adt: {} vs fd {}",
            acc.adt,
            fd.adt
        );
    }

    #[test]
    fn power_matches_finite_difference() {
        check(
            "Power(l=0)",
            DepartureTerm::Power {
                n: 0.7,
                d: 2.0,
                t: 1.5,
                l: 0.0,
            },
            0.8,
            1.3,
        );
        check(
            "Power(l=2)",
            DepartureTerm::Power {
                n: -0.3,
                d: 1.0,
                t: 0.7,
                l: 2.0,
            },
            0.9,
            1.1,
        );
    }

    #[test]
    fn gaussian_matches_finite_difference() {
        check(
            "Gaussian",
            DepartureTerm::Gaussian {
                n: 0.5,
                d: 2.0,
                t: 1.2,
                eta: 0.9,
                epsilon: 0.5,
                beta: 1.5,
                gamma: 1.2,
            },
            0.85,
            1.15,
        );
    }

    #[test]
    fn gerg_exponential_matches_finite_difference() {
        check(
            "GergExponential",
            DepartureTerm::GergExponential {
                n: -0.4,
                d: 1.0,
                t: 3.65,
                eta: 1.0,
                epsilon: 0.5,
                beta: 1.0,
                gamma: 0.5,
            },
            0.8,
            1.3,
        );
        // Also check the eta=beta=0 "power-only" degenerate case.
        check(
            "GergExponential(eta=beta=0)",
            DepartureTerm::GergExponential {
                n: 0.28,
                d: 2.0,
                t: 1.85,
                eta: 0.0,
                epsilon: 0.0,
                beta: 0.0,
                gamma: 0.0,
            },
            0.8,
            1.3,
        );
    }
}
