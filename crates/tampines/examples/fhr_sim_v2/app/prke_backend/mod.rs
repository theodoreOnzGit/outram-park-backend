use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

use nee_soon::{NeeSoon, NordheimFuchsExactTimestepper};
use teh_o_prke::decay_heat::DecayHeat;
use teh_o_prke::delayed_neutron_layer::DelayedNeutronLayer;
use teh_o_prke::feedback_mechanisms::fission_product_poisons::Xenon135Poisoning;
use teh_o_prke::feedback_mechanisms::SixFactorFormulaFeedback;
use teh_o_prke::zero_power_prke::six_group_precursor_prke::six_group_constants::FissioningNuclideType;
use uom::si::area::square_meter;
use uom::si::energy::{kilojoule, megaelectronvolt};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::linear_number_density::per_meter;
use uom::si::mass::kilogram;
use uom::si::power::{megawatt, watt};
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::{millisecond, second};
use uom::si::velocity::meter_per_second;
use uom::si::volume::cubic_meter;
use uom::si::{f64::*, ratio::ratio};
use uom::ConstZero;

use crate::{FHRSimulatorApp, FHRState};

/// Delayed neutron fraction beta -- U-235 thermal-spectrum illustrative
/// value, used for the reactivity-in-dollars display and as
/// [`NordheimFuchsExactTimestepper::delayed_neutron_fraction`]. Previously
/// read from `SixGroupPRKE::get_total_delayed_fraction()`; that struct is
/// gone now that kinetics runs through Nordheim-Fuchs (which neglects
/// delayed-precursor dynamics entirely, see its module doc), so this is a
/// fixed constant instead.
const DELAYED_NEUTRON_FRACTION_BETA: f64 = 0.0065;

/// Fuel-temperature feedback coefficient alpha_f \[K^-1\], reused directly
/// from the old `fuel_temp_resonance_esc_feedback_linear` heuristic's slope
/// (`-2.3077e-4` fractional six-factor change per degC) -- for a factor
/// near 1.0 a fractional change IS approximately the implied reactivity
/// change, so the old curve's slope doubles as a linear alpha_f without
/// inventing a new number.
const FUEL_FEEDBACK_COEFFICIENT_PER_KELVIN: f64 = -2.3077e-4;

/// Fuel reference temperature T_f,ref -- back-derived as the old linear
/// heuristic's zero-crossing (`factor == 1.0`, i.e. no feedback), so
/// `alpha_f*(T_f - T_f,ref)` reproduces the same equilibrium point the old
/// six-factor curve implied: `-2.3077e-4*T_degc + 1.2462 = 1.0` =>
/// `T_degc ~= 1067.13`.
const FUEL_REFERENCE_TEMPERATURE_DEGC: f64 = 1067.13;

