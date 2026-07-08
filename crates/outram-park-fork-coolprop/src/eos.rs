//! Helmholtz-energy-explicit equation of state: hardcoded fluid data +
//! enum-dispatched evaluation of the reduced Helmholtz energy `α(δ, τ)` and
//! its first/second derivatives w.r.t. the reduced density `δ = ρ/ρ_r` and
//! inverse reduced temperature `τ = T_r/T`.
//!
//! This is the OUTRAM PARK fork of CoolProp's `HelmholtzEOSBackend`: the same
//! Span–Wagner / IAPWS term structure, but every fluid's coefficients are
//! **hardcoded Rust `const` data** (see [`crate::fluids`]) — there is no
//! runtime JSON — and term dispatch is by **enum `match`**, never trait
//! objects.
//!
//! # Units
//!
//! All EOS data is **molar** (CoolProp's internal convention): `ρ_r`/`ρ_c` in
//! mol/m³, `R` in J/(mol·K), `molar_mass` in kg/mol. The property layer
//! ([`crate::props`]) converts to mass-based SI at the boundary.

/// One group of residual-Helmholtz terms, tagged by its functional form.
/// Each variant stores the per-term coefficient arrays (all the same length),
/// exactly as CoolProp's `alphar` blocks — but as `&'static [f64]` hardcoded
/// slices.
#[derive(Debug, Clone, Copy)]
pub enum ResidualTerm {
    /// `n·δ^d·τ^t·exp(-δ^l)` (with `l == 0` meaning **no** exponential factor,
    /// i.e. a pure polynomial term). CoolProp `ResidualHelmholtzPower`.
    Power {
        n: &'static [f64],
        t: &'static [f64],
        d: &'static [f64],
        l: &'static [f64],
    },
    /// `n·δ^d·τ^t·exp(-η(δ-ε)² - β(τ-γ)²)` — the Gaussian bell-shaped terms.
    /// CoolProp `ResidualHelmholtzGaussian`.
    Gaussian {
        n: &'static [f64],
        t: &'static [f64],
        d: &'static [f64],
        eta: &'static [f64],
        epsilon: &'static [f64],
        beta: &'static [f64],
        gamma: &'static [f64],
    },
    /// `n·δ^d·τ^t·exp(-g·δ^l)` — a Power term with an explicit coefficient `g`
    /// (and possibly non-integer `l`) on the exponential. CoolProp
    /// `ResidualHelmholtzExponential` (a special case of its
    /// `ResidualHelmholtzGeneralizedExponential`). Power is the `g == 1` case.
    Exponential {
        n: &'static [f64],
        t: &'static [f64],
        d: &'static [f64],
        g: &'static [f64],
        l: &'static [f64],
    },
    /// `n·δ^d·τ^t·exp(-g_d·δ^{l_d} - g_t·τ^{l_t})` — an exponential in **both**
    /// `δ` and `τ`. CoolProp `ResidualHelmholtzDoubleExponential`; the
    /// Lemmon–Jacobsen (2005) R125 term (`ResidualHelmholtzLemmon2005`) is the
    /// special case `g_d = g_t = 1`, which the codegen lowers to this form.
    DoubleExponential {
        n: &'static [f64],
        t: &'static [f64],
        d: &'static [f64],
        gd: &'static [f64],
        ld: &'static [f64],
        gt: &'static [f64],
        lt: &'static [f64],
    },
    /// `n·τ^t·exp(1/(b + β(τ-γ)²))·δ^d·exp(η(δ-ε)²)` — the Gao et al. bell-shaped
    /// term (a τ-side pole inside the exponent). CoolProp `ResidualHelmholtzGaoB`
    /// (used by Ammonia). The `τ` and `δ` factors separate, so the first/second
    /// derivatives follow from the per-factor expressions in CoolProp's `all()`.
    GaoB {
        n: &'static [f64],
        t: &'static [f64],
        d: &'static [f64],
        eta: &'static [f64],
        beta: &'static [f64],
        gamma: &'static [f64],
        epsilon: &'static [f64],
        b: &'static [f64],
    },
    /// The non-analytic critical-region terms (IAPWS-95 / Span–Wagner).
    /// CoolProp `ResidualHelmholtzNonAnalytic`.
    ///
    /// **Not yet evaluated** — see [`Self::accumulate`]. Data is carried so a
    /// fluid definition is complete; the contribution is currently zero, which
    /// only affects accuracy within ~1 % of the critical point (bead op-kbc:
    /// implement non-analytic terms + δ=1 handling next).
    NonAnalytic {
        n: &'static [f64],
        a: &'static [f64],
        b: &'static [f64],
        beta: &'static [f64],
        big_a: &'static [f64],
        big_b: &'static [f64],
        big_c: &'static [f64],
        big_d: &'static [f64],
    },
}

