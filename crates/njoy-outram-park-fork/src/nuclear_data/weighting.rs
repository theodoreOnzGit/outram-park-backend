//! Group-collapse **weighting spectra** for the fast-range multigroup set.
//!
//! When the smooth fast range above a nuclide's WMP `e_max` is collapsed to a
//! handful of constants ([`super::Mgxs::collapse`]), each group constant is a
//! **flux-weighted average** σ_g = ∫σ(E)φ(E)dE / ∫φ(E)dE. The weight φ(E) is the
//! assumed neutron flux shape *inside* each group — the group constant is only as
//! representative as that assumed shape.
//!
//! This module holds the closed set of weighting shapes the offline bake can use.
//! It is intentionally a small, human-navigable menu: pick the one that matches
//! the spectrum your problem actually lives in.
//!
//! # Which weight for which problem
//!
//! | Variant | Shape φ(E) | Physical regime |
//! |---|---|---|
//! | [`WeightingSpectrum::Watt`] | `e^{−E/a}·sinh(√(bE))` | **fast / unmoderated** — a bare metal sphere, a fast reactor. The default. |
//! | [`WeightingSpectrum::OneOverE`] | `1/E` | **epithermal / slowing-down** — the flat-lethargy region of a moderated lattice between fission and thermal. |
//! | [`WeightingSpectrum::Maxwellian`] | `E·e^{−E/kT}` | **thermal** — a well-moderated thermal system. |
//!
//! # Fidelity contract (unchanged)
//!
//! Choosing a weight does **not** add self-shielding. There is still no Boltzmann
//! solve, no Bondarenko dilution, no URR treatment — the weight is a fixed assumed
//! shape, not a solved flux. A softer weight simply produces a *different baked
//! set* better matched to a moderated spectrum; the runtime lookup
//! ([`super::Mgxs::micro`]) stays a piecewise-constant, temperature-independent
//! index either way. Callers needing a solved flux use the HIGH tier
//! (on-device ENDF reprocessing).

/// The neutron-flux shape used to weight a fast-range group collapse.
///
/// A closed enum (workspace rule: no trait objects). Each variant supplies its
/// unnormalized weight through [`WeightingSpectrum::weight`]; absolute
/// normalization is irrelevant to a weighted average, so only the *shape* matters.
///
/// See the module docs for a table of which regime each variant models. The
/// [`Default`] is [`WeightingSpectrum::Watt`] with the U-235 thermal-fission
/// parameters — the right choice for the fast bare-sphere problems this crate
/// ships CORE data for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeightingSpectrum {
    /// Watt fission spectrum `φ(E) ∝ e^{−E/a}·sinh(√(b·E))`.
    ///
    /// The hard, unmoderated fast spectrum. `a` \[eV\], `b` \[eV⁻¹\].
    Watt {
        /// Watt parameter `a` \[eV\].
        a: f64,
        /// Watt parameter `b` \[eV⁻¹\].
        b: f64,
    },
    /// 1/E slowing-down (constant-lethargy) flux, `φ(E) ∝ 1/E`.
    ///
    /// The epithermal weight for the flat region of a moderated spectrum between
    /// the fission source and the thermal peak.
    OneOverE,
    /// Maxwellian thermal flux `φ(E) ∝ E·e^{−E/kT}`.
    ///
    /// A well-moderated thermal spectrum. `temp_ev` is the moderator temperature
    /// kT \[eV\] (e.g. 0.0253 eV ≈ 293.6 K). Note: above every nuclide's WMP
    /// `e_max` the fast range is well past the thermal peak, so this weight only
    /// makes sense if the fast group structure is extended down into the thermal
    /// range — it is offered for completeness and moderated group-set experiments.
    Maxwellian {
        /// Moderator temperature kT \[eV\].
        temp_ev: f64,
    },
}

impl Default for WeightingSpectrum {
    /// A representative fast-fission Watt weight (U-235 thermal-fission parameters,
    /// `a = 0.988 MeV`, `b = 2.249 × 10⁻⁶ eV⁻¹`) — the correct default for the fast
    /// bare-sphere spectra this crate's CORE data targets.
    fn default() -> Self {
        WeightingSpectrum::Watt {
            a: 0.988e6,
            b: 2.249e-6,
        }
    }
}

impl WeightingSpectrum {
    /// Unnormalized flux weight φ(E) at incident energy `e` \[eV\].
    ///
    /// This is the weighting function for the fast-range MGXS collapse
    /// ([`super::Mgxs::collapse`]). Absolute normalization is irrelevant — only the
    /// shape matters for a weighted average — so each form is returned without its
    /// leading constant. Returns `0.0` for non-positive energies.
    pub fn weight(&self, e: f64) -> f64 {
        if e <= 0.0 {
            return 0.0;
        }
        match *self {
            WeightingSpectrum::Watt { a, b } => (-e / a).exp() * (b * e).sqrt().sinh(),
            WeightingSpectrum::OneOverE => 1.0 / e,
            WeightingSpectrum::Maxwellian { temp_ev } => {
                if temp_ev <= 0.0 {
                    0.0
                } else {
                    e * (-e / temp_ev).exp()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default is the fast Watt weight, non-negative and peaked in the
    /// hundreds-of-keV to ~MeV range (not monotone).
    #[test]
    fn watt_default_is_peaked_and_nonnegative() {
        let w = WeightingSpectrum::default();
        let at_low = w.weight(1.0e4);
        let at_peak = w.weight(7.0e5);
        let at_high = w.weight(1.5e7);
        assert!(at_low >= 0.0 && at_peak >= 0.0 && at_high >= 0.0);
        assert!(at_peak > at_low, "Watt rises off the low tail");
        assert!(at_peak > at_high, "Watt falls off the high tail");
    }

    /// 1/E is monotonically decreasing and positive.
    #[test]
    fn one_over_e_is_monotone_decreasing() {
        let w = WeightingSpectrum::OneOverE;
        assert!(w.weight(1.0e3) > w.weight(1.0e4));
        assert!(w.weight(1.0e4) > w.weight(1.0e6));
        assert!(w.weight(1.0) > 0.0);
        assert_eq!(w.weight(0.0), 0.0);
        assert_eq!(w.weight(-5.0), 0.0);
    }

    /// The Maxwellian flux φ(E) ∝ E·e^{−E/kT} peaks at E = kT.
    #[test]
    fn maxwellian_peaks_at_kt() {
        let kt = 0.0253;
        let w = WeightingSpectrum::Maxwellian { temp_ev: kt };
        let at_peak = w.weight(kt);
        assert!(at_peak > w.weight(0.5 * kt));
        assert!(at_peak > w.weight(2.0 * kt));
        // Degenerate temperature yields no weight rather than a NaN.
        assert_eq!(
            WeightingSpectrum::Maxwellian { temp_ev: 0.0 }.weight(kt),
            0.0
        );
    }
}
