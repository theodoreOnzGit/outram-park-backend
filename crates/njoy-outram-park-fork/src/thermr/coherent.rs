//! Coherent-elastic (Bragg) thermal scattering — the first **THERMR** computation.
//!
//! For a crystalline bound scatterer (graphite, Be, Al, …) the coherent-elastic
//! cross section is a sum of Bragg reflections. ENDF MF=7/MT=2 (`LTHR=1`) stores
//! the **cumulative structure factor** `S(E, T)`: a step function of incident
//! energy that jumps up at each Bragg edge `E_i`, tabulated at one or more
//! temperatures. The cross section is then
//!
//! ```text
//!   σ_coh(E, T) = S(E, T) / E,   with S(E, T) = S(E_i, T) for  E_i ≤ E < E_{i+1},
//! ```
//!
//! i.e. below the first Bragg edge the coherent-elastic cross section is zero,
//! then it follows a `1/E` sawtooth that steps up at every edge. The Bragg-edge
//! *energies* are temperature-independent (set by the lattice spacing); the `S`
//! values fall with temperature through the Debye-Waller factor, so
//! `σ_coh(E, T)` at fixed `E` *decreases* as `T` rises. Ported from the
//! coherent-elastic branch of `thermr.f90` (`coh`/`sigcoh`; the temperature
//! matching mirrors `rdelas`).
//!
//! Every method takes the temperature explicitly and resolves it per the policy
//! in the [`mf7` module docs](super::mf7): match a tabulated temperature within
//! the NJOY `T/1000 + 5` K tolerance, interpolate between tabulated points with
//! the evaluation's `LI` law, or refuse with
//! [`TemperatureOutOfRange`](crate::NjoyError::TemperatureOutOfRange) outside
//! the tabulated range.
//!
//! The scattered neutron leaves with the **same energy** (elastic) at the
//! discrete Bragg cosines `μ_i = 1 − 2·E_i/E` for every edge `E_i ≤ E`, each
//! weighted by its structure-factor increment `S(E_i) − S(E_{i−1})`. This module
//! provides the cross section and those discrete angular weights — the inputs the
//! thermal ACE ITCE/ITCA blocks and the [`super::scattering`] consumer surface
//! need.

use super::mf7::{interp_s_temperature, select_temperature, CoherentElastic, TemperatureSelection};
use crate::NjoyError;

impl CoherentElastic {
    /// Base temperature `T₀` \[K\] of the evaluation (the lowest tabulated
    /// temperature; 296 K for the ENDF/B-VIII.0 graphite evaluations).
    pub fn base_temperature_k(&self) -> f64 {
        self.temperatures_k.first().copied().unwrap_or(0.0)
    }

    /// Resolve `temp_k` \[K\] against this evaluation's tabulated temperatures
    /// (tolerance-match → interpolate → refuse; see the
    /// [`mf7` module docs](super::mf7)).
    ///
    /// # Errors
    /// [`NjoyError::TemperatureOutOfRange`] outside the tabulated range.
    pub fn select(&self, temp_k: f64) -> Result<TemperatureSelection, NjoyError> {
        select_temperature(&self.temperatures_k, &self.temp_interp, temp_k)
    }

    /// The temperature \[K\] evaluation at `temp_k` actually represents: the
    /// tabulated value when `temp_k` matches one within the NJOY tolerance,
    /// otherwise `temp_k` itself (interpolated). Callers that must report the
    /// physical temperature of their data should use this, not the request.
    ///
    /// # Errors
    /// [`NjoyError::TemperatureOutOfRange`] outside the tabulated range.
    pub fn resolved_temperature_k(&self, temp_k: f64) -> Result<f64, NjoyError> {
        Ok(self
            .select(temp_k)?
            .resolved_temperature_k(&self.temperatures_k))
    }

    /// Cumulative structure factor `S(E_i)` \[eV·b\] at one Bragg-edge index for
    /// an already-resolved temperature selection.
    fn s_at_index(&self, i: usize, sel: &TemperatureSelection) -> f64 {
        match *sel {
            TemperatureSelection::Tabulated(j) => self.s_tables[j][i],
            TemperatureSelection::Interpolated {
                lo,
                hi,
                li,
                target_k,
            } => interp_s_temperature(
                self.temperatures_k[lo],
                self.s_tables[lo][i],
                self.temperatures_k[hi],
                self.s_tables[hi][i],
                target_k,
                li,
            ),
        }
    }