/// One group of ideal-gas-Helmholtz terms. CoolProp `alpha0` blocks.
#[derive(Debug, Clone, Copy)]
pub enum IdealTerm {
    /// `ln(δ) + a1 + a2·τ` — the lead term. CoolProp `IdealGasHelmholtzLead`.
    Lead { a1: f64, a2: f64 },
    /// `a·ln(τ)`. CoolProp `IdealGasHelmholtzLogTau`.
    LogTau { a: f64 },
    /// `Σ n_k·ln(1 - exp(-t_k·τ))` — Planck–Einstein (vibrational) terms.
    /// CoolProp `IdealGasHelmholtzPlanckEinstein`.
    PlanckEinstein { n: &'static [f64], t: &'static [f64] },
    /// `a1 + a2·τ` — a reference-state offset for enthalpy/entropy. Affects
    /// only absolute `h`/`s`/`u`; pressure, `c_v`, `c_p` and the speed of
    /// sound are unchanged. CoolProp `IdealGasHelmholtzEnthalpyEntropyOffset`
    /// (`src/Helmholtz.cpp`: `alphar += a1 + a2*tau; dalphar_dtau += a2`).
    EnthalpyEntropyOffset { a1: f64, a2: f64 },
    /// `Σ n_k·τ^{t_k}` — an ideal-gas power series in `τ`. CoolProp
    /// `IdealGasHelmholtzPower`.
    Power { n: &'static [f64], t: &'static [f64] },
    /// `Σ n_k·ln(c_k + d_k·exp(θ_k·τ))` — the generalized Planck–Einstein form.
    /// CoolProp `IdealGasHelmholtzPlanckEinsteinGeneralized`; the codegen also
    /// lowers `IdealGasHelmholtzPlanckEinsteinFunctionT` and the sinh/cosh part
    /// of `IdealGasHelmholtzCP0AlyLee` into this form.
    PlanckEinsteinGeneralized {
        n: &'static [f64],
        theta: &'static [f64],
        c: &'static [f64],
        d: &'static [f64],
    },
    /// Ideal-gas Helmholtz contribution of a **constant** isobaric heat
    /// capacity `c_p⁰/R`, integrated to the Helmholtz form. `tau0 = T_r/T0` is
    /// the reference-state ratio precomputed by the codegen. CoolProp
    /// `IdealGasHelmholtzCP0Constant`.
    CP0Constant { cp_over_r: f64, tau0: f64 },
    /// Ideal-gas Helmholtz contribution of a **polynomial-in-T** isobaric heat
    /// capacity `c_p⁰/R = Σ c_k·T^{t_k}`, integrated to the Helmholtz form.
    /// `tc` is the reducing temperature `T_r`, `t0` the reference temperature.
    /// CoolProp `IdealGasHelmholtzCP0PolyT` (the `IdealGasHelmholtzCP0AlyLee`
    /// constant term also lowers to this).
    CP0PolyT {
        c: &'static [f64],
        t: &'static [f64],
        tc: f64,
        t0: f64,
    },
}

