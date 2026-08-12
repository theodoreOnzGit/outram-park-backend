//! Peng-Robinson 1978 (PR78) — the ω-dependent α-slope refit of Peng-Robinson.
//!
//! Ported from DWSIM (GPL-3.0), Visual-Basic reference source:
//! - `DWSIM.Thermodynamics/PropertyPackages/Models/PengRobinson78.vb`
//!   (commit `1abf72d`): the α-slope branch `L145-151` (repeated at
//!   L321-327/L478-484/L790-796/L891-897/L1066-1072), the pure-component
//!   `a_i`/`b_i` at L150-151, and the shared PR compressibility solve `Z_PR`
//!   and fugacity `CalcLnFugCPU` that this port instead reuses from
//!   [`crate::thermo::cubic_eos`].
//!
//! ## What PR78 changes vs the base PR (1976)
//!
//! PR78 is the *only* difference-from-base-PR: the α-function slope `κ(ω)`.
//! Peng & Robinson (1978) refit the slope for heavy / high-acentric-factor
//! species, splitting it at `ω = 0.491`:
//!
//! `κ(ω) = 0.37464 + 1.54226 ω − 0.26992 ω²`                       (ω ≤ 0.491)
//! `κ(ω) = 0.379642 + 1.48503 ω − 0.164423 ω² + 0.016666 ω³`       (ω > 0.491)
//!
//! (`PengRobinson78.vb` L145-149). Below the threshold PR78 is **identical** to
//! the base 1976 PR; above it the cubic-in-ω branch corrects the systematic
//! vapour-pressure error PR76 makes for heavy species. Everything else — the
//! co-volume `b_i = Ωb R Tc / Pc`, `Ωa = 0.45724`, the van der Waals one-fluid
//! mixing, the compressibility cubic, the fugacity coefficient, and the
//! enthalpy/entropy departures — is **unchanged**.
//!
//! This module therefore **reuses** [`crate::thermo::cubic_eos`] wherever the
//! math is unchanged: the compressibility roots come from
//! [`CubicEos::z_roots`] / [`CubicEos::z_vapor`] / [`CubicEos::z_liquid`], and
//! the PR constants (`Ωa`, `Ωb`, `u`, `w`, `√(u²−4w)`) from the
//! [`CubicEos::PengRobinson`] accessors. Only the per-component attraction
//! `a_i(T)` (through `κ`) is re-derived here.
//!
//! > **DWSIM low-ω constant note.** DWSIM's PR78 low-ω branch literally carries
//! > `1.5422` (`PengRobinson78.vb` L146), a truncation of the canonical PR
//! > `1.54226`. This port uses the canonical `1.54226` for the low-ω branch (by
//! > delegating to [`CubicEos::PengRobinson`]'s `alpha_slope`), so PR78 reduces
//! > **exactly** to the base PR for `ω ≤ 0.491`. The `< 6e-5` difference from
//! > DWSIM's truncated constant is far below physical significance.
//!
//! ## Units — documented raw `f64` (SI), per the crate `CLAUDE.md`
//!
//! Temperature K, pressure Pa, `a` J·m³/mol² (= Pa·m⁶/mol²), `b` m³/mol,
//! enthalpy departure J/mol, entropy departure J/(mol·K). `κ`, `α`, `Z`,
//! `Tr = T/Tc`, mole fractions `z`, and `k_ij` are dimensionless. Raw `f64`
//! matches the base kernel; every public signature spells out its units.
//!
//! ## Design (crate `CLAUDE.md`)
//!
//! No `Box`/`dyn`, no lifetimes, no channels. Exposed as **free functions**
//! (mirroring [`crate::thermo::eos_variants`]) composed on top of the existing
//! [`CubicEos`] kernel — no new enum variant, because PR78 differs only in the
//! scalar `κ(ω)` and would otherwise duplicate the base kernel verbatim.
//!
//! > **⚠️ Untrusted AI-assisted draft — pending human V&V.** Early-stage
//! > translation; the tests below are *verification* (does PR78 equal base PR
//! > below the crossover and diverge above it?), not validation against
//! > experimental VLE. Not for nuclear facility operation, reactor control,
//! > safety-critical, or licensing decisions. Independent OUTRAM PARK fork, not
//! > the official DWSIM.

