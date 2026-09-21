// SPDX-License-Identifier: GPL-3.0
//
// Code-to-code verification of `changi::puff` against the upstream
// Hammerling Research Group `puff` R package (MIT, commit 5213d58).
//
// Uses only changi + approx, and reads its fixture through `include_str!`, so
// there is no filesystem access at run time and no target gate is needed.

//! # V&V: Gaussian puff dispersion, code-to-code
//!
//! ## Methodology
//!
//! Every reference value in this file is produced by **executing the upstream
//! R itself** — not by re-deriving the dispersion physics and not by
//! transcribing numbers from the paper. `dev/gen_puff_reference.R` sources
//! `R/helpers.R`, `R/simulate_sensor_mode.R` and `R/simulate_grid_mode.R` from
//! the gitignored clone at `upstream_source/puff` (commit `5213d58`), sweeps
//! each function over a grid chosen to straddle **every branch, band edge and
//! bin cutoff**, and writes `tests/data/puff_reference.csv` at `%.17g`
//! round-trip precision.
//!
//! Upstream is double-precision R and this port is `f64`, so — unlike the
//! FLEXPART comparison in `flexpart_code_to_code.rs`, where upstream's
//! single-precision storage sets a `~1e-7` floor — there is no precision gap to
//! account for. Residual disagreement is libm differences in `exp`, `ln`,
//! `tan` and `powf` plus summation order, which is why the tolerances below sit
//! near machine epsilon rather than at an engineering bound.
//!
//! ## Branch coverage
//!
//! * **`is_day`** — all 24 hours, so both inclusive bounds are on the grid.
//! * **`get_stab_class`** — 21 wind speeds against all 24 hours, with the
//!   speeds placed on both sides of and exactly on every band edge (2, 3, 5,
//!   6 m/s), plus the missing-wind path. All three outputs are compared: the
//!   number of classes returned, and each class.
//! * **`compute_sigma_vals`** — all six classes over 35 distances placed on and
//!   around every cutoff in every class's table, into the saturated regime, and
//!   at non-positive distances where upstream yields `NA`.
//! * **`wind_vector_convert`** — 8 speeds × 31 directions including every
//!   cardinal and the `0°/360°` wrap.
//! * **`gpuff`** — 6 classes × 4 masses × 7 travel distances × 4 release
//!   heights × 10 receptor positions, the receptors including the ground, the
//!   release height, well above it, and the puff's own centre.
//! * **Both simulate modes** — four scenarios spanning day and night, ambiguous
//!   and unambiguous stability, `puff_dt == sim_dt` and `puff_dt > sim_dt`, and
//!   a puff lifetime short enough to force expiry mid-run.
//!
//! ## Results (2026-09-21, upstream `5213d58`, 10 170 cases, all passing)
//!
//! Measured maxima are printed by each test with `--nocapture` and recorded in
//! `docs/puff-code-to-code.md`.
//!
//! ## Upstream defects reproduced
//!
//! Four, each pinned by a named test here and described in
//! `docs/puff-code-to-code.md`:
//!
//! 1. **`gpuff`'s `U` parameter is never read.** It is declared, documented as
//!    "Wind speed in m/s", and absent from the body. Demonstrated by sweeping
//!    it over seven values including `0`, `1e6` and a negative, and showing the
//!    answer never moves — see `gpuff_wind_speed_argument_is_inert`.
//! 2. **An ambiguous stability class silently loses its second member.**
//!    `gpuff` indexes the `2 × n` matrix from `compute_sigma_vals` with
//!    `sigma.vec[1]`/`sigma.vec[2]`; R is column-major, so those are `sigma_y`
//!    and `sigma_z` of the *first* class only.
//! 3. **An ambiguous stability class doubles the emitted mass.** The simulate
//!    drivers build their puff record with `data.frame(...)`, and R recycles
//!    the length-1 columns against the length-2 `stab_class`, producing **two
//!    puff rows each carrying the full `q_per_puff`**. Six of the ten
//!    (wind speed × day/night) regimes are ambiguous, so this is the common
//!    case, not an edge case.
//! 4. **Grid mode never writes its last output row.** Its index arithmetic
//!    reaches at most `floor(n_steps / (output_dt / sim_dt))` of
//!    `length(seq(start, end, by = output_dt))` rows.
//!
//! Defect 3 is the only one where the port's *default* differs: mass
//! conservation is not a style preference, so
//! [`EmissionPolicy::OnePuffPerEmission`] is the default and
//! `UpstreamRecycleStabilityClasses` must be asked for by name. The fixture is
//! replayed with the bug-compatible policy, because the fixture's job is to
//! establish what upstream computes.
//!
//! **This is verification, not validation.** It establishes that the Rust port
//! computes what the upstream R computes. It says nothing about whether either
//! reproduces measured atmospheric dispersion — that needs a tracer-release
//! benchmark and is not claimed here.

