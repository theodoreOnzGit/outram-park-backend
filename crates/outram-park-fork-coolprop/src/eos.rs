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
