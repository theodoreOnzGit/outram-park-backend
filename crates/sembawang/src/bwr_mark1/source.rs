//! The Unit 1 release as a `changi` source term (gh:#1040, F9's "wire the
//! release to changi"): the hand-off the offsite chain consumes, as
//! [`crate::chain::pad_for_dispersion`] expects it.
//!
//! - **What leaves**: the model's cumulative releases to the environment
//!   (vented to the stack plus leaked to the reactor building, whose own
//!   retention is not modelled), as fractions of each class's inventory
//!   ([`super::compare::Histories`]).
//! - **Inventories**: each nuclide's activity at the scram, `lambda N`, from
//!   the model's ENDF/B-VIII summation ([`super::fp_decay`]). Cs-137, which
//!   builds up through the irradiation, is scaled to the BSAF
//!   specification's cesium mass (the summation's 95.0 kg against the
//!   specified 154.0 kg: its 584-day irradiation is MELCOR's default, not
//!   Unit 1's history). I-131 and Xe-133 are not scaled: at 8 and 5 days
//!   they stand at equilibrium with the power whatever the history, and an
//!   element mass (stable I-127 and long-lived I-129 for iodine) says
//!   nothing about them. The summation has no neutron capture (stated
//!   there), which biases the Cs-137 share of cesium low.
//! - **Nuclides**: Cs-137 (aerosol), I-131 (halogen) and Xe-133 (noble gas),
//!   their classes' isotopes taken to move together. Decay constants from
//!   `boon-lay` (`changi`'s rule), deposition groups by atomic number
//!   ([`DepositionGroup::from_atomic_number`]).
//! - **Windows**: the histories' samples grouped into windows of
//!   `window_s`; each window's activity is its share of the inventory
//!   decayed from the scram to the window's middle (the release is taken as
//!   uniform within a window, as `changi` treats it).
//!
//! Not done here: the dispersion run itself, which needs the Fukushima
//! meteorology (not held).

use super::compare::Histories;
use super::fp_decay::Summation;
use super::params::Params;
use boon_lay::triso_atops_fork::nuclide_model::nuclide_database::find_nuclide;
use changi::activity::deposition::DepositionGroup;
use changi::activity::source::{NuclideRelease, ReleaseWindow, SourceTerm};
use petir::mathf::RealMath;
use uom::si::f64::{Radioactivity, Time};
use uom::si::frequency::hertz;
use uom::si::radioactivity::becquerel;
use uom::si::time::second;

/// Atomic mass unit, kg.
const U_KG: f64 = 1.660_539_07e-27;

/// One nuclide's activity at the scram, Bq, from the summation: `lambda N`
/// with `N` the nuclide's mass over `A u`, scaled by `scale`.
pub fn inventory_bq(sum: &Summation, endf_name: &str, boon_lay_name: &str, scale: f64) -> f64 {
    let n = find_nuclide(boon_lay_name).unwrap_or_else(|| panic!("{boon_lay_name} not in boon-lay"));
    let atoms = sum.nuclide_mass_kg(endf_name) / (n.a as f64 * U_KG);
    n.decay_constant().get::<hertz>() * atoms * scale
}

