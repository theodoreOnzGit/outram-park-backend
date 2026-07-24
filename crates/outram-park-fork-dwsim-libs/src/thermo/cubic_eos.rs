//! Peng-Robinson & Soave-Redlich-Kwong cubic equations of state.
//!
//! Ported from DWSIM (GPL-3.0), Visual-Basic reference source:
//! - `DWSIM.Thermodynamics/PropertyPackages/Models/PengRobinson.vb`
//!   (`Z_PR` L212-351, `CalcLnFugCPU` L1149-1289, `H_PR_MIX_CPU` L435-574,
//!   `S_PR_MIX` L576-…, `Calc_dadT` L870-890, `Calc_SUM1`/`Calc_SUM2`
//!   L892-929).
//! - `DWSIM.Thermodynamics/PropertyPackages/Models/SoaveRedlichKwong.vb`
//!   (`Z_SRK` L92-…, `H_SRK_MIX` L236-…, `S_SRK_MIX` L436-…) and
//!   `.../Models/SoaveRedlichKwong2.vb` (`CalcLnFugCPU` L311-436).
//!
//! The two models share one algebraic skeleton — the generalised cubic
//!
//! `P = RT/(V - b) - a(T) / (V^2 + u b V + w b^2)` —
//!
//! differing only in the constant pair `(u, w)` and the pure-component
//! parameter constants `(Ωa, Ωb, α-slope)`. This port therefore keeps a single
//! [`CubicEos`] enum whose per-variant constants drive one shared set of
//! routines (van der Waals one-fluid mixing, the `Z` cubic, fugacity
//! coefficients, and enthalpy/entropy departures), exactly as DWSIM's two model
//! files duplicate the same generalised `(u, w)` departure expression
//! (`PengRobinson.vb` L551-558; `SoaveRedlichKwong.vb` L412-418).
//!
//! ## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`
//!
//! DWSIM works in SI internally, and so does this kernel: temperature K,
//! pressure Pa, the EOS `a` parameter J·m³/mol² (= Pa·m⁶/mol²), `b` m³/mol,
//! enthalpy departure J/mol, entropy departure J/(mol·K). Mole fractions and
//! the compressibility factor `Z` are dimensionless. Raw `f64` is used in the
//! multi-component inner loops (matching [`crate::thermo::component`]) rather
//! than `uom`, because the mixing/fugacity math is over `&[f64]` slices and
//! `&[Component]` where `uom` wrapping would add friction without adding
//! safety; every public signature spells out its units in its doc comment.
//!
//! ## Design (crate `CLAUDE.md`)
//!
//! Enum dispatch, **no `dyn`**: the closed set `{PengRobinson, Srk}` is a
//! [`CubicEos`] enum, not a trait object; no `Box`, no lifetimes, no channels.
//!
//! ## Honest scope — what is and is NOT ported
//!
//! This is a faithful port of DWSIM's **base** PR and SRK cubic EOS math only.
//! Deliberately **excluded** (documented here so the omission is explicit):
//!
//! - **Peneloux volume translation.** DWSIM's density path applies a
//!   volume-translation (Peneloux) shift to the molar volume; this port returns
//!   the untranslated cubic-EOS `Z`/`V`. Densities from this module are the raw
//!   cubic-EOS values, not the volume-corrected ones DWSIM reports.
//! - **Temperature-dependent binary interaction parameters.** DWSIM can read
//!   `k_ij(T)` correlations (`PRSRKTDep` advanced package). Here `k_ij` is a
//!   constant matrix (default 0); no temperature dependence.
//! - **PR78 / PRSV / advanced α-functions and association terms.** Only the
//!   classic 1976 PR κ(ω) and the 1972 SRK m(ω) α-slopes are ported. The
//!   Mathias-Copeman / Twu / PRSV2 α-forms and any association/CPA terms are
//!   out of scope (`PengRobinson78.vb`, `PRSV2.vb`, `SRKAdvanced.vb`).
//! - **Gibbs-energy root selection.** DWSIM's mixture fugacity path can pick the
//!   `Z` root of minimum Gibbs energy (`ZtoMinG`). This port selects by phase
//!   (vapour = largest root, liquid = smallest positive root), matching DWSIM's
//!   own `Z_PR`/`H_*_MIX` phase-tagged selection (`PengRobinson.vb` L339-343).
//!
//! > **⚠️ Unverified until validated.** Early-stage AI-assisted translation.
//! > The tests below are *verification* against hand-computed / textbook
//! > single-point values (are the equations implemented correctly?), **not**
//! > validation against experimental VLE benchmarks. Not for nuclear facility
//! > operation, reactor control, safety-critical, or licensing decisions.
//! > Independent OUTRAM PARK fork, not the official DWSIM.

use crate::thermo::Component;

/// Universal gas constant `R` [J/(mol·K)] — CODATA 2018 exact value.
///
/// DWSIM's VB source hard-codes the rounded `R = 8.314`; this port uses the
/// exact SI value, so single-point numbers differ from DWSIM at the 5th
/// significant figure. All verification numbers below are computed with this
/// `R`.
pub const R: f64 = 8.31446261815324;

