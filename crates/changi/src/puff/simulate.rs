// SPDX-License-Identifier: GPL-3.0
//
// Ported from puff (R, MIT) `R/simulate_sensor_mode.R`, `R/simulate_grid_mode.R`.
// Upstream: Hammerling-Research-Group/puff @ 5213d58. See ../mod.rs for the
// full provenance block.

//! The two run modes: concentration at named sensors, and on a regular grid.
//!
//! Both march a fixed time step, emit a puff every `puff_dt`, advect every live
//! puff analytically from its own emission time, and sum the contributions.
//! They differ in *where* concentrations are evaluated and — more consequentially
//! — in *how they are reduced in time*: sensor mode averages over each output
//! interval, grid mode samples instantaneously at the end of it. See
//! [`simulate_grid_mode`] for why that asymmetry matters.

use uom::si::f64::{Length, Mass, MassRate, Time, Velocity};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

use super::concentration::gaussian_puff_methane_ppm;
use super::stability::{stability_class, StabilityClass};
use super::wind::{wind_speed, WindComponents};

/// How many puffs an emission event produces when the stability class is
/// ambiguous.
///
/// Six of upstream's ten (wind speed × day/night) regimes return **two**
/// stability classes — see [`super::stability::StabilitySet`]. Upstream then
/// builds its puff record with
///
/// ```r
/// new_puff <- data.frame(
///   time_emitted = current_elapsed,   # length 1
///   ...,
///   stab_class   = as.character(stab_class),  # length 1 OR 2
///   mass         = q_per_puff         # length 1
/// )
/// ```
///
/// and R recycles the length-1 columns against the length-2 `stab_class`, so
/// the data frame gains **two rows — each carrying the full `q_per_puff`**.
/// The emitted mass is therefore doubled across most of the wind-speed range,
/// which `q_per_puff = (emission_rate / 3600) * puff_dt` plainly does not
/// intend.
///
/// The workspace rule is that correct physics is the default and a divergence
/// must be an explicit, visible act, so [`Self::OnePuffPerEmission`] is
/// `Default` and the bug-compatible variant has to be asked for by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmissionPolicy {
    /// **Default, and mass-conserving.** One puff per emission event, carrying
    /// the whole of `q_per_puff`, dispersing with the primary (more unstable)
    /// class. This matches what upstream's own `gpuff` does with an ambiguous
    /// class — it uses the first and discards the second — so it is also the
    /// reading most consistent with the rest of upstream.
    #[default]
    OnePuffPerEmission,
    /// **Bug-compatible.** Reproduces upstream's recycling: one puff per
    /// stability class, each with the full `q_per_puff`, so an ambiguous
    /// condition emits twice the mass. Required to reproduce upstream's
    /// numbers, and used by the code-to-code fixture for exactly that reason.
    UpstreamRecycleStabilityClasses,
}

/// A source of emissions: a position and a release height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Source {
    /// Eastward position, metres in the site frame.
    pub x: Length,
    /// Northward position, metres in the site frame.
    pub y: Length,
    /// Release height above ground.
    pub height: Length,
}

/// A point where concentration is evaluated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Receptor {
    /// Eastward position, metres in the site frame.
    pub x: Length,
    /// Northward position, metres in the site frame.
    pub y: Length,
    /// Height above ground.
    pub z: Length,
}

/// Everything that does not change between the two run modes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunConfig {
    /// Simulation time step. Every puff is advected and every receptor
    /// evaluated at this cadence.
    pub sim_dt: Time,
    /// Interval between puff emissions. Upstream requires this to be a positive
    /// integer multiple of `sim_dt` so no temporal interpolation is needed;
    /// that requirement is asserted here rather than left to the caller.
    pub puff_dt: Time,
    /// Output reporting interval.
    pub output_dt: Time,
    /// Total simulated duration, `end_time - start_time`.
    pub duration: Time,
    /// How long a puff is tracked before being dropped. Upstream's default is
    /// 1200 s.
    pub puff_duration: Time,
    /// Hour of day at the start of the run, 0–23 local, used for the day/night
    /// half of the stability lookup.
    ///
    /// Upstream re-derives this from each timestamp, so a run crossing 07:00 or
    /// 19:00 changes regime mid-run. This port takes the *start* hour and holds
    /// it, which is identical for any run inside a single day/night block and
    /// differs otherwise — see `docs/puff-code-to-code.md`. The fixture only
    /// covers runs inside one block, so the difference is stated rather than
    /// verified.
    pub start_hour: u32,
    /// Which emission policy to apply. See [`EmissionPolicy`].
    pub emission_policy: EmissionPolicy,
}

