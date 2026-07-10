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
    /// Ethane dilute viscosity (Friend et al., hardcoded).
    Ethane,
    /// Cyclohexane dilute viscosity (Tariq, JPCRD 2014, hardcoded).
    Cyclohexane,
    /// `η₀ = Σ aᵢ·(T/T_reducing)^{tᵢ}` \[Pa·s\]. CoolProp `powers_of_Tr`.
    PowersOfTr { a: &'static [f64], t: &'static [f64], t_reducing: f64 },
    /// `η₀ = C·√T / Σ aᵢ·(T/T_reducing)^{tᵢ}`. CoolProp
    /// `collision_integral_powers_of_Tstar`.
    CollisionIntegralPowersOfTstar { c: f64, a: &'static [f64], t: &'static [f64], t_reducing: f64 },
    /// Neufeld/kinetic-theory Lennard-Jones collision integral \[Pa·s\].
    /// CoolProp `kinetic_theory`.
    KineticTheory { molar_mass: f64, epsilon_over_k: f64, sigma_eta: f64 },
}

/// Initial-density viscosity correction.
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
    /// Empirical `Σ nᵢ·δ^{dᵢ}·τ^{tᵢ}` \[Pa·s\] directly (Tariq form), `τ =
    /// T_reducing/T`, `δ = ρ_molar/ρ_molar_reducing`. CoolProp `empirical`.
    Empirical {
        n: &'static [f64],
        d: &'static [f64],
        t: &'static [f64],
        t_reducing: f64,
        rhomolar_reducing: f64,
    },
}

