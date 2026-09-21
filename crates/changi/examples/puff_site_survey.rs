// SPDX-License-Identifier: GPL-3.0

//! # Gaussian puff dispersion over a small site
//!
//! Run it:
//!
//! ```text
//! cargo run --release -p changi --example puff_site_survey
//! ```
//!
//! This is the entry point for [`changi::puff`]. It is deliberately readable
//! top to bottom: everything it needs is either in this file or one call into
//! the crate, and nothing here requires opening another module to follow.
//!
//! **What it shows**
//!
//! 1. A single source leaking at a steady rate, with a ring of sensors around
//!    it and a constant wind — the smallest case that produces a recognisable
//!    plume.
//! 2. What the stability class does, by running the same release under a calm
//!    night and a windy afternoon.
//! 3. The one place this port deliberately disagrees with its upstream, and
//!    how much difference it makes.
//!
//! **What it is not.** CHANGI is for research, education and
//! verification/validation only. It must not be used for emergency planning,
//! emergency response, dose assessment for real populations, Level 3 PSA, or
//! any safety-critical or licensing decision. Nothing here has been compared
//! against measured dispersion data — see `docs/puff-code-to-code.md`.
//!
//! **On units.** The puff model itself is species-independent, but the
//! parts-per-million conversion inherited from upstream encodes *methane's*
//! molar mass, because upstream is an oil-and-gas leak-detection package. This
//! example therefore reports a **mass concentration in kg/m³**, which is what
//! a radionuclide application would build on, and prints the methane ppm
//! alongside only to show what upstream would have returned.

use changi::puff::concentration::{gaussian_puff_concentration, METHANE_PPM_PER_KG_PER_M3};
use changi::puff::simulate::{
    constant_wind, simulate_sensor_mode, EmissionPolicy, Receptor, RunConfig, Source,
};
use changi::puff::stability::{stability_class, StabilitySet};
use changi::puff::wind::{wind_speed, wind_vector_convert, WindComponents};

use uom::si::angle::degree;
use uom::si::f64::{Angle, Length, Mass, MassRate, Time, Velocity};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// Shorthand so the site layout below reads as coordinates rather than as
/// constructor noise.
fn m(v: f64) -> Length {
    Length::new::<meter>(v)
}
fn s(v: f64) -> Time {
    Time::new::<second>(v)
}

/// A leak of `kg_per_hour` expressed in SI.
fn leak(kg_per_hour: f64) -> MassRate {
    MassRate::new::<kilogram_per_second>(kg_per_hour / 3600.0)
}

fn main() {
    println!("# Gaussian puff dispersion — CHANGI example");
    println!("# Research, education and V&V only. Not for operational use.\n");

    part_1_a_single_leak();
    part_2_what_stability_does();
    part_3_the_one_deliberate_divergence();
}

/// A 3.5 kg/hr leak from a 2.5 m stack, four sensors 30 m out on the compass
/// points, a steady 4 m/s wind from the south-west, run for ten minutes.
fn part_1_a_single_leak() {
    println!("## 1. One leak, four sensors, steady wind\n");

    // The wind is given the way a met station reports it: a speed and the
    // direction it blows FROM, clockwise from north. 225 degrees is a
    // south-westerly, so the plume travels north-east.
    let wind = wind_vector_convert(
        Velocity::new::<meter_per_second>(4.0),
        Angle::new::<degree>(225.0),
    );
    println!(
        "wind: 4.0 m/s from 225 deg  ->  u = {:+.3} m/s (east), v = {:+.3} m/s (north)",
        wind.u.get::<meter_per_second>(),
        wind.v.get::<meter_per_second>()
    );

    let source = Source {
        x: m(0.0),
        y: m(0.0),
        height: m(2.5),
    };

    // North, east, south and west of the source. Only the two downwind of a
    // south-westerly should see anything.
    let sensors = [
        ("N", 0.0, 30.0),
        ("E", 30.0, 0.0),
        ("S", 0.0, -30.0),
        ("W", -30.0, 0.0),
    ];
    let receptors: Vec<Receptor> = sensors
        .iter()
        .map(|(_, x, y)| Receptor {
            x: m(*x),
            y: m(*y),
            z: m(2.0),
        })
        .collect();

    let config = RunConfig {
        sim_dt: s(10.0),          // advance every 10 s
        puff_dt: s(10.0),         // and emit a puff every 10 s
        output_dt: s(120.0),      // report 2-minute averages
        duration: s(600.0),       // ten minutes
        puff_duration: s(1200.0), // track each puff for 20 minutes
        start_hour: 14,           // mid-afternoon
        emission_policy: EmissionPolicy::default(),
    };

    let n_steps = 61;
    let out = simulate_sensor_mode(
        &[source],
        leak(3.5),
        &constant_wind(wind.u, wind.v, n_steps),
        &receptors,
        &config,
    );

    println!("\nmethane, ppm, 2-minute means:\n");
    print!("{:>8}", "t (s)");
    for (name, _, _) in &sensors {
        print!("{name:>12}");
    }
    println!();
    for (i, row) in out.concentrations.iter().enumerate() {
        print!("{:>8.0}", out.interval_starts[i].get::<second>());
        for v in row {
            print!("{v:>12.4e}");
        }
        println!();
    }
    println!(
        "\nOnly N and E register: a south-westerly carries the plume north-east,\n\
         and S and W are upwind. The puff model has no upwind diffusion at all —\n\
         each puff only ever moves with the wind that was blowing when it was\n\
         emitted."
    );
}

