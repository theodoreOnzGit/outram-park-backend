// SPDX-License-Identifier: GPL-3.0

//! Property tests for [`changi::activity`].
//!
//! # These are consistency checks, NOT verification
//!
//! `changi::activity` is not a port. It has no upstream, so there is nothing to
//! compare it against and **no code-to-code verification is possible**.
//! Manufacturing one — inventing an "expected" number and asserting the code
//! reproduces it — would be worse than having none, because it would read like
//! evidence while being a transcription of this code's own output.
//!
//! What these tests can do is pin properties that must hold for *any* correct
//! implementation of the model, and would break under most plausible mistakes:
//! linearity, conservation across a re-segmentation, exactness of the decay
//! weighting, and the sign of every monotone relationship.
//!
//! **One test is stronger than the rest**: `a_stable_nuclide_through_the_whole_
//! chain_matches_simulate_sensor_mode` drives a complete source term through
//! `survey` and compares it against `puff::simulate::simulate_sensor_mode`,
//! which *is* checked against upstream R. It is the only thread connecting this
//! module to a verified path, and it agrees to better than 1e-12 relative.

use changi::activity::chi_over_q::{dilution_factors, DilutionFactors, StabilitySource};
use changi::activity::deposition::{DepositionGroup, DryDepositionVelocity};
use changi::activity::source::{NuclideRelease, ReleaseWindow, SourceTerm};
use changi::activity::survey::{survey, DepositionVelocities};
use changi::puff::concentration::METHANE_PPM_PER_KG_PER_M3;
use changi::puff::simulate::{
    constant_wind, simulate_sensor_mode, EmissionPolicy, Receptor, RunConfig, Source,
};
use changi::puff::stability::StabilityClass;
use uom::si::f64::{Frequency, Length, MassRate, Radioactivity, Time, Velocity};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::radioactivity::becquerel;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

const STEP_S: f64 = 10.0;
const WIND_M_PER_S: f64 = 4.0;

fn source() -> Source {
    Source {
        x: Length::new::<meter>(0.0),
        y: Length::new::<meter>(0.0),
        height: Length::new::<meter>(20.0),
    }
}

fn receptors(heights: f64) -> Vec<Receptor> {
    [200.0, 500.0, 1000.0]
        .iter()
        .map(|d| Receptor {
            x: Length::new::<meter>(*d),
            y: Length::new::<meter>(0.0),
            z: Length::new::<meter>(heights),
        })
        .collect()
}

fn config(duration_s: f64) -> RunConfig {
    RunConfig {
        sim_dt: Time::new::<second>(STEP_S),
        puff_dt: Time::new::<second>(STEP_S),
        output_dt: Time::new::<second>(STEP_S),
        duration: Time::new::<second>(duration_s),
        puff_duration: Time::new::<second>(1200.0),
        start_hour: 12,
        emission_policy: EmissionPolicy::OnePuffPerEmission,
    }
}

fn wind(n: usize) -> Vec<changi::puff::wind::WindComponents> {
    constant_wind(
        Velocity::new::<meter_per_second>(WIND_M_PER_S),
        Velocity::new::<meter_per_second>(0.0),
        n,
    )
}

fn window(a: f64, b: f64) -> ReleaseWindow {
    ReleaseWindow::new(Time::new::<second>(a), Time::new::<second>(b))
}

fn bq(x: f64) -> Radioactivity {
    Radioactivity::new::<becquerel>(x)
}

fn factors(cfg: &RunConfig, bounds: &[Time], z: f64) -> DilutionFactors {
    let n = (cfg.duration.get::<second>() / STEP_S) as usize + 1;
    dilution_factors(
        &[source()],
        bounds,
        &wind(n + 1),
        &receptors(z),
        cfg,
        StabilitySource::Fixed(StabilityClass::D),
    )
}