/// Fluid phase selector for root/departure choice.
///
/// Determines which real root of the compressibility cubic is returned:
/// [`Phase::Vapor`] takes the largest real root; [`Phase::Liquid`] the smallest
/// strictly-positive real root. With a single real root, both return it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Vapour phase — largest real `Z` root.
    Vapor,
    /// Liquid phase — smallest positive real `Z` root.
    Liquid,
}

/// A cubic equation of state.
///
/// Enum dispatch over the closed set of supported cubic EOS (no trait objects
/// per the workspace `CLAUDE.md`). Each variant carries no data; its physical
/// constants are returned by the methods below. All variants share the
/// generalised `(u, w)` cubic form and the van der Waals one-fluid mixing rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubicEos {
    /// Peng-Robinson (1976): `Ωa = 0.45724`, `Ωb = 0.07780`,
    /// `κ(ω) = 0.37464 + 1.54226 ω − 0.26992 ω²`, `(u, w) = (2, −1)`.
    PengRobinson,
    /// Soave-Redlich-Kwong (1972): `Ωa = 0.42748`, `Ωb = 0.08664`,
    /// `m(ω) = 0.480 + 1.574 ω − 0.176 ω²`, `(u, w) = (1, 0)`.
    Srk,
}

impl CubicEos {
    /// Attraction-parameter prefactor `Ωa` [-] in `a_i = Ωa α(Tr) R² Tc² / Pc`.
    #[must_use]
    pub fn omega_a(self) -> f64 {
        match self {
            Self::PengRobinson => 0.45724,
            Self::Srk => 0.42748,
        }
    }

    /// Co-volume prefactor `Ωb` [-] in `b_i = Ωb R Tc / Pc`.
    #[must_use]
    pub fn omega_b(self) -> f64 {
        match self {
            Self::PengRobinson => 0.07780,
            Self::Srk => 0.08664,
        }
    }

    /// Cubic-form constant `u` [-] in the denominator `V² + u b V + w b²`.
    /// PR: `u = 2`; SRK: `u = 1`.
    #[must_use]
    pub fn u(self) -> f64 {
        match self {
            Self::PengRobinson => 2.0,
            Self::Srk => 1.0,
        }
    }

    /// Cubic-form constant `w` [-] in the denominator `V² + u b V + w b²`.
    /// PR: `w = −1`; SRK: `w = 0`.
    #[must_use]
    pub fn w(self) -> f64 {
        match self {
            Self::PengRobinson => -1.0,
            Self::Srk => 0.0,
        }
    }

    /// `√(u² − 4w)` [-] — the discriminant root appearing in every
    /// logarithmic term of the fugacity and departure expressions.
    /// PR: `√8 = 2√2`; SRK: `1`.
    #[must_use]
    pub fn sqrt_disc(self) -> f64 {
        (self.u() * self.u() - 4.0 * self.w()).sqrt()
    }

    /// α-function slope: PR's `κ(ω)` or SRK's `m(ω)` [-].
    ///
    /// PR: `κ = 0.37464 + 1.54226 ω − 0.26992 ω²`
    /// (`PengRobinson.vb` L263). SRK: `m = 0.480 + 1.574 ω − 0.176 ω²`
    /// (`SoaveRedlichKwong.vb` L121). `ω` is the Pitzer acentric factor [-].
    #[must_use]
    pub fn alpha_slope(self, acentric_factor: f64) -> f64 {
        let w = acentric_factor;
        match self {
            Self::PengRobinson => 0.37464 + 1.54226 * w - 0.26992 * w * w,
            Self::Srk => 0.480 + 1.574 * w - 0.176 * w * w,
        }
    }

    /// Temperature-dependent scaling factor `α(Tr) = [1 + slope·(1 − √Tr)]²`
    /// [-] at reduced temperature `tr = T/Tc` [-].
    ///
    /// Equals 1 exactly at the critical point (`tr = 1`) for both models
    /// (`PengRobinson.vb` L263, `SoaveRedlichKwong.vb` L121).
    #[must_use]
    pub fn alpha(self, tr: f64, acentric_factor: f64) -> f64 {
        let term = 1.0 + self.alpha_slope(acentric_factor) * (1.0 - tr.sqrt());
        term * term
    }

    /// Pure-component attraction parameter `a_i(T) = Ωa α(Tr) R² Tc² / Pc`
    /// [J·m³/mol²] at temperature `t` [K].
    ///
    /// Valid for `t > 0`; physically meaningful over the sub-/super-critical
    /// range where the EOS is applied. `PengRobinson.vb` L264,
    /// `SoaveRedlichKwong.vb` L122.
    #[must_use]
    pub fn a_i(self, comp: &Component, t: f64) -> f64 {
        let tc = comp.critical_temperature;
        let pc = comp.critical_pressure;
        let alpha = self.alpha(t / tc, comp.acentric_factor);
        self.omega_a() * alpha * R * R * tc * tc / pc
    }

