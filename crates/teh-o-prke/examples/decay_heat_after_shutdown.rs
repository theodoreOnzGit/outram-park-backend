//! # Decay Heat After Shutdown
//!
//! This example demonstrates why reactor shutdown is not the end of the story:
//! **fission stops almost instantly, but decay heat persists for days.**
//! This is the central safety crisis of every nuclear reactor.
//!
//! ## Physics Background
//!
//! When you stop a reactor by inserting control rods or dropping boron, you
//! kill the chain reaction almost instantly. The fission neutrons vanish in
//! ~1e-4 seconds. But the reactor contains ~200 billion fission products from
//! hours of operation, many of them radioactive. These unstable nuclei decay
//! by emitting beta particles and gamma rays, releasing heat at a rate that is
//! typically 5-7% of full operating power—for weeks.
//!
//! This decay heat must be removed continuously, even with the reactor shut down
//! and cooled. Fukushima Daiichi Unit 1 failed not because of the earthquake
//! or tsunami directly, but because decay-heat removal systems were damaged
//! *after* shutdown and the core melted from residual heat over the next hours.
//!
//! ## Model
//!
//! We use six-group point reactor kinetics to model the fission power during
//! shutdown, and a 23-group exponential-decay model (1978 draft ANS Standard)
//! for the fission products. The reactor is:
//!
//! - **Steady state** at full power with decay heat at equilibrium.
//! - **Scrammed** by inserting a large negative reactivity insertion.
//! - **Cooled** with zero fission power, showing the decay-heat tail.
//!
//! ## Expected Results
//!
//! Immediately after shutdown: ~6-7% of full power as decay heat.
//! After 1 hour: ~1-2% of full power.
//! After 1 day: <0.5% of full power.
//!
//! Even that <0.5% is still dangerous at large scales: a 1000 MW reactor
//! shedding 5 MW of decay heat must reject it continuously or the core will
//! heat up at a rate of tens of °C per hour, leading to fuel damage within
//! minutes. You cannot turn off a nuclear reactor the way you turn off a car.

