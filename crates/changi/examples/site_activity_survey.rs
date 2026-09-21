// SPDX-License-Identifier: GPL-3.0

//! A prescribed release carried downwind: activity released (Bq and Ci), then
//! time-integrated air concentration (Bq·s/m³) and dry deposition (Bq/m²)
//! against distance.
//!
//! ```bash
//! cargo run --release -p changi --example site_activity_survey
//! ```
//!
//! This is the end-to-end demonstration of `changi::activity` with a
//! **prescribed** source term (GitHub issue #229): `sembawang` is not involved,
//! so the chain stands on its own.
//!
//! **Research and education only.** The release is an illustrative round
//! number, not a plant value. No dose quantity is computed. The dispersion model
//! is the Gaussian puff ported from the R package `puff` 0.1.1 —
//! Pasquill-Gifford sigmas fitted over roughly 0.1-10 km, no turbulence
//! closure, dry conditions. It is **not** FLEXPART.
//!
//! The `changi::activity` layer has **no upstream and no code-to-code
//! verification**; its consistency checks are in
//! `crates/changi/docs/activity-consistency-checks.md`. The deposition
//! velocities are **uncited order-of-magnitude placeholders**. Nothing here has
//! been compared against measured dispersion.

use changi::activity::chi_over_q::{dilution_factors, StabilitySource};
use changi::activity::deposition::DepositionGroup;
use changi::activity::source::{NuclideRelease, ReleaseWindow, SourceTerm};
use changi::activity::survey::{survey, total_released, DepositionVelocities};
use changi::puff::simulate::{constant_wind, EmissionPolicy, Receptor, RunConfig, Source};
use changi::puff::stability::StabilityClass;
use uom::si::f64::{Frequency, Length, Radioactivity, Time, Velocity};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::radioactivity::{becquerel, curie};
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// Receptor distances, metres downwind on the plume centreline.
const DISTANCES_M: [f64; 8] = [100.0, 200.0, 500.0, 1000.0, 2000.0, 3000.0, 5000.0, 8000.0];

/// Wind speed, held constant.
const WIND_SPEED_M_PER_S: f64 = 4.0;

/// Simulation and emission step.
const STEP_S: f64 = 10.0;

/// The release lasts one hour, at constant rate, split into this many windows.
const RELEASE_S: f64 = 3600.0;
const N_RELEASE_WINDOWS: usize = 6;

/// Puffs are dropped at this age. At 4 m/s the reach is 10 km, beyond the
/// farthest receptor.
const PUFF_LIFETIME_S: f64 = 2500.0;

/// Activity released per nuclide over the hour. **Illustrative, not a plant
/// value** — everything downstream is linear in it.
const RELEASED_CI: f64 = 1.0;

/// Half-lives, seconds (Kr-88 2.84 h, I-131 8.02 d).
const KR88_HALF_LIFE_S: f64 = 2.84 * 3600.0;
const I131_HALF_LIFE_S: f64 = 8.02 * 86_400.0;