/// One live puff.
#[derive(Debug, Clone, Copy)]
struct Puff {
    time_emitted_s: f64,
    wind_u: f64,
    wind_v: f64,
    class: StabilityClass,
    mass_kg: f64,
}

/// Concentration at each sensor, averaged over each output interval.
///
/// Row `i` is output interval `i`, column `j` is sensor `j`, in the order the
/// sensors were supplied.
#[derive(Debug, Clone, PartialEq)]
pub struct SensorSeries {
    /// `[interval][sensor]`, **parts per million** of methane. A bare `f64`
    /// rather than a `uom::Ratio` — see
    /// [`super::concentration::gaussian_puff_methane_ppm`] for the measured
    /// reason (`Ratio`'s base-unit round trip pushes small concentrations
    /// subnormal and truncates them).
    pub concentrations: Vec<Vec<f64>>,
    /// The start time of each interval, as an offset from the run start.
    pub interval_starts: Vec<Time>,
}

/// Concentration on the grid, sampled at the end of each output interval.
#[derive(Debug, Clone, PartialEq)]
pub struct GridSeries {
    /// `[output step][grid point]`, **parts per million** of methane, as a bare
    /// `f64` for the same reason as [`SensorSeries::concentrations`]. Grid points
    /// are in the order produced by varying `x` fastest, then `y`, then `z` —
    /// R's `expand.grid` order, preserved so a comparison is index-for-index.
    pub concentrations: Vec<Vec<f64>>,
}

fn validate(config: &RunConfig) {
    let sim = config.sim_dt.get::<second>();
    let puff = config.puff_dt.get::<second>();
    let out = config.output_dt.get::<second>();
    assert!(sim > 0.0, "sim_dt must be positive; got {sim} s");
    assert!(puff > 0.0, "puff_dt must be positive; got {puff} s");
    assert!(out > 0.0, "output_dt must be positive; got {out} s");
    assert!(
        config.duration.get::<second>() >= 0.0,
        "duration must be non-negative"
    );
    assert!(
        config.puff_duration.get::<second>() > 0.0,
        "puff_duration must be positive"
    );
    let multiple = puff / sim;
    assert!(
        (multiple - multiple.round()).abs() < 1e-9,
        "puff_dt must be an integer multiple of sim_dt (upstream's own \
         documented requirement); got puff_dt = {puff} s, sim_dt = {sim} s"
    );
    assert!(config.start_hour < 24, "start_hour must be 0-23");
}

/// Emit the puffs due at one time step, appending them to `live`.
fn emit(
    live: &mut Vec<Puff>,
    elapsed_s: f64,
    wind: WindComponents,
    config: &RunConfig,
    emission_rate: MassRate,
) {
    let speed = wind_speed(wind);
    let set = stability_class(Some(speed), config.start_hour);
    let q_per_puff = emission_rate.get::<kilogram_per_second>() * config.puff_dt.get::<second>();
    let base = Puff {
        time_emitted_s: elapsed_s,
        wind_u: wind.u.get::<meter_per_second>(),
        wind_v: wind.v.get::<meter_per_second>(),
        class: set.primary(),
        mass_kg: q_per_puff,
    };
    match config.emission_policy {
        EmissionPolicy::OnePuffPerEmission => live.push(base),
        EmissionPolicy::UpstreamRecycleStabilityClasses => {
            live.push(base);
            if let Some(second_class) = set.secondary() {
                // Upstream's recycling: the SAME mass again, not half of it.
                live.push(Puff {
                    class: second_class,
                    ..base
                });
            }
        }
    }
}

/// Total concentration at one receptor from every live puff, in ppm.
fn sum_over_puffs(live: &[Puff], source: Source, receptor: Receptor, elapsed_s: f64) -> f64 {
    let mut total = 0.0;
    for p in live {
        let age = elapsed_s - p.time_emitted_s;
        let px = source.x.get::<meter>() + p.wind_u * age;
        let py = source.y.get::<meter>() + p.wind_v * age;
        let travel = (px - source.x.get::<meter>()).hypot(py - source.y.get::<meter>());
        total += gaussian_puff_methane_ppm(
            Mass::new::<kilogram>(p.mass_kg),
            p.class,
            Length::new::<meter>(px),
            Length::new::<meter>(py),
            source.height,
            (receptor.x, receptor.y, receptor.z),
            Length::new::<meter>(travel),
        );
    }
    total
}

