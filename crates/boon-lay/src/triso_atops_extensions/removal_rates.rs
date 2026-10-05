// SPDX-License-Identifier: GPL-3.0
//
// NOT a port (gh:#583, 2026-10-05). Element-dependent plate-out and
// purification rates for the TRISO-ATOPS coolant pools. The routing it
// generalises is upstream's (`trisoatops.py`, lines 118-124, commit de374c8),
// as ported in `crate::triso_atops_fork::normal_operation`; that code is not
// modified. [`RemovalRates::upstream`] reproduces it exactly.

//! # Removal rates per transport group or per element
//!
//! Upstream TRISO-ATOPS removes atoms from the circulating helium with one
//! plate-out constant `k_plate` and one purification (clean-up) constant
//! `k_clean` per reactor, routed by transport group:
//!
//! | group | `k_plate` | `k_clean` |
//! |---|---|---|
//! | noble gases | 0 | `k_clean` |
//! | halogens | `k_plate` | `k_clean` |
//! | special metals (Rb, Sr, Cs, Ba, Eu), silver (Ag, Pd), other | `k_plate` | 0 |
//!
//! [`RemovalRates`] holds a separate pair for each group, and optionally for
//! single elements (by atomic number `Z`). An element override wins over its
//! group. [`RemovalRates::upstream`] fills every group the way upstream does.
//!
//! ## From per-cycle data to rate constants
//!
//! Plant data often come as a **fraction removed per circuit cycle** (a
//! plate-out fraction) or a **purification efficiency** applied to the part
//! of the flow that is purified. With the helium well mixed and each pass
//! independent, a fraction `f` removed every cycle of length `t_c` is a
//! first-order rate:
//!
//! ```text
//! k = -ln(1 - f) / t_c          (-> f / t_c for small f)
//! ```
//!
//! For purification, the fraction removed per cycle is the efficiency `η` times
//! the fraction `φ` of the circuit flow sent through the purification system,
//! so `f = η φ` ([`purification_rate`]). Both conversions are this module's
//! modelling assumptions, stated here; neither comes from upstream.

use uom::si::f64::{Frequency, Ratio, Time};
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;
use uom::si::time::second;

use crate::triso_atops_fork::activities::live_pools::PoolRates;
use crate::triso_atops_fork::nuclide_model::ElementGroup;

/// One plate-out and one purification rate constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupRates {
    /// Plate-out rate constant `k_plate` (`s^-1`).
    pub k_plate: Frequency,
    /// Purification (clean-up) rate constant `k_clean` (`s^-1`).
    pub k_clean: Frequency,
}

impl GroupRates {
    /// Both rates in `s^-1`.
    #[must_use]
    pub fn per_second(k_plate: f64, k_clean: f64) -> Self {
        Self {
            k_plate: Frequency::new::<hertz>(k_plate),
            k_clean: Frequency::new::<hertz>(k_clean),
        }
    }

    /// No plate-out and no purification.
    #[must_use]
    pub fn zero() -> Self {
        Self::per_second(0.0, 0.0)
    }
}

/// Plate-out and purification rates for every TRISO-ATOPS transport group,
/// with optional per-element overrides. Build it with [`Self::upstream`] and
/// change what the question needs with [`Self::with_group`] and
/// [`Self::with_element`].
#[derive(Debug, Clone, PartialEq)]
pub struct RemovalRates {
    noble_gas: GroupRates,
    halogen: GroupRates,
    special_metal: GroupRates,
    silver: GroupRates,
    other: GroupRates,
    /// `(Z, rates)`; the last entry for a `Z` wins.
    elements: Vec<(u32, GroupRates)>,
}