/// Scaling per nuclide is exact, so doubling the release must exactly double
/// both the air concentration and the deposition. Any accidental
/// activity-dependence in the dispersion — a clamp, a floor, a normalisation
/// by the wrong thing — breaks this.
#[test]
fn everything_downstream_is_exactly_linear_in_the_released_activity() {
    let cfg = config(600.0);
    let bounds = [Time::new::<second>(0.0), cfg.duration];
    let air = factors(&cfg, &bounds, 1.5);
    let ground = factors(&cfg, &bounds, 0.0);
    let v = DepositionVelocities::order_of_magnitude_placeholder();

    let build = |q: f64| {
        SourceTerm::new(
            vec![window(0.0, 600.0)],
            vec![NuclideRelease {
                label: "I-131".to_string(),
                decay_constant: Frequency::new::<hertz>(core::f64::consts::LN_2 / 693_000.0),
                deposition_group: DepositionGroup::Halogen,
                released: vec![bq(q)],
            }],
        )
    };

    let one = survey(&build(1.0e12), &air, &ground, &v);
    let two = survey(&build(2.0e12), &air, &ground, &v);

    for r in 0..one.n_receptors() {
        let a1 = one.at(r)[0].air.becquerel_seconds_per_cubic_meter();
        let a2 = two.at(r)[0].air.becquerel_seconds_per_cubic_meter();
        let g1 = one.at(r)[0].ground.becquerel_per_square_meter();
        let g2 = two.at(r)[0].ground.becquerel_per_square_meter();
        assert!(a1 > 0.0, "receptor {r} should see something");
        assert!((a2 - 2.0 * a1).abs() <= 8.0 * f64::EPSILON * a2.abs());
        assert!((g2 - 2.0 * g1).abs() <= 8.0 * f64::EPSILON * g2.abs());
    }
}

/// **The one that matters.** A complete source term through `survey` must
/// reproduce `simulate_sensor_mode`, which is itself checked against upstream
/// R. This is the only tie the `activity` module has to a verified path.
///
/// The comparison is exact, for the reasons spelled out on
/// `chi_over_q::tests::unit_response_agrees_with_the_ported_sum`: `output_dt ==
/// sim_dt` gives one step per output interval, and the sensor run is made one
/// step longer than the dilution run because upstream drops its final
/// timestamp. Matching the windows is what removes the need for a tolerance.
#[test]
fn a_stable_nuclide_through_the_whole_chain_matches_simulate_sensor_mode() {
    let n_shared = 60_usize;
    let mut sensor_cfg = config(n_shared as f64 * STEP_S);
    sensor_cfg.output_dt = sensor_cfg.sim_dt;
    let dilution_cfg = config((n_shared - 1) as f64 * STEP_S);

    let w = wind(n_shared + 1);
    let rs = receptors(1.5);
    let bounds = [Time::new::<second>(0.0), dilution_cfg.duration];

    // One kilogram per puff through the ported path.
    let rate = MassRate::new::<kilogram_per_second>(1.0 / sensor_cfg.puff_dt.get::<second>());
    let series = simulate_sensor_mode(&[source()], rate, &w, &rs, &sensor_cfg);
    assert_eq!(series.concentrations.len(), n_shared);

    let air = dilution_factors(
        &[source()],
        &bounds,
        &w,
        &rs,
        &dilution_cfg,
        // MUST match how `simulate_sensor_mode` classifies, which is from the
        // wind speed and the hour. Forcing a class here would compare two
        // different atmospheres and the disagreement would look like a bug in
        // the chain -- it was 28 % when this test was first written that way.
        StabilitySource::FromWind,
    );

    // A stable nuclide releasing `q_total` Bq uniformly over the same window
    // that the ported run released `n_shared` kg over.
    let q_total = 7.3e14_f64;
    let term = SourceTerm::new(
        vec![window(0.0, dilution_cfg.duration.get::<second>())],
        vec![NuclideRelease {
            label: "stable".to_string(),
            decay_constant: Frequency::new::<hertz>(0.0),
            deposition_group: DepositionGroup::NobleGas,
            released: vec![bq(q_total)],
        }],
    );
    let v = DepositionVelocities::order_of_magnitude_placeholder();
    let result = survey(&term, &air, &air, &v);

    for r in 0..rs.len() {
        // From the ported path: ppm -> kg/m^3 -> Bq/m^3 (q_total spread over
        // n_shared kg), integrated over the shared steps.
        let mut from_port = 0.0;
        for row in &series.concentrations {
            from_port += row[r] / METHANE_PPM_PER_KG_PER_M3 * STEP_S;
        }
        from_port *= q_total / n_shared as f64;

        let from_chain = result.at(r)[0].air.becquerel_seconds_per_cubic_meter();

        assert!(from_port > 0.0, "receptor {r} should see something");
        let rel = (from_chain - from_port).abs() / from_port;
        assert!(
            rel < 1e-12,
            "receptor {r}: chain {from_chain:e} vs ported {from_port:e}, rel {rel:e}"
        );
    }
}