    /// The `(E_i \[eV\], S(E_i) \[eV·b\])` Bragg-edge table at temperature
    /// `temp_k` \[K\] — tabulated, or `LI`-law interpolated between the two
    /// bracketing tabulated temperatures.
    ///
    /// # Errors
    /// [`NjoyError::TemperatureOutOfRange`] outside the tabulated range.
    pub fn s_of_e_at(&self, temp_k: f64) -> Result<Vec<(f64, f64)>, NjoyError> {
        let sel = self.select(temp_k)?;
        Ok(self
            .bragg_energies_ev
            .iter()
            .enumerate()
            .map(|(i, &e)| (e, self.s_at_index(i, &sel)))
            .collect())
    }

    /// Coherent-elastic cross section \[barn\] at incident energy `e_ev` \[eV\]
    /// and temperature `temp_k` \[K\].
    ///
    /// Returns `0` below the first Bragg edge; otherwise `S(E, T)/E` with `S`
    /// the cumulative structure factor held as a step function in `E` and
    /// resolved in `T` per the module temperature policy. Valid over the
    /// evaluation's tabulated temperature range (296–2000 K for ENDF/B-VIII.0
    /// graphite) and any positive energy.
    ///
    /// # Errors
    /// [`NjoyError::TemperatureOutOfRange`] outside the tabulated range.
    pub fn cross_section(&self, e_ev: f64, temp_k: f64) -> Result<f64, NjoyError> {
        let sel = self.select(temp_k)?;
        if e_ev <= 0.0 {
            return Ok(0.0);
        }
        // Largest Bragg edge ≤ e gives the current cumulative S (histogram).
        let n_below = self.bragg_energies_ev.partition_point(|&edge| edge <= e_ev);
        if n_below == 0 {
            return Ok(0.0); // below the first Bragg edge
        }
        Ok(self.s_at_index(n_below - 1, &sel) / e_ev)
    }

