//! # Reactivity Insertion Transient Example
//!
//! This example demonstrates a **reactivity insertion transient** using six-group
//! point reactor kinetics. We simulate a reactor initially at steady state
//! (zero reactivity), then insert a step of positive reactivity. The neutron
//! population rises, initially driven by prompt neutrons, then stabilized by
//! delayed neutron precursors.
//!
//! ## Physics Background
//!
//! In a nuclear reactor, fissions produce neutrons in two ways:
//! - **Prompt neutrons**: emitted almost instantly (~1e-14 s after fission).
//! - **Delayed neutrons**: emitted by decay of fission products (precursors),
//!   with decay constants ranging from ~0.08 s^-1 to ~3 s^-1.
//!
//! The delayed neutrons are crucial: they prevent the reactor from becoming
//! "prompt critical" (uncontrollable). When reactivity is positive but below
//! the delayed neutron fraction β, the delayed neutrons slow the rise.
//!
//! ## Workflow
//!
//! 1. Create a SixGroupPRKE instance (U-235, default decay constants and delays).
//! 2. Initialize at steady state: zero reactivity, zero source, neutron population = 1 /m³.
//! 3. Run several timesteps to reach equilibrium precursor concentrations.
//! 4. At t=0.5 s, insert a positive reactivity step (20% of β).
//! 5. Continue for ~2 seconds, printing power vs. time every 0.1 seconds.
//! 6. Observe the transient response.
//!
//! ## Expected Behavior
//!
//! - **Before insertion (t < 0.5 s):** Neutron population ≈ 1 /m³ (constant).
//! - **At insertion (t ≈ 0.5 s):** Population jumps slightly due to prompt neutron
//!   generation time feedback.
//! - **After insertion (t > 0.5 s):** Power rises smoothly, driven first by prompt
//!   neutrons, then modulated by delayed precursor decay.