/// A pure-fluid Helmholtz EOS: reducing/critical parameters plus the residual
/// and ideal term lists. All-`&'static`, so fluid definitions are `const`.
#[derive(Debug, Clone, Copy)]
pub struct FluidEos {
    /// Fluid name (as in CoolProp).
    pub name: &'static str,
    /// Molar mass \[kg/mol\].
    pub molar_mass: f64,
    /// Gas constant used by this EOS \[J/(mol·K)\].
    pub gas_constant: f64,
    /// Reducing temperature `T_r` \[K\] (for `τ = T_r/T`).
    pub t_reducing: f64,
    /// Reducing molar density `ρ_r` \[mol/m³\] (for `δ = ρ/ρ_r`).
    pub rho_reducing: f64,
    /// Critical temperature \[K\].
    pub t_critical: f64,
    /// Critical molar density \[mol/m³\].
    pub rho_critical: f64,
    /// Critical pressure \[Pa\].
    pub p_critical: f64,
    /// Triple-point temperature \[K\].
    pub t_triple: f64,
    /// Maximum valid temperature \[K\].
    pub t_max: f64,
    /// Maximum valid pressure \[Pa\].
    pub p_max: f64,
    /// Acentric factor \[-\].
    pub acentric: f64,
    /// Residual-Helmholtz term groups.
    pub residual: &'static [ResidualTerm],
    /// Ideal-gas-Helmholtz term groups.
    pub ideal: &'static [IdealTerm],
}

/// The reduced Helmholtz energy and its first/second derivatives at a state
/// `(δ, τ)`. Fields mirror the standard Span–Wagner notation:
/// `a = α`, `ad = ∂α/∂δ`, `at = ∂α/∂τ`, `add = ∂²α/∂δ²`, `att = ∂²α/∂τ²`,
/// `adt = ∂²α/∂δ∂τ`.
#[derive(Debug, Default, Clone, Copy)]
pub struct HelmholtzDerivs {
    pub a: f64,
    pub ad: f64,
    pub at: f64,
    pub add: f64,
    pub att: f64,
    pub adt: f64,
}

impl HelmholtzDerivs {
    #[inline]
    fn add_scaled(&mut self, o: &HelmholtzDerivs) {
        self.a += o.a;
        self.ad += o.ad;
        self.at += o.at;
        self.add += o.add;
        self.att += o.att;
        self.adt += o.adt;
    }
}