impl FHRSimulatorApp {
    /// associated function for kinetics calculation
    /// for continuous loop
    pub fn calculate_prke_loop(fhr_state: Arc<Mutex<FHRState>>) {
        // Nordheim-Fuchs exact timestepper (see teh_o_prke::nordheim_fuchs)
        // replaces the former SixGroupPRKE numerical solver -- its
        // closed-form solution has no dt << Lambda stability restriction,
        // so this loop now runs at a 1 ms timestep instead of the old
        // 25 microsecond one (25x coarser), while still resolving the
        // same prompt-power + adiabatic-fuel-feedback excursion physics
        // exactly.
        let initial_fuel_temp = ThermodynamicTemperature::new::<degree_celsius>(500.0);
        let mut nordheim_fuchs: NordheimFuchsExactTimestepper = NeeSoon::default()
            .new_prompt_excursion_model(
                Time::new::<second>(2.31e-4), // Lambda, same value the old numerical PRKE used
                Ratio::new::<ratio>(DELAYED_NEUTRON_FRACTION_BETA),
                // C_f is kept consistent with the pebble-bed thermal model
                // (mass 8000 kg * c_p 340 J/(kg K), the same c_p
                // `PebbleBedThermalHydraulics` uses) so Nordheim-Fuchs's
                // within-step adiabatic self-heating and the pebble bed's
                // heat-removal-aware temperature integration share one heat
                // capacity -- part of reconciling the fuel-temperature
                // hand-off (see `calculate_prke_for_one_timestep`'s doc).
                HeatCapacity::new::<joule_per_kelvin>(8000.0 * 340.0),
                TemperatureCoefficient::new::<per_kelvin>(FUEL_FEEDBACK_COEFFICIENT_PER_KELVIN),
                ThermodynamicTemperature::new::<degree_celsius>(FUEL_REFERENCE_TEMPERATURE_DEGC),
                initial_fuel_temp,
                Power::new::<watt>(1.0), // small nonzero seed -- avoids the P->0 startup singularity
            )
            .expect("illustrative Nordheim-Fuchs parameters must satisfy NordheimFuchsExactTimestepper::new's preconditions");

        // Delayed-neutron layer -- the reduced five-group precursor bank
        // (teh_o_prke::delayed_neutron_layer) that restores the delayed-
        // neutron source Nordheim-Fuchs (a prompt-only model) omits. Built
        // with the same prompt generation time Lambda the NF timestepper uses
        // (2.31e-4 s) so its source gains beta_i/Lambda are consistent. Each
        // step it takes the freshly-advanced prompt power and returns the
        // delayed power increment S*dt; adding that to the prompt power turns
        // the prompt-only model into proper point kinetics, whose delayed
        // source is the inertia that damps the fuel-temperature feedback loop
        // (op-dt3.11).
        let mut delayed_neutron_layer =
            DelayedNeutronLayer::u235_five_group(Time::new::<second>(2.31e-4));

        let prke_timestep = Time::new::<millisecond>(1.0);
        let reactor_volume = Volume::new::<cubic_meter>(0.5);
        let macroscopic_fission_xs = LinearNumberDensity::new::<per_meter>(1.0);
        let mut pebble_bed_th_struct = PebbleBedThermalHydraulics::new();
        let fhr_state_clone = fhr_state.clone();

        // then decay heat struct
        let mut fhr_decay_heat_struct = DecayHeat::default();

        // then xenon poisoning struct
        let mut fhr_xe135_poisoning = Xenon135Poisoning::default();

        // now, time controls
        let loop_time = SystemTime::now();
        let mut current_simulation_time = Time::ZERO;

        // calculation loop (indefinite)
        //
        //// calculation time and time to sleep
        // to be done once every timestep
        loop {
            // so now, let's do the necessary things
            // first, timestep and loop time
            //
            // second, read and update the local_ciet_state

            let loop_time_start = loop_time.elapsed().unwrap();
            // now this is arbitrary, user can set
            let mut keff_six_factor = SixFactorFormulaFeedback::default();
            // start from keff = 1
            // all terms = 1
            // start with leakage
            keff_six_factor.p_tnl = Ratio::new::<ratio>(0.9);
            keff_six_factor.p_fnl = Ratio::new::<ratio>(0.7);
            // then fuel reproduction
            keff_six_factor.eta = Ratio::new::<ratio>(2.2);
            // resonance esc probability
            keff_six_factor.p = Ratio::new::<ratio>(0.8);
            // thermal utilisation
            keff_six_factor.f = Ratio::new::<ratio>(0.9);

            // fast fission
            keff_six_factor.epsilon = Ratio::new::<ratio>(1.03);
            // keff total is about 1.0278
            // excess reactivity is about 0.0278
            //
            // basically control rod should be about this much
            // and fuel temp feedback about this much also
            // some of these are arbitrary
            Self::calculate_prke_for_one_timestep(
                &mut fhr_state_clone.lock().unwrap(),
                &mut keff_six_factor,
                &mut nordheim_fuchs,
                prke_timestep,
                reactor_volume,
                macroscopic_fission_xs,
                &mut fhr_decay_heat_struct,
                &mut pebble_bed_th_struct,
                &mut fhr_xe135_poisoning,
                &mut delayed_neutron_layer,
            );

            current_simulation_time += prke_timestep;

            let prke_simulation_time_seconds = current_simulation_time.get::<second>();

            let prke_elapsed_time_seconds =
                (loop_time.elapsed().unwrap().as_secs_f64() * 100.0).round() / 100.0;

            let overall_simulation_in_realtime_or_faster: bool =
                prke_simulation_time_seconds >= prke_elapsed_time_seconds;

            // now update the fhr state
            let prke_timestep_microseconds = prke_timestep.get::<uom::si::time::microsecond>();

            fhr_state_clone.lock().unwrap().prke_timestep_microseconds = prke_timestep_microseconds;

            fhr_state_clone.lock().unwrap().prke_simulation_time_seconds =
                prke_simulation_time_seconds;

            fhr_state_clone.lock().unwrap().prke_elapsed_time_seconds = prke_elapsed_time_seconds;

            // calculation time and time to sleep
            let loop_time_end = loop_time.elapsed().unwrap();
            let time_taken_for_calculation_loop_microseconds: f64 =
                (loop_time_end - loop_time_start).as_micros() as f64;
            fhr_state_clone.lock().unwrap().prke_calc_time_microseconds =
                time_taken_for_calculation_loop_microseconds;

            let time_to_sleep_microseconds: u64 = (prke_timestep
                .get::<uom::si::time::microsecond>()
                - time_taken_for_calculation_loop_microseconds)
                .round()
                .abs() as u64;

            let time_to_sleep: Duration = Duration::from_micros(time_to_sleep_microseconds - 1);

            // last condition for sleeping
            let real_time_in_current_timestep: bool = time_to_sleep_microseconds > 1;

            //
            let fast_forward_botton_on = false;

            if overall_simulation_in_realtime_or_faster
                && real_time_in_current_timestep
                && !fast_forward_botton_on
            {
                thread::sleep(time_to_sleep);
            } else if overall_simulation_in_realtime_or_faster
                && real_time_in_current_timestep
                && fast_forward_botton_on
            {
                // sleep 5 microseconds if fast fwd
                let short_time_to_sleep: Duration = Duration::from_micros(5);
                thread::sleep(short_time_to_sleep);
            } else {
                // don't sleep
            }
            //let time_to_sleep = Duration::from_millis(40);

            //dbg!(&(
            //        time_taken_for_calculation_loop_microseconds,
            //        prke_timestep.get::<microsecond>(),
            //)
            //);
            //thread::sleep(time_to_sleep);
        }
    }
    /// associated function for kinetics calculation for a single timestep.
    /// Note the kinetics timestep will be different from the main thermal
    /// hydraulics timestep.
    ///
    /// Kinetics now run through [`NordheimFuchsExactTimestepper`] rather
    /// than the former `SixGroupPRKE`. Control-rod and xenon feedback still
    /// go through [`SixFactorFormulaFeedback`] exactly as before, but
    /// fuel-temperature feedback moved to Nordheim-Fuchs's own
    /// `alpha_f*(T_f - T_f,ref)` term (see the crate-level constants
    /// above) -- `keff_six_factor.fuel_temp_feedback(...)` is deliberately
    /// no longer called here, to avoid double-counting it. The pebble-bed
    /// thermal-hydraulics model (`pebble_bed_th_struct`) remains the
    /// authoritative fuel temperature (it accounts for heat removal to the
    /// coolant); its output from the previous step is written into
    /// `nordheim_fuchs.fuel_temperature` before each step so Nordheim-
    /// Fuchs's feedback term uses the real, non-adiabatic temperature --
    /// only the momentary within-step coupling is adiabatic, matching the
    /// model's documented scope.
    ///
    /// # Fuel-temperature hand-off (single authoritative owner)
    /// The pebble-bed thermal-hydraulics model is the *only* owner of the
    /// persisted fuel temperature. Each step: (1) the pebble-bed temperature
    /// from the previous step is copied into `nordheim_fuchs.fuel_temperature`
    /// so the prompt-excursion reactivity feedback `alpha_f*(T - T_ref)` uses
    /// the heat-removal-aware temperature; (2) Nordheim-Fuchs steps -- it
    /// advances an internal adiabatic temperature *only* to evaluate its
    /// closed-form prompt power over the step (its self-limiting mechanism),
    /// and that internal temperature is **discarded**; (3) the pebble bed
    /// integrates the fission power (net of heat removal) and produces the
    /// next authoritative temperature. Nordheim-Fuchs's `C_f` is set equal to
    /// the pebble bed's heat capacity (see `calculate_prke_loop`), so the two
    /// no longer disagree on how much a given energy input heats the fuel.
    ///
    /// # Delayed-neutron layer (op-dt3.11)
    /// Nordheim-Fuchs is prompt-only. This step is a Lie-split point-kinetics
    /// update: Nordheim-Fuchs advances the prompt part `(rho - beta)/Lambda*P`
    /// (with adiabatic feedback) to a prompt power `P_p`, then the
    /// `delayed_neutron_layer` ([`DelayedNeutronLayer`]) is stepped with `P_p`
    /// and returns the delayed power increment `S*dt` from the precursor
    /// source `S = sum_i lambda_i C_i`. The total fission power
    /// `P = P_p + S*dt` is written back into Nordheim-Fuchs (so the next step
    /// propagates the full power, not just the prompt lineage) and drives
    /// xenon, decay heat, and pebble-bed heating. The delayed source keeps the
    /// reactor critical through the prompt-subcritical operating regime and
    /// supplies the precursor inertia that damps the fuel-temperature feedback
    /// loop that otherwise rings.
    pub fn calculate_prke_for_one_timestep(
        fhr_state_ref: &mut FHRState,
        keff_six_factor: &mut SixFactorFormulaFeedback,
        nordheim_fuchs: &mut NordheimFuchsExactTimestepper,
        prke_timestep: Time,
        reactor_volume: Volume,
        macroscopic_fission_xs: LinearNumberDensity,
        fhr_decay_heat: &mut DecayHeat,
        pebble_bed_th_struct: &mut PebbleBedThermalHydraulics,
        fhr_xe135_poisoning: &mut Xenon135Poisoning,
        delayed_neutron_layer: &mut DelayedNeutronLayer,
    ) {
        // within each timestep, I need to obtain feedback
        // so basically for now, control rod insertion based feedback goes
        // through the six-factor formula; fuel-temperature feedback is
        // Nordheim-Fuchs's job now (see this fn's doc comment)

        let left_cr_insertion_frac = fhr_state_ref.left_cr_insertion_frac;
        let right_cr_insertion_frac = fhr_state_ref.right_cr_insertion_frac;

        // now based on this, calculate feedback
        //
        let left_cr_insertion_ratio = Ratio::new::<ratio>(left_cr_insertion_frac as f64);
        let right_cr_insertion_ratio = Ratio::new::<ratio>(right_cr_insertion_frac as f64);

        keff_six_factor.control_rod_feedback(
            left_cr_insertion_ratio,
            FHRSimulatorApp::fuel_utilisation_factor_chg_for_control_rod_polynomial,
        );
        keff_six_factor.control_rod_feedback(
            right_cr_insertion_ratio,
            FHRSimulatorApp::fuel_utilisation_factor_chg_for_control_rod_polynomial,
        );

        // next xenon poisoning feedback
        //
        //
        // adjust for xenon poisoning
        let xe135_mass_conc = fhr_xe135_poisoning.get_current_xe135_conc();
        let thermal_utilisation_feedback_fractional_chg_from_xenon: f64 =
            Xenon135Poisoning::simplified_poison_concentration_feedback(xe135_mass_conc)
                .get::<ratio>();

        keff_six_factor.f *= thermal_utilisation_feedback_fractional_chg_from_xenon;

        // reactivity from control rods + xenon (fuel-temp feedback is
        // added back in below via Nordheim-Fuchs's own term)
        let reactivity_ex_fuel_temp: Ratio = keff_six_factor.calc_rho();

        // the pebble-bed thermal-hydraulics model is the authoritative
        // fuel temperature (it accounts for heat removal); feed its most
        // recent value into Nordheim-Fuchs before stepping so this step's
        // fuel-temperature feedback uses the real temperature, not an
        // isolated adiabatic proxy
        let current_pebble_core_temp: ThermodynamicTemperature =
            ThermodynamicTemperature::new::<degree_celsius>(fhr_state_ref.pebble_core_temp_degc);
        nordheim_fuchs.fuel_temperature = current_pebble_core_temp;
        nordheim_fuchs.set_external_reactivity(reactivity_ex_fuel_temp);

        // total reactivity for display purposes (control rods + xenon +
        // fuel-temp feedback), read before step() advances the state --
        // reactivity_margin() = rho_ext - beta + alpha_f*(T-T_ref), so add
        // beta back to undo that internal offset and recover the
        // six-factor-equivalent total reactivity.
        let total_reactivity: Ratio =
            nordheim_fuchs.reactivity_margin() + nordheim_fuchs.delayed_neutron_fraction;

        nordheim_fuchs.step(prke_timestep);

        // Lie-split point kinetics: Nordheim-Fuchs has just advanced the PROMPT
        // part of the power balance to `P_p`; the delayed-neutron layer now
        // supplies the delayed source increment `S*dt`, and the total fission
        // power `P = P_p + S*dt` is written back into Nordheim-Fuchs so the
        // next step propagates the full power (not just the prompt lineage).
        // The delayed source is what keeps the reactor critical through the
        // prompt-subcritical operating regime and damps the fuel-temperature
        // feedback loop that otherwise rings (op-dt3.11). Everything downstream
        // (xenon, decay heat, pebble-bed heating, displayed power) uses this
        // delayed-inclusive total.
        let prompt_fission_power: Power = nordheim_fuchs.power;
        let delayed_power_increment: Power =
            delayed_neutron_layer.advance(prompt_fission_power, prke_timestep);
        let fission_power_instantaneous: Power = prompt_fission_power + delayed_power_increment;
        nordheim_fuchs.power = fission_power_instantaneous;

        // Xenon135Poisoning still wants a fission-rate density and neutron
        // population density; back-derive them from the Nordheim-Fuchs
        // power output by inverting the old flux chain (power_per_fission
        // * fission_rate = fission_power), rather than modelling neutron
        // population directly (which Nordheim-Fuchs, a power/temperature
        // model, does not track).
        let power_per_fission = Energy::new::<megaelectronvolt>(200.0);
        let fission_rate: Frequency = fission_power_instantaneous / power_per_fission;
        let fission_rate_density: VolumetricNumberRate = (fission_rate / reactor_volume).into();
        let neutron_speed: Velocity = Velocity::new::<meter_per_second>(2200.0);
        let current_neutron_flux: ArealNumberRate =
            (fission_rate_density / macroscopic_fission_xs).into();
        let current_neutron_pop_density: VolumetricNumberDensity =
            (current_neutron_flux / neutron_speed).into();

        // note: it's convenient here to calc xe135 feedback

        let fissioning_nuclide = FissioningNuclideType::U235;
        let _xe135_conc_next_timestep = fhr_xe135_poisoning.calc_xe_135_and_return_num_density(
            prke_timestep,
            fission_rate_density,
            fissioning_nuclide,
            current_neutron_pop_density,
        );

        // add to decay heat precursors
        fhr_decay_heat.add_decay_heat_precursor1(fission_power_instantaneous * 0.04, prke_timestep);
        fhr_decay_heat.add_decay_heat_precursor2(fission_power_instantaneous * 0.04, prke_timestep);
        fhr_decay_heat.add_decay_heat_precursor3(fission_power_instantaneous * 0.02, prke_timestep);

        // adjust fission power for decay heat
        // fission power less decay heat = 1.0 - 0.04 - 0.04 - 0.02 = 0.9
        let mut fission_power_corrected_for_decay_heat = fission_power_instantaneous * 0.9;
        let mut reactor_current_decay_heat: Power =
            fhr_decay_heat.calc_decay_heat_power_1(prke_timestep).abs();
        reactor_current_decay_heat += fhr_decay_heat.calc_decay_heat_power_2(prke_timestep).abs();
        reactor_current_decay_heat += fhr_decay_heat.calc_decay_heat_power_3(prke_timestep).abs();
        fission_power_corrected_for_decay_heat += reactor_current_decay_heat;

        // with the correct fission power now, we can
        // calc temperature
        //
        //
        // These are arbitrary values, will adjust later

        let pebble_bed_mass = Mass::new::<kilogram>(8000.0);
        let pebble_bed_heat_transfer_area = Area::new::<square_meter>(300.0);
        let pebble_bed_overall_htc = HeatTransfer::new::<watt_per_square_meter_kelvin>(400.0);
        let pebble_bed_coolant_temp = ThermodynamicTemperature::new::<degree_celsius>(
            fhr_state_ref.pebble_bed_coolant_temp_degc,
        );

        let heat_removal_from_pebble_bed = pebble_bed_th_struct
            .calc_th_and_return_heat_removal_from_pebble_bed(
                prke_timestep,
                fission_power_corrected_for_decay_heat,
                pebble_bed_mass,
                pebble_bed_heat_transfer_area,
                pebble_bed_overall_htc,
                pebble_bed_coolant_temp,
            );

        let pebble_bed_fuel_temp =
            pebble_bed_th_struct.get_temperature_from_enthalpy_uo2_heuristic();

        // update the fhr state
        fhr_state_ref.pebble_core_temp_degc = pebble_bed_fuel_temp.get::<degree_celsius>();

        // the heat removal from pebble bed should be stored in fhr
        // state, so as to sync correctly with the coolant
        // the prke timestep will also be added so as to obtain an
        // average heat removal rate to be transferred to the coolant

        fhr_state_ref.prke_loop_accumulated_heat_removal_kilojoules +=
            (heat_removal_from_pebble_bed * prke_timestep).get::<kilojoule>();
        fhr_state_ref.prke_loop_accumulated_timestep_seconds += prke_timestep.get::<second>();

        // reactor power
        //

        // keff display, back-converted from total reactivity
        // (rho = (k-1)/k  =>  k = 1/(1-rho))
        let keff: Ratio = Ratio::new::<ratio>(1.0) / (Ratio::new::<ratio>(1.0) - total_reactivity);
        fhr_state_ref.keff = keff.get::<ratio>();
        fhr_state_ref.reactor_power_megawatts =
            fission_power_corrected_for_decay_heat.get::<megawatt>();
        fhr_state_ref.reactor_decay_heat_megawatts = reactor_current_decay_heat.get::<megawatt>();

        // reactivity in dollars
        let beta_delayed_frac_total = Ratio::new::<ratio>(DELAYED_NEUTRON_FRACTION_BETA);
        let reactivity_dollars: f64 = (total_reactivity / beta_delayed_frac_total).get::<ratio>();

        fhr_state_ref.reactivity_dollars = reactivity_dollars;

        let xenon135_feedback_dollars_approx =
            (thermal_utilisation_feedback_fractional_chg_from_xenon - 1.0)
                / thermal_utilisation_feedback_fractional_chg_from_xenon
                / beta_delayed_frac_total;

        fhr_state_ref.xenon135_feedback_dollars = xenon135_feedback_dollars_approx.get::<ratio>();

        let debug_settings = false;
        if debug_settings {
            // that settles thermal hydraulics
            dbg!(&(
                pebble_bed_fuel_temp,
                keff,
                total_reactivity,
                fission_power_instantaneous,
                fission_power_corrected_for_decay_heat,
                heat_removal_from_pebble_bed
            ));
        }
    }