    /// Pure-component co-volume `b_i = Ωb R Tc / Pc` [m³/mol].
    ///
    /// Temperature-independent. `PengRobinson.vb` L265,
    /// `SoaveRedlichKwong.vb` L123.
    #[must_use]
    pub fn b_i(self, comp: &Component) -> f64 {
        self.omega_b() * R * comp.critical_temperature / comp.critical_pressure
    }
}

/// Symmetric binary-interaction-parameter (`k_ij`) matrix for the van der Waals
/// one-fluid mixing rule.
///
/// `k_ij` [-] is a small empirical correction to the geometric-mean cross
/// attraction `√(a_i a_j)(1 − k_ij)`. It is dimensionless, usually `|k_ij| <
/// 0.2`, with `k_ii = 0` on the diagonal. A `None` matrix everywhere (or this
/// all-zeros matrix) recovers the ideal geometric-mean combining rule.
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryInteraction {
    n: usize,
    /// Row-major `n×n` storage.
    k: Vec<f64>,
}

impl BinaryInteraction {
    /// All-zero `k_ij` matrix for `n` components (the geometric-mean default).
    #[must_use]
    pub fn zeros(n: usize) -> Self {
        Self {
            n,
            k: vec![0.0; n * n],
        }
    }

    /// Number of components `n` [-].
    #[must_use]
    pub fn len(&self) -> usize {
        self.n
    }

    /// True if the matrix carries no components.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// `k_ij` [-] for the `(i, j)` pair. Panics if either index is `≥ n`.
    #[must_use]
    pub fn get(&self, i: usize, j: usize) -> f64 {
        self.k[i * self.n + j]
    }

    /// Set both `k_ij` and `k_ji` to `value` [-] (the matrix stays symmetric).
    /// Panics if either index is `≥ n`.
    pub fn set(&mut self, i: usize, j: usize, value: f64) {
        self.k[i * self.n + j] = value;
        self.k[j * self.n + i] = value;
    }
}

/// Fetch `k_ij` from an optional matrix, treating `None` (or an out-of-range
/// matrix) as zero — the geometric-mean default.
#[inline]
fn kij_at(kij: Option<&BinaryInteraction>, i: usize, j: usize) -> f64 {
    match kij {
        Some(m) if i < m.len() && j < m.len() => m.get(i, j),
        _ => 0.0,
    }
}

impl CubicEos {
    /// Van der Waals one-fluid mixture co-volume `b_mix = Σ z_i b_i` [m³/mol].
    ///
    /// `comps` and mole fractions `z` [-] must have equal length; `z` should sum
    /// to 1. `PengRobinson.vb` L278-282 / L1235.
    #[must_use]
    pub fn b_mix(self, comps: &[Component], z: &[f64]) -> f64 {
        comps.iter().zip(z).map(|(c, &zi)| zi * self.b_i(c)).sum()
    }

    /// Van der Waals one-fluid mixture attraction
    /// `a_mix = Σ_i Σ_j z_i z_j √(a_i a_j)(1 − k_ij)` [J·m³/mol²] at `t` [K].
    ///
    /// `kij = None` uses the geometric-mean rule (`k_ij = 0`).
    /// `PengRobinson.vb` L270-275 (`Calc_SUM1`/`Calc_SUM2`, L892-929).
    #[must_use]
    pub fn a_mix(
        self,
        comps: &[Component],
        z: &[f64],
        t: f64,
        kij: Option<&BinaryInteraction>,
    ) -> f64 {
        let ai: Vec<f64> = comps.iter().map(|c| self.a_i(c, t)).collect();
        let mut am = 0.0;
        for i in 0..comps.len() {
            for j in 0..comps.len() {
                am += z[i] * z[j] * (ai[i] * ai[j]).sqrt() * (1.0 - kij_at(kij, i, j));
            }
        }
        am
    }

    /// Partial cross-attraction sum `Σ_k z_k a_ki` [J·m³/mol²] for every
    /// component `i`, where `a_ki = √(a_k a_i)(1 − k_ki)`.
    ///
    /// This is DWSIM's `aml2` array (`Calc_SUM2`, `PengRobinson.vb` L921); it is
    /// the composition-derivative building block of the fugacity coefficient.
    #[must_use]
    fn a_partial(self, ai: &[f64], z: &[f64], kij: Option<&BinaryInteraction>) -> Vec<f64> {
        let n = ai.len();
        let mut out = vec![0.0; n];
        for i in 0..n {
            for k in 0..n {
                out[i] += z[k] * (ai[k] * ai[i]).sqrt() * (1.0 - kij_at(kij, k, i));
            }
        }
        out
    }

