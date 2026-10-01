//! **Indicative accumulated dose** \[Sv\] at the 24 dispersion receptors: the
//! time integral of [`super::dose_rate`]'s indicative effective dose rate over
//! plant time, for the Map tab's "TEDE vs downwind distance" graph
//! (maintainer request, 2026-10-01, gh:#470).
//!
//! # SCOPE — binding, do not soften
//!
//! **Indicative, research and education only. Not a dose to any real person,
//! not a TEDE in the regulatory sense, and not for emergency, regulatory,
//! occupational or medical use** (`RESPONSIBLE_USE.md`,
//! `crates/buangkok/CLAUDE.md`). Everything [`super::dose_rate`]'s module doc
//! says about its source and coefficients carries over unchanged: the source
//! is five nuclides leaked at ~1 %/day from the primary circuit, with no
//! building, filter or stack credit, and is not a source term.
//!
//! # What is integrated
//!
//! `D_i = ∫ ḋ_i(t) dt` per receptor `i`, per pathway, with `ḋ` the receptor's
//! indicative dose rate exactly as the Map tab's dose-rate table computes it
//! (same buangkok calls through [`super::dose_rate::receptor_split`]):
//!
//! | Pathway | Kind | In the integral |
//! |---|---|---|
//! | cloud submersion (FGR-15, semi-infinite cloud) | external | yes |
//! | inhalation, **committed** (FGR-11, adult) | internal (committed) | yes, minus Kr-85/Xe-133 (no FGR-11 coefficient: missing, not zero) |
//! | ground shine (FGR-15) from the dry deposit | external | yes |
//! | ingestion | internal | **no** — no food-chain model anywhere in the simulator |
//! | resuspension inhalation, skin/clothing contamination | — | **no** |
//!
//! "TEDE" strictly is external (deep) dose plus committed internal dose. The
//! three pathways above are the parts this simulator has; ingestion is absent,
//! so the integral **under-states** a TEDE from that omission. The inhalation
//! term mixes ICRP 26/30 (FGR-11) with ICRP 103 (FGR-15) weighting, as the dose
//! rate it integrates already does.
//!
//! # Integration rule: zero-order hold over plant time
//!
//! Each plant step adds `ḋ x dt`, with `ḋ` the receptor rate **as published at
//! the end of that step** (the dispersion channel's latest receptor sample).
//! That sample refreshes on the channel's 2 s physics throttle, and also at the
//! map's 10 Hz cadence when a map is on screen, so the hold is at most one
//! throttle interval long. A GUI run and a headless run therefore integrate the
//! same rate history sampled at different cadences and need not agree to the
//! last digit; each is deterministic on its own (no wall clock enters the
//! integral -- `dt` is plant time).
//!
//! The clock is the **plant** clock. The map's plume fast-forward moves the
//! plume clock ahead of the plant clock; the sample then reflects the
//! fast-forwarded plume, but the dose still accrues per second of plant time.
//!
//! A rate that is not finite (absolute release arm unavailable) adds nothing,
//! and its duration is counted in [`TedeAccumulator::unavailable_s`] so the
//! graph can say the integral is partial. Never added as zero silently.
//!
//! # Windows
//!
//! - **Since plant start** (`t = 0`, the plant's construction; a restarted
//!   simulation builds a fresh plant, and [`TedeAccumulator::reset`] zeroes it).
//! - **Trailing ~96 h**, for comparison with the NRC 2023 EPZ sizing figure
//!   (10 mSv TEDE over 96 h -- a reference, not a threshold for this demo):
//!   one-hour buckets, the current plus the 95 before it, so the window spans
//!   **between 95 h and 96 h, never more than 96 h**. Identical to the
//!   since-start value until the plant has run 95 h.
//!
//! Per `RESPONSIBLE_USE.md`, AI-assisted draft pending human review.

use super::atmospheric_dispersion::{RECEPTOR_COUNT, ReceptorResult};
use super::dose_rate::{self, Pathway};