use changi::puff::concentration::gaussian_puff_methane_ppm;
use changi::puff::dispersion::pasquill_gifford_sigmas;
use changi::puff::simulate::{
    constant_wind, simulate_grid_mode, simulate_sensor_mode, EmissionPolicy, Receptor, RunConfig,
    Source,
};
use changi::puff::stability::{is_day, stability_class, StabilityClass, StabilitySet};
use changi::puff::wind::{interpolate_wind, wind_vector_convert, WindComponents};

use uom::si::angle::degree;
use uom::si::f64::{Angle, Length, Mass, MassRate, Time, Velocity};
use uom::si::length::{kilometer, meter};
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

const FIXTURE: &str = include_str!("data/puff_reference.csv");

struct Case {
    function: String,
    args: Vec<f64>,
    expected: f64,
}

fn cases() -> Vec<Case> {
    FIXTURE
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty() && !l.starts_with("function,"))
        .map(|l| {
            let mut f = l.split(',');
            let function = f.next().expect("function column").trim().to_string();
            let arg_field = f.next().expect("args column");
            let expected = f.next().expect("expected column");
            let args = arg_field
                .split(';')
                .map(|a| a.trim().parse::<f64>().expect("f64 arg"))
                .collect();
            Case {
                function,
                args,
                expected: expected.trim().parse::<f64>().expect("f64 expected"),
            }
        })
        .collect()
}

/// Relative deviation, falling back to absolute where the reference is zero.
fn deviation(got: f64, want: f64) -> f64 {
    if want == 0.0 {
        got.abs()
    } else {
        (got - want).abs() / want.abs()
    }
}

/// Replay every fixture row in `name` through `eval` and assert agreement.
///
/// Returns the observed maximum deviation so the test can print the real
/// number, per the workspace V&V rule that results are recorded and not just
/// methodology. A `NaN` on both sides counts as agreement: upstream's `NA` and
/// the port's `None` both mean "no value here".
fn check_group(name: &str, tol: f64, eval: impl Fn(&[f64]) -> f64) -> f64 {
    let all = cases();
    let group: Vec<&Case> = all.iter().filter(|c| c.function == name).collect();
    assert!(
        !group.is_empty(),
        "no fixture rows for `{name}` — regenerate with dev/gen_puff_reference.R"
    );
    let mut worst = 0.0_f64;
    let mut worst_args: Vec<f64> = Vec::new();
    for c in &group {
        let got = eval(&c.args);
        if got.is_nan() && c.expected.is_nan() {
            continue;
        }
        let dev = deviation(got, c.expected);
        assert!(
            dev <= tol,
            "{name}{:?}: port {got:e} vs upstream {:e} (rel dev {dev:e} > {tol:e})",
            c.args,
            c.expected
        );
        if dev > worst {
            worst = dev;
            worst_args = c.args.clone();
        }
    }
    println!(
        "{name}: {} cases, max_rel_dev = {worst:e} (tol {tol:e}) at args {worst_args:?}",
        group.len()
    );
    worst
}

fn class_from_index(i: f64) -> StabilityClass {
    match i as u32 {
        1 => StabilityClass::A,
        2 => StabilityClass::B,
        3 => StabilityClass::C,
        4 => StabilityClass::D,
        5 => StabilityClass::E,
        6 => StabilityClass::F,
        other => panic!("fixture carries an unknown stability index {other}"),
    }
}

