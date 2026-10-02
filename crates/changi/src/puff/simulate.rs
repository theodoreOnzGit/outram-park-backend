// SPDX-License-Identifier: GPL-3.0
//
// Ported from puff (R, MIT) `R/simulate_sensor_mode.R`, `R/simulate_grid_mode.R`.
// Upstream: Hammerling-Research-Group/puff @ 5213d58. See ../mod.rs for the
// full provenance block.

//! The two run modes: concentration at named sensors, and on a regular grid.
//!
//! Both march a fixed time step, emit a puff every `puff_dt`, advect every live
//! puff, and sum the contributions. They differ in *where* concentrations are
//! evaluated and — more consequentially — in *how they are reduced in time*:
//! sensor mode averages over each output interval, grid mode samples
//! instantaneously at the end of it. See [`simulate_grid_mode`] for why that
//! asymmetry matters.
//!
//! **How a puff is advected is a policy** — see [`AdvectionPolicy`]. The
//! default integrates each puff's own trajectory step by step with the wind
//! that actually blows, so a puff *turns* when the wind turns and remembers
//! where it had got to. Upstream's analytic `source + u_emit * age` is retained
//! as the bug-compatible variant the code-to-code fixture asks for by name.

use uom::si::f64::{Length, Mass, MassRate, Time, Velocity};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

use super::concentration::{gaussian_puff_concentration, gaussian_puff_methane_ppm};
use super::stability::{stability_class, StabilityClass, StabilitySet};
use super::wind::{wind_speed, WindComponents};

/// How a live puff's position and dispersion distance are obtained.
///
/// # Why this exists — a puff that cannot turn is not Lagrangian
///
/// Upstream places a puff analytically: it stores the wind sampled at the
/// puff's **moment of emission** and, forever after, puts the puff at
/// `source + (u, v) * age`. A puff therefore flies a perfectly straight line on
/// the wind of its birth, and **never responds to the wind again**. For the
/// steady wind upstream's own examples run at that is exact and free. For a
/// wind that veers — which is the case a puff model is reached for in the first
/// place — it is wrong: a plume that should bend into a dog-leg stays a
/// straight ray, and the model has no memory of where each puff had actually
/// got to.
///
/// A *Lagrangian* treatment is the fix, and it is what the word already
/// promises: each puff is a parcel carrying its own state, and its position is
/// the **integral of the wind it has actually experienced**,
///
/// ```text
/// (x, y)_{n+1} = (x, y)_n + (u, v)(t_n) * dt
/// ```
///
/// so the wind changing rotates only what happens *next*. History is state, not
/// something recomputed from the present.
///
/// # Dispersion distance: PATH LENGTH, not net displacement
///
/// The second half of the fix, and the easier one to miss. Pasquill–Gifford
/// `sigma_y`, `sigma_z` grow with the distance a puff has travelled *through
/// the turbulent field*. On a straight trajectory that is the same number as
/// the straight-line distance from the source, which is why upstream can write
/// `hypot(dx, dy)` and be right. On a curved trajectory the two part company,
/// and net displacement is the wrong one — a puff blown 500 m east and then
/// 500 m back west has dispersed for 1 km of travel while sitting 0 m from the
/// stack, and `hypot` would call it undispersed.
///
/// [`Self::LagrangianTrajectory`] therefore accumulates **path length**
/// alongside position. [`Self::UpstreamFrozenWind`] keeps `hypot`, because on
/// its straight ray the two agree identically and the fixture compares digits.
///
/// # The two agree exactly for a constant wind — by construction
///
/// Marching `dx += u * dt` for `n` steps gives `u * n * dt = u * age`, so on a
/// constant wind the Lagrangian population sits exactly where the analytic one
/// does, and the accumulated path equals `|U| * age`. The difference is
/// therefore confined to precisely the case upstream gets wrong.
///
/// It is **not bit-identical**: `n` accumulated additions do not round the same
/// way as one multiplication. That is the whole reason the bug-compatible
/// variant has to exist rather than being inferred — `tests/puff_code_to_code.rs`
/// compares against upstream R near machine epsilon, and a summation-order
/// difference of a few ulps would read as a translation error.
/// `tests::the_two_advection_policies_agree_on_a_constant_wind` measures the
/// actual agreement.
///
/// # Which is the default, and why
///
/// [`Self::LagrangianTrajectory`], because the workspace rule is that physics
/// the model is meant to represent is applied unless a caller explicitly
/// ablates it, and an ablation must be a visible act rather than the default
/// state. Same reasoning, and the same shape, as [`EmissionPolicy`] in this
/// file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AdvectionPolicy {
    /// **Default.** Integrate each puff's trajectory with the wind that
    /// actually blows at each step, accumulating position and path length. A
    /// puff turns when the wind turns, and keeps the position it had reached.
    #[default]
    LagrangianTrajectory,
    /// **Bug-compatible.** Upstream's analytic placement: the wind is frozen at
    /// emission and the puff is put at `source + u_emit * age`, with dispersion
    /// distance `hypot` of that displacement. A puff never turns. Required to
    /// reproduce upstream's numbers digit for digit, and used by the
    /// code-to-code fixture for exactly that reason.
    UpstreamFrozenWind,
}

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
    /// How puffs are advected. See [`AdvectionPolicy`]; the default is the
    /// Lagrangian trajectory, and upstream's frozen wind must be asked for.
    pub advection: AdvectionPolicy,
}

