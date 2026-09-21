// SPDX-License-Identifier: GPL-3.0

//! The joined chain in one run: NP-MHTGR fission-product release under a
//! prescribed heat-up, carried downwind by `changi`. Reports the activity
//! released (Bq and Ci), the time-integrated air concentration (Bq·s/m³) and
//! the dry deposition (Bq/m²) against distance.
//!
//! ```bash
//! cargo run --release -p sembawang --example npmhtgr_chain
//! ```
//!
//! ```text
//! prescribed transient + inventory
//!   -> boon-lay TRISO-ATOPS release          (verified code-to-code vs upstream)
//!   -> sembawang::accident_release           (orchestration, no upstream)
//!   -> sembawang::chain::pad_for_dispersion  (plumbing, no upstream)
//!   -> changi::activity dilution + survey    (Gaussian puff, `puff` 0.1.1 port)
//! ```
//!
//! GitHub issue #236, under epic #226.
//!
//! **Research and education only. No dose quantity is computed.** Every input
//! is prescribed:
//!
//! - **Inventory:** [`CoreInventory::unit`], 1 Ci of each nuclide per radial
//!   ring. Not a physical inventory. Every number printed is linear in it.
//! - **Accident-phase failure fractions:** `1e-4` / `1e-4`, a shakedown choice,
//!   **not cited**.
//! - **Temperature:** an analytic ramp and hold. Nothing here solves for it.
//! - **Weather:** Pasquill class D and 4 m/s, held constant for the whole
//!   ~28 h release. Generic, not any site's meteorology. A real 28 h release
//!   would see the weather change.
//! - **Release height:** 30 m, a caller's choice.
//! - **Deposition velocities:** `changi`'s uncited order-of-magnitude
//!   placeholders.
//!
//! The release half is verified code-to-code in `boon-lay`. The dispersion
//! half (`changi::puff`) is verified code-to-code against upstream R. The
//! activity layer and this join have **no upstream**; they are covered by
//! consistency tests only (`tests/chain_handoff.rs`,
//! `changi/tests/activity_properties.rs`). Nothing here has been compared
//! against measured release or dispersion data.
//!
//! # Time-step convergence (measured 2026-09-21)
//!
//! Puff emission and simulation step `STEP_S` was varied, everything else
//! fixed. I-131 time-integrated air concentration at 1.5 m [Bq·s/m³]:
//!
//! | x [m] | 20 s | 10 s | 5 s |
//! |---|---|---|---|
//! | 100 | 1.85e-5 | 7.61e-4 | 5.02e-4 |
//! | 200 | 3.49 | 7.79 | 7.35 |
//! | 500 | 137.1 | 132.0 | 132.0 |
//! | 1000 | 98.28 | 98.28 | 98.28 |
//! | 8000 | 6.131 | 6.131 | 6.131 |
//!
//! From 500 m out, 10 s and 5 s agree to four significant figures, so the
//! example uses 10 s. **The 100 m and 200 m rows are not converged** and
//! must not be quoted. They are the far tail of a plume released at 30 m
//! that has not yet reached breathing height, sampled coarsely by puffs that
//! move 40 m per step. The value there is small and step-dependent.
//!
//! Cross-check against `changi`'s `site_activity_survey` (10 s step, 1 Ci
//! over 1 h): its I-131 at 1 km, 8.68e5 Bq·s/m³ for 3.7e10 Bq, scaled to this
//! run's 4.19e6 Bq, predicts 98.4. This run gives 98.28. The small
//! difference is expected, since the release here is spread over 28 h, not 1 h.

use changi::activity::chi_over_q::{dilution_factors, StabilitySource};
use changi::activity::survey::{survey, total_released, DepositionVelocities};
use changi::puff::simulate::{constant_wind, EmissionPolicy, Receptor, RunConfig, Source};
use changi::puff::stability::StabilityClass;
use sembawang::accident::release::{accident_release, PlantParameters};
use sembawang::chain::pad_for_dispersion;
use sembawang::inventory::CoreInventory;
use sembawang::scenario::TemperatureTransient;
use uom::si::f64::{Length, ThermodynamicTemperature, Time, Velocity};
use uom::si::length::meter;
use uom::si::radioactivity::{becquerel, curie};
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// Nuclides carried. Each must be one of TRISO-ATOPS' 84.
const NUCLIDES: [&str; 5] = ["Kr-85", "Kr-88", "I-131", "Te-132", "Cs-137"];