fn class_index(c: StabilityClass) -> f64 {
    match c {
        StabilityClass::A => 1.0,
        StabilityClass::B => 2.0,
        StabilityClass::C => 3.0,
        StabilityClass::D => 4.0,
        StabilityClass::E => 5.0,
        StabilityClass::F => 6.0,
    }
}

/// Decode the fixture's wind-speed column, where `-1` flags upstream's
/// missing-value path.
fn wind_arg(v: f64) -> Option<Velocity> {
    if v < 0.0 {
        None
    } else {
        Some(Velocity::new::<meter_per_second>(v))
    }
}

// ── helpers.R ────────────────────────────────────────────────────────────────

/// Day/night classification, compared as a decision rather than to a tolerance.
#[test]
fn is_day_matches_upstream() {
    let all = cases();
    let rows: Vec<&Case> = all.iter().filter(|c| c.function == "is_day").collect();
    assert_eq!(rows.len(), 24, "every hour must be covered");
    for c in &rows {
        let got = f64::from(u8::from(is_day(c.args[0] as u32)));
        assert_eq!(got, c.expected, "hour {}", c.args[0]);
    }
    println!("is_day: 24 hours agree exactly");
}

/// The number of stability classes a condition admits.
///
/// Checked first and separately: if the port and upstream disagree about
/// *how many* classes a condition has, every downstream comparison — and the
/// whole emission-policy divergence — is built on sand.
#[test]
fn stability_class_count_matches_upstream() {
    let all = cases();
    let rows: Vec<&Case> = all
        .iter()
        .filter(|c| c.function == "get_stab_class.count")
        .collect();
    assert!(!rows.is_empty());
    let mut ambiguous = 0usize;
    for c in &rows {
        let set = stability_class(wind_arg(c.args[0]), c.args[1] as u32);
        assert_eq!(
            set.len() as f64,
            c.expected,
            "U = {} m/s, hour = {}",
            c.args[0],
            c.args[1]
        );
        if set.len() == 2 {
            ambiguous += 1;
        }
    }
    println!(
        "get_stab_class: {} conditions, {ambiguous} ambiguous ({:.0}%)",
        rows.len(),
        100.0 * ambiguous as f64 / rows.len() as f64
    );
    // The fixture's proportion depends on how densely the speed and hour axes
    // are sampled, so the load-bearing claim is pinned against the TABLE's ten
    // regimes rather than against this sweep: see
    // `stability::tests::six_of_ten_regimes_are_ambiguous`. What is asserted
    // here is only that the sweep actually exercised both cases.
    assert!(
        ambiguous > 0 && ambiguous < rows.len(),
        "the sweep must cover both ambiguous and unambiguous conditions"
    );
}

/// The primary (first) stability class, which is the one that actually
/// propagates into `gpuff`.
#[test]
fn stability_primary_class_matches_upstream() {
    let all = cases();
    let rows: Vec<&Case> = all
        .iter()
        .filter(|c| c.function == "get_stab_class.class1")
        .collect();
    assert!(!rows.is_empty());
    for c in &rows {
        let set = stability_class(wind_arg(c.args[0]), c.args[1] as u32);
        assert_eq!(
            class_index(set.primary()),
            c.expected,
            "U = {} m/s, hour = {}",
            c.args[0],
            c.args[1]
        );
    }
    println!("get_stab_class.class1: {} conditions agree", rows.len());
}

/// The secondary class, encoded `0` where the condition is unambiguous.
#[test]
fn stability_secondary_class_matches_upstream() {
    let all = cases();
    let rows: Vec<&Case> = all
        .iter()
        .filter(|c| c.function == "get_stab_class.class2")
        .collect();
    assert!(!rows.is_empty());
    for c in &rows {
        let set = stability_class(wind_arg(c.args[0]), c.args[1] as u32);
        let got = set.secondary().map_or(0.0, class_index);
        assert_eq!(
            got, c.expected,
            "U = {} m/s, hour = {}",
            c.args[0], c.args[1]
        );
    }
    println!("get_stab_class.class2: {} conditions agree", rows.len());
}