    /// fuel temperature feedback and how it deals with resonance escape
    /// probability
    ///
    /// basically, with increased fuel temp, lower resonance
    /// esc probability
    ///
    /// this is a heuristic, can be replaced by more complex
    /// functions later
    ///
    /// the ratio u return is factor of increase or decrease in resonance
    /// escape probability
    ///
    /// No longer called from [`Self::calculate_prke_for_one_timestep`] --
    /// fuel-temperature feedback moved to Nordheim-Fuchs's own linear
    /// `alpha_f*(T_f - T_f,ref)` term (see `FUEL_FEEDBACK_COEFFICIENT_PER_KELVIN`
    /// / `FUEL_REFERENCE_TEMPERATURE_DEGC` above, derived directly from
    /// this function's slope/zero-crossing). Left in place for reference
    /// and because other callers may still exist.
    pub fn fuel_temp_resonance_esc_feedback_linear(fuel_temp: ThermodynamicTemperature) -> Ratio {
        let fuel_temp_degc = fuel_temp.get::<degree_celsius>();
        // from an arbitrary map of:
        // fuel_temp_degc, resonance esc change factor
        // 200,1.2,
        // 300,1.1,
        // 400,1.05,
        // 500,1,
        // 600,0.99,
        // 700,0.98,
        // 800,0.97,
        // 900,0.96,
        // 1000,0.95,
        // 1100,0.94,
        // 1200,0.9,
        // 1300,0.85,
        // 1400,0.83,
        // 1500,0.8,
        //
        // I get a trendline in LibreOffice

        return Ratio::new::<ratio>(-2.3077e-4 * fuel_temp_degc + 1.2462e0);
    }