impl ResidualTerm {
    /// Accumulate this term group's contribution to `acc` at `(delta, tau)`.
    pub fn accumulate(&self, delta: f64, tau: f64, acc: &mut HelmholtzDerivs) {
        match self {
            ResidualTerm::Power { n, t, d, l } => {
                for i in 0..n.len() {
                    let (di, li, ti, ni) = (d[i], l[i], t[i], n[i]);
                    // exp(-δ^l), with l == 0 meaning no exponential factor.
                    let (e, dl) = if li == 0.0 {
                        (1.0, 0.0) // dl := l·δ^l  →  0
                    } else {
                        let dpow_l = delta.powf(li);
                        ((-dpow_l).exp(), li * dpow_l)
                    };
                    let a = ni * delta.powf(di) * tau.powf(ti) * e;
                    let fd = (di - dl) / delta; // (∂a/∂δ)/a
                    let ft = ti / tau; //           (∂a/∂τ)/a
                    // (∂²a/∂δ²)/a = [ (d-dl)(d-1-dl) - l·dl ] / δ²
                    let fdd = ((di - dl) * (di - 1.0 - dl) - li * dl) / (delta * delta);
                    acc.add_scaled(&HelmholtzDerivs {
                        a,
                        ad: a * fd,
                        at: a * ft,
                        add: a * fdd,
                        att: a * ti * (ti - 1.0) / (tau * tau),
                        adt: a * ft * fd,
                    });
                }
            }
            ResidualTerm::Gaussian { n, t, d, eta, epsilon, beta, gamma } => {
                for i in 0..n.len() {
                    let (di, ti, ni) = (d[i], t[i], n[i]);
                    let (etai, epsi, betai, gami) = (eta[i], epsilon[i], beta[i], gamma[i]);
                    let g = (-etai * (delta - epsi).powi(2) - betai * (tau - gami).powi(2)).exp();
                    let a = ni * delta.powf(di) * tau.powf(ti) * g;
                    let fd = di / delta - 2.0 * etai * (delta - epsi);
                    let ft = ti / tau - 2.0 * betai * (tau - gami);
                    acc.add_scaled(&HelmholtzDerivs {
                        a,
                        ad: a * fd,
                        at: a * ft,
                        add: a * (fd * fd - di / (delta * delta) - 2.0 * etai),
                        att: a * (ft * ft - ti / (tau * tau) - 2.0 * betai),
                        adt: a * fd * ft,
                    });
                }
            }
            ResidualTerm::Exponential { n, t, d, g, l } => {
                // Same B-factor structure as Power, but the exponential is
                // exp(-g·δ^l): define dl := l·g·δ^l = -δ·∂u/∂δ, then every
                // δ-derivative factor is identical to Power's.
                for i in 0..n.len() {
                    let (di, li, ti, ni, gi) = (d[i], l[i], t[i], n[i], g[i]);
                    let (e, dl) = if li == 0.0 || gi == 0.0 {
                        (1.0, 0.0)
                    } else {
                        let dpow_l = delta.powf(li);
                        ((-gi * dpow_l).exp(), li * gi * dpow_l)
                    };
                    let a = ni * delta.powf(di) * tau.powf(ti) * e;
                    let fd = (di - dl) / delta;
                    let ft = ti / tau;
                    let fdd = ((di - dl) * (di - 1.0 - dl) - li * dl) / (delta * delta);
                    acc.add_scaled(&HelmholtzDerivs {
                        a,
                        ad: a * fd,
                        at: a * ft,
                        add: a * fdd,
                        att: a * ti * (ti - 1.0) / (tau * tau),
                        adt: a * ft * fd,
                    });
                }
            }
            ResidualTerm::DoubleExponential { n, t, d, gd, ld, gt, lt } => {
                // u = -gd·δ^ld - gt·τ^lt; the δ and τ parts of u are
                // independent, so the B-factors separate (adt = ad·at/a).
                for i in 0..n.len() {
                    let (di, ti, ni) = (d[i], t[i], n[i]);
                    let (gdi, ldi, gti, lti) = (gd[i], ld[i], gt[i], lt[i]);
                    // DL := ld·gd·δ^ld = -δ·∂u/∂δ ; MT := lt·gt·τ^lt = -τ·∂u/∂τ
                    let (edelta, dl) = if ldi == 0.0 || gdi == 0.0 {
                        (1.0, 0.0)
                    } else {
                        let p = delta.powf(ldi);
                        ((-gdi * p).exp(), ldi * gdi * p)
                    };
                    let (etau, mt) = if lti == 0.0 || gti == 0.0 {
                        (1.0, 0.0)
                    } else {
                        let p = tau.powf(lti);
                        ((-gti * p).exp(), lti * gti * p)
                    };
                    let a = ni * delta.powf(di) * tau.powf(ti) * edelta * etau;
                    let bd = di - dl; //          δ·(∂ln a/∂δ)
                    let bt = ti - mt; //          τ·(∂ln a/∂τ)
                    let bd2 = (di - dl) * (di - 1.0 - dl) - ldi * dl;
                    let bt2 = (ti - mt) * (ti - 1.0 - mt) - lti * mt;
                    acc.add_scaled(&HelmholtzDerivs {
                        a,
                        ad: a * bd / delta,
                        at: a * bt / tau,
                        add: a * bd2 / (delta * delta),
                        att: a * bt2 / (tau * tau),
                        adt: a * bd * bt / (delta * tau),
                    });
                }
            }
            ResidualTerm::GaoB { n, t, d, eta, beta, gamma, epsilon, b } => {
                // a = n·Ftau·Fdelta with Ftau = τ^t·exp(1/P), P = b + β(τ-γ)²,
                // and Fdelta = δ^d·exp(η(δ-ε)²). The factors separate, so we
                // transcribe CoolProp's per-factor first/second derivatives.
                for i in 0..n.len() {
                    let (ni, ti, di) = (n[i], t[i], d[i]);
                    let (etai, betai, gami, epsi, bi) = (eta[i], beta[i], gamma[i], epsilon[i], b[i]);
                    let gmt = gami - tau;
                    let p = bi + betai * gmt * gmt;
                    let ftau = tau.powf(ti) * (1.0 / p).exp();
                    let de = delta - epsi;
                    let fdelta = delta.powf(di) * (etai * de * de).exp();
                    let a = ni * ftau * fdelta;

                    // τ·dFtau/dτ and τ²·d²Ftau/dτ² (CoolProp `taudFtaudtau`, `tau2d2Ftaudtau2`)
                    let taud_ftau = (2.0 * betai * tau.powf(ti + 1.0) * gmt + ti * tau.powf(ti) * p * p)
                        * (1.0 / p).exp() / (p * p);
                    let tau2d2_ftau = tau.powf(ti)
                        * (4.0 * betai * ti * tau * p * p * gmt
                            + 2.0 * betai * tau * tau
                                * (4.0 * betai * p * gmt * gmt + 2.0 * betai * gmt * gmt - p * p)
                            + ti * p.powi(4) * (ti - 1.0))
                        * (1.0 / p).exp() / p.powi(4);

                    // δ·dFdelta/dδ and δ²·d²Fdelta/dδ² (CoolProp `deltadFdeltaddelta`, `delta2d2Fdeltaddelta2`)
                    let deltad_fdelta = (di * delta.powf(di) + 2.0 * delta.powf(di + 1.0) * etai * de)
                        * (etai * de * de).exp();
                    let delta2d2_fdelta = delta.powf(di)
                        * (4.0 * di * delta * etai * de + di * (di - 1.0)
                            + 2.0 * delta * delta * etai * (2.0 * etai * de * de + 1.0))
                        * (etai * de * de).exp();

                    acc.add_scaled(&HelmholtzDerivs {
                        a,
                        ad: ni * ftau * deltad_fdelta / delta,
                        at: ni * fdelta * taud_ftau / tau,
                        add: ni * ftau * delta2d2_fdelta / (delta * delta),
                        att: ni * fdelta * tau2d2_ftau / (tau * tau),
                        adt: ni * taud_ftau * deltad_fdelta / (tau * delta),
                    });
                }
            }
            ResidualTerm::NonAnalytic { .. } => {
                // TODO(op-kbc): non-analytic critical-region terms + the δ=1
                // limit handling. Currently a no-op; only affects accuracy
                // within ~1 % of the critical point. Away from the critical
                // point ψ = exp(-C(δ-1)² - D(τ-1)²) ≈ 0 so the true
                // contribution is already negligible.
            }
        }
    }
}