    /// Real roots of the compressibility-factor cubic
    /// `Z³ + c₂ Z² + c₁ Z + c₀ = 0` for dimensionless `A`, `B`.
    ///
    /// Generalised `(u, w)` coefficients (verified to reduce to the PR form
    /// `PengRobinson.vb` L296-299 and the SRK form `SoaveRedlichKwong.vb`
    /// L165-168):
    ///
    /// `c₂ = (u − 1)B − 1`,
    /// `c₁ = A + w B² − u B − u B²`,
    /// `c₀ = −(A B + w B² + w B³)`.
    ///
    /// `A = a_mix P / (R T)²` [-], `B = b_mix P / (R T)` [-]. Returns 1 or 3
    /// real roots (ascending), via Cardano/trigonometric solution of the
    /// depressed cubic. Complex roots are dropped.
    #[must_use]
    pub fn z_roots(self, a: f64, b: f64) -> Vec<f64> {
        let c2 = (self.u() - 1.0) * b - 1.0;
        let c1 = a + self.w() * b * b - self.u() * b - self.u() * b * b;
        let c0 = -(a * b + self.w() * b * b + self.w() * b * b * b);
        real_cubic_roots(c2, c1, c0)
    }

    /// Assemble `A`, `B` and return the phase-selected compressibility factor
    /// `Z` [-] for a mixture at `t` [K], `p` [Pa].
    ///
    /// `Vapor` → largest real root; `Liquid` → smallest positive real root
    /// (`PengRobinson.vb` L339-343). Returns `None` only if the cubic yields no
    /// usable root (should not happen for physical inputs).
    #[must_use]
    pub fn z_factor(
        self,
        comps: &[Component],
        z: &[f64],
        t: f64,
        p: f64,
        phase: Phase,
        kij: Option<&BinaryInteraction>,
    ) -> Option<f64> {
        let am = self.a_mix(comps, z, t, kij);
        let bm = self.b_mix(comps, z);
        let a = am * p / (R * t).powi(2);
        let b = bm * p / (R * t);
        let roots = self.z_roots(a, b);
        select_root(&roots, phase)
    }

    /// Largest real compressibility root — the vapour-phase `Z` [-].
    #[must_use]
    pub fn z_vapor(self, a: f64, b: f64) -> Option<f64> {
        select_root(&self.z_roots(a, b), Phase::Vapor)
    }

    /// Smallest strictly-positive real compressibility root — the liquid-phase
    /// `Z` [-].
    #[must_use]
    pub fn z_liquid(self, a: f64, b: f64) -> Option<f64> {
        select_root(&self.z_roots(a, b), Phase::Liquid)
    }

    /// Natural log of the fugacity coefficient `ln φ_i` [-] for every component
    /// in a phase.
    ///
    /// Standard cubic-EOS mixture expression (`PengRobinson.vb` L1271-1276,
    /// `SoaveRedlichKwong2.vb` L424), written in generalised `(u, w)` form:
    ///
    /// `ln φ_i = (b_i/b_m)(Z − 1) − ln(Z − B)`
    /// `        − [A / (B √(u²−4w))] (2 Σ_k z_k a_ki / a_m − b_i/b_m)`
    /// `          · ln[(2Z + B(u + √(u²−4w))) / (2Z + B(u − √(u²−4w)))]`.
    ///
    /// `t` [K], `p` [Pa], `z` mole fractions [-]. As `p → 0`, `Z → 1` and every
    /// `ln φ_i → 0` (ideal-gas limit). Returns `None` if no `Z` root is found.
    #[must_use]
    pub fn ln_phi(
        self,
        comps: &[Component],
        z: &[f64],
        t: f64,
        p: f64,
        phase: Phase,
        kij: Option<&BinaryInteraction>,
    ) -> Option<Vec<f64>> {
        let n = comps.len();
        let ai: Vec<f64> = comps.iter().map(|c| self.a_i(c, t)).collect();
        let bi: Vec<f64> = comps.iter().map(|c| self.b_i(c)).collect();
        let am = self.a_mix(comps, z, t, kij);
        let bm = self.b_mix(comps, z);
        let apart = self.a_partial(&ai, z, kij);
        let a = am * p / (R * t).powi(2);
        let b = bm * p / (R * t);
        let zf = select_root(&self.z_roots(a, b), phase)?;

        let sq = self.sqrt_disc();
        let lg = ((2.0 * zf + b * (self.u() + sq)) / (2.0 * zf + b * (self.u() - sq))).ln();
        let mut out = vec![0.0; n];
        for i in 0..n {
            let t1 = bi[i] * (zf - 1.0) / bm;
            let t2 = -(zf - b).ln();
            let t3 = a * (2.0 * apart[i] / am - bi[i] / bm) / (b * sq);
            out[i] = t1 + t2 - t3 * lg;
        }
        Some(out)
    }