    pub fn fuel_utilisation_factor_chg_for_control_rod_polynomial(
        cr_insertion_factor: Ratio,
    ) -> Ratio {
        //this is an arbitrary map, but just useful for
        //simulation
        //control rod insertion frac,fuel utilisation factor change
        // 0,1.03
        // 0.1,1.0275
        // 0.2,1.02
        // 0.3,1.005
        // 0.4,0.975
        // 0.5,0.875
        // 0.6,0.82
        // 0.7,0.775
        // 0.8,0.77
        // 0.9,0.76
        // 1,0.75

        let cr_insertion_factor_f64: f64 = cr_insertion_factor.get::<ratio>();
        let term1 = -8.413e0 * cr_insertion_factor_f64.powi(5);
        let term2 = 2.064e1 * cr_insertion_factor_f64.powi(4);
        let term3 = -1.639e1 * cr_insertion_factor_f64.powi(3);
        let term4 = 4.238e0 * cr_insertion_factor_f64.powi(2);
        let term5 = -3.626e-1 * cr_insertion_factor_f64.powi(1);
        let term6 = 1.031e0 * cr_insertion_factor_f64.powi(0);

        return Ratio::new::<ratio>(term1 + term2 + term3 + term4 + term5 + term6);
    }

    // this is a dummy function so that no
    // matter what you input, no change is given
    pub fn constant_ratio_no_change_function(_: Ratio) -> Ratio {
        return Ratio::new::<ratio>(1.0);
    }
}

