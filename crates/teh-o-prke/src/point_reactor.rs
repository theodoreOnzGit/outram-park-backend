//! # `PointReactor`: kinetics + decay heat (+ actinide slot) in one state (gh:#1082)
//!
//! RELAP-7 Theory Manual (INL/EXT-14-31366), §7.1 Eqs (414)-(416) and §7.4
//! Eq (429). One state holds
//!
//! - the N-group point kinetics ([`PointKinetics`], Eqs 412-413) in neutron
//!   density;
//! - the conversion density -> flux -> fission rate -> immediate power
//!   ([`FissionPowerConversion`]):
//!
//!   ```text
//!   phi(t) = n(t) v              (414)
//!   psi(t) = V Sigma_f phi(t)    (415)
//!   P_f(t) = Q_f psi(t)          (416)
//!   ```
//!
//! - fission-product decay heat ([`AnsDecayHeat`], Eqs 417-418) driven by
//!   `psi`, with the capture correction `G` (Eq 420) as a settable factor;
//! - an actinide (U-239/Np-239) slot, [`ActinideDecayModel`], whose only
//!   variant today is `None`: the actinide equations (§7.3, Eqs 421-423) are
//!   gh:#1081 and are deliberately not implemented here.
//!
//! and reports the total power, Eq (429):
//!
//! ```text
//! P_T = Q_f psi + G sum_a sum_j lambda_aj gamma_aj + P_actinide
//! ```
//!
//! `Q_f` is the **immediate** energy per fission (prompt and delayed-neutron
//! energy, *excluding* fission-product decay), so the decay term is not
//! counted twice.
//!
//! ## Coupling between kinetics and decay heat
//!
//! Each [`PointReactor::step`] advances the kinetics by backward Euler, then
//! the decay-heat groups analytically with the end-of-step fission rate held
//! over the step (consistent with backward Euler's end-of-step evaluation).
//! This is still an operator split, first order in `dt`; the decay groups do
//! not feed back on the kinetics, so the split only affects how `psi` is
//! sampled. RELAP-7's own transformation (§7.4, Eqs 430-442) integrates all
//! of these as one system; that is not reproduced.
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use uom::si::areal_number_rate::per_square_meter_second;
use uom::si::energy::joule;
use uom::si::f64::*;
use uom::si::frequency::hertz;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::reciprocal_length::reciprocal_meter;
use uom::si::velocity::meter_per_second;
use uom::si::volume::cubic_meter;
use uom::si::volumetric_number_density::per_cubic_meter;
use uom::ConstZero;

use crate::ans_decay_heat::AnsDecayHeat;
use crate::teh_o_prke_error::TehOPrkeError;
use crate::zero_power_prke::n_group_prke::PointKinetics;

/// The constants of RELAP-7 Eqs (414)-(416): one-speed neutron velocity
/// `v`, core volume `V`, macroscopic fission cross section `Sigma_f`, and
/// immediate energy per fission `Q_f`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FissionPowerConversion {
    /// `v`
    pub neutron_speed: Velocity,
    /// `V`
    pub core_volume: Volume,
    /// `Sigma_f`
    pub macroscopic_fission_cross_section: ReciprocalLength,
    /// `Q_f`, immediate (prompt + delayed-neutron) energy per fission,
    /// excluding fission-product decay energy.
    pub immediate_energy_per_fission: Energy,
}

impl FissionPowerConversion {
    /// Scalar flux `phi = n v` (Eq 414).
    pub fn flux(&self, neutron_density: VolumetricNumberDensity) -> ArealNumberRate {
        ArealNumberRate::new::<per_square_meter_second>(
            neutron_density.get::<per_cubic_meter>() * self.neutron_speed.get::<meter_per_second>(),
        )
    }

    /// Fission rate `psi = V Sigma_f phi` in fissions per second (Eq 415).
    pub fn fission_rate(&self, neutron_density: VolumetricNumberDensity) -> Frequency {
        Frequency::new::<hertz>(
            self.core_volume.get::<cubic_meter>()
                * self
                    .macroscopic_fission_cross_section
                    .get::<reciprocal_meter>()
                * self.flux(neutron_density).get::<per_square_meter_second>(),
        )
    }

    /// Immediate fission power `P_f = Q_f psi` (Eq 416).
    pub fn immediate_power(&self, neutron_density: VolumetricNumberDensity) -> Power {
        Power::new::<watt>(
            self.immediate_energy_per_fission.get::<joule>()
                * self.fission_rate(neutron_density).get::<hertz>(),
        )
    }