fn main() {
    // The run carries on for one puff lifetime after the release stops, so the
    // last puffs reach the far receptors. That tail is its own window with zero
    // release, which keeps the windows contiguous as SourceTerm requires.
    let run_s = RELEASE_S + PUFF_LIFETIME_S;
    let window_s = RELEASE_S / N_RELEASE_WINDOWS as f64;
    let mut bounds: Vec<Time> = (0..=N_RELEASE_WINDOWS)
        .map(|i| Time::new::<second>(i as f64 * window_s))
        .collect();
    bounds.push(Time::new::<second>(run_s));
    let windows: Vec<ReleaseWindow> = bounds
        .windows(2)
        .map(|w| ReleaseWindow::new(w[0], w[1]))
        .collect();

    let per_window = Radioactivity::new::<curie>(RELEASED_CI / N_RELEASE_WINDOWS as f64);
    let mut released = vec![per_window; N_RELEASE_WINDOWS];
    released.push(Radioactivity::new::<becquerel>(0.0));

    let lambda = |half_life_s: f64| Frequency::new::<hertz>(core::f64::consts::LN_2 / half_life_s);
    let term = SourceTerm::new(
        windows,
        vec![
            NuclideRelease {
                label: "Kr-88".to_string(),
                decay_constant: lambda(KR88_HALF_LIFE_S),
                deposition_group: DepositionGroup::NobleGas,
                released: released.clone(),
            },
            NuclideRelease {
                label: "I-131".to_string(),
                decay_constant: lambda(I131_HALF_LIFE_S),
                deposition_group: DepositionGroup::Halogen,
                released,
            },
        ],
    );

    let n_steps = (run_s / STEP_S) as usize + 1;
    let config = RunConfig {
        sim_dt: Time::new::<second>(STEP_S),
        puff_dt: Time::new::<second>(STEP_S),
        output_dt: Time::new::<second>(STEP_S),
        duration: Time::new::<second>(run_s),
        puff_duration: Time::new::<second>(PUFF_LIFETIME_S),
        start_hour: 12,
        emission_policy: EmissionPolicy::OnePuffPerEmission,
    };
    let source = Source {
        x: Length::new::<meter>(0.0),
        y: Length::new::<meter>(0.0),
        height: Length::new::<meter>(30.0),
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

    println!("Site activity survey -- prescribed release, Gaussian puff (`puff` 0.1.1 port)");
    println!("RESEARCH AND EDUCATION ONLY. Illustrative release; no dose computed.");
    println!(
        "Pasquill class {}, wind {WIND_SPEED_M_PER_S} m/s, release height {} m, \
         {RELEASE_S:.0} s release at constant rate",
        class.letter(),
        source.height.get::<meter>()
    );
    println!();

    println!("Released");
    println!("  {:<7}  {:>12}  {:>10}  {:>10}", "nuclide", "Bq", "Ci", "group");
    for n in &term.nuclides {
        let q = n.total_released();
        println!(
            "  {:<7}  {:>12.4e}  {:>10.4}  {:>10}",
            n.label,
            q.get::<becquerel>(),
            q.get::<curie>(),
            n.deposition_group.label()
        );
    }
    let total = total_released(&term);
    println!(
        "  {:<7}  {:>12.4e}  {:>10.4}",
        "total",
        total.get::<becquerel>(),
        total.get::<curie>()
    );
    println!();

    println!("Downwind, centreline (air at 1.5 m, deposition at ground level)");
    println!(
        "  {:>7}  {:>15}  {:>15}  {:>15}  {:>15}",
        "x [m]", "Kr-88 [Bq.s/m3]", "I-131 [Bq.s/m3]", "Kr-88 [Bq/m2]", "I-131 [Bq/m2]"
    );
    for (r, d) in DISTANCES_M.iter().enumerate() {
        let row = result.at(r);
        println!(
            "  {:>7.0}  {:>15.4e}  {:>15.4e}  {:>15.4e}  {:>15.4e}",
            d,
            row[0].air.becquerel_seconds_per_cubic_meter(),
            row[1].air.becquerel_seconds_per_cubic_meter(),
            row[0].ground.becquerel_per_square_meter(),
            row[1].ground.becquerel_per_square_meter(),
        );
    }
    println!();

    println!("Reading these numbers:");
    println!("  * Everything is linear in the release: scale by your own Bq to rescale.");
    println!("  * Kr-88 deposits exactly zero -- it is a noble gas.");
    println!(
        "  * I-131 deposition uses an UNCITED placeholder velocity ({:.0e} m/s). Treat the\n    \
         Bq/m2 column as order-of-magnitude plumbing, not a result.",
        velocities.halogen.meters_per_second()
    );
    println!(
        "  * Dry deposition only, and it does not deplete the plume. No wet deposition,\n    \
         plume rise, building wake or daughter ingrowth."
    );
    println!(
        "  * The near rows are low because the release is 30 m up: at 100-200 m the plume\n    \
         has not yet spread down to breathing height. The ground-level peak is further out."
    );
    println!(
        "  * Pasquill-Gifford is fitted over roughly 0.1-10 km. The 100 m row is at the\n    \
         edge of that range, and a real site boundary is often closer still."
    );
    println!(
        "  * The emission step contributes nothing (an unmoved puff reports zero), which\n    \
         under-counts the very nearest receptors."
    );
    println!(
        "  * Puffs are dropped at {PUFF_LIFETIME_S:.0} s (reach {:.0} m); a receptor beyond\n    \
         the reach would read exactly zero with no error.",
        air.reach().get::<meter>()
    );
}