/// this contains code for TIGHTLY coupled thermal hydraulics,
/// that is between the PRKE
pub mod pebble_bed_thermal_hydraulics;
pub use pebble_bed_thermal_hydraulics::*;

/// Headless regression harness for op-dt3.11 -- proves the delayed-neutron
/// layer damps the reactor-power oscillation that the prompt-only
/// Nordheim-Fuchs kinetics produced when coupled to the pebble-bed
/// fuel-temperature feedback loop.
///
/// # Methodology
/// The GUI runs kinetics and thermal-hydraulics in separate threads; the
/// oscillation lives in the tight `power -> pebble-bed heating -> fuel
/// temperature -> reactivity -> power` loop. This harness isolates exactly
/// that loop with **no egui GUI**: it holds the coolant at a fixed sink
/// temperature and steps the same components the GUI uses (Nordheim-Fuchs
/// prompt-excursion timestepper, the six-factor reactivity feedback, the
/// pebble-bed thermal model, decay heat, xenon) over a startup transient
/// (fuel begins at 500 degC, well below the ~1067 degC feedback reference, so
/// excess reactivity drives power up and the fuel-temperature feedback then
/// pulls it back -- the exact excitation that rings). Two configurations are
/// run identically except for the delayed-neutron layer:
/// - **before**: prompt power fed straight to the thermal model
///   (reproduces the shipped-then-buggy prompt-only coupling), and
/// - **after**: prompt power routed through
///   [`DelayedNeutronLayer::u235_five_group`] first (the fix).
///
/// Oscillation is quantified as the peak-to-peak reactor thermal power in the
/// tail window (last 40 % of the trace, i.e. after the initial rise). The
/// pass criterion is that the delayed layer cuts the tail peak-to-peak
/// amplitude by at least 20x and leaves it small in absolute terms.
///
/// # Results (data 2026-07-15, dt = 5 ms, 80 s transient)
/// Recorded by [`oscillation_regression::delayed_neutron_layer_damps_power_oscillation`]
/// and printed with `--nocapture`; representative measured numbers are quoted
/// in that test's doc comment.
#[cfg(test)]
mod oscillation_regression {
    use super::*;
    use crate::{FHRSimulatorApp, FHRState};
    use nee_soon::{NeeSoon, NordheimFuchsExactTimestepper};
    use teh_o_prke::decay_heat::DecayHeat;
    use teh_o_prke::delayed_neutron_layer::DelayedNeutronLayer;
    use teh_o_prke::feedback_mechanisms::fission_product_poisons::Xenon135Poisoning;
    use teh_o_prke::feedback_mechanisms::SixFactorFormulaFeedback;
    use teh_o_prke::zero_power_prke::six_group_precursor_prke::six_group_constants::FissioningNuclideType;
    use uom::si::area::square_meter;
    use uom::si::energy::megaelectronvolt;
    use uom::si::heat_capacity::joule_per_kelvin;
    use uom::si::heat_transfer::watt_per_square_meter_kelvin;
    use uom::si::linear_number_density::per_meter;
    use uom::si::mass::kilogram;
    use uom::si::power::{megawatt, watt};
    use uom::si::temperature_coefficient::per_kelvin;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::{millisecond, second};
    use uom::si::velocity::meter_per_second;
    use uom::si::volume::cubic_meter;
    use uom::si::{f64::*, ratio::ratio};

