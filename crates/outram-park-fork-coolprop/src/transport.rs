//! Transport properties — dynamic viscosity `μ` \[Pa·s\] and thermal
//! conductivity `λ` \[W/(m·K)\] — ported from CoolProp's `TransportRoutines`.
//!
//! These are **separate correlations** from the Helmholtz EOS (CoolProp's
//! `TRANSPORT` block), evaluated at the EOS state `(T, ρ)`. Coverage is the
//! common model subset:
//!
//! - **Viscosity** = dilute + higher-order:
//!   dilute `collision_integral`, higher-order `modified_Batschinski_Hildebrand`.
//! - **Conductivity** = dilute + residual:
//!   dilute `ratio_of_polynomials` or `eta0_and_poly`; residual `polynomial` or
//!   `polynomial_and_exponential`.
//!
//! # Deliberate omission — critical enhancement
//!
//! The near-critical enhancement terms (viscosity/conductivity
//! `simplified_Olchowy_Sengers`, etc.) are **not** evaluated — the same fast-path
//! simplification `tampines-steam-tables` makes. Away from the critical point
//! their contribution is negligible; within roughly a few percent of `T_c`,`ρ_c`
//! `λ` in particular is under-predicted. Fluids whose transport is *hardcoded*
//! (Water, CO₂, the R-blends, …) or uses an unimplemented model carry **no**
//! transport model here (`Fluid::transport()` → the relevant field is `None`) —
//! never a wrong number.

use crate::fluid::Fluid;

/// Dilute-gas viscosity model (the `T`-only, zero-density limit).
#[derive(Debug, Clone, Copy)]
pub enum ViscosityDilute {
    /// `η₀ = C·√(M[g/mol]·T) / (σ[nm]²·𝔖)`, `𝔖 = exp(Σ aᵢ·(ln T*)^{tᵢ})`,
    /// `T* = T/(ε/k)`. CoolProp `collision_integral`.
    CollisionIntegral {
        c: f64,
        a: &'static [f64],
        t: &'static [f64],
        /// Molar mass \[kg/mol\] (the transport block's own value).
        molar_mass: f64,
        /// Lennard-Jones `ε/k` \[K\].
        epsilon_over_k: f64,
        /// Lennard-Jones `σ` \[m\].
        sigma_eta: f64,
    },
    /// `η₀ = Σ aᵢ·T^{tᵢ}` \[Pa·s\]. CoolProp `powers_of_T`.
    PowersOfT { a: &'static [f64], t: &'static [f64] },
}

/// Initial-density (linear-in-ρ) viscosity correction.
#[derive(Debug, Clone, Copy)]
pub enum ViscosityInitial {
    /// Rainwater–Friend second viscosity virial: `B_η = N_A·σ³·Σ bᵢ·(T*)^{tᵢ}`
    /// \[m³/mol\], `T* = T/(ε/k)`; the contribution is `η₀·B_η·ρ_molar`.
    /// CoolProp `Rainwater-Friend`.
    RainwaterFriend {
        b: &'static [f64],
        t: &'static [f64],
        epsilon_over_k: f64,
        sigma_eta: f64,
    },
}

impl ViscosityInitial {
    /// Second viscosity virial `B_η` \[m³/mol\] at temperature `t` \[K\].
    fn b_eta(&self, t: f64) -> f64 {
        match self {
            ViscosityInitial::RainwaterFriend { b, t: tt, epsilon_over_k, sigma_eta } => {
                let tstar = t / epsilon_over_k;
                let mut s = 0.0;
                for i in 0..b.len() {
                    s += b[i] * tstar.powf(tt[i]);
                }
                6.022_141_29e23 * sigma_eta.powi(3) * s
            }
        }
    }
}

/// Higher-order (density-dependent) viscosity model.
#[derive(Debug, Clone, Copy)]
pub enum ViscosityHigherOrder {
    /// CoolProp `modified_Batschinski_Hildebrand` (see `TransportRoutines`).
    /// `δ = ρ_molar/ρ_reduce`, `τ = T_reduce/T`.
    ModifiedBatschinskiHildebrand {
        t_reduce: f64,
        rhomolar_reduce: f64,
        a: &'static [f64],
        d1: &'static [f64],
        t1: &'static [f64],
        gamma: &'static [f64],
        l: &'static [f64],
        f: &'static [f64],
        d2: &'static [f64],
        t2: &'static [f64],
        g: &'static [f64],
        h: &'static [f64],
        p: &'static [f64],
        q: &'static [f64],
    },
}

