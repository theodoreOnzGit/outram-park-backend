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
//! **Where it runs.** Every condition here is an illustrative **Singapore**
//! one, from [`changi::puff::climatology`] — CHANGI is named for Changi, and
//! the numbers have their provenance recorded in one place rather than being
//! round numbers picked per example. Singapore's mean surface wind is about
//! 2 m/s, which is light enough to matter: see part 2.
//!
//! **What it shows**
//!
//! 1. A single source leaking at a steady rate, with a ring of sensors around
//!    it and the northeast monsoon blowing — the smallest case that produces a
//!    recognisable plume.
//! 2. What the stability class does, across the four monsoon regimes, and why
//!    Singapore's mean wind lands somewhere awkward.
//! 3. The one place this port deliberately disagrees with its upstream, and
//!    how much difference it makes under Singapore conditions specifically.
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

use changi::puff::climatology::{SingaporeWind, MEAN_WIND_SPEED_M_PER_S};
use changi::puff::concentration::{gaussian_puff_concentration, METHANE_PPM_PER_KG_PER_M3};
use changi::puff::simulate::{
    constant_wind, simulate_sensor_mode, EmissionPolicy, Receptor, RunConfig, Source,
};
use changi::puff::stability::StabilitySet;
use changi::puff::wind::wind_speed;

use uom::si::f64::{Length, Mass, MassRate, Time};
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
    println!("## 1. One leak, four sensors, the northeast monsoon\n");

    // The northeast monsoon: December to early March, Singapore's windiest
    // season. The direction is given the way a met station reports it -- the
    // direction the wind blows FROM, clockwise from north -- so 030 degrees is
    // a northeasterly and the plume travels toward the SOUTH-WEST.
    let monsoon = SingaporeWind::NortheastMonsoon;
    let wind = monsoon.components();
    println!(
        "wind: {} ({}), {:.1} m/s from {:.0} deg",
        monsoon.label(),
        monsoon.season(),
        monsoon.speed().get::<meter_per_second>(),
        monsoon.direction().get::<uom::si::angle::degree>()
    );
    println!(
        "  ->  u = {:+.3} m/s (east), v = {:+.3} m/s (north)",
        wind.u.get::<meter_per_second>(),
        wind.v.get::<meter_per_second>()
    );

    let source = Source {
        x: m(0.0),
        y: m(0.0),
        height: m(2.5),
    };

    // North, east, south and west of the source. A northeasterly puts the
    // plume over the south and west sensors; north and east are upwind.
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
        "\nOnly S and W register: a northeasterly carries the plume south-west,\n\
         and N and E are upwind. The puff model has no upwind diffusion at all —\n\
         each puff only ever moves with the wind that was blowing when it was\n\
         emitted.\n\n\
         S dominates W by more than three orders of magnitude, and that is\n\
         geometry, not physics: a wind FROM 030 deg blows TOWARD 210 deg, which\n\
         is south-south-west — much nearer due south than due west. The sensor\n\
         nearest the plume axis wins, steeply, because the crosswind profile is\n\
         Gaussian. Move the monsoon a few degrees and that ranking changes,\n\
         which is worth remembering before reading any single sensor as `the`\n\
         downwind concentration."
    );
}