impl RemovalRates {
    /// **Upstream's model**: one `k_plate` and one `k_clean` per reactor,
    /// routed as `trisoatops.py`, lines 118-124 (commit `de374c8`) routes them.
    /// Noble gases do not plate out; only noble gases and halogens are
    /// purified. With the purification system switched off upstream
    /// (`hps_tog = false`), use [`Self::without_purification`] on the result.
    #[must_use]
    pub fn upstream(k_plate: Frequency, k_clean: Frequency) -> Self {
        let zero = Frequency::new::<hertz>(0.0);
        Self {
            noble_gas: GroupRates { k_plate: zero, k_clean },
            halogen: GroupRates { k_plate, k_clean },
            special_metal: GroupRates { k_plate, k_clean: zero },
            silver: GroupRates { k_plate, k_clean: zero },
            other: GroupRates { k_plate, k_clean: zero },
            elements: Vec::new(),
        }
    }

    /// The same rates with every `k_clean` set to 0: no purification system
    /// (upstream's `hps_tog = false`).
    #[must_use]
    pub fn without_purification(mut self) -> Self {
        let zero = Frequency::new::<hertz>(0.0);
        for g in [
            &mut self.noble_gas,
            &mut self.halogen,
            &mut self.special_metal,
            &mut self.silver,
            &mut self.other,
        ] {
            g.k_clean = zero;
        }
        for (_, r) in &mut self.elements {
            r.k_clean = zero;
        }
        self
    }

    /// Replace one group's rates.
    #[must_use]
    pub fn with_group(mut self, group: ElementGroup, rates: GroupRates) -> Self {
        *self.group_mut(group) = rates;
        self
    }

    /// Give one element (atomic number `z`) its own rates, overriding its
    /// group's. A second call for the same `z` replaces the first.
    #[must_use]
    pub fn with_element(mut self, z: u32, rates: GroupRates) -> Self {
        self.elements.retain(|(e, _)| *e != z);
        self.elements.push((z, rates));
        self
    }

    /// The rates of one group.
    #[must_use]
    pub fn group(&self, group: ElementGroup) -> GroupRates {
        match group {
            ElementGroup::NobleGas => self.noble_gas,
            ElementGroup::Halogen => self.halogen,
            ElementGroup::SpecialMetal => self.special_metal,
            ElementGroup::Silver => self.silver,
            ElementGroup::Other => self.other,
        }
    }

    fn group_mut(&mut self, group: ElementGroup) -> &mut GroupRates {
        match group {
            ElementGroup::NobleGas => &mut self.noble_gas,
            ElementGroup::Halogen => &mut self.halogen,
            ElementGroup::SpecialMetal => &mut self.special_metal,
            ElementGroup::Silver => &mut self.silver,
            ElementGroup::Other => &mut self.other,
        }
    }

