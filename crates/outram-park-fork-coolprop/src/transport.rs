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
    /// CO₂ dilute viscosity (Laesecke & Muzny, JPCRD 2017, Eq. 4) — a fixed
    /// hardcoded correlation. CoolProp `viscosity_dilute_CO2_LaeseckeJPCRD2017`.
    CO2LaeseckeJPCRD2017,
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
    /// CO₂ higher-order (residual) viscosity (Laesecke & Muzny, JPCRD 2017,
    /// Eqs. 8–9) — a fixed hardcoded correlation needing the fluid's triple
    /// temperature, gas constant and molar mass. CoolProp
    /// `viscosity_CO2_higher_order_hardcoded_LaeseckeJPCRD2017`.
    CO2LaeseckeJPCRD2017 { ttriple: f64, gas_constant: f64, molar_mass: f64 },
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
    /// CO₂ dilute conductivity (Huber et al., JPCRD 2016, Eq. 3) — a fixed
    /// hardcoded correlation in the EOS reduced temperature. CoolProp
    /// `conductivity_dilute_hardcoded_CO2_HuberJPCRD2016`.
    CO2HuberJPCRD2016,
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

/// A fluid-specific **hardcoded** transport formulation (CoolProp implements
/// these as one-off functions rather than the generic dilute+residual models).
/// When present it supplies **both** `μ` and `λ`, overriding the correlation
/// fields.
#[derive(Debug, Clone, Copy)]
pub enum HardcodedTransport {
    /// Helium-4 (Arp–McCarty–Friend viscosity, NIST TN-1334; Hands & Arp
    /// conductivity). The near-critical conductivity enhancement `λ_c`
    /// (3.5–12 K) is omitted, consistent with the rest of this crate.
    Helium,
    /// Water (IAPWS R12-08 viscosity, R15-11 conductivity). The critical
    /// enhancement (`μ̄₂` / `λ̄₂`) is omitted — it needs EOS derivatives at a
    /// reference temperature — so accuracy degrades within a few percent of the
    /// critical point.
    Water,
}

impl HardcodedTransport {
    /// Dynamic viscosity \[Pa·s\] at temperature `t` \[K\] and mass density
    /// `rho` \[kg/m³\].
    fn viscosity(&self, t: f64, rho: f64) -> f64 {
        match self {
            HardcodedTransport::Helium => helium_viscosity(t, rho),
            HardcodedTransport::Water => water_viscosity(t, rho),
        }
    }
    /// Thermal conductivity \[W/(m·K)\] at `(T, ρ)` (dilute + excess; the
    /// near-critical term is omitted).
    fn conductivity(&self, t: f64, rho: f64) -> f64 {
        match self {
            HardcodedTransport::Helium => helium_conductivity(t, rho),
            HardcodedTransport::Water => water_conductivity(t, rho),
        }
    }
}

/// Water viscosity \[Pa·s\] (IAPWS R12-08 — CoolProp `viscosity_water_hardcoded`),
/// dilute `μ̄₀` × finite-density `μ̄₁`; the critical enhancement `μ̄₂` is omitted.
fn water_viscosity(t: f64, rho: f64) -> f64 {
    let tbar = t / 647.096;
    let rhobar = rho / 322.0;
    let mubar_0 = 100.0 * tbar.sqrt()
        / (1.67752 + 2.20462 / tbar + 0.6366564 / (tbar * tbar) - 0.241605 / (tbar * tbar * tbar));
    // Sparse H[6][7] coefficient matrix (IAPWS R12-08 Table 2).
    #[rustfmt::skip]
    const H: [[f64; 7]; 6] = [
        [5.20094e-1,  2.22531e-1, -2.81378e-1,  1.61913e-1, -3.25372e-2, 0.0,        0.0        ],
        [8.50895e-2,  9.99115e-1, -9.06851e-1,  2.57399e-1,  0.0,        0.0,        0.0        ],
        [-1.08374,    1.88797,    -7.72479e-1,  0.0,         0.0,        0.0,        0.0        ],
        [-2.89555e-1, 1.26613,    -4.89837e-1,  0.0,         6.98452e-2, 0.0,       -4.35673e-3 ],
        [0.0,         0.0,        -2.57040e-1,  0.0,         0.0,        8.72102e-3, 0.0        ],
        [0.0,         1.20573e-1,  0.0,         0.0,         0.0,        0.0,       -5.93264e-4 ],
    ];
    let mut sum = 0.0;
    for (i, row) in H.iter().enumerate() {
        for (j, &h) in row.iter().enumerate() {
            sum += (1.0 / tbar - 1.0).powi(i as i32) * h * (rhobar - 1.0).powi(j as i32);
        }
    }
    let mubar_1 = (rhobar * sum).exp();
    mubar_0 * mubar_1 / 1e6
}

