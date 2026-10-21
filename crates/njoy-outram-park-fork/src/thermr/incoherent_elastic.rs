//! Incoherent-elastic thermal scattering — the second **THERMR** elastic mode.
//!
//! For a hydrogenous solid (H in ZrH, H in polyethylene, …) the bound protons
//! scatter neutrons elastically but *incoherently*: the neutron keeps its energy
//! (elastic) yet the angular distribution is a smooth exponential in the cosine
//! rather than the discrete Bragg peaks of the coherent (crystalline) case.
//!
//! ENDF MF=7/MT=2 (`LTHR=2` or `3`) stores a characteristic bound cross section
//! `σ_b` (= SB) and the temperature-dependent Debye-Waller integral `W'(T)`. The
//! cross section is
//!
//! ```text
//!   σ(E,T) = (σ_b / 2N) · (1 − e^{−4·E·W'}) / (2·E·W'),
//! ```
//!
//! with `N` the number of principal scattering atoms. The angular distribution
//! of the (elastic) outgoing neutron is
//!
//! ```text
//!   p(μ) ∝ exp(−2·E·W'·(1 − μ)),   μ ∈ [−1, 1],
//! ```
//!
//! forward-peaked and increasingly so as `E·W'` grows. Ported from the
//! incoherent-elastic branch of `thermr.f90` (`iel`); the ACE writer consumes the
//! cross section and the equally-probable cosines for the ITCE/ITCX/ITCA blocks.

use petir::mathf::RealMath;
use super::mf7::IncoherentElastic;
use crate::endf::interp::terpa;
use crate::NjoyError;

impl IncoherentElastic {
    /// Debye-Waller integral `W'(T)` \[eV⁻¹\] at temperature `temp_k` \[K\],
    /// linearly interpolated in `T` over the tabulated `(T, W')` table and clamped
    /// to the end values outside the tabulated range.
    ///
    /// The ENDF tables are effectively lin-lin in `T`, so a straight linear
    /// interpolation reproduces `thermr.f90`'s `terpa` here.
    pub fn debye_waller(&self, temp_k: f64) -> f64 {
        let t = &self.wp_of_t;
        match t.first() {
            None => 0.0,
            Some(&(t0, w0)) if temp_k <= t0 => w0,
            Some(_) => {
                let &(tn, wn) = t.last().unwrap();
                if temp_k >= tn {
                    return wn;
                }
                // Find the bracketing interval and interpolate linearly.
                let hi = t.iter().position(|&(tt, _)| tt >= temp_k).unwrap();
                let (t_lo, w_lo) = t[hi - 1];
                let (t_hi, w_hi) = t[hi];
                w_lo + (w_hi - w_lo) * (temp_k - t_lo) / (t_hi - t_lo)
            }
        }
    }

    /// Incoherent-elastic cross section \[barn\] at incident energy `e` \[eV\] and
    /// temperature `temp_k` \[K\] for a material with `natom` principal scattering
    /// atoms:
    ///
    /// ```text
    ///   σ(E,T) = (σ_b / 2N) · (1 − e^{−4·E·W'}) / (2·E·W').
    /// ```
    ///
    /// Returns `0` for non-positive energy or a vanishing Debye-Waller integral.
    pub fn cross_section(&self, e_ev: f64, temp_k: f64, natom: f64) -> f64 {
        let wp = self.debye_waller(temp_k);
        if e_ev <= 0.0 || wp <= 0.0 || natom <= 0.0 {
            return 0.0;
        }
        let c2 = 2.0 * e_ev * wp; // = 2·E·W'
        let x1 = (-2.0 * c2).r_exp(); // = e^{−4·E·W'}
        let c1 = self.sb / (2.0 * natom);
        c1 * (1.0 - x1) / c2
    }

    /// `nbin` equally-probable representative scattering cosines for incoherent-
    /// elastic scattering at incident energy `e` \[eV\] and temperature `temp_k`
    /// \[K\]. Each returned cosine is the mean of `μ` over one `1/nbin`-probability
    /// bin of the exponential angular distribution `p(μ) ∝ e^{−2EW'(1−μ)}`.
    ///
    /// This is the ITCA-block data (`NCL = nbin − 1` discrete cosines). Ports the
    /// equal-probability-bin construction of `thermr.f90::iel`. Falls back to a
    /// uniform (isotropic) spread when `E·W' → 0` (the distribution is flat).
    pub fn equiprobable_cosines(&self, e_ev: f64, temp_k: f64, nbin: usize) -> Vec<f64> {
        let wp = self.debye_waller(temp_k);
        let c2 = 2.0 * e_ev * wp;
        let n = nbin as f64;
        if c2 <= 0.0 || nbin == 0 {
            // Flat distribution ⇒ uniform cosines at the bin midpoints.
            return (0..nbin)
                .map(|k| -1.0 + 2.0 * (k as f64 + 0.5) / n)
                .collect();
        }
        let rc2 = 1.0 / c2;
        let x1 = (-2.0 * c2).r_exp(); // CDF normalisation: ∫ over [−1,1] ∝ (1 − x1)
        let r1x1 = 1.0 / (1.0 - x1);
        let mut u = -1.0; // lower edge of the current equal-probability bin
        let mut out = Vec::with_capacity(nbin);
        for _ in 0..nbin {
            let x2 = (-c2 * (1.0 - u)).r_exp();
            // Upper edge of this bin: CDF(unow) − CDF(u) = 1/nbin.
            let unow = 1.0 + rc2 * ((1.0 - x1) / n + x2).r_ln();
            // Representative cosine = nbin · ∫_u^unow μ p(μ) dμ (the bin mean).
            let mu = n
                * rc2
                * ((-c2 * (1.0 - unow)).r_exp() * (c2 * unow - 1.0) - x2 * (c2 * u - 1.0))
                * r1x1;
            out.push(mu.clamp(-1.0, 1.0));
            u = unow;
        }
        out
    }

