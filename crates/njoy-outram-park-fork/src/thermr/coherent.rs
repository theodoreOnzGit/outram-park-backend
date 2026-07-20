//! Coherent-elastic (Bragg) thermal scattering — the first **THERMR** computation.
//!
//! For a crystalline bound scatterer (graphite, Be, Al, …) the coherent-elastic
//! cross section is a sum of Bragg reflections. ENDF MF=7/MT=2 (`LTHR=1`) stores
//! the **cumulative structure factor** `S(E)`: a step function that jumps up at
//! each Bragg edge `E_i`. The cross section is then
//!
//! ```text
//!   σ_coh(E) = S(E) / E,   with S(E) = S(E_i) for  E_i ≤ E < E_{i+1},
//! ```
//!
//! i.e. below the first Bragg edge the coherent-elastic cross section is zero,
//! then it follows a `1/E` sawtooth that steps up at every edge. Ported from the
//! coherent-elastic branch of `thermr.f90` (`coh`/`sigcoh`).
//!
//! The scattered neutron leaves with the **same energy** (elastic) at the
//! discrete Bragg cosines `μ_i = 1 − 2·E_i/E` for every edge `E_i ≤ E`, each
//! weighted by its structure-factor increment `S(E_i) − S(E_{i−1})`. This module
//! provides the cross section and those discrete angular weights — the inputs the
//! thermal ACE ITCE/ITCA blocks need.

use super::mf7::CoherentElastic;

impl CoherentElastic {
    /// Coherent-elastic cross section \[barn\] at incident energy `e` \[eV\].
    ///
    /// Returns `0` below the first Bragg edge; otherwise `S(E)/E` with `S` the
    /// cumulative structure factor held as a step function.
    pub fn cross_section(&self, e_ev: f64) -> f64 {
        if e_ev <= 0.0 {
            return 0.0;
        }
        // Largest Bragg edge ≤ e gives the current cumulative S (histogram).
        let s = self
            .s_of_e
            .iter()
            .take_while(|&&(edge, _)| edge <= e_ev)
            .last()
            .map(|&(_, s)| s);
        match s {
            Some(s) => s / e_ev,
            None => 0.0, // below the first Bragg edge
        }
    }

    /// The discrete Bragg reflections active at incident energy `e` \[eV\]:
    /// `(μ_i, weight_i)` where `μ_i = 1 − 2·E_i/E` is the cosine of the scattering
    /// angle for edge `E_i` and `weight_i = S(E_i) − S(E_{i−1})` is that edge's
    /// structure-factor increment (its share of the coherent cross section).
    ///
    /// The neutron scatters elastically (same outgoing energy) to one of these
    /// cosines with probability proportional to the weight.
    pub fn bragg_reflections(&self, e_ev: f64) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        let mut prev_s = 0.0;
        for &(edge, s) in &self.s_of_e {
            if edge > e_ev {
                break;
            }
            let mu = 1.0 - 2.0 * edge / e_ev;
            out.push((mu, s - prev_s));
            prev_s = s;
        }
        out
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
        let first_edge = ce.s_of_e[0].0;
        assert_eq!(ce.cross_section(first_edge * 0.5), 0.0, "no coherent σ below edge 1");
        assert!(ce.cross_section(first_edge * 1.001) > 0.0, "σ turns on at edge 1");
    }

    #[test]
    fn cross_section_is_s_over_e_and_sawtooths() {
        let ce = al27_coherent();
        // Just above the top edge, σ·E must equal the total cumulative S.
        let (top_edge, top_s) = *ce.s_of_e.last().unwrap();
        let e = top_edge * 1.5;
        assert!((ce.cross_section(e) * e - top_s).abs() < 1e-6 * top_s, "σ = S/E");
        // Within a single Bragg interval σ falls as 1/E (S constant), so σ·E is flat.
        let (e1, e2) = (top_edge * 1.1, top_edge * 1.4);
        assert!(
            (ce.cross_section(e1) * e1 - ce.cross_section(e2) * e2).abs() < 1e-6 * top_s,
            "S constant between edges ⇒ σ ∝ 1/E"
        );
    }

    #[test]
    fn bragg_reflection_cosines_are_physical() {
        let ce = al27_coherent();
        let (top_edge, _) = *ce.s_of_e.last().unwrap();
        let e = top_edge * 1.2;
        let refl = ce.bragg_reflections(e);
        assert!(refl.len() > 5, "many active Bragg edges above the top");
        // Every cosine in [-1, 1]; weights non-negative and summing to S(E).
        assert!(refl.iter().all(|&(mu, _)| (-1.0..=1.0).contains(&mu)), "μ ∈ [-1,1]");
        assert!(refl.iter().all(|&(_, w)| w >= -1e-12), "weights ≥ 0");
        let sum_w: f64 = refl.iter().map(|&(_, w)| w).sum();
        assert!((sum_w - ce.cross_section(e) * e).abs() < 1e-6 * sum_w, "Σ weights = S(E)");
    }
}