/// Splitting a release across N contiguous windows, with the activity split in
/// the same proportion as the time, must reproduce the single-window answer.
/// Segment attribution is bookkeeping; it must not move any physics.
#[test]
fn splitting_the_release_across_windows_conserves_the_total() {
    let cfg = config(600.0);
    let v = DepositionVelocities::order_of_magnitude_placeholder();
    let lambda = Frequency::new::<hertz>(core::f64::consts::LN_2 / 3600.0);
    let q = 1.0e14_f64;

    let one_bounds = [Time::new::<second>(0.0), Time::new::<second>(600.0)];
    let one = survey(
        &SourceTerm::new(
            vec![window(0.0, 600.0)],
            vec![NuclideRelease {
                label: "Kr-88".to_string(),
                decay_constant: lambda,
                deposition_group: DepositionGroup::NobleGas,
                released: vec![bq(q)],
            }],
        ),
        &factors(&cfg, &one_bounds, 1.5),
        &factors(&cfg, &one_bounds, 0.0),
        &v,
    );

    let three_bounds = [
        Time::new::<second>(0.0),
        Time::new::<second>(200.0),
        Time::new::<second>(400.0),
        Time::new::<second>(600.0),
    ];
    let three = survey(
        &SourceTerm::new(
            vec![window(0.0, 200.0), window(200.0, 400.0), window(400.0, 600.0)],
            vec![NuclideRelease {
                label: "Kr-88".to_string(),
                decay_constant: lambda,
                deposition_group: DepositionGroup::NobleGas,
                released: vec![bq(q / 3.0), bq(q / 3.0), bq(q / 3.0)],
            }],
        ),
        &factors(&cfg, &three_bounds, 1.5),
        &factors(&cfg, &three_bounds, 0.0),
        &v,
    );

    for r in 0..one.n_receptors() {
        let a = one.at(r)[0].air.becquerel_seconds_per_cubic_meter();
        let b = three.at(r)[0].air.becquerel_seconds_per_cubic_meter();
        assert!(a > 0.0);
        let rel = (a - b).abs() / a;
        assert!(
            rel < 0.02,
            "receptor {r}: one window {a:e} vs three {b:e}, rel {rel:e}"
        );
    }
}