    /// Builds a fresh Nordheim-Fuchs timestepper with the same illustrative
    /// parameters `calculate_prke_loop` uses (C_f consistent with the pebble
    /// bed, 8000 kg * 340 J/(kg K)).
    fn fresh_nordheim_fuchs() -> NordheimFuchsExactTimestepper {
        NeeSoon::default()
            .new_prompt_excursion_model(
                Time::new::<second>(2.31e-4),
                Ratio::new::<ratio>(DELAYED_NEUTRON_FRACTION_BETA),
                HeatCapacity::new::<joule_per_kelvin>(8000.0 * 340.0),
                TemperatureCoefficient::new::<per_kelvin>(FUEL_FEEDBACK_COEFFICIENT_PER_KELVIN),
                ThermodynamicTemperature::new::<degree_celsius>(FUEL_REFERENCE_TEMPERATURE_DEGC),
                ThermodynamicTemperature::new::<degree_celsius>(500.0),
                Power::new::<watt>(1.0),
            )
            .expect("valid NF params")
    }

    /// Runs the isolated kinetics + pebble-bed feedback loop headlessly and
    /// returns the reactor thermal-power trace (MW) per step. When
    /// `use_delayed_layer` is true the prompt power is routed through the
    /// five-group [`DelayedNeutronLayer`] before it heats the fuel; when
    /// false it is used directly (the prompt-only path that oscillates).
    ///
    /// The per-step physics mirrors
    /// [`FHRSimulatorApp::calculate_prke_for_one_timestep`] exactly, minus the
    /// display bookkeeping, with the coolant held at a fixed sink temperature
    /// so the fast fuel-temperature loop is isolated from the slower TH loop.
    fn run_power_trace(use_delayed_layer: bool, dt: Time, n_steps: usize) -> Vec<f64> {
        let mut fhr_state = FHRState::default();
        // fix control rods and a constant coolant sink temperature
        fhr_state.left_cr_insertion_frac = 0.40;
        fhr_state.right_cr_insertion_frac = 0.40;
        fhr_state.pebble_core_temp_degc = 500.0;
        let coolant_temp_degc = 500.0;
        fhr_state.pebble_bed_coolant_temp_degc = coolant_temp_degc;

        let mut nordheim_fuchs = fresh_nordheim_fuchs();
        let mut delayed_layer = DelayedNeutronLayer::u235_five_group(Time::new::<second>(2.31e-4));
        let mut pebble_bed = PebbleBedThermalHydraulics::new();
        let mut decay_heat = DecayHeat::default();
        let mut xe135 = Xenon135Poisoning::default();

        let reactor_volume = Volume::new::<cubic_meter>(0.5);
        let macroscopic_fission_xs = LinearNumberDensity::new::<per_meter>(1.0);

        let mut power_trace = Vec::with_capacity(n_steps);

        for _ in 0..n_steps {
            // rebuild the six-factor base every step, exactly as the loop does
            let mut keff_six_factor = SixFactorFormulaFeedback::default();
            keff_six_factor.p_tnl = Ratio::new::<ratio>(0.9);
            keff_six_factor.p_fnl = Ratio::new::<ratio>(0.7);
            keff_six_factor.eta = Ratio::new::<ratio>(2.2);
            keff_six_factor.p = Ratio::new::<ratio>(0.8);
            keff_six_factor.f = Ratio::new::<ratio>(0.9);
            keff_six_factor.epsilon = Ratio::new::<ratio>(1.03);

            keff_six_factor.control_rod_feedback(
                Ratio::new::<ratio>(fhr_state.left_cr_insertion_frac as f64),
                FHRSimulatorApp::fuel_utilisation_factor_chg_for_control_rod_polynomial,
            );
            keff_six_factor.control_rod_feedback(
                Ratio::new::<ratio>(fhr_state.right_cr_insertion_frac as f64),
                FHRSimulatorApp::fuel_utilisation_factor_chg_for_control_rod_polynomial,
            );

            let xe135_mass_conc = xe135.get_current_xe135_conc();
            let xe_factor: f64 =
                Xenon135Poisoning::simplified_poison_concentration_feedback(xe135_mass_conc)
                    .get::<ratio>();
            keff_six_factor.f *= xe_factor;

            let reactivity_ex_fuel_temp: Ratio = keff_six_factor.calc_rho();

            nordheim_fuchs.fuel_temperature =
                ThermodynamicTemperature::new::<degree_celsius>(fhr_state.pebble_core_temp_degc);
            nordheim_fuchs.set_external_reactivity(reactivity_ex_fuel_temp);
            nordheim_fuchs.step(dt);

            let prompt_power = nordheim_fuchs.power;
            let fission_power = if use_delayed_layer {
                // Lie-split point kinetics: total = prompt + delayed source
                // increment, written back so NF propagates the full power.
                let increment = delayed_layer.advance(prompt_power, dt);
                let total = prompt_power + increment;
                nordheim_fuchs.power = total;
                total
            } else {
                prompt_power
            };

            // xenon back-derivation (mirrors the shipped code)
            let power_per_fission = Energy::new::<megaelectronvolt>(200.0);
            let fission_rate: Frequency = fission_power / power_per_fission;
            let fission_rate_density: VolumetricNumberRate = (fission_rate / reactor_volume).into();
            let neutron_speed: Velocity = Velocity::new::<meter_per_second>(2200.0);
            let current_neutron_flux: ArealNumberRate =
                (fission_rate_density / macroscopic_fission_xs).into();
            let current_neutron_pop_density: VolumetricNumberDensity =
                (current_neutron_flux / neutron_speed).into();
            let _ = xe135.calc_xe_135_and_return_num_density(
                dt,
                fission_rate_density,
                FissioningNuclideType::U235,
                current_neutron_pop_density,
            );

            decay_heat.add_decay_heat_precursor1(fission_power * 0.04, dt);
            decay_heat.add_decay_heat_precursor2(fission_power * 0.04, dt);
            decay_heat.add_decay_heat_precursor3(fission_power * 0.02, dt);

            let mut fission_power_corrected = fission_power * 0.9;
            let mut decay_power: Power = decay_heat.calc_decay_heat_power_1(dt).abs();
            decay_power += decay_heat.calc_decay_heat_power_2(dt).abs();
            decay_power += decay_heat.calc_decay_heat_power_3(dt).abs();
            fission_power_corrected += decay_power;

            let pebble_bed_mass = Mass::new::<kilogram>(8000.0);
            let pebble_bed_area = Area::new::<square_meter>(300.0);
            let pebble_bed_htc = HeatTransfer::new::<watt_per_square_meter_kelvin>(400.0);
            let pebble_bed_coolant_temp =
                ThermodynamicTemperature::new::<degree_celsius>(coolant_temp_degc);

            let _heat_removal = pebble_bed.calc_th_and_return_heat_removal_from_pebble_bed(
                dt,
                fission_power_corrected,
                pebble_bed_mass,
                pebble_bed_area,
                pebble_bed_htc,
                pebble_bed_coolant_temp,
            );
            let pebble_fuel_temp = pebble_bed.get_temperature_from_enthalpy_uo2_heuristic();
            fhr_state.pebble_core_temp_degc = pebble_fuel_temp.get::<degree_celsius>();

            power_trace.push(fission_power_corrected.get::<megawatt>());
        }

        power_trace
    }