    /// The discrete Bragg reflections active at incident energy `e_ev` \[eV\]
    /// and temperature `temp_k` \[K\]: `(μ_i, weight_i)` where
    /// `μ_i = 1 − 2·E_i/E` is the cosine of the scattering angle for edge `E_i`
    /// and `weight_i = S(E_i) − S(E_{i−1})` is that edge's structure-factor
    /// increment \[eV·b\] (its share of the coherent cross section; the weights
    /// sum to `S(E, T) = σ_coh·E`).
    ///
    /// The neutron scatters elastically (same outgoing energy) to one of these
    /// cosines with probability proportional to the weight. Empty below the
    /// first Bragg edge.
    ///
    /// # Errors
    /// [`NjoyError::TemperatureOutOfRange`] outside the tabulated range.
    pub fn bragg_reflections(&self, e_ev: f64, temp_k: f64) -> Result<Vec<(f64, f64)>, NjoyError> {
        let sel = self.select(temp_k)?;
        let mut out = Vec::new();
        let mut prev_s = 0.0;
        for (i, &edge) in self.bragg_energies_ev.iter().enumerate() {
            if edge > e_ev {
                break;
            }
            let s = self.s_at_index(i, &sel);
            let mu = 1.0 - 2.0 * edge / e_ev;
            out.push((mu, s - prev_s));
            prev_s = s;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::super::mf7::parse_mf7;
    use crate::endf::tape::Tape;
    use std::fs::File;

    fn al27_coherent() -> super::CoherentElastic {
        let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        p.push("tests/resources/tsl-013_Al_027-ENDF8.0.endf");
        let tape = Tape::read(File::open(p).unwrap()).unwrap();
        parse_mf7(&tape, 53).unwrap().coherent_elastic.unwrap()
    }

    #[test]
    fn zero_below_first_bragg_edge() {
        let ce = al27_coherent();
        let t0 = ce.base_temperature_k();
        let first_edge = ce.bragg_energies_ev[0];
        assert_eq!(
            ce.cross_section(first_edge * 0.5, t0).unwrap(),
            0.0,
            "no coherent σ below edge 1"
        );
        assert!(
            ce.cross_section(first_edge * 1.001, t0).unwrap() > 0.0,
            "σ turns on at edge 1"
        );
    }

    #[test]
    fn cross_section_is_s_over_e_and_sawtooths() {
        let ce = al27_coherent();
        let t0 = ce.base_temperature_k();
        // Just above the top edge, σ·E must equal the total cumulative S.
        let top_edge = *ce.bragg_energies_ev.last().unwrap();
        let top_s = *ce.s_tables[0].last().unwrap();
        let e = top_edge * 1.5;
        assert!(
            (ce.cross_section(e, t0).unwrap() * e - top_s).abs() < 1e-6 * top_s,
            "σ = S/E"
        );
        // Within a single Bragg interval σ falls as 1/E (S constant), so σ·E is flat.
        let (e1, e2) = (top_edge * 1.1, top_edge * 1.4);
        assert!(
            (ce.cross_section(e1, t0).unwrap() * e1 - ce.cross_section(e2, t0).unwrap() * e2).abs()
                < 1e-6 * top_s,
            "S constant between edges ⇒ σ ∝ 1/E"
        );
    }

    #[test]
    fn bragg_reflection_cosines_are_physical() {
        let ce = al27_coherent();
        let t0 = ce.base_temperature_k();
        let top_edge = *ce.bragg_energies_ev.last().unwrap();
        let e = top_edge * 1.2;
        let refl = ce.bragg_reflections(e, t0).unwrap();
        assert!(refl.len() > 5, "many active Bragg edges above the top");
        // Every cosine in [-1, 1]; weights non-negative and summing to S(E).
        assert!(
            refl.iter().all(|&(mu, _)| (-1.0..=1.0).contains(&mu)),
            "μ ∈ [-1,1]"
        );
        assert!(refl.iter().all(|&(_, w)| w >= -1e-12), "weights ≥ 0");
        let sum_w: f64 = refl.iter().map(|&(_, w)| w).sum();
        assert!(
            (sum_w - ce.cross_section(e, t0).unwrap() * e).abs() < 1e-6 * sum_w,
            "Σ weights = S(E)"
        );
    }

    /// The Debye-Waller factor suppresses S(E, T) as temperature rises, so at
    /// fixed energy the coherent-elastic cross section must **decrease** with
    /// temperature across every tabulated temperature of the Al-27 fixture.
    /// (The decisive graphite version of this check, with measured numbers,
    /// lives in `tests/thermal_graphite_coherent.rs`.)
    #[test]
    fn cross_section_decreases_with_temperature() {
        let ce = al27_coherent();
        assert!(
            ce.temperatures_k.len() > 1,
            "fixture must tabulate several T"
        );
        let e = *ce.bragg_energies_ev.last().unwrap() * 1.5;
        let sigmas: Vec<f64> = ce
            .temperatures_k
            .iter()
            .map(|&t| ce.cross_section(e, t).unwrap())
            .collect();
        assert!(
            sigmas.windows(2).all(|w| w[1] < w[0]),
            "σ_coh_el(E, T) must fall with T (Debye-Waller): {sigmas:?} at {:?} K",
            ce.temperatures_k
        );
    }

    /// Temperature interpolation: between two tabulated temperatures the
    /// interpolated σ must lie strictly between the bracketing tabulated
    /// values (S is monotone-decreasing in T at fixed E).
    #[test]
    fn interpolated_cross_section_is_bracketed() {
        let ce = al27_coherent();
        let (t_lo, t_hi) = (ce.temperatures_k[0], ce.temperatures_k[1]);
        let t_mid = 0.5 * (t_lo + t_hi);
        assert!(
            (t_mid - t_lo) > super::super::mf7::temperature_match_tolerance_k(t_mid),
            "midpoint must be beyond the snap tolerance for this test to bite"
        );
        let e = *ce.bragg_energies_ev.last().unwrap() * 1.5;
        let (s_lo, s_hi) = (
            ce.cross_section(e, t_lo).unwrap(),
            ce.cross_section(e, t_hi).unwrap(),
        );
        let s_mid = ce.cross_section(e, t_mid).unwrap();
        assert!(
            s_hi < s_mid && s_mid < s_lo,
            "σ({t_mid} K) = {s_mid} must lie between σ({t_hi} K) = {s_hi} and σ({t_lo} K) = {s_lo}"
        );
    }

    /// Out-of-range temperatures are refused, never snapped.
    #[test]
    fn out_of_range_temperature_is_an_error() {
        let ce = al27_coherent();
        let below = ce.temperatures_k.first().unwrap() - 50.0;
        let above = ce.temperatures_k.last().unwrap() + 500.0;
        for bad in [below, above] {
            assert!(
                matches!(
                    ce.cross_section(0.01, bad),
                    Err(crate::NjoyError::TemperatureOutOfRange { .. })
                ),
                "must refuse {bad} K"
            );
        }
    }
}