/// A noble gas must deposit exactly zero through the entire chain, not merely
/// a small number — the velocity is exactly zero, not a small placeholder.
#[test]
fn a_noble_gas_deposits_exactly_zero_through_the_whole_chain() {
    let cfg = config(600.0);
    let bounds = [Time::new::<second>(0.0), cfg.duration];
    let result = survey(
        &SourceTerm::new(
            vec![window(0.0, 600.0)],
            vec![
                NuclideRelease {
                    label: "Kr-85".to_string(),
                    decay_constant: Frequency::new::<hertz>(0.0),
                    deposition_group: DepositionGroup::NobleGas,
                    released: vec![bq(1.0e15)],
                },
                NuclideRelease {
                    label: "Cs-137".to_string(),
                    decay_constant: Frequency::new::<hertz>(0.0),
                    deposition_group: DepositionGroup::Aerosol,
                    released: vec![bq(1.0e15)],
                },
            ],
        ),
        &factors(&cfg, &bounds, 1.5),
        &factors(&cfg, &bounds, 0.0),
        &DepositionVelocities::order_of_magnitude_placeholder(),
    );

    for r in 0..result.n_receptors() {
        assert_eq!(
            result.at(r)[0].ground.becquerel_per_square_meter(),
            0.0,
            "a noble gas must deposit exactly nothing"
        );
        assert!(
            result.at(r)[1].ground.becquerel_per_square_meter() > 0.0,
            "the aerosol released identically must deposit something"
        );
        // Airborne, they are identical: deposition is diagnostic, not depleting.
        let air_gas = result.at(r)[0].air.becquerel_seconds_per_cubic_meter();
        let air_aerosol = result.at(r)[1].air.becquerel_seconds_per_cubic_meter();
        assert_eq!(
            air_gas, air_aerosol,
            "deposition does not deplete the plume in this model, so two nuclides \
             released identically must have identical airborne concentrations"
        );
    }
}

/// Decay in transit can only reduce, never increase, and a shorter half-life
/// must never leave more behind than a longer one.
#[test]
fn decay_in_transit_is_monotone_in_the_half_life() {
    let cfg = config(1200.0);
    let bounds = [Time::new::<second>(0.0), cfg.duration];
    let air = factors(&cfg, &bounds, 1.5);
    let ground = factors(&cfg, &bounds, 0.0);
    let v = DepositionVelocities::order_of_magnitude_placeholder();

    // Stable, 10 minutes, 189 s (Kr-89), 30 s.
    let half_lives = [f64::INFINITY, 600.0, 189.0, 30.0];
    let term = SourceTerm::new(
        vec![window(0.0, 1200.0)],
        half_lives
            .iter()
            .map(|t| NuclideRelease {
                label: format!("t{t}"),
                decay_constant: Frequency::new::<hertz>(if t.is_infinite() {
                    0.0
                } else {
                    core::f64::consts::LN_2 / t
                }),
                deposition_group: DepositionGroup::Aerosol,
                released: vec![bq(1.0e15)],
            })
            .collect(),
    );
    let result = survey(&term, &air, &ground, &v);

    for r in 0..result.n_receptors() {
        let row = result.at(r);
        for k in 1..row.len() {
            let longer = row[k - 1].air.becquerel_seconds_per_cubic_meter();
            let shorter = row[k].air.becquerel_seconds_per_cubic_meter();
            assert!(
                shorter <= longer,
                "receptor {r}: half-life {} gave {shorter:e}, which exceeds the \
                 longer-lived {}'s {longer:e}",
                half_lives[k],
                half_lives[k - 1]
            );
        }
        // And decay must actually bite at range, not be a no-op.
        let stable = row[0].air.becquerel_seconds_per_cubic_meter();
        let fastest = row[row.len() - 1].air.becquerel_seconds_per_cubic_meter();
        assert!(
            fastest < 0.5 * stable,
            "a 30 s half-life should lose most of its activity before arriving: \
             {fastest:e} against a stable {stable:e}"
        );
    }
}

/// Deposition is exactly proportional to the deposition velocity, which is what
/// lets a reader rescale a reported figure when a cited velocity finally
/// replaces the placeholder.
#[test]
fn deposition_scales_exactly_with_the_deposition_velocity() {
    let cfg = config(600.0);
    let bounds = [Time::new::<second>(0.0), cfg.duration];
    let air = factors(&cfg, &bounds, 1.5);
    let ground = factors(&cfg, &bounds, 0.0);

    let term = SourceTerm::new(
        vec![window(0.0, 600.0)],
        vec![NuclideRelease {
            label: "Cs-137".to_string(),
            decay_constant: Frequency::new::<hertz>(0.0),
            deposition_group: DepositionGroup::Aerosol,
            released: vec![bq(1.0e15)],
        }],
    );

    let mut slow = DepositionVelocities::order_of_magnitude_placeholder();
    slow.aerosol = DryDepositionVelocity::new(Velocity::new::<meter_per_second>(1.0e-3));
    let mut fast = slow;
    fast.aerosol = DryDepositionVelocity::new(Velocity::new::<meter_per_second>(5.0e-3));

    let a = survey(&term, &air, &ground, &slow);
    let b = survey(&term, &air, &ground, &fast);

    for r in 0..a.n_receptors() {
        let d1 = a.at(r)[0].ground.becquerel_per_square_meter();
        let d2 = b.at(r)[0].ground.becquerel_per_square_meter();
        assert!(d1 > 0.0);
        assert!((d2 - 5.0 * d1).abs() <= 8.0 * f64::EPSILON * d2.abs());
        // Airborne is untouched: deposition does not deplete.
        assert_eq!(
            a.at(r)[0].air.becquerel_seconds_per_cubic_meter(),
            b.at(r)[0].air.becquerel_seconds_per_cubic_meter()
        );
    }
}

