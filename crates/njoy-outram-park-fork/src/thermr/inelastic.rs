//! Incoherent-inelastic thermal scattering from S(α,β) — the core THERMR physics.
//!
//! In the incoherent approximation a bound scatterer's double-differential
//! scattering cross section is (ENDF-102 §7; `sig`/`sigl`/`calcem` in
//! `thermr.f90`):
//!
//! ```text
//!   d²σ/dE'dμ (E→E',μ) = (σ_b / 2kT)·√(E'/E)·S̃(α,β)·exp(−β/2)
//! ```
//!
//! with the dimensionless momentum- and energy-transfer variables
//!
//! ```text
//!   α = (E + E' − 2μ√(E·E')) / (A·kT),      β = (E' − E) / kT,
//! ```
//!
//! where `A = B(3)` is the mass ratio, `kT` the temperature in eV, and `σ_b` the
//! bound cross section. `S̃(α,β)` is the (β-symmetric) scattering law stored in
//! MF=7/MT=4; the `exp(−β/2)` factor restores detailed balance for the physical
//! (asymmetric) `S`. When `LAT=1` the tabulated `α,β` are scaled to the reference
//! `kT₀ = 0.0253 eV`, so the lookup uses `α·kT/kT₀`, `β·kT/kT₀`.
//!
//! Integrating over angle gives `σ(E→E')`; integrating that over `E'` gives the
//! incoherent-inelastic cross section `σ_inel(E)`. This module computes all three.
//!
//! ## Numerics
//!
//! Fixed-grid quadratures (dense trapezoid over μ and a kinematically-bounded E'
//! grid) — robust and adequate for the cross section. NJOY refines both grids
//! adaptively to a tolerance; that refinement (and the liquid `cliq` small-α
//! correction) are future accuracy work, noted where they apply.

use super::mf7::{AlphaTable, IncoherentInelastic};
use crate::common::phys::BK_EV_PER_K;

/// Reference temperature kT₀ = 0.0253 eV (`therm`/`tevz` in `thermr.f90`).
const TEVZ: f64 = 0.0253;

/// One equally-probable outgoing-energy bin of a thermal inelastic emission:
/// an outgoing energy and its equally-probable scattering cosines. This is the
/// per-bin content of the ACE ITXE block (IFENG=0).
#[derive(Debug, Clone)]
pub struct OutgoingBin {
    /// Representative outgoing neutron energy \[eV\] for this equiprobable bin.
    pub e_out_ev: f64,
    /// Equally-probable scattering cosines (ascending), length `nang`.
    pub cosines: Vec<f64>,
}

/// Invert a monotone CDF: find `x` where the cumulative `cum` reaches `target`,
/// by linear interpolation. `x` and `cum` are the same length and ascending.
fn invert_cdf(x: &[f64], cum: &[f64], target: f64) -> f64 {
    match cum.iter().position(|&c| c >= target) {
        None => x[x.len() - 1],
        Some(0) => x[0],
        Some(i) => {
            let (c0, c1) = (cum[i - 1], cum[i]);
            if (c1 - c0).abs() < 1e-30 {
                x[i]
            } else {
                x[i - 1] + (target - c0) / (c1 - c0) * (x[i] - x[i - 1])
            }
        }
    }
}

/// `ln S` floor for `S = 0` table entries (`sabflg` in `thermr.f90`).
const SABFLG: f64 = -225.0;

impl IncoherentInelastic {
    /// Mass ratio `A` of the principal scatterer (`B(3)`).
    pub fn mass_ratio(&self) -> f64 {
        self.b[2]
    }

    /// Bound scattering cross section `σ_b` \[barn\] of the principal scatterer:
    /// `(B(1)/natom)·((A+1)/A)²`. `natom` is the number of principal atoms in the
    /// material (`B(6)` records it; pass `1.0` for a monatomic scatterer).
    pub fn sigma_bound(&self, natom: f64) -> f64 {
        let a = self.mass_ratio();
        (self.b[0] / natom) * ((a + 1.0) / a).powi(2)
    }