/// Whether a step emits, matching upstream's `t_idx == 1 || elapsed %% puff_dt == 0`.
fn emits_at(step: usize, elapsed_s: f64, puff_dt_s: f64) -> bool {
    step == 0 || (elapsed_s % puff_dt_s).abs() < 1e-9
}

/// Advance the puff population for one step: emit, age, and drop the expired.
fn step_population(
    live: &mut Vec<Puff>,
    step: usize,
    elapsed_s: f64,
    wind: WindComponents,
    config: &RunConfig,
    emission_rate: MassRate,
) {
    if emits_at(step, elapsed_s, config.puff_dt.get::<second>()) {
        emit(live, elapsed_s, wind, config, emission_rate);
    }
    let max_age = config.puff_duration.get::<second>();
    live.retain(|p| elapsed_s - p.time_emitted_s <= max_age);
}

/// Simulate concentration at a set of sensors.
///
/// Ports `simulate_sensor_mode`. Each source emits independently and the
/// contributions are summed, so multiple sources are linear — which they are,
/// the model being a sum of Gaussians.
///
/// # Arguments
/// - `sources` — one or more emission points. All share `emission_rate`,
///   exactly as upstream ("Applied uniformly to all sources").
/// - `emission_rate` — mass per unit time from **each** source.
/// - `wind` — one [`WindComponents`] per simulation step. Must be at least as
///   long as the number of steps; upstream indexes `wind_u[t_idx]` with no
///   bounds check and silently produces `NA` concentrations past the end.
/// - `sensors` — receptor positions.
/// - `config` — timing and policy, see [`RunConfig`].
///
/// # Returns
/// A [`SensorSeries`]: the **mean** concentration over each output interval,
/// where interval `i` covers simulation times `[i*output_dt, (i+1)*output_dt)`.
/// The half-open interval and the arithmetic mean both follow upstream's
/// `aggregate(..., by = cut(sim_timestamps, breaks = output_timestamps),
/// FUN = mean)`; R's `cut.POSIXt` defaults to `right = FALSE`, which is what
/// makes the intervals left-closed. The final simulation timestamp falls
/// outside the last interval and is dropped, so there is **one fewer output row
/// than there are output timestamps** — a rounding-off of the last step that is
/// upstream's behaviour and is reproduced.
///
/// # Panics
/// Panics if `config` is inconsistent (see [`RunConfig`]), if `sensors` or
/// `sources` is empty, or if `wind` is shorter than the number of simulation
/// steps. Upstream produces `NA` in the last case rather than stopping.
#[must_use]
pub fn simulate_sensor_mode(
    sources: &[Source],
    emission_rate: MassRate,
    wind: &[WindComponents],
    sensors: &[Receptor],
    config: &RunConfig,
) -> SensorSeries {
    validate(config);
    assert!(!sources.is_empty(), "need at least one source");
    assert!(!sensors.is_empty(), "need at least one sensor");

    let sim_dt = config.sim_dt.get::<second>();
    let out_dt = config.output_dt.get::<second>();
    let n_steps = (config.duration.get::<second>() / sim_dt).floor() as usize + 1;
    assert!(
        wind.len() >= n_steps,
        "need one wind sample per simulation step: {n_steps} steps, {} samples",
        wind.len()
    );

    // Per-step, per-sensor totals summed over sources.
    let mut per_step = vec![vec![0.0_f64; sensors.len()]; n_steps];
    for source in sources {
        let mut live: Vec<Puff> = Vec::new();
        for step in 0..n_steps {
            let elapsed = (step as f64) * sim_dt;
            step_population(&mut live, step, elapsed, wind[step], config, emission_rate);
            if live.is_empty() {
                continue;
            }
            for (j, sensor) in sensors.iter().enumerate() {
                per_step[step][j] += sum_over_puffs(&live, *source, *sensor, elapsed);
            }
        }
    }

    // Upstream's aggregate-by-cut: left-closed intervals, arithmetic mean, the
    // final timestamp dropped.
    let n_output_ts = (config.duration.get::<second>() / out_dt).floor() as usize + 1;
    let n_intervals = n_output_ts.saturating_sub(1);
    let mut concentrations = Vec::with_capacity(n_intervals);
    let mut interval_starts = Vec::with_capacity(n_intervals);
    for i in 0..n_intervals {
        let lo = (i as f64) * out_dt;
        let hi = ((i + 1) as f64) * out_dt;
        let mut sums = vec![0.0_f64; sensors.len()];
        let mut count = 0usize;
        for (step, row) in per_step.iter().enumerate() {
            let t = (step as f64) * sim_dt;
            if t >= lo && t < hi {
                for (j, v) in row.iter().enumerate() {
                    sums[j] += v;
                }
                count += 1;
            }
        }
        let n = count.max(1) as f64;
        concentrations.push(sums.into_iter().map(|s| s / n).collect());
        interval_starts.push(Time::new::<second>(lo));
    }

    SensorSeries {
        concentrations,
        interval_starts,
    }
}