/// Width of one trailing-window bucket \[s\] (1 h).
pub const WINDOW_BUCKET_S: f64 = 3600.0;

/// Buckets in the trailing window, the current one included: 96, so the
/// window covers 95 to 96 h (see the module doc).
pub const WINDOW_BUCKETS: usize = 96;

/// µSv/h x s -> Sv: `1e-6 Sv/µSv / 3600 s/h`. A unit conversion only.
const SV_PER_USV_PER_H_SECOND: f64 = 1.0e-6 / 3600.0;

/// The per-receptor dose integrals. Owned by the plant, advanced once per plant
/// step, published to the snapshot.
#[derive(Clone, Debug)]
pub struct TedeAccumulator {
    /// Plant time integrated so far \[s\].
    elapsed_s: f64,
    /// Per receptor, plant time during which its rate was not finite and so
    /// added nothing \[s\].
    unavailable_s: [f64; RECEPTOR_COUNT],
    /// Per receptor, per pathway ([`Pathway::index`] order), dose since plant
    /// start \[Sv\].
    since_start_sv: [[f64; 3]; RECEPTOR_COUNT],
    /// Per receptor, the all-pathway dose in each hour bucket \[Sv\], a ring
    /// indexed by `hour % WINDOW_BUCKETS`.
    window_sv: [[f64; WINDOW_BUCKETS]; RECEPTOR_COUNT],
    /// The hour index (`floor(t / 1 h)`) of the newest bucket in the ring.
    current_hour: u64,
}

impl Default for TedeAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

/// The accumulator's published state, one entry per receptor in the
/// dispersion channel's order (distance-major, then sector) -- the same order
/// as `HtgrSnapshot::receptors`, whose distance and bearing it is read with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TedeSnapshot {
    /// Plant time integrated \[s\] (since plant start).
    pub elapsed_s: f64,
    /// Per receptor, per pathway ([`Pathway::index`] order) dose since plant
    /// start \[Sv\].
    pub since_start_sv_by_pathway: [[f64; 3]; RECEPTOR_COUNT],
    /// Per receptor, all-pathway dose over the trailing 95–96 h \[Sv\].
    pub trailing_96h_sv: [f64; RECEPTOR_COUNT],
    /// The longest time any receptor's rate was unavailable \[s\]; non-zero
    /// means the integrals are partial.
    pub unavailable_s: f64,
}

impl Default for TedeSnapshot {
    fn default() -> Self {
        Self {
            elapsed_s: 0.0,
            since_start_sv_by_pathway: [[0.0; 3]; RECEPTOR_COUNT],
            trailing_96h_sv: [0.0; RECEPTOR_COUNT],
            unavailable_s: 0.0,
        }
    }
}

impl TedeSnapshot {
    /// Receptor `i`'s all-pathway dose since plant start \[Sv\].
    pub fn since_start_sv(&self, i: usize) -> f64 {
        self.since_start_sv_by_pathway[i].iter().sum()
    }
}

/// One receptor's indicative dose rate per pathway \[µSv/h\], in
/// [`Pathway::index`] order: the same buangkok split the Map tab's dose-rate
/// table shows, summed over the nuclides whose coefficient exists. `NAN` in a
/// slot whose input is not finite.
pub fn receptor_rates_usv_per_h(r: &ReceptorResult) -> [f64; 3] {
    let split = dose_rate::receptor_split(
        1.0,
        &r.instantaneous_air_bq_per_m3_by_nuclide,
        &r.ground_bq_per_m2_absolute_by_nuclide,
        dose_rate::coefficients(),
    );
    Pathway::ALL.map(|p| dose_rate::pathway_total(&split, p).0)
}

impl TedeAccumulator {
    /// All integrals zero at `t = 0`.
    pub fn new() -> Self {
        Self {
            elapsed_s: 0.0,
            unavailable_s: [0.0; RECEPTOR_COUNT],
            since_start_sv: [[0.0; 3]; RECEPTOR_COUNT],
            window_sv: [[0.0; WINDOW_BUCKETS]; RECEPTOR_COUNT],
            current_hour: 0,
        }
    }