impl IdealTerm {
    /// Accumulate this ideal-gas term group's contribution to `acc`.
    pub fn accumulate(&self, delta: f64, tau: f64, acc: &mut HelmholtzDerivs) {
        match self {
            IdealTerm::Lead { a1, a2 } => {
                acc.add_scaled(&HelmholtzDerivs {
                    a: delta.ln() + a1 + a2 * tau,
                    ad: 1.0 / delta,
                    at: *a2,
                    add: -1.0 / (delta * delta),
                    att: 0.0,
                    adt: 0.0,
                });
            }
            IdealTerm::LogTau { a } => {
                acc.add_scaled(&HelmholtzDerivs {
                    a: a * tau.ln(),
                    ad: 0.0,
                    at: a / tau,
                    add: 0.0,
                    att: -a / (tau * tau),
                    adt: 0.0,
                });
            }
            IdealTerm::PlanckEinstein { n, t } => {
                for i in 0..n.len() {
                    let (ni, tti) = (n[i], t[i]);
                    let e = (tti * tau).exp(); // e^{t·τ}
                    acc.a += ni * (1.0 - (-tti * tau).exp()).ln();
                    acc.at += ni * tti / (e - 1.0);
                    acc.att += -ni * tti * tti * e / ((e - 1.0) * (e - 1.0));
                    // no δ dependence
                }
            }
            IdealTerm::EnthalpyEntropyOffset { a1, a2 } => {
                acc.a += a1 + a2 * tau;
                acc.at += a2;
                // no δ dependence, no second τ-derivative
            }
            IdealTerm::Power { n, t } => {
                for i in 0..n.len() {
                    let (ni, ti) = (n[i], t[i]);
                    acc.a += ni * tau.powf(ti);
                    acc.at += ni * ti * tau.powf(ti - 1.0);
                    acc.att += ni * ti * (ti - 1.0) * tau.powf(ti - 2.0);
                    // no δ dependence
                }
            }
            IdealTerm::PlanckEinsteinGeneralized { n, theta, c, d } => {
                for i in 0..n.len() {
                    let (ni, thi, ci, di) = (n[i], theta[i], c[i], d[i]);
                    let e = (thi * tau).exp();
                    let para = ci + di * e;
                    acc.a += ni * para.ln();
                    acc.at += ni * thi * di * e / para;
                    acc.att += ni * thi * thi * ci * di * e / (para * para);
                    // no δ dependence
                }
            }
            IdealTerm::CP0Constant { cp_over_r, tau0 } => {
                let cr = *cp_over_r;
                acc.a += cr - cr * tau / tau0 + cr * (tau / tau0).ln();
                acc.at += cr / tau - cr / tau0;
                acc.att += -cr / (tau * tau);
                // no δ dependence
            }
            IdealTerm::CP0PolyT { c, t, tc, t0 } => {
                let (tc, t0) = (*tc, *t0);
                let tau0 = tc / t0;
                for i in 0..c.len() {
                    let (ci, ti) = (c[i], t[i]);
                    if ti.abs() < 10.0 * f64::EPSILON {
                        acc.a += ci - ci * tau / tau0 + ci * (tau / tau0).ln();
                        acc.at += ci / tau - ci / tau0;
                        acc.att += -ci / (tau * tau);
                    } else if (ti + 1.0).abs() < 10.0 * f64::EPSILON {
                        acc.a += ci * tau / tc * (tau0 / tau).ln() + ci / tc * (tau - tau0);
                        acc.at += ci / tc * (tau0 / tau).ln();
                        acc.att += -ci / (tau * tc);
                    } else {
                        acc.a += -ci * tc.powf(ti) * tau.powf(-ti) / (ti * (ti + 1.0))
                            - ci * t0.powf(ti + 1.0) * tau / (tc * (ti + 1.0))
                            + ci * t0.powf(ti) / ti;
                        acc.at += ci * tc.powf(ti) * tau.powf(-ti - 1.0) / (ti + 1.0)
                            - ci * tc.powf(ti) / (tau0.powf(ti + 1.0) * (ti + 1.0));
                        acc.att += -ci * (tc / tau).powf(ti) / (tau * tau);
                    }
                    // no δ dependence
                }
            }
        }
    }
}

impl FluidEos {
    /// Residual reduced Helmholtz energy `αʳ` and derivatives at `(δ, τ)`.
    pub fn residual_derivs(&self, delta: f64, tau: f64) -> HelmholtzDerivs {
        let mut acc = HelmholtzDerivs::default();
        for term in self.residual {
            term.accumulate(delta, tau, &mut acc);
        }
        acc
    }

    /// Ideal-gas reduced Helmholtz energy `α⁰` and derivatives at `(δ, τ)`.
    pub fn ideal_derivs(&self, delta: f64, tau: f64) -> HelmholtzDerivs {
        let mut acc = HelmholtzDerivs::default();
        for term in self.ideal {
            term.accumulate(delta, tau, &mut acc);
        }
        acc
    }
}