/// Simulate concentration on a regular grid.
///
/// Ports `simulate_grid_mode`. The grid is the Cartesian product of the three
/// coordinate vectors, flattened with `x` varying fastest then `y` then `z`,
/// which is R's `expand.grid` order.
///
/// # Arguments
/// - `sources`, `emission_rate`, `wind`, `config` — as for
///   [`simulate_sensor_mode`].
/// - `grid_x`, `grid_y`, `grid_z` — the grid axes.
///
/// # Returns
/// A [`GridSeries`] with one row per output timestamp.
///
/// # Two divergences from sensor mode, both upstream's
///
/// 1. **Instantaneous, not averaged.** Grid mode writes the concentration at
///    the single step where `step_index % (output_dt / sim_dt) == 0`, whereas
///    sensor mode averages the whole interval. The two modes therefore do not
///    report the same quantity, and a grid value will be noisier than the
///    sensor value at the same place and time.
/// 2. **The last output row is usually zero.** Upstream allocates
///    `length(seq(start, end, by = output_dt))` rows but only ever fills index
///    `t_idx / (output_dt / sim_dt)` for `t_idx` in `1..n_steps`, which reaches
///    at most `floor(n_steps / (output_dt / sim_dt))`. With upstream's own
///    documented example (`n_steps = 361`, `output_dt/sim_dt = 12`) that is 30
///    of 31 rows, leaving the last all zeros.
///
/// Both are reproduced rather than corrected, because this function's contract
/// is to be comparable with upstream. They are recorded in
/// `docs/puff-code-to-code.md` as upstream defects 3 and 4.
///
/// # Panics
/// As [`simulate_sensor_mode`], plus if any grid axis is empty.
#[must_use]
pub fn simulate_grid_mode(
    sources: &[Source],
    emission_rate: MassRate,
    wind: &[WindComponents],
    grid_x: &[Length],
    grid_y: &[Length],
    grid_z: &[Length],
    config: &RunConfig,
) -> GridSeries {
    validate(config);
    assert!(!sources.is_empty(), "need at least one source");
    assert!(
        !grid_x.is_empty() && !grid_y.is_empty() && !grid_z.is_empty(),
        "every grid axis needs at least one coordinate"
    );

    let sim_dt = config.sim_dt.get::<second>();
    let out_dt = config.output_dt.get::<second>();
    let n_steps = (config.duration.get::<second>() / sim_dt).floor() as usize + 1;
    assert!(
        wind.len() >= n_steps,
        "need one wind sample per simulation step: {n_steps} steps, {} samples",
        wind.len()
    );

    // expand.grid order: x fastest, then y, then z.
    let mut grid: Vec<Receptor> = Vec::with_capacity(grid_x.len() * grid_y.len() * grid_z.len());
    for z in grid_z {
        for y in grid_y {
            for x in grid_x {
                grid.push(Receptor {
                    x: *x,
                    y: *y,
                    z: *z,
                });
            }
        }
    }

    let n_output = (config.duration.get::<second>() / out_dt).floor() as usize + 1;
    let stride = (out_dt / sim_dt).round() as usize;
    assert!(
        stride >= 1,
        "output_dt must be at least sim_dt for grid mode's index arithmetic"
    );
    let mut out = vec![vec![0.0_f64; grid.len()]; n_output];

    for source in sources {
        let mut live: Vec<Puff> = Vec::new();
        for step in 0..n_steps {
            let elapsed = (step as f64) * sim_dt;
            step_population(&mut live, step, elapsed, wind[step], config, emission_rate);
            if live.is_empty() {
                continue;
            }
            // Upstream indexes from 1: `t_idx %% stride == 0` with t_idx = step + 1.
            let t_idx = step + 1;
            if t_idx % stride != 0 {
                continue;
            }
            let output_idx = t_idx / stride;
            // Upstream writes `concentrations[output_idx, ]` with 1-based
            // indexing, so slot `output_idx - 1` here. Rows past the end are
            // impossible in R (it would error), so a guard is a real bound.
            if output_idx == 0 || output_idx > n_output {
                continue;
            }
            for (g, receptor) in grid.iter().enumerate() {
                out[output_idx - 1][g] += sum_over_puffs(&live, *source, *receptor, elapsed);
            }
        }
    }

    GridSeries {
        concentrations: out,
    }
}