/// A complete viscosity model: dilute + (optional) initial-density + higher-order
/// (Pa·s).
#[derive(Debug, Clone, Copy)]
pub struct ViscosityModel {
    pub dilute: ViscosityDilute,
    pub initial: Option<ViscosityInitial>,
    pub higher_order: ViscosityHigherOrder,
}

/// Dilute-gas thermal-conductivity model.
#[derive(Debug, Clone, Copy)]
pub enum ConductivityDilute {
    /// `λ₀ = (Σ Aᵢ·Tr^{nᵢ}) / (Σ Bᵢ·Tr^{mᵢ})`, `Tr = T/T_reducing`.
    /// CoolProp `ratio_of_polynomials`.
    RatioPolynomials {
        t_reducing: f64,
        a: &'static [f64],
        n: &'static [f64],
        b: &'static [f64],
        m: &'static [f64],
    },
    /// `λ₀ = A₀·η₀[µPa·s] + Σ_{i≥1} Aᵢ·τ^{tᵢ}`, `τ = T_r/T`, `η₀` the dilute
    /// viscosity. CoolProp `eta0_and_poly` (needs the viscosity model).
    Eta0AndPoly { a: &'static [f64], t: &'static [f64] },
}

/// Residual (density-dependent) thermal-conductivity model.
#[derive(Debug, Clone, Copy)]
pub enum ConductivityResidual {
    /// `Σ Bᵢ·τ^{tᵢ}·δ^{dᵢ}`, `τ = T_reducing/T`, `δ = ρ_mass/ρ_mass_reducing`.
    /// CoolProp `polynomial`.
    Polynomial {
        t_reducing: f64,
        rhomass_reducing: f64,
        b: &'static [f64],
        t: &'static [f64],
        d: &'static [f64],
    },
    /// `Σ Aᵢ·τ^{tᵢ}·δ^{dᵢ}·exp(−γᵢ·δ^{lᵢ})`, `τ`,`δ` the **EOS** reduced
    /// variables. CoolProp `polynomial_and_exponential`.
    PolynomialAndExponential {
        a: &'static [f64],
        t: &'static [f64],
        d: &'static [f64],
        gamma: &'static [f64],
        l: &'static [f64],
    },
}

/// A complete conductivity model: dilute + residual (W/(m·K)); the critical
/// enhancement is omitted (see module docs).
#[derive(Debug, Clone, Copy)]
pub struct ConductivityModel {
    pub dilute: ConductivityDilute,
    pub residual: ConductivityResidual,
}

/// The transport models a fluid carries. Either may be `None` (hardcoded or
/// unimplemented model type, or the fluid ships no transport data).
#[derive(Debug, Clone, Copy)]
pub struct FluidTransport {
    pub viscosity: Option<ViscosityModel>,
    pub conductivity: Option<ConductivityModel>,
}

impl ViscosityDilute {
    /// Dilute-gas viscosity \[Pa·s\] at temperature `t` \[K\].
    fn eval(&self, t: f64) -> f64 {
        match self {
            ViscosityDilute::CollisionIntegral { c, a, t: tt, molar_mass, epsilon_over_k, sigma_eta } => {
                let tstar = t / epsilon_over_k;
                let sigma_nm = sigma_eta * 1e9;
                let mm_kgkmol = molar_mass * 1000.0;
                let ln_tstar = tstar.ln();
                let mut summer = 0.0;
                for i in 0..a.len() {
                    summer += a[i] * ln_tstar.powf(tt[i]);
                }
                let s = summer.exp();
                c * (mm_kgkmol * t).sqrt() / (sigma_nm * sigma_nm * s)
            }
            ViscosityDilute::PowersOfT { a, t: tt } => {
                let mut summer = 0.0;
                for i in 0..a.len() {
                    summer += a[i] * t.powf(tt[i]);
                }
                summer
            }
        }
    }
}

impl ViscosityHigherOrder {
    /// Higher-order viscosity \[Pa·s\] at molar density `rho_molar` \[mol/m³\]
    /// and temperature `t` \[K\].
    fn eval(&self, rho_molar: f64, t: f64) -> f64 {
        match self {
            ViscosityHigherOrder::ModifiedBatschinskiHildebrand {
                t_reduce, rhomolar_reduce, a, d1, t1, gamma, l, f, d2, t2, g, h, p, q,
            } => {
                let delta = rho_molar / rhomolar_reduce;
                let tau = t_reduce / t;
                let mut s = 0.0;
                for i in 0..a.len() {
                    s += a[i] * delta.powf(d1[i]) * tau.powf(t1[i]) * (gamma[i] * delta.powf(l[i])).exp();
                }
                let mut ff = 0.0;
                for i in 0..f.len() {
                    ff += f[i] * delta.powf(d2[i]) * tau.powf(t2[i]);
                }
                let mut num = 0.0;
                for i in 0..g.len() {
                    num += g[i] * tau.powf(h[i]);
                }
                let mut den = 0.0;
                for i in 0..p.len() {
                    den += p[i] * tau.powf(q[i]);
                }
                let delta0 = num / den;
                s + ff * (1.0 / (delta0 - delta) - 1.0 / delta0)
            }
        }
    }
}