    /// The rates that apply to element `z`: its own override if it has one,
    /// else its transport group's ([`ElementGroup::from_atomic_number`]).
    #[must_use]
    pub fn for_element(&self, z: u32) -> GroupRates {
        self.elements
            .iter()
            .find(|(e, _)| *e == z)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| self.group(ElementGroup::from_atomic_number(z)))
    }

    /// The [`PoolRates`] for stepping element `z`'s pools live with
    /// [`crate::triso_atops_fork::activities::live_pools::step`]: this
    /// element's plate-out and purification rates, its decay constant and a
    /// primary-circuit leak `k_leak` (one rate for every element; the leak is
    /// `live_pools`' own extension, upstream has none).
    #[must_use]
    pub fn pool_rates(&self, z: u32, decay_constant: Frequency, k_leak: Frequency) -> PoolRates {
        let r = self.for_element(z);
        PoolRates {
            decay: decay_constant.get::<hertz>(),
            plate_out: r.k_plate.get::<hertz>(),
            clean_up: r.k_clean.get::<hertz>(),
            leak: k_leak.get::<hertz>(),
        }
    }

    /// Rates from **per-cycle** data, element by element.
    ///
    /// For each element listed in `data`: plate-out
    /// `k_plate = rate_from_fraction_per_cycle(plate_out, cycle_time)`, and
    /// purification `k_clean = purification_rate(efficiency,
    /// purified_flow_fraction, cycle_time)`. An element that `data` lists for
    /// only one of the two keeps `fallback`'s value for the other, and an
    /// element `data` does not list keeps `fallback`'s rates entirely; both
    /// are reported in the returned list, so a caller can say which numbers
    /// are cited and which are not.
    ///
    /// `cycle_time` (the helium's time round the primary circuit) and
    /// `purified_flow_fraction` (the fraction of the circuit flow sent through
    /// the purification system each cycle) are **required**: no default is
    /// given because the workspace has no cited value for either (gh:#583).
    ///
    /// # Returns
    /// The rates, and the `(Z, what)` pairs that fell back, `what` being
    /// `"plate-out"` or `"purification"`.
    ///
    /// # Panics
    /// If a fraction is outside `[0, 1)` or `cycle_time` is not positive
    /// (see [`rate_from_fraction_per_cycle`]).
    #[must_use]
    pub fn from_per_cycle(
        data: &[PerCycleRemoval],
        cycle_time: Time,
        purified_flow_fraction: Ratio,
        fallback: &RemovalRates,
    ) -> (Self, Vec<(u32, &'static str)>) {
        let mut out = fallback.clone();
        let mut fell_back = Vec::new();
        for d in data {
            let base = fallback.for_element(d.z);
            let k_plate = match d.plate_out_per_cycle {
                Some(f) => rate_from_fraction_per_cycle(Ratio::new::<ratio>(f), cycle_time),
                None => {
                    fell_back.push((d.z, "plate-out"));
                    base.k_plate
                }
            };
            let k_clean = match d.purification_efficiency {
                Some(e) => purification_rate(Ratio::new::<ratio>(e), purified_flow_fraction, cycle_time),
                None => {
                    fell_back.push((d.z, "purification"));
                    base.k_clean
                }
            };
            out = out.with_element(d.z, GroupRates { k_plate, k_clean });
        }
        (out, fell_back)
    }
}

/// `k = -ln(1 - f) / t_c`: the first-order rate that removes the fraction `f`
/// every cycle of length `t_c`, for a well-mixed circuit whose passes are
/// independent (this module's assumption, stated in the module doc).
///
/// # Panics
/// If `f` is outside `[0, 1)` (a fraction of 1 per cycle is an infinite rate)
/// or `t_c` is not positive: a caller error, not a physical state.
#[must_use]
pub fn rate_from_fraction_per_cycle(fraction: Ratio, cycle_time: Time) -> Frequency {
    let f = fraction.get::<ratio>();
    let t = cycle_time.get::<second>();
    assert!((0.0..1.0).contains(&f), "fraction per cycle {f} outside [0, 1)");
    assert!(t > 0.0 && t.is_finite(), "cycle time {t} s is not positive");
    Frequency::new::<hertz>(-(-f).ln_1p() / t)
}

/// The purification rate when a fraction `φ` of the circuit flow passes
/// through the purification system each cycle and it removes a fraction `η`
/// (its efficiency) of what passes: `f = η φ` per cycle, then
/// [`rate_from_fraction_per_cycle`].
///
/// # Panics
/// As [`rate_from_fraction_per_cycle`], and if `η` or `φ` is outside `[0, 1]`.
#[must_use]
pub fn purification_rate(efficiency: Ratio, purified_flow_fraction: Ratio, cycle_time: Time) -> Frequency {
    let (eta, phi) = (efficiency.get::<ratio>(), purified_flow_fraction.get::<ratio>());
    assert!((0.0..=1.0).contains(&eta), "efficiency {eta} outside [0, 1]");
    assert!((0.0..=1.0).contains(&phi), "purified flow fraction {phi} outside [0, 1]");
    rate_from_fraction_per_cycle(Ratio::new::<ratio>(eta * phi), cycle_time)
}

/// Per-cycle removal data for one element.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerCycleRemoval {
    /// Element symbol, for reports.
    pub element: &'static str,
    /// Atomic number.
    pub z: u32,
    /// Fraction plated out per circuit cycle, if the source gives one.
    pub plate_out_per_cycle: Option<f64>,
    /// Purification efficiency (fraction of what passes through the
    /// purification system that it removes), if the source gives one.
    pub purification_efficiency: Option<f64>,
}