/// Build a constant wind series of `n` samples, a convenience for examples and
/// tests that do not care about wind variability.
#[must_use]
pub fn constant_wind(u: Velocity, v: Velocity, n: usize) -> Vec<WindComponents> {
    vec![WindComponents { u, v }; n]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(policy: EmissionPolicy, start_hour: u32) -> RunConfig {
        RunConfig {
            sim_dt: Time::new::<second>(10.0),
            puff_dt: Time::new::<second>(10.0),
            output_dt: Time::new::<second>(120.0),
            duration: Time::new::<second>(600.0),
            puff_duration: Time::new::<second>(1200.0),
            start_hour,
            emission_policy: policy,
        }
    }

    fn source() -> Source {
        Source {
            x: Length::new::<meter>(0.0),
            y: Length::new::<meter>(0.0),
            height: Length::new::<meter>(2.5),
        }
    }

    fn sensors() -> Vec<Receptor> {
        vec![Receptor {
            x: Length::new::<meter>(20.0),
            y: Length::new::<meter>(10.0),
            z: Length::new::<meter>(2.0),
        }]
    }

    /// The default policy conserves mass; the bug-compatible one does not.
    /// This is the test the "correct physics is the default" rule asks for —
    /// it fails if the default is ever flipped back.
    #[test]
    fn the_default_policy_emits_one_puff_per_event() {
        assert_eq!(
            EmissionPolicy::default(),
            EmissionPolicy::OnePuffPerEmission
        );

        // 1.5 m/s at midday is the ambiguous A/B regime.
        let wind = constant_wind(
            Velocity::new::<meter_per_second>(1.5),
            Velocity::new::<meter_per_second>(0.0),
            64,
        );
        let rate = MassRate::new::<kilogram_per_second>(3.5 / 3600.0);

        let mut live_default = Vec::new();
        emit(
            &mut live_default,
            0.0,
            wind[0],
            &cfg(EmissionPolicy::OnePuffPerEmission, 12),
            rate,
        );
        let mut live_upstream = Vec::new();
        emit(
            &mut live_upstream,
            0.0,
            wind[0],
            &cfg(EmissionPolicy::UpstreamRecycleStabilityClasses, 12),
            rate,
        );

        assert_eq!(live_default.len(), 1);
        assert_eq!(live_upstream.len(), 2);

        let mass_default: f64 = live_default.iter().map(|p| p.mass_kg).sum();
        let mass_upstream: f64 = live_upstream.iter().map(|p| p.mass_kg).sum();
        let intended = 3.5 / 3600.0 * 10.0;
        assert!((mass_default - intended).abs() < 1e-18);
        assert!(
            (mass_upstream - 2.0 * intended).abs() < 1e-18,
            "the bug-compatible policy must reproduce the doubling, or the \
             code-to-code fixture cannot match upstream"
        );
    }

    /// The mass doubles exactly; the reported CONCENTRATION does not.
    ///
    /// The two recycled puffs carry *different* stability classes, so the
    /// second disperses differently and contributes a different concentration.
    /// Pinned because it is easy — and wrong — to describe upstream's defect as
    /// "the answer doubles", which would invite a reader to divide a published
    /// result by two and call it corrected.
    ///
    /// Measured 2026-09-21 at 1.5 m/s, midday (the ambiguous A/B regime), one
    /// source, receptor 50 m downwind: ratio **1.78**, not 2.00.
    #[test]
    fn doubling_the_mass_does_not_double_the_concentration() {
        let wind = constant_wind(
            Velocity::new::<meter_per_second>(1.5),
            Velocity::new::<meter_per_second>(0.0),
            64,
        );
        let rate = MassRate::new::<kilogram_per_second>(3.5 / 3600.0);
        let receptors = vec![Receptor {
            x: Length::new::<meter>(50.0),
            y: Length::new::<meter>(20.0),
            z: Length::new::<meter>(2.0),
        }];
        let peak = |policy| {
            simulate_sensor_mode(&[source()], rate, &wind, &receptors, &cfg(policy, 12))
                .concentrations
                .iter()
                .map(|row| row[0])
                .fold(0.0_f64, f64::max)
        };
        let ours = peak(EmissionPolicy::OnePuffPerEmission);
        let upstream = peak(EmissionPolicy::UpstreamRecycleStabilityClasses);
        assert!(ours > 0.0, "the test case must actually register something");
        let ratio = upstream / ours;
        assert!(
            ratio > 1.0 && ratio < 2.0,
            "expected a ratio strictly between 1 and 2 (the two puffs disperse \
             differently), got {ratio}"
        );
        assert!(
            (ratio - 1.78).abs() < 0.01,
            "measured ratio moved from 1.78 to {ratio}; re-measure and update \
             the doc comment, the example and docs/puff-code-to-code.md"
        );
    }

    /// In an unambiguous regime the two policies must agree exactly, which
    /// bounds the divergence to where the stability table is ambiguous.
    #[test]
    fn the_policies_agree_where_stability_is_unambiguous() {
        // 8 m/s is the U >= 6 regime: class D, one class, day or night.
        let wind = constant_wind(
            Velocity::new::<meter_per_second>(8.0),
            Velocity::new::<meter_per_second>(0.0),
            64,
        );
        let rate = MassRate::new::<kilogram_per_second>(3.5 / 3600.0);
        let a = simulate_sensor_mode(
            &[source()],
            rate,
            &wind,
            &sensors(),
            &cfg(EmissionPolicy::OnePuffPerEmission, 12),
        );
        let b = simulate_sensor_mode(
            &[source()],
            rate,
            &wind,
            &sensors(),
            &cfg(EmissionPolicy::UpstreamRecycleStabilityClasses, 12),
        );
        assert_eq!(a, b);
    }

    /// Sources are linear: two identical sources give exactly twice one.
    #[test]
    fn sources_superpose_linearly() {
        let wind = constant_wind(
            Velocity::new::<meter_per_second>(8.0),
            Velocity::new::<meter_per_second>(0.0),
            64,
        );
        let rate = MassRate::new::<kilogram_per_second>(3.5 / 3600.0);
        let one = simulate_sensor_mode(
            &[source()],
            rate,
            &wind,
            &sensors(),
            &cfg(EmissionPolicy::OnePuffPerEmission, 12),
        );
        let two = simulate_sensor_mode(
            &[source(), source()],
            rate,
            &wind,
            &sensors(),
            &cfg(EmissionPolicy::OnePuffPerEmission, 12),
        );
        for (a, b) in one.concentrations.iter().zip(two.concentrations.iter()) {
            for (x, y) in a.iter().zip(b.iter()) {
                assert!((y - 2.0 * x).abs() <= 1e-12 * y.abs().max(1e-30));
            }
        }
    }

    /// Grid mode's final output row is left at zero by upstream's index
    /// arithmetic. Pinned, because a reader meeting a row of zeros would
    /// otherwise assume the plume had gone.
    #[test]
    fn grid_modes_last_output_row_is_never_written() {
        let wind = constant_wind(
            Velocity::new::<meter_per_second>(2.0),
            Velocity::new::<meter_per_second>(1.0),
            64,
        );
        let g = simulate_grid_mode(
            &[source()],
            MassRate::new::<kilogram_per_second>(3.5 / 3600.0),
            &wind,
            &[Length::new::<meter>(20.0)],
            &[Length::new::<meter>(10.0)],
            &[Length::new::<meter>(2.0)],
            &cfg(EmissionPolicy::UpstreamRecycleStabilityClasses, 12),
        );
        // duration 600 s, output_dt 120 s -> 6 output timestamps.
        assert_eq!(g.concentrations.len(), 6);
        let last = &g.concentrations[5];
        assert!(last.iter().all(|c| *c == 0.0));
        // and an earlier row is not zero, so the test is not vacuous
        assert!(g.concentrations[4].iter().any(|c| *c > 0.0));
    }

    #[test]
    #[should_panic(expected = "puff_dt must be an integer multiple of sim_dt")]
    fn a_non_multiple_puff_interval_is_rejected() {
        let mut c = cfg(EmissionPolicy::OnePuffPerEmission, 12);
        c.puff_dt = Time::new::<second>(15.0);
        validate(&c);
    }
}