/// The release in `h` as a source term of windows `window_s` long (the
/// first starting at the scram), with Cs-137, I-131 and Xe-133.
///
/// # Panics
/// If the histories are empty, or a nuclide is missing from `boon-lay`.
#[must_use]
pub fn source_term(h: &Histories, prm: &Params, sum: &Summation, window_s: f64) -> SourceTerm {
    assert!(!h.t_h.is_empty(), "no histories");
    let cs_scale = prm.cs_inventory_kg / sum.element_mass_kg("Cs");
    let nuclides = [
        ("Cs137", "Cs-137", &h.cs_released, cs_scale),
        ("I131", "I-131", &h.i_released, 1.0),
        ("Xe133", "Xe-133", &h.xe_released, 1.0),
    ];
    let end_s = h.t_h[h.t_h.len() - 1] * 3600.0;
    let n_win = (end_s / window_s).ceil().max(1.0) as usize;
    let edge = |k: usize| (k as f64 * window_s).min(end_s);
    // The cumulative fraction at `t_s`: the last sample at or before it (0
    // before the first).
    let at = |series: &[f64], t_s: f64| {
        let i = h.t_h.partition_point(|&x| x * 3600.0 <= t_s + 1e-9);
        if i == 0 { 0.0 } else { series[i - 1] }
    };
    let windows: Vec<ReleaseWindow> = (0..n_win).map(|k| ReleaseWindow::new(Time::new::<second>(edge(k)), Time::new::<second>(edge(k + 1)))).collect();
    let releases = nuclides
        .iter()
        .map(|&(endf, name, series, scale)| {
            let nuc = find_nuclide(name).unwrap_or_else(|| panic!("{name} not in boon-lay"));
            let lambda = nuc.decay_constant().get::<hertz>();
            let a0 = inventory_bq(sum, endf, name, scale);
            let released = (0..n_win)
                .map(|k| {
                    let (t0, t1) = (edge(k), edge(k + 1));
                    let df = (at(series, t1) - at(series, t0)).max(0.0);
                    Radioactivity::new::<becquerel>(a0 * df * (-lambda * 0.5 * (t0 + t1)).r_exp())
                })
                .collect();
            NuclideRelease { label: name.to_string(), decay_constant: nuc.decay_constant(), deposition_group: DepositionGroup::from_atomic_number(nuc.z), released }
        })
        .collect();
    SourceTerm::new(windows, releases)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bwr_mark1::compare::histories;
    use crate::bwr_mark1::fp_decay::compute;
    use crate::bwr_mark1::Mechanisms;

    /// A hand-made history: half the cesium leaves at 1.5 h, the rest at
    /// 3.5 h; hourly windows put each half in its window, decayed from the
    /// scram to the window's middle, and nothing elsewhere. The groups are
    /// changi's by atomic number.
    #[test]
    fn windows_carry_each_hours_release() {
        let prm = Params::default();
        let sum = compute(&prm);
        let t_h = vec![0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0];
        let cs = vec![0.0, 0.0, 0.5, 0.5, 0.5, 0.5, 1.0, 1.0];
        let h = Histories { t_h: t_h.clone(), cs_released: cs, i_released: vec![0.0; 8], xe_released: vec![0.0; 8], ..Default::default() };
        let st = source_term(&h, &prm, &sum, 3600.0);
        assert_eq!(st.windows.len(), 4);
        let cs137 = &st.nuclides[0];
        let a0 = inventory_bq(&sum, "Cs137", "Cs-137", prm.cs_inventory_kg / sum.element_mass_kg("Cs"));
        let bq: Vec<f64> = cs137.released.iter().map(|a| a.get::<becquerel>()).collect();
        let lam = cs137.decay_constant.get::<hertz>();
        assert_eq!(bq[0], 0.0);
        assert!((bq[1] - 0.5 * a0 * (-lam * 5400.0).exp()).abs() < 1e-9 * a0, "{bq:?}");
        assert_eq!(bq[2], 0.0);
        assert!((bq[3] - 0.5 * a0 * (-lam * 12600.0).exp()).abs() < 1e-9 * a0, "{bq:?}");
        assert_eq!(cs137.deposition_group, DepositionGroup::Aerosol);
        assert_eq!(st.nuclides[1].deposition_group, DepositionGroup::Halogen);
        assert_eq!(st.nuclides[2].deposition_group, DepositionGroup::NobleGas);
    }

    /// The model's first 30 h as a source term, padded for dispersion
    /// (`chain::pad_for_dispersion`, which adds nothing): the Cs-137 total
    /// equals its inventory times the released fraction (Cs-137's decay over
    /// a day is 6e-5); the inventories at the scram, by hand from the
    /// summation's masses: Cs-137 30.09 kg x (154.0/95.01) = 48.77 kg, 1.566e17
    /// Bq; I-131 and Xe-133 from the summation unscaled (at equilibrium).
    ///
    /// Results (2026-10-29): inventories Cs-137 1.565e17 Bq, I-131 1.358e18
    /// Bq (0.98e15 Bq per MWt), Xe-133 2.895e18 Bq. Released by 30 h:
    /// Cs-137 ~~7.96e13~~ 7.84e13 Bq (the BSAF test's 0.0783 PBq at 26 h),
    /// I-131 ~~5.54e15~~ 5.98e15 Bq (0.4 %), Xe-133 2.53e18 Bq (the noble
    /// gas nearly all vented, less its decay). (Moved by `see_through` the
    /// same day; by MCCI then to Cs-137 6.43e13 and I-131 6.08e15 Bq.)
    #[test]
    fn the_model_release_hands_off_to_changi() {
        let prm = Params::default();
        let sum = compute(&prm);
        let cs = inventory_bq(&sum, "Cs137", "Cs-137", prm.cs_inventory_kg / sum.element_mass_kg("Cs"));
        let i = inventory_bq(&sum, "I131", "I-131", 1.0);
        let xe = inventory_bq(&sum, "Xe133", "Xe-133", 1.0);
        assert!((cs / 1.566e17 - 1.0).abs() < 2e-3, "{cs:e}");
        let h = histories(Mechanisms::default(), prm, 60.0, 30.0 * 3600.0);
        let st = source_term(&h, &prm, &sum, 3600.0);
        let padded = crate::chain::pad_for_dispersion(&st, Time::new::<second>(6.0 * 3600.0));
        let total = |t: &SourceTerm, k: usize| t.nuclides[k].total_released().get::<becquerel>();
        for k in 0..3 {
            assert!((total(&padded, k) - total(&st, k)).abs() <= 1e-12 * total(&st, k).max(1.0));
        }
        let f = h.cs_released[h.cs_released.len() - 1];
        assert!((total(&st, 0) / (f * cs) - 1.0).abs() < 1e-4, "{} vs {}", total(&st, 0), f * cs);
        assert!((i / 1.358e18 - 1.0).abs() < 1e-3 && (xe / 2.895e18 - 1.0).abs() < 1e-3, "{i:e} {xe:e}");
        assert!((total(&st, 0) / 6.434e13 - 1.0).abs() < 1e-3 && (total(&st, 1) / 6.077e15 - 1.0).abs() < 1e-3, "{:e} {:e}", total(&st, 0), total(&st, 1));
        assert!((total(&st, 2) / 2.533e18 - 1.0).abs() < 1e-3, "{:e}", total(&st, 2));
    }
}
