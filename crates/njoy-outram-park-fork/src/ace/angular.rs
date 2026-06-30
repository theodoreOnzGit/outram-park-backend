//! Parse ENDF MF=4 elastic angular distributions and convert them to the ACE
//! tabulated-cosine form used by the AND block.
//!
//! Ports the angular-distribution path of NJOY2016 `acefc.f90` (`topfil` +
//! `ptleg`/`pttab` for the `newfor=1` "new format") for **elastic scattering
//! (MT=2)** only — the one angular distribution OpenMC needs before it can track
//! a single collision. Inelastic-level and continuum angular distributions
//! (which couple with the MF=5/MF=6 energy distributions) are a later increment.
//!
//! ## ENDF MF=4 → ACE conversion
//!
//! ENDF gives the elastic angular distribution as, per incident energy, either
//!
//! - **Legendre coefficients** (LTT=1): `f(μ) = Σ_{l=0}^{NL} (2l+1)/2 · a_l ·
//!   P_l(μ)`, with `a_0 ≡ 1`; or
//! - a **tabulated** `f(μ)` (LTT=2); or
//! - both (LTT=3: Legendre at low energy, tabulated at high energy).
//!
//! ACE stores, per incident energy, a tabulated probability density with its
//! cumulative distribution: an interpolation flag `JJ`, a point count `NP`, then
//! `μ(1..NP)`, `pdf(1..NP)`, `cdf(1..NP)`. This module evaluates `f(μ)`,
//! linearises it to lin-lin within a tolerance, clamps it non-negative,
//! renormalises so `∫ f dμ = 1`, and accumulates the CDF.
//!
//! Cosines are stored in the frame ENDF provides (LCT=2, centre-of-mass, for
//! essentially all elastic evaluations), which is the frame an ACE reader assumes
//! for elastic scattering.

use crate::endf::{records::SectionCursor, tape::Section};
use crate::reconr::linearize::linearize_tab1;
use crate::NjoyError;

/// Relative tolerance for linearising `f(μ)` to a lin-lin cosine grid.
const ANGLE_TOL: f64 = 5.0e-3;

/// Maximum bisection depth when linearising a Legendre `f(μ)`.
const MAX_DEPTH: u32 = 20;

/// One incident-energy angular distribution, already converted to ACE form.
#[derive(Debug, Clone)]
pub struct EnergyAngular {
    /// Incident neutron energy \[MeV\].
    pub e_mev: f64,
    /// Tabulated cosine grid (ascending, −1 … +1). Empty ⇒ isotropic at this
    /// energy (the AND block stores locator `0` and no data).
    pub cosines: Vec<f64>,
    /// Probability density `f(μ)` on `cosines`, non-negative, normalised so the
    /// trapezoidal integral over `[−1, 1]` is 1.
    pub pdf: Vec<f64>,
    /// Cumulative distribution on `cosines`: `cdf[0] = 0`, `cdf[last] = 1`.
    pub cdf: Vec<f64>,
}

impl EnergyAngular {
    /// Whether this energy is isotropic (no tabulated data is stored).
    pub fn is_isotropic(&self) -> bool {
        self.cosines.is_empty()
    }
}

/// The elastic (MT=2) angular distribution across all tabulated incident
/// energies, in ACE tabulated-cosine form.
///
/// Built by [`parse_elastic_angular`]. An empty [`energies`](Self::energies)
/// list (or one where every entry is isotropic) means elastic is isotropic
/// overall — the AND block then stores locator `0` for the reaction.
#[derive(Debug, Clone, Default)]
pub struct ElasticAngular {
    /// Per-incident-energy distributions, ascending in energy.
    pub energies: Vec<EnergyAngular>,
}

impl ElasticAngular {
    /// True if there is no anisotropic energy to store (whole reaction isotropic).
    pub fn is_all_isotropic(&self) -> bool {
        self.energies.iter().all(EnergyAngular::is_isotropic)
    }
}

/// Parse an MF=4/MT=2 section into ACE elastic angular distributions.
///
/// Handles LTT = 1 (Legendre), 2 (tabulated), and 3 (both), and the LI=1 /
/// LTT=0 fully-isotropic case (returns an empty distribution).
///
/// # Errors
/// Returns [`NjoyError::EndfParse`] if the record structure is malformed.
pub fn parse_elastic_angular(section: &Section) -> Result<ElasticAngular, NjoyError> {
    let mut cur = SectionCursor::new(&section.rows);
    let head = cur.read_cont()?; // ZA, AWR, 0, LTT, 0, 0
    let ltt = head.l2;
    let trans = cur.read_cont()?; // 0, AWR, LI, LCT, 0, 0
    let li = trans.l1;

    // Fully isotropic: no angular data follows.
    if ltt == 0 || li == 1 {
        return Ok(ElasticAngular::default());
    }

    let mut energies: Vec<EnergyAngular> = Vec::new();

    // LTT=1 or 3: a Legendre section (TAB2 over energies, each a LIST of a_l).
    if ltt == 1 || ltt == 3 {
        let tab2 = cur.read_tab2()?;
        for _ in 0..tab2.head.n2 {
            let list = cur.read_list()?;
            let e_ev = list.head.c2;
            energies.push(from_legendre(e_ev, &list.data));
        }
    }

    // LTT=2 or 3: a tabulated section (TAB2 over energies, each a TAB1 f(μ)).
    if ltt == 2 || ltt == 3 {
        let tab2 = cur.read_tab2()?;
        for _ in 0..tab2.head.n2 {
            let tab1 = cur.read_tab1()?;
            let e_ev = tab1.head.c2;
            energies.push(from_tabulated(e_ev, &tab1.interp, &tab1.pairs));
        }
    }

    energies.sort_by(|a, b| a.e_mev.partial_cmp(&b.e_mev).unwrap());
    Ok(ElasticAngular { energies })
}