/// The same instantaneous release seen under two very different atmospheres.
fn part_2_what_stability_does() {
    println!("\n## 2. What the stability class does, across the monsoons\n");

    // One puff's worth of mass, 200 m downwind, measured at head height.
    let puff_mass = Mass::new::<kilogram>(3.5 / 3600.0 * 10.0);
    let travel = m(200.0);
    let receptor = (m(200.0), m(0.0), m(2.0));

    let report = |label: &str, set: StabilitySet, speed: f64| {
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
        let classes = match set {
            StabilitySet::One(c) => c.letter().to_string(),
            StabilitySet::Two(a, b) => format!("{}/{}", a.letter(), b.letter()),
        };
        println!(
            "{label:>26}  {speed:>5.1}  {classes:>6}  {density:>13.4e}  {:>13.4e}",
            density * METHANE_PPM_PER_KG_PER_M3
        );
    };

    println!(
        "{:>26}  {:>5}  {:>6}  {:>13}  {:>13}",
        "condition", "m/s", "class", "kg/m^3", "ppm (methane)"
    );
    for c in SingaporeWind::ALL {
        let speed = c.speed().get::<meter_per_second>();
        report(&format!("{} (14:00)", c.label()), c.stability(14), speed);
        report(&format!("{} (02:00)", c.label()), c.stability(2), speed);
    }

    println!(
        "\nRead the inter-monsoon rows together: SAME 1 m/s wind, and the night\n\
         reading is far above the afternoon one. It is not the wind that orders\n\
         this table — it is convection. A calm sunny afternoon is class A, the\n\
         most unstable there is, and vigorous vertical mixing dilutes the plume\n\
         faster than any amount of horizontal wind. A calm clear night is class\n\
         E/F, a stable layer that suppresses vertical motion and keeps the plume\n\
         tight and travelling.\n\n\
         That is why the stability class, not the wind speed, is the single\n\
         largest control on how fast a plume dilutes — and why a dispersion\n\
         estimate that reports only wind speed is not telling you much."
    );

    println!(
        "\nTwo things specific to Singapore are visible here.\n\n\
         FIRST, the two-letter entries. The Pasquill table is a RANGE, not a\n\
         point, and upstream preserves that: two letters means the condition is\n\
         genuinely ambiguous between adjacent classes. At NIGHT, every row below\n\
         a monsoon surge is ambiguous — so in Singapore that is not an edge\n\
         case, it is the normal state. Part 3 shows what upstream does with it.\n\n\
         SECOND, the mean wind sits ON a band edge. The Pasquill table switches\n\
         at {MEAN_WIND_SPEED_M_PER_S:.0} m/s, and that is exactly Singapore's\n\
         mean surface wind. A daytime condition a hair below reads A/B; at the\n\
         mean exactly it reads a single B. A measurement uncertainty of a few cm/s straddles that, so\n\
         the daytime class here is decided by noise as much as by weather."
    );

    println!(
        "\nWorth stating plainly: light winds are where this model is WEAKEST.\n\
         The Pasquill-Gifford fits come from tracer campaigns in steadier flow,\n\
         and at 1-2 m/s the wind direction wanders enough over a puff's lifetime\n\
         that holding it fixed from the moment of emission — which is exactly\n\
         what this port does, following upstream — is a real approximation and\n\
         not a small one."
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
    println!(
        "{:>26}  {:>5}  {:>6}  {:>11}  {:>11}  {:>6}",
        "condition", "m/s", "class", "ours", "upstream", "ratio"
    );
    for c in SingaporeWind::ALL {
        for hour in [14u32, 2] {
            let set = c.stability(hour);
            let wind = c.components();

            // Put the receptor 50 m DOWNWIND along this condition's own plume
            // axis. A fixed receptor would sit upwind of half these winds and
            // report a ratio of two numerical zeros, which says nothing.
            let (wu, wv) = (
                wind.u.get::<meter_per_second>(),
                wind.v.get::<meter_per_second>(),
            );
            let norm = wu.hypot(wv);
            let receptors = vec![Receptor {
                x: m(50.0 * wu / norm),
                y: m(50.0 * wv / norm),
                z: m(2.0),
            }];
            let run = |policy| {
                let config = RunConfig {
                    sim_dt: s(10.0),
                    puff_dt: s(10.0),
                    output_dt: s(120.0),
                    duration: s(600.0),
                    puff_duration: s(1200.0),
                    start_hour: hour,
                    emission_policy: policy,
                };
                simulate_sensor_mode(
                    &[source],
                    leak(3.5),
                    &constant_wind(wind.u, wind.v, 61),
                    &receptors,
                    &config,
                )
                .concentrations
                .iter()
                .map(|row| row[0])
                .fold(0.0_f64, f64::max)
            };
            let ours = run(EmissionPolicy::OnePuffPerEmission);
            let upstream = run(EmissionPolicy::UpstreamRecycleStabilityClasses);
            let classes = match set {
                StabilitySet::One(k) => k.letter().to_string(),
                StabilitySet::Two(a, b) => format!("{}/{}", a.letter(), b.letter()),
            };
            println!(
                "{:>26}  {:>5.1}  {classes:>6}  {ours:>11.4e}  {upstream:>11.4e}  {:>6.2}",
                format!("{} ({:02}:00)", c.label(), hour),
                wind_speed(wind).get::<meter_per_second>(),
                if ours > 0.0 {
                    upstream / ours
                } else {
                    f64::NAN
                }
            );
        }
    }

    println!(
        "\nEvery ratio is 1.00 where the class is unambiguous, and above 1 where\n\
         it is not — the divergence is bounded to the ambiguous regimes, and\n\
         both halves of that are pinned by tests.\n\n\
         Note which rows those are. In Singapore, EVERY night-time row except a\n\
         monsoon surge is affected. Upstream's defect is not a corner case here;\n\
         it is the normal operating condition.\n\n\
         And the ratio is NOT a clean 2.0 — it ranges from about 1.04 to about\n\
         4.0 across these eight rows, on BOTH sides of 2. The emitted MASS is\n\
         doubled exactly, but the two duplicated puffs are given DIFFERENT\n\
         stability classes, so the second disperses differently from the first\n\
         and contributes a different concentration. Where the two classes are\n\
         far apart the extra puff can dominate the reading; where they are close\n\
         it barely moves it. Doubling the mass does not double the answer, and\n\
         it does not even bound it.\n\n\
         So the error upstream's recycling introduces is not a clean factor you\n\
         can divide back out of a published result. It depends on how far apart\n\
         the two classes disperse at the receptor in question, which depends on\n\
         the geometry. If you are reproducing a published `puff` result, ask for\n\
         `UpstreamRecycleStabilityClasses` rather than trying to correct for it.\n\
         For anything where the emitted mass has to match the source term —\n\
         which is every radiological application — leave the default alone."
    );
}