    /// The double-differential cross section `d²σ/dE'dμ` \[barn\] for scattering
    /// from incident energy `e` \[eV\] to outgoing `ep` \[eV\] through cosine `mu`,
    /// at temperature `temp_k` \[K\]. Zero outside the tabulated `(α,β)` range.
    pub fn double_differential(&self, e: f64, ep: f64, mu: f64, temp_k: f64, natom: f64) -> f64 {
        if e <= 0.0 || ep <= 0.0 {
            return 0.0;
        }
        let tev = BK_EV_PER_K * temp_k;
        let a_mass = self.mass_ratio();

        let beta = (ep - e) / tev; // signed β
        let alpha = (e + ep - 2.0 * mu * (e * ep).sqrt()) / (a_mass * tev);

        // LAT=1: the table is stored at kT₀, so scale the lookup variables.
        let (a_tab, b_tab) = if self.lat == 1 {
            (alpha * tev / TEVZ, beta.abs() * tev / TEVZ)
        } else {
            (alpha, beta.abs())
        };

        let ln_s = self.interp_ln_s(a_tab, b_tab);
        // The scattering law S̃ was floored (S ≈ 0) ⇒ no data here.
        if ln_s <= SABFLG + 1.0 {
            return 0.0;
        }
        // ln of the detailed-balance factor S̃·exp(−β/2). A large positive value
        // means deep downscatter into the many-phonon tail where S̃ has fallen
        // below the numerical floor — the physical product is negligible, but
        // exp(−β/2) would overflow first, so treat it as zero.
        let arg = ln_s - beta / 2.0;
        if arg > 20.0 {
            return 0.0;
        }
        let sigc = (ep / e).sqrt() / (2.0 * tev);
        let sig = sigc * self.sigma_bound(natom) * arg.exp();
        if sig.is_finite() && sig > 0.0 {
            sig
        } else {
            0.0
        }
    }

    /// `σ(E→E')` \[barn\]: the double-differential integrated over cosine `μ`.
    pub fn sigma_e_to_ep(&self, e: f64, ep: f64, temp_k: f64, natom: f64) -> f64 {
        // Dense trapezoid over μ ∈ [−1, 1]; the integrand is smooth but can peak
        // near the μ that minimises α, so use a fine grid.
        const NMU: usize = 200;
        let mut sum = 0.0;
        let mut prev = self.double_differential(e, ep, -1.0, temp_k, natom);
        for k in 1..=NMU {
            let mu = -1.0 + 2.0 * k as f64 / NMU as f64;
            let cur = self.double_differential(e, ep, mu, temp_k, natom);
            sum += 0.5 * (cur + prev) * (2.0 / NMU as f64);
            prev = cur;
        }
        sum
    }

    /// The incoherent-inelastic cross section `σ_inel(E)` \[barn\] at incident
    /// energy `e` \[eV\] and temperature `temp_k` \[K\]: `σ(E→E')` integrated over
    /// the kinematically-allowed outgoing energies `E'`.
    pub fn cross_section(&self, e: f64, temp_k: f64, natom: f64) -> f64 {
        let (eps, sig) = self.sigma_ep_profile(e, temp_k, natom);
        let mut sum = 0.0;
        for k in 1..eps.len() {
            sum += 0.5 * (sig[k] + sig[k - 1]) * (eps[k] - eps[k - 1]);
        }
        sum
    }

    /// The outgoing-energy profile `(E'[], σ(E→E')[])` used to integrate the
    /// cross section and to build the equiprobable emission bins.
    ///
    /// The E' grid is the table's own β grid mapped to both scatter directions
    /// (`|E'−E| = β·D`, `D = kT₀` for LAT=1 else kT) — the points where S(α,β) has
    /// structure. A naive uniform dE' grid wastes all its resolution on the empty
    /// high-energy tail.
    fn sigma_ep_profile(&self, e: f64, temp_k: f64, natom: f64) -> (Vec<f64>, Vec<f64>) {
        if e <= 0.0 || self.beta.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let tev = BK_EV_PER_K * temp_k;
        let d = if self.lat == 1 { TEVZ } else { tev };
        let mut eps: Vec<f64> = Vec::with_capacity(2 * self.beta.len() + 1);
        eps.push(e); // quasi-elastic point β = 0
        for &bj in &self.beta {
            let down = e - bj * d;
            if down > 1.0e-5 {
                eps.push(down);
            }
            eps.push(e + bj * d);
        }
        eps.sort_by(|a, b| a.partial_cmp(b).unwrap());
        eps.dedup_by(|a, b| (*a - *b).abs() < 1e-10 * b.abs().max(1.0));
        let sig = eps.iter().map(|&ep| self.sigma_e_to_ep(e, ep, temp_k, natom)).collect();
        (eps, sig)
    }