use crate::thermo::cubic_eos::{BinaryInteraction, CubicEos, Phase, R};
use crate::thermo::Component;

/// The acentric-factor threshold `ω = 0.491` at which PR78 switches from the
/// base-PR slope to the 1978 cubic-in-ω branch (`PengRobinson78.vb` L145).
pub const PR78_OMEGA_THRESHOLD: f64 = 0.491;

/// PR78 α-slope `κ(ω)` [-] (`PengRobinson78.vb` L145-149).
///
/// For `ω ≤ 0.491` this is the **canonical base-PR slope**
/// `0.37464 + 1.54226 ω − 0.26992 ω²` — obtained by delegating to
/// [`CubicEos::PengRobinson`]'s `alpha_slope`, so PR78 is bit-for-bit equal to
/// base PR below the threshold. For `ω > 0.491` it is the 1978 refit
/// `0.379642 + 1.48503 ω − 0.164423 ω² + 0.016666 ω³`. `ω` is the Pitzer
/// acentric factor [-]; valid over the usual `−0.1 ≲ ω ≲ 2` range.
#[must_use]
pub fn pr78_kappa(acentric_factor: f64) -> f64 {
    let w = acentric_factor;
    if w <= PR78_OMEGA_THRESHOLD {
        // Reuse the base-PR κ(ω) exactly (canonical 1.54226 constant).
        CubicEos::PengRobinson.alpha_slope(w)
    } else {
        0.379642 + 1.48503 * w - 0.164423 * w * w + 0.016666 * w * w * w
    }
}

/// PR78 α-function `α(T) = [1 + κ(1 − √Tr)]²` [-] for a component at `t` [K].
///
/// Uses [`pr78_kappa`] for the slope. `Tr = T/Tc`; `t > 0`. Equals 1 exactly at
/// the critical point (`Tr = 1`). For `ω ≤ 0.491` this equals
/// [`CubicEos::alpha`] for Peng-Robinson exactly.
#[must_use]
pub fn pr78_alpha(comp: &Component, t: f64) -> f64 {
    let tr = t / comp.critical_temperature;
    let term = 1.0 + pr78_kappa(comp.acentric_factor) * (1.0 - tr.sqrt());
    term * term
}

/// PR78 pure-component attraction `a_i(T) = 0.45724 · α_PR78(T) · R² Tc² / Pc`
/// [J·m³/mol²] at `t` [K] (`PengRobinson78.vb` L150-151).
///
/// Identical to [`CubicEos::a_i`] for Peng-Robinson except the α uses the 1978
/// slope [`pr78_alpha`]. The co-volume `b_i` is **unchanged** — obtain it from
/// `CubicEos::PengRobinson.b_i(comp)`. Valid for `t > 0`.
#[must_use]
pub fn pr78_a_i(comp: &Component, t: f64) -> f64 {
    let tc = comp.critical_temperature;
    let pc = comp.critical_pressure;
    CubicEos::PengRobinson.omega_a() * pr78_alpha(comp, t) * R * R * tc * tc / pc
}

#[inline]
fn kij_at(kij: Option<&BinaryInteraction>, i: usize, j: usize) -> f64 {
    match kij {
        Some(m) if i < m.len() && j < m.len() => m.get(i, j),
        _ => 0.0,
    }
}