    /// `iel`'s Debye-Waller integral `dwa` at `temp_k` (`thermr.f90:1298-1318`,
    /// `mat = 20`): `terpa` on the `W'(T)` TAB1 under its own interpolation
    /// law, after moving an end temperature onto `temp_k` when `temp_k` lies
    /// within 10 % beyond it (so the end value is held there). A one-point
    /// table is taken as is within `temp_k/10` of its temperature.
    ///
    /// # Errors
    /// [`NjoyError::TemperatureOutOfRange`] where upstream stops with "bad
    /// temperature for debye-waller factor".
    pub fn iel_dwa(&self, temp_k: f64) -> Result<f64, NjoyError> {
        const UP: f64 = 1.1;
        const DN: f64 = 0.9;
        let t = &self.wp_of_t;
        let (Some(&(tt1, w1)), Some(&(ttn, _))) = (t.first(), t.last()) else {
            return Err(NjoyError::NotPorted("incoherent elastic with an empty W'(T) table"));
        };
        let refuse = || NjoyError::TemperatureOutOfRange { requested_k: temp_k, min_k: tt1, max_k: ttn };
        if t.len() == 1 {
            if (temp_k - tt1).abs() > temp_k / 10.0 {
                return Err(refuse());
            }
            return Ok(w1);
        }
        if temp_k < DN * tt1 || temp_k > UP * ttn {
            return Err(refuse());
        }
        let mut pairs = t.clone();
        if tt1 > temp_k {
            pairs[0].0 = temp_k;
        }
        let n = pairs.len();
        if ttn < temp_k {
            pairs[n - 1].0 = temp_k;
        }
        Ok(terpa(&self.interp, &pairs, temp_k).0)
    }

    /// One `iel` record (`thermr.f90:1377-1404`): the incoherent-elastic cross
    /// section at `e` \[eV\] and its `nbin` equally probable cosines, with
    /// `dwa` from [`Self::iel_dwa`] and `natom` THERMR's card-2 value.
    ///
    /// Upstream's arithmetic in upstream's order (`c1*rc2*(1-x1)`, not
    /// [`Self::cross_section`]'s `c1*(1-x1)/c2`), and no clamp on the
    /// cosines: the two differ in the last bit, which moves a printed word.
    pub fn iel_record(&self, e: f64, dwa: f64, natom: f64, nbin: usize) -> (f64, Vec<f64>) {
        let c1 = self.sb / (2.0 * natom);
        let n = nbin as f64;
        let c2 = 2.0 * e * dwa;
        let mut u = -1.0_f64;
        let rc2 = 1.0 / c2;
        let x1 = (-2.0 * c2).r_exp();
        let r1x1 = 1.0 / (1.0 - x1);
        let xsec = c1 * rc2 * (1.0 - x1);
        let mut mu = Vec::with_capacity(nbin);
        for _ in 0..nbin {
            let x2 = (-c2 * (1.0 - u)).r_exp();
            let unow = 1.0 + rc2 * ((1.0 - x1) / n + x2).r_ln();
            mu.push(n * rc2 * ((-c2 * (1.0 - unow)).r_exp() * (c2 * unow - 1.0) - x2 * (c2 * u - 1.0)) * r1x1);
            u = unow;
        }
        (xsec, mu)
    }
}

#[cfg(test)]
mod tests {
    use super::super::mf7::IncoherentElastic;

    /// A minimal ZrH-like incoherent-elastic table: σ_b ≈ 81 barn, W' rising with
    /// temperature. Values are illustrative, not a specific evaluation.
    fn zrh_like() -> IncoherentElastic {
        IncoherentElastic {
            temperature_k: 296.0,
            sb: 81.0,
            interp: vec![(3, 2)],
            wp_of_t: vec![(296.0, 8.0e-3), (400.0, 1.1e-2), (600.0, 1.6e-2)],
        }
    }

