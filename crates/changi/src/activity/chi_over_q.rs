// SPDX-License-Identifier: GPL-3.0

//! Unit-release dilution factors, binned by travel time.
//!
//! # Why a unit release
//!
//! A consequence calculation needs the same dispersion answer for every nuclide
//! in the source term, differing only in how much was released and how fast it
//! decays on the way. Running the puff train once per nuclide would recompute
//! identical geometry ten or more times.
//!
//! So it is run **once with unit mass** and the response scaled afterwards.
//! This is **exact, not an approximation**: the puff kernel is linear in mass
//! (pinned by `puff::concentration::tests::concentration_is_linear_in_puff_mass`)
//! and puffs superpose by summation, so
//!
//! ```text
//! chi(r; Q) = Q * chi(r; 1)
//! ```
//!
//! holds identically. It is the standard idiom in consequence codes.
//!
//! # Why binned by travel time, and why the binning is exact
//!
//! Decay in transit is **not** optional at these scales — Kr-89's half-life is
//! 189 s against a default puff lifetime of 1200 s, so ignoring it overstates
//! the far field by roughly 80x. But the surviving fraction depends on how long
//! a puff has been travelling, which differs puff by puff, so a single scalar
//! `chi/Q` cannot carry it.
//!
//! The fix is to keep the response resolved by puff **age**. A puff's age is
//! `elapsed - time_emitted`, and both are integer multiples of `sim_dt` —
//! `RunConfig` asserts that `puff_dt` is an integer multiple of `sim_dt`, which
//! is upstream's own documented requirement. So bin `a` carries travel time
//! *exactly* `a * sim_dt`, `exp(-lambda * a * sim_dt)` is exact, and nothing is
//! smeared across bins.
//!
//! The reported dilution factor for nuclide `n` released in segment `s` is then
//!
//! ```text
//! chi/Q (r, s, n) = sum over a of  bins[r][s][a] * exp(-lambda_n * a * sim_dt)
//! ```
//!
//! # Known truncation: puffs are dropped at `puff_duration`
//!
//! `RunConfig::puff_duration` (upstream default 1200 s) is a hard cutoff, not a
//! decay — a puff older than it simply stops existing. At 4 m/s that is 4800 m,
//! so a receptor at 10 km reads **exactly zero** with no error and no warning.
//! [`dilution_factors`] computes the reach and panics if *every* receptor is
//! beyond it, because an all-zero result is otherwise indistinguishable from a
//! correct answer. A mixed set is allowed through — check [`DilutionFactors::reach`].

use uom::si::f64::{Frequency, Length, MassRate, Time};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::time::second;

use super::units::DilutionFactor;
use crate::flexpart::decay::surviving_fraction;
use crate::puff::simulate::{
    emit_with_classes, emits_at, puff_unit_response, Puff, Receptor, RunConfig, Source,
};
use crate::puff::stability::{stability_class, StabilityClass, StabilitySet};
use crate::puff::wind::{wind_speed, WindComponents};

/// Where each puff's Pasquill stability class comes from.
///
/// Upstream derives the class *from* the wind speed and the hour, which means a
/// sweep cannot hold wind constant and vary stability — the very thing a
/// sensitivity study wants. [`Self::Fixed`] is the way to say it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StabilitySource {
    /// **Default.** Derive it as upstream does, from the wind speed at the
    /// moment of emission and [`RunConfig::start_hour`].
    ///
    /// Six of the ten wind-speed/day-night regimes are ambiguous between two
    /// adjacent classes. Whether the second one is also emitted is
    /// [`RunConfig::emission_policy`]'s decision, not this enum's — pass
    /// [`crate::puff::simulate::EmissionPolicy::UpstreamRecycleStabilityClasses`]
    /// to reproduce upstream's doubling.
    #[default]
    FromWind,
    /// Hold one class for the whole run, ignoring wind speed and hour.
    ///
    /// The set is then unambiguous, so [`RunConfig::emission_policy`] has
    /// nothing to recycle and both policies emit one puff per event.
    Fixed(StabilityClass),
}

