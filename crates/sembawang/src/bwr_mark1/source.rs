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
//! ~~Not done here: the dispersion run itself, which needs the Fukushima
//! meteorology (not held).~~ **CORRECTED 2026-10-29:** [`near_field`] runs
//! it, under a stated, assumed [`Weather`] (the meteorology is still not
//! held): the source term padded by [`crate::chain::pad_for_dispersion`],
//! `changi`'s puff train for the dilution factors, and `changi`'s survey for
//! the time-integrated air concentration and dry deposition on the plume's
//! centreline. An illustrative chain demonstration within the puff model's
//! few-km reach, not a reconstruction of the 2011 plume, and no dose.

use super::compare::Histories;
use super::fp_decay::Summation;
use super::params::Params;
use boon_lay::triso_atops_fork::nuclide_model::nuclide_database::find_nuclide;
use changi::activity::deposition::DepositionGroup;
use changi::activity::source::{NuclideRelease, ReleaseWindow, SourceTerm};
use changi::activity::chi_over_q::{dilution_factors, StabilitySource};
use changi::activity::survey::{survey, DepositionVelocities, SiteSurvey};
use changi::puff::simulate::{constant_wind, AdvectionPolicy, EmissionPolicy, Receptor, RunConfig, Source};
use changi::puff::stability::StabilityClass;
use petir::mathf::RealMath;
use uom::si::f64::{Length, Radioactivity, Time, Velocity};
use uom::si::length::meter;
use uom::si::velocity::meter_per_second;
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

/// The weather a [`near_field`] run assumes: one wind speed, held toward
/// the receptors for the whole run, one Pasquill class, and the height the
/// release leaves at.
#[derive(Debug, Clone, Copy)]
pub struct Weather {
    pub wind: Velocity,
    pub class: StabilityClass,
    pub release_height: Length,
}

impl Weather {
    /// The assumed default, every value \[A\]: 3 m/s, Pasquill D (neutral,
    /// the common overcast or windy case), released at 120 m, the top of the
    /// stack Units 1 and 2 share \[U\] (the vent's route; the building's
    /// leak, which left lower, is released there too, stated). Held toward the
    /// receptors for the whole release, which the real wind was not: an upper
    /// bound along one direction.
    pub fn assumed() -> Self {
        Self {
            wind: Velocity::new::<meter_per_second>(3.0),
            class: StabilityClass::D,
            release_height: Length::new::<meter>(120.0),
        }
    }
}

/// Puffs are emitted and moved every `NEAR_FIELD_STEP_S`, and dropped at
/// `NEAR_FIELD_PUFF_S` (the puff model's reach is this times the wind: 6 km
/// at 3 m/s).
pub const NEAR_FIELD_STEP_S: f64 = 10.0;
pub const NEAR_FIELD_PUFF_S: f64 = 2000.0;

