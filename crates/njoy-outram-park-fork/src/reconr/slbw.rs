//! Single-Level Breit-Wigner (SLBW) resonance cross sections.
//!
//! Ported from `rdf2bw` (LRF=1 path) and `csbw` in NJOY2016 `reconr.f90`.
//! Phase 2b: this module is a stub — the formulas are documented but not yet
//! wired into the main RECONR pipeline. The SLBW evaluation path is exercised
//! in unit tests only.
//!
//! ## Physics background
//!
//! For each resonance at energy E_r with total width Γ, neutron width Γ_n,
//! capture width Γ_γ, and fission width Γ_f, the SLBW cross sections are:
//!
//! ```text
//! σ_n(E) = π/k² × g_J × [Γ_n² / Δ² + 2Γ_n sin(φ_l) × (E-E_r) / Δ²]  (elastic)
//! σ_r(E) = π/k² × g_J × Γ_n × Γ_r / Δ²                               (reaction r)
//!
//! where Δ² = (E - E_r)² + (Γ/2)²
//!       k   = wave number = √(2 m_n E) / ℏ
//!       g_J = (2J+1) / (2(2I+1))   statistical weight
//!       φ_l = potential scattering phase shift for l-th partial wave
//! ```
//!
//! Units: energies in eV, cross sections in barns.

use crate::common::phys::{AMU_G, EV_ERG, HBAR_ERG_S};

/// One SLBW resonance parameter set (from MF=2/MT=151 LIST record).
#[derive(Debug, Clone)]
pub struct SlbwResonance {
    /// Resonance energy [eV].
    pub er: f64,
    /// Angular momentum quantum number l.
    pub l: u32,
    /// Spin quantum number J.
    pub aj: f64,
    /// Total width Γ [eV].
    pub gt: f64,
    /// Neutron width Γ_n [eV] (at E = E_r for s-wave; reduced otherwise).
    pub gn: f64,
    /// Gamma (capture) width Γ_γ [eV].
    pub gg: f64,
    /// Fission width Γ_f [eV].
    pub gf: f64,
}

/// Cross sections contributed by one resonance at energy `e` [eV].
///
/// All values in barns.
#[derive(Debug, Clone, Copy)]
pub struct SlbwResult {
    /// Elastic scattering cross section contribution [b].
    pub elastic: f64,
    /// Capture (radiative) cross section contribution [b].
    pub capture: f64,
    /// Fission cross section contribution [b].
    pub fission: f64,
}

/// Evaluate SLBW cross sections for one resonance at incident energy `e` [eV].
///
/// `awr` is the target atomic weight ratio (target mass / neutron mass).
/// `spi` is the target spin (I).
/// `ap` is the potential scattering radius [10⁻¹² cm].
///
/// Returns `None` if `e ≤ 0` or the resonance has zero neutron width.
pub fn eval_slbw(res: &SlbwResonance, e: f64, awr: f64, spi: f64, ap: f64) -> Option<SlbwResult> {
    if e <= 0.0 || res.gn == 0.0 {
        return None;
    }

    // Reduced neutron mass in grams: μ = m_n × A / (A+1)
    let mn_g = AMU_G;  // neutron mass ≈ 1 amu
    let mu_g = mn_g * awr / (awr + 1.0);

    // Wave number k = √(2 μ e) / ℏ  [cm⁻¹]
    let e_erg = e * EV_ERG;
    let k = (2.0 * mu_g * e_erg).sqrt() / HBAR_ERG_S;

    // π/k² in barns (1 barn = 10⁻²⁴ cm²)
    let pi_over_k2 = std::f64::consts::PI / (k * k) * 1.0e24;

    // Statistical weight factor g_J = (2J+1) / (4I+2)
    let gj = (2.0 * res.aj + 1.0) / (4.0 * spi + 2.0);

    // Energy-dependent neutron width (s-wave: Γ_n(E) = Γ_n × √(E_r / E))
    // For l=0 only in this stub; higher l requires penetrability factors.
    let gn_e = if res.l == 0 {
        res.gn * (res.er / e).sqrt()
    } else {
        res.gn // Phase 2b: add penetrability for l>0
    };

    // Potential scattering phase shift φ_l (s-wave: φ_0 = k·a)
    let phi = if res.l == 0 {
        let a_cm = ap * 1.0e-12; // AP from ENDF is in units of 10⁻¹² cm
        (k * a_cm).atan()
    } else {
        0.0 // Phase 2b
    };

    // Breit-Wigner denominator Δ² = (E - E_r)² + (Γ_tot/2)²
    let delta = (e - res.er).powi(2) + (res.gt / 2.0).powi(2);

    // Elastic: resonance + interference term
    let elastic = pi_over_k2 * gj * (
        gn_e * gn_e / delta
        + 2.0 * gn_e * phi.sin() * (e - res.er) / delta
    );

    // Capture: simple Breit-Wigner
    let capture = pi_over_k2 * gj * gn_e * res.gg / delta;

    // Fission
    let fission = pi_over_k2 * gj * gn_e * res.gf / delta;

    Some(SlbwResult { elastic, capture, fission })
}