impl StabilitySource {
    fn set(self, wind: WindComponents, start_hour: u32) -> StabilitySet {
        match self {
            Self::FromWind => stability_class(Some(wind_speed(wind)), start_hour),
            Self::Fixed(c) => StabilitySet::One(c),
        }
    }
}

/// Dilution factors for every receptor, release segment and travel-time bin.
///
/// Indexed `[receptor][segment][bin]`, each entry in **s/m^3 per unit activity
/// released by one source in that segment**. Multiple sources are summed into
/// the same entry, matching upstream's convention that one emission rate is
/// "applied uniformly to all sources" — so the activity a caller multiplies
/// back in is the release from **each** source, not the total over all of them.
///
/// Size is `n_receptors * n_segments * n_bins` f64s: for 20 receptors, 10
/// segments and 121 bins that is about 200 kB.
#[derive(Debug, Clone, PartialEq)]
pub struct DilutionFactors {
    values: Vec<f64>,
    n_receptors: usize,
    n_segments: usize,
    n_bins: usize,
    sim_dt_s: f64,
    reach_m: f64,
}

impl DilutionFactors {
    /// Number of receptors, in the order supplied.
    #[must_use]
    pub const fn n_receptors(&self) -> usize {
        self.n_receptors
    }

    /// Number of release segments, in the order the boundaries defined them.
    #[must_use]
    pub const fn n_segments(&self) -> usize {
        self.n_segments
    }

    /// Number of travel-time bins. Bin `a` is travel time `a * sim_dt`.
    #[must_use]
    pub const fn n_bins(&self) -> usize {
        self.n_bins
    }

    /// The travel time bin `a` represents — exactly `a * sim_dt`.
    #[must_use]
    pub fn travel_time(&self, bin: usize) -> Time {
        Time::new::<second>(bin as f64 * self.sim_dt_s)
    }

    /// How far a puff travels before `puff_duration` drops it, at the fastest
    /// wind in the run. Receptors beyond this read zero — see the module docs.
    #[must_use]
    pub fn reach(&self) -> Length {
        Length::new::<meter>(self.reach_m)
    }

    /// One bin's contribution, undecayed.
    ///
    /// # Panics
    /// Panics if any index is out of range.
    #[must_use]
    pub fn bin(&self, receptor: usize, segment: usize, bin: usize) -> DilutionFactor {
        DilutionFactor::new(self.values[self.index(receptor, segment, bin)])
    }

    /// The dilution factor for one receptor and segment, with decay in transit.
    ///
    /// Sums the bins weighted by `exp(-decay_constant * travel_time)`. Pass a
    /// zero `decay_constant` for a stable nuclide, which returns the bins' plain
    /// sum with no rounding penalty (the weight is exactly 1).
    ///
    /// The decay constant belongs to the nuclide and should come from
    /// `boon-lay`'s `TrisoAtopsNuclide::decay_constant()`, **not** from
    /// [`crate::flexpart::decay::decay_constant`] — the latter carries
    /// upstream FLEXPART's truncated `0.693147` rather than `ln 2`.
    ///
    /// # Panics
    /// Panics if `receptor` or `segment` is out of range, or if
    /// `decay_constant` is negative.
    #[must_use]
    pub fn dilution(
        &self,
        receptor: usize,
        segment: usize,
        decay_constant: Frequency,
    ) -> DilutionFactor {
        let lambda = decay_constant.get::<hertz>();
        assert!(
            lambda >= 0.0,
            "decay constant must be non-negative; got {lambda} Hz"
        );
        let mut total = 0.0;
        for a in 0..self.n_bins {
            let v = self.values[self.index(receptor, segment, a)];
            if v == 0.0 {
                continue;
            }
            total += v * surviving_fraction(lambda, a as f64 * self.sim_dt_s);
        }
        DilutionFactor::new(total)
    }