    /// Build the ACE thermal inelastic emission for incident energy `e` \[eV\]:
    /// `nieb` equally-probable outgoing energies, each with `nang` equally-probable
    /// scattering cosines. This is the ITXE-block data (IFENG=0), the equiprobable
    /// form `sigl`/`calcem` produce.
    ///
    /// Returns an empty vector when the cross section is zero (no emission).
    pub fn equiprobable_emission(
        &self,
        e: f64,
        temp_k: f64,
        natom: f64,
        nieb: usize,
        nang: usize,
    ) -> Vec<OutgoingBin> {
        let (eps, sig) = self.sigma_ep_profile(e, temp_k, natom);
        if eps.len() < 2 {
            return Vec::new();
        }
        // Cumulative ∫σ(E→E')dE'.
        let mut cum = vec![0.0f64; eps.len()];
        for k in 1..eps.len() {
            cum[k] = cum[k - 1] + 0.5 * (sig[k] + sig[k - 1]) * (eps[k] - eps[k - 1]);
        }
        let total = cum[eps.len() - 1];
        if total <= 0.0 {
            return Vec::new();
        }
        // Equally-probable E' at the bin midpoints (k−0.5)/nieb of the CDF.
        (0..nieb)
            .map(|k| {
                let target = (k as f64 + 0.5) / nieb as f64 * total;
                let ep = invert_cdf(&eps, &cum, target);
                let cosines = self.equiprobable_cosines(e, ep, temp_k, natom, nang);
                OutgoingBin { e_out_ev: ep, cosines }
            })
            .collect()
    }

    /// `nang` equally-probable scattering cosines for scattering `e → ep` at
    /// temperature `temp_k` — the CDF of the angular distribution inverted at the
    /// bin midpoints. Falls back to a uniform spread when the angular distribution
    /// integrates to zero.
    fn equiprobable_cosines(
        &self,
        e: f64,
        ep: f64,
        temp_k: f64,
        natom: f64,
        nang: usize,
    ) -> Vec<f64> {
        const NMU: usize = 200;
        let mu: Vec<f64> = (0..=NMU).map(|k| -1.0 + 2.0 * k as f64 / NMU as f64).collect();
        let f: Vec<f64> =
            mu.iter().map(|&m| self.double_differential(e, ep, m, temp_k, natom)).collect();
        let mut cum = vec![0.0f64; mu.len()];
        for k in 1..mu.len() {
            cum[k] = cum[k - 1] + 0.5 * (f[k] + f[k - 1]) * (mu[k] - mu[k - 1]);
        }
        let total = cum[mu.len() - 1];
        if total <= 0.0 {
            // Isotropic fallback: uniform cosines at the bin midpoints.
            return (0..nang).map(|j| -1.0 + 2.0 * (j as f64 + 0.5) / nang as f64).collect();
        }
        (0..nang)
            .map(|j| {
                let target = (j as f64 + 0.5) / nang as f64 * total;
                invert_cdf(&mu, &cum, target).clamp(-1.0, 1.0)
            })
            .collect()
    }

    /// Interpolate `ln S̃(α, β)` at table coordinates `(a, b)` (both ≥ 0). Bilinear
    /// in `ln S` over the `(α, β)` grid; returns [`SABFLG`] (⇒ `S ≈ 0`) outside the
    /// tabulated range.
    fn interp_ln_s(&self, a: f64, b: f64) -> f64 {
        let beta = &self.beta;
        if beta.is_empty() || b > *beta.last().unwrap() {
            return SABFLG;
        }
        // β bracket.
        let jb = match beta.iter().position(|&bj| b < bj) {
            Some(0) => 0,
            Some(j) => j - 1,
            None => beta.len() - 1,
        };
        let jb1 = (jb + 1).min(beta.len() - 1);

        let s_lo = interp_alpha_ln_s(&self.s_tables[jb], a);
        if jb1 == jb {
            return s_lo;
        }
        let s_hi = interp_alpha_ln_s(&self.s_tables[jb1], a);
        // Linear-in-ln S interpolation across β.
        let (b0, b1) = (beta[jb], beta[jb1]);
        if (b1 - b0).abs() < 1e-30 {
            return s_lo;
        }
        let t = (b - b0) / (b1 - b0);
        s_lo + t * (s_hi - s_lo)
    }
}