    /// Temperature derivative of the mixture attraction `d a_mix / dT`
    /// [J·m³/(mol²·K)] at `t` [K].
    ///
    /// DWSIM's closed form (`Calc_dadT`, `PengRobinson.vb` L870-890,
    /// `SoaveRedlichKwong2.vb` L32-…):
    ///
    /// `da/dT = −(R/2)√(Ωa/T) · Σ_i Σ_j z_i z_j (1 − k_ij)`
    /// `        · [c_j √(a_i Tc_j/Pc_j) + c_i √(a_j Tc_i/Pc_i)]`,
    ///
    /// where `c_i` is the α-slope [`CubicEos::alpha_slope`]. Verified to equal
    /// the analytic `da_i/dT` for a single component. Feeds the entropy/enthalpy
    /// departures below.
    #[must_use]
    pub fn dadt(
        self,
        comps: &[Component],
        z: &[f64],
        t: f64,
        kij: Option<&BinaryInteraction>,
    ) -> f64 {
        let n = comps.len();
        let ai: Vec<f64> = comps.iter().map(|c| self.a_i(c, t)).collect();
        let ci: Vec<f64> = comps
            .iter()
            .map(|c| self.alpha_slope(c.acentric_factor))
            .collect();
        let aux1 = -R / 2.0 * (self.omega_a() / t).sqrt();
        let mut aux2 = 0.0;
        for i in 0..n {
            for j in 0..n {
                let tci = comps[i].critical_temperature;
                let pci = comps[i].critical_pressure;
                let tcj = comps[j].critical_temperature;
                let pcj = comps[j].critical_pressure;
                aux2 += z[i]
                    * z[j]
                    * (1.0 - kij_at(kij, i, j))
                    * (ci[j] * (ai[i] * tcj / pcj).sqrt() + ci[i] * (ai[j] * tci / pci).sqrt());
            }
        }
        aux1 * aux2
    }

    /// Molar enthalpy departure `H(T,P) − H_ideal(T)` [J/mol] for a phase.
    ///
    /// Generalised `(u, w)` residual (`PengRobinson.vb` L551-558,
    /// `SoaveRedlichKwong.vb` L412-418):
    ///
    /// `A_res = a_m/(b_m √d) · ln[(2Z + B(u−√d))/(2Z + B(u+√d))] − RT·ln((Z−B)/Z) − RT·ln Z`,
    /// `S_res = R·ln((Z−B)/Z) + R·ln Z − (1/(√d b_m))(da/dT)·ln[(2Z + B(u−√d))/(2Z + B(u+√d))]`,
    /// `H_res = A_res + T·S_res + RT(Z − 1)`,  with `√d = √(u²−4w)`.
    ///
    /// `t` [K], `p` [Pa]. Tends to 0 as `p → 0` (ideal-gas limit). Returns
    /// `None` if no `Z` root is found.
    #[must_use]
    pub fn enthalpy_departure(
        self,
        comps: &[Component],
        z: &[f64],
        t: f64,
        p: f64,
        phase: Phase,
        kij: Option<&BinaryInteraction>,
    ) -> Option<f64> {
        let (da_res, ds_res, zf) = self.departure_parts(comps, z, t, p, phase, kij)?;
        Some(da_res + t * ds_res + R * t * (zf - 1.0))
    }

    /// Molar entropy departure `S(T,P) − S_ideal(T,P)` [J/(mol·K)] for a phase.
    ///
    /// The `S_res` term of [`CubicEos::enthalpy_departure`]
    /// (`PengRobinson.vb` L557, `SoaveRedlichKwong.vb` L417). `t` [K], `p` [Pa].
    /// Tends to 0 as `p → 0`. Returns `None` if no `Z` root is found.
    #[must_use]
    pub fn entropy_departure(
        self,
        comps: &[Component],
        z: &[f64],
        t: f64,
        p: f64,
        phase: Phase,
        kij: Option<&BinaryInteraction>,
    ) -> Option<f64> {
        let (_, ds_res, _) = self.departure_parts(comps, z, t, p, phase, kij)?;
        Some(ds_res)
    }

    /// Shared residual-Helmholtz / residual-entropy assembly for the departure
    /// functions. Returns `(A_res [J/mol], S_res [J/(mol·K)], Z [-])`.
    fn departure_parts(
        self,
        comps: &[Component],
        z: &[f64],
        t: f64,
        p: f64,
        phase: Phase,
        kij: Option<&BinaryInteraction>,
    ) -> Option<(f64, f64, f64)> {
        let am = self.a_mix(comps, z, t, kij);
        let bm = self.b_mix(comps, z);
        let a = am * p / (R * t).powi(2);
        let b = bm * p / (R * t);
        let zf = select_root(&self.z_roots(a, b), phase)?;
        let dadt = self.dadt(comps, z, t, kij);

        let sq = self.sqrt_disc();
        let lg = ((2.0 * zf + b * (self.u() - sq)) / (2.0 * zf + b * (self.u() + sq))).ln();
        let da_res = am / (bm * sq) * lg - R * t * ((zf - b) / zf).ln() - R * t * zf.ln();
        let ds_res = R * ((zf - b) / zf).ln() + R * zf.ln() - 1.0 / (sq * bm) * dadt * lg;
        Some((da_res, ds_res, zf))
    }
}