    fn index(&self, receptor: usize, segment: usize, bin: usize) -> usize {
        assert!(
            receptor < self.n_receptors && segment < self.n_segments && bin < self.n_bins,
            "index out of range: receptor {receptor}/{}, segment {segment}/{}, bin {bin}/{}",
            self.n_receptors,
            self.n_segments,
            self.n_bins
        );
        (receptor * self.n_segments + segment) * self.n_bins + bin
    }
}

/// Run the puff train once with unit mass and accumulate the binned response.
///
/// # Arguments
/// - `sources` — one or more release points. Every source emits the same unit
///   release, so the result is per unit released **by each source**.
/// - `segment_boundaries` — ascending times splitting the run into release
///   segments, first element `0` and last the run duration; `n + 1` boundaries
///   define `n` segments. A puff is attributed to the segment containing its
///   emission time. One segment covering the whole run is the simple case.
/// - `wind` — one sample per simulation step, as [`crate::puff::simulate`].
/// - `receptors` — where the dilution factor is wanted.
/// - `config` — timing; `emission_policy` still decides whether an ambiguous
///   stability class emits one puff or two.
/// - `stability` — see [`StabilitySource`].
///
/// # Panics
/// Panics if `sources`, `receptors` or the segment list is empty or malformed,
/// if `wind` is shorter than the number of steps, if `config` is inconsistent,
/// or if **every** receptor lies beyond [`DilutionFactors::reach`] (which would
/// silently return all zeros — see the module docs).
#[must_use]
pub fn dilution_factors(
    sources: &[Source],
    segment_boundaries: &[Time],
    wind: &[WindComponents],
    receptors: &[Receptor],
    config: &RunConfig,
    stability: StabilitySource,
) -> DilutionFactors {
    assert!(!sources.is_empty(), "need at least one source");
    assert!(!receptors.is_empty(), "need at least one receptor");
    assert!(
        segment_boundaries.len() >= 2,
        "need at least two segment boundaries to define one segment"
    );

    let sim_dt = config.sim_dt.get::<second>();
    let n_steps = (config.duration.get::<second>() / sim_dt).floor() as usize + 1;
    assert!(
        wind.len() >= n_steps,
        "need one wind sample per simulation step: {n_steps} steps, {} samples",
        wind.len()
    );

    let bounds: Vec<f64> = segment_boundaries
        .iter()
        .map(|t| t.get::<second>())
        .collect();
    assert!(
        bounds.windows(2).all(|w| w[1] > w[0]),
        "segment boundaries must be strictly ascending"
    );
    let n_segments = bounds.len() - 1;

    let max_age = config.puff_duration.get::<second>();
    let n_bins = (max_age / sim_dt).floor() as usize + 1;
    let n_receptors = receptors.len();

    // A unit puff: q_per_puff = emission_rate * puff_dt, so an emission rate of
    // 1/puff_dt gives exactly 1 kg per puff and the response reads directly in
    // m^-3 per kg.
    let unit_rate = MassRate::new::<kilogram_per_second>(1.0 / config.puff_dt.get::<second>());

    let mut values = vec![0.0_f64; n_receptors * n_segments * n_bins];
    // Mass emitted per segment by ONE source. Counted rather than derived, so
    // it follows whatever `emission_policy` and `stability` actually did —
    // including upstream's mass-doubling recycle.
    let mut emitted_kg = vec![0.0_f64; n_segments];

    let fastest = wind
        .iter()
        .take(n_steps)
        .map(|w| wind_speed(*w).get::<uom::si::velocity::meter_per_second>())
        .fold(0.0_f64, f64::max);
    let reach_m = fastest * max_age;

    for (source_index, source) in sources.iter().enumerate() {
        let mut live: Vec<Puff> = Vec::new();
        for step in 0..n_steps {
            let elapsed = (step as f64) * sim_dt;

            if emits_at(step, elapsed, config.puff_dt.get::<second>()) {
                let before = live.len();
                let set = stability.set(wind[step], config.start_hour);
                emit_with_classes(&mut live, elapsed, wind[step], set, config, unit_rate);
                if source_index == 0 {
                    // Every source emits identically, so count once.
                    if let Some(seg) = segment_of(&bounds, elapsed) {
                        for p in &live[before..] {
                            emitted_kg[seg] += p.mass_kg;
                        }
                    }
                }
            }
            live.retain(|p| elapsed - p.time_emitted_s <= max_age);
            if live.is_empty() {
                continue;
            }

            for p in &live {
                let Some(seg) = segment_of(&bounds, p.time_emitted_s) else {
                    continue;
                };
                let age = elapsed - p.time_emitted_s;
                let a = (age / sim_dt).round() as usize;
                if a >= n_bins {
                    // Only reachable if `puff_duration` is not a multiple of
                    // `sim_dt`, where the retain bound falls between bins.
                    continue;
                }
                for (r, receptor) in receptors.iter().enumerate() {
                    let response = puff_unit_response(p, *source, *receptor, elapsed);
                    if response == 0.0 {
                        continue;
                    }
                    values[(r * n_segments + seg) * n_bins + a] += response * sim_dt;
                }
            }
        }
    }

    // Normalise to a unit release: the accumulator holds kg.s/m^3 for however
    // much mass the segment actually emitted.
    for r in 0..n_receptors {
        for (seg, &mass) in emitted_kg.iter().enumerate() {
            if mass == 0.0 {
                continue;
            }
            for a in 0..n_bins {
                values[(r * n_segments + seg) * n_bins + a] /= mass;
            }
        }
    }

    let result = DilutionFactors {
        values,
        n_receptors,
        n_segments,
        n_bins,
        sim_dt_s: sim_dt,
        reach_m,
    };

    assert!(
        result.values.iter().any(|v| *v != 0.0),
        "every dilution factor is zero. The most likely cause is that all {n_receptors} \
         receptors lie beyond the puff reach of {reach_m:.0} m (puff_duration {max_age} s \
         at up to {fastest:.2} m/s), so every puff was dropped before arriving. \
         Raise RunConfig::puff_duration or move the receptors closer."
    );

    result
}

