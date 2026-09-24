// SPDX-License-Identifier: GPL-3.0

//! Dilution factors (`chi/Q`) against downwind distance, with and without decay
//! in transit.
//!
//! ```bash
//! cargo run --release -p changi --example chi_over_q_survey
//! ```
//!
//! **Research and education only.** This is a demonstration of
//! [`changi::activity::chi_over_q`], not a site assessment. It computes no dose
//! quantity, and the model it runs is the Gaussian puff ported from the R
//! package `puff` 0.1.1 — empirical Pasquill-Gifford sigmas, fitted over
//! roughly 0.1-10 km, no turbulence closure, dry conditions. It is **not**
//! FLEXPART.
//!
//! The numbers it prints have **no upstream and no code-to-code verification**.
//! They have never been compared against measured dispersion.

use changi::activity::chi_over_q::{dilution_factors, StabilitySource};
use changi::puff::climatology::MEAN_WIND_SPEED_M_PER_S;
use changi::puff::simulate::{constant_wind, EmissionPolicy, Receptor, RunConfig, Source};
use changi::puff::stability::StabilityClass;
use uom::si::f64::{Frequency, Length, Time, Velocity};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// Distances to report, metres downwind on the plume centreline.
const DISTANCES_M: [f64; 8] = [100.0, 200.0, 500.0, 1000.0, 2000.0, 3000.0, 4000.0, 5000.0];

/// Wind speed held constant for the whole run — Singapore's **mean surface
/// wind**, about 2 m/s.
///
/// Taken from [`changi::puff::climatology`] rather than written as a round
/// number here, so the figure and its provenance live in one place. It is
/// roughly half the 4 m/s this example used previously, and light winds give
/// systematically higher concentrations: less dilution per unit distance, and
/// a longer transit time for decay to act over. Note also that 2 m/s sits
/// exactly on a Pasquill band edge — see that module for what that costs.
const WIND_SPEED_M_PER_S: f64 = MEAN_WIND_SPEED_M_PER_S;

/// Simulation and emission step. `puff_dt` equals it, so one puff is emitted
/// per step and every travel-time bin is exactly one step wide.
const STEP_S: f64 = 10.0;

/// Long enough for the plume to reach the far receptors and for the near ones
/// to settle.
const DURATION_S: f64 = 3600.0;

/// Puffs are dropped at this age. At 4 m/s it is the 6 km reach the header
/// prints — the far receptors would read exactly zero if it were left at
/// upstream's 1200 s default.
const PUFF_LIFETIME_S: f64 = 1500.0;

fn main() {
    let n_steps = (DURATION_S / STEP_S) as usize + 1;
    let config = RunConfig {
        sim_dt: Time::new::<second>(STEP_S),
        puff_dt: Time::new::<second>(STEP_S),
        output_dt: Time::new::<second>(STEP_S),
        duration: Time::new::<second>(DURATION_S),
        puff_duration: Time::new::<second>(PUFF_LIFETIME_S),
        start_hour: 12,
        emission_policy: EmissionPolicy::OnePuffPerEmission,
    };

    let source = Source {
        x: Length::new::<meter>(0.0),
        y: Length::new::<meter>(0.0),
        height: Length::new::<meter>(30.0),
    };
    let receptors: Vec<Receptor> = DISTANCES_M
        .iter()
        .map(|d| Receptor {
            x: Length::new::<meter>(*d),
            y: Length::new::<meter>(0.0),
            z: Length::new::<meter>(1.5),
        })
        .collect();
    let wind = constant_wind(
        Velocity::new::<meter_per_second>(WIND_SPEED_M_PER_S),
        Velocity::new::<meter_per_second>(0.0),
        n_steps,
    );
    let bounds = [Time::new::<second>(0.0), config.duration];

    println!("chi/Q against downwind distance -- Gaussian puff (`puff` 0.1.1 port)");
    println!(
        "wind {WIND_SPEED_M_PER_S} m/s, release height {} m, receptor height 1.5 m, \
         {DURATION_S:.0} s continuous release",
        source.height.get::<meter>()
    );
    println!();

    // Three stability classes: very unstable, neutral, very stable.
    for class in [StabilityClass::A, StabilityClass::D, StabilityClass::F] {
        let factors = dilution_factors(
            &[source],
            &bounds,
            &wind,
            &receptors,
            &config,
            StabilitySource::Fixed(class),
        );

        println!(
            "Pasquill class {} -- puff reach {:.0} m",
            class.letter(),
            factors.reach().get::<meter>()
        );
        println!(
            "  {:>9}  {:>14}  {:>14}  {:>14}  {:>8}",
            "x [m]", "stable [s/m3]", "Kr-88 [s/m3]", "Kr-89 [s/m3]", "Kr-89/st"
        );

        // Two short-lived noble gases, to show that decay in transit is not a
        // detail: half-lives from the IAEA values `boon-lay` carries.
        let kr88 = Frequency::new::<hertz>(core::f64::consts::LN_2 / (2.84 * 3600.0));
        let kr89 = Frequency::new::<hertz>(core::f64::consts::LN_2 / 189.0);

        for (r, d) in DISTANCES_M.iter().enumerate() {
            let stable = factors
                .dilution(r, 0, Frequency::new::<hertz>(0.0))
                .seconds_per_cubic_meter();
            let a88 = factors.dilution(r, 0, kr88).seconds_per_cubic_meter();
            let a89 = factors.dilution(r, 0, kr89).seconds_per_cubic_meter();
            let ratio = if stable > 0.0 { a89 / stable } else { f64::NAN };
            println!("  {d:>9.0}  {stable:>14.4e}  {a88:>14.4e}  {a89:>14.4e}  {ratio:>8.3}");
        }
        println!();
    }

    println!("Reading these numbers:");
    println!(
        "  * chi/Q is s/m3. Multiply by an activity released in Bq to get a \
         time-integrated\n    air concentration in Bq.s/m3. No dose quantity is computed."
    );
    println!(
        "  * The last column is why decay in transit is not optional: Kr-89 \
         (189 s half-life)\n    loses most of its activity before reaching the far receptors, \
         while the stable\n    column would overstate it by the reciprocal of that ratio."
    );
    println!(
        "  * Class F concentrates more than class A on the centreline because it \
         disperses\n    less -- correct on-axis, and the opposite of what it does off-axis."
    );
    println!(
        "  * Puffs are dropped at {PUFF_LIFETIME_S:.0} s. Any receptor beyond the \
         printed reach\n    reads exactly zero, with no error -- check the reach before \
         trusting a far-field zero."
    );
    println!(
        "  * Pasquill-Gifford is fitted over roughly 0.1-10 km. The 100 m column is \
         below\n    that range, and a real site boundary is often closer still."
    );
}