    /// The neutron density that gives immediate fission power `power`
    /// (inverse of [`Self::immediate_power`]).
    pub fn neutron_density_for_power(&self, power: Power) -> VolumetricNumberDensity {
        VolumetricNumberDensity::new::<per_cubic_meter>(
            power.get::<watt>()
                / (self.immediate_energy_per_fission.get::<joule>()
                    * self.core_volume.get::<cubic_meter>()
                    * self
                        .macroscopic_fission_cross_section
                        .get::<reciprocal_meter>()
                    * self.neutron_speed.get::<meter_per_second>()),
        )
    }
}

/// Actinide (U-239 / Np-239) decay-power model slot (RELAP-7 §7.3).
///
/// Only `None` exists. The actinide equations (Eqs 421-423) are gh:#1081;
/// this enum is the hook they will be added to, so `PointReactor` already
/// sums their power in [`PointReactor::total_power`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ActinideDecayModel {
    /// No actinide decay power (not modelled; gh:#1081).
    #[default]
    None,
}

impl ActinideDecayModel {
    /// Actinide decay power; zero for `None`.
    pub fn power(&self) -> Power {
        match self {
            Self::None => Power::ZERO,
        }
    }
}

/// Kinetics, decay heat and actinides in one state; total power by Eq 429.
#[derive(Clone, Debug, PartialEq)]
pub struct PointReactor {
    kinetics: PointKinetics,
    conversion: FissionPowerConversion,
    decay_heat: AnsDecayHeat,
    actinides: ActinideDecayModel,
    capture_correction: Ratio,
}

impl PointReactor {
    /// Assemble a reactor. Actinides `None`, capture correction `G = 1`.
    pub fn new(
        kinetics: PointKinetics,
        conversion: FissionPowerConversion,
        decay_heat: AnsDecayHeat,
    ) -> Self {
        Self {
            kinetics,
            conversion,
            decay_heat,
            actinides: ActinideDecayModel::None,
            capture_correction: Ratio::new::<ratio>(1.0),
        }
    }

    /// Set the capture correction `G` applied to the decay power (Eq 420),
    /// e.g. from [`crate::ans_decay_heat::neutron_capture_correction`]. It is
    /// a held value: the caller updates it as time since shutdown advances.
    pub fn set_capture_correction(&mut self, g: Ratio) {
        self.capture_correction = g;
    }

    /// Put the kinetics at critical equilibrium at immediate power `power`
    /// (decay heat untouched).
    pub fn set_critical_equilibrium(&mut self, power: Power, neutron_generation_time: Time) {
        let n = self.conversion.neutron_density_for_power(power);
        self.kinetics
            .set_critical_equilibrium(n, neutron_generation_time);
    }

    /// Saturate the decay heat to an infinitely long irradiation at the
    /// current fission rate.
    pub fn saturate_decay_heat(&mut self) -> Result<(), TehOPrkeError> {
        let psi = self.fission_rate();
        self.decay_heat.saturate(psi)
    }

    /// Advance one step: kinetics by backward Euler, then decay heat at the
    /// end-of-step fission rate.
    pub fn step(
        &mut self,
        timestep: Time,
        reactivity: Ratio,
        neutron_generation_time: Time,
        background_source_rate: VolumetricNumberRate,
    ) -> Result<(), TehOPrkeError> {
        self.kinetics.step_implicit(
            timestep,
            reactivity,
            neutron_generation_time,
            background_source_rate,
        )?;
        let psi = self.fission_rate();
        self.decay_heat.advance(psi, timestep)
    }

    /// The kinetics state.
    pub fn kinetics(&self) -> &PointKinetics {
        &self.kinetics
    }

    /// The decay-heat state.
    pub fn decay_heat(&self) -> &AnsDecayHeat {
        &self.decay_heat
    }

    /// The conversion constants.
    pub fn conversion(&self) -> &FissionPowerConversion {
        &self.conversion
    }

    /// The actinide slot.
    pub fn actinides(&self) -> ActinideDecayModel {
        self.actinides
    }

    /// Fission rate `psi` (Eq 415).
    pub fn fission_rate(&self) -> Frequency {
        self.conversion
            .fission_rate(self.kinetics.neutron_density())
    }

    /// Immediate fission power `Q_f psi` (Eq 416).
    pub fn immediate_fission_power(&self) -> Power {
        self.conversion
            .immediate_power(self.kinetics.neutron_density())
    }

    /// Corrected fission-product decay power `G P'_gamma` (Eq 420).
    pub fn decay_power(&self) -> Power {
        self.decay_heat.total_decay_power() * self.capture_correction.get::<ratio>()
    }