/// PR78 mixture attraction `a_mix = Σ_i Σ_j z_i z_j √(a_i a_j)(1 − k_ij)`
/// [J·m³/mol²] at `t` [K], using the PR78 pure-component `a_i` from
/// [`pr78_a_i`].
///
/// The **identical** van der Waals one-fluid mixing rule as
/// [`CubicEos::a_mix`]; only the per-component `a_i` differs. `z` are mole
/// fractions [-]; `kij = None` uses the geometric-mean rule. The mixture
/// co-volume is unchanged: use `CubicEos::PengRobinson.b_mix(comps, z)`.
///
/// # Panics
/// Panics (via slice indexing) if `comps` and `z` differ in length.
#[must_use]
pub fn pr78_a_mix(comps: &[Component], z: &[f64], t: f64, kij: Option<&BinaryInteraction>) -> f64 {
    let ai: Vec<f64> = comps.iter().map(|c| pr78_a_i(c, t)).collect();
    let mut am = 0.0;
    for i in 0..comps.len() {
        for j in 0..comps.len() {
            am += z[i] * z[j] * (ai[i] * ai[j]).sqrt() * (1.0 - kij_at(kij, i, j));
        }
    }
    am
}

/// Phase-selected PR78 compressibility factor `Z` [-] of a mixture at `t` [K],
/// `p` [Pa].
///
/// Assembles `A = a_mix P/(RT)²`, `B = b_mix P/(RT)` from the PR78 attraction
/// [`pr78_a_mix`] and the unchanged co-volume, then **reuses**
/// [`CubicEos::z_vapor`] / [`CubicEos::z_liquid`] for the root solve. `Vapor` →
/// largest real root; `Liquid` → smallest positive real root. Returns `None` if
/// the cubic yields no usable root.
#[must_use]
pub fn z_factor(
    comps: &[Component],
    z: &[f64],
    t: f64,
    p: f64,
    phase: Phase,
    kij: Option<&BinaryInteraction>,
) -> Option<f64> {
    let am = pr78_a_mix(comps, z, t, kij);
    let bm = CubicEos::PengRobinson.b_mix(comps, z);
    let a = am * p / (R * t).powi(2);
    let b = bm * p / (R * t);
    match phase {
        Phase::Vapor => CubicEos::PengRobinson.z_vapor(a, b),
        Phase::Liquid => CubicEos::PengRobinson.z_liquid(a, b),
    }
}

/// Natural log of the PR78 fugacity coefficient `ln φ_i` [-] for every component
/// in a phase at `t` [K], `p` [Pa].
///
/// The standard PR mixture expression (identical algebra to
/// [`CubicEos::ln_phi`], only the PR78 `a_i` fed in):
///
/// `ln φ_i = (b_i/b_m)(Z − 1) − ln(Z − B)`
/// `        − [A/(B√8)](2 Σ_k z_k a_ki / a_m − b_i/b_m)`
/// `          · ln[(2Z + B(2 + √8)) / (2Z + B(2 − √8))]`,
///
/// with `a_ki = √(a_k a_i)(1 − k_ki)`, `√8 = 2√2` for Peng-Robinson. As
/// `p → 0`, every `ln φ_i → 0`. Returns `None` if no `Z` root is found.
#[must_use]
pub fn ln_phi(
    comps: &[Component],
    z: &[f64],
    t: f64,
    p: f64,
    phase: Phase,
    kij: Option<&BinaryInteraction>,
) -> Option<Vec<f64>> {
    let eos = CubicEos::PengRobinson;
    let n = comps.len();
    let ai: Vec<f64> = comps.iter().map(|c| pr78_a_i(c, t)).collect();
    let bi: Vec<f64> = comps.iter().map(|c| eos.b_i(c)).collect();
    let am = pr78_a_mix(comps, z, t, kij);
    let bm = eos.b_mix(comps, z);
    // a_partial[i] = Σ_k z_k √(a_k a_i)(1 − k_ki).
    let apart: Vec<f64> = (0..n)
        .map(|i| {
            (0..n)
                .map(|k| z[k] * (ai[k] * ai[i]).sqrt() * (1.0 - kij_at(kij, k, i)))
                .sum::<f64>()
        })
        .collect();

    let a = am * p / (R * t).powi(2);
    let b = bm * p / (R * t);
    let zf = match phase {
        Phase::Vapor => eos.z_vapor(a, b),
        Phase::Liquid => eos.z_liquid(a, b),
    }?;

    let u = eos.u();
    let sq = eos.sqrt_disc();
    let lg = ((2.0 * zf + b * (u + sq)) / (2.0 * zf + b * (u - sq))).ln();
    let mut out = vec![0.0; n];
    for i in 0..n {
        let t1 = bi[i] * (zf - 1.0) / bm;
        let t2 = -(zf - b).ln();
        let t3 = a * (2.0 * apart[i] / am - bi[i] / bm) / (b * sq);
        out[i] = t1 + t2 - t3 * lg;
    }
    Some(out)
}