/// Which segment contains `t`. The last segment is closed at its upper end so a
/// puff emitted exactly at the run duration is not lost.
fn segment_of(bounds: &[f64], t: f64) -> Option<usize> {
    let last = bounds.len() - 2;
    for i in 0..=last {
        let upper_closed = i == last;
        if t >= bounds[i] && (t < bounds[i + 1] || (upper_closed && t <= bounds[i + 1])) {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::puff::concentration::METHANE_PPM_PER_KG_PER_M3;
    use crate::puff::simulate::{constant_wind, simulate_sensor_mode, EmissionPolicy};
    use uom::si::f64::{Mass, Velocity};
    use uom::si::mass::kilogram;
    use uom::si::velocity::meter_per_second;

    fn source() -> Source {
        Source {
            x: Length::new::<meter>(0.0),
            y: Length::new::<meter>(0.0),
            height: Length::new::<meter>(10.0),
        }
    }

    fn receptor_at(x: f64) -> Receptor {
        Receptor {
            x: Length::new::<meter>(x),
            y: Length::new::<meter>(0.0),
            z: Length::new::<meter>(2.0),
        }
    }

    fn config(duration_s: f64) -> RunConfig {
        RunConfig {
            sim_dt: Time::new::<second>(10.0),
            puff_dt: Time::new::<second>(10.0),
            output_dt: Time::new::<second>(10.0),
            duration: Time::new::<second>(duration_s),
            puff_duration: Time::new::<second>(600.0),
            start_hour: 12,
            emission_policy: EmissionPolicy::OnePuffPerEmission,
        }
    }

    fn wind_series(n: usize) -> Vec<WindComponents> {
        constant_wind(
            Velocity::new::<meter_per_second>(4.0),
            Velocity::new::<meter_per_second>(0.0),
            n,
        )
    }

    /// The two copies of the advection — `sum_over_puffs`'s and
    /// `puff_unit_response`'s — must agree. `simulate.rs` says so in the doc
    /// comment on `puff_unit_response`; this is the check behind that claim.
    ///
    /// It goes through `simulate_sensor_mode`, so it compares the **whole
    /// verified path** against the unit-response path, and it is the only tie
    /// this module has back to code that is checked against an upstream.
    ///
    /// The comparison is **exact**, not an engineering bound, and the two runs
    /// are set up so it can be:
    ///
    /// - `output_dt == sim_dt`, so upstream's left-closed `cut` puts exactly
    ///   one simulation step in each output interval and the reported interval
    ///   mean *is* that step's concentration.
    /// - The sensor run is one step longer than the dilution run. Upstream
    ///   drops its final timestamp (one fewer output row than output
    ///   timestamps), so the sensor series covers steps `0..n-1` — which is
    ///   precisely what a dilution run of duration `(n-1) * sim_dt` covers.
    ///   Matching the windows is what removes the need for a tolerance, rather
    ///   than absorbing the mismatch into one.
    /// - Both are normalised by the same emitted mass: one 1 kg puff per step.
    ///
    /// What is left is floating-point summation order and the ppm round trip,
    /// which is why the bound is `1e-12` relative and not zero.
    #[test]
    fn unit_response_agrees_with_the_ported_sum() {
        let sim_dt = 10.0;
        let n_shared_steps = 60_usize;

        // The ported path: one step longer, because its reduction drops the last.
        let mut sensor_cfg = config(n_shared_steps as f64 * sim_dt);
        sensor_cfg.output_dt = sensor_cfg.sim_dt;
        // The unit-response path: exactly the shared window.
        let dilution_cfg = config((n_shared_steps - 1) as f64 * sim_dt);

        let wind = wind_series(n_shared_steps + 1);
        let receptors: Vec<Receptor> = (1..=4).map(|k| receptor_at(100.0 * k as f64)).collect();

        let rate = MassRate::new::<kilogram_per_second>(1.0 / sensor_cfg.puff_dt.get::<second>());
        let series = simulate_sensor_mode(&[source()], rate, &wind, &receptors, &sensor_cfg);
        assert_eq!(
            series.concentrations.len(),
            n_shared_steps,
            "output_dt == sim_dt should give one row per step, less the dropped final one"
        );

        let factors = dilution_factors(
            &[source()],
            &[Time::new::<second>(0.0), dilution_cfg.duration],
            &wind,
            &receptors,
            &dilution_cfg,
            StabilitySource::FromWind,
        );

        // Summed over receptors, both are the time integral of concentration
        // per kilogram released.
        let from_bins: f64 = (0..receptors.len())
            .map(|r| {
                factors
                    .dilution(r, 0, Frequency::new::<hertz>(0.0))
                    .seconds_per_cubic_meter()
            })
            .sum();

        let mut from_port = 0.0;
        for row in &series.concentrations {
            for v in row {
                from_port += v / METHANE_PPM_PER_KG_PER_M3 * sim_dt;
            }
        }
        from_port /= n_shared_steps as f64;

        assert!(from_port > 0.0, "the ported path should see something");
        let rel = (from_bins - from_port).abs() / from_port;
        assert!(
            rel < 1e-12,
            "unit-response integral {from_bins:e} vs ported {from_port:e}, \
             relative difference {rel:e}"
        );
    }

    /// The kernel is linear in mass, so the unit response times a mass must
    /// equal the kernel evaluated at that mass. This is what makes scaling per
    /// nuclide exact rather than approximate.
    #[test]
    fn the_unit_response_scales_exactly_with_mass() {
        use crate::puff::concentration::gaussian_puff_concentration;
        use uom::si::mass_density::kilogram_per_cubic_meter;

        let class = StabilityClass::D;
        let mass = Mass::new::<kilogram>(37.0);
        let direct = gaussian_puff_concentration(
            mass,
            class,
            Length::new::<meter>(400.0),
            Length::new::<meter>(0.0),
            Length::new::<meter>(10.0),
            (
                Length::new::<meter>(420.0),
                Length::new::<meter>(0.0),
                Length::new::<meter>(2.0),
            ),
            Length::new::<meter>(400.0),
        )
        .get::<kilogram_per_cubic_meter>();
        let unit = gaussian_puff_concentration(
            Mass::new::<kilogram>(1.0),
            class,
            Length::new::<meter>(400.0),
            Length::new::<meter>(0.0),
            Length::new::<meter>(10.0),
            (
                Length::new::<meter>(420.0),
                Length::new::<meter>(0.0),
                Length::new::<meter>(2.0),
            ),
            Length::new::<meter>(400.0),
        )
        .get::<kilogram_per_cubic_meter>();
        assert!((direct - unit * 37.0).abs() <= 8.0 * f64::EPSILON * direct.abs());
    }

    /// Bin `a` must carry travel time exactly `a * sim_dt` — the property the
    /// whole decay treatment rests on.
    #[test]
    fn bins_carry_exact_travel_time() {
        let cfg = config(300.0);
        let factors = dilution_factors(
            &[source()],
            &[Time::new::<second>(0.0), cfg.duration],
            &wind_series(31),
            &[receptor_at(200.0)],
            &cfg,
            StabilitySource::FromWind,
        );
        for a in 0..factors.n_bins() {
            let expected = a as f64 * 10.0;
            assert_eq!(factors.travel_time(a).get::<second>(), expected);
        }
    }

    /// A zero decay constant must reproduce the plain sum of the bins, exactly.
    #[test]
    fn a_stable_nuclide_is_the_undecayed_sum() {
        let cfg = config(300.0);
        let factors = dilution_factors(
            &[source()],
            &[Time::new::<second>(0.0), cfg.duration],
            &wind_series(31),
            &[receptor_at(200.0)],
            &cfg,
            StabilitySource::FromWind,
        );
        let mut plain = 0.0;
        for a in 0..factors.n_bins() {
            plain += factors.bin(0, 0, a).seconds_per_cubic_meter();
        }
        let decayed = factors
            .dilution(0, 0, Frequency::new::<hertz>(0.0))
            .seconds_per_cubic_meter();
        assert_eq!(plain, decayed);
        assert!(plain > 0.0, "the receptor should see something");
    }

    /// Decay must weight each bin by exactly `exp(-lambda * a * sim_dt)`.
    #[test]
    fn decay_weights_each_bin_by_its_own_exact_travel_time() {
        let cfg = config(300.0);
        let factors = dilution_factors(
            &[source()],
            &[Time::new::<second>(0.0), cfg.duration],
            &wind_series(31),
            &[receptor_at(200.0)],
            &cfg,
            StabilitySource::FromWind,
        );
        // Kr-89-like: 189 s half-life.
        let lambda = core::f64::consts::LN_2 / 189.0;
        let mut expected = 0.0;
        for a in 0..factors.n_bins() {
            let v = factors.bin(0, 0, a).seconds_per_cubic_meter();
            expected += v * (-lambda * a as f64 * 10.0).exp();
        }
        let got = factors
            .dilution(0, 0, Frequency::new::<hertz>(lambda))
            .seconds_per_cubic_meter();
        assert!((got - expected).abs() <= 1e-12 * expected.abs());
        assert!(
            got < expected * 1.0 + f64::EPSILON,
            "decay can only reduce the answer"
        );
    }

    /// Splitting one release window into N equal segments must leave the total
    /// unchanged: segment attribution is bookkeeping, not physics.
    #[test]
    fn splitting_a_window_into_equal_segments_conserves_the_total() {
        let cfg = config(600.0);
        let wind = wind_series(61);
        let receptors = [receptor_at(300.0)];

        let one = dilution_factors(
            &[source()],
            &[Time::new::<second>(0.0), cfg.duration],
            &wind,
            &receptors,
            &cfg,
            StabilitySource::Fixed(StabilityClass::D),
        );
        let three = dilution_factors(
            &[source()],
            &[
                Time::new::<second>(0.0),
                Time::new::<second>(200.0),
                Time::new::<second>(400.0),
                Time::new::<second>(600.0),
            ],
            &wind,
            &receptors,
            &cfg,
            StabilitySource::Fixed(StabilityClass::D),
        );

        let lumped = one
            .dilution(0, 0, Frequency::new::<hertz>(0.0))
            .seconds_per_cubic_meter();
        // Each segment is normalised per unit release *by that segment*, so the
        // equivalent of the lumped factor is the mass-weighted mean. The three
        // segments emit equal mass here, so it is the plain mean.
        let split: f64 = (0..3)
            .map(|s| {
                three
                    .dilution(0, s, Frequency::new::<hertz>(0.0))
                    .seconds_per_cubic_meter()
            })
            .sum::<f64>()
            / 3.0;
        let rel = (lumped - split).abs() / lumped;
        assert!(
            rel < 0.02,
            "lumped {lumped:e} vs split mean {split:e}, rel {rel:e}"
        );
    }

    /// `Fixed` must actually override the wind-derived class, and a different
    /// class must give a different answer — otherwise the knob does nothing.
    #[test]
    fn a_fixed_stability_class_overrides_the_wind_derived_one() {
        let cfg = config(300.0);
        let wind = wind_series(31);
        let receptors = [receptor_at(500.0)];
        let bounds = [Time::new::<second>(0.0), cfg.duration];

        let unstable = dilution_factors(
            &[source()],
            &bounds,
            &wind,
            &receptors,
            &cfg,
            StabilitySource::Fixed(StabilityClass::A),
        )
        .dilution(0, 0, Frequency::new::<hertz>(0.0))
        .seconds_per_cubic_meter();
        let stable = dilution_factors(
            &[source()],
            &bounds,
            &wind,
            &receptors,
            &cfg,
            StabilitySource::Fixed(StabilityClass::F),
        )
        .dilution(0, 0, Frequency::new::<hertz>(0.0))
        .seconds_per_cubic_meter();

        assert!(unstable > 0.0 && stable > 0.0);
        assert!(
            stable > unstable,
            "class F disperses less than class A, so it must concentrate more at \
             a receptor on the plume centreline: F {stable:e}, A {unstable:e}"
        );
    }

    /// An all-zero result is the module's worst failure mode, so it must not be
    /// returned silently.
    #[test]
    #[should_panic(expected = "beyond the puff reach")]
    fn every_receptor_beyond_reach_panics_rather_than_returning_zeros() {
        let cfg = config(300.0);
        let _ = dilution_factors(
            &[source()],
            &[Time::new::<second>(0.0), cfg.duration],
            &wind_series(31),
            // 4 m/s * 600 s = 2400 m of reach.
            &[receptor_at(50_000.0)],
            &cfg,
            StabilitySource::FromWind,
        );
    }

    #[test]
    fn segment_lookup_closes_the_last_interval_at_its_upper_end() {
        let bounds = [0.0, 10.0, 20.0];
        assert_eq!(segment_of(&bounds, 0.0), Some(0));
        assert_eq!(segment_of(&bounds, 9.999), Some(0));
        assert_eq!(segment_of(&bounds, 10.0), Some(1));
        assert_eq!(segment_of(&bounds, 20.0), Some(1));
        assert_eq!(segment_of(&bounds, 20.001), None);
        assert_eq!(segment_of(&bounds, -0.001), None);
    }
}