/// **Liu & Cao (2002), HTR-10, per-cycle removal data.**
///
/// Source: Liu Y. and Cao J., NED 218 (2002) 81-90, Section 2.4.1, as recorded
/// in `crates/changi/docs/References.md` (the paper's basis for its
/// normal-operation release, citing the authors' earlier reference, Liu
/// Yuanzhong 1994):
///
/// - plate-out per cycle: 30 % for Rb and Sr, 50 % for Ag and Cs, 20 % for I;
/// - purification efficiency: 99 % for I, Kr, Xe, C and H-3, 90 % for Sr, Ag,
///   Cs and Rb (set conservatively, per the source).
///
/// Elements the source does not list are absent, not zero. Noble gases have
/// no plate-out figure: [`RemovalRates::from_per_cycle`] keeps the fallback's
/// value for them (0 in [`RemovalRates::upstream`]).
#[must_use]
pub fn liu_cao_2002_htr10() -> Vec<PerCycleRemoval> {
    let e = |element, z, plate: Option<f64>, eff: Option<f64>| PerCycleRemoval {
        element,
        z,
        plate_out_per_cycle: plate,
        purification_efficiency: eff,
    };
    vec![
        e("H", 1, None, Some(0.99)),
        e("C", 6, None, Some(0.99)),
        e("Kr", 36, None, Some(0.99)),
        e("Rb", 37, Some(0.30), Some(0.90)),
        e("Sr", 38, Some(0.30), Some(0.90)),
        e("Ag", 47, Some(0.50), Some(0.90)),
        e("I", 53, Some(0.20), Some(0.99)),
        e("Xe", 54, None, Some(0.99)),
        e("Cs", 55, Some(0.50), Some(0.90)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hz(v: f64) -> Frequency {
        Frequency::new::<hertz>(v)
    }

    /// **The upstream table, exactly.** Methodology: `upstream(7.5e-5,
    /// 8.77e-5)` (Stoyer et al. 2026 Case A, Table 3) queried for one element
    /// of each group: Kr (noble gas), I (halogen), Cs and Sr (special metals),
    /// Ag (silver), Zr (other). Expected: upstream's routing,
    /// `trisoatops.py` lines 118-124. **Result (2026-10-05):** exact.
    #[test]
    fn upstream_routes_as_trisoatops() {
        let r = RemovalRates::upstream(hz(7.5e-5), hz(8.77e-5));
        assert_eq!(r.for_element(36), GroupRates::per_second(0.0, 8.77e-5));
        assert_eq!(r.for_element(53), GroupRates::per_second(7.5e-5, 8.77e-5));
        for z in [55, 38, 47, 40] {
            assert_eq!(r.for_element(z), GroupRates::per_second(7.5e-5, 0.0), "Z {z}");
        }
        let off = r.without_purification();
        assert_eq!(off.for_element(53), GroupRates::per_second(7.5e-5, 0.0));
        assert_eq!(off.for_element(36), GroupRates::zero());
    }

    /// An element override wins over its group; a group change reaches every
    /// element of the group without one. **Result (2026-10-05):** passes.
    #[test]
    fn element_overrides_its_group() {
        let r = RemovalRates::upstream(hz(7.5e-5), hz(8.77e-5))
            .with_group(ElementGroup::SpecialMetal, GroupRates::per_second(2e-4, 1e-5))
            .with_element(55, GroupRates::per_second(3e-4, 0.0))
            .with_element(55, GroupRates::per_second(4e-4, 0.0));
        assert_eq!(r.for_element(55), GroupRates::per_second(4e-4, 0.0));
        assert_eq!(r.for_element(38), GroupRates::per_second(2e-4, 1e-5));
        assert_eq!(r.for_element(53), GroupRates::per_second(7.5e-5, 8.77e-5));
    }

    /// **The per-cycle conversion.** Methodology: a fraction 0.5 per 10 s cycle
    /// must give ln 2 / 10; a small fraction must approach f / t_c; f = 0 gives
    /// 0; purification at efficiency 0.9 on 2 % of the flow equals the
    /// per-cycle rate of 0.018. Pass: 1e-15 relative. **Result (2026-10-05):**
    /// passes.
    #[test]
    fn per_cycle_fractions_become_first_order_rates() {
        let t = Time::new::<second>(10.0);
        let k = |f: f64| rate_from_fraction_per_cycle(Ratio::new::<ratio>(f), t).get::<hertz>();
        assert!((k(0.5) - std::f64::consts::LN_2 / 10.0).abs() < 1e-15);
        assert!((k(1e-9) / 1e-10 - 1.0).abs() < 1e-8);
        assert_eq!(k(0.0), 0.0);
        let p = purification_rate(Ratio::new::<ratio>(0.9), Ratio::new::<ratio>(0.02), t).get::<hertz>();
        assert!((p / k(0.018) - 1.0).abs() < 1e-15);
    }

    /// A fraction of 1 per cycle is an infinite rate: refused, not returned.
    #[test]
    #[should_panic(expected = "outside [0, 1)")]
    fn a_whole_cycle_fraction_is_refused() {
        let _ = rate_from_fraction_per_cycle(Ratio::new::<ratio>(1.0), Time::new::<second>(1.0));
    }

    /// **The Liu & Cao table is the one recorded in changi's References.md.**
    /// Methodology: every figure of the recorded basis (plate-out per cycle 30
    /// % Rb, Sr; 50 % Ag, Cs; 20 % I; purification 99 % I, Kr, Xe, C, H-3; 90 %
    /// Sr, Ag, Cs, Rb) is checked by element. **Result (2026-10-05):** passes.
    #[test]
    fn liu_cao_table_matches_the_record() {
        let t = liu_cao_2002_htr10();
        let get = |s: &str| *t.iter().find(|e| e.element == s).unwrap();
        for (s, plate, eff) in [
            ("Rb", Some(0.30), Some(0.90)),
            ("Sr", Some(0.30), Some(0.90)),
            ("Ag", Some(0.50), Some(0.90)),
            ("Cs", Some(0.50), Some(0.90)),
            ("I", Some(0.20), Some(0.99)),
            ("Kr", None, Some(0.99)),
            ("Xe", None, Some(0.99)),
            ("C", None, Some(0.99)),
            ("H", None, Some(0.99)),
        ] {
            let e = get(s);
            assert_eq!((e.plate_out_per_cycle, e.purification_efficiency), (plate, eff), "{s}");
        }
        assert_eq!(t.len(), 9);
    }

    /// **From per-cycle data, with the fallbacks reported.** Methodology: Liu &
    /// Cao's table with an ARBITRARY test cycle time of 10 s and purified flow
    /// fraction of 0.01 (not HTR-10 values, which are unsourced), over the
    /// upstream fallback. Cs must get -ln(0.5)/10 plate-out and -ln(1 - 0.009)/10
    /// purification; Kr keeps the fallback's plate-out (0) and says so; Ba,
    /// absent from the table, keeps its group's rates. **Result (2026-10-05):**
    /// passes.
    #[test]
    fn per_cycle_rates_with_reported_fallbacks() {
        let fallback = RemovalRates::upstream(hz(7.5e-5), hz(8.77e-5));
        let (r, fell_back) = RemovalRates::from_per_cycle(
            &liu_cao_2002_htr10(),
            Time::new::<second>(10.0),
            Ratio::new::<ratio>(0.01),
            &fallback,
        );
        let cs = r.for_element(55);
        assert!((cs.k_plate.get::<hertz>() - std::f64::consts::LN_2 / 10.0).abs() < 1e-15);
        assert!((cs.k_clean.get::<hertz>() + (-0.009f64).ln_1p() / 10.0).abs() < 1e-18);
        assert_eq!(r.for_element(36).k_plate.get::<hertz>(), 0.0);
        assert!(fell_back.contains(&(36, "plate-out")));
        assert!(!fell_back.iter().any(|(z, _)| *z == 55));
        assert_eq!(r.for_element(56), fallback.for_element(56));
    }
}