/// One live puff.
///
/// `pub(crate)` rather than private so [`crate::activity`] can drive the same
/// puff population without a second copy of the advection. It is **not** part of
/// the public API and is not re-exported — the visibility is the minimum that
/// lets one crate-internal consumer reuse this loop, and no field or method of
/// it changed when that consumer was added.
///
/// # The trajectory fields are DISPLACEMENTS from the source, not absolute
///
/// `dx_m`, `dy_m` are measured from the emitting source's position, so a puff
/// does not need to know where it was born and [`emit_with_classes`] does not
/// need the [`Source`] passed to it. Every source emits into its own `live`
/// vector and the wind is source-independent, so a source-relative trajectory
/// is exactly as general as an absolute one — and it keeps the `pub(crate)`
/// emission interface [`crate::activity`] shares unchanged.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Puff {
    pub(crate) time_emitted_s: f64,
    /// Wind at the **moment of emission**. Read only by
    /// [`AdvectionPolicy::UpstreamFrozenWind`]; the Lagrangian path has already
    /// integrated it into `dx_m`/`dy_m` and does not look back at it.
    pub(crate) wind_u: f64,
    /// See [`Self::wind_u`].
    pub(crate) wind_v: f64,
    /// Eastward displacement from the source \[m\], integrated step by step.
    pub(crate) dx_m: f64,
    /// Northward displacement from the source \[m\], integrated step by step.
    pub(crate) dy_m: f64,
    /// **Path length** travelled \[m\] — the argument Pasquill–Gifford wants.
    /// Not `hypot(dx, dy)`: see [`AdvectionPolicy`] on why net displacement is
    /// the wrong distance once a trajectory curves.
    pub(crate) path_m: f64,
    pub(crate) class: StabilityClass,
    pub(crate) mass_kg: f64,
}

/// A puff's displacement from its source and its dispersion distance, under
/// whichever [`AdvectionPolicy`] is in force.
///
/// Returns `(dx, dy, distance)` in metres. This is the **single** place the
/// policy is read, so the two call sites that need a puff's position
/// ([`sum_over_puffs`] and [`puff_unit_response`]) cannot disagree about it.
fn puff_offset(p: &Puff, policy: AdvectionPolicy, elapsed_s: f64) -> (f64, f64, f64) {
    match policy {
        AdvectionPolicy::LagrangianTrajectory => (p.dx_m, p.dy_m, p.path_m),
        AdvectionPolicy::UpstreamFrozenWind => {
            let age = elapsed_s - p.time_emitted_s;
            let dx = p.wind_u * age;
            let dy = p.wind_v * age;
            (dx, dy, dx.hypot(dy))
        }
    }
}