    /// Zero every integral and the clock.
    ///
    /// The simulator has no "reset plant" control: **Restart simulation**
    /// builds a fresh `HtgrPlant`, whose accumulator starts at zero through
    /// [`Self::new`]. This is the in-place equivalent, for a future reset
    /// control and for the tests.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Advance by one plant step of `dt_s` seconds at the dispersion channel's
    /// latest receptor sample. `None` (no dispersion evaluation yet) means no
    /// air or deposit has been computed anywhere, so the rate is zero.
    pub fn accumulate(&mut self, dt_s: f64, receptors: Option<&[ReceptorResult]>) {
        let mut rates = [[0.0; 3]; RECEPTOR_COUNT];
        if let Some(rs) = receptors {
            for (slot, r) in rates.iter_mut().zip(rs.iter()) {
                *slot = receptor_rates_usv_per_h(r);
            }
        }
        self.accumulate_rates(dt_s, &rates);
    }

    /// The integration core: add `rates_usv_per_h[i][p] x dt_s` (zero-order
    /// hold) to receptor `i`, pathway `p`. Separated from [`Self::accumulate`]
    /// so a test can drive it with a known rate.
    pub fn accumulate_rates(&mut self, dt_s: f64, rates_usv_per_h: &[[f64; 3]; RECEPTOR_COUNT]) {
        if !(dt_s > 0.0) {
            return;
        }
        // The step is filed under the hour containing its midpoint (error at
        // most one 0.1 s step per hour boundary, on the window only).
        let hour = ((self.elapsed_s + 0.5 * dt_s) / WINDOW_BUCKET_S).floor() as u64;
        if hour > self.current_hour {
            // Clear the buckets the clock has moved into, at most the whole ring.
            let advanced = (hour - self.current_hour).min(WINDOW_BUCKETS as u64);
            for k in 1..=advanced {
                let slot = ((self.current_hour + k) % WINDOW_BUCKETS as u64) as usize;
                for ring in self.window_sv.iter_mut() {
                    ring[slot] = 0.0;
                }
            }
            self.current_hour = hour;
        }
        let slot = (hour % WINDOW_BUCKETS as u64) as usize;
        for i in 0..RECEPTOR_COUNT {
            let mut any_missing = false;
            for p in 0..3 {
                let rate = rates_usv_per_h[i][p];
                if rate.is_finite() {
                    let d = rate * dt_s * SV_PER_USV_PER_H_SECOND;
                    self.since_start_sv[i][p] += d;
                    self.window_sv[i][slot] += d;
                } else {
                    any_missing = true;
                }
            }
            if any_missing {
                self.unavailable_s[i] += dt_s;
            }
        }
        self.elapsed_s += dt_s;
    }

    /// Plant time integrated so far \[s\].
    #[cfg_attr(not(test), allow(dead_code))] // read by the tests; the GUI reads the snapshot
    pub fn elapsed_s(&self) -> f64 {
        self.elapsed_s
    }

    /// Per receptor, per pathway dose since plant start \[Sv\].
    #[cfg_attr(not(test), allow(dead_code))] // read by the tests; the GUI reads the snapshot
    pub fn since_start_sv(&self) -> &[[f64; 3]; RECEPTOR_COUNT] {
        &self.since_start_sv
    }

    /// Per receptor, all-pathway dose over the trailing 95–96 h \[Sv\].
    pub fn trailing_window_sv(&self) -> [f64; RECEPTOR_COUNT] {
        self.window_sv.map(|ring| ring.iter().sum())
    }

    /// The longest time any receptor's rate was unavailable \[s\].
    pub fn unavailable_s(&self) -> f64 {
        self.unavailable_s.iter().copied().fold(0.0, f64::max)
    }

    /// Publish onto the shared snapshot.
    pub fn write_snapshot(&self, s: &mut crate::app::state::HtgrSnapshot) {
        s.tede = self.snapshot();
    }