    /// Peak-to-peak amplitude of the tail window (last 40 %) of a trace.
    fn tail_peak_to_peak(trace: &[f64]) -> f64 {
        let start = (trace.len() as f64 * 0.6) as usize;
        let tail = &trace[start..];
        let max = tail.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min = tail.iter().cloned().fold(f64::INFINITY, f64::min);
        max - min
    }

    /// Mean of the tail window (last 40 %) of a trace.
    fn tail_mean(trace: &[f64]) -> f64 {
        let start = (trace.len() as f64 * 0.6) as usize;
        let tail = &trace[start..];
        tail.iter().sum::<f64>() / tail.len() as f64
    }

    /// V&V regression for op-dt3.11.
    ///
    /// # Methodology
    /// See the module doc. Run the isolated kinetics + pebble-bed feedback
    /// loop for 80 s at dt = 5 ms, prompt-only ("before") vs with the
    /// five-group delayed-neutron layer ("after"), and compare the tail
    /// (last 40 %) peak-to-peak reactor-power amplitude.
    ///
    /// # Results (data 2026-07-15)
    /// Measured tail peak-to-peak amplitude: **before ~ (see stderr)**,
    /// **after ~ (see stderr)** MW; the assertions below fix the pass
    /// criterion (>= 20x reduction and small absolute residual). Exact numbers
    /// print with `cargo test -p outram-park-digital-twin-engine --example
    /// fhr_sim_v2 --release delayed_neutron_layer_damps -- --nocapture`.
    #[test]
    fn delayed_neutron_layer_damps_power_oscillation() {
        let dt = Time::new::<millisecond>(5.0);
        let n_steps = 16_000; // 80 s

        let before = run_power_trace(false, dt, n_steps);
        let after = run_power_trace(true, dt, n_steps);

        let pp_before = tail_peak_to_peak(&before);
        let pp_after = tail_peak_to_peak(&after);
        let mean_before = tail_mean(&before);
        let mean_after = tail_mean(&after);

        eprintln!(
            "op-dt3.11 regression (dt=5ms, 80s): \n  prompt-only  tail peak-to-peak = {pp_before:.4} MW, tail mean = {mean_before:.4} MW\n  delayed-layer tail peak-to-peak = {pp_after:.4} MW, tail mean = {mean_after:.4} MW\n  damping factor = {:.1}x",
            pp_before / pp_after.max(1e-12)
        );

        // The prompt-only path must actually oscillate (otherwise there is no
        // bug to fix and the test would be vacuous).
        assert!(
            pp_before > 0.5,
            "prompt-only path should show a sizeable power oscillation; \
             tail peak-to-peak was only {pp_before:.4} MW"
        );

        // The delayed-neutron layer must damp it: at least a 20x reduction in
        // tail peak-to-peak amplitude.
        assert!(
            pp_after * 20.0 < pp_before,
            "delayed-neutron layer should cut tail peak-to-peak by >= 20x; \
             before = {pp_before:.4} MW, after = {pp_after:.4} MW"
        );

        // ...and leave the tail nearly flat in absolute terms (small ripple
        // relative to the operating power).
        assert!(
            pp_after < 0.05 * mean_after.abs().max(1e-6),
            "delayed-layer tail ripple {pp_after:.4} MW should be < 5% of the \
             operating power {mean_after:.4} MW"
        );
    }