/// Convert one Legendre incident-energy point to ACE tabulated-cosine form.
///
/// `coeffs` are `a_1 … a_NL` (the `a_0 ≡ 1` term is implicit). All-zero
/// coefficients ⇒ isotropic.
fn from_legendre(e_ev: f64, coeffs: &[f64]) -> EnergyAngular {
    if coeffs.iter().all(|&a| a == 0.0) {
        return isotropic(e_ev);
    }
    // Adaptively tabulate f(μ) over [−1, 1] to ANGLE_TOL.
    let f = |mu: f64| legendre_pdf(coeffs, mu);
    let grid = adaptive_tabulate(&f);
    finalize(e_ev, grid)
}

/// Convert one tabulated incident-energy point to ACE tabulated-cosine form.
///
/// `interp`/`pairs` are the ENDF TAB1 `f(μ)` table; it is linearised to lin-lin
/// first so the stored grid is purely lin-lin (ACE `JJ = 2`).
fn from_tabulated(e_ev: f64, interp: &[(u32, u32)], pairs: &[(f64, f64)]) -> EnergyAngular {
    if pairs.iter().all(|&(_, f)| f == 0.0) {
        return isotropic(e_ev);
    }
    let grid = linearize_tab1(interp, pairs, ANGLE_TOL);
    finalize(e_ev, grid)
}

/// An isotropic entry (no stored data; the AND block uses locator `0`).
fn isotropic(e_ev: f64) -> EnergyAngular {
    EnergyAngular { e_mev: e_ev / 1.0e6, cosines: Vec::new(), pdf: Vec::new(), cdf: Vec::new() }
}

/// Clamp the raw `(μ, f)` grid non-negative, renormalise to unit integral, and
/// build the CDF — the common tail of both conversion paths.
fn finalize(e_ev: f64, grid: Vec<(f64, f64)>) -> EnergyAngular {
    let cosines: Vec<f64> = grid.iter().map(|&(m, _)| m).collect();
    let mut pdf: Vec<f64> = grid.iter().map(|&(_, f)| f.max(0.0)).collect();

    // Normalise so the trapezoidal integral over the cosine grid is 1.
    let area = trapz(&cosines, &pdf);
    if area > 0.0 {
        for p in &mut pdf {
            *p /= area;
        }
    }

    // Cumulative trapezoidal integral → CDF, pinned to [0, 1].
    let mut cdf = vec![0.0f64; cosines.len()];
    for i in 1..cosines.len() {
        let dm = cosines[i] - cosines[i - 1];
        cdf[i] = cdf[i - 1] + 0.5 * (pdf[i] + pdf[i - 1]) * dm;
    }
    if let Some(&last) = cdf.last() {
        if last > 0.0 {
            for c in &mut cdf {
                *c /= last;
            }
        }
    }

    EnergyAngular { e_mev: e_ev / 1.0e6, cosines, pdf, cdf }
}

/// Trapezoidal integral of `y` over the grid `x` (both same length, ascending).
fn trapz(x: &[f64], y: &[f64]) -> f64 {
    let mut s = 0.0;
    for i in 1..x.len() {
        s += 0.5 * (y[i] + y[i - 1]) * (x[i] - x[i - 1]);
    }
    s
}

/// Evaluate a Legendre angular density at `μ`:
/// `f(μ) = Σ_{l=0}^{NL} (2l+1)/2 · a_l · P_l(μ)`, with `a_0 ≡ 1` and
/// `coeffs[l−1] = a_l` for `l = 1 … NL`.
fn legendre_pdf(coeffs: &[f64], mu: f64) -> f64 {
    let mut p_prev = 1.0; // P_0(μ)
    let mut p_cur = mu; // P_1(μ)
    let mut f = 0.5; // l = 0 term: (1/2)·a_0·P_0
    if !coeffs.is_empty() {
        f += 1.5 * coeffs[0] * p_cur; // l = 1 term: (3/2)·a_1·P_1
    }
    for l in 2..=coeffs.len() {
        let lf = l as f64;
        let p_next = ((2.0 * lf - 1.0) * mu * p_cur - (lf - 1.0) * p_prev) / lf;
        f += (2.0 * lf + 1.0) / 2.0 * coeffs[l - 1] * p_next;
        p_prev = p_cur;
        p_cur = p_next;
    }
    f
}