impl ViscosityInitial {
    /// The initial-density viscosity contribution \[Pa·s\] at temperature `t`
    /// \[K\], molar density `rho_molar` \[mol/m³\], with the dilute viscosity
    /// `eta_dilute` \[Pa·s\].
    fn contribution(&self, t: f64, rho_molar: f64, eta_dilute: f64) -> f64 {
        match self {
            ViscosityInitial::RainwaterFriend { b, t: tt, epsilon_over_k, sigma_eta } => {
                let tstar = t / epsilon_over_k;
                let mut s = 0.0;
                for i in 0..b.len() {
                    s += b[i] * tstar.powf(tt[i]);
                }
                let b_eta = 6.022_141_29e23 * sigma_eta.powi(3) * s; // m³/mol
                eta_dilute * b_eta * rho_molar
            }
            ViscosityInitial::Empirical { n, d, t: tt, t_reducing, rhomolar_reducing } => {
                let tau = t_reducing / t;
                let delta = rho_molar / rhomolar_reducing;
                let mut summer = 0.0;
                for i in 0..n.len() {
                    summer += n[i] * delta.powf(d[i]) * tau.powf(tt[i]);
                }
                summer
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
    /// Hardcoded higher-order viscosity for a specific fluid (hydrogen,
    /// benzene, toluene, n-hexane, n-heptane, ethane). `molar_mass` converts the
    /// EOS molar density to the mass density the correlation uses.
    Hydrogen { molar_mass: f64 },
    Toluene { molar_mass: f64 },
    Benzene { molar_mass: f64 },
    Hexane { molar_mass: f64 },
    Heptane { molar_mass: f64 },
    Ethane,
    /// Quiñones-Cisneros & Deiters **friction theory** (CoolProp
    /// `friction_theory`). Density enters through the pressure and `∂p/∂T`, so
    /// this needs the EOS. Optional terms are empty slices. `Arr` xor `Adrdr`;
    /// `Aii`, `Arrr`/`Aaaa` present only for some fluids.
    FrictionTheory {
        t_reduce: f64,
        c1: f64,
        c2: f64,
        ai: &'static [f64],
        aa: &'static [f64],
        ar: &'static [f64],
        aaa: &'static [f64],
        arr: &'static [f64],
        adrdr: &'static [f64],
        aii: &'static [f64],
        arrr: &'static [f64],
        aaaa: &'static [f64],
        na: f64,
        nr: f64,
        naa: f64,
        nrr: f64,
        nii: f64,
        nrrr: f64,
        naaa: f64,
    },
}

/// A complete viscosity model (Pa·s): either the generic dilute + (optional)
/// initial-density + higher-order composition, or a fluid-specific hardcoded
/// formula. (The variants differ in size; these are `&'static` const data, so
/// the size difference is immaterial.)
#[derive(Debug, Clone, Copy)]
#[allow(clippy::large_enum_variant)]
pub enum ViscosityModel {
    /// Dilute + initial-density + higher-order correlation.
    Correlation {
        dilute: ViscosityDilute,
        initial: Option<ViscosityInitial>,
        higher_order: ViscosityHigherOrder,
    },
    /// A whole-fluid hardcoded formula (Water, HeavyWater, the xylenes, …).
    Hardcoded(HardcodedViscosity),
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
    /// Ethane dilute conductivity (hardcoded; needs the ethane dilute viscosity
    /// and the ideal-gas `∂²α⁰/∂τ²`). CoolProp `conductivity_dilute_hardcoded_ethane`.
    EthaneHardcoded,
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

/// Near-critical conductivity enhancement `λ_c` \[W/(m·K)\].
#[derive(Debug, Clone, Copy)]
pub enum CriticalConductivity {
    /// The simplified Olchowy–Sengers crossover (CoolProp
    /// `conductivity_critical_simplified_Olchowy_Sengers`). Needs `c_p`,`c_v`,
    /// the viscosity, and the EOS `δ`-derivatives at `T` and a reference `T`.
    SimplifiedOlchowySengers {
        r0: f64,
        gamma: f64,
        big_gamma: f64,
        zeta0: f64,
        qd: f64,
        /// Reference temperature `T_ref` \[K\]; when `≤ 0`, use `1.5·T_c`.
        /// (The critical exponent `ν = 0.63` and `k_B` are constants.)
        t_ref: f64,
    },
    /// R123 critical term (CoolProp `conductivity_critical_hardcoded_R123`).
    R123,
    /// Ammonia critical term (Tufeu et al. — CoolProp
    /// `conductivity_critical_hardcoded_ammonia`).
    Ammonia,
}

/// A complete conductivity model (W/(m·K)): the generic dilute + residual (+
/// optional near-critical enhancement) composition, or a hardcoded formula.
#[derive(Debug, Clone, Copy)]
pub enum ConductivityModel {
    /// Dilute + residual + optional critical enhancement.
    Correlation {
        dilute: ConductivityDilute,
        residual: ConductivityResidual,
        critical: Option<CriticalConductivity>,
    },
    /// A whole-fluid hardcoded formula.
    Hardcoded(HardcodedConductivity),
}

/// A fluid-specific **hardcoded viscosity** formula (CoolProp implements these
/// as one-off functions rather than the generic dilute+higher-order model).
#[derive(Debug, Clone, Copy)]
pub enum HardcodedViscosity {
    /// Helium-4 (Arp–McCarty–Friend, NIST TN-1334).
    Helium,
    /// Water (IAPWS R12-08); the critical enhancement `μ̄₂` is omitted.
    Water,
    /// Heavy water / D₂O (IAPWS 1994).
    HeavyWater,
    /// o-Xylene (Cao, JPCRD 2016). Needs the fluid's molar mass for `ρ_molar`.
    OXylene { molar_mass: f64 },
    /// m-Xylene (Cao, JPCRD 2016).
    MXylene { molar_mass: f64 },
    /// p-Xylene (Balogun, JPCRD 2016).
    PXylene { molar_mass: f64 },
    /// R23 / trifluoromethane (a friction-theory-like hardcoded form).
    R23 { molar_mass: f64 },
    /// Methanol (Chung-style hardcoded form, needs the molar mass).
    Methanol { molar_mass: f64 },
}

impl HardcodedViscosity {
    /// Dynamic viscosity \[Pa·s\] at `(T, ρ)`.
    fn eval(&self, t: f64, rho: f64) -> f64 {
        match self {
            HardcodedViscosity::Helium => helium_viscosity(t, rho),
            HardcodedViscosity::Water => water_viscosity(t, rho),
            HardcodedViscosity::HeavyWater => heavywater_viscosity(t, rho),
            HardcodedViscosity::OXylene { molar_mass } => {
                xylene_viscosity(t, rho / molar_mass, 0.22225, 630.259, 2.6845,
                    &[-2.05581e-3, 2.38762, 0.0, 10.4497, 15.9587],
                    &[10.3, 3.3, 25.0, 0.7, 0.4],
                    &[2.65651e-3, 0.0, 1.77616e-12, -18.2446, 0.0], &[0.8, 0.0, 4.4])
            }
            HardcodedViscosity::MXylene { molar_mass } => {
                xylene_viscosity(t, rho / molar_mass, 0.22115, 616.89, 2.665,
                    &[-0.268950, -0.0290018, 0.0, 14.7728, 17.1128],
                    &[6.8, 3.3, 22.0, 0.6, 0.4],
                    &[0.320971, 0.0, 1.72866e-10, -18.9852, 0.0], &[0.3, 0.0, 3.2])
            }
            HardcodedViscosity::PXylene { molar_mass } => pxylene_viscosity(t, rho / molar_mass),
            HardcodedViscosity::R23 { molar_mass } => r23_viscosity(t, rho / molar_mass),
            HardcodedViscosity::Methanol { molar_mass } => methanol_viscosity(t, rho, rho / molar_mass),
        }
    }
}

/// Methanol viscosity \[Pa·s\] (Xiang et al. — CoolProp `viscosity_methanol_hardcoded`),
/// a Chung/hard-sphere blend. `rho` mass \[kg/m³\], `rhomolar` \[mol/m³\].
#[allow(clippy::excessive_precision)]
fn methanol_viscosity(t: f64, rho: f64, rhomolar: f64) -> f64 {
    let (epsilon_over_k, sigma0, delta, n_a, m): (f64, f64, f64, f64, f64) =
        (577.87, 0.3408e-9, 0.4575, 6.02214129e23, 32.04216);
    let tstar = t / epsilon_over_k;
    let rhor = rho / 273.0;
    let tr = t / 512.6;
    // Rainwater–Friend initial density (B_eta) and second (C_eta) coefficients.
    let b = [-19.572881, 219.73999, -1015.3226, 2471.01251, -3375.1717, 2491.6597, -787.26086, 14.085455, -0.34664158];
    let bt = [0.0, -0.25, -0.5, -0.75, -1.0, -1.25, -1.5, -2.5, -5.5];
    let b_eta_star: f64 = (0..9).map(|i| b[i] * tstar.powf(bt[i])).sum();
    let b_eta = n_a * sigma0.powi(3) * b_eta_star; // m³/mol
    let cc = [1.86222085e-3, 9.990338];
    let c_eta_star = cc[0] * tstar.powi(3) * (cc[1] * tstar.powf(-0.5)).exp();
    let c_eta = (n_a * sigma0.powi(3)).powi(2) * c_eta_star; // m⁶/mol²
    let eta_g = 1.0 + b_eta * rhomolar + c_eta * rhomolar * rhomolar;
    let a = [1.16145, -0.14874, 0.52487, -0.77320, 2.16178, -2.43787, 0.95976e-3,
        0.10225, -0.97346, 0.10657, -0.34528, -0.44557, -2.58055];
    let d = [-1.181909, 0.5031030, -0.6268461, 0.5169312, -0.2351349, 5.3980235e-2, -4.9069617e-3];
    let e = [0.0, 4.018368, -4.239180, 2.245110, -0.5750698, 2.3021026e-2, 2.5696775e-2, -6.8372749e-3, 7.2707189e-4, -2.9255711e-5];
    let omega_lj = a[0] * tstar.powf(a[1]) + a[2] * (a[3] * tstar).exp() + a[4] * (a[5] * tstar).exp();
    let omega_delta = a[7] * tstar.powf(a[8]) + a[9] * (a[10] * tstar).exp() + a[11] * (a[12] * tstar).exp();
    let omega_sm = omega_lj * (1.0 + delta.powi(2) / (1.0 + a[6] * delta.powi(6)) * omega_delta);
    let eta_0 = 2.66957e-26 * (m * t).sqrt() / (sigma0.powi(2) * omega_sm);
    let mut summerd: f64 = (0..7).map(|i| d[i] / tr.powi(i as i32)).sum();
    summerd += (1..10).map(|j| e[j] * rhor.powi(j as i32)).sum::<f64>();
    let sigmac = 0.7193422e-9;
    let sigma_hs = summerd * sigmac;
    let b_hs = 2.0 * std::f64::consts::PI * n_a * sigma_hs.powi(3) / 3.0; // m³/mol
    let zeta = b_hs * rhomolar / 4.0;
    let g_sigma_hs = (1.0 - 0.5 * zeta) / (1.0 - zeta).powi(3);
    let eta_e = 1.0 / g_sigma_hs + 0.8 * b_hs * rhomolar + 0.761 * g_sigma_hs * (b_hs * rhomolar).powi(2);
    let f = 1.0 / (1.0 + (5.0 * (rhor - 1.0)).exp());
    eta_0 * (f * eta_g + (1.0 - f) * eta_e)
}

/// Heavy-water viscosity \[Pa·s\] (IAPWS 1994 — CoolProp `viscosity_heavywater_hardcoded`).
fn heavywater_viscosity(t: f64, rho: f64) -> f64 {
    let tbar = t / 643.847;
    let rhobar = rho / 358.0;
    let a = [1.000000, 0.940695, 0.578377, -0.202044];
    #[rustfmt::skip]
    let ii = [0usize,1,2,3,4,5, 0,1,2,3, 0,1,2,5, 0,1,2,3, 0,1,3,5, 0,1,5,3];
    #[rustfmt::skip]
    let jj = [0usize,0,0,0,0,0, 1,1,1,1, 2,2,2,2, 3,3,3,3, 4,4,4,4, 5,5,5,6];
    #[rustfmt::skip]
    let bij = [0.4864192,-0.2448372,-0.8702035,0.8716056,-1.051126,0.3458395, 0.3509007,1.315436,1.297752,1.353448,
               -0.2847572,-1.037026,-1.287846,-0.02148229, 0.07013759,0.4660127,0.2292075,-0.4857462,
               0.01641220,-0.02884911,0.1607171,-0.009603846, -0.01163815,-0.008239587,0.004559914,-0.003886659];
    let mu0 = tbar.sqrt() / (a[0] + a[1] / tbar + a[2] / (tbar * tbar) + a[3] / (tbar * tbar * tbar));
    let mut summer = 0.0;
    for i in 0..26 {
        summer += bij[i] * (1.0 / tbar - 1.0).powi(ii[i] as i32) * (rhobar - 1.0).powi(jj[i] as i32);
    }
    let mu1 = (rhobar * summer).exp();
    55.2651e-6 * mu0 * mu1
}

/// o/m-Xylene viscosity \[Pa·s\] (Cao, JPCRD 2016), shared form. `rho_molar` in
/// mol/m³; `tc`,`rhoc_molL` reduce it; `s0_pref` scales `η₀`.
#[allow(clippy::too_many_arguments)]
fn xylene_viscosity(t: f64, rho_molar: f64, s0_pref: f64, tc: f64, rhoc_kmolm3: f64,
                    dd: &[f64], n: &[f64], ee: &[f64], k: &[f64]) -> f64 {
    let tr = t / tc;
    let rhor = rho_molar / 1000.0 / rhoc_kmolm3;
    let ln_seta = -1.4933 + 473.2 / t - 57033.0 / (t * t);
    let eta0 = s0_pref * t.sqrt() / ln_seta.exp(); // µPa·s
    let rho_moll = rho_molar / 1000.0;
    let eta1 = (13.2814 - 10862.4 / t + 1664060.0 / (t * t)) * rho_moll; // µPa·s
    let f = (dd[0] + ee[0] * tr.powf(-k[0])) * rhor.powf(n[0]) + dd[1] * rhor.powf(n[1])
        + ee[2] * rhor.powf(n[2]) / tr.powf(k[2])
        + (dd[3] * rhor + ee[3] * tr) * rhor.powf(n[3])
        + dd[4] * rhor.powf(n[4]);
    let delta_eta = rhor.powf(2.0 / 3.0) * tr.sqrt() * f; // µPa·s
    (eta0 + eta1 + delta_eta) / 1e6
}

/// p-Xylene viscosity \[Pa·s\] (Balogun, JPCRD 2016). `rho_molar` in mol/m³.
fn pxylene_viscosity(t: f64, rho_molar: f64) -> f64 {
    let tr = t / 616.168;
    let rhor = rho_molar / 1000.0 / 2.69392;
    let ln_seta = -1.4933 + 473.2 / t - 57033.0 / (t * t);
    let eta0 = 0.22005 * t.sqrt() / ln_seta.exp();
    let rho_moll = rho_molar / 1000.0;
    let eta1 = (13.2814 - 10862.4 / t + 1664060.0 / (t * t)) * rho_moll;
    let sum1 = 122.919 * rhor.powf(1.5) - 282.329 * rhor.powi(2) + 279.348 * rhor.powi(3)
        - 146.776 * rhor.powi(4) + 28.361 * rhor.powi(5) - 0.004585 * rhor.powi(11);
    let sum2 = 15.337 * rhor.powf(1.5) - 0.0004382 * rhor.powi(11) + 0.00002307 * rhor.powi(15);
    let delta_eta = rhor.powf(2.0 / 3.0) * (sum1 + 1.0 / tr.sqrt() * sum2);
    (eta0 + eta1 + delta_eta) / 1e6
}

/// R23 viscosity \[Pa·s\] (Shan et al. — CoolProp `viscosity_R23_hardcoded`).
/// `rho_molar` in mol/m³.
fn r23_viscosity(t: f64, rho_molar: f64) -> f64 {
    let (c1, c2, delta_gstar, rho_l, rhocbar, delta_eta_max, ru, tc, molar_mass) =
        (1.3163, 0.1832, 771.23, 32.174, 7.5114, 3.967, 8.31451, 299.2793, 70.014);
    let a = [0.4425728, -0.5138403, 0.1547566, -0.02821844, 0.001578286];
    let (e_k, sigma) = (243.91, 0.4278);
    let tstar = t / e_k;
    let log_tstar = tstar.ln();
    let omega = (a[0] + a[1] * log_tstar + a[2] * log_tstar.powi(2) + a[3] * log_tstar.powi(3)
        + a[4] * log_tstar.powi(4))
    .exp();
    let eta_dg = 1.25 * 0.021357 * (molar_mass * t).sqrt() / (sigma * sigma * omega); // µPa·s
    let rhobar = rho_molar / 1000.0; // mol/L
    let eta_l = c2 * (rho_l * rho_l) / (rho_l - rhobar) * t.sqrt()
        * (rhobar / (rho_l - rhobar) * delta_gstar / (ru * t)).exp();
    let chi = rhobar - rhocbar;
    let tau = t - tc;
    let delta_eta_c = 4.0 * delta_eta_max / ((chi.exp() + (-chi).exp()) * (tau.exp() + (-tau).exp()));
    (((rho_l - rhobar) / rho_l).powf(c1) * eta_dg + (rhobar / rho_l).powf(c1) * eta_l + delta_eta_c) / 1e6
}

/// A fluid-specific **hardcoded conductivity** formula.
#[derive(Debug, Clone, Copy)]
pub enum HardcodedConductivity {
    /// Helium-4 (Hands & Arp); the near-critical `λ_c` (3.5–12 K) is omitted.
    Helium,
    /// Water (IAPWS R15-11); the near-critical `λ̄₂` is omitted.
    Water,
    /// Heavy water / D₂O (IAPWS 1994), including its (self-contained) critical term.
    HeavyWater,
    /// R23 / trifluoromethane. `rho_molar` from the fluid's molar mass.
    R23 { molar_mass: f64 },
    /// Methane (Friend et al., JPCRD 1989) — dilute + residual + full critical
    /// region. Needs the EOS (ideal `∂²α⁰/∂τ²`, residual `δ`-derivatives, and
    /// the saturated-liquid density).
    Methane,
}

impl HardcodedConductivity {
    /// Thermal conductivity \[W/(m·K)\] of `fluid` at `(T, ρ)`.
    fn eval(&self, fluid: Fluid, t: f64, rho: f64) -> f64 {
        match self {
            HardcodedConductivity::Helium => helium_conductivity(t, rho),
            HardcodedConductivity::Water => water_conductivity(t, rho),
            HardcodedConductivity::HeavyWater => heavywater_conductivity(t, rho),
            HardcodedConductivity::R23 { molar_mass } => r23_conductivity(t, rho / molar_mass),
            HardcodedConductivity::Methane => methane_conductivity(fluid, t, rho),
        }
    }
}

/// Methane thermal conductivity \[W/(m·K)\] (Friend, Ely & Ingham, JPCRD 1989 —
/// CoolProp `conductivity_hardcoded_methane`), dilute + residual + critical.
fn methane_conductivity(fluid: Fluid, t: f64, rho: f64) -> f64 {
    let eos = fluid.eos();
    let rhomolar = rho / eos.molar_mass;
    let delta = rhomolar / 10139.0;
    let tau = 190.55 / t;
    let tau_eos = eos.t_reducing / t;
    let res = eos.residual_derivs(rhomolar / eos.rho_reducing, tau_eos);
    let att = eos.ideal_derivs(rhomolar / eos.rho_reducing, tau_eos).att;

    // Dilute viscosity (Friend), needed by lambda_dilute and lambda_critical.
    let cc = [0.0, -3.0328138281, 16.918880086, -37.189364917, 41.288861858,
        -24.615921140, 8.9488430959, -1.8739245042, 0.20966101390, -9.6570437074e-3];
    let tv = t / 174.0;
    let omega22: f64 = (1..=9).map(|i| cc[i] * tv.powf((i as f64 - 1.0) / 3.0 - 1.0)).sum();
    let eta_dilute = 10.50 * tv.sqrt() * omega22;
    let re = [0usize, 1, 1, 2, 2, 2, 3, 3, 4, 4, 1, 1];
    let se = [0.0, 0.0, 1.0, 0.0, 1.0, 1.5, 0.0, 2.0, 0.0, 1.0, 0.0, 1.0];
    let ge = [0.0, 0.41250137, -0.14390912, 0.10366993, 0.40287464, -0.24903524,
        -0.12953131, 0.06575776, 0.02566628, -0.03716526, -0.38798341, 0.03533815];
    let et = |i: usize| ge[i] * delta.powi(re[i] as i32) * tau.powf(se[i]);
    let eta_residual = 12.149 * (1..=9).map(et).sum::<f64>() / (1.0 + (10..=11).map(et).sum::<f64>());
    let eta = eta_residual + eta_dilute;

    // Dilute conductivity.
    let f_int = 1.458850 - 0.4377162 / tv;
    let lambda_dilute = 0.51828 * eta_dilute * (3.75 - f_int * (tau_eos * tau_eos * att + 1.5));

    // Residual conductivity (uses the saturated-liquid density near the dome).
    let rl = [0usize, 1, 3, 4, 4, 5, 5, 2];
    let sl = [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0];
    let jl = [0.0, 2.4149207, 0.55166331, -0.52837734, 0.073809553, 0.24465507, -0.047613626, 1.5554612];
    let summer: f64 = (1..=6).map(|i| jl[i] * delta.powi(rl[i] as i32) * tau.powf(sl[i])).sum();
    let mut delta_sigma_star = 1.0;
    if t < eos.t_critical && rhomolar < eos.rho_critical {
        if let Some(rho_l) = crate::ancillaries::saturated_liquid_density_ancillary(fluid, t) {
            delta_sigma_star = (rho_l / eos.molar_mass) / eos.rho_critical;
        }
    }
    let lambda_residual = 6.29638 * (summer + jl[7] * delta.powi(2) / delta_sigma_star);

    // Critical region.
    let tstar = 1.0 - 1.0 / tau;
    let rhostar = 1.0 - delta;
    let (f_t, f_rho, f_a) = (2.646, 2.678, -0.637);
    let f = (-f_t * tstar.abs().sqrt() - f_rho * rhostar.powi(2) - f_a * rhostar).exp();
    let chi_t_star = if tstar.abs() < 0.03 {
        if rhostar.abs() < 1e-16 {
            let (lambda_c, gamma_c) = (0.0801, 1.190);
            lambda_c * tstar.abs().powf(-gamma_c)
        } else if rhostar.abs() < 0.03 {
            let (beta, w, s, e, a, b, r, q) = (0.355, -1.401, -6.098, 0.287, 3.352, 0.732, 0.535, 0.1133);
            let omega = w * tstar * rhostar.abs().powf(-1.0 / beta);
            let mut theta = 1.0;
            if tstar < -rhostar.abs().powf(-1.0 / beta) / s {
                theta = 1.0 + e * (1.0 + s * tstar * rhostar.abs().powf(-1.0 / beta)).powf(2.0 * beta);
            }
            q * rhostar.abs().powf(-a) * theta.powf(b) / (theta + omega * (theta + r))
        } else {
            0.28631 * delta * tau / (1.0 + 2.0 * delta * res.ad + delta.powi(2) * res.add)
        }
    } else {
        0.28631 * delta * tau / (1.0 + 2.0 * delta * res.ad + delta.powi(2) * res.add)
    };
    let lambda_critical = 91.855 / (eta * tau.powi(2))
        * (1.0 + delta * res.ad - delta * tau * res.adt).powi(2)
        * chi_t_star.powf(0.4681)
        * f;

    (lambda_dilute + lambda_residual + lambda_critical) * 0.001
}

/// Heavy-water conductivity \[W/(m·K)\] (IAPWS 1994 — CoolProp
/// `conductivity_hardcoded_heavywater`), including its self-contained critical term.
fn heavywater_conductivity(t: f64, rho: f64) -> f64 {
    let tbar = t / 643.847;
    let rhobar = rho / 358.0;
    let a = [1.00000, 37.3223, 22.5485, 13.0465, 0.0, -2.60735];
    let lambda0 = a[0] + a[1] * tbar + a[2] * tbar.powi(2) + a[3] * tbar.powi(3)
        + a[4] * tbar.powi(4) + a[5] * tbar.powi(5);
    let be = -2.506;
    let b = [-167.310, 483.656, -191.039, 73.0358, -7.57467];
    let delta_lambda = b[0] * (1.0 - (be * rhobar).exp()) + b[1] * rhobar + b[2] * rhobar.powi(2)
        + b[3] * rhobar.powi(3) + b[4] * rhobar.powi(4);
    let f_1 = (0.144847 * tbar - 5.64493 * tbar.powi(2)).exp();
    let f_2 = (-2.80000 * (rhobar - 1.0).powi(2)).exp()
        - 0.080738543 * (-17.9430 * (rhobar - 0.125698).powi(2)).exp();
    let tau = tbar / ((tbar - 1.1).abs() + 1.1);
    let f_3 = 1.0 + (60.0 * (tau - 1.0) + 20.0).exp();
    let f_4 = 1.0 + (100.0 * (tau - 1.0) + 15.0).exp();
    let delta_lambda_c = 35429.6 * f_1 * f_2
        * (1.0 + f_2.powi(2) * (5000.0e6 * f_1.powi(4) / f_3 + 3.5 * f_2 / f_4));
    let delta_lambda_l = -741.112 * f_1.powf(1.2) * (1.0 - (-(rhobar / 2.5).powi(10)).exp());
    (lambda0 + delta_lambda + delta_lambda_c + delta_lambda_l) * 0.742128e-3
}

/// R23 conductivity \[W/(m·K)\] (CoolProp `conductivity_hardcoded_R23`).
/// `rho_molar` in mol/m³.
fn r23_conductivity(t: f64, rho_molar: f64) -> f64 {
    let (b1, b2, c1, c2, delta_gstar, rho_l, rhocbar, delta_lambda_max, ru, tc) =
        (-2.5370, 0.05366, 0.94215, 0.14914, 2508.58, 68.345, 7.5114, 25.0, 8.31451, 299.2793);
    let lambda_dg = b1 + b2 * t; // mW/m/K
    let rhobar = rho_molar / 1000.0; // mol/L
    let lambda_l = c2 * (rho_l * rho_l) / (rho_l - rhobar) * t.sqrt()
        * (rhobar / (rho_l - rhobar) * delta_gstar / (ru * t)).exp();
    let chi = rhobar - rhocbar;
    let tau = t - tc;
    let delta_lambda_c =
        4.0 * delta_lambda_max / ((chi.exp() + (-chi).exp()) * (tau.exp() + (-tau).exp()));
    (((rho_l - rhobar) / rho_l).powf(c1) * lambda_dg + (rhobar / rho_l).powf(c1) * lambda_l
        + delta_lambda_c)
        / 1e3
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

/// The transport models a fluid carries. Either field is `None` when the fluid
/// ships no supported model for that property.
#[derive(Debug, Clone, Copy)]
pub struct FluidTransport {
    pub viscosity: Option<ViscosityModel>,
    pub conductivity: Option<ConductivityModel>,
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
            ViscosityDilute::Ethane => {
                let c = [0.0, -3.0328138281, 16.918880086, -37.189364917, 41.288861858,
                    -24.615921140, 8.9488430959, -1.8739245042, 0.20966101390, -9.6570437074e-3];
                let tstar = t / 245.0;
                let omega: f64 = (1..=9).map(|i| c[i] * tstar.powf((i as f64 - 1.0) / 3.0 - 1.0)).sum();
                12.0085 * tstar.sqrt() * omega / 1e6
            }
            ViscosityDilute::Cyclohexane => {
                let s_eta = (-1.5093 + 364.87 / t - 39537.0 / (t * t)).exp();
                0.19592 * t.sqrt() / s_eta / 1e6
            }
            ViscosityDilute::PowersOfTr { a, t: tt, t_reducing } => {
                let tr = t / t_reducing;
                (0..a.len()).map(|i| a[i] * tr.powf(tt[i])).sum()
            }
            ViscosityDilute::CollisionIntegralPowersOfTstar { c, a, t: tt, t_reducing } => {
                let tstar = t / t_reducing;
                let summer: f64 = (0..a.len()).map(|i| a[i] * tstar.powf(tt[i])).sum();
                c * t.sqrt() / summer
            }
            ViscosityDilute::KineticTheory { molar_mass, epsilon_over_k, sigma_eta } => {
                let tstar = t / epsilon_over_k;
                let sigma_nm = sigma_eta * 1e9;
                let mm_kgkmol = molar_mass * 1000.0;
                let omega22 = 1.16145 * tstar.powf(-0.14874) + 0.52487 * (-0.77320 * tstar).exp()
                    + 2.16178 * (-2.43787 * tstar).exp();
                26.692e-9 * (mm_kgkmol * t).sqrt() / (sigma_nm * sigma_nm * omega22)
            }
        }
    }
}

impl ViscosityHigherOrder {
    /// Higher-order viscosity \[Pa·s\] at molar density `rho_molar` \[mol/m³\]
    /// and temperature `t` \[K\]. `fluid` supplies the EOS (needed by friction
    /// theory for the pressure and `∂p/∂T`).
    fn eval(&self, fluid: Fluid, rho_molar: f64, t: f64) -> f64 {
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
            ViscosityHigherOrder::Hydrogen { molar_mass } => {
                let tr = t / 33.145;
                let rhor = rho_molar * molar_mass * 0.011;
                let c = [0.0, 6.43449673e-6, 4.56334068e-2, 2.32797868e-1, 9.58326120e-1, 1.27941189e-1, 3.63576595e-1];
                c[1] * rhor.powi(2)
                    * (c[2] * tr + c[3] / tr + c[4] * rhor.powi(2) / (c[5] + tr) + c[6] * rhor.powi(6)).exp()
            }
            ViscosityHigherOrder::Toluene { molar_mass } => {
                let tr = t / 591.75;
                let rhor = rho_molar * molar_mass / 291.987;
                let c = [19.919216, -2.6557905, -135.904211, -7.9962719, -11.014795, -10.113817];
                1e-6 * rhor.powf(2.0 / 3.0) * tr.sqrt()
                    * ((c[0] * rhor + c[1] * rhor.powi(4)) / tr
                        + c[2] * rhor.powi(3) / (rhor * rhor + c[3] + c[4] * tr)
                        + c[5] * rhor)
            }
            ViscosityHigherOrder::Benzene { molar_mass } => {
                let tr = t / 562.02;
                let rhor = rho_molar * molar_mass / 304.792;
                let c = [-9.98945, 86.06260, 2.74872, 1.11130, -1.0, -134.1330, -352.473, 6.60989, 88.4174];
                1e-6 * rhor.powf(2.0 / 3.0) * tr.sqrt()
                    * (c[0] * rhor.powi(2) + c[1] * rhor / (c[2] + c[3] * tr + c[4] * rhor)
                        + (c[5] * rhor + c[6] * rhor.powi(2)) / (c[7] + c[8] * rhor.powi(2)))
            }
            ViscosityHigherOrder::Hexane { molar_mass } => {
                let tr = t / 507.82;
                let rhor = rho_molar * molar_mass / 233.182;
                let c = [2.53402335 / 1e6, -9.724061002 / 1e6, 0.469437316, 158.5571631,
                    72.42916856 / 1e6, 10.60751253, 8.628373915, -6.61346441, -2.212724566];
                rhor.powf(2.0 / 3.0) * tr.sqrt()
                    * (c[0] / tr + c[1] / (c[2] + tr + c[3] * rhor * rhor)
                        + c[4] * (1.0 + rhor) / (c[5] + c[6] * tr + c[7] * rhor + rhor * rhor + c[8] * rhor * tr))
            }
            ViscosityHigherOrder::Heptane { molar_mass } => {
                let tr = t / 540.13;
                let rhor = rho_molar * molar_mass / 232.0;
                let c = [0.0, 22.15000 / 1e6, -15.00870 / 1e6, 3.71791 / 1e6, 77.72818 / 1e6,
                    9.73449, 9.51900, -6.34076, -2.51909];
                rhor.powf(2.0 / 3.0) * tr.sqrt()
                    * (c[1] * rhor + c[2] * rhor.powi(2) + c[3] * rhor.powi(3)
                        + c[4] * rhor / (c[5] + c[6] * tr + c[7] * rhor + rhor * rhor + c[8] * rhor * tr))
            }
            ViscosityHigherOrder::Ethane => {
                let tau = 305.33 / t;
                let delta = rho_molar / 6870.0;
                let r = [0usize, 1, 1, 2, 2, 2, 3, 3, 4, 4, 1, 1];
                let s = [0.0, 0.0, 1.0, 0.0, 1.0, 1.5, 0.0, 2.0, 0.0, 1.0, 0.0, 1.0];
                let g = [0.0, 0.47177003, -0.23950311, 0.39808301, -0.27343335, 0.35192260,
                    -0.21101308, -0.00478579, 0.07378129, -0.030435255, -0.30435286, 0.001215675];
                let term = |i: usize| g[i] * delta.powi(r[i] as i32) * tau.powf(s[i]);
                let sum1: f64 = (1..=9).map(term).sum();
                let sum2: f64 = (10..=11).map(term).sum();
                15.977 * sum1 / (1.0 + sum2) / 1e6
            }
            ViscosityHigherOrder::FrictionTheory {
                t_reduce, c1, c2, ai, aa, ar, aaa, arr, adrdr, aii, arrr, aaaa,
                na, nr, naa, nrr, nii, nrrr, naaa,
            } => {
                let tau = t_reduce / t;
                let psi1 = tau.exp() - c1;
                let psi2 = (tau * tau).exp() - c2;
                let kk = |arr: &[f64], n: f64| {
                    if arr.len() >= 3 {
                        (arr[0] + arr[1] * psi1 + arr[2] * psi2) * tau.powf(n)
                    } else {
                        0.0
                    }
                };
                let ki = kk(ai, 1.0);
                let ka = kk(aa, *na);
                let kr = kk(ar, *nr);
                let kaa = kk(aaa, *naa);
                let (krr, kdrdr) = if arr.len() >= 3 {
                    (kk(arr, *nrr), 0.0)
                } else {
                    (0.0, kk(adrdr, *nrr))
                };
                let kii = kk(aii, *nii);
                let (krrr, kaaa) = if arrr.len() >= 3 && aaaa.len() >= 3 {
                    (kk(arrr, *nrrr), kk(aaaa, *naaa))
                } else {
                    (0.0, 0.0)
                };
                // Pressure and its T-derivative from the EOS [bar].
                let eos = fluid.eos();
                let r = eos.gas_constant;
                let d = rho_molar / eos.rho_reducing;
                let tau_eos = eos.t_reducing / t;
                let res = eos.residual_derivs(d, tau_eos);
                // p = ρR T(1+δα_δ);  ∂p/∂T|_ρ = ρR(1 + δα_δ − δτα_δτ).
                let p = rho_molar * r * t * (1.0 + d * res.ad) / 1e5;
                let dpdt = rho_molar * r * (1.0 + d * res.ad - d * tau_eos * res.adt);
                let pr = t * dpdt / 1e5;
                let pa = p - pr;
                let pid = rho_molar * r * t / 1e5;
                let deltapr = pr - pid;
                ka * pa + kr * deltapr + ki * pid + kaa * pa * pa + kdrdr * deltapr * deltapr
                    + krr * pr * pr + kii * pid * pid + krrr * pr * pr * pr + kaaa * pa * pa * pa
            }
        }
    }
}

impl ViscosityModel {
    /// Total dynamic viscosity \[Pa·s\] of `fluid` at temperature `t` \[K\] and
    /// mass density `rho` \[kg/m³\].
    pub fn eval(&self, fluid: Fluid, t: f64, rho: f64) -> f64 {
        match self {
            ViscosityModel::Hardcoded(hc) => hc.eval(t, rho),
            ViscosityModel::Correlation { dilute, initial, higher_order } => {
                let rho_molar = rho / fluid.eos().molar_mass;
                let eta_dilute = dilute.eval(t);
                let eta_initial = initial.map(|i| i.contribution(t, rho_molar, eta_dilute)).unwrap_or(0.0);
                eta_dilute + eta_initial + higher_order.eval(fluid, rho_molar, t)
            }
        }
    }

    /// The dilute-gas viscosity \[Pa·s\] alone (needed by the `eta0_and_poly`
    /// conductivity model). `None` for a hardcoded viscosity (not decomposable).
    fn dilute_only(&self, t: f64) -> Option<f64> {
        match self {
            ViscosityModel::Correlation { dilute, .. } => Some(dilute.eval(t)),
            ViscosityModel::Hardcoded(_) => None,
        }
    }
}

impl CriticalConductivity {
    /// Near-critical conductivity enhancement `λ_c` \[W/(m·K)\] at `(T, ρ)`.
    fn eval(&self, fluid: Fluid, t: f64, rho: f64) -> f64 {
        match self {
            CriticalConductivity::SimplifiedOlchowySengers { r0, gamma, big_gamma, zeta0, qd, t_ref } => {
                const NU: f64 = 0.63;
                const K_B: f64 = 1.3806488e-23;
                let pi = std::f64::consts::PI;
                let eos = fluid.eos();
                let r = eos.gas_constant;
                let tc = eos.t_reducing;
                let rhoc = eos.rho_reducing;
                let pcrit = eos.p_critical;
                let tref = if *t_ref > 0.0 { *t_ref } else { 1.5 * tc };
                let rho_molar = rho / eos.molar_mass;
                let delta = rho_molar / rhoc;

                let res = eos.residual_derivs(delta, tc / t);
                let dp_drho = r * t * (1.0 + 2.0 * delta * res.ad + delta * delta * res.add);
                let res_ref = eos.residual_derivs(delta, tc / tref);
                let dp_drho_ref = r * tref * (1.0 + 2.0 * delta * res_ref.ad + delta * delta * res_ref.add);
                let x = pcrit / (rhoc * rhoc) * rho_molar / dp_drho;
                let xref = pcrit / (rhoc * rhoc) * rho_molar / dp_drho_ref * tref / t;
                let num = x - xref;
                if num < f64::EPSILON * 10.0 {
                    return 0.0;
                }
                let zeta = zeta0 * (num / big_gamma).powf(NU / gamma);
                let st = crate::props::state_trho(fluid, t, rho);
                let (cp, cv) = (st.cp * eos.molar_mass, st.cv * eos.molar_mass);
                let mu = match viscosity(fluid, t, rho) {
                    Some(m) => m,
                    None => return 0.0,
                };
                let omega = 2.0 / pi * ((cp - cv) / cp * (zeta * qd).atan() + cv / cp * (zeta * qd));
                let omega0 = 2.0 / pi
                    * (1.0 - (-1.0 / (1.0 / (qd * zeta) + (zeta * qd) * (zeta * qd) / 3.0 / delta / delta)).exp());
                rho_molar * cp * r0 * K_B * t / (6.0 * pi * mu * zeta) * (omega - omega0)
            }
            CriticalConductivity::R123 => {
                let eos = fluid.eos();
                let tau = eos.t_reducing / t;
                let delta = (rho / eos.molar_mass) / eos.rho_reducing;
                0.486742e-2 * (-100.0 * (tau - 1.0).powi(4) - 7.08535 * (delta - 1.0).powi(2)).exp()
            }
            CriticalConductivity::Ammonia => {
                let pi = std::f64::consts::PI;
                let (tc, rhoc) = (405.4, 235.0);
                let (lambda_p, nu, gamma, delta_e, zeta0p, a_zeta, gamma0p) =
                    (1.2, 0.63, 1.24, 0.50, 1.34e-10, 1.0, 0.423e-8);
                let k_b = 1.3806504e-23;
                let tt = ((t - tc) / tc).abs();
                let a_chi = a_zeta / 0.7;
                let eta_b = (2.60 + 1.6 * tt) * 1e-5;
                let dpdt = (2.18 - 0.12 / (17.8 * tt).exp()) * 1e5;
                let x_t = 0.61 * rhoc + 16.5 * tt.ln();
                let dl_i = lambda_p * (k_b * t * t)
                    / (6.0 * pi * eta_b * (zeta0p * tt.powf(-nu) * (1.0 + a_zeta * tt.powf(delta_e))))
                    * dpdt * dpdt * gamma0p * tt.powf(-gamma) * (1.0 + a_chi * tt.powf(delta_e));
                let dl_id = dl_i * (-36.0 * tt * tt).exp();
                if rho < 0.6 * rhoc {
                    dl_id * (x_t * x_t) / (x_t * x_t + (0.6 * rhoc - 0.96 * rhoc).powi(2)) * rho.powi(2)
                        / (0.6 * rhoc).powi(2)
                } else {
                    dl_id * (x_t * x_t) / (x_t * x_t + (rho - 0.96 * rhoc).powi(2))
                }
            }
        }
    }
}

impl ConductivityModel {
    /// Total thermal conductivity \[W/(m·K)\] at `(T, ρ)`. `dilute_visc` is the
    /// fluid's dilute viscosity (needed only by the `eta0_and_poly` dilute
    /// model); pass `None` if the fluid has no viscosity model.
    fn eval(&self, fluid: Fluid, t: f64, rho: f64, dilute_visc: Option<f64>) -> Option<f64> {
        let (dilute_m, residual_m, critical_m) = match self {
            ConductivityModel::Hardcoded(hc) => return Some(hc.eval(fluid, t, rho)),
            ConductivityModel::Correlation { dilute, residual, critical } => (dilute, residual, critical),
        };
        let eos = fluid.eos();
        let tau_eos = eos.t_reducing / t;
        let delta_eos = (rho / eos.molar_mass) / eos.rho_reducing;

        let lambda_dilute = match dilute_m {
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
            ConductivityDilute::EthaneHardcoded => {
                let e_k = 245.0;
                let tstar = t / e_k;
                let fint = 1.7104147 - 0.6936482 / tstar;
                let att = eos.ideal_derivs(delta_eos, tau_eos).att; // ∂²α⁰/∂τ²
                let eta0 = ViscosityDilute::Ethane.eval(t);
                0.276505e-3 * (eta0 * 1e6) * (3.75 - fint * (tau_eos * tau_eos * att + 1.5))
            }
        };

        let lambda_residual = match residual_m {
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

        let lambda_critical = critical_m.map(|c| c.eval(fluid, t, rho)).unwrap_or(0.0);
        Some(lambda_dilute + lambda_residual + lambda_critical)
    }
}

/// Dynamic viscosity `μ` \[Pa·s\] of `fluid` at temperature `t` \[K\] and mass
/// density `rho` \[kg/m³\], or [`None`] if the fluid has no supported viscosity
/// model.
pub fn viscosity(fluid: Fluid, t: f64, rho: f64) -> Option<f64> {
    let mu = fluid.transport()?.viscosity?.eval(fluid, t, rho);
    (mu.is_finite() && mu > 0.0).then_some(mu)
}

/// Thermal conductivity `λ` \[W/(m·K)\] of `fluid` at `(T, ρ)`, or [`None`] if
/// the fluid has no supported conductivity model. Includes the near-critical
/// enhancement where the fluid provides it.
pub fn conductivity(fluid: Fluid, t: f64, rho: f64) -> Option<f64> {
    let transport = fluid.transport()?;
    // The eta0_and_poly dilute model needs the dilute viscosity.
    let dilute_visc = transport.viscosity.and_then(|v| v.dilute_only(t));
    let lambda = transport.conductivity?.eval(fluid, t, rho, dilute_visc)?;
    (lambda.is_finite() && lambda > 0.0).then_some(lambda)
}