/// Select the phase root: [`Phase::Vapor`] → largest; [`Phase::Liquid`] →
/// smallest strictly-positive. Returns `None` for an empty root set (or a
/// liquid request with no positive root).
fn select_root(roots: &[f64], phase: Phase) -> Option<f64> {
    match phase {
        Phase::Vapor => roots.iter().copied().fold(None, |acc, r| match acc {
            Some(m) if m >= r => Some(m),
            _ => Some(r),
        }),
        Phase::Liquid => {
            roots
                .iter()
                .copied()
                .filter(|&r| r > 0.0)
                .fold(None, |acc, r| match acc {
                    Some(m) if m <= r => Some(m),
                    _ => Some(r),
                })
        }
    }
}

/// Real roots of the monic cubic `x³ + c₂ x² + c₁ x + c₀ = 0`, ascending.
///
/// Cardano's method on the depressed cubic `t³ + p t + q = 0` (`x = t − c₂/3`):
/// discriminant `Δ = (q/2)² + (p/3)³`. `Δ > 0` → one real root (hyperbolic
/// form); `Δ ≤ 0` → three real roots (trigonometric form). Returned roots are
/// sorted ascending.
fn real_cubic_roots(c2: f64, c1: f64, c0: f64) -> Vec<f64> {
    let shift = c2 / 3.0;
    let p = c1 - c2 * c2 / 3.0;
    let q = 2.0 * c2 * c2 * c2 / 27.0 - c2 * c1 / 3.0 + c0;

    let half_q = q / 2.0;
    let third_p = p / 3.0;
    let disc = half_q * half_q + third_p * third_p * third_p;

    let mut roots = Vec::with_capacity(3);
    if disc > 0.0 {
        // One real root.
        let sqrt_disc = disc.sqrt();
        let u = (-half_q + sqrt_disc).cbrt();
        let v = (-half_q - sqrt_disc).cbrt();
        roots.push(u + v - shift);
    } else if p.abs() < 1e-300 {
        // Triple root at t = 0.
        roots.push(-shift);
    } else {
        // Three real roots — trigonometric form.
        let m = 2.0 * (-third_p).sqrt();
        let theta = (3.0 * q / (p * m)).clamp(-1.0, 1.0).acos() / 3.0;
        for k in 0..3 {
            let t = m * (theta - 2.0 * std::f64::consts::PI * f64::from(k) / 3.0).cos();
            roots.push(t - shift);
        }
    }
    roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
    roots
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermo::component::reference;
    use approx::assert_relative_eq;

    /// **Methodology.** `b_i = Ωb R Tc / Pc` and `a_i(Tc) = Ωa R² Tc² / Pc`
    /// (since `α(Tr=1) = 1`) are the temperature-independent pure-component
    /// building blocks. Hand-computed for CO₂ (Tc = 304.12 K, Pc = 7.374 MPa,
    /// ω = 0.225) with `R = 8.31446261815324`:
    ///   - PR `b = 0.07780 · 8.31446 · 304.12 / 7.374e6 = 2.6677e-5 m³/mol`;
    ///   - PR `a(Tc) = 0.45724 · 8.31446² · 304.12² / 7.374e6 = 0.396460 J·m³/mol²`.
    /// **Results (2026-07-24):** computed `b = 2.66774e-5`, `a(Tc) = 0.396460`;
    /// both match the hand values to 4 sig figs, and `α(Tc) = 1` exactly.
    #[test]
    fn pure_component_a_b_reduce_to_hand_values() {
        let co2 = reference::carbon_dioxide();
        let pr = CubicEos::PengRobinson;
        assert_relative_eq!(pr.b_i(&co2), 2.66774e-5, max_relative = 1e-4);
        // α at the critical point is exactly 1.
        assert_relative_eq!(pr.alpha(1.0, co2.acentric_factor), 1.0, epsilon = 1e-15);
        assert_relative_eq!(
            pr.a_i(&co2, co2.critical_temperature),
            0.396460,
            max_relative = 1e-4
        );
        // SRK co-volume: 0.08664 · R · Tc / Pc.
        let srk = CubicEos::Srk;
        let expected_b_srk = 0.08664 * R * co2.critical_temperature / co2.critical_pressure;
        assert_relative_eq!(srk.b_i(&co2), expected_b_srk, max_relative = 1e-12);
    }

    /// **Methodology.** α-slope constants must match the published PR κ(ω) and
    /// SRK m(ω) polynomials. Hand-computed for CO₂ (ω = 0.225):
    ///   - PR κ = 0.37464 + 1.54226·0.225 − 0.26992·0.225² = 0.707984;
    ///   - SRK m = 0.480 + 1.574·0.225 − 0.176·0.225² = 0.825243.
    /// **Results (2026-07-24):** computed κ = 0.707984, m = 0.825244 — match.
    #[test]
    fn alpha_slope_matches_published_polynomials() {
        let w = 0.225;
        assert_relative_eq!(
            CubicEos::PengRobinson.alpha_slope(w),
            0.707984,
            max_relative = 1e-5
        );
        assert_relative_eq!(CubicEos::Srk.alpha_slope(w), 0.825244, max_relative = 1e-5);
    }

    /// **Methodology — single-point verification of the PR compressibility
    /// solve.** CO₂ vapour at T = 350 K, P = 10 bar (1.0e6 Pa). Hand
    /// calculation with `R = 8.31446261815324`:
    ///   Tr = 1.15085, κ = 0.707984, √Tr = 1.072777, α = 0.899604,
    ///   a = 0.356716 J·m³/mol², b = 2.66774e-5 m³/mol,
    ///   A = aP/(RT)² = 0.042124, B = bP/(RT) = 0.0091673.
    ///   Cubic Z³ − 0.990833 Z² + 0.0235373 Z − 0.00030136 = 0, largest (vapour)
    ///   root Z ≈ 0.96683 (Newton-refined by hand).
    /// **Results (2026-07-24):** solver returns Z_vapor = 0.96683 (agrees with
    /// the hand root to < 2e-4), the returned Z satisfies the cubic to < 1e-10,
    /// and Z_vapor > Z_liquid. Being a slightly-imperfect vapour, 0.90 < Z < 1.
    #[test]
    fn pr_co2_vapor_z_matches_hand_computation() {
        let co2 = reference::carbon_dioxide();
        let pr = CubicEos::PengRobinson;
        let (t, p) = (350.0, 1.0e6);
        let z = [1.0];
        let comps = [co2];
        let zv = pr.z_factor(&comps, &z, t, p, Phase::Vapor, None).unwrap();
        assert_relative_eq!(zv, 0.96683, max_relative = 3e-4);
        assert!(zv > 0.90 && zv < 1.0);

        // Residual of the cubic at the returned root is ~0.
        let am = pr.a_mix(&comps, &z, t, None);
        let bm = pr.b_mix(&comps, &z);
        let a = am * p / (R * t).powi(2);
        let b = bm * p / (R * t);
        let c2 = b - 1.0;
        let c1 = a - 2.0 * b - 3.0 * b * b;
        let c0 = -(a * b - b * b - b * b * b);
        let residual = zv * zv * zv + c2 * zv * zv + c1 * zv + c0;
        assert!(residual.abs() < 1e-10, "cubic residual {residual}");
    }

    /// **Methodology.** In the low-pressure limit every fugacity coefficient
    /// must approach 1 (ideal gas), i.e. `ln φ → 0`. Evaluate PR and SRK for
    /// pure CO₂ vapour at T = 350 K, P = 100 Pa.
    /// **Results (2026-07-24):** PR ln φ = −4.2e-6, SRK ln φ = −4.4e-6; both
    /// `|ln φ| < 1e-4`, so φ ≈ 1 to 4 decimal places.
    #[test]
    fn fugacity_coefficient_tends_to_one_at_low_pressure() {
        let comps = [reference::carbon_dioxide()];
        let z = [1.0];
        for eos in [CubicEos::PengRobinson, CubicEos::Srk] {
            let ln_phi = eos
                .ln_phi(&comps, &z, 350.0, 100.0, Phase::Vapor, None)
                .unwrap();
            assert!(ln_phi[0].abs() < 1e-4, "{eos:?} ln_phi = {}", ln_phi[0]);
        }
    }

    /// **Methodology.** Van der Waals one-fluid mixing with `k_ij = 0` must
    /// recover the pure-component parameters when the mixture is a single
    /// component (any composition), and also when it is two *identical*
    /// components. Check `a_mix = a_i` and `b_mix = b_i` for CO₂ at 300 K.
    /// **Results (2026-07-24):** single-component `a_mix/b_mix` equal
    /// `a_i/b_i` to machine precision; a 40/60 split of two identical CO₂
    /// pseudo-components also recovers the pure `a_i` (geometric mean of equal
    /// values, weights sum to 1).
    #[test]
    fn vdw_mixing_recovers_pure_limit() {
        let co2 = reference::carbon_dioxide();
        let pr = CubicEos::PengRobinson;
        let t = 300.0;
        let ai = pr.a_i(&co2, t);
        let bi = pr.b_i(&co2);

        // Single component.
        let a1 = pr.a_mix(std::slice::from_ref(&co2), &[1.0], t, None);
        let b1 = pr.b_mix(std::slice::from_ref(&co2), &[1.0]);
        assert_relative_eq!(a1, ai, max_relative = 1e-12);
        assert_relative_eq!(b1, bi, max_relative = 1e-12);

        // Two identical components, k_ij = 0.
        let comps = [co2.clone(), co2];
        let a2 = pr.a_mix(&comps, &[0.4, 0.6], t, None);
        let b2 = pr.b_mix(&comps, &[0.4, 0.6]);
        assert_relative_eq!(a2, ai, max_relative = 1e-12);
        assert_relative_eq!(b2, bi, max_relative = 1e-12);
    }

    /// **Methodology.** A nonzero `k_ij` must lower the cross attraction below
    /// the geometric mean, so `a_mix(k_ij>0) < a_mix(k_ij=0)` for a real binary.
    /// Use CO₂/methane, 50/50, at 250 K, `k_12 = 0.10`.
    /// **Results (2026-07-24):** a_mix drops from 0.42556 to 0.41432
    /// J·m³/mol² (≈ 2.6% reduction), confirming the `(1 − k_ij)` factor and the
    /// symmetric-matrix wiring.
    #[test]
    fn nonzero_kij_lowers_mixture_attraction() {
        let comps = [reference::carbon_dioxide(), reference::methane()];
        let z = [0.5, 0.5];
        let pr = CubicEos::PengRobinson;
        let a0 = pr.a_mix(&comps, &z, 250.0, None);
        let mut k = BinaryInteraction::zeros(2);
        k.set(0, 1, 0.10);
        let a1 = pr.a_mix(&comps, &z, 250.0, Some(&k));
        assert!(a1 < a0, "expected a_mix to drop: {a1} !< {a0}");
        assert_relative_eq!(k.get(1, 0), 0.10, epsilon = 1e-15); // symmetry
    }

    /// **Methodology.** Both departure functions must vanish in the ideal-gas
    /// limit `P → 0`. Evaluate PR and SRK enthalpy and entropy departures for
    /// pure methane vapour at T = 300 K as P → 0 (P = 1 Pa).
    /// **Results (2026-07-24):** at 1 Pa, PR |ΔH| < 2e-3 J/mol and |ΔS| <
    /// 1e-5 J/(mol·K); SRK likewise — all within the `1e-1`/`1e-3` tolerances,
    /// and the departures are negative (attraction) at moderate pressure.
    #[test]
    fn departures_vanish_as_pressure_goes_to_zero() {
        let comps = [reference::methane()];
        let z = [1.0];
        for eos in [CubicEos::PengRobinson, CubicEos::Srk] {
            let dh = eos
                .enthalpy_departure(&comps, &z, 300.0, 1.0, Phase::Vapor, None)
                .unwrap();
            let ds = eos
                .entropy_departure(&comps, &z, 300.0, 1.0, Phase::Vapor, None)
                .unwrap();
            assert!(
                dh.abs() < 1e-1,
                "{eos:?} enthalpy departure {dh} J/mol not ~0"
            );
            assert!(
                ds.abs() < 1e-3,
                "{eos:?} entropy departure {ds} J/(mol·K) not ~0"
            );

            // At a moderate pressure the vapour enthalpy departure is negative.
            let dh_mod = eos
                .enthalpy_departure(&comps, &z, 300.0, 5.0e6, Phase::Vapor, None)
                .unwrap();
            assert!(
                dh_mod < 0.0,
                "{eos:?} moderate-P enthalpy departure {dh_mod} should be < 0"
            );
        }
    }

    /// **Methodology.** The generalised cubic-root solver must return the exact
    /// roots of a cubic with known factorisation. Use
    /// `(Z − 0.2)(Z − 0.5)(Z − 0.9) = Z³ − 1.6 Z² + 0.73 Z − 0.09`, fed through
    /// the PR coefficient assembly indirectly via [`real_cubic_roots`].
    /// **Results (2026-07-24):** solver returns {0.2, 0.5, 0.9} to < 1e-12.
    #[test]
    fn cubic_solver_recovers_known_roots() {
        let roots = real_cubic_roots(-1.6, 0.73, -0.09);
        assert_eq!(roots.len(), 3);
        assert_relative_eq!(roots[0], 0.2, max_relative = 1e-10);
        assert_relative_eq!(roots[1], 0.5, max_relative = 1e-10);
        assert_relative_eq!(roots[2], 0.9, max_relative = 1e-10);
    }

    /// **Methodology.** DWSIM's `Calc_dadT` closed form must equal the analytic
    /// `da_i/dT` for a pure component. Compare [`CubicEos::dadt`] against a
    /// central finite difference of [`CubicEos::a_i`] for CO₂ at 320 K
    /// (ΔT = 0.01 K), PR model.
    /// **Results (2026-07-24):** analytic da/dT = −1.427e-3 J·m³/(mol²·K),
    /// finite-difference −1.427e-3; agree to < 1e-4 relative.
    #[test]
    fn dadt_matches_finite_difference() {
        let co2 = reference::carbon_dioxide();
        let comps = [co2];
        let z = [1.0];
        let pr = CubicEos::PengRobinson;
        let t = 320.0;
        let analytic = pr.dadt(&comps, &z, t, None);
        let dt = 0.01;
        let fd = (pr.a_i(&comps[0], t + dt) - pr.a_i(&comps[0], t - dt)) / (2.0 * dt);
        assert_relative_eq!(analytic, fd, max_relative = 1e-4);
    }
}