/// Adaptively tabulate `f(μ)` over `[−1, 1]` so lin-lin interpolation reproduces
/// it within [`ANGLE_TOL`]. Seeds a coarse grid (to catch Legendre oscillation)
/// then bisects each interval recursively.
fn adaptive_tabulate(f: &impl Fn(f64) -> f64) -> Vec<(f64, f64)> {
    // Coarse seed: 9 points across [−1, 1].
    const SEED: usize = 9;
    let seed: Vec<f64> = (0..SEED).map(|i| -1.0 + 2.0 * i as f64 / (SEED - 1) as f64).collect();

    let mut out: Vec<(f64, f64)> = Vec::new();
    out.push((seed[0], f(seed[0])));
    for w in seed.windows(2) {
        let (a, b) = (w[0], w[1]);
        refine(f, a, f(a), b, f(b), 0, &mut out);
        out.push((b, f(b)));
    }
    out
}

/// Recursive midpoint refinement of `f` on `[a, b]` (interior points only).
fn refine(
    f: &impl Fn(f64) -> f64,
    a: f64,
    fa: f64,
    b: f64,
    fb: f64,
    depth: u32,
    out: &mut Vec<(f64, f64)>,
) {
    if depth >= MAX_DEPTH {
        return;
    }
    let m = 0.5 * (a + b);
    if m <= a || m >= b {
        return;
    }
    let fm = f(m);
    let f_lin = 0.5 * (fa + fb);
    let scale = fm.abs().max(1.0e-10);
    if (f_lin - fm).abs() > ANGLE_TOL * scale {
        refine(f, a, fa, m, fm, depth + 1, out);
        out.push((m, fm));
        refine(f, m, fm, b, fb, depth + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legendre_p0_is_isotropic_half() {
        // Only a_0 (implicit): f(μ) = 1/2 everywhere.
        let f = legendre_pdf(&[], 0.3);
        assert!((f - 0.5).abs() < 1e-12);
    }

    #[test]
    fn legendre_first_order_is_linear() {
        // a_1 = 0.1 → f(μ) = 1/2 + (3/2)(0.1)μ.
        let a = [0.1];
        for &mu in &[-1.0, -0.4, 0.0, 0.7, 1.0] {
            let want = 0.5 + 1.5 * 0.1 * mu;
            assert!((legendre_pdf(&a, mu) - want).abs() < 1e-12, "μ={mu}");
        }
    }

    #[test]
    fn finalize_normalises_and_builds_cdf() {
        // Linear-forward-peaked density a_1 = 0.3.
        let f = |mu: f64| legendre_pdf(&[0.3], mu);
        let grid = adaptive_tabulate(&f);
        let ea = finalize(1.0e6, grid);
        assert_eq!(ea.e_mev, 1.0);
        // pdf integrates to 1, cdf monotone from 0 to 1.
        assert!((trapz(&ea.cosines, &ea.pdf) - 1.0).abs() < 1e-6);
        assert_eq!(*ea.cdf.first().unwrap(), 0.0);
        assert!((ea.cdf.last().unwrap() - 1.0).abs() < 1e-12);
        assert!(ea.cdf.windows(2).all(|w| w[1] >= w[0] - 1e-12), "cdf monotone");
        assert!(ea.pdf.iter().all(|&p| p >= 0.0), "pdf non-negative");
    }

    #[test]
    fn zero_coeffs_are_isotropic() {
        let ea = from_legendre(2.0e6, &[0.0, 0.0]);
        assert!(ea.is_isotropic());
        assert_eq!(ea.e_mev, 2.0);
    }

    #[test]
    fn mean_cosine_equals_first_legendre_coefficient() {
        // For f(μ) = Σ (2l+1)/2 · a_l · P_l(μ), the mean cosine
        // ⟨μ⟩ = ∫_{-1}^{1} μ f(μ) dμ = a_1 exactly. The converted tabulated pdf
        // must reproduce this — a direct check that the Legendre→tabulated
        // conversion preserves the physical anisotropy.
        // Exact first moment of a piecewise-linear pdf: on each segment the
        // pdf is c + sμ, so ∫ μ(c+sμ)dμ = c(b²−a²)/2 + s(b³−a³)/3. (A plain
        // trapezoid would be inexact here because μ·pdf is quadratic.)
        let moment1 = |cos: &[f64], pdf: &[f64]| -> f64 {
            let mut m = 0.0;
            for i in 1..cos.len() {
                let (a, b, pa, pb) = (cos[i - 1], cos[i], pdf[i - 1], pdf[i]);
                let s = (pb - pa) / (b - a);
                let c = pa - s * a;
                m += c * (b * b - a * a) / 2.0 + s * (b * b * b - a * a * a) / 3.0;
            }
            m
        };
        for &a1 in &[0.1_f64, 0.3, -0.2] {
            let ea = from_legendre(1.0e6, &[a1, 0.05]);
            let mu_bar = moment1(&ea.cosines, &ea.pdf);
            // Limited only by how well the lin-lin grid approximates f(μ).
            assert!(
                (mu_bar - a1).abs() < 1.0e-3,
                "⟨μ⟩ = {mu_bar} should equal a_1 = {a1}"
            );
        }
    }
}