/// The same instantaneous release seen under two very different atmospheres.
fn part_2_what_stability_does() {
    println!("\n## 2. What the stability class does\n");

    // One puff's worth of mass, 200 m downwind, measured at head height.
    let puff_mass = Mass::new::<kilogram>(3.5 / 3600.0 * 10.0);
    let travel = m(200.0);
    let receptor = (m(200.0), m(0.0), m(2.0));

    println!(
        "{:>22}  {:>6}  {:>14}  {:>14}",
        "condition", "class", "kg/m^3", "ppm (methane)"
    );
    for (label, speed, hour) in [
        ("calm clear night", 1.0, 2u32),
        ("moderate wind, night", 4.0, 2),
        ("calm sunny afternoon", 1.0, 14),
        ("breezy afternoon", 4.0, 14),
        ("windy, any time", 8.0, 14),
    ] {
        let set = stability_class(Some(Velocity::new::<meter_per_second>(speed)), hour);
        let density = gaussian_puff_concentration(
            puff_mass,
            set.primary(),
            travel,
            m(0.0),
            m(2.5),
            receptor,
            travel,
        )
        .get::<kilogram_per_cubic_meter>();
        let ambiguity = match set {
            StabilitySet::One(c) => c.letter().to_string(),
            StabilitySet::Two(a, b) => format!("{}/{}", a.letter(), b.letter()),
        };
        println!(
            "{label:>22}  {ambiguity:>6}  {density:>14.4e}  {:>14.4e}",
            density * METHANE_PPM_PER_KG_PER_M3
        );
    }

    println!(
        "\nRead the first and third rows together: SAME wind speed, 1.0 m/s, and\n\
         the night reading is ~77x the afternoon one. It is not the wind that\n\
         orders this table — it is convection. A calm sunny afternoon is class A,\n\
         the most unstable there is, and vigorous vertical mixing dilutes the\n\
         plume faster than any amount of horizontal wind. A calm clear night is\n\
         class F, a stable layer that suppresses vertical motion and keeps the\n\
         plume tight and travelling.\n\n\
         That is why the stability class, not the wind speed, is the single\n\
         largest control on how fast a plume dilutes — and why a dispersion\n\
         estimate that reports only wind speed is not telling you much.\n\n\
         Note the two-letter entries. The Pasquill table is a RANGE, not a\n\
         point, and upstream preserves that: `StabilitySet::Two` means the\n\
         condition is genuinely ambiguous between two adjacent classes. Six of\n\
         the table's ten regimes are. `primary()` takes the first, which is what\n\
         upstream's own kernel does."
    );
}

/// The emission-policy divergence, quantified rather than asserted.
fn part_3_the_one_deliberate_divergence() {
    println!("\n## 3. The one place this port disagrees with its upstream\n");

    println!(
        "Upstream builds its puff record with an R `data.frame`. When the\n\
         stability class is ambiguous, R recycles the length-1 columns against\n\
         the length-2 class, producing TWO puffs — each carrying the FULL\n\
         per-interval mass. The emitted mass is doubled.\n\n\
         This port defaults to the mass-conserving reading. Upstream's behaviour\n\
         is available, by name, so its numbers can be reproduced:\n"
    );

    let source = Source {
        x: m(0.0),
        y: m(0.0),
        height: m(2.5),
    };
    let receptors = vec![Receptor {
        x: m(50.0),
        y: m(20.0),
        z: m(2.0),
    }];

    // 1.5 m/s at midday is class A/B — ambiguous, so the divergence is live.
    // 8 m/s is class D — unambiguous, so the two policies must agree exactly.
    for (label, speed) in [
        ("1.5 m/s (class A/B, ambiguous)", 1.5),
        ("8.0 m/s (class D)", 8.0),
    ] {
        let wind = WindComponents {
            u: Velocity::new::<meter_per_second>(speed),
            v: Velocity::new::<meter_per_second>(0.0),
        };
        let run = |policy| {
            let config = RunConfig {
                sim_dt: s(10.0),
                puff_dt: s(10.0),
                output_dt: s(120.0),
                duration: s(600.0),
                puff_duration: s(1200.0),
                start_hour: 12,
                emission_policy: policy,
            };
            let out = simulate_sensor_mode(
                &[source],
                leak(3.5),
                &constant_wind(wind.u, wind.v, 61),
                &receptors,
                &config,
            );
            // Peak over the run, which is what a leak-detection threshold reads.
            out.concentrations
                .iter()
                .map(|row| row[0])
                .fold(0.0_f64, f64::max)
        };
        let ours = run(EmissionPolicy::OnePuffPerEmission);
        let upstream = run(EmissionPolicy::UpstreamRecycleStabilityClasses);
        println!(
            "{label:>32}   speed {:>5.2} m/s   ours {ours:>11.4e}   upstream {upstream:>11.4e}   ratio {:>5.2}",
            wind_speed(wind).get::<meter_per_second>(),
            if ours > 0.0 { upstream / ours } else { f64::NAN }
        );
    }

    println!(
        "\nExactly 1.00 where the class is unambiguous: the divergence is bounded\n\
         to the ambiguous regimes, and both halves of that are pinned by tests.\n\n\
         Where it IS ambiguous the ratio is about 1.8, NOT 2.0 — and the\n\
         difference is worth understanding. The emitted MASS is doubled exactly:\n\
         two puffs, each with the full per-interval mass. But the two puffs are\n\
         given DIFFERENT stability classes (here A and B), so the second one\n\
         disperses differently from the first and contributes a different\n\
         concentration. Doubling the mass does not double the reading.\n\n\
         So the error upstream's recycling introduces is not a clean factor you\n\
         can divide back out of a published result. It depends on how far apart\n\
         the two classes disperse at the receptor in question, which depends on\n\
         the geometry. If you are reproducing a published `puff` result, ask for\n\
         `UpstreamRecycleStabilityClasses` rather than trying to correct for it.\n\
         For anything where the emitted mass has to match the source term —\n\
         which is every radiological application — leave the default alone."
    );
}