/// Water thermal conductivity \[W/(m·K)\] (IAPWS R15-11 — CoolProp
/// `conductivity_hardcoded_water`), dilute `λ̄₀` × finite-density `λ̄₁`; the
/// near-critical `λ̄₂` is omitted.
fn water_conductivity(t: f64, rho: f64) -> f64 {
    let tbar = t / 647.096;
    let rhobar = rho / 322.0;
    let lambdabar_0 = tbar.sqrt()
        / (2.443221e-3 + 1.323095e-2 / tbar + 6.770357e-3 / tbar.powi(2)
            - 3.454586e-3 / tbar.powi(3)
            + 4.096266e-4 / tbar.powi(4));
    #[rustfmt::skip]
    const L: [[f64; 6]; 5] = [
        [ 1.60397357, -0.646013523,  0.111443906,  0.102997357, -0.0504123634,  0.00609859258],
        [ 2.33771842, -2.78843778,   1.53616167,  -0.463045512,  0.0832827019, -0.00719201245],
        [ 2.19650529, -4.54580785,   3.55777244,  -1.40944978,   0.275418278,  -0.0205938816 ],
        [-1.21051378,  1.60812989,  -0.621178141,  0.0716373224, 0.0,           0.0          ],
        [-2.7203370,   4.57586331,  -3.18369245,   1.1168348,   -0.19268305,    0.012913842  ],
    ];
    let mut sum = 0.0;
    for (i, row) in L.iter().enumerate() {
        for (j, &l) in row.iter().enumerate() {
            sum += l * (1.0 / tbar - 1.0).powi(i as i32) * (rhobar - 1.0).powi(j as i32);
        }
    }
    let lambdabar_1 = (rhobar * sum).exp();
    lambdabar_0 * lambdabar_1 * 1e-3
}

/// Helium-4 viscosity \[Pa·s\] (Arp, McCarty & Friend, NIST TN-1334, 1998 —
/// CoolProp `viscosity_helium_hardcoded`).
fn helium_viscosity(t: f64, rho_mass: f64) -> f64 {
    let rho = rho_mass / 1000.0; // g/cm³
    let x = if t <= 300.0 { t.ln() } else { 300.0_f64.ln() };
    let b = -47.5295259 / x + 87.6799309 - 42.0741589 * x + 8.33128289 * x * x - 0.589252385 * x * x * x;
    let c = 547.309267 / x - 904.870586 + 431.404928 * x - 81.4504854 * x * x + 5.37008433 * x * x * x;
    let d = -1684.39324 / x + 3331.08630 - 1632.19172 * x + 308.804413 * x * x - 20.2936367 * x * x * x;
    let eta_0_slash =
        -0.135311743 / x + 1.00347841 + 1.20654649 * x - 0.149564551 * x * x + 0.012520841 * x * x * x;
    let eta_e_slash = rho * b + rho * rho * c + rho * rho * rho * d;
    let ln_eta = eta_0_slash + eta_e_slash;
    // Correlation is in µg/(cm·s): /10 → µPa·s, /1e6 → Pa·s.
    if t <= 100.0 {
        ln_eta.exp() / 10.0 / 1e6
    } else {
        let eta_0 = 196.0 * t.powf(0.71938) * (12.451 / t - 295.67 / t / t - 4.1249).exp();
        (ln_eta.exp() + eta_0 - eta_0_slash.exp()) / 10.0 / 1e6
    }
}