/// The Unit 1 release carried downwind under `weather` (gh:#1040, F9): the
/// time-integrated air concentration at 1.5 m and the dry deposition at
/// each of `distances` on the plume's centreline. `changi`'s Gaussian puff
/// (a port of `puff` 0.1.1, Pasquill-Gifford sigmas over about 0.1-10 km)
/// with its uncited order-of-magnitude deposition velocities. Research,
/// education and V&V only: no dose, and the weather is assumed, not 2011's.
///
/// # Panics
/// If every distance is beyond the puff model's reach (`changi` refuses an
/// all-zero result).
pub fn near_field(term: &SourceTerm, weather: &Weather, distances: &[Length]) -> SiteSurvey {
    let step = Time::new::<second>(NEAR_FIELD_STEP_S);
    let term = crate::chain::pad_for_dispersion(term, Time::new::<second>(NEAR_FIELD_PUFF_S));
    let run = term.end();
    let n_steps = (run.get::<second>() / NEAR_FIELD_STEP_S) as usize + 1;
    let config = RunConfig {
        sim_dt: step,
        puff_dt: step,
        output_dt: step,
        duration: run,
        puff_duration: Time::new::<second>(NEAR_FIELD_PUFF_S),
        start_hour: 12,
        emission_policy: EmissionPolicy::OnePuffPerEmission,
        advection: AdvectionPolicy::default(),
    };
    let source = Source { x: Length::new::<meter>(0.0), y: Length::new::<meter>(0.0), height: weather.release_height };
    let wind = constant_wind(weather.wind, Velocity::new::<meter_per_second>(0.0), n_steps);
    let bounds = term.segment_boundaries();
    let factors = |z: f64| {
        let receptors: Vec<Receptor> =
            distances.iter().map(|d| Receptor { x: *d, y: Length::new::<meter>(0.0), z: Length::new::<meter>(z) }).collect();
        dilution_factors(&[source], &bounds, &wind, &receptors, &config, StabilitySource::Fixed(weather.class))
    };
    survey(&term, &factors(1.5), &factors(0.0), &DepositionVelocities::order_of_magnitude_placeholder())
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
    /// same day; by MCCI then to Cs-137 6.43e13 and I-131 6.08e15 Bq; by the
    /// drywell's outer heat path to 5.90e13 and 5.67e15 Bq; by the core
    /// plate to 6.49e13 and 9.06e15 Bq; by the bottom of the core's
    /// radiation to 7.77e13 and 2.57e16 Bq, the I-131 nearly tripled by the
    /// later, dry head failure, 2026-10-29; by a plate per ring to 5.99e13
    /// and 9.07e15 Bq, the early branch again; by the pool in two layers to
    /// 1.54e14 and 1.07e16 Bq, the flange leak open from about 17.5 h; by
    /// the plate's debris oxidising to 1.89e14 and 1.12e16 Bq; by the
    /// plenum's oxidation steam consumed to 1.85e14 and 1.05e16 Bq; by the
    /// control blades to 7.40e13 and 4.99e16 Bq; by their heat of fusion to
    /// 8.00e13 and 5.99e16 Bq; by the cavity debris's phases to 1.07e14 and
    /// 8.08e16 Bq; by the vent and flange choked to 9.76e13 and 7.51e16 Bq;
    /// by the blades' diffusion limit to 9.70e13 and 7.48e16 Bq; by the
    /// two-sided equalising to 7.55e13 and 8.28e16 Bq.)
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
        assert!((total(&st, 0) / 7.4755e13 - 1.0).abs() < 1e-3 && (total(&st, 1) / 9.7700e16 - 1.0).abs() < 1e-3, "{:e} {:e}", total(&st, 0), total(&st, 1));
        // Xe-133 2.533e18 -> 2.521e18 Bq with the bottom of the core's
        // radiation, -> 2.530e18 with a plate per ring (2026-10-29), ->
        // 2.538e18 with the control blades, -> 2.541e18 with the cavity
        // debris's phases, -> 2.539e18 with the vent and flange choked
        // (2026-10-30).
        assert!((total(&st, 2) / 2.5386e18 - 1.0).abs() < 1e-3, "{:e}", total(&st, 2));
    }

    /// The near-field dispersion run (`near_field`) under the assumed
    /// weather (3 m/s, Pasquill D, 120 m release, held toward the receptors
    /// for 30 h). Methodology: the model's 0-30 h release (the pins above),
    /// on the centreline at 1-5 km. Predicted before the run by hand
    /// Gaussian (PG class D, ground reflection): chi/Q about 4e-8 s/m^3 at
    /// 1 km and 2e-6 at 3-5 km, so time-integrated air at 3-5 km of about
    /// 5e12 (Xe-133), 5e10 (I-131) and 1.5e8 (Cs-137) Bq s/m^3, a factor 3
    /// either way, and about 50x lower at 1 km.
    ///
    /// Results (2026-10-29), pinned below. Time-integrated air, Bq s/m^3,
    /// Cs-137 / I-131 / Xe-133: 1 km 3.97e6 / 1.31e9 / 1.29e11; 3 km
    /// 1.26e8 / 4.15e10 / 4.07e12; 4 km (the peak) 1.34e8 / 4.43e10 /
    /// 4.34e12, chi/Q 1.72e-6 s/m^3; 5 km 1.27e8 / 4.19e10 / 4.11e12.
    /// Deposited at 4 km: Cs-137 1.34e5, I-131 4.43e8 Bq/m^2 (the placeholder
    /// velocities, 1e-3 and 1e-2 m/s). Every predicted number held within
    /// its factor of 3 (at 1 km 32x below 3 km, against the 50x predicted).
    /// No
    /// comparison with measurement is made: the weather is assumed and the
    /// puff model reaches only about 6 km. Also checked: the noble gas
    /// deposits nothing, and every nuclide's air integral is its release
    /// times one shared chi/Q but for decay in transit (Xe-133's 5.2 d and
    /// I-131's 8 d against at most 2000 s of travel: within 0.5 %).
    #[test]
    fn the_release_carried_downwind() {
        let prm = Params::default();
        let sum = compute(&prm);
        let h = histories(Mechanisms::default(), prm, 60.0, 30.0 * 3600.0);
        let st = source_term(&h, &prm, &sum, 3600.0);
        let d: Vec<Length> = [1000.0, 2000.0, 3000.0, 4000.0, 5000.0].iter().map(|m| Length::new::<meter>(*m)).collect();
        let r = near_field(&st, &Weather::assumed(), &d);
        let air = |k: usize, n: usize| r.at(k)[n].air.becquerel_seconds_per_cubic_meter();
        let ground = |k: usize, n: usize| r.at(k)[n].ground.becquerel_per_square_meter();
        for k in 0..5 {
            println!(
                "{} m: Cs-137 {:.3e}, I-131 {:.3e}, Xe-133 {:.3e} Bq s/m3; deposited Cs-137 {:.3e}, I-131 {:.3e} Bq/m2",
                d[k].get::<meter>(), air(k, 0), air(k, 1), air(k, 2), ground(k, 0), ground(k, 1)
            );
            assert_eq!(ground(k, 2), 0.0);
            let q = |n: usize| st.nuclides[n].total_released().get::<becquerel>();
            assert!((air(k, 0) / q(0) / (air(k, 2) / q(2)) - 1.0).abs() < 5e-3);
            assert!((air(k, 1) / q(1) / (air(k, 2) / q(2)) - 1.0).abs() < 5e-3);
        }
        // Moved by a plate per ring (2026-10-29, the release's own change):
        // Xe-133 1.286e11 -> 1.291e11 at 1 km, 4.338e12 -> 4.353e12 at
        // 4 km; Cs-137 1.340e8 -> 1.032e8; deposited I-131 4.428e8 ->
        // 1.562e8 Bq/m^2. By the pool in two layers: Cs-137 at 4 km 1.032e8
        // -> 2.658e8, deposited I-131 1.562e8 -> 1.848e8 Bq/m^2; by the
        // plate's debris oxidising, 3.262e8 and 1.929e8; by the plenum's
        // oxidation steam consumed, 3.183e8 and 1.802e8; by the control
        // blades, Xe-133 1.295e11 at 1 km and 4.368e12 at 4 km, Cs-137
        // 1.275e8, deposited I-131 8.595e8; by their heat of fusion,
        // Cs-137 1.379e8, deposited I-131 1.032e9; by the cavity debris's
        // phases, Xe-133 1.297e11 and 4.373e12, Cs-137 1.846e8, I-131
        // 1.390e9; by the vent and flange choked, Xe-133 1.295e11 and
        // 4.368e12, Cs-137 1.683e8, I-131 1.293e9; by the blades' diffusion
        // limit, Cs-137 1.672e8, I-131 1.287e9; by the two-sided
        // equalising, 1.301e8 and 1.425e9; by the blades' B4C, 1.648e8 and
        // 1.487e9; by the debris falling through the pedestal's water,
        // 1.646e8 and 1.486e9; with UO2 carried by candling Zr, 1.289e8 and 1.682e9.
        assert!((air(0, 2) / 1.2954e11 - 1.0).abs() < 1e-3 && (air(3, 2) / 4.3676e12 - 1.0).abs() < 1e-3, "{:e} {:e}", air(0, 2), air(3, 2));
        assert!((air(3, 0) / 1.2888e8 - 1.0).abs() < 1e-3 && (ground(3, 1) / 1.6817e9 - 1.0).abs() < 1e-3, "{:e} {:e}", air(3, 0), ground(3, 1));
    }
}