/// Advance every live puff by one step of `dt` on `wind`.
///
/// A no-op under [`AdvectionPolicy::UpstreamFrozenWind`], whose positions are
/// derived from age on demand and must not be marched.
///
/// Forward Euler on the wind at the **start** of the interval the puff
/// traverses, which is the sample upstream would have indexed had it advected
/// at all. It is exact for a constant wind (see [`AdvectionPolicy`]) and
/// first-order in `dt` otherwise; `sim_dt` is 10 s at this crate's working
/// points, against wind that changes on minutes, so the truncation error is far
/// below the dispersion model's own fidelity. Stated rather than assumed.
pub(crate) fn advect(live: &mut [Puff], wind: WindComponents, dt: Time, policy: AdvectionPolicy) {
    if policy == AdvectionPolicy::UpstreamFrozenWind {
        return;
    }
    let u = wind.u.get::<meter_per_second>();
    let v = wind.v.get::<meter_per_second>();
    let dt_s = dt.get::<second>();
    let step_dx = u * dt_s;
    let step_dy = v * dt_s;
    // The length of THIS leg, accumulated — not recomputed from the endpoints.
    let leg = step_dx.hypot(step_dy);
    for p in live.iter_mut() {
        p.dx_m += step_dx;
        p.dy_m += step_dy;
        p.path_m += leg;
    }
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
    emit_with_classes(live, elapsed_s, wind, set, config, emission_rate);
}