use teh_o_prke::prelude::*;
use uom::si::f64::*;
use uom::si::time::second;
use uom::si::ratio::ratio;
use uom::si::volumetric_number_rate::per_cubic_meter_second;
use uom::si::volumetric_number_density::per_cubic_meter;
use uom::si::power::megawatt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ========================================================================
    // STEP 1: Set up the point reactor kinetics (PRKE) solver
    // ========================================================================

    let mut prke = SixGroupPRKE::default();

    // Timestep: 0.1 seconds. Large enough to give readable tables in this
    // example, but small enough to capture the transient dynamics.
    let dt: Time = Time::new::<second>(0.1);

    // Neutron generation time (prompt neutron lifetime).
    // Typical value: ~1e-4 seconds (100 microseconds).
    let generation_time: Time = Time::new::<second>(1.0e-4);

    // Total delayed neutron fraction β for U-235.
    let beta = prke.get_total_delayed_fraction();

    // No external neutron source (the reactor is self-sustaining via fission).
    let background_source: VolumetricNumberRate =
        VolumetricNumberRate::new::<per_cubic_meter_second>(0.0);

    // ========================================================================
    // STEP 2: Pre-equilibrate the reactor at steady state (full power)
    // ========================================================================

    // At steady state with zero reactivity, the precursor concentrations must
    // reach equilibrium. We simulate this by running at ρ = 0 for a while.
    let zero_reactivity: Ratio = Ratio::new::<ratio>(0.0);

    println!("=== Pre-equilibration Phase (0 to 10 s) ===");
    println!("Running 100 timesteps at zero reactivity to reach steady state...");

    for _ in 0..100 {
        prke.solve_next_timestep_precursor_concentration_and_neutron_pop_vector_implicit(
            dt,
            zero_reactivity,
            generation_time,
            background_source,
        )?;
    }

    // By this point, precursor concentrations have settled to the steady-state
    // value for zero reactivity. The neutron population is at 1.0 n/m³ by default.
    let steady_state_neutron_pop = prke
        .get_current_neutron_population_density()
        .get::<per_cubic_meter>();

    // Define a "full power" level. In a real reactor this is ~1e18 n/m³. Here
    // we normalize to whatever the steady-state value is.
    let full_power_neutron_density = steady_state_neutron_pop;
    let normalized_full_power_mw = 1000.0; // Nominal 1000 MW reactor

    println!(
        "  Steady-state neutron density: {:.6e} n/m³",
        steady_state_neutron_pop
    );

    // ========================================================================
    // STEP 3: Set up decay heat at equilibrium (full power operation)
    // ========================================================================

    // The decay heat state starts with all groups at their equilibrium values
    // for this fission power. This is the correct starting point for a shutdown
    // transient: if we started cold, we would miss the bulk of the decay heat.
    let full_power: Power = Power::new::<megawatt>(normalized_full_power_mw);
    let mut decay_heat = DecayHeat::new_at_equilibrium(FissioningNuclide::U235Thermal, full_power);

    let equilibrium_decay_heat = decay_heat.total_decay_heat_power();
    let prompt_power_fraction = decay_heat.prompt_power_fraction();

    println!("\n=== Initial Steady State (Full Power Operation) ===");
    println!(
        "Fission power (prompt):      {:.3} MW",
        (full_power * prompt_power_fraction).get::<megawatt>()
    );
    println!(
        "Decay heat power:            {:.3} MW ({:.2}% of full power)",
        equilibrium_decay_heat.get::<megawatt>(),
        (equilibrium_decay_heat / full_power).get::<ratio>() * 100.0
    );
    println!(
        "Total thermal power:         {:.3} MW",
        (full_power * prompt_power_fraction + equilibrium_decay_heat).get::<megawatt>()
    );

    // ========================================================================
    // STEP 4: Scram insertion (large negative reactivity step)
    // ========================================================================

    // A typical scram is worth between -4% and -8% (Δk/k, or sometimes quoted as
    // Δρ in dollars or cents where ρ_dollar = β). Here we insert a scram worth
    // -5%, which is large enough to stop fission immediately.
    let scram_reactivity: Ratio = Ratio::new::<ratio>(-0.05);

    println!("\n=== Scram Inserted ===");
    println!(
        "Inserting negative reactivity: {:.4} Δk/k (scram worth ~${:.1})",
        scram_reactivity.get::<ratio>(),
        scram_reactivity.get::<ratio>() / beta.get::<ratio>() // Approximate dollars
    );

    // ========================================================================
    // STEP 5: Run the transient (neutron power collapse and decay heat decay)
    // ========================================================================

    println!("\n=== Transient Phase (Fission Power Collapse + Decay Heat Decay) ===\n");
    println!(
        "{:<10} | {:<18} | {:<18} | {:<15}",
        "Time (s)", "Fission Pwr (MW)", "Decay Heat (MW)", "Decay Heat (%)"
    );
    println!("{}", "-".repeat(75));

    let mut time: Time = Time::new::<second>(0.0);
    let total_time: Time = Time::new::<second>(3700.0); // ~1 hour + 1 minute
    let print_interval: Time = Time::new::<second>(10.0); // Print every 10 seconds

    let mut next_print_time: Time = Time::new::<second>(0.0);

    let timesteps_total = (total_time / dt).get::<ratio>() as usize;
    let scram_time: Time = Time::new::<second>(0.0); // Scram at t=0 of this phase

    for _step in 0..timesteps_total {
        // At the first step, insert the scram (large negative reactivity).
        let current_reactivity = if time >= scram_time {
            scram_reactivity
        } else {
            zero_reactivity
        };

        // Solve one timestep of PRKE
        prke.solve_next_timestep_precursor_concentration_and_neutron_pop_vector_implicit(
            dt,
            current_reactivity,
            generation_time,
            background_source,
        )?;

        // Get current fission power (relative to steady state, then scaled to MW)
        let current_neutron_pop = prke
            .get_current_neutron_population_density()
            .get::<per_cubic_meter>();
        let relative_power = current_neutron_pop / full_power_neutron_density;
        let current_fission_power =
            Power::new::<megawatt>(normalized_full_power_mw * relative_power);

        // Advance the decay heat with this fission power
        decay_heat.advance_timestep(current_fission_power, dt);

        // Get the total decay heat power
        let current_decay_heat = decay_heat.total_decay_heat_power();
        let decay_heat_fraction = current_decay_heat / full_power;

        // Increment time
        time = time + dt;

        // Print at regular intervals
        if time >= next_print_time {
            println!(
                "{:<10.1} | {:<18.6} | {:<18.6} | {:<14.2}%",
                time.get::<second>(),
                current_fission_power.get::<megawatt>(),
                current_decay_heat.get::<megawatt>(),
                decay_heat_fraction.get::<ratio>() * 100.0
            );

            next_print_time = next_print_time + print_interval;
        }
    }

    // ========================================================================
    // STEP 6: Interpretation and safety context
    // ========================================================================

    println!("\n=== Physical Interpretation ===");
    println!();
    println!("1. FISSION POWER COLLAPSE:");
    println!("   The neutron population drops by a factor of ~1e-4 or more within");
    println!("   seconds of the scram. This is because the large negative reactivity");
    println!(
        "   is far below prompt critical (β ≈ {:.4}), so the prompt neutron",
        beta.get::<ratio>()
    );
    println!("   population equilibrates to a much lower level. The precursors then");
    println!("   decay away over timescales of 1-100 seconds.");
    println!();
    println!("2. DECAY HEAT IS NOT PROPORTIONAL TO FISSION POWER:");
    println!("   The decay heat at t=0+ (immediately after scram) depends on the");
    println!("   fission-product inventory, which was built up during operation.");
    println!("   It does NOT depend on the current fission power, which is now zero.");
    println!("   The decay heat instead follows Tobias equation (32): a sum of 23");
    println!("   exponential decays, each proportional to exp(-lambda_i * t).");
    println!();
    println!("3. TIMESCALE HIERARCHY:");
    println!("   - Fission power collapses: 1-10 seconds");
    println!("   - Decay heat falls to 1-2% of full power: ~1 hour");
    println!("   - Decay heat reaches <0.5% of full power: ~1 day");
    println!("   - Some groups decay over WEEKS (longest half-life ~1 million years)");
    println!();

    println!("=== Why Fukushima Daiichi Melted ===");
    println!();
    println!("Unit 1 was shut down safely on 2011-03-11 at 14:46 JST. The decay heat");
    println!("at shutdown was ~60 MW (6% of 1000 MW). The tsunami destroyed the seawater");
    println!("cooling systems at ~15:35. The core, still generating 60 MW of decay heat,");
    println!("was unpumpable. The fuel temperature rose above 1200°C within hours, steam");
    println!("reacted with zirconium cladding to produce hydrogen, pressure built, and");
    println!("the core was exposed. Full meltdown by ~16:00 on 2011-03-12.");
    println!();
    println!("A modern reactor uses multiple passive cooling systems (natural convection,");
    println!("heat pipes, gravity-fed cooling) that do NOT require pumps, power, or human");
    println!("intervention. These can handle decay heat indefinitely. But they did not");
    println!("exist at Fukushima in 2011, and they were not retrofitted after the accident.");
    println!("Many older reactors worldwide are still in the same vulnerable state.");
    println!();
    println!("The lesson: **The reactor never stops producing heat. It only stops");
    println!("producing power.** If you cannot remove that heat, the core melts.");

    Ok(())
}