/// Crosswind Pasquill–Gifford spread.
///
/// `sigma_y = 465.11628 * x * tan((pi/180)(c - d ln x))`, which is where
/// upstream's 9-digit degrees-to-radians literal `0.017453293` matters: it
/// differs from `PI/180` in the 9th significant figure, so substituting the
/// "better" constant would show up here at `~3e-8` relative.
#[test]
fn sigma_y_matches_upstream() {
    check_group("compute_sigma_vals.sigma_y", 1e-13, |a| {
        pasquill_gifford_sigmas(class_from_index(a[0]), Length::new::<kilometer>(a[1]))
            .map_or(f64::NAN, |s| s.sigma_y.get::<meter>())
    });
}

/// Vertical Pasquill–Gifford spread, including the 5000 m cap and every
/// distance-bin edge.
#[test]
fn sigma_z_matches_upstream() {
    check_group("compute_sigma_vals.sigma_z", 1e-13, |a| {
        pasquill_gifford_sigmas(class_from_index(a[0]), Length::new::<kilometer>(a[1]))
            .map_or(f64::NAN, |s| s.sigma_z.get::<meter>())
    });
}

/// Eastward wind component from met-convention speed and direction.
#[test]
fn wind_u_component_matches_upstream() {
    check_group("wind_vector_convert.u", 1e-12, |a| {
        wind_vector_convert(
            Velocity::new::<meter_per_second>(a[0]),
            Angle::new::<degree>(a[1]),
        )
        .u
        .get::<meter_per_second>()
    });
}

/// Northward wind component.
#[test]
fn wind_v_component_matches_upstream() {
    check_group("wind_vector_convert.v", 1e-12, |a| {
        wind_vector_convert(
            Velocity::new::<meter_per_second>(a[0]),
            Angle::new::<degree>(a[1]),
        )
        .v
        .get::<meter_per_second>()
    });
}

/// Decode an `interpolate_wind_data` row: `[n, sp_0..sp_{n-1}, dir_0..dir_{n-1},
/// duration, step, index]`.
fn decode_interp(a: &[f64]) -> (Vec<WindComponents>, Time, Time, usize) {
    let n = a[0] as usize;
    let obs: Vec<WindComponents> = (0..n)
        .map(|i| {
            wind_vector_convert(
                Velocity::new::<meter_per_second>(a[1 + i]),
                Angle::new::<degree>(a[1 + n + i]),
            )
        })
        .collect();
    (
        obs,
        Time::new::<second>(a[1 + 2 * n]),
        Time::new::<second>(a[2 + 2 * n]),
        a[3 + 2 * n] as usize,
    )
}

/// Resampled eastward wind. Upstream converts to `(u, v)` before
/// interpolating, so the port is fed already-converted observations too.
#[test]
fn interpolated_wind_u_matches_upstream() {
    check_group("interpolate_wind_data.u", 1e-12, |a| {
        let (obs, dur, step, k) = decode_interp(a);
        interpolate_wind(&obs, dur, step)[k]
            .u
            .get::<meter_per_second>()
    });
}

/// Resampled northward wind.
#[test]
fn interpolated_wind_v_matches_upstream() {
    check_group("interpolate_wind_data.v", 1e-12, |a| {
        let (obs, dur, step, k) = decode_interp(a);
        interpolate_wind(&obs, dur, step)[k]
            .v
            .get::<meter_per_second>()
    });
}

/// The Gaussian puff kernel itself.
///
/// Row layout: `[class, Q, travel_distance, H, receptor_x, receptor_y,
/// receptor_z]`, with the puff centre at `(travel_distance, 0)` — the geometry
/// the simulate drivers actually produce, rather than an arbitrary one.
#[test]
fn gpuff_matches_upstream() {
    check_group("gpuff", 1e-12, |a| {
        gaussian_puff_methane_ppm(
            Mass::new::<kilogram>(a[1]),
            class_from_index(a[0]),
            Length::new::<meter>(a[2]),
            Length::new::<meter>(0.0),
            Length::new::<meter>(a[3]),
            (
                Length::new::<meter>(a[4]),
                Length::new::<meter>(a[5]),
                Length::new::<meter>(a[6]),
            ),
            Length::new::<meter>(a[2]),
        )
    });
}

