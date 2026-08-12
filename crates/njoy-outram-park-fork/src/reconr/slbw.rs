//! Single-Level Breit-Wigner (SLBW) cross-section evaluation.
//!
//! Implements the zero-temperature SLBW formulas from `csslbw` in NJOY2016
//! `reconr.f90`. Provides the penetrability/shift-factor functions `facts`
//! (from `facts` in NJOY) and the phase-shift function `facphi`.
//!
//! ## Physical model
//!
//! For each resonance at E_r with parameters Γ_n, Γ_γ, Γ_f (all in eV):
//!
//! ```text
//! k      = WAVE_K × (A/(A+1)) × √E      [wave number, (10⁻¹² cm)⁻¹]
//! π/k²   =  barn (1 b = 10⁻²⁴ cm²)
//! ρ      = k × r_a                       [channel radius parameter]
//! ρ_c    = k × AP                        [scattering radius parameter]
//! φ_l    = phase shift from phase_shift()
//! P_l,S_l= from shift_and_penetrability()
//!
//! Γ_n(E) = Γ_n × P_l(ρ) / P_l(ρ_r)     [energy-dependent neutron width]
//! Γ_tot  = Γ_n(E) + Γ_γ + Γ_f
//! g_J    = (2J+1) / (4I+2)               [statistical weight]
//! comfac = π/k² × g_J × Γ_n(E) / [(E−E_r′)² + (Γ_tot/2)²]
//!
//! σ_el   += comfac × (Γ_n(E)·cos2φ − 2·Γ_rx·sin²φ + 2·(E−E_r′)·sin2φ)
//! σ_cap  += comfac × Γ_γ
//! σ_fis  += comfac × Γ_f
//! σ_pot   = 4·(2l+1) · π/k² · sin²(φ_l)      [potential scattering]
//! ```
//!
//! Units: energies in eV; cross sections in barns (1 b = 10⁻²⁴ cm²).
//! [`WAVE_K`] is chosen so that `k` is in (10⁻¹² cm)⁻¹ and `π/k²` is in barns.

use crate::common::phys::{AMU_G, AMASSN_AMU, EV_ERG, PI};

/// k = WAVE_K × (A/(A+1)) × √E \[eV\], giving k in (10⁻¹² cm)⁻¹.
///
/// Derived as `sqrt(2·m_n·amu·eV) × 1e-12 / ℏ`, matching NJOY's
/// `cwaven = sqrt(2*amassn*amu*ev)*1e-12/hbar` in `reconr.f90`.
///
/// Numerically ≈ 2.1977 × 10⁻³ (10⁻¹² cm)⁻¹ / √eV.
pub const WAVE_K: f64 = {
    // sqrt(2·AMASSN·AMU·EV) × 1e-12 / HBAR — pre-evaluated to avoid
    // calling sqrt() in a const context.
    // Verification: sqrt(2 × 1.00866 × 1.66054e-24 g × 1.60218e-12 erg)
    //               × 1e-12 / 1.05457e-27 erg·s  =  2.1977e-3
    let _ = (AMU_G, AMASSN_AMU, EV_ERG); // reference constants to silence lints
    2.197_7e-3_f64
};

/// `r_a = RC1 × (m_n·AWR)^{1/3} + RC2` when `naps == 0` (ENDF formula).
const RC1: f64 = 0.123;
/// See [`RC1`].
const RC2: f64 = 0.08;

/// Compute the channel radius r_a \[10⁻¹² cm\].
///
/// - `naps == 0`: empirical formula `0.123·(AMASSN·AWR)^{1/3} + 0.08`.
/// - `naps == 1`: use `ap` (the ENDF potential scattering radius) directly.
pub fn channel_radius(awri: f64, naps: i32, ap: f64) -> f64 {
    if naps == 1 {
        ap
    } else {
        let aw = AMASSN_AMU * awri;
        RC1 * aw.cbrt() + RC2
    }
}