/// Interpolate `ln S(α)` within one `β` slice at momentum transfer `a`. Linear in
/// `ln S` over the `α` grid; clamps to the first point below the grid (S is large
/// at small α) and returns [`SABFLG`] above it (S → 0).
fn interp_alpha_ln_s(table: &AlphaTable, a: f64) -> f64 {
    let alpha = &table.alpha;
    let s = &table.s;
    if alpha.is_empty() {
        return SABFLG;
    }
    let ln = |v: f64| if v > 0.0 { v.ln() } else { SABFLG };
    if a <= alpha[0] {
        return ln(s[0]); // clamp (NJOY adds a liquid small-α correction — future)
    }
    if a >= *alpha.last().unwrap() {
        return SABFLG;
    }
    let i = alpha.partition_point(|&x| x <= a) - 1;
    let (a0, a1) = (alpha[i], alpha[i + 1]);
    let (l0, l1) = (ln(s[i]), ln(s[i + 1]));
    let t = (a - a0) / (a1 - a0);
    l0 + t * (l1 - l0)
}

#[cfg(test)]
mod tests {
    use super::super::mf7::parse_mf7;
    use super::*;
    use crate::endf::tape::Tape;
    use std::fs::File;

    fn al27() -> IncoherentInelastic {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/resources/tsl-013_Al_027-ENDF8.0.endf");
        let tape = Tape::read(File::open(p).unwrap()).unwrap();
        parse_mf7(&tape, 53).unwrap().incoherent_inelastic.unwrap()
    }

    #[test]
    fn bound_cross_section_is_physical() {
        let ii = al27();
        // Al: A ≈ 26.75, B(1) ≈ 1.348 ⇒ σ_b ≈ 1.45 b (Al bound scattering ~1.5 b).
        let sb = ii.sigma_bound(1.0);
        assert!((1.2..1.7).contains(&sb), "Al σ_b ≈ 1.45 b, got {sb}");
        assert!((ii.mass_ratio() - 26.75).abs() < 0.1);
    }

    #[test]
    fn double_differential_nonnegative_and_finite() {
        let ii = al27();
        let temp = ii.temperature_k;
        for &mu in &[-1.0, -0.5, 0.0, 0.5, 1.0] {
            for &ep in &[0.001, 0.0253, 0.1] {
                let s = ii.double_differential(0.0253, ep, mu, temp, 1.0);
                assert!(s >= 0.0 && s.is_finite(), "d²σ ≥ 0 finite, got {s}");
            }
        }
    }

    #[test]
    fn inelastic_cross_section_is_positive_and_finite() {
        let ii = al27();
        let temp = ii.temperature_k;
        for &e in &[0.001, 0.0253, 0.1, 0.5, 1.0] {
            let xs = ii.cross_section(e, temp, 1.0);
            assert!(xs.is_finite() && xs >= 0.0, "σ_inel(E={e}) finite ≥ 0, got {xs}");
        }
    }

    #[test]
    fn cross_section_rises_toward_free_gas_limit() {
        // For this cold (T=20 K) crystal the incoherent-inelastic cross section is
        // *suppressed* at thermal energies (few phonons to exchange) and rises
        // toward the free-atom cross section σ_free = B(1)/natom as the incident
        // energy climbs above the phonon spectrum — the correct free-gas limit.
        let ii = al27();
        let temp = ii.temperature_k;
        let sigma_free = ii.b[0]; // = B(1)/natom (natom = 1) = free-atom σ
        let a = ii.cross_section(0.0253, temp, 1.0);
        let b = ii.cross_section(0.5, temp, 1.0);
        let c = ii.cross_section(1.5, temp, 1.0);
        assert!(a < b && b < c, "σ_inel rises with E: {a} < {b} < {c}");
        assert!(
            c > 0.5 * sigma_free && c < 1.3 * sigma_free,
            "σ_inel(1.5 eV)={c} should approach σ_free={sigma_free}"
        );
    }
}