/// [`emit`] with the stability classes supplied rather than derived from the
/// wind speed and the hour.
///
/// Factored out so [`crate::activity`] can hold the stability class fixed
/// across a sweep — something [`stability_class`] cannot express, since it
/// derives the class *from* the wind speed. [`emit`] is the only caller inside
/// this module and passes exactly what it computed before, so upstream's
/// behaviour on the verified path is unchanged by the split.
pub(crate) fn emit_with_classes(
    live: &mut Vec<Puff>,
    elapsed_s: f64,
    wind: WindComponents,
    set: StabilitySet,
    config: &RunConfig,
    emission_rate: MassRate,
) {
    let q_per_puff = emission_rate.get::<kilogram_per_second>() * config.puff_dt.get::<second>();
    let base = Puff {
        time_emitted_s: elapsed_s,
        wind_u: wind.u.get::<meter_per_second>(),
        wind_v: wind.v.get::<meter_per_second>(),
        // A newly emitted puff is AT its source and has travelled nothing. It is
        // advected for the first time on the next step, which is what makes the
        // marched position equal `u * age` on a constant wind.
        dx_m: 0.0,
        dy_m: 0.0,
        path_m: 0.0,
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
fn sum_over_puffs(
    live: &[Puff],
    source: Source,
    receptor: Receptor,
    elapsed_s: f64,
    policy: AdvectionPolicy,
) -> f64 {
    let mut total = 0.0;
    for p in live {
        let (dx, dy, travel) = puff_offset(p, policy, elapsed_s);
        let px = source.x.get::<meter>() + dx;
        let py = source.y.get::<meter>() + dy;
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

/// One puff's contribution at one receptor, **per kilogram of puff mass**.
///
/// This is [`gaussian_puff_concentration`] evaluated at `mass = 1 kg`, so the
/// returned bare `f64` carries units of **m^-3**: multiply by a puff mass in kg
/// to get kg/m^3, or by an activity in Bq to get Bq/m^3. Scaling afterwards is
/// *exact*, not an approximation — the kernel is linear in mass, which
/// `concentration::tests::concentration_is_linear_in_puff_mass` pins.
///
/// The puff's own `mass_kg` is deliberately **ignored**. A caller wanting the
/// puff's actual contribution multiplies it back in; a caller building a
/// unit-release response (which is why this exists) does not want it at all.
///
/// It repeats [`sum_over_puffs`]'s *kernel call* rather than calling it, and
/// that duplication is deliberate. `sum_over_puffs` goes through
/// [`gaussian_puff_methane_ppm`], whose `1e6 * 1.524` is methane-specific and
/// whose multiply order is what the code-to-code fixture compares against
/// upstream R. Routing it through this function would change that arithmetic to
/// chase a two-line saving.
///
/// **The advection itself is no longer duplicated** — CORRECTED 2026-09-27.
/// Both sites now call [`puff_offset`], so the policy cannot be applied
/// inconsistently. The struck instruction below is kept because it is what the
/// fix was: ~~"If the advection is ever corrected, both copies must move
/// together"~~ — there is one copy now. The two remain pinned to agree by
/// `activity::chi_over_q::tests::unit_response_agrees_with_the_ported_sum`.
pub(crate) fn puff_unit_response(
    p: &Puff,
    source: Source,
    receptor: Receptor,
    elapsed_s: f64,
    policy: AdvectionPolicy,
) -> f64 {
    let (dx, dy, travel) = puff_offset(p, policy, elapsed_s);
    let px = source.x.get::<meter>() + dx;
    let py = source.y.get::<meter>() + dy;
    gaussian_puff_concentration(
        Mass::new::<kilogram>(1.0),
        p.class,
        Length::new::<meter>(px),
        Length::new::<meter>(py),
        source.height,
        (receptor.x, receptor.y, receptor.z),
        Length::new::<meter>(travel),
    )
    .get::<kilogram_per_cubic_meter>()
}

/// Whether a step emits, matching upstream's `t_idx == 1 || elapsed %% puff_dt == 0`.
pub(crate) fn emits_at(step: usize, elapsed_s: f64, puff_dt_s: f64) -> bool {
    step == 0 || (elapsed_s % puff_dt_s).abs() < 1e-9
}

/// Advance the puff population for one step: emit, age, and drop the expired.
///
/// **Advection is NOT done here.** It belongs between steps, on the wind that
/// blew during the interval just traversed, and this function is called *at* a
/// step with that step's own wind. The two run modes therefore call
/// [`advect`] themselves before calling this — see [`simulate_sensor_mode`]'s
/// loop for the ordering and why it makes the marched position equal `u * age`
/// on a constant wind.
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
            // Advect first, on the wind that blew over the interval just
            // traversed, THEN emit — so a puff emitted at this step starts at
            // its source with zero travel, and one emitted `k` steps ago has
            // been advected exactly `k` times. On a constant wind that is
            // `u * age`, identical to upstream's analytic placement.
            if step > 0 {
                advect(&mut live, wind[step - 1], config.sim_dt, config.advection);
            }
            step_population(&mut live, step, elapsed, wind[step], config, emission_rate);
            if live.is_empty() {
                continue;
            }
            for (j, sensor) in sensors.iter().enumerate() {
                per_step[step][j] +=
                    sum_over_puffs(&live, *source, *sensor, elapsed, config.advection);
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
            // Advect then emit — see `simulate_sensor_mode`'s loop.
            if step > 0 {
                advect(&mut live, wind[step - 1], config.sim_dt, config.advection);
            }
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
                out[output_idx - 1][g] +=
                    sum_over_puffs(&live, *source, *receptor, elapsed, config.advection);
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
            advection: AdvectionPolicy::default(),
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

    /// The Lagrangian default must be the default, and a **veering wind must
    /// bend the plume** rather than sweeping the whole of it onto the new
    /// bearing.
    ///
    /// # Why this test is the point of the whole `AdvectionPolicy` change
    ///
    /// Reported 2026-09-27: *"when puff particles go around, and the wind
    /// direction changes, the puffs don't seem to remember their last known
    /// location."* Under [`AdvectionPolicy::UpstreamFrozenWind`] that is
    /// exactly right, and it is upstream's model: a puff's position is
    /// *recomputed* from its age every time it is evaluated, so it has no
    /// memory at all — it only looks as though it does while the wind holds
    /// still.
    ///
    /// # Methodology
    ///
    /// One puff, emitted at `t = 0`, `sim_dt = 10 s`. The wind blows **due east
    /// at 5 m/s for 10 steps (100 s)**, then turns **due north at 5 m/s for 10
    /// steps**. No dispersion kernel is involved: the advection is checked
    /// directly on the puff's own state, because that is where the defect
    /// lives.
    ///
    /// Predicted, stated before measuring — a real Lagrangian parcel must end
    /// at the **corner of the dog-leg**, `(500 m east, 500 m north)`, having
    /// travelled a **path length of 1000 m**, with a net displacement of only
    /// `707.1 m`. The frozen-wind puff must instead sit at
    /// `(1000 m east, 0)` — 1000 m along the wind of its *birth*, with the
    /// second leg never having happened.
    ///
    /// # Results (measured 2026-09-27)
    ///
    /// | Quantity | Lagrangian | Frozen wind (upstream) |
    /// |---|---|---|
    /// | `dx` | **500.0 m** | 1000.0 m |
    /// | `dy` | **500.0 m** | 0.0 m |
    /// | dispersion distance | **1000.0 m** (path) | 1000.0 m (`hypot`) |
    /// | net displacement | 707.1 m | 1000.0 m |
    ///
    /// Both match the prediction exactly. Note the dispersion distance agrees
    /// by coincidence at this particular geometry — the legs are equal — while
    /// the *positions* are 500 m apart. A test written on the distance alone
    /// would have passed against the bug.
    ///
    /// **Interpretation.** The Lagrangian puff turns and keeps the position it
    /// had reached; the frozen-wind puff is teleported onto a ray it never
    /// flew. The path length, not the net displacement, is what feeds
    /// Pasquill–Gifford — `1000 m` against `707.1 m`, a 41 % error in
    /// dispersion distance that grows without bound as a trajectory doubles
    /// back.
    #[test]
    fn a_veering_wind_bends_a_lagrangian_puff_and_teleports_an_upstream_one() {
        assert_eq!(
            AdvectionPolicy::default(),
            AdvectionPolicy::LagrangianTrajectory,
            "correct physics is the default; an ablation must be asked for by name"
        );

        let dt = Time::new::<second>(10.0);
        let east = WindComponents {
            u: Velocity::new::<meter_per_second>(5.0),
            v: Velocity::new::<meter_per_second>(0.0),
        };
        let north = WindComponents {
            u: Velocity::new::<meter_per_second>(0.0),
            v: Velocity::new::<meter_per_second>(5.0),
        };

        // One puff, emitted on the easterly. Its stored emission wind is east,
        // which is what the frozen-wind policy will keep using.
        let mut live = Vec::new();
        emit(
            &mut live,
            0.0,
            east,
            &cfg(EmissionPolicy::OnePuffPerEmission, 12),
            MassRate::new::<kilogram_per_second>(1.0),
        );

        for _ in 0..10 {
            advect(&mut live, east, dt, AdvectionPolicy::LagrangianTrajectory);
        }
        for _ in 0..10 {
            advect(&mut live, north, dt, AdvectionPolicy::LagrangianTrajectory);
        }
        let elapsed = 200.0;
        let p = live[0];

        let (dx_l, dy_l, dist_l) = puff_offset(&p, AdvectionPolicy::LagrangianTrajectory, elapsed);
        let (dx_u, dy_u, dist_u) = puff_offset(&p, AdvectionPolicy::UpstreamFrozenWind, elapsed);
        println!(
            "VEERING WIND (east 100 s, then north 100 s), one puff at t = {elapsed} s\n  \
             Lagrangian : dx = {dx_l:.1} m, dy = {dy_l:.1} m, path = {dist_l:.1} m, \
             net displacement = {:.1} m\n  \
             frozen wind: dx = {dx_u:.1} m, dy = {dy_u:.1} m, hypot = {dist_u:.1} m",
            dx_l.hypot(dy_l),
        );

        // The Lagrangian puff is at the corner of the dog-leg.
        assert!((dx_l - 500.0).abs() < 1e-9, "dx = {dx_l}, wanted 500 m");
        assert!((dy_l - 500.0).abs() < 1e-9, "dy = {dy_l}, wanted 500 m");
        // Its dispersion distance is the PATH, 1000 m, not the 707.1 m chord.
        assert!(
            (dist_l - 1000.0).abs() < 1e-9,
            "path = {dist_l}, wanted 1000 m"
        );
        assert!(
            dist_l > dx_l.hypot(dy_l),
            "on a bent trajectory the path {dist_l} must exceed the net displacement {}",
            dx_l.hypot(dy_l)
        );

        // The frozen-wind puff never turned: still due east, second leg lost.
        assert!((dx_u - 1000.0).abs() < 1e-9, "dx = {dx_u}, wanted 1000 m");
        assert!(
            dy_u.abs() < 1e-9,
            "dy = {dy_u}, wanted 0 m -- it never turned"
        );
        // And it is half a kilometre from where the parcel actually is.
        assert!(
            (dx_u - dx_l).hypot(dy_u - dy_l) > 490.0,
            "the two policies must disagree on a veering wind; they are only \
             {:.1} m apart",
            (dx_u - dx_l).hypot(dy_u - dy_l)
        );
    }

    /// The two advection policies must agree on a **constant** wind, which is
    /// what makes the Lagrangian default safe to ship over a verified port.
    ///
    /// # Methodology
    ///
    /// The identity is algebraic: marching `dx += u*dt` for `n` steps gives
    /// `u*n*dt = u*age`. So on a constant wind the marched population must sit
    /// where upstream's analytic one does, and the accumulated path must equal
    /// `|U|*age`. Checked over a full 1200 s puff lifetime at 10 s steps (120
    /// advections, the simulator's own working point) on a wind with both
    /// components non-zero, so a bug in either axis shows.
    ///
    /// Asserted at **1e-9 relative**, not at zero: `n` accumulated additions do
    /// not round the same way as one multiplication, and the residual is real
    /// floating-point summation error rather than a modelling difference.
    ///
    /// # Results (measured 2026-09-27)
    ///
    /// Wind `(u, v) = (3.0, -1.5) m/s`, 120 steps of 10 s, `age = 1200 s`:
    ///
    /// | Quantity | Analytic | Marched | Relative |
    /// |---|---|---|---|
    /// | `dx` | 3600 m | 3600 m | printed by the test |
    /// | `dy` | -1800 m | -1800 m | printed by the test |
    /// | distance | 4024.92 m | 4024.92 m | printed by the test |
    ///
    /// **Interpretation.** The Lagrangian default reproduces upstream's answer
    /// wherever upstream's assumption holds, so switching the default changes
    /// no result computed on a steady wind — including every number in
    /// `docs/puff-code-to-code.md`, whose fixture is constant-wind throughout.
    /// It is nonetheless **not bit-identical**, which is why that fixture pins
    /// [`AdvectionPolicy::UpstreamFrozenWind`] explicitly rather than relying
    /// on this agreement.
    #[test]
    fn the_two_advection_policies_agree_on_a_constant_wind() {
        let dt = Time::new::<second>(10.0);
        let wind = WindComponents {
            u: Velocity::new::<meter_per_second>(3.0),
            v: Velocity::new::<meter_per_second>(-1.5),
        };
        let mut live = Vec::new();
        emit(
            &mut live,
            0.0,
            wind,
            &cfg(EmissionPolicy::OnePuffPerEmission, 12),
            MassRate::new::<kilogram_per_second>(1.0),
        );
        let steps = 120;
        for _ in 0..steps {
            advect(&mut live, wind, dt, AdvectionPolicy::LagrangianTrajectory);
        }
        let elapsed = steps as f64 * dt.get::<second>();
        let p = live[0];
        let (dx_l, dy_l, dist_l) = puff_offset(&p, AdvectionPolicy::LagrangianTrajectory, elapsed);
        let (dx_u, dy_u, dist_u) = puff_offset(&p, AdvectionPolicy::UpstreamFrozenWind, elapsed);
        let rel = |a: f64, b: f64| {
            if b == 0.0 {
                a.abs()
            } else {
                (a - b).abs() / b.abs()
            }
        };
        println!(
            "CONSTANT WIND ({} , {}) m/s over {elapsed} s in {steps} steps\n  \
             analytic: dx = {dx_u:.6} m, dy = {dy_u:.6} m, dist = {dist_u:.6} m\n  \
             marched : dx = {dx_l:.6} m, dy = {dy_l:.6} m, dist = {dist_l:.6} m\n  \
             relative: dx {:.2e}, dy {:.2e}, dist {:.2e}",
            wind.u.get::<meter_per_second>(),
            wind.v.get::<meter_per_second>(),
            rel(dx_l, dx_u),
            rel(dy_l, dy_u),
            rel(dist_l, dist_u),
        );
        assert!(rel(dx_l, dx_u) < 1e-9, "dx: {dx_l} against {dx_u}");
        assert!(rel(dy_l, dy_u) < 1e-9, "dy: {dy_l} against {dy_u}");
        assert!(
            rel(dist_l, dist_u) < 1e-9,
            "distance: {dist_l} against {dist_u}"
        );
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
    ///
    /// **1.78 is this configuration's number, not a characteristic one.**
    /// Measured across the four Singapore monsoon conditions at two hours
    /// (2026-09-23), the ratio spans **1.04 to 3.97** — on both sides of 2. The
    /// assertion below therefore pins the bound `1 < ratio < 2` only for the
    /// specific case it constructs, and the general claim is that the ratio is
    /// geometry-dependent, not that it is under 2.
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
            "expected a ratio strictly between 1 and 2 FOR THIS CONFIGURATION \
             (the two puffs disperse differently); got {ratio}. Note the ratio \
             exceeds 2 in other geometries -- see the doc comment."
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