impl ViscosityModel {
    /// Total dynamic viscosity \[Pa·s\] at temperature `t` \[K\] and mass
    /// density `rho` \[kg/m³\] (using the fluid's molar mass for `ρ_molar`):
    /// dilute + initial-density (`η₀·B_η·ρ_molar`) + higher-order.
    pub fn eval(&self, t: f64, rho: f64, molar_mass: f64) -> f64 {
        let rho_molar = rho / molar_mass;
        let eta_dilute = self.dilute.eval(t);
        let eta_initial = self
            .initial
            .map(|i| eta_dilute * i.b_eta(t) * rho_molar)
            .unwrap_or(0.0);
        eta_dilute + eta_initial + self.higher_order.eval(rho_molar, t)
    }
}

impl ConductivityModel {
    /// Total thermal conductivity \[W/(m·K)\] at `(T, ρ)`. `dilute_visc` is the
    /// fluid's dilute viscosity (needed only by the `eta0_and_poly` dilute
    /// model); pass `None` if the fluid has no viscosity model.
    fn eval(&self, fluid: Fluid, t: f64, rho: f64, dilute_visc: Option<f64>) -> Option<f64> {
        let eos = fluid.eos();
        let tau_eos = eos.t_reducing / t;
        let delta_eos = (rho / eos.molar_mass) / eos.rho_reducing;

        let lambda_dilute = match &self.dilute {
            ConductivityDilute::RatioPolynomials { t_reducing, a, n, b, m } => {
                let tr = t / t_reducing;
                let mut s1 = 0.0;
                for i in 0..a.len() {
                    s1 += a[i] * tr.powf(n[i]);
                }
                let mut s2 = 0.0;
                for i in 0..b.len() {
                    s2 += b[i] * tr.powf(m[i]);
                }
                s1 / s2
            }
            ConductivityDilute::Eta0AndPoly { a, t: tt } => {
                let eta0_upas = dilute_visc? * 1e6;
                let mut summer = a[0] * eta0_upas;
                for i in 1..a.len() {
                    summer += a[i] * tau_eos.powf(tt[i]);
                }
                summer
            }
        };

        let lambda_residual = match &self.residual {
            ConductivityResidual::Polynomial { t_reducing, rhomass_reducing, b, t: tt, d } => {
                let tau = t_reducing / t;
                let delta = rho / rhomass_reducing;
                let mut summer = 0.0;
                for i in 0..b.len() {
                    summer += b[i] * tau.powf(tt[i]) * delta.powf(d[i]);
                }
                summer
            }
            ConductivityResidual::PolynomialAndExponential { a, t: tt, d, gamma, l } => {
                let mut summer = 0.0;
                for i in 0..a.len() {
                    summer += a[i] * tau_eos.powf(tt[i]) * delta_eos.powf(d[i])
                        * (-gamma[i] * delta_eos.powf(l[i])).exp();
                }
                summer
            }
        };

        Some(lambda_dilute + lambda_residual)
    }
}

/// Dynamic viscosity `μ` \[Pa·s\] of `fluid` at temperature `t` \[K\] and mass
/// density `rho` \[kg/m³\], or [`None`] if the fluid has no supported viscosity
/// model. Critical enhancement is omitted (see module docs).
pub fn viscosity(fluid: Fluid, t: f64, rho: f64) -> Option<f64> {
    let model = fluid.transport()?.viscosity?;
    let mu = model.eval(t, rho, fluid.eos().molar_mass);
    (mu.is_finite() && mu > 0.0).then_some(mu)
}

/// Thermal conductivity `λ` \[W/(m·K)\] of `fluid` at `(T, ρ)`, or [`None`] if
/// the fluid has no supported conductivity model. Critical enhancement is
/// omitted (see module docs).
pub fn conductivity(fluid: Fluid, t: f64, rho: f64) -> Option<f64> {
    let transport = fluid.transport()?;
    let model = transport.conductivity?;
    // The eta0_and_poly dilute model needs the dilute viscosity.
    let dilute_visc = transport.viscosity.map(|v| v.dilute.eval(t));
    let lambda = model.eval(fluid, t, rho, dilute_visc)?;
    (lambda.is_finite() && lambda > 0.0).then_some(lambda)
}