/// **Upstream defect 2** — an ambiguous stability class loses its second
/// member.
///
/// `compute_sigma_vals` returns a `2 × n` matrix; `gpuff` reads `sigma.vec[1]`
/// and `sigma.vec[2]`, and R stores matrices column-major, so those are
/// `sigma_y` and `sigma_z` of the **first** class. The second is computed and
/// discarded.
///
/// The fixture calls upstream's `gpuff` with each ambiguous pair
/// `get_stab_class` can return; this asserts the port's
/// [`StabilitySet::primary`] reproduces it — i.e. that `primary()` is not just
/// a convenient accessor but the verified semantics.
#[test]
fn gpuff_ambiguous_class_uses_only_the_first() {
    let all = cases();
    let rows: Vec<&Case> = all
        .iter()
        .filter(|c| c.function == "gpuff.ambiguous_uses_first")
        .collect();
    assert!(!rows.is_empty(), "no ambiguous-class fixture rows");
    for c in &rows {
        let primary = class_from_index(c.args[0]);
        let secondary = class_from_index(c.args[1]);
        assert_ne!(
            primary, secondary,
            "the fixture pair must actually be ambiguous"
        );
        let set = StabilitySet::Two(primary, secondary);
        let got = gaussian_puff_methane_ppm(
            Mass::new::<kilogram>(1.0),
            set.primary(),
            Length::new::<meter>(100.0),
            Length::new::<meter>(0.0),
            Length::new::<meter>(2.0),
            (
                Length::new::<meter>(120.0),
                Length::new::<meter>(10.0),
                Length::new::<meter>(2.0),
            ),
            Length::new::<meter>(100.0),
        );
        assert!(
            deviation(got, c.expected) <= 1e-12,
            "pair ({}, {}): port {got:e} vs upstream {:e}",
            primary.letter(),
            secondary.letter(),
            c.expected
        );
    }
    println!(
        "gpuff with an ambiguous class: {} pairs, all use the first only",
        rows.len()
    );
}

/// **Upstream defect 1** — `gpuff`'s `U` parameter is never read.
///
/// It is declared in the signature and documented as "Wind speed in m/s", and
/// the body never mentions it. Rather than assert that from inspection, the
/// fixture sweeps `U` over `{0, 1e-9, 1, 5, 100, 1e6, -7}` holding everything
/// else fixed and records upstream's answer each time. This asserts the seven
/// values are **bit-identical**, which is what proves the parameter inert, and
/// that the port — which does not take `U` at all — reproduces them.
///
/// The same shape as TRISO-ATOPS' inert pressure argument recorded in
/// `boon-lay`: a parameter that looks like a physical control, has a physical
/// default, and cannot affect its own answer.
#[test]
fn gpuff_wind_speed_argument_is_inert() {
    let all = cases();
    let rows: Vec<&Case> = all
        .iter()
        .filter(|c| c.function == "gpuff.u_is_inert")
        .collect();
    assert!(rows.len() >= 5, "need a real sweep of U to show inertness");
    let first = rows[0].expected;
    for c in &rows {
        assert_eq!(
            c.expected, first,
            "U = {} moved upstream's answer, so it is NOT inert — re-measure \
             and correct the defect list",
            c.args[0]
        );
    }
    // The port takes no U at all; it must land on the same value.
    let got = gaussian_puff_methane_ppm(
        Mass::new::<kilogram>(1.0),
        StabilityClass::C,
        Length::new::<meter>(100.0),
        Length::new::<meter>(0.0),
        Length::new::<meter>(2.0),
        (
            Length::new::<meter>(150.0),
            Length::new::<meter>(20.0),
            Length::new::<meter>(2.0),
        ),
        Length::new::<meter>(100.0),
    );
    assert!(
        deviation(got, first) <= 1e-12,
        "port {got:e} vs upstream {first:e}"
    );
    println!(
        "gpuff: U swept over {} values including 0, 1e6 and a negative — \
         answer bit-identical throughout",
        rows.len()
    );
}