/// Helium-4 thermal conductivity \[W/(m·K)\] (Hands & Arp — CoolProp
/// `conductivity_hardcoded_helium`), dilute `λ₀` + excess `λ_e`; the
/// near-critical `λ_c` (3.5–12 K) is omitted.
fn helium_conductivity(t: f64, rho: f64) -> f64 {
    let rhoc = 68.0;
    let summer = 3.739232544 / t - 2.620316969e1 / t / t + 5.982252246e1 / t / t / t
        - 4.926397634e1 / t / t / t / t;
    let lambda_0 = 2.787_003_4e-3 * t.powf(7.034007057e-1) * summer.exp();
    let c = [
        1.862970530e-4, -7.275964435e-7, -1.427549651e-4, 3.290833592e-5, -5.213335363e-8,
        4.492659933e-8, -5.924416513e-9, 7.087321137e-6, -6.013335678e-6, 8.067145814e-7,
        3.995125013e-7,
    ];
    let t13 = t.powf(1.0 / 3.0);
    let t23 = t.powf(2.0 / 3.0);
    let lambda_e = (c[0] + c[1] * t + c[2] * t13 + c[3] * t23) * rho
        + (c[4] + c[5] * t13 + c[6] * t23) * rho * rho * rho
        + (c[7] + c[8] * t13 + c[9] * t23 + c[10] / t) * rho * rho * (rho / rhoc).ln();
    lambda_0 + lambda_e
}

/// The transport models a fluid carries. The correlation fields may each be
/// `None` (unimplemented model or no data); a `Some(hardcoded)` supersedes them
/// with a fluid-specific formula for **both** `μ` and `λ`.
#[derive(Debug, Clone, Copy)]
pub struct FluidTransport {
    pub viscosity: Option<ViscosityModel>,
    pub conductivity: Option<ConductivityModel>,
    pub hardcoded: Option<HardcodedTransport>,
}

impl ViscosityDilute {
    /// Dilute-gas viscosity \[Pa·s\] at temperature `t` \[K\].
    // The CO2 Laesecke coefficients are transcribed verbatim from CoolProp.
    #[allow(clippy::excessive_precision)]
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
            ViscosityDilute::CO2LaeseckeJPCRD2017 => {
                let a = [
                    1749.354893188350, -369.069300007128, 5423856.34887691, -2.21283852168356,
                    -269503.247933569, 73145.021531826, 5.34368649509278,
                ];
                let t3 = t.powf(1.0 / 3.0);
                let den = a[0] + a[1] * t.powf(1.0 / 6.0) + a[2] * (a[3] * t3).exp()
                    + (a[4] + a[5] * t3) / t3.exp()
                    + a[6] * t.sqrt();
                0.0010055 * t.sqrt() / den
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
            ViscosityHigherOrder::CO2LaeseckeJPCRD2017 { ttriple, gas_constant, molar_mass } => {
                let (c1, c2, gamma) = (0.360603235428487, 0.121550806591497, 8.06282737481277);
                let rho_tl = 1178.53;
                let rho_mass = rho_molar * molar_mass;
                let tr = t / ttriple;
                let rhor = rho_mass / rho_tl;
                let eta_tl = rho_tl.powf(2.0 / 3.0) * (gas_constant * ttriple).sqrt()
                    / (molar_mass.powf(1.0 / 6.0) * 84446887.43579945);
                eta_tl * (c1 * tr * rhor.powi(3) + (rhor.powi(2) + rhor.powf(gamma)) / (tr - c2))
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
            ConductivityDilute::CO2HuberJPCRD2016 => {
                let l = [0.0151874307, 0.0280674040, 0.0228564190, -0.00741624210];
                let lambda_0 = tau_eos.powf(-0.5)
                    / (l[0] + l[1] * tau_eos + l[2] * tau_eos.powi(2) + l[3] * tau_eos.powi(3));
                lambda_0 / 1000.0 // mW/m/K -> W/m/K
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
    let transport = fluid.transport()?;
    let mu = match transport.hardcoded {
        Some(hc) => hc.viscosity(t, rho),
        None => transport.viscosity?.eval(t, rho, fluid.eos().molar_mass),
    };
    (mu.is_finite() && mu > 0.0).then_some(mu)
}

/// Thermal conductivity `λ` \[W/(m·K)\] of `fluid` at `(T, ρ)`, or [`None`] if
/// the fluid has no supported conductivity model. Critical enhancement is
/// omitted (see module docs).
pub fn conductivity(fluid: Fluid, t: f64, rho: f64) -> Option<f64> {
    let transport = fluid.transport()?;
    let lambda = match transport.hardcoded {
        Some(hc) => hc.conductivity(t, rho),
        None => {
            let model = transport.conductivity?;
            // The eta0_and_poly dilute model needs the dilute viscosity.
            let dilute_visc = transport.viscosity.map(|v| v.dilute.eval(t));
            model.eval(fluid, t, rho, dilute_visc)?
        }
    };
    (lambda.is_finite() && lambda > 0.0).then_some(lambda)
}