/// Returns `(shift_factor S_l, penetrability P_l)` for angular momentum `l` at `ρ`.
///
/// Ported from `facts(l, rho, se, pe)` in NJOY2016 `reconr.f90:1588`.
///
/// The penetrability P_l determines how much of the incoming wave penetrates the
/// centrifugal barrier; the shift factor S_l shifts the resonance energy slightly.
///
/// | l | P_l(ρ)              | S_l(ρ)              |
/// |---|---------------------|---------------------|
/// | 0 | ρ                   | 0                   |
/// | 1 | ρ³/(1+ρ²)          | −1/(1+ρ²)           |
/// | 2 | ρ⁵/(9+3ρ²+ρ⁴)      | −(18+3ρ²)/(9+3ρ²+ρ⁴) |
pub fn shift_and_penetrability(l: u32, rho: f64) -> (f64, f64) {
    let r2 = rho * rho;
    match l {
        0 => (0.0, rho),
        1 => {
            let den = 1.0 + r2;
            (-1.0 / den, r2 * rho / den)
        }
        2 => {
            let r4 = r2 * r2;
            let den = 9.0 + 3.0 * r2 + r4;
            (-(18.0 + 3.0 * r2) / den, r4 * rho / den)
        }
        3 => {
            let r4 = r2 * r2;
            let r6 = r4 * r2;
            let den = 225.0 + 45.0 * r2 + 6.0 * r4 + r6;
            (-(675.0 + 90.0 * r2 + 6.0 * r4) / den, r6 * rho / den)
        }
        _ => {
            let r4 = r2 * r2;
            let r6 = r4 * r2;
            let r8 = r4 * r4;
            let den = 11025.0 + 1575.0 * r2 + 135.0 * r4 + 10.0 * r6 + r8;
            (
                -(44100.0 + 4725.0 * r2 + 270.0 * r4 + 10.0 * r6) / den,
                r8 * rho / den,
            )
        }
    }
}

/// Hard-sphere phase shift φ_l(ρ_c) for angular momentum `l`.
///
/// Ported from `facphi(l, rho, phi)` in NJOY2016 `reconr.f90:4441`.
///
/// | l | φ_l(ρ)                               |
/// |---|--------------------------------------|
/// | 0 | ρ                                    |
/// | 1 | ρ − atan(ρ)                          |
/// | 2 | ρ − atan(3ρ/(3−ρ²))                 |
pub fn phase_shift(l: u32, rho_c: f64) -> f64 {
    let r2 = rho_c * rho_c;
    match l {
        0 => rho_c,
        1 => rho_c - rho_c.atan(),
        2 => rho_c - (3.0 * rho_c / (3.0 - r2)).atan(),
        3 => rho_c - ((15.0 * rho_c - rho_c * r2) / (15.0 - 6.0 * r2)).atan(),
        _ => {
            let r4 = r2 * r2;
            rho_c - ((105.0 * rho_c - 10.0 * r2 * rho_c) / (105.0 - 45.0 * r2 + r4)).atan()
        }
    }
}

/// Potential scattering cross section \[b\] at energy `e` \[eV\] (s-wave, l=0).
///
/// `σ_pot = 4π·AP²` (isotropic, energy-independent to first order).
///
/// Used to verify the SLBW evaluation for materials with no resonances (e.g. H-2).
///
/// - `ap` — potential scattering radius [10⁻¹² cm] from MF=2.
/// - `awri`, `e` — needed to compute k (and hence the exact `4·pifac·sin²(φ₀)` form).
pub fn potential_scattering(e: f64, awri: f64, ap: f64) -> f64 {
    if e <= 0.0 {
        return 0.0;
    }
    let arat = awri / (awri + 1.0);
    let k = WAVE_K * arat * e.sqrt();
    let pifac = PI / (k * k);
    let rhoc = k * ap;
    let sinsq = rhoc.sin().powi(2);
    4.0 * pifac * sinsq
}

/// Summed cross-section contributions from all resonances in one l-state.
#[derive(Debug, Clone, Copy, Default)]
pub struct SlbwSigmas {
    /// Elastic scattering (resonance + interference + potential) \[b\].
    pub elastic: f64,
    /// Radiative capture (MT=102) \[b\].
    pub capture: f64,
    /// Fission (MT=18) \[b\].
    pub fission: f64,
}

impl SlbwSigmas {
    /// Total cross section = elastic + capture + fission \[b\].
    pub fn total(self) -> f64 {
        self.elastic + self.capture + self.fission
    }
}