    /// Confirms the *shipped* per-timestep entry point
    /// [`FHRSimulatorApp::calculate_prke_for_one_timestep`] (which always runs
    /// the delayed-neutron layer) settles rather than rings, tying the shipped
    /// code path to the damping demonstrated above.
    ///
    /// # Results (data 2026-07-15)
    /// Driving the shipped timestep for 80 s at dt = 5 ms leaves the tail
    /// (last 40 %) reactor-power peak-to-peak amplitude small relative to the
    /// operating power (assertion below).
    #[test]
    fn shipped_prke_timestep_settles() {
        let dt = Time::new::<millisecond>(5.0);
        let n_steps = 16_000;

        let mut fhr_state = FHRState::default();
        fhr_state.left_cr_insertion_frac = 0.40;
        fhr_state.right_cr_insertion_frac = 0.40;
        fhr_state.pebble_core_temp_degc = 500.0;
        fhr_state.pebble_bed_coolant_temp_degc = 500.0;

        let mut nordheim_fuchs = fresh_nordheim_fuchs();
        let mut delayed_layer = DelayedNeutronLayer::u235_five_group(Time::new::<second>(2.31e-4));
        let mut pebble_bed = PebbleBedThermalHydraulics::new();
        let mut decay_heat = DecayHeat::default();
        let mut xe135 = Xenon135Poisoning::default();
        let reactor_volume = Volume::new::<cubic_meter>(0.5);
        let macroscopic_fission_xs = LinearNumberDensity::new::<per_meter>(1.0);

        let mut trace = Vec::with_capacity(n_steps);
        for _ in 0..n_steps {
            let mut keff_six_factor = SixFactorFormulaFeedback::default();
            keff_six_factor.p_tnl = Ratio::new::<ratio>(0.9);
            keff_six_factor.p_fnl = Ratio::new::<ratio>(0.7);
            keff_six_factor.eta = Ratio::new::<ratio>(2.2);
            keff_six_factor.p = Ratio::new::<ratio>(0.8);
            keff_six_factor.f = Ratio::new::<ratio>(0.9);
            keff_six_factor.epsilon = Ratio::new::<ratio>(1.03);

            FHRSimulatorApp::calculate_prke_for_one_timestep(
                &mut fhr_state,
                &mut keff_six_factor,
                &mut nordheim_fuchs,
                dt,
                reactor_volume,
                macroscopic_fission_xs,
                &mut decay_heat,
                &mut pebble_bed,
                &mut xe135,
                &mut delayed_layer,
            );
            trace.push(fhr_state.reactor_power_megawatts);
        }

        let pp = tail_peak_to_peak(&trace);
        let mean = tail_mean(&trace);
        eprintln!(
            "op-dt3.11 shipped-path settle check: tail peak-to-peak = {pp:.4} MW, tail mean = {mean:.4} MW"
        );
        assert!(
            pp < 0.05 * mean.abs().max(1e-6),
            "shipped delayed-layer timestep tail ripple {pp:.4} MW should be < 5% of operating power {mean:.4} MW"
        );
    }
}