// ── simulate_sensor_mode.R / simulate_grid_mode.R ────────────────────────────

/// The scenario block both simulate groups carry, in the generator's order.
struct SimScenario {
    hour: u32,
    wind: WindComponents,
    emission_rate: MassRate,
    config: RunConfig,
    source: Source,
}

/// Decode `[id, hour, u, v, rate_kg_per_hr, sim_dt, puff_dt, output_dt,
/// duration, puff_duration, src_x, src_y, src_z, ...]`.
///
/// Replayed with [`EmissionPolicy::UpstreamRecycleStabilityClasses`]: the
/// fixture comes from the stock R, which carries the mass-doubling recycling,
/// and the comparison's job is to reproduce what upstream computes. The
/// *corrected* default is pinned separately by
/// `simulate::tests::the_default_policy_emits_one_puff_per_event`.
fn decode_scenario(a: &[f64]) -> SimScenario {
    let hour = a[1] as u32;
    SimScenario {
        hour,
        wind: WindComponents {
            u: Velocity::new::<meter_per_second>(a[2]),
            v: Velocity::new::<meter_per_second>(a[3]),
        },
        emission_rate: MassRate::new::<kilogram_per_second>(a[4] / 3600.0),
        config: RunConfig {
            sim_dt: Time::new::<second>(a[5]),
            puff_dt: Time::new::<second>(a[6]),
            output_dt: Time::new::<second>(a[7]),
            duration: Time::new::<second>(a[8]),
            puff_duration: Time::new::<second>(a[9]),
            start_hour: hour,
            emission_policy: EmissionPolicy::UpstreamRecycleStabilityClasses,
        },
        source: Source {
            x: Length::new::<meter>(a[10]),
            y: Length::new::<meter>(a[11]),
            height: Length::new::<meter>(a[12]),
        },
    }
}

/// The sensor array the generator uses, in its order.
fn fixture_sensors() -> Vec<Receptor> {
    [
        (20.0, 10.0, 2.0),
        (100.0, 50.0, 2.0),
        (-30.0, 0.0, 2.0),
        (0.0, 0.0, 2.0),
        (250.0, -120.0, 5.0),
    ]
    .iter()
    .map(|(x, y, z)| Receptor {
        x: Length::new::<meter>(*x),
        y: Length::new::<meter>(*y),
        z: Length::new::<meter>(*z),
    })
    .collect()
}

fn wind_for(sc: &SimScenario) -> Vec<WindComponents> {
    let n = (sc.config.duration.get::<second>() / sc.config.sim_dt.get::<second>()) as usize + 2;
    constant_wind(sc.wind.u, sc.wind.v, n)
}

/// Sensor mode end-to-end: every output interval, every sensor, four scenarios.
///
/// This is the group that checks the *composition* — the emission schedule, the
/// puff-expiry rule, the analytic advection, the sum over live puffs, and
/// upstream's `aggregate`-by-`cut` time reduction — rather than any one
/// formula. Each of those is individually simple; getting them to line up is
/// not.
#[test]
fn simulate_sensor_mode_matches_upstream() {
    check_group("simulate_sensor_mode", 1e-11, |a| {
        let sc = decode_scenario(a);
        let out = simulate_sensor_mode(
            &[sc.source],
            sc.emission_rate,
            &wind_for(&sc),
            &fixture_sensors(),
            &sc.config,
        );
        let (i, j) = (a[13] as usize, a[14] as usize);
        out.concentrations[i][j]
    });
}

/// The number of output rows sensor mode produces.
///
/// Compared separately and exactly: upstream's `cut` drops the final
/// simulation timestamp, so there is one fewer row than there are output
/// timestamps. An off-by-one there would otherwise surface as a confusing
/// index panic rather than as the row-count disagreement it is.
#[test]
fn simulate_sensor_mode_row_count_matches_upstream() {
    let all = cases();
    let rows: Vec<&Case> = all
        .iter()
        .filter(|c| c.function == "simulate_sensor_mode.n_rows")
        .collect();
    assert!(!rows.is_empty());
    for c in &rows {
        let sc = decode_scenario(&c.args);
        let out = simulate_sensor_mode(
            &[sc.source],
            sc.emission_rate,
            &wind_for(&sc),
            &fixture_sensors(),
            &sc.config,
        );
        assert_eq!(
            out.concentrations.len() as f64,
            c.expected,
            "scenario {} (hour {})",
            c.args[0],
            sc.hour
        );
    }
    println!(
        "simulate_sensor_mode: row counts agree on {} scenarios",
        rows.len()
    );
}