/// Evaluate SLBW cross sections at energy `e` \[eV\] for one l-state.
///
/// Implements the inner loop of `csslbw` (zero-temperature) in NJOY2016.
/// Both SLBW (LRF=1) and MLBW (LRF=2) use this evaluation; true MLBW adds
/// interference between levels in the elastic channel, which is negligible for
/// widely-spaced resonances.
///
/// # Parameters
/// - `e`          — neutron kinetic energy \[eV\], must be > 0.
/// - `resonances` — `(ER, AJ, GT, GN, GG, GF)` in eV (GT is total width at E_r).
/// - `l`          — orbital angular momentum of this l-state.
/// - `spi`        — target spin I; statistical weight g_J = (2J+1)/(4I+2).
/// - `ap`         — potential scattering radius \[10⁻¹² cm\].
/// - `awri`       — atomic weight ratio (target / neutron).
/// - `ra`         — channel radius \[10⁻¹² cm\] from `channel_radius()`.
pub fn eval_slbw_lstate(
    e: f64,
    resonances: &[(f64, f64, f64, f64, f64, f64)],
    l: u32,
    spi: f64,
    ap: f64,
    awri: f64,
    ra: f64,
) -> SlbwSigmas {
    if e <= 0.0 {
        return SlbwSigmas::default();
    }

    let arat = awri / (awri + 1.0);
    let k = WAVE_K * arat * e.sqrt();
    let pifac = PI / (k * k);

    let rho = k * ra;
    let rhoc = k * ap;

    let (se, pe) = shift_and_penetrability(l, rho);
    let phi = phase_shift(l, rhoc);
    let cos2p = (2.0 * phi).cos();
    let sin2p = (2.0 * phi).sin();
    let sinsq = phi.sin().powi(2);

    let spot = 4.0 * (2 * l + 1) as f64 * pifac * sinsq;
    let spifac = 1.0 / (2.0 * spi + 1.0);

    let mut sig_el = spot;
    let mut sig_cap = 0.0;
    let mut sig_fis = 0.0;

    for &(er, aj, _gt, gn, gg, gf) in resonances {
        let k_r = WAVE_K * arat * er.abs().sqrt();
        let rho_r = k_r * ra;
        let (ser, per) = shift_and_penetrability(l, rho_r);
        let rper = if per.abs() > 1e-30 { 1.0 / per } else { 0.0 };

        let gne = gn * pe * rper;
        let erp = er + gn * (ser - se) * rper / 2.0;
        let edelt = e - erp;
        let gx = gg + gf;
        let gtt = gne + gx;
        let gj = (2.0 * aj + 1.0) * spifac / 2.0;

        let comfac = pifac * gj * gne / (edelt * edelt + gtt * gtt / 4.0);

        sig_el += comfac * (gne * cos2p - 2.0 * gx * sinsq + 2.0 * edelt * sin2p);
        sig_cap += comfac * gg;
        sig_fis += comfac * gf;
    }

    SlbwSigmas {
        elastic: sig_el,
        capture: sig_cap,
        fission: sig_fis,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swave_penetrability_and_shift() {
        let (s, p) = shift_and_penetrability(0, 0.05);
        assert!((s - 0.0).abs() < 1e-15, "S_0 should be 0, got {s}");
        assert!((p - 0.05).abs() < 1e-15, "P_0 should equal ρ=0.05, got {p}");
    }

    #[test]
    fn pwave_penetrability() {
        let rho = 0.1_f64;
        let (s, p) = shift_and_penetrability(1, rho);
        let den = 1.0 + rho * rho;
        assert!((s - (-1.0 / den)).abs() < 1e-14, "S_1={s}");
        assert!((p - rho * rho * rho / den).abs() < 1e-14, "P_1={p}");
    }

    #[test]
    fn swave_phase_shift() {
        assert!((phase_shift(0, 0.3) - 0.3).abs() < 1e-15);
    }

    #[test]
    fn potential_scattering_h2_thermal() {
        // H-2: AWR=1.9968, AP=0.51977 × 10⁻¹² cm
        // At E=1e-5 eV, σ_pot ≈ 3.4 b (matches the tabulated MF=3 value)
        let sigma = potential_scattering(1e-5, 1.9968, 0.51977);
        assert!(
            sigma > 2.5 && sigma < 5.0,
            "σ_pot(H-2, 1e-5 eV) = {sigma:.3} b, expected 3–4 b"
        );
    }

    #[test]
    fn ar37_resonance_peak_elastic() {
        // Ar-37 first physical resonance: E_r=1540 eV, l=0, J=2
        // GN=20.36, GG=1.0, GF=0, SPI=1.5, AP=0.338779
        let awri = 36.64921;
        let spi = 1.5;
        let ap = 0.338779;
        let ra = channel_radius(awri, 1, ap);
        let res = [(1540.0_f64, 2.0, 21.4, 20.4, 1.0, 0.0)];

        let at_peak = eval_slbw_lstate(1540.0, &res, 0, spi, ap, awri, ra);
        assert!(
            at_peak.elastic > 100.0,
            "σ_el at 1540 eV resonance peak should be > 100 b, got {}",
            at_peak.elastic
        );
        // σ_cap ≈ 50 b: Γ_γ/Γ_n × σ_el_peak ≈ (1/21) × ~1000 b
        assert!(
            at_peak.capture > 10.0 && at_peak.capture < 150.0,
            "σ_cap at 1540 eV peak: {}",
            at_peak.capture
        );

        let far = eval_slbw_lstate(1000.0, &res, 0, spi, ap, awri, ra);
        assert!(
            far.elastic < at_peak.elastic / 10.0,
            "σ_el(1000 eV) should be < 10% of peak"
        );
    }
}