/// Ground level and breathing height give **different** answers, and which is
/// larger depends on `sigma_z` against the release height `H`. This is the
/// reason `survey` takes two factor sets rather than reusing one.
///
/// The puff's vertical term is
/// `exp(-(z-H)^2 / 2 sigma_z^2) + exp(-(z+H)^2 / 2 sigma_z^2)`, which is
/// symmetric under `z -> -z`. So `z = 0` is a **stationary** point, and for
/// `sigma_z < H` — an elevated plume that has not yet spread down to the
/// surface — it is a local *minimum*: concentration *increases* with height
/// near the ground, and breathing height reads **higher** than ground level.
/// Far downwind, once `sigma_z` exceeds `H`, the profile flattens and the two
/// converge.
///
/// So there is no blanket "breathing height under-predicts" or "over-predicts".
/// Substituting one height for the other is wrong in a direction that changes
/// with distance, which is worse than a consistent bias and is why nothing here
/// lets a caller do it by accident.
#[test]
fn ground_level_and_breathing_height_differ_by_a_sign_that_depends_on_distance() {
    let cfg = config(1800.0);
    let bounds = [Time::new::<second>(0.0), cfg.duration];
    let air = factors(&cfg, &bounds, 1.5);
    let ground = factors(&cfg, &bounds, 0.0);
    let v = DepositionVelocities::order_of_magnitude_placeholder();

    let term = SourceTerm::new(
        vec![window(0.0, 1800.0)],
        vec![NuclideRelease {
            label: "Cs-137".to_string(),
            decay_constant: Frequency::new::<hertz>(0.0),
            deposition_group: DepositionGroup::Aerosol,
            released: vec![bq(1.0e15)],
        }],
    );

    let at_ground = survey(&term, &air, &ground, &v);
    let at_breathing = survey(&term, &air, &air, &v);

    // Receptors are 200 m, 500 m, 1000 m with H = 20 m. Under class D,
    // sigma_z is roughly 8.5 m at 200 m and roughly 31 m at 1000 m, so the
    // relationship reverses across the set.
    let ratios: Vec<f64> = (0..at_ground.n_receptors())
        .map(|r| {
            let g = at_ground.at(r)[0].ground.becquerel_per_square_meter();
            let b = at_breathing.at(r)[0].ground.becquerel_per_square_meter();
            assert!(g > 0.0 && b > 0.0, "receptor {r} should see something");
            b / g
        })
        .collect();

    assert!(
        ratios[0] > 1.0,
        "at 200 m the plume is still aloft (sigma_z < H), so breathing height must \
         read higher than ground level; got ratio {:e}",
        ratios[0]
    );
    assert!(
        ratios[ratios.len() - 1] < ratios[0],
        "the two heights must converge with distance as sigma_z grows past H: \
         ratios {ratios:?}"
    );
    // And they are never the same, which is the whole point of the two sets.
    for (r, ratio) in ratios.iter().enumerate() {
        assert!(
            (ratio - 1.0).abs() > 1e-12,
            "receptor {r}: the two heights gave the same answer, ratio {ratio:e}"
        );
    }
}