/// Radial rings and axial nodes.
const N_RADIAL: usize = 2;
const N_AXIAL: usize = 3;

/// The prescribed transient: start, peak, ramp time, total time, samples.
const START_C: f64 = 700.0;
const PEAK_C: f64 = 1600.0;
const RAMP_S: f64 = 1.0e4;
const TOTAL_S: f64 = 1.0e5;
const SAMPLES: usize = 51;

/// Receptor distances, metres downwind on the plume centreline.
const DISTANCES_M: [f64; 8] = [100.0, 200.0, 500.0, 1000.0, 2000.0, 3000.0, 5000.0, 8000.0];

/// Weather, release height and puff timing.
const WIND_SPEED_M_PER_S: f64 = 4.0;
const RELEASE_HEIGHT_M: f64 = 30.0;
const STEP_S: f64 = 10.0;
/// Puffs are dropped at this age. At 4 m/s the reach is 10 km, beyond the
/// farthest receptor. The dispersion run continues this long after the last
/// release.
const PUFF_LIFETIME_S: f64 = 2500.0;

fn main() {
    // ---- release: sembawang over boon-lay's TRISO-ATOPS fork ----
    let inventory = CoreInventory::unit(&NUCLIDES, N_RADIAL, N_AXIAL);
    let transient = TemperatureTransient::from_ramp(
        ThermodynamicTemperature::new::<degree_celsius>(START_C),
        ThermodynamicTemperature::new::<degree_celsius>(PEAK_C),
        Time::new::<second>(RAMP_S),
        Time::new::<second>(TOTAL_S),
        SAMPLES,
        N_RADIAL,
        N_AXIAL,
    )
    .expect("the transient has enough samples");
    // Accident-phase fractions 1e-4 / 1e-4 are a shakedown choice, not cited.
    let plant = PlantParameters::np_mhtgr_reference(1.0e-4, 1.0e-4, 0.0);
    let release = accident_release(&inventory, &transient, &plant)
        .expect("the release calculation runs");

    // ---- handoff: pad the windows so they partition the dispersion run ----
    let term = pad_for_dispersion(&release.source_term, Time::new::<second>(PUFF_LIFETIME_S));
    let run = term.end();

    // ---- dispersion and deposition: changi ----
    let n_steps = (run.get::<second>() / STEP_S) as usize + 1;
    let config = RunConfig {
        sim_dt: Time::new::<second>(STEP_S),
        puff_dt: Time::new::<second>(STEP_S),
        output_dt: Time::new::<second>(STEP_S),
        duration: run,
        puff_duration: Time::new::<second>(PUFF_LIFETIME_S),
        start_hour: 12,
        emission_policy: EmissionPolicy::OnePuffPerEmission,
    };
    let source = Source {
        x: Length::new::<meter>(0.0),
        y: Length::new::<meter>(0.0),
        height: Length::new::<meter>(RELEASE_HEIGHT_M),
    };
    let receptors_at = |z: f64| -> Vec<Receptor> {
        DISTANCES_M
            .iter()
            .map(|d| Receptor {
                x: Length::new::<meter>(*d),
                y: Length::new::<meter>(0.0),
                z: Length::new::<meter>(z),
            })
            .collect()
    };
    let wind = constant_wind(
        Velocity::new::<meter_per_second>(WIND_SPEED_M_PER_S),
        Velocity::new::<meter_per_second>(0.0),
        n_steps,
    );
    let class = StabilityClass::D;
    let bounds = term.segment_boundaries();
    let factors_at = |z: f64| {
        dilution_factors(
            &[source],
            &bounds,
            &wind,
            &receptors_at(z),
            &config,
            StabilitySource::Fixed(class),
        )
    };
    let air = factors_at(1.5);
    let ground = factors_at(0.0);
    let velocities = DepositionVelocities::order_of_magnitude_placeholder();
    let result = survey(&term, &air, &ground, &velocities);

    // ---- report ----
    println!("NP-MHTGR release carried downwind -- sembawang -> changi, one run");
    println!("RESEARCH AND EDUCATION ONLY. Unit inventory (1 Ci per nuclide per ring); no dose computed.");
    println!(
        "Transient {START_C:.0} C -> {PEAK_C:.0} C over {RAMP_S:.0} s, held to {TOTAL_S:.0} s. \
         Weather: class {}, {WIND_SPEED_M_PER_S} m/s, release height {RELEASE_HEIGHT_M} m.",
        class.letter()
    );
    println!(
        "{} release windows, padded to {} for a {:.0} s dispersion run.",
        release.source_term.windows.len(),
        term.windows.len(),
        run.get::<second>()
    );
    println!();

    println!("Released");
    println!(
        "  {:<7}  {:>12}  {:>12}  {:>12}  {:>10}",
        "nuclide", "Bq", "Ci", "fraction", "group"
    );
    for n in &term.nuclides {
        let q = n.total_released();
        let inv_ci = inventory
            .nuclides
            .iter()
            .find(|i| i.name == n.label)
            .map_or(f64::NAN, |i| i.total().get::<curie>());
        println!(
            "  {:<7}  {:>12.4e}  {:>12.4e}  {:>12.4e}  {:>10}",
            n.label,
            q.get::<becquerel>(),
            q.get::<curie>(),
            q.get::<curie>() / inv_ci,
            n.deposition_group.label()
        );
    }
    let total = total_released(&term);
    println!(
        "  {:<7}  {:>12.4e}  {:>12.4e}",
        "total",
        total.get::<becquerel>(),
        total.get::<curie>()
    );
    println!();

    println!("Time-integrated air concentration at 1.5 m, centreline [Bq.s/m3]");
    print_table(&term, &result, |t| t.air.becquerel_seconds_per_cubic_meter());
    println!();
    println!("Dry deposition at ground level, centreline [Bq/m2]");
    print_table(&term, &result, |t| t.ground.becquerel_per_square_meter());
    println!();

    println!("Upstream behaviours that affected the release:");
    for line in release.caveats.lines() {
        println!("  * {line}");
    }
    println!();
    println!("Reading these numbers:");
    println!("  * Linear in the inventory: scale by a real inventory (in Ci per nuclide) to rescale.");
    println!("  * Kr-88 releases like Kr-85 because the half-life cancels in the release bookkeeping,");
    println!("    but it DOES decay in transit here, so its air column falls off faster with distance.");
    println!("  * Noble gases deposit exactly zero. The Bq/m2 table uses UNCITED placeholder");
    println!("    deposition velocities: order-of-magnitude plumbing, not a result.");
    println!("  * One weather condition held for ~28 h is a simplification a real release would not meet.");
    println!("  * Dry deposition only, not depleting; no wet deposition, plume rise or building wake.");
    println!("  * The 100 m and 200 m rows are NOT converged in the time step (see the doc comment);");
    println!("    do not quote them. From 500 m out, 10 s and 5 s steps agree to 4 significant figures.");
    println!("  * Pasquill-Gifford is fitted over roughly 0.1-10 km. The near rows are low because the");
    println!("    plume from 30 m has not yet reached breathing height.");
    println!(
        "  * Puffs are dropped at {PUFF_LIFETIME_S:.0} s (reach {:.0} m).",
        air.reach().get::<meter>()
    );
}

/// Print one quantity per nuclide (columns) against distance (rows).
fn print_table(
    term: &changi::activity::source::SourceTerm,
    result: &changi::activity::survey::SiteSurvey,
    value: impl Fn(&changi::activity::survey::NuclideTotals) -> f64,
) {
    print!("  {:>7}", "x [m]");
    for n in &term.nuclides {
        print!("  {:>11}", n.label);
    }
    println!();
    for (r, d) in DISTANCES_M.iter().enumerate() {
        print!("  {d:>7.0}");
        for t in result.at(r) {
            print!("  {:>11.3e}", value(t));
        }
        println!();
    }
}