use teh_o_prke::prelude::*;
use uom::si::f64::*;
use uom::si::time::second;
use uom::si::ratio::ratio;
use uom::si::volumetric_number_rate::per_cubic_meter_second;
use uom::si::volumetric_number_density::per_cubic_meter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ========================================================================
    // STEP 1: Set up the reactor kinetics solver
    // ========================================================================

    // Create a SixGroupPRKE instance with default U-235 constants.
    // The default state has:
    // - neutron_population = 1 /m³
    // - precursor concentrations = 0 (need to build up to equilibrium)
    let mut prke = SixGroupPRKE::default();

    // ========================================================================
    // STEP 2: Define simulation parameters
    // ========================================================================

    // Timestep: 0.01 seconds (10 ms). Small enough for stability with the
    // implicit solver, but not so small that the simulation takes forever.
    let dt: Time = Time::new::<second>(0.01);

    // Neutron generation time (prompt neutron lifetime in the reactor core).
    // Typical value: ~1e-4 seconds (100 microseconds).
    let generation_time: Time = Time::new::<second>(1.0e-4);

    // Total delayed neutron fraction β for U-235: ~0.0065 (0.65%).
    let beta = prke.get_total_delayed_fraction();

    // Reactivity to insert: 20% of β (well below prompt critical).
    // Prompt critical would be at ρ = β ≈ 0.0065; we insert 0.0013.
    let reactivity_step: Ratio = Ratio::new::<ratio>(0.2) * beta;

    // No external neutron source.
    let background_source: VolumetricNumberRate =
        VolumetricNumberRate::new::<per_cubic_meter_second>(0.0);

    // Time at which to insert the reactivity step.
    let insertion_time: Time = Time::new::<second>(0.5);

    // Total simulation time.
    let total_time: Time = Time::new::<second>(3.0);

    // Printing interval (print every 0.1 s).
    let print_interval: Time = Time::new::<second>(0.1);

    // ========================================================================
    // STEP 3: Pre-equilibrate (build up precursor concentrations at zero ρ)
    // ========================================================================

    // At steady state with zero reactivity, the precursor concentrations
    // must match the neutron population and the delayed neutron production rate.
    // We reach this by running several "initialization" steps at ρ=0.

    let zero_reactivity: Ratio = Ratio::new::<ratio>(0.0);

    // Run 1000 timesteps (10 seconds) at zero reactivity to reach equilibrium.
    // In reality, precursors reach steady state much faster (~seconds), but
    // we run extra to be safe.
    for _ in 0..1000 {
        prke.solve_next_timestep_precursor_concentration_and_neutron_pop_vector_implicit(
            dt,
            zero_reactivity,
            generation_time,
            background_source,
        )?;
    }

    // ========================================================================
    // STEP 4: Run the transient with reactivity insertion
    // ========================================================================

    let mut time: Time = Time::new::<second>(0.0);
    let mut next_print_time: Time = Time::new::<second>(0.0);

    // Print header for the output table
    println!("Time (s)    | Neutron Pop (n/m³) | Relative Power (%)");
    println!("------------|--------------------|-----------------");

    // Assume initial steady-state power = 1 (normalized). After insertion,
    // power rises relative to this baseline.
    let initial_neutron_pop = prke
        .get_current_neutron_population_density()
        .get::<per_cubic_meter>();

    let timesteps_total = (total_time / dt).get::<ratio>() as usize;

    // Captured either side of the insertion so the prompt jump can be checked
    // against theory at the end.
    //
    // SUBTLETY WORTH UNDERSTANDING: the prompt jump is often called
    // "instantaneous", but it is not. It settles with time constant
    //
    //     tau_prompt = Lambda / (beta - rho)
    //
    // which here is 1e-4 / 0.0052 ~= 0.019 s -- *larger* than our 0.01 s
    // timestep. Sampling one step after insertion therefore catches the jump
    // only ~40% complete (about 1.09, not 1.25). Sample instead after several
    // tau_prompt, while still staying well short of the fastest precursor
    // group (~0.33 s), so the prompt jump has finished but the delayed rise
    // has barely begun. That window is what the approximation describes.
    let tau_prompt = generation_time / (beta - reactivity_step);
    let jump_sample_time = insertion_time + 5.0 * tau_prompt;
    let mut pop_before_insertion: Option<f64> = None;
    let mut pop_after_jump: Option<f64> = None;

    for _step in 0..timesteps_total {
        // Determine current reactivity: step up at insertion_time
        let current_reactivity = if time >= insertion_time {
            reactivity_step
        } else {
            zero_reactivity
        };

        // Grab the population immediately before the first stepped solve.
        if time >= insertion_time && pop_before_insertion.is_none() {
            pop_before_insertion = Some(
                prke.get_current_neutron_population_density()
                    .get::<per_cubic_meter>(),
            );
        }

        // Solve one timestep
        prke.solve_next_timestep_precursor_concentration_and_neutron_pop_vector_implicit(
            dt,
            current_reactivity,
            generation_time,
            background_source,
        )?;

        // ...and again once the prompt transient has settled (5 tau_prompt).
        if pop_before_insertion.is_some() && pop_after_jump.is_none() && time >= jump_sample_time {
            pop_after_jump = Some(
                prke.get_current_neutron_population_density()
                    .get::<per_cubic_meter>(),
            );
        }

        // Increment time
        time = time + dt;

        // Print data at regular intervals
        if time >= next_print_time {
            let neutron_pop = prke
                .get_current_neutron_population_density()
                .get::<per_cubic_meter>();

            // Relative power: neutron population / initial population * 100%
            let relative_power = (neutron_pop / initial_neutron_pop) * 100.0;

            println!(
                "{:10.2} | {:18.6e} | {:17.2}",
                time.get::<second>(),
                neutron_pop,
                relative_power
            );

            next_print_time = next_print_time + print_interval;
        }
    }

    // ========================================================================
    // STEP 5: Interpret the results
    // ========================================================================

    println!("\n=== Interpretation ===");
    println!("Before t=0.5 s:");
    println!("  The reactor is at steady state with zero reactivity.");
    println!("  Neutron population remains constant around 1.0 n/m³.");
    println!("  Delayed neutron precursor concentrations are in equilibrium,");
    println!("  producing just enough delayed neutrons to maintain the population.");
    println!("\nAt t≈0.5 s:");
    println!(
        "  A positive reactivity step is inserted ({:.4} Δk/k).",
        reactivity_step.get::<ratio>()
    );
    println!("  The system is now supercritical (k > 1).");
    println!("\nAfter t=0.5 s:");
    println!("  Power rises smoothly, NOT exponentially.");
    println!(
        "  This smooth rise is because most of the reactivity (ρ ≈ {:.4}) ",
        reactivity_step.get::<ratio>()
    );
    println!(
        "  is well below prompt critical (β ≈ {:.4}).",
        beta.get::<ratio>()
    );
    println!("  Delayed neutrons (with lifetimes from 0.23 s to 56 s) control the rise.");
    println!("  The power increase rate is limited by precursor decay rates.");

    // Self-check against the prompt jump approximation.
    //
    // For a step of reactivity rho well below prompt critical, PRKE predicts an
    // effectively instantaneous jump in power by the factor
    //
    //     P_after / P_before  =  beta / (beta - rho)
    //
    // because the prompt neutron population re-equilibrates on the generation
    // time (here 1e-4 s) while the precursors have not yet had time to move.
    // Everything after that jump is the slow, delayed-neutron-controlled rise.
    //
    // This is the cheapest correctness check available on a PRKE solver, so the
    // example asserts it rather than merely describing it.
    let before = pop_before_insertion.expect("insertion never happened");
    let after = pop_after_jump.expect("no post-insertion step recorded");
    let observed_jump = after / before;

    let rho = reactivity_step.get::<ratio>();
    let b = beta.get::<ratio>();
    let predicted_jump = b / (b - rho);
    println!("\n=== Self-check: prompt jump approximation ===");
    println!(
        "  tau_prompt = Lambda/(beta - rho)               = {:.4} s",
        tau_prompt.get::<second>()
    );
    println!(
        "  sampled at t = insertion + 5*tau_prompt        = {:.3} s",
        jump_sample_time.get::<second>()
    );
    println!(
        "  predicted P_after/P_before = beta/(beta - rho) = {:.4}",
        predicted_jump
    );
    println!(
        "  observed  P_after/P_before                     = {:.4}",
        observed_jump
    );
    let jump_error = (observed_jump - predicted_jump).abs() / predicted_jump;
    println!(
        "  relative difference                           = {:.2}%",
        jump_error * 100.0
    );
    assert!(
        jump_error < 0.02,
        "prompt jump off by {:.2}% -- expected {:.4}, got {:.4}",
        jump_error * 100.0,
        predicted_jump,
        observed_jump
    );
    println!("  -> within 2% of theory. The solver reproduces the prompt jump.");

    Ok(())
}