    /// Total power `P_T` (Eq 429): immediate + corrected decay + actinide.
    pub fn total_power(&self) -> Power {
        self.immediate_fission_power() + self.decay_power() + self.actinides.power()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ans_decay_heat::{AnsStandard, DecayHeatIsotope};
    use crate::zero_power_prke::six_group_precursor_prke::six_group_constants::FissioningNuclideType;
    use uom::si::energy::megaelectronvolt;
    use uom::si::time::second;

    fn conv() -> FissionPowerConversion {
        FissionPowerConversion {
            neutron_speed: Velocity::new::<meter_per_second>(2200.0),
            core_volume: Volume::new::<cubic_meter>(2.0),
            macroscopic_fission_cross_section: ReciprocalLength::new::<reciprocal_meter>(5.0),
            immediate_energy_per_fission: Energy::new::<megaelectronvolt>(200.0),
        }
    }

    /// **V&V: Eqs 414-416 by hand.** n = 1e12 /m³, v = 2200 m/s, V = 2 m³,
    /// Sigma_f = 5 /m, Q_f = 200 MeV: phi = 2.2e15 /m²/s, psi = 2.2e16 /s,
    /// P_f = 2.2e16 x 3.204353268e-11 J = 7.04958e5 W; the inverse returns
    /// n. *Result (2026-11-01):* all to 1e-14 relative.
    #[test]
    fn density_to_fission_rate_to_power_by_hand() {
        let c = conv();
        let n = VolumetricNumberDensity::new::<per_cubic_meter>(1.0e12);
        assert!((c.flux(n).get::<per_square_meter_second>() / 2.2e15 - 1.0).abs() < 1e-14);
        assert!((c.fission_rate(n).get::<hertz>() / 2.2e16 - 1.0).abs() < 1e-14);
        let p = c.immediate_power(n).get::<watt>();
        assert!(
            (p / (2.2e16 * 200.0 * 1.602_176_634e-13) - 1.0).abs() < 1e-14,
            "{p}"
        );
        let back = c.neutron_density_for_power(Power::new::<watt>(p));
        assert!(((back / n).get::<ratio>() - 1.0).abs() < 1e-14);
    }

    /// **V&V: Eq 429 at equilibrium, by hand.** Critical at 10 MW immediate,
    /// U-235 Draft1978 decay heat saturated: P'_gamma = psi x 13.183 MeV
    /// with psi = 10 MW / 200 MeV = 3.1211e17 /s, i.e. 0.65915 MW
    /// (13.183/200 of 10 MW); with G = 1.01 the decay term is 0.66574 MW and
    /// P_T = 10.66574 MW. A 100 s critical transient keeps it there.
    /// *Result (2026-11-01):* decay/immediate = 0.065915 to 1e-5; holds over
    /// 100 s to 1e-9 relative.
    #[test]
    fn total_power_at_equilibrium_by_hand() {
        let lam = Time::new::<second>(1.0e-4);
        let mut pr = PointReactor::new(
            PointKinetics::from_nuclide(FissioningNuclideType::U235),
            conv(),
            AnsDecayHeat::default(),
        );
        pr.set_critical_equilibrium(Power::new::<watt>(1.0e7), lam);
        pr.saturate_decay_heat().unwrap();
        let ratio_decay = (pr.decay_power() / pr.immediate_fission_power()).get::<ratio>();
        assert!((ratio_decay - 13.183 / 200.0).abs() < 1e-5, "{ratio_decay}");
        pr.set_capture_correction(Ratio::new::<ratio>(1.01));
        let p_t0 = pr.total_power().get::<watt>();
        assert!((p_t0 - (1.0e7 + 1.01 * ratio_decay * 1.0e7)).abs() < 1e-6 * p_t0);
        for _ in 0..1000 {
            pr.step(
                Time::new::<second>(0.1),
                Ratio::ZERO,
                lam,
                VolumetricNumberRate::ZERO,
            )
            .unwrap();
        }
        assert!((pr.total_power().get::<watt>() / p_t0 - 1.0).abs() < 1e-9);
        assert_eq!(pr.actinides(), ActinideDecayModel::None);
        assert_eq!(pr.actinides().power(), Power::ZERO);
        assert_eq!(pr.kinetics().group_count(), 6);
        assert_eq!(pr.decay_heat().standard(), AnsStandard::Draft1978);
        assert_eq!(pr.conversion().core_volume, conv().core_volume);
    }

    /// Prompt-only Nordheim-Fuchs run through `PointReactor`. Returns (peak
    /// power W, energy released J, time of peak s).
    fn nordheim_fuchs_run(
        kinetics: PointKinetics,
        rho_ext: f64,
        dt: f64,
        t_end: f64,
    ) -> (f64, f64, f64) {
        let lam = Time::new::<second>(1.0e-4);
        let (alpha, heat_cap) = (-1.0e-5, 1.0e5); // 1/K, J/K
        let mut pr = PointReactor::new(
            kinetics,
            conv(),
            AnsDecayHeat::new(AnsStandard::Draft1978)
                .with_built_in_isotope(DecayHeatIsotope::U235, Ratio::new::<ratio>(1.0))
                .unwrap(),
        );
        pr.set_critical_equilibrium(Power::new::<watt>(1.0), lam);
        let mut temp_rise = 0.0;
        let (mut peak, mut t_peak) = (0.0, 0.0);
        let steps = (t_end / dt).round() as usize;
        for k in 1..=steps {
            let rho = Ratio::new::<ratio>(rho_ext + alpha * temp_rise);
            pr.step(
                Time::new::<second>(dt),
                rho,
                lam,
                VolumetricNumberRate::ZERO,
            )
            .unwrap();
            let p = pr.immediate_fission_power().get::<watt>();
            temp_rise += p * dt / heat_cap;
            if p > peak {
                peak = p;
                t_peak = k as f64 * dt;
            }
        }
        (peak, temp_rise * heat_cap, t_peak)
    }

    /// **V&V: Nordheim-Fuchs prompt excursion with adiabatic feedback.**
    ///
    /// *Methodology.* Prompt-only kinetics (`PointKinetics` with zero delayed
    /// groups, the Nordheim-Fuchs assumption), `Lambda = 1e-4 s`, step
    /// `rho_0 = 0.005` at `P_0 = 1 W`, adiabatic fuel `C = 1e5 J/K`,
    /// feedback `rho = rho_0 + alpha dT`, `alpha = -1e-5 /K`, so
    /// `b = |alpha|/C = 1e-10 /J`. Power from `n` through Eqs 414-416. The
    /// closed forms (from `dP/dE = (rho_0 - b E)/Lambda`):
    ///
    /// ```text
    /// P_max = P_0 + rho_0^2 / (2 b Lambda)              = 1.250000001e9 W
    /// E_tot = (rho_0 + sqrt(rho_0^2 + 2 b Lambda P_0)) / b = 1.00000000e8 J
    /// ```
    ///
    /// Integrated by backward Euler at `dt = 1e-5 s` to 1.5 s, with the
    /// feedback lagged one step (temperature from the previous step).
    ///
    /// *Prediction (before running).* Errors of order `dt rho_0 / Lambda =
    /// 5e-4`: peak and energy within 1e-2 relative (the gate), likely ~1e-3.
    ///
    /// *Results (2026-11-01):* peak 1.250208e9 W (rel 1.7e-4) at 0.4466 s;
    /// energy 1.000083e8 J (rel 8.3e-5); better than the ~1e-3 predicted.
    /// A six-group run at `rho_ext = beta + rho_0`, printed for comparison
    /// and not gated (delayed neutrons are outside the closed form), peaks
    /// 2.8 % higher and has released 31 % more energy by 1.5 s.
    #[test]
    fn nordheim_fuchs_prompt_excursion() {
        let (rho0, b, big_l, p0): (f64, f64, f64, f64) = (0.005, 1.0e-10, 1.0e-4, 1.0);
        let p_max = p0 + rho0 * rho0 / (2.0 * b * big_l);
        let e_tot = (rho0 + (rho0 * rho0 + 2.0 * b * big_l * p0).sqrt()) / b;
        let prompt = PointKinetics::new(vec![], vec![]).unwrap();
        let (peak, energy, t_peak) = nordheim_fuchs_run(prompt, rho0, 1.0e-5, 1.5);
        let (rp, re) = (peak / p_max - 1.0, energy / e_tot - 1.0);
        println!(
            "NF prompt-only: P_max ref {p_max:.6e} W, PRKE {peak:.6e} W (rel {rp:.3e}) at {t_peak:.4} s; \
             E ref {e_tot:.6e} J, PRKE {energy:.6e} J (rel {re:.3e})"
        );
        assert!(rp.abs() < 1e-2 && re.abs() < 1e-2);

        let six = PointKinetics::from_nuclide(FissioningNuclideType::U235);
        let beta = six.total_delayed_fraction().get::<ratio>();
        let (peak6, energy6, t6) = nordheim_fuchs_run(six, beta + rho0, 1.0e-5, 1.5);
        println!(
            "NF six-group (rho_ext = beta + rho_0, not gated): P_max {peak6:.6e} W ({:.3e} vs prompt closed form), \
             E(1.5 s) {energy6:.6e} J ({:.3e}), peak at {t6:.4} s",
            peak6 / p_max - 1.0,
            energy6 / e_tot - 1.0
        );
    }
}