    /// The published state.
    pub fn snapshot(&self) -> TedeSnapshot {
        TedeSnapshot {
            elapsed_s: self.elapsed_s,
            since_start_sv_by_pathway: self.since_start_sv,
            trailing_96h_sv: self.trailing_window_sv(),
            unavailable_s: self.unavailable_s(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uniform(rate: [f64; 3]) -> [[f64; 3]; RECEPTOR_COUNT] {
        [rate; RECEPTOR_COUNT]
    }

    /// **A constant rate r held for time T integrates to r·T**, per pathway,
    /// on both windows while T < 95 h.
    ///
    /// Methodology: 36 000 steps of 0.1 s (1 h of plant time, the plant's
    /// timestep) at r = (2, 3, 5) µSv/h; expected `r x 1 h` = (2, 3, 5) µSv =
    /// (2e-6, 3e-6, 5e-6) Sv, total 1e-5 Sv. Pass: relative error < 1e-9
    /// (summation round-off only).
    ///
    /// Result (2026-10-01): passes; the error is round-off at the 1e-12 level.
    #[test]
    fn a_constant_rate_integrates_to_rate_times_time() {
        let mut acc = TedeAccumulator::new();
        let rate = [2.0, 3.0, 5.0];
        for _ in 0..36_000 {
            acc.accumulate_rates(0.1, &uniform(rate));
        }
        assert!((acc.elapsed_s() - 3600.0).abs() < 1e-6);
        for i in 0..RECEPTOR_COUNT {
            for p in 0..3 {
                let want = rate[p] * 1.0e-6;
                let got = acc.since_start_sv()[i][p];
                assert!(
                    ((got - want) / want).abs() < 1e-9,
                    "{i} {p}: {got} vs {want}"
                );
            }
            let window = acc.trailing_window_sv()[i];
            assert!(((window - 1.0e-5) / 1.0e-5).abs() < 1e-9);
        }
        assert_eq!(acc.unavailable_s(), 0.0);
    }

    /// **Reset zeroes everything**, clock included.
    #[test]
    fn reset_zeroes_the_integrals() {
        let mut acc = TedeAccumulator::new();
        for _ in 0..100 {
            acc.accumulate_rates(0.1, &uniform([1.0, f64::NAN, 1.0]));
        }
        assert!(acc.since_start_sv()[0][0] > 0.0 && acc.unavailable_s() > 0.0);
        acc.reset();
        assert_eq!(acc.elapsed_s(), 0.0);
        assert_eq!(acc.unavailable_s(), 0.0);
        assert!(acc.since_start_sv().iter().flatten().all(|d| *d == 0.0));
        assert!(acc.trailing_window_sv().iter().all(|d| *d == 0.0));
    }

    /// **Deterministic**: the same rate history gives bit-identical integrals.
    #[test]
    fn the_integral_is_deterministic() {
        let run = || {
            let mut acc = TedeAccumulator::new();
            for k in 0..5_000 {
                let r = 1.0 + (k as f64 * 0.37).sin().abs();
                acc.accumulate_rates(0.1, &uniform([r, 0.5 * r, 0.1 * r]));
            }
            (
                acc.since_start_sv().to_vec(),
                acc.trailing_window_sv().to_vec(),
            )
        };
        let (a, b) = (run(), run());
        assert_eq!(
            a.0.iter()
                .flatten()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            b.0.iter()
                .flatten()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            a.1.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            b.1.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
    }

    /// **A non-finite rate adds nothing and is counted**, never added as zero
    /// silently; the finite pathways of the same receptor still accrue.
    #[test]
    fn an_unavailable_rate_is_counted_not_added() {
        let mut acc = TedeAccumulator::new();
        for _ in 0..10 {
            acc.accumulate_rates(1.0, &uniform([3600.0, f64::NAN, 0.0]));
        }
        assert!((acc.unavailable_s() - 10.0).abs() < 1e-12);
        assert!((acc.since_start_sv()[0][0] - 1.0e-5).abs() < 1e-15);
        assert_eq!(acc.since_start_sv()[0][1], 0.0);
    }

    /// **The trailing window drops old hours.** 100 h at 1 µSv/h: since-start
    /// is 100 µSv; the window holds the current hour plus the 95 before it.
    /// With steps of exactly 1 h landing whole in buckets, that is 96 µSv
    /// (96 complete hours: the window's upper bound, never more).
    #[test]
    fn the_trailing_window_is_at_most_96_hours() {
        let mut acc = TedeAccumulator::new();
        for _ in 0..100 {
            acc.accumulate_rates(3600.0, &uniform([1.0, 0.0, 0.0]));
        }
        assert!((acc.since_start_sv()[0][0] - 100.0e-6).abs() < 1e-15);
        let w = acc.trailing_window_sv()[0];
        assert!((w - 96.0e-6).abs() < 1e-15, "window {w}");
    }

    /// **Headless, through the real plant**: stepping `HtgrPlant` (no GUI, no
    /// thread, no map, so no wall clock) advances the published integral by
    /// exactly the plant time stepped, keeps every value finite and
    /// non-negative, and gives bit-identical snapshots on two runs.
    ///
    /// A harness check, not physics V&V: it pins the wiring and determinism,
    /// not the dose.
    #[test]
    fn the_plant_publishes_a_deterministic_integral() {
        use crate::app::state::HtgrSnapshot;
        use crate::physics::{HtgrPlant, PlantCommands, plant_timestep};
        use uom::si::time::second;
        let run = || {
            let mut plant = HtgrPlant::new();
            for _ in 0..40 {
                plant.step(plant_timestep(), PlantCommands::default());
            }
            let mut s = HtgrSnapshot::default();
            plant.write_snapshot(&mut s);
            s.tede
        };
        let (a, b) = (run(), run());
        let want = 40.0 * plant_timestep().get::<second>();
        assert!(
            (a.elapsed_s - want).abs() < 1e-9,
            "{} vs {want}",
            a.elapsed_s
        );
        assert!(a
            .since_start_sv_by_pathway
            .iter()
            .flatten()
            .all(|d| d.is_finite() && *d >= 0.0));
        assert!(a.trailing_96h_sv.iter().all(|d| d.is_finite() && *d >= 0.0));
        let bits = |t: &TedeSnapshot| -> Vec<u64> {
            t.since_start_sv_by_pathway
                .iter()
                .flatten()
                .chain(t.trailing_96h_sv.iter())
                .map(|v| v.to_bits())
                .collect()
        };
        assert_eq!(bits(&a), bits(&b));
    }

    /// **The rate is the dose-rate table's**: a receptor sample goes through
    /// the same buangkok split ([`dose_rate::receptor_split`]), so the
    /// submersion + inhalation pair equals the map's air factor
    /// ([`dose_rate::air_dose_rate_per_unit_chi_over_q`]) on the same air
    /// concentrations, to round-off.
    #[test]
    fn the_rate_is_the_map_tables_rate() {
        let air: [f64; dose_rate::N] = [7.0e-3, 2.0e-3, 3.0e-4, 1.0e-4, 5.0e-5];
        let r = ReceptorResult {
            bearing_deg: 0.0,
            distance_m: 100.0,
            chi_over_q: 0.0,
            instantaneous_chi_over_q: 0.0,
            instantaneous_air_bq_per_m3_by_nuclide: air,
            air_bq_s_per_m3: 0.0,
            ground_bq_per_m2: 0.0,
            air_bq_s_per_m3_absolute: None,
            ground_bq_per_m2_absolute: None,
            ground_bq_per_m2_absolute_by_nuclide: [1.0; dose_rate::N],
        };
        let rates = receptor_rates_usv_per_h(&r);
        let map_air = dose_rate::air_dose_rate_per_unit_chi_over_q(&air, dose_rate::coefficients());
        let ours = rates[Pathway::Submersion.index()] + rates[Pathway::Inhalation.index()];
        assert!(((ours - map_air) / map_air).abs() < 1e-12);
        assert!(rates[Pathway::GroundShine.index()] > 0.0);
    }
}