/// Potential scattering cross section from the hard-sphere formula.
///
/// σ_pot = 4π/k² × sin²(φ_l)  (s-wave only in this stub)
///
/// `ap` — potential scattering radius [10⁻¹² cm].
/// `e`  — incident energy [eV].
/// `awr` — atomic weight ratio.
pub fn potential_scattering(e: f64, awr: f64, ap: f64) -> f64 {
    if e <= 0.0 {
        return 0.0;
    }
    let mn_g = AMU_G;
    let mu_g = mn_g * awr / (awr + 1.0);
    let e_erg = e * EV_ERG;
    let k = (2.0 * mu_g * e_erg).sqrt() / HBAR_ERG_S;
    let a_cm = ap * 1.0e-12;
    let phi = (k * a_cm).atan();
    let pi_over_k2 = std::f64::consts::PI / (k * k) * 1.0e24;
    4.0 * pi_over_k2 * phi.sin().powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slbw_at_resonance_peak() {
        // A synthetic s-wave resonance at E_r=1 eV, Γ_n=Γ_γ=0.1 eV, Γ_tot=0.2 eV
        // At E = E_r the Breit-Wigner denominator = (Γ/2)²
        let res = SlbwResonance {
            er: 1.0, l: 0, aj: 0.5, gt: 0.2, gn: 0.1, gg: 0.1, gf: 0.0,
        };
        let result = eval_slbw(&res, 1.0, 1.0, 0.5, 0.5).unwrap();
        // Peak capture: π/k² × gJ × Γ_n × Γ_γ / (Γ/2)²
        // All positive and finite
        assert!(result.capture > 0.0, "capture at resonance peak must be positive");
        assert!(result.elastic > 0.0, "elastic at resonance peak must be positive");
        assert_eq!(result.fission, 0.0, "no fission for Gf=0 resonance");
    }

    #[test]
    fn slbw_far_from_resonance_is_small() {
        let res = SlbwResonance {
            er: 1.0, l: 0, aj: 0.5, gt: 0.001, gn: 0.0005, gg: 0.0005, gf: 0.0,
        };
        let peak = eval_slbw(&res, 1.0, 1.0, 0.5, 0.5).unwrap();
        let far  = eval_slbw(&res, 1e6,  1.0, 0.5, 0.5).unwrap();
        assert!(far.capture < peak.capture * 1e-6, "capture falls off far from resonance");
    }

    #[test]
    fn potential_scattering_positive() {
        // H-2 AP ≈ 0.51977 × 10⁻¹² cm, AWR ≈ 1.9968
        let sigma = potential_scattering(1.0e-5, 1.9968, 0.51977);
        assert!(sigma > 0.0, "potential scattering must be positive");
        // Should be ~3 barns (H-2 elastic XS at thermal energies is about 3.4 b)
        assert!(sigma > 1.0 && sigma < 10.0, "σ_pot(H-2, thermal) ≈ 3-4 b, got {sigma}");
    }
}