/// Two co-located sources, to check the superposition is upstream's.
#[test]
fn simulate_sensor_mode_two_sources_matches_upstream() {
    check_group("simulate_sensor_mode.two_sources", 1e-11, |a| {
        let sc = decode_scenario(a);
        let out = simulate_sensor_mode(
            &[sc.source, sc.source],
            sc.emission_rate,
            &wind_for(&sc),
            &fixture_sensors(),
            &sc.config,
        );
        let (i, j) = (a[13] as usize, a[14] as usize);
        out.concentrations[i][j]
    });
}

fn fixture_grid() -> (Vec<Length>, Vec<Length>, Vec<Length>) {
    (
        [-20.0, 0.0, 20.0, 100.0]
            .iter()
            .map(|v| Length::new::<meter>(*v))
            .collect(),
        [-10.0, 0.0, 10.0]
            .iter()
            .map(|v| Length::new::<meter>(*v))
            .collect(),
        [2.0, 10.0]
            .iter()
            .map(|v| Length::new::<meter>(*v))
            .collect(),
    )
}

/// Grid mode end-to-end, including its instantaneous (not averaged) sampling
/// and its `expand.grid` flattening order.
#[test]
fn simulate_grid_mode_matches_upstream() {
    check_group("simulate_grid_mode", 1e-11, |a| {
        let sc = decode_scenario(a);
        let (gx, gy, gz) = fixture_grid();
        let out = simulate_grid_mode(
            &[sc.source],
            sc.emission_rate,
            &wind_for(&sc),
            &gx,
            &gy,
            &gz,
            &sc.config,
        );
        let (i, j) = (a[13] as usize, a[14] as usize);
        out.concentrations[i][j]
    });
}

/// **Upstream defect 4** — grid mode's last output row is never written.
///
/// Upstream allocates `length(seq(start, end, by = output_dt))` rows but its
/// write index only reaches `floor(n_steps / (output_dt / sim_dt))`. The
/// shortfall is the final row, which stays at its initialised zero. Checked by
/// comparing the row count *and* confirming the last row is all zeros on both
/// sides — a row of zeros that a reader would otherwise take for "the plume has
/// passed".
#[test]
fn simulate_grid_mode_last_row_is_never_written() {
    let all = cases();
    let shape: Vec<&Case> = all
        .iter()
        .filter(|c| c.function == "simulate_grid_mode.n_rows")
        .collect();
    assert!(!shape.is_empty());
    let mut checked = 0usize;
    for c in &shape {
        let sc = decode_scenario(&c.args);
        let (gx, gy, gz) = fixture_grid();
        let out = simulate_grid_mode(
            &[sc.source],
            sc.emission_rate,
            &wind_for(&sc),
            &gx,
            &gy,
            &gz,
            &sc.config,
        );
        assert_eq!(out.concentrations.len() as f64, c.expected);

        // Upstream's own values for the final row, from the fixture.
        let last = out.concentrations.len() - 1;
        let upstream_last: Vec<&Case> = all
            .iter()
            .filter(|r| {
                r.function == "simulate_grid_mode"
                    && r.args[0] == c.args[0]
                    && r.args[13] as usize == last
            })
            .collect();
        assert!(
            !upstream_last.is_empty(),
            "no fixture rows for the last row"
        );
        assert!(
            upstream_last.iter().all(|r| r.expected == 0.0),
            "upstream's last grid row is not all zeros — defect 4 no longer \
             holds; re-measure and correct the defect list"
        );
        assert!(
            out.concentrations[last].iter().all(|v| *v == 0.0),
            "the port must reproduce the unwritten last row"
        );
        checked += 1;
    }
    println!("simulate_grid_mode: last row unwritten in all {checked} scenarios");
}