/// PR78 temperature derivative of the mixture attraction `d a_mix / dT`
/// [J·m³/(mol²·K)] at `t` [K].
///
/// The DWSIM closed form (`Calc_dadT`), with the PR78 α-slope [`pr78_kappa`] as
/// the per-component `c_i`:
///
/// `da/dT = −(R/2)√(Ωa/T) Σ_i Σ_j z_i z_j (1 − k_ij)`
/// `        [c_j √(a_i Tc_j/Pc_j) + c_i √(a_j Tc_i/Pc_i)]`.
///
/// Feeds the entropy/enthalpy departures below.
#[must_use]
pub fn dadt(comps: &[Component], z: &[f64], t: f64, kij: Option<&BinaryInteraction>) -> f64 {
    let eos = CubicEos::PengRobinson;
    let n = comps.len();
    let ai: Vec<f64> = comps.iter().map(|c| pr78_a_i(c, t)).collect();
    let ci: Vec<f64> = comps
        .iter()
        .map(|c| pr78_kappa(c.acentric_factor))
        .collect();
    let aux1 = -R / 2.0 * (eos.omega_a() / t).sqrt();
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

/// PR78 molar enthalpy departure `H(T,P) − H_ideal(T)` [J/mol] for a phase at
/// `t` [K], `p` [Pa].
///
/// The generalised PR `(u, w) = (2, −1)` residual (identical form to
/// [`CubicEos::enthalpy_departure`], fed the PR78 `a_mix` and [`dadt`]):
///
/// `A_res = a_m/(b_m √8) ln[(2Z+B(2−√8))/(2Z+B(2+√8))] − RT ln((Z−B)/Z) − RT ln Z`,
/// `S_res = R ln((Z−B)/Z) + R ln Z − (da/dT)/(√8 b_m) ln[(2Z+B(2−√8))/(2Z+B(2+√8))]`,
/// `H_res = A_res + T S_res + RT(Z − 1)`.
///
/// Tends to 0 as `p → 0`. Returns `None` if no `Z` root is found.
#[must_use]
pub fn enthalpy_departure(
    comps: &[Component],
    z: &[f64],
    t: f64,
    p: f64,
    phase: Phase,
    kij: Option<&BinaryInteraction>,
) -> Option<f64> {
    let (da_res, ds_res, zf) = departure_parts(comps, z, t, p, phase, kij)?;
    Some(da_res + t * ds_res + R * t * (zf - 1.0))
}

/// PR78 molar entropy departure `S(T,P) − S_ideal(T,P)` [J/(mol·K)] for a phase.
///
/// The `S_res` term of [`enthalpy_departure`]. Tends to 0 as `p → 0`. Returns
/// `None` if no `Z` root is found.
#[must_use]
pub fn entropy_departure(
    comps: &[Component],
    z: &[f64],
    t: f64,
    p: f64,
    phase: Phase,
    kij: Option<&BinaryInteraction>,
) -> Option<f64> {
    let (_, ds_res, _) = departure_parts(comps, z, t, p, phase, kij)?;
    Some(ds_res)
}

/// Shared residual-Helmholtz / residual-entropy assembly. Returns
/// `(A_res [J/mol], S_res [J/(mol·K)], Z [-])`.
fn departure_parts(
    comps: &[Component],
    z: &[f64],
    t: f64,
    p: f64,
    phase: Phase,
    kij: Option<&BinaryInteraction>,
) -> Option<(f64, f64, f64)> {
    let eos = CubicEos::PengRobinson;
    let am = pr78_a_mix(comps, z, t, kij);
    let bm = eos.b_mix(comps, z);
    let a = am * p / (R * t).powi(2);
    let b = bm * p / (R * t);
    let zf = match phase {
        Phase::Vapor => eos.z_vapor(a, b),
        Phase::Liquid => eos.z_liquid(a, b),
    }?;
    let da = dadt(comps, z, t, kij);
    let u = eos.u();
    let sq = eos.sqrt_disc();
    let lg = ((2.0 * zf + b * (u - sq)) / (2.0 * zf + b * (u + sq))).ln();
    let da_res = am / (bm * sq) * lg - R * t * ((zf - b) / zf).ln() - R * t * zf.ln();
    let ds_res = R * ((zf - b) / zf).ln() + R * zf.ln() - 1.0 / (sq * bm) * da * lg;
    Some((da_res, ds_res, zf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermo::component::reference;
    use approx::assert_relative_eq;

    /// **Methodology — the α-crossover.** PR78 must equal the base PR
    /// ([`CubicEos::PengRobinson`]) for a **low-ω** component (`ω < 0.491`) and
    /// diverge for a **high-ω** one. Below the threshold PR78 delegates to the
    /// base-PR slope, so `κ`, `a_i`, the vapour `Z`, and every `ln φ_i` must
    /// match to machine precision. Above the threshold the 1978 cubic branch
    /// gives a different `κ`, hence a different `a_i`.
    ///
    /// Low-ω probe: CO₂ (`ω = 0.225`) at `T = 350 K`, `P = 10 bar`.
    /// High-ω probe: a synthetic heavy species with `ω = 0.8` at `Tr = 0.7`.
    ///
    /// **Measured result (2026-08-03):** for CO₂, PR78 `κ = 0.707984` equals
    /// base PR to `< 1e-15`, PR78 vapour `Z = 0.96682` equals base PR `Z` to
    /// `< 1e-12`, and PR78 `ln φ` equals base PR `ln φ` to `< 1e-12`. For the
    /// `ω = 0.8` species PR78 `κ = 0.379642 + 1.48503·0.8 − 0.164423·0.64 +
    /// 0.016666·0.512 = 1.470968` differs from base PR `κ = 1.435699` by
    /// `≈ 2.5%`, and the PR78 `a_i` differs from the base-PR `a_i` at `Tr = 0.7`
    /// by `≈ 0.94%` (> 0.5%) — confirming the crossover is wired correctly.
    #[test]
    fn pr78_equals_base_pr_below_threshold_diverges_above() {
        let eos = CubicEos::PengRobinson;

        // --- Low-ω: exact agreement with base PR. ---
        let co2 = reference::carbon_dioxide();
        assert!(co2.acentric_factor < PR78_OMEGA_THRESHOLD);
        assert_relative_eq!(
            pr78_kappa(co2.acentric_factor),
            eos.alpha_slope(co2.acentric_factor),
            epsilon = 1e-15
        );
        let comps = [co2.clone()];
        let zz = [1.0];
        let (t, p) = (350.0, 1.0e6);
        let z_pr78 = z_factor(&comps, &zz, t, p, Phase::Vapor, None).unwrap();
        let z_base = eos.z_factor(&comps, &zz, t, p, Phase::Vapor, None).unwrap();
        assert_relative_eq!(z_pr78, z_base, max_relative = 1e-12);
        let lp78 = ln_phi(&comps, &zz, t, p, Phase::Vapor, None).unwrap();
        let lpb = eos.ln_phi(&comps, &zz, t, p, Phase::Vapor, None).unwrap();
        assert_relative_eq!(lp78[0], lpb[0], max_relative = 1e-12);

        // --- High-ω: the 1978 branch diverges from base PR. ---
        let heavy = Component::new(
            "HeavyProbe",
            0.2,
            700.0,
            2.0e6,
            f64::NAN,
            0.8,
            f64::NAN,
            [0.0; 5],
            f64::NAN,
        )
        .unwrap();
        assert!(heavy.acentric_factor > PR78_OMEGA_THRESHOLD);
        let k78 = pr78_kappa(0.8);
        let kbase = eos.alpha_slope(0.8);
        assert_relative_eq!(k78, 1.470968, max_relative = 1e-5);
        assert!(
            (k78 - kbase).abs() / kbase > 0.01,
            "k78={k78} kbase={kbase}"
        );
        // a_i at Tr = 0.7 differs by ~0.94% (> 0.5%).
        let t_hi = 0.7 * heavy.critical_temperature;
        let a78 = pr78_a_i(&heavy, t_hi);
        let abase = eos.a_i(&heavy, t_hi);
        assert!(
            (a78 - abase).abs() / abase > 0.005,
            "a78={a78} abase={abase}"
        );
    }

    /// **Methodology.** At the critical point (`Tr = 1`) the PR78 α must be
    /// exactly 1 for any ω, since `(1 − √1) = 0`. Check the high-ω species so the
    /// 1978 branch is exercised.
    /// **Measured result (2026-08-03):** `α_PR78(Tr=1) = 1.0` to `< 1e-15`.
    #[test]
    fn pr78_alpha_unity_at_critical() {
        let heavy = Component::new(
            "HeavyProbe",
            0.2,
            700.0,
            2.0e6,
            f64::NAN,
            0.8,
            f64::NAN,
            [0.0; 5],
            f64::NAN,
        )
        .unwrap();
        assert_relative_eq!(
            pr78_alpha(&heavy, heavy.critical_temperature),
            1.0,
            epsilon = 1e-15
        );
    }

    /// **Methodology.** The PR78 fugacity coefficient must approach 1 (i.e.
    /// `ln φ → 0`) in the low-pressure ideal-gas limit. Pure CO₂ vapour at
    /// `T = 350 K`, `P = 100 Pa`.
    /// **Measured result (2026-08-03):** `ln φ = −4.2e-6` (|·| < 1e-4).
    #[test]
    fn pr78_fugacity_tends_to_one_at_low_pressure() {
        let comps = [reference::carbon_dioxide()];
        let lp = ln_phi(&comps, &[1.0], 350.0, 100.0, Phase::Vapor, None).unwrap();
        assert!(lp[0].abs() < 1e-4, "ln phi {}", lp[0]);
    }

    /// **Methodology.** PR78 departures must vanish as `p → 0` and equal the base
    /// PR departures for a low-ω component. Pure CO₂ vapour at `T = 320 K`.
    /// **Measured result (2026-08-03):** at 1 Pa |ΔH| < 0.1 J/mol; at 3 MPa the
    /// PR78 enthalpy departure equals the base-PR value to `< 1e-9` (identical
    /// below the ω-threshold).
    #[test]
    fn pr78_departures_match_base_pr_low_omega() {
        let comps = [reference::carbon_dioxide()];
        let z = [1.0];
        let dh0 = enthalpy_departure(&comps, &z, 320.0, 1.0, Phase::Vapor, None).unwrap();
        assert!(dh0.abs() < 0.1, "near-ideal ΔH {dh0}");
        let dh = enthalpy_departure(&comps, &z, 320.0, 3.0e6, Phase::Vapor, None).unwrap();
        let dh_base = CubicEos::PengRobinson
            .enthalpy_departure(&comps, &z, 320.0, 3.0e6, Phase::Vapor, None)
            .unwrap();
        assert_relative_eq!(dh, dh_base, max_relative = 1e-9);
        assert!(dh < 0.0, "moderate-P ΔH should be < 0: {dh}");
    }
}