    #[test]
    fn debye_waller_interpolates_and_clamps() {
        let ie = zrh_like();
        assert_eq!(ie.debye_waller(296.0), 8.0e-3, "at the first point");
        assert_eq!(ie.debye_waller(100.0), 8.0e-3, "clamped below the table");
        assert_eq!(ie.debye_waller(1000.0), 1.6e-2, "clamped above the table");
        // Midpoint 348 K is halfway between 296 and 400 K.
        let mid = ie.debye_waller(348.0);
        assert!((mid - 9.5e-3).abs() < 1e-6, "linear interp, got {mid}");
    }

    #[test]
    fn cross_section_is_positive_and_falls_with_energy() {
        let ie = zrh_like();
        let t = 296.0;
        let lo = ie.cross_section(1.0e-3, t, 1.0);
        let hi = ie.cross_section(1.0, t, 1.0);
        assert!(lo > 0.0 && hi > 0.0, "σ > 0");
        // (1 − e^{−4EW'})/(2EW') decreases monotonically with E·W'.
        assert!(hi < lo, "σ decreases as E rises: lo={lo}, hi={hi}");
        // Low-energy limit: (1−e^{−4EW'})/(2EW') → 2, so σ → σ_b/N (bound xs).
        let bound = ie.sb / 1.0; // natom = 1
        assert!(
            (lo - bound).abs() / bound < 0.05,
            "σ(E→0) → σ_b/N: {lo} vs {bound}"
        );
    }

    #[test]
    fn equiprobable_cosines_are_ordered_forward_peaked_and_bounded() {
        let ie = zrh_like();
        let mus = ie.equiprobable_cosines(1.0, 296.0, 8);
        assert_eq!(mus.len(), 8);
        assert!(mus.iter().all(|&m| (-1.0..=1.0).contains(&m)), "μ ∈ [−1,1]");
        assert!(
            mus.windows(2).all(|w| w[1] >= w[0]),
            "cosines ascending by bin"
        );
        // The distribution is forward-peaked, so the mean cosine is > 0.
        let mean: f64 = mus.iter().sum::<f64>() / mus.len() as f64;
        assert!(mean > 0.0, "forward-peaked ⇒ ⟨μ⟩ > 0, got {mean}");
    }

    /// `iel`'s `dwa` holds an end value up to 10 % past the table, refuses
    /// beyond it, and takes a one-point table within `T/10`
    /// (`thermr.f90:1298-1318`).
    #[test]
    fn iel_dwa_holds_ends_within_ten_percent_and_refuses_beyond() {
        let ie = zrh_like();
        assert_eq!(ie.iel_dwa(296.0).unwrap(), 8.0e-3);
        assert_eq!(ie.iel_dwa(280.0).unwrap(), 8.0e-3, "within 0.9 T1: held");
        assert_eq!(ie.iel_dwa(650.0).unwrap(), 1.6e-2, "within 1.1 Tn: held");
        assert!((ie.iel_dwa(348.0).unwrap() - 9.5e-3).abs() < 1e-15, "lin-lin");
        assert!(ie.iel_dwa(250.0).is_err(), "below 0.9 T1");
        assert!(ie.iel_dwa(700.0).is_err(), "above 1.1 Tn");
        let one = IncoherentElastic { wp_of_t: vec![(296.0, 8.0e-3)], ..zrh_like() };
        assert_eq!(one.iel_dwa(300.0).unwrap(), 8.0e-3);
        assert!(one.iel_dwa(400.0).is_err());
    }

    /// `iel_record` is upstream's arithmetic; the analytic helpers agree with
    /// it to rounding, which is what they were before 2026-10-14.
    #[test]
    fn iel_record_agrees_with_the_analytic_helpers_to_rounding() {
        let ie = zrh_like();
        let dwa = ie.iel_dwa(296.0).unwrap();
        for e in [1.0e-5, 1.0e-2, 0.5, 3.75] {
            let (x, mu) = ie.iel_record(e, dwa, 1.0, 16);
            let xa = ie.cross_section(e, 296.0, 1.0);
            assert!(((x - xa) / xa).abs() < 1e-14, "σ at {e}: {x} vs {xa}");
            for (a, b) in mu.iter().zip(ie.equiprobable_cosines(e, 296.0, 16)) {
                assert!((a - b).abs() < 1e-9, "μ at {e}: {a} vs {b}");
            }
        }
    }

    #[test]
    fn equiprobable_cosines_fall_back_to_uniform_at_zero_energy() {
        let ie = zrh_like();
        let mus = ie.equiprobable_cosines(0.0, 296.0, 4);
        let want = [-0.75, -0.25, 0.25, 0.75];
        for (g, w) in mus.iter().zip(want) {
            assert!((g - w).abs() < 1e-12, "uniform fallback, got {g} want {w}");
        }
    }
}
