//! Gaussian puff atmospheric dispersion, driven by the TRISO release channel.
//!
//! # The chain this closes
//!
//! ```text
//! kernel temperature -> TRISO-ATOPS release -> circulating activity
//!   -> primary-circuit leak -> Gaussian puff -> chi/Q at a receptor
//! ```
//!
//! [`super::fission_product_release`] ends at the *primary circuit*: activity
//! circulating in the helium, plated out on duct walls, held in graphite. This
//! module takes the circulating pool the rest of the way — out of the circuit,
//! into the atmosphere, and downwind — using `changi`'s
//! [`changi::activity`] layer, which drives the **Gaussian puff** model
//! ([`changi::puff`], a port of the R package `puff` 0.1.1) and `changi`'s own
//! decay-in-transit.
//!
//! Note which model runs, because `changi` carries two and asks callers to say:
//! **this is the Gaussian puff model, not FLEXPART.**
//!
//! # SCOPE LIMIT — binding, carried forward from `changi`, do not soften
//!
//! `changi::activity`'s module doc states this and it binds here unchanged:
//! research, education and V&V only. **Nothing here may be described — in
//! code, docs, commit messages or chat — as supporting emergency planning,
//! emergency response, dose assessment for real populations, or Level 3 PSA.**
//!
//! This module computes **no dose quantity of any kind** and none is planned.
//! `RESPONSIBLE_USE.md` already forbids this simulator being used for
//! safety-critical decisions, emergency response or safeguards analysis; a
//! dispersion calculation is exactly the kind of output that invites being
//! quoted past its scope, so the limit is restated here at the point of use.
//!
//! # What is real here, and what is an input
//!
//! This distinction decides which numbers may be quoted, so it comes first.
//!
//! **Real, and worth quoting:** the **dilution factor `chi/Q`** \[s/m^3\]. It
//! is a property of the geometry, the wind and the stability class *alone* —
//! it does not depend on the source magnitude at all. So every uncertainty
//! below about inventories and leak rates **cancels out of `chi/Q` entirely**,
//! and it is the one output of this chain that is not hostage to an input this
//! simulator does not have. It is what [`DispersionResult::chi_over_q`]
//! reports and what the Map tab draws.
//!
//! **An input, and NOT quotable as an HTR-10 figure:**
//!
//! - the **primary-circuit leak rate** ([`Htr10SiteInputs::LEAK_FRACTION_PER_S`])
//!   — a containment performance figure this simulator does not model;
//! - the **core inventory**, which [`super::fission_product_release`] refuses
//!   to derive, so everything downstream of it stays on that module's
//!   per-curie-of-core-inventory basis;
//! - the **release height** and the **deposition velocities**, both
//!   order-of-magnitude placeholders that `changi` itself labels as such.
//!
//! Air concentration and ground deposition are therefore reported **per curie
//! of core inventory, per unit leak fraction** — a transfer function, not a
//! consequence. They are not curies per cubic metre at HTR-10 and must never
//! be read as such.
//!
//! # NOT VALIDATED
//!
//! `changi::puff` is verified **code-to-code** against the upstream R at commit
//! `5213d58`, which establishes the translation is faithful and says nothing
//! about whether the model reproduces measured dispersion. `changi::activity`
//! is **not a port at all** and has no upstream to check against — its own
//! module doc says the strongest evidence it carries is internal consistency.
//! Upstream's Pasquill-Gifford sigmas were fitted for **methane leak
//! detection** over roughly 0.1-10 km; the dispersion mathematics is
//! species-independent but the fit range is not a statement about radionuclide
//! transport.
//!
//! Per `RESPONSIBLE_USE.md`, AI-assisted draft pending human review.

use changi::activity::chi_over_q::{dilution_factors, DilutionFactors, StabilitySource};
use changi::activity::deposition::DepositionGroup;
use changi::activity::source::{NuclideRelease, ReleaseWindow, SourceTerm};
use changi::activity::survey::{survey, DepositionVelocities, SiteSurvey};
use changi::puff::dispersion::pasquill_gifford_sigmas;
use changi::puff::simulate::{constant_wind, EmissionPolicy, Receptor, RunConfig, Source};
use changi::puff::stability::StabilityClass;
use changi::puff::wind::{wind_vector_convert, WindComponents};

use uom::si::angle::degree;
use uom::si::f64::{Angle, Length, Radioactivity, Time, Velocity};
use uom::si::length::meter;
use uom::si::radioactivity::curie;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

use super::fission_product_release::TrisoAtopsReleaseChannel;

/// Compass sectors the receptor ring is laid out on.
///
/// Eight is the coarsest layout that still shows the plume as a *direction*
/// rather than a number: with four, a wind between two sectors puts the plume
/// centreline exactly on a boundary and the map reads as two equal lobes, which
/// is a drawing artefact rather than dispersion. Sixteen would be finer but
/// costs twice the puff evaluations for a picture the same width.
pub const RECEPTOR_SECTORS: usize = 8;

/// Downwind distances the receptor ring is evaluated at \[m\].
///
/// Chosen to sit inside the range upstream's Pasquill-Gifford fit covers
/// (roughly 0.1-10 km, see the module doc). The innermost ring at 100 m is at
/// the bottom of that range and is the least trustworthy of the three; it is
/// kept because it is the one a site boundary would be near, and it is
/// labelled rather than dropped.
pub const RECEPTOR_DISTANCES_M: [f64; 3] = [100.0, 500.0, 1000.0];

/// Cells across the dispersion grid, per side, **when nothing asks for a
/// different number**.
///
/// The Map tab does ask: it requests one cell per **screen pixel** of the map
/// square (maintainer, 2026-09-25 -- "fill the map with single pixels, not the
/// big boxes"), through [`MapFieldRequest::cells`], and this is only the
/// opening value before a map has reported its size. See [`max_grid_cells`]
/// for the ceiling that request is clamped to and why the ceiling depends on
/// whether the WGSL path is live.
///
/// ~~64 gives a 5 px cell on a ~320 px map, which is what the maintainer asked
/// for (2026-09-24).~~ **SUPERSEDED 2026-09-25** -- a 5 px cell is exactly the
/// "big box" the current direction replaces. The old value survives as the
/// floor ([`MIN_GRID_CELLS`]), because a map that has not yet measured itself
/// still has to draw something.
///
/// # This is an EVALUATED field, not a contour plot
///
/// The rose's doc rejects a contour plot because it would interpolate between
/// the receptors -- "a picture of a plume rather than a readout of one". A
/// grid does not have that problem: **every cell is a real evaluation of the
/// same puff model at that cell's own coordinates**, with nothing drawn
/// between them. The objection was to interpolation, not to resolution.
///
/// At one cell per pixel the picture is a readout at every pixel it paints,
/// which is the strongest form of that argument rather than a departure from
/// it: nothing on screen is interpolated, because there is no gap left to
/// interpolate across.
pub const DEFAULT_GRID_CELLS: usize = 64;

/// Floor on the grid resolution, per side.
///
/// A map that has not yet measured its own rectangle -- the first frame, or a
/// headless run -- still gets a field. 64 is the resolution the map ran at
/// from 2026-09-24 to 2026-09-25, so the floor is a known-good picture rather
/// than an invented one.
pub const MIN_GRID_CELLS: usize = 64;

/// Ceiling on the grid resolution when the WGSL kernel is dispatching on a
/// **GPU**.
///
/// 512 x 512 = 262 144 cells. At the instantaneous puff population this model
/// carries ([`field_states_at`](AtmosphericDispersionChannel::field_states_at)
/// -- **120** contributing puffs, `puff_duration / puff_dt`; the just-emitted
/// one has not travelled and has no sigma) that is **31.5 M kernel
/// evaluations per field**.
///
/// **Measured 2026-09-25** on this workspace's development host (16 logical
/// cores, one adapter present) by `cargo run --release -p changi --example
/// field_timing --features gpu`, median of 5 after a warm-up, at 120 puffs:
///
/// | cells | evaluations | serial | pooled (16 cores) | GPU |
/// |---|---|---|---|---|
/// | 64 | 0.49 M | 2.67 ms | 0.35 ms | 0.57 ms |
/// | 192 | 4.42 M | 24.4 ms | 2.81 ms | 0.50 ms |
/// | 256 | 7.86 M | 43.2 ms | 4.93 ms | 0.65 ms |
/// | **512** | **31.5 M** | 171 ms | 19.6 ms | **1.33 ms** |
///
/// 1.33 ms is negligible against the 100 ms `PHYSICS_TICK`, which is what
/// makes a full-window map affordable at all.
///
/// It is a **cost** ceiling, not a physics one: the model is equally valid at
/// any resolution, and nothing about the answer changes with it.
pub const MAX_GRID_CELLS_GPU: usize = 512;

/// Ceiling on the grid resolution with **no usable GPU adapter**, so the field
/// runs on `changi`'s own CPU thread pool.
///
/// 256 x 256 = 65 536 cells x 120 puffs = **7.9 M evaluations**, measured at
/// **4.93 ms** pooled in the table above.
///
/// # Why not 512 on the CPU too, and why this number moved twice in one day
///
/// ~~512 on that path would be ~145 ms and would miss the tick.~~
/// **CORRECTED 2026-09-25 by measurement**: 512 pooled is **19.6 ms**, which
/// fits inside 100 ms. The estimate had been scaled from `changi`'s
/// 7 260-puff row and was ~7x too pessimistic.
///
/// ~~The ceiling stays at 192 because the plant step already costs ~96 ms of
/// every 100 ms tick, so a 19.6 ms field would drop the simulator to ~0.85x
/// real time.~~ **CORRECTED AGAIN, same day, by measuring the plant instead
/// of citing it.** `tests::where_the_plant_step_spends_its_time` gives
/// **0.265 s of wall clock per second of plant time -- a real-time ratio of
/// 3.78** -- so a 100 ms tick's plant step costs ~26 ms, not ~96 ms. The
/// 0.96 s/s figure in `crate::app`'s doc table predates the steam
/// generator's substep reduction and is stale; it is corrected there too.
///
/// So the honest ceiling is set by the **smallest host**, not this one:
/// 4.93 ms pooled here on 16 cores is ~20 ms on four, which still leaves the
/// tick comfortable, while 512 would be ~78 ms on four and would not. 256 is
/// the largest resolution that is safe without knowing the core count, and a
/// host with a GPU is not held to it -- it gets [`MAX_GRID_CELLS_GPU`].
///
/// **This is a cadence choice and never a model choice.** Both paths compute
/// the same field, and `changi`'s own
/// `field_gpu_agrees_with_the_serial_reference` pins them to 1e-4 relative.
pub const MAX_GRID_CELLS_CPU: usize = 256;

/// The largest grid this host will actually evaluate, probing for the GPU
/// rather than assuming one.
///
/// `changi::puff::wgsl::has_gpu_field()` probes the adapter once per process
/// and caches it, which is exactly the question being asked here: not "is the
/// `gpu` feature on" but "will [`field_auto`](changi::puff::wgsl::field_auto)
/// really take the GPU path on this machine".
pub fn max_grid_cells() -> usize {
    if changi::puff::wgsl::has_gpu_field() {
        MAX_GRID_CELLS_GPU
    } else {
        MAX_GRID_CELLS_CPU
    }
}

/// Half-width of the grid, metres: it spans `+/- GRID_HALF_WIDTH_M` about the
/// release point on both axes.
///
/// 1250 m is 1.25x the outermost receptor ring (1000 m), which is exactly
/// what the square map panel shows: the rings are drawn to `0.40 * size` and
/// the panel's half-width is `0.50 * size`. So the field fills the white box
/// corner to corner instead of leaving a blank margin outside the outer ring
/// (maintainer, 2026-09-24).
pub const GRID_HALF_WIDTH_M: f64 = 1250.0;

/// What the map asks the dispersion channel for: a resolution and a plume
/// clock.
///
/// A **command**, carried on [`super::PlantCommands`] exactly as the
/// meteorology is, so the GUI and a headless run drive the field through the
/// same one-way path rather than the GUI reaching into the channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapFieldRequest {
    /// Cells per side the map wants -- **one per screen pixel** of the map
    /// square. Clamped to `[MIN_GRID_CELLS, max_grid_cells()]` before use;
    /// the request is what the map can show, the clamp is what the host can
    /// afford.
    pub cells: usize,
    /// How far **ahead of the plant clock** the plume is evaluated.
    ///
    /// # Why the plume may legitimately run ahead, and what it may not claim
    ///
    /// The field is `chi/Q`, a dilution factor, and this module's doc says
    /// what that buys: **it does not depend on the source at all.** Its only
    /// inputs are the wind, the stability class and the time elapsed since
    /// the release began. So evaluating it at `t + offset` is not an
    /// extrapolation and not a skipped calculation -- it is the same closed
    /// form at a later argument, exact at any offset.
    ///
    /// What it does **not** do is advance the plant. The reactor, the release
    /// channel and the receptor ring's activity columns all stay on the plant
    /// clock, because those *are* source-dependent and the plant genuinely
    /// cannot skip time (its timestep is pinned by a Courant limit -- see
    /// `physics::STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP`). The Map tab
    /// therefore shows both clocks and says which is which; anything else
    /// would be a picture of a plant state that was never computed.
    ///
    /// Maintainer direction, 2026-09-25: the fast-forward button drives the
    /// plume clock only, for exactly this reason.
    pub plume_clock_offset: Time,
}

impl Default for MapFieldRequest {
    fn default() -> Self {
        Self {
            cells: DEFAULT_GRID_CELLS,
            plume_clock_offset: Time::new::<second>(0.0),
        }
    }
}

/// An **instantaneous** `chi/Q` field on a square grid centred on the release
/// point, as it stands at one moment of the plume clock.
///
/// Row-major, `cells * cells` entries, **north-up**: row 0 is the northernmost
/// row, so it can be painted straight down the screen without the caller
/// having to remember to flip it.
///
/// # Instantaneous, not time-integrated -- the two are different quantities
///
/// ~~The field is the same puff run the receptor ring uses, summed over its
/// output steps.~~ **CHANGED 2026-09-25** (maintainer direction: "timestep
/// according to real-time, I want to see a real-time plume"). The field is now
/// the concentration **at the plume clock's current instant**, per unit
/// release rate:
///
/// ```text
/// chi(x, t)/Q = sum over puffs alive at t of  puff_dt / ((2 pi)^{3/2} sy^2 sz)
///                                             * exp(-r^2 / 2 sy^2)
///                                             * 2 exp(-H^2 / 2 sz^2)
/// ```
///
/// The old field summed that expression over every output step of a 1 200 s
/// run as well, which made it a *time-accumulated* quantity that could not
/// change with the clock -- so the map was a still photograph of a settled
/// plume from the first frame onwards, whatever the plant was doing. The new
/// one starts empty at `t = 0`, grows out of the stack, and reaches its
/// settled population after one puff lifetime.
///
/// **Both carry units of s/m^3 and they are NOT the same number.** The
/// receptor ring's [`ReceptorResult::chi_over_q`] remains the time-integrated
/// dilution factor from `changi::activity`'s own f64 path and is what the
/// table quotes; this field is the instantaneous one and is what the map
/// paints. A reader must not compare a cell against a table row.
#[derive(Debug, Clone)]
pub struct DispersionGrid {
    /// Instantaneous `chi/Q` \[s/m^3\] per cell, row-major, north-up.
    pub chi_over_q: Vec<f64>,
    /// Half-width of the covered square \[m\].
    pub half_width_m: f64,
    /// Cells per side.
    pub cells: usize,
    /// The plume clock this field was evaluated at \[s\] -- plant time plus
    /// [`MapFieldRequest::plume_clock_offset`].
    pub plume_time_s: f64,
}

impl DispersionGrid {
    /// The largest `chi/Q` in the field, for scaling a colour ramp.
    pub fn peak(&self) -> f64 {
        self.chi_over_q.iter().copied().fold(0.0_f64, f64::max)
    }

    /// `chi/Q` at `(column, row)`, or `None` outside the grid.
    pub fn at(&self, column: usize, row: usize) -> Option<f64> {
        if column >= self.cells || row >= self.cells {
            return None;
        }
        self.chi_over_q.get(row * self.cells + column).copied()
    }
}

/// Total receptors: one per sector per distance.
pub const RECEPTOR_COUNT: usize = RECEPTOR_SECTORS * RECEPTOR_DISTANCES_M.len();

/// Site and release inputs. **Every field here is an input, not HTR-10 data**
/// — see the module doc.
#[derive(Debug, Clone, Copy)]
pub struct Htr10SiteInputs {
    /// Release height above ground.
    pub release_height: Length,
    /// Height receptors are evaluated at.
    pub receptor_height: Length,
}

impl Htr10SiteInputs {
    /// Fraction of the **circulating** primary-circuit activity that reaches
    /// the atmosphere per second \[1/s\].
    ///
    /// # This is the single largest uncertainty in the chain, and it is an input
    ///
    /// Going from activity circulating in the primary helium to activity in the
    /// atmosphere requires a containment and leakage model: circuit leak rate,
    /// building retention, filtration, stack release. **This simulator still
    /// models none of that chain** — what it now has is the first term of it.
    ///
    /// ~~HTR-10 has a published design leak rate; it is not in this workspace,
    /// and inventing one and calling it HTR-10's would be putting an
    /// unverified number under a reactor's name. `1e-7 /s` is a round
    /// order-of-magnitude placeholder — roughly 0.9 % of the circulating
    /// inventory per day.~~
    ///
    /// **CORRECTED 2026-09-24 — the published figure is now in the
    /// workspace.** Liu and Cao (2002), p. 4: leakage from the primary
    /// circuit is **about 1 % per day**, which is
    /// `0.01 / 86400 = 1.1574e-7 /s`. The placeholder was 1e-7, so the
    /// correction is 16 % and the previous order of magnitude was right —
    /// which is luck, not vindication: it was a round number chosen to look
    /// like one.
    ///
    /// **What this does NOT become.** A primary-circuit leak rate is not a
    /// release-to-environment fraction. Between the two sit building
    /// retention, filtration and the stack, and this simulator models none of
    /// them — so treating the product of this and an inventory as an
    /// environmental source term still over-states it by whatever those
    /// remove. The same paper's Table 5 reports the airborne activity that
    /// actually reaches the environment; it is **not digitised here**, and
    /// until it is, this remains a circuit leak and nothing more.
    ///
    /// **`chi/Q` does not depend on this at all** (see the module doc), which
    /// is why `chi/Q` is the quotable output and the concentrations are not.
    pub const LEAK_FRACTION_PER_S: f64 = 1.157_4e-7;

    /// Release height \[m\] — **the published HTR-10 stack height.**
    ///
    /// ~~An indicative drawing/modelling input, not a published HTR-10 stack
    /// height. 30 m is a round number of the order of a reactor building.~~
    /// **CORRECTED 2026-09-24:** Liu and Cao (2002), p. 5 give a **40 m
    /// chimney** against a 12 m reactor building, with a 9 m/s outlet
    /// velocity.
    ///
    /// The correction matters more than its size suggests. The Gaussian
    /// puff's ground-level concentration is *strongly* sensitive to release
    /// height through `exp(-(z-H)^2 / 2 sigma_z^2)` — a taller release moves
    /// the ground-level maximum further downwind and lowers it — so 30 -> 40 m
    /// is a leading sensitivity, not a detail.
    ///
    /// **The 9 m/s outlet velocity is recorded and NOT used**: it would drive
    /// a momentum plume rise, raising the effective release height above the
    /// physical stack, and this model has no plume-rise term at all. So the
    /// effective height is under-stated by whatever that rise would be, and
    /// the ground-level concentration correspondingly over-stated. Stated
    /// rather than quietly ignored.
    pub const RELEASE_HEIGHT_M: f64 = 40.0;

    /// Reactor building height \[m\] — Liu and Cao (2002), p. 5.
    ///
    /// Recorded for the building-wake question rather than used: a 40 m stack
    /// against a 12 m building is a ratio of 3.3, comfortably clear of the
    /// 2.5x rule of thumb below which a plume is entrained into the building
    /// wake. So neglecting wake effects is defensible here, and this constant
    /// is what lets a reader check that rather than take it on trust.
    #[allow(dead_code)] // recorded provenance, deliberately unused -- see above
    pub const BUILDING_HEIGHT_M: f64 = 12.0;

    /// Stack outlet velocity \[m/s\] — Liu and Cao (2002), p. 5.
    ///
    /// Recorded, not used. See [`Self::RELEASE_HEIGHT_M`] on plume rise.
    #[allow(dead_code)] // recorded provenance, deliberately unused -- see above
    pub const STACK_EXIT_VELOCITY_M_PER_S: f64 = 9.0;

    /// Receptor height \[m\]. 1.5 m is the conventional breathing height and is
    /// used here purely as the height at which the air concentration is
    /// evaluated; **no dose quantity is computed from it** (see the scope
    /// limit).
    pub const RECEPTOR_HEIGHT_M: f64 = 1.5;

    /// The placeholder site inputs.
    pub fn placeholder() -> Self {
        Self {
            release_height: Length::new::<meter>(Self::RELEASE_HEIGHT_M),
            receptor_height: Length::new::<meter>(Self::RECEPTOR_HEIGHT_M),
        }
    }
}

/// Operator-settable meteorology. These are the knobs the GUI exposes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Meteorology {
    /// Wind speed at release height.
    pub speed: Velocity,
    /// Wind direction in **meteorological convention** — the direction the wind
    /// is blowing *from*, degrees clockwise from north. This is the convention
    /// `changi::puff::wind::wind_vector_convert` takes, and it is the one a
    /// weather report uses; a plume therefore travels *towards* the opposite
    /// bearing, which is the classic sign error in a dispersion display and is
    /// why the convention is named in the type rather than left to a comment.
    pub direction_from: Angle,
    /// Hour of day, 0-23, for the day/night half of the Pasquill lookup.
    pub hour: u32,
    /// Where the stability class comes from. Defaults to deriving it from the
    /// wind speed and hour as upstream does; a fixed class is what a
    /// sensitivity sweep wants.
    pub stability: StabilitySource,
}

impl Default for Meteorology {
    /// A light, neutral default: 3 m/s from the north at midday.
    ///
    /// 3 m/s sits in the middle of the Pasquill table rather than at an
    /// endpoint, so the default condition is not one of the ambiguous
    /// two-class regimes; midday is unambiguously day. Both are chosen so the
    /// opening screen shows one well-defined stability class instead of a
    /// regime whose behaviour depends on the emission policy.
    fn default() -> Self {
        Self {
            speed: Velocity::new::<meter_per_second>(3.0),
            direction_from: Angle::new::<degree>(0.0),
            hour: 12,
            stability: StabilitySource::FromWind,
        }
    }
}

/// One receptor's position and result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReceptorResult {
    /// Compass bearing of the receptor from the source, degrees clockwise from
    /// north.
    pub bearing_deg: f64,
    /// Distance from the source \[m\].
    pub distance_m: f64,
    /// **The quotable output**: the dilution factor `chi/Q` \[s/m^3\], summed
    /// over travel-time bins for a non-decaying species.
    ///
    /// Independent of the source magnitude, so independent of every inventory
    /// and leak-rate input in this chain. See the module doc.
    pub chi_over_q: f64,
    /// Time-integrated air concentration summed over the tracked nuclides,
    /// **per curie of core inventory** \[Bq·s/m^3 per Ci\]. Not a
    /// concentration at HTR-10.
    pub air_bq_s_per_m3: f64,
    /// Ground deposition summed over the tracked nuclides, on the same basis
    /// \[Bq/m^2 per Ci\]. Uses `changi`'s order-of-magnitude placeholder
    /// deposition velocities.
    pub ground_bq_per_m2: f64,
}

/// The most recent dispersion evaluation.
#[derive(Debug, Clone)]
pub struct DispersionResult {
    /// One entry per receptor, ordered distance-major then sector.
    pub receptors: Vec<ReceptorResult>,
    /// The stability class that actually ran, for display. `None` when the
    /// class was derived per puff from a varying wind (it does not here, since
    /// the wind is held constant over a run).
    pub stability: Option<StabilityClass>,
    /// Plant time the evaluation was taken at \[s\].
    pub evaluated_at_s: f64,
    /// The evaluated `chi/Q` field the Map tab paints, one value per cell.
    ///
    /// Every cell is a real evaluation of the same puff model at that cell's
    /// coordinates -- see [`GRID_CELLS`] on why that is not the contour plot
    /// the rose's docs reject.
    pub grid: DispersionGrid,
}

impl DispersionResult {
    /// The highest `chi/Q` over all receptors — the plume centreline at the
    /// closest ring, in practice.
    #[allow(dead_code)] // part of the result's public surface; no caller in this example yet
    pub fn peak_chi_over_q(&self) -> f64 {
        self.receptors
            .iter()
            .map(|r| r.chi_over_q)
            .fold(0.0_f64, f64::max)
    }
}

/// How often the dispersion model is re-run \[s of plant time\].
///
/// # Why this is much slower than everything else in the plant
///
/// A puff run is `O(steps * puffs_alive * receptors)`, and with 24 receptors
/// over a 20-minute puff lifetime it is far and away the most expensive thing
/// this simulator would do per step — while being *quasi-steady* in exactly the
/// sense [`super::fission_product_release`] is: it carries no state between
/// calls, so running it more often integrates nothing more accurately.
///
/// 60 s is chosen against the physics: the dispersion result depends on the
/// wind and the release rate, the wind is an operator input that does not
/// change on its own, and the release rate follows the kernel temperature,
/// which moves on the bed's ~184 s time constant. Nothing the model reads can
/// meaningfully change faster than this.
pub const DISPERSION_EVALUATION_INTERVAL_S: f64 = 60.0;

/// How often the Map tab's `chi/Q` **field** is allowed to refresh \[s of
/// **wall-clock** time\], as distinct from [`DISPERSION_EVALUATION_INTERVAL_S`],
/// which throttles the receptor ring.
///
/// # The field and the ring are on different clocks, for a real reason
///
/// [`DISPERSION_EVALUATION_INTERVAL_S`]'s reasoning -- that nothing the model
/// reads can change faster than the release rate, which follows the kernel
/// temperature, which moves on the bed's ~184 s time constant -- is correct
/// **for the ring**, because the ring's activity columns depend on the
/// source. It does **not** transfer to the field: `chi/Q` is a dilution
/// factor that by construction does not depend on the source at all (see the
/// module doc), only on meteorology, geometry and **elapsed time since the
/// release began**.
///
/// So the field is rate-limited on **wall-clock** time, not plant time: a
/// paused or fast-forwarded simulation must not change how responsive the map
/// feels to a hand on the slider. `0.1 s` is the 10 Hz the Map tab targets.
///
/// # ~~This is purely a rate limit, not a schedule~~ -- CORRECTED 2026-09-25
///
/// ~~[`AtmosphericDispersionChannel::refresh_field`] only ever recomputes
/// when the meteorology has actually changed, which is the overwhelmingly
/// common case (the wind sits still far more often than an operator is
/// dragging it).~~
///
/// That was true while the field was time-accumulated and therefore could not
/// move with the clock. It is **false now**: the field is instantaneous (see
/// [`DispersionGrid`]), so its argument -- the plume clock -- advances on
/// every plant tick, and the refresh is a genuine **10 Hz schedule** rather
/// than an idle check. The cache is still what stops redundant work: the key
/// is `(meteorology, resolution, plume clock)`, so a paused plant with a still
/// wind recomputes nothing, which is the one case the old wording described
/// correctly.
///
/// A slow field computation is still not "fixed" by blocking the caller: it
/// simply refreshes less often than 10 Hz, with the last field staying on
/// screen until the next one is ready. The resolution ceiling that keeps it
/// affordable is [`max_grid_cells`], and it is a cadence choice, never a model
/// choice.
pub const FIELD_REFRESH_INTERVAL_S: f64 = 0.1;

/// Everything a cached field depends on.
///
/// Equality on this is the cache test. It carries the **plume clock** as well
/// as the meteorology and the resolution because the field is instantaneous:
/// the clock is an argument of the model, not a scheduling detail. Comparing
/// only the meteorology -- which is what this did until 2026-09-25 -- would
/// freeze the plume at whatever instant it was first drawn at, which is
/// precisely the still photograph the real-time map replaces.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FieldKey {
    /// The wind and stability the field was computed for.
    meteorology: Meteorology,
    /// Cells per side actually evaluated (after the [`max_grid_cells`] clamp,
    /// not as requested -- two requests that clamp to the same number share a
    /// field, correctly).
    cells: usize,
    /// The plume clock the field was evaluated at \[s\].
    plume_time_s: f64,
}

/// The dispersion channel: fixed site inputs, operator meteorology, and the
/// most recent result.
#[derive(Debug, Clone)]
pub struct AtmosphericDispersionChannel {
    site: Htr10SiteInputs,
    meteorology: Meteorology,
    latest: Option<DispersionResult>,
    last_evaluated_s: Option<f64>,
    /// What the map is asking for: resolution and plume clock offset.
    request: MapFieldRequest,
    /// The last computed map field, reused while nothing it depends on has
    /// moved.
    grid_cache: Option<DispersionGrid>,
    /// The `(meteorology, resolution, plume clock)` `grid_cache` was computed
    /// for. All three, because the field now depends on all three -- see
    /// [`Self::grid_is_current_for`].
    grid_key: Option<FieldKey>,
    /// Wall-clock time [`Self::refresh_field`] last actually recomputed the
    /// field, for the [`FIELD_REFRESH_INTERVAL_S`] rate limit. Deliberately
    /// `std::time::Instant`, not plant time -- see that constant's doc.
    last_field_refresh: Option<std::time::Instant>,
}

impl AtmosphericDispersionChannel {
    /// Build the channel with the placeholder site inputs and default
    /// meteorology.
    pub fn new() -> Self {
        Self {
            site: Htr10SiteInputs::placeholder(),
            meteorology: Meteorology::default(),
            latest: None,
            last_evaluated_s: None,
            request: MapFieldRequest::default(),
            grid_cache: None,
            grid_key: None,
            last_field_refresh: None,
        }
    }

    /// The operator's current meteorology.
    pub fn meteorology(&self) -> Meteorology {
        self.meteorology
    }

    /// Set the meteorology and force a re-evaluation on the next update, since
    /// the wind is the input the result is most sensitive to and an operator
    /// who just moved it expects the map to follow.
    pub fn set_meteorology(&mut self, meteorology: Meteorology) {
        self.meteorology = meteorology;
        self.last_evaluated_s = None;
    }

    /// The most recent result, `None` before the first evaluation.
    pub fn latest(&self) -> Option<&DispersionResult> {
        self.latest.as_ref()
    }

    /// Re-run the dispersion model if the throttle allows.
    ///
    /// Returns `true` when an evaluation actually ran. Does nothing when the
    /// release channel has not yet produced a source term — there is no
    /// fallback source, because a dispersion map drawn from an invented
    /// release would look exactly like one drawn from a real one.
    pub fn update(&mut self, sim_time_s: f64, release: &TrisoAtopsReleaseChannel) -> bool {
        if release.latest().is_empty() {
            return false;
        }
        let due = match self.last_evaluated_s {
            None => true,
            Some(last) => (sim_time_s - last).abs() >= DISPERSION_EVALUATION_INTERVAL_S,
        };
        if !due {
            return false;
        }
        let result = self.evaluate(sim_time_s, release);
        // Remember the field and everything it belongs to, so the next tick
        // reuses it instead of recomputing an identical one.
        self.grid_cache = Some(result.grid.clone());
        self.grid_key = Some(FieldKey {
            meteorology: self.meteorology,
            cells: result.grid.cells,
            plume_time_s: result.grid.plume_time_s,
        });
        self.latest = Some(result);
        self.last_evaluated_s = Some(sim_time_s);
        true
    }

    /// What the map is currently asking for.
    pub fn field_request(&self) -> MapFieldRequest {
        self.request
    }

    /// Accept the map's request: a resolution and a plume-clock offset.
    ///
    /// Stores it and nothing more -- no cache is invalidated here, because
    /// the cache key already carries both (see [`FieldKey`]), so a request
    /// that changes nothing costs nothing and a request that does is picked
    /// up by the next [`Self::refresh_field`] without a second mechanism to
    /// keep in step.
    pub fn set_field_request(&mut self, request: MapFieldRequest) {
        self.request = request;
    }

    /// The grid resolution this host will actually evaluate, from the map's
    /// request and the [`max_grid_cells`] ceiling.
    pub fn field_cells(&self) -> usize {
        self.request.cells.clamp(MIN_GRID_CELLS, max_grid_cells())
    }

    /// The plume clock for a given plant time: plant time plus the operator's
    /// fast-forward offset.
    ///
    /// See [`MapFieldRequest::plume_clock_offset`] for why the plume is
    /// allowed to run ahead of the plant and what that must never be read as.
    pub fn plume_time_s(&self, sim_time_s: f64) -> f64 {
        (sim_time_s + self.request.plume_clock_offset.get::<second>()).max(0.0)
    }

    /// Refresh the map field on its own, faster clock -- see
    /// [`FIELD_REFRESH_INTERVAL_S`] for why the field and the ring must not
    /// share a throttle.
    ///
    /// `sim_time_s` is **plant** time; the plume clock this evaluates at is
    /// [`Self::plume_time_s`] of it.
    ///
    /// Returns `true` if the field was actually recomputed. Two independent
    /// reasons return `false` without doing any work:
    ///
    /// 1. the cached field already matches the current meteorology,
    ///    resolution **and** plume clock -- which now means a genuinely
    ///    stopped plume (a paused plant with a still wind), not merely an
    ///    untouched wind slider;
    /// 2. under [`FIELD_REFRESH_INTERVAL_S`] of wall-clock time has passed
    ///    since the last refresh -- the rate limit that keeps a dragged
    ///    slider, or a plant ticking faster than 10 Hz, from triggering a
    ///    field evaluation per frame.
    ///
    /// Does **not** touch [`Self::update`]'s throttle or its receptor ring:
    /// the two are deliberately independent clocks.
    pub fn refresh_field(&mut self, sim_time_s: f64) -> bool {
        let key = FieldKey {
            meteorology: self.meteorology,
            cells: self.field_cells(),
            plume_time_s: self.plume_time_s(sim_time_s),
        };
        if self.grid_is_current_for(&key) {
            return false;
        }
        let now = std::time::Instant::now();
        if let Some(last) = self.last_field_refresh {
            if now.duration_since(last).as_secs_f64() < FIELD_REFRESH_INTERVAL_S {
                return false;
            }
        }

        let grid = self.compute_field(&self.run_config(), &key);
        self.grid_cache = Some(grid.clone());
        self.grid_key = Some(key);
        self.last_field_refresh = Some(now);
        // So a snapshot written before the next `update()` picks up the fresh
        // field rather than the one `latest` was built with.
        if let Some(latest) = &mut self.latest {
            latest.grid = grid;
        }
        true
    }

    /// Whether the cached grid was computed for exactly this key.
    ///
    /// ~~The field changes only when the *meteorology* does, never because
    /// the release rate moved.~~ **CORRECTED 2026-09-25.** The first half of
    /// that was wrong and the second half is still right: `chi/Q` remains
    /// source-independent, so a moving release rate still cannot change the
    /// field -- but the field is now instantaneous, so the **plume clock**
    /// changes it, and so does the resolution the map asks for. All three
    /// live in [`FieldKey`] rather than being compared by hand, so a fourth
    /// input cannot be forgotten here the way the clock would have been.
    fn grid_is_current_for(&self, key: &FieldKey) -> bool {
        self.grid_key.as_ref() == Some(key)
    }

    /// Run the puff model once, ignoring the throttle. Pure with respect to
    /// plant state — this is what [`Self::update`] calls and what a test calls
    /// directly to sweep wind or stability.
    pub fn evaluate(
        &self,
        sim_time_s: f64,
        release: &TrisoAtopsReleaseChannel,
    ) -> DispersionResult {
        let config = self.run_config();
        let wind = self.wind_series(&config);
        let receptors = self.receptor_ring();
        let sources = [Source {
            x: Length::new::<meter>(0.0),
            y: Length::new::<meter>(0.0),
            height: self.site.release_height,
        }];

        // ONE release window spanning the run: the release rate is held at
        // whatever the kernel temperature currently implies, which is the
        // quasi-steady reading this channel makes (see the module doc on the
        // throttle). Resolving the release into several windows would imply
        // this model tracks a release *history*, which it does not.
        let boundaries = [Time::new::<second>(0.0), config.duration];

        let air = dilution_factors(
            &sources,
            &boundaries,
            &wind,
            &receptors,
            &config,
            self.meteorology.stability,
        );
        // Ground-level factors: the same run, evaluated at z = 0, which is what
        // deposition sees. Two separate factor sets is `changi::survey`'s own
        // interface, not a duplication introduced here.
        let ground_receptors = self.receptor_ring_at(Length::new::<meter>(0.0));
        let ground = dilution_factors(
            &sources,
            &boundaries,
            &wind,
            &ground_receptors,
            &config,
            self.meteorology.stability,
        );

        // The map's field: the INSTANTANEOUS plume at the current plume clock,
        // on a square grid at ground level (see `DispersionGrid` on why that
        // is a different quantity from the ring's time-integrated chi/Q). It
        // is reused verbatim whenever the meteorology, the resolution and the
        // plume clock are all unchanged; `chi/Q` is a dilution factor and does
        // not depend on the source, so a moving release rate cannot change it.
        let key = FieldKey {
            meteorology: self.meteorology,
            cells: self.field_cells(),
            plume_time_s: self.plume_time_s(sim_time_s),
        };
        let grid = match (&self.grid_cache, self.grid_is_current_for(&key)) {
            (Some(cached), true) => cached.clone(),
            _ => self.compute_field(&config, &key),
        };

        let source_term = self.source_term(release, config.duration);
        let site = survey(
            &source_term,
            &air,
            &ground,
            &DepositionVelocities::order_of_magnitude_placeholder(),
        );

        DispersionResult {
            grid,
            receptors: self.collect(&air, &site),
            stability: match self.meteorology.stability {
                StabilitySource::Fixed(c) => Some(c),
                StabilitySource::FromWind => {
                    // The wind is constant over a run here, so the derived set
                    // is the same for every puff and its primary class is what
                    // ran. Reported rather than recomputed per puff.
                    Some(
                        changi::puff::stability::stability_class(
                            Some(self.meteorology.speed),
                            self.meteorology.hour,
                        )
                        .primary(),
                    )
                }
            },
            evaluated_at_s: sim_time_s,
        }
    }

    /// The puff run configuration.
    ///
    /// `sim_dt` 10 s, `puff_dt` 10 s, over a 1200 s run with a 1200 s puff
    /// lifetime — upstream's own default lifetime. At 3 m/s a puff covers
    /// 3.6 km in that time, which carries it past the outermost 1 km ring with
    /// margin, so no receptor is truncated by a puff being dropped mid-flight.
    /// [`tests::the_run_outlasts_the_outermost_receptor`] pins that.
    fn run_config(&self) -> RunConfig {
        RunConfig {
            sim_dt: Time::new::<second>(10.0),
            puff_dt: Time::new::<second>(10.0),
            output_dt: Time::new::<second>(60.0),
            duration: Time::new::<second>(1200.0),
            puff_duration: Time::new::<second>(1200.0),
            start_hour: self.meteorology.hour,
            // Mass-conserving, which is `changi`'s default and the reading that
            // does not double the emitted mass in an ambiguous stability
            // regime. The bug-compatible variant exists only to reproduce
            // upstream's fixture and has no place in a plant model.
            emission_policy: EmissionPolicy::OnePuffPerEmission,
        }
    }

    /// A constant wind over the run, from the operator's speed and direction.
    ///
    /// Constant because this simulator has no meteorology: inventing a varying
    /// wind would put a time structure into the result that nothing in the
    /// plant model produces. `changi` takes a full series, so a future met
    /// input drops in here without touching anything else.
    fn wind_series(&self, config: &RunConfig) -> Vec<WindComponents> {
        let components =
            wind_vector_convert(self.meteorology.speed, self.meteorology.direction_from);
        let steps =
            (config.duration.get::<second>() / config.sim_dt.get::<second>()).floor() as usize + 2;
        constant_wind(components.u, components.v, steps)
    }

    /// The receptor ring at breathing height.
    fn receptor_ring(&self) -> Vec<Receptor> {
        self.receptor_ring_at(self.site.receptor_height)
    }

    /// The receptor ring at an arbitrary height, ordered **distance-major then
    /// sector** — the order [`Self::collect`] and the Map tab both assume.
    fn receptor_ring_at(&self, z: Length) -> Vec<Receptor> {
        let mut receptors = Vec::with_capacity(RECEPTOR_COUNT);
        for distance in RECEPTOR_DISTANCES_M {
            for sector in 0..RECEPTOR_SECTORS {
                let bearing_deg = 360.0 * sector as f64 / RECEPTOR_SECTORS as f64;
                let (x, y) = bearing_to_site_frame(bearing_deg, distance);
                receptors.push(Receptor {
                    x: Length::new::<meter>(x),
                    y: Length::new::<meter>(y),
                    z,
                });
            }
        }
        receptors
    }

    /// The flattened puff states the map field sums over -- **the population
    /// alive at one instant of the plume clock.**
    ///
    /// # Built analytically, not by running the simulator again
    ///
    /// For a **constant** wind -- which is what this simulator has, and says
    /// so -- a puff's whole history is closed form: a puff emitted at `t_j`
    /// is, at time `t`, at `(u, v) * (t - t_j)` with dispersion set by the
    /// distance `|U| * (t - t_j)` it has travelled. So the population can be
    /// written down directly at any `t`, with no time-marching and no state
    /// carried between calls -- which is also why the plume clock may be
    /// jumped an hour ahead and still be exact rather than extrapolated.
    ///
    /// That matters because the alternative was handing every grid cell to
    /// `dilution_factors`, which re-walks every puff at every step for every
    /// receptor. Here the expensive per-puff work -- the branchy
    /// Pasquill-Gifford table walk -- happens **once per state** rather than
    /// once per (state, cell), and what crosses to the GPU is pure arithmetic.
    ///
    /// # What changed on 2026-09-25, and what it cost
    ///
    /// ~~The 7 200 states the field sums over.~~ This used to loop over every
    /// output step of the run *as well as* every emission, accumulating a
    /// time-**accumulated** field of ~7 260 states that could not change with
    /// the clock. It now returns only the puffs alive **now**:
    /// `puff_duration / puff_dt` = **120** states at the shipped
    /// configuration (the newest emission has age zero, so it has no sigma
    /// and is dropped), a 60x reduction in kernel evaluations, which is what
    /// pays for one cell per screen pixel.
    ///
    /// Emission starts at the **oldest puff still alive**, not at `t = 0`, so
    /// the loop is O(puffs alive) rather than O(plume clock): a two-hour
    /// fast-forward costs exactly what the first minute does.
    ///
    /// Weights are per **unit release rate**: a puff carries `Q * puff_dt` of
    /// mass, so at unit `Q` the weight is `puff_dt` \[s\] and the sum is
    /// `chi/Q` in s/m^3 -- independent of the source, exactly as the receptor
    /// ring's `chi/Q` is.
    fn field_states_at(
        &self,
        config: &RunConfig,
        plume_time_s: f64,
    ) -> Vec<changi::puff::wgsl::PuffState> {
        use changi::puff::wgsl::PuffState;

        let components =
            wind_vector_convert(self.meteorology.speed, self.meteorology.direction_from);
        let (u, v) = (
            components.u.get::<meter_per_second>(),
            components.v.get::<meter_per_second>(),
        );
        let speed = (u * u + v * v).sqrt();

        let puff_dt = config.puff_dt.get::<second>();
        let lifetime = config.puff_duration.get::<second>();
        if puff_dt <= 0.0 || plume_time_s < 0.0 {
            return Vec::new();
        }

        // The oldest emission still inside the puff lifetime. Starting here
        // rather than at zero is what keeps an hour-long fast-forward the
        // same cost as the opening minute.
        let oldest_alive = (plume_time_s - lifetime).max(0.0);
        let first = (oldest_alive / puff_dt).ceil() as i64;
        let last = (plume_time_s / puff_dt).floor() as i64;

        let mut states = Vec::with_capacity((last - first + 1).max(0) as usize);
        for index in first..=last {
            let age = plume_time_s - index as f64 * puff_dt;
            if age < 0.0 || age > lifetime {
                continue;
            }
            let travel = speed * age;
            // Zero travel is upstream's NA path: a puff that has not moved
            // has no sigma and contributes nothing.
            let Some(sig) =
                pasquill_gifford_sigmas(self.stability_for_field(), Length::new::<meter>(travel))
            else {
                continue;
            };
            states.push(PuffState {
                x: (u * age) as f32,
                y: (v * age) as f32,
                sigma_y: sig.sigma_y.get::<meter>() as f32,
                sigma_z: sig.sigma_z.get::<meter>() as f32,
                // Mass per unit release RATE in one puff, so the sum is
                // `chi/Q` in s/m^3.
                weight: puff_dt as f32,
            });
        }
        states
    }

    /// The stability class the field uses.
    ///
    /// The wind is constant over a run, so a class derived per puff is the
    /// same for every puff and resolving it once is exact rather than an
    /// approximation.
    fn stability_for_field(&self) -> StabilityClass {
        match self.meteorology.stability {
            StabilitySource::Fixed(c) => c,
            StabilitySource::FromWind => changi::puff::stability::stability_class(
                Some(self.meteorology.speed),
                self.meteorology.hour,
            )
            .primary(),
        }
    }

    /// Evaluate the map field at one instant of the plume clock.
    ///
    /// Sums [`Self::field_states_at`] over the grid through
    /// `changi::puff::wgsl::field_auto`, which dispatches the WGSL kernel on
    /// the GPU when one is there and falls back to changi's own CPU thread
    /// pool when it is not. Both paths exist because
    /// `outram-mc-libs/CLAUDE.md`'s GPU policy makes the CPU route mandatory
    /// and trusted, and here it is also the reference the GPU result is
    /// checked against (`field_gpu_agrees_with_the_serial_reference`, 1e-4
    /// relative).
    ///
    /// # Cost, and why the resolution is capped rather than the cadence
    ///
    /// ~~Measured on the simulator's own 7 260-puff working point (see
    /// `changi::puff::wgsl`'s timing table, 2026-09-24): ~2.0 ms on the GPU,
    /// ~15.9 ms pooled on 16 cores, ~136 ms serial.~~ **RE-MEASURED
    /// 2026-09-25** -- the working point moved. The instantaneous population
    /// is **120 puffs**, not 7 260, so the cost is set by the grid instead:
    /// at one cell per screen pixel, `cargo run --release -p changi --example
    /// field_timing --features gpu` gives **1.33 ms on the GPU and 19.6 ms
    /// pooled on 16 cores at 512 cells**, and **0.50 / 2.81 ms at 192**.
    /// [`MAX_GRID_CELLS_GPU`] carries the full table and
    /// [`MAX_GRID_CELLS_CPU`] says why the CPU ceiling is lower than what
    /// merely fits.
    ///
    fn compute_field(&self, config: &RunConfig, key: &FieldKey) -> DispersionGrid {
        use changi::puff::wgsl::{field_auto, FieldGrid};

        let states = self.field_states_at(config, key.plume_time_s);
        let grid = FieldGrid {
            cells: key.cells,
            half_width_m: GRID_HALF_WIDTH_M as f32,
            source_height_m: self.site.release_height.get::<meter>() as f32,
        };
        DispersionGrid {
            chi_over_q: field_auto(&states, &grid)
                .into_iter()
                .map(f64::from)
                .collect(),
            half_width_m: GRID_HALF_WIDTH_M,
            cells: key.cells,
            plume_time_s: key.plume_time_s,
        }
    }

    /// Build the released source term from the release channel's circulating
    /// pool.
    ///
    /// `released = circulating_activity * LEAK_FRACTION_PER_S * duration`, per
    /// nuclide, on the release channel's per-curie-of-core-inventory basis. The
    /// leak fraction is the input the module doc warns about.
    fn source_term(&self, release: &TrisoAtopsReleaseChannel, duration: Time) -> SourceTerm {
        let window = ReleaseWindow::new(Time::new::<second>(0.0), duration);
        let seconds = duration.get::<second>();

        let nuclides: Vec<NuclideRelease> = release
            .latest()
            .iter()
            .map(|r| {
                // Ci -> Bq through `uom`'s own curie unit rather than a
                // local 3.7e10, which is the convention `changi::activity`'s
                // units module sets ("3.7e10 never has to be written down").
                let released = Radioactivity::new::<curie>(
                    r.activities.circulating_activity
                        * Htr10SiteInputs::LEAK_FRACTION_PER_S
                        * seconds,
                );
                NuclideRelease {
                    label: r.name.to_string(),
                    decay_constant: r.decay_constant,
                    // From the atomic number, so a nuclide added to the release
                    // channel is grouped by its element rather than by a table
                    // that has to be kept in step. Note this is a DEPOSITION
                    // grouping and is deliberately not `boon-lay`'s transport
                    // grouping -- they differ for Se and Te, and `changi`'s
                    // `DepositionGroup` doc says why.
                    deposition_group: DepositionGroup::from_atomic_number(r.z),
                    released: vec![released],
                }
            })
            .collect();

        SourceTerm::new(vec![window], nuclides)
    }

    /// Project the dilution factors and the survey onto one row per receptor.
    fn collect(&self, air: &DilutionFactors, site: &SiteSurvey) -> Vec<ReceptorResult> {
        let mut out = Vec::with_capacity(RECEPTOR_COUNT);
        let mut index = 0;
        for distance in RECEPTOR_DISTANCES_M {
            for sector in 0..RECEPTOR_SECTORS {
                let bearing_deg = 360.0 * sector as f64 / RECEPTOR_SECTORS as f64;
                // chi/Q for a NON-DECAYING species: the sum over travel-time
                // bins with no decay applied. That is the pure dilution the
                // geometry and wind produce, which is what makes it comparable
                // between nuclides and independent of the source.
                let chi_over_q: f64 = (0..air.n_bins())
                    .map(|bin| air.bin(index, 0, bin).seconds_per_cubic_meter())
                    .sum();

                out.push(ReceptorResult {
                    bearing_deg,
                    distance_m: distance,
                    chi_over_q,
                    air_bq_s_per_m3: site.total_air(index).becquerel_seconds_per_cubic_meter(),
                    ground_bq_per_m2: site.total_ground(index).becquerel_per_square_meter(),
                });
                index += 1;
            }
        }
        out
    }
}

impl Default for AtmosphericDispersionChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a compass bearing (degrees clockwise from north) and a distance
/// into the puff model's `(x east, y north)` site frame.
///
/// ```text
/// x = d sin(bearing),   y = d cos(bearing)
/// ```
///
/// Sine on `x` and cosine on `y`, not the other way round: a bearing of 0 is
/// due north, which is `+y`. Getting this backwards mirrors the whole map about
/// the north-east diagonal and still looks like a plume, which is why
/// [`tests::bearings_map_to_the_right_compass_points`] checks all four cardinal
/// points rather than one.
pub fn bearing_to_site_frame(bearing_deg: f64, distance_m: f64) -> (f64, f64) {
    let radians = bearing_deg.to_radians();
    (distance_m * radians.sin(), distance_m * radians.cos())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::thermodynamic_temperature::kelvin;

    fn release_at(kernel_k: f64) -> TrisoAtopsReleaseChannel {
        let mut channel = TrisoAtopsReleaseChannel::new_htr10();
        channel.update(
            0.0,
            Some(uom::si::f64::ThermodynamicTemperature::new::<kelvin>(
                kernel_k,
            )),
            uom::si::f64::ThermodynamicTemperature::new::<kelvin>(950.0),
        );
        channel
    }

    /// Bearings must map to the right compass points in the puff model's
    /// `(x east, y north)` frame.
    ///
    /// All four cardinals are checked, not one: swapping sine and cosine
    /// mirrors the map about the NE diagonal, which leaves north and east
    /// *looking* plausible while putting the plume in the wrong place. A
    /// one-point check would pass.
    #[test]
    fn bearings_map_to_the_right_compass_points() {
        let d = 100.0;
        let cases = [
            (0.0, 0.0, d),    // north -> +y
            (90.0, d, 0.0),   // east  -> +x
            (180.0, 0.0, -d), // south -> -y
            (270.0, -d, 0.0), // west  -> -x
        ];
        for (bearing, want_x, want_y) in cases {
            let (x, y) = bearing_to_site_frame(bearing, d);
            assert!(
                (x - want_x).abs() < 1e-9 && (y - want_y).abs() < 1e-9,
                "bearing {bearing} gave ({x:.3}, {y:.3}), wanted ({want_x:.3}, {want_y:.3})"
            );
        }
    }

    /// V&V: the plume must go **downwind**, and `chi/Q` must fall with
    /// distance.
    ///
    /// # Why both halves are needed
    ///
    /// Meteorological wind direction is the direction the wind blows *from*,
    /// so a north wind carries the plume **south**. That sign is the classic
    /// error in a dispersion display, and it cannot be caught by a
    /// falls-with-distance check alone — a mirrored plume still falls with
    /// distance perfectly well. Equally, a correct direction with a
    /// concentration that did not decay would be a broken dispersion kernel.
    /// So both are asserted.
    ///
    /// **Methodology.** A north wind (`direction_from = 0`) at 3 m/s, midday.
    /// Compare `chi/Q` downwind (bearing 180) against upwind (bearing 0) at
    /// each ring, and check the fall-off along the plume. Pass criteria:
    /// downwind strictly exceeds upwind at every ring, and `chi/Q` falls
    /// monotonically **beyond the ground-level maximum** — see the finding
    /// below for why the qualifier is there and is not a weakening.
    ///
    /// **Results (2026-09-22)**, `chi/Q` \[s/m^3\], wind from the north:
    ///
    /// | Bearing | 100 m | 500 m | 1000 m |
    /// |---|---|---|---|
    /// | 180 (downwind) | 1.4975e-5 | 1.7949e-5 | 4.3966e-6 |
    /// | 0 (upwind) | 1.8763e-16 | 3.7736e-23 | 1.9776e-28 |
    ///
    /// Downwind exceeds upwind by **eleven orders of magnitude** at the first
    /// ring and twenty-two by the last, so the bearing convention, the
    /// meteorological from/to inversion and the receptor ordering are all
    /// wired the right way round. Stability came out class **B**.
    ///
    /// # The ground-level maximum is downwind of the stack — a finding
    ///
    /// **This test failed on first run** because it asserted `chi/Q` falls
    /// monotonically from the first ring, and it does not: 1.4975e-5 at 100 m
    /// *rises* to 1.7949e-5 at 500 m before falling. Rather than relax the
    /// assertion, the cause was isolated by sweeping the release height:
    ///
    /// | `H` | 100 m | 500 m | 1000 m |
    /// |---|---|---|---|
    /// | 0 m | **5.0962e-4** | 2.1701e-5 | 4.5846e-6 |
    /// | 5 m | 4.5401e-4 | 2.1585e-5 | 4.5792e-6 |
    /// | 15 m | 1.8664e-4 | 2.0687e-5 | 4.5368e-6 |
    /// | 30 m | **1.4975e-5** | 1.7949e-5 | 4.3966e-6 |
    /// | 60 m | **5.0528e-8** | 1.0397e-5 | 3.8819e-6 |
    ///
    /// At a **ground-level release the fall-off is monotone from the first
    /// ring**, and raising `H` collapses the 100 m value by four orders while
    /// barely touching 500 m and 1000 m. That is the signature of an
    /// *elevated* plume: close to an elevated stack the plume has not yet
    /// spread down to the ground, so ground-level concentration is suppressed,
    /// rises to a maximum some way downwind, and only then falls. It is
    /// physics, and it is the release-height sensitivity
    /// [`Htr10SiteInputs::RELEASE_HEIGHT_M`] warns about, showing up exactly
    /// where that doc comment says it will.
    ///
    /// So the test now asserts monotone fall-off **beyond** the maximum, and
    /// separately asserts that the `H = 0` case **is** monotone from the first
    /// ring — which is the discriminator. A wiring fault would break the
    /// ground-level case too; only an elevated release breaks one and not the
    /// other. That is a stronger check than the one that failed, not a weaker
    /// one.
    ///
    /// **Interpretation.** This checks the *wiring* — bearing convention, wind
    /// conversion, receptor ordering — not the dispersion physics, which is
    /// `changi`'s and is verified code-to-code against upstream. A model whose
    /// plume went upwind would be wired wrong however good its sigmas were.
    #[test]
    fn the_plume_goes_downwind_and_thins_with_distance() {
        let release = release_at(1200.0);
        let channel = AtmosphericDispersionChannel::new();
        let result = channel.evaluate(0.0, &release);

        let at = |bearing: f64, distance: f64| -> f64 {
            result
                .receptors
                .iter()
                .find(|r| {
                    (r.bearing_deg - bearing).abs() < 1e-9 && (r.distance_m - distance).abs() < 1e-9
                })
                .map(|r| r.chi_over_q)
                .expect("receptor is in the ring")
        };

        let downwind: Vec<f64> = RECEPTOR_DISTANCES_M.iter().map(|d| at(180.0, *d)).collect();
        let upwind: Vec<f64> = RECEPTOR_DISTANCES_M.iter().map(|d| at(0.0, *d)).collect();

        println!(
            "wind FROM north at 3 m/s, class {:?}\n  downwind (S): {:?}\n  upwind   (N): {:?}",
            result.stability, downwind, upwind
        );

        for (i, distance) in RECEPTOR_DISTANCES_M.iter().enumerate() {
            assert!(
                downwind[i] > upwind[i],
                "at {distance} m the plume must be DOWNWIND: south {:.4e} vs north {:.4e} \
                 -- a north wind blows FROM the north",
                downwind[i],
                upwind[i]
            );
        }
        assert!(
            downwind[0] > 0.0,
            "the nearest downwind receptor must see something"
        );

        // Beyond the ground-level maximum the fall-off must be monotone. The
        // maximum itself sits downwind of an elevated stack -- see the doc
        // comment -- so the check starts from wherever the peak is rather than
        // assuming it is at the first ring.
        let peak_at = downwind
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
            .expect("the downwind ring is populated");
        assert!(
            downwind[peak_at..].windows(2).all(|w| w[1] < w[0]),
            "beyond the ground-level maximum (ring {peak_at}) chi/Q must fall; got {downwind:?}"
        );

        // THE DISCRIMINATOR: at a ground-level release the fall-off must be
        // monotone from the very first ring. A wiring fault would break this
        // case too; only an elevated plume breaks one and not the other.
        let ground_level = AtmosphericDispersionChannel {
            site: Htr10SiteInputs {
                release_height: Length::new::<meter>(0.0),
                receptor_height: Length::new::<meter>(Htr10SiteInputs::RECEPTOR_HEIGHT_M),
            },
            ..AtmosphericDispersionChannel::new()
        };
        let flat = ground_level.evaluate(0.0, &release);
        let flat_downwind: Vec<f64> = RECEPTOR_DISTANCES_M
            .iter()
            .map(|d| {
                flat.receptors
                    .iter()
                    .find(|r| {
                        (r.bearing_deg - 180.0).abs() < 1e-9 && (r.distance_m - d).abs() < 1e-9
                    })
                    .map(|r| r.chi_over_q)
                    .expect("receptor is in the ring")
            })
            .collect();
        println!("  ground-level release (H = 0): {flat_downwind:?}");
        assert!(
            flat_downwind.windows(2).all(|w| w[1] < w[0]),
            "a GROUND-LEVEL release must fall monotonically from the first ring -- if this \
             fails the non-monotonicity above is a wiring fault, not an elevated plume; \
             got {flat_downwind:?}"
        );
        assert!(
            flat_downwind[0] > downwind[0],
            "a ground-level release must give a HIGHER close-in concentration than an \
             elevated one: {:.4e} vs {:.4e}",
            flat_downwind[0],
            downwind[0]
        );
    }

    /// V&V: `chi/Q` must be **independent of the source magnitude**, which is
    /// the property that makes it the one quotable output of this chain.
    ///
    /// # Why this is the load-bearing test of the module
    ///
    /// The module doc claims that every uncertainty about inventories and leak
    /// rates cancels out of `chi/Q`, and that claim is what licenses reporting
    /// `chi/Q` as a real number while refusing to report concentrations as
    /// HTR-10 figures. If it were false — if `chi/Q` picked up any dependence
    /// on the source — then the one number this module offers would be hostage
    /// to the placeholder leak fraction, and the honest thing would be to
    /// report nothing at all.
    ///
    /// **Methodology.** Evaluate at kernel temperatures 1000 K and 1500 K,
    /// which change the TRISO release rate by orders of magnitude (see
    /// [`super::super::fission_product_release`]), and compare `chi/Q` at every
    /// receptor. Pass criterion: bit-identical, not merely close — `chi/Q` is
    /// computed from the dilution factors alone and the source never enters
    /// it, so any difference at all would mean the separation has leaked.
    ///
    /// **Results (2026-09-22).** Every receptor identical across a source that
    /// moved by orders of magnitude; the air concentrations, which *should*
    /// track the source, moved as expected. Both recorded by the `println!`.
    ///
    /// **Interpretation.** The separation holds, so `chi/Q` may be quoted as a
    /// dispersion result and the concentrations may not be quoted as anything
    /// but a transfer function.
    #[test]
    fn chi_over_q_is_independent_of_the_source() {
        let channel = AtmosphericDispersionChannel::new();
        let cool = channel.evaluate(0.0, &release_at(1000.0));
        let hot = channel.evaluate(0.0, &release_at(1500.0));

        let mut worst = 0.0_f64;
        for (a, b) in cool.receptors.iter().zip(hot.receptors.iter()) {
            worst = worst.max((a.chi_over_q - b.chi_over_q).abs());
        }

        let air_cool: f64 = cool.receptors.iter().map(|r| r.air_bq_s_per_m3).sum();
        let air_hot: f64 = hot.receptors.iter().map(|r| r.air_bq_s_per_m3).sum();
        println!(
            "chi/Q worst difference over a source change of x{:.3e}: {worst:.3e} s/m^3\n\
             summed air concentration moved {air_cool:.4e} -> {air_hot:.4e} Bq.s/m^3 per Ci",
            if air_cool > 0.0 {
                air_hot / air_cool
            } else {
                f64::NAN
            }
        );

        assert_eq!(
            worst, 0.0,
            "chi/Q must not depend on the source at all; worst difference {worst:e}"
        );
        assert!(
            air_hot > air_cool,
            "the air concentration SHOULD track the source: {air_cool:e} -> {air_hot:e}"
        );
    }

    /// The puff run must outlast the flight to the outermost receptor,
    /// otherwise that ring is reading a truncated plume rather than a thin one.
    ///
    /// This is the kind of configuration error that produces a physically
    /// plausible picture — concentration falling steeply with distance — for a
    /// numerical reason, so it is pinned rather than eyeballed.
    #[test]
    fn the_run_outlasts_the_outermost_receptor() {
        let channel = AtmosphericDispersionChannel::new();
        let config = channel.run_config();
        let speed = channel.meteorology.speed.get::<meter_per_second>();
        let reach_m = speed * config.puff_duration.get::<second>();
        let outermost = RECEPTOR_DISTANCES_M.iter().cloned().fold(0.0_f64, f64::max);

        println!(
            "puff reach at {speed} m/s over {} s = {reach_m:.0} m against an outermost \
             receptor at {outermost:.0} m",
            config.puff_duration.get::<second>()
        );
        assert!(
            reach_m > outermost * 1.5,
            "puffs reach {reach_m:.0} m but the outermost receptor is at {outermost:.0} m; \
             that ring would be truncated, not merely dilute"
        );
    }

    /// The throttle must hold, and a release channel with nothing in it must
    /// not produce a dispersion map.
    ///
    /// The second half matters: a map drawn from an invented source would look
    /// exactly like one drawn from a real one, so the channel declines rather
    /// than substituting anything.
    #[test]
    fn the_throttle_holds_and_an_empty_release_produces_nothing() {
        let release = release_at(1200.0);
        let mut channel = AtmosphericDispersionChannel::new();

        assert!(
            channel.update(0.0, &release),
            "the first call must evaluate"
        );
        assert!(channel.latest().is_some());
        assert!(
            !channel.update(30.0, &release),
            "half an interval later must NOT re-evaluate"
        );
        assert!(
            channel.update(60.0, &release),
            "a full interval later must re-evaluate"
        );

        // Moving the wind forces a re-evaluation, because it is the input the
        // result is most sensitive to.
        channel.set_meteorology(Meteorology {
            direction_from: Angle::new::<degree>(90.0),
            ..Meteorology::default()
        });
        assert!(
            channel.update(61.0, &release),
            "a wind change must force a re-evaluation inside the throttle"
        );

        // An un-evaluated release channel has nothing to disperse.
        let empty = TrisoAtopsReleaseChannel::new_htr10();
        let mut fresh = AtmosphericDispersionChannel::new();
        assert!(
            !fresh.update(0.0, &empty),
            "an empty release channel must not produce a dispersion map"
        );
        assert!(fresh.latest().is_none());
    }

    /// Turning the wind must turn the plume, by the same angle.
    ///
    /// A map that responded to wind *speed* but not *direction* would still
    /// look alive, which is why this checks that the peak moves to the sector
    /// the wind actually points at rather than merely that something changed.
    #[test]
    fn turning_the_wind_turns_the_plume() {
        let release = release_at(1200.0);

        for (from_deg, want_downwind_deg) in [(0.0, 180.0), (90.0, 270.0), (270.0, 90.0)] {
            let mut channel = AtmosphericDispersionChannel::new();
            channel.set_meteorology(Meteorology {
                direction_from: Angle::new::<degree>(from_deg),
                ..Meteorology::default()
            });
            let result = channel.evaluate(0.0, &release);

            // The peak receptor on the innermost ring.
            let peak = result
                .receptors
                .iter()
                .filter(|r| (r.distance_m - RECEPTOR_DISTANCES_M[0]).abs() < 1e-9)
                .max_by(|a, b| a.chi_over_q.total_cmp(&b.chi_over_q))
                .expect("the innermost ring is populated");

            println!(
                "wind from {from_deg:>5.1} deg -> peak at bearing {:>5.1} deg \
                 (chi/Q {:.4e} s/m^3), wanted {want_downwind_deg:.1}",
                peak.bearing_deg, peak.chi_over_q
            );
            assert!(
                (peak.bearing_deg - want_downwind_deg).abs() < 1e-9,
                "wind from {from_deg} deg must put the plume at {want_downwind_deg} deg, \
                 not {}",
                peak.bearing_deg
            );
        }
    }

    /// **The field's own clock is a no-op once it is caught up.** The first
    /// call on a fresh channel has nothing cached yet and must actually
    /// compute; an immediate second call, at the **same plume clock** and
    /// with the meteorology unchanged, must not.
    ///
    /// Same plant time in both calls is the load-bearing part: the field is
    /// instantaneous now, so a second call at a *later* clock is a genuinely
    /// different field and is supposed to recompute.
    #[test]
    fn refresh_field_is_a_no_op_once_the_clock_and_meteorology_are_cached() {
        let mut channel = AtmosphericDispersionChannel::new();
        assert!(
            channel.refresh_field(300.0),
            "the first call has nothing cached and must compute"
        );
        assert!(
            !channel.refresh_field(300.0),
            "an unchanged clock and meteorology must not trigger another computation"
        );
    }

    /// **A meteorology change is allowed one refresh, then the rate limit
    /// holds** -- this is [`FIELD_REFRESH_INTERVAL_S`]'s whole point: a
    /// dragged slider must not trigger a field evaluation on every frame.
    ///
    /// # The wall clock is stepped explicitly, and that is a correction
    ///
    /// ~~This test called `refresh_field` twice in a row and expected the
    /// second to compute.~~ **CORRECTED 2026-09-25.** It did pass that way,
    /// but only because the old field -- 7 260 puffs over 4 096 cells -- took
    /// longer to compute than [`FIELD_REFRESH_INTERVAL_S`], so the rate limit
    /// had already expired by the time the second call asked. It was a test
    /// passing on a timing accident, and the instantaneous field (120 puffs,
    /// sub-millisecond at the default resolution) exposed it immediately.
    /// The interval is now stepped by hand, so what is asserted is the rule
    /// rather than the host's speed.
    #[test]
    fn changing_the_meteorology_forces_one_refresh_then_the_rate_limit_holds() {
        let mut channel = AtmosphericDispersionChannel::new();
        assert!(channel.refresh_field(300.0), "establish the baseline cache");

        channel.set_meteorology(Meteorology {
            direction_from: Angle::new::<degree>(90.0),
            ..Meteorology::default()
        });
        assert!(
            !channel.refresh_field(300.0),
            "inside the interval, even a changed meteorology must be refused"
        );
        channel.last_field_refresh = Some(
            std::time::Instant::now()
                - std::time::Duration::from_secs_f64(FIELD_REFRESH_INTERVAL_S + 0.05),
        );
        assert!(
            channel.refresh_field(300.0),
            "past the interval, a changed meteorology must trigger exactly one refresh"
        );
        assert!(
            !channel.refresh_field(300.0),
            "an immediate second call must be refused by the rate limit"
        );
    }

    /// **The rate limit is genuinely time-based, not merely "the cache
    /// happens to already match".** With a cache the current state does NOT
    /// match, a refresh attempted well inside [`FIELD_REFRESH_INTERVAL_S`] of
    /// the last one must still be refused; the same call must succeed once
    /// that interval has genuinely elapsed. This is the branch that actually
    /// protects the plant loop while an operator drags the wind slider faster
    /// than 10 Hz.
    #[test]
    fn refresh_field_rate_limit_is_time_based_not_cache_based() {
        let mut channel = AtmosphericDispersionChannel::new();
        // The cache belongs to a DIFFERENT meteorology from the one now set,
        // refreshed "just now" -- so the key does not match and the only
        // thing that can block this call is the wall-clock gate.
        channel.grid_key = Some(FieldKey {
            meteorology: Meteorology {
                direction_from: Angle::new::<degree>(45.0),
                ..Meteorology::default()
            },
            cells: channel.field_cells(),
            plume_time_s: 300.0,
        });
        channel.last_field_refresh = Some(std::time::Instant::now());
        assert!(
            !channel.refresh_field(300.0),
            "a refresh inside FIELD_REFRESH_INTERVAL_S must be refused even \
             though the cache does not match the current state"
        );

        // Once the interval has genuinely elapsed, the same stale cache must
        // be allowed to refresh.
        channel.last_field_refresh = Some(
            std::time::Instant::now()
                - std::time::Duration::from_secs_f64(FIELD_REFRESH_INTERVAL_S + 0.05),
        );
        assert!(
            channel.refresh_field(300.0),
            "past the interval, a stale cache must be allowed to refresh"
        );
    }

    /// **The clock moves the field.** A later plume clock is a different
    /// field, and the cache must not swallow it.
    ///
    /// This is the regression test for the defect the real-time map exists to
    /// fix: until 2026-09-25 the cache key was the meteorology alone, so the
    /// map drew one still photograph and kept it however far the plant ran.
    #[test]
    fn a_later_plume_clock_is_a_different_field() {
        let mut channel = AtmosphericDispersionChannel::new();
        assert!(channel.refresh_field(300.0), "establish the baseline cache");
        // Step past the wall-clock rate limit, which is not what is under
        // test here.
        channel.last_field_refresh = Some(
            std::time::Instant::now()
                - std::time::Duration::from_secs_f64(FIELD_REFRESH_INTERVAL_S + 0.05),
        );
        assert!(
            channel.refresh_field(360.0),
            "a minute later on the plume clock must recompute the field"
        );
        let grid = channel.grid_cache.as_ref().expect("a field was computed");
        assert!(
            (grid.plume_time_s - 360.0).abs() < 1e-9,
            "the field must record the clock it was evaluated at, got {}",
            grid.plume_time_s
        );
    }

    /// **V&V: the plume grows out of the stack and then settles.**
    ///
    /// **Methodology.** Default meteorology (3 m/s from the north, midday,
    /// class B from the wind). The instantaneous puff population is taken at
    /// a sweep of plume clocks and, for each, the number of contributing
    /// puffs and the downwind reach of the furthest one are recorded. Pass
    /// criteria, all three stated before the run: (a) an empty population at
    /// `t = 0`, because the only puff emitted has not travelled and has no
    /// sigma; (b) the population grows while `t < puff_duration`; (c) it is
    /// capped thereafter, so a fast-forward of an hour costs the same as the
    /// first minute.
    ///
    /// **Results (2026-09-25)** -- printed by this test, `puff_dt` 10 s,
    /// `puff_duration` 1200 s:
    ///
    /// | plume clock | contributing puffs | furthest reach |
    /// |---|---|---|
    /// | 0 s | 0 | -- |
    /// | 60 s | 6 | 180 m |
    /// | 600 s | 60 | 1800 m |
    /// | 1200 s | 120 | 3600 m |
    /// | 3600 s (1 h fast-forward) | 120 | 3600 m |
    /// | 7200 s (2 h fast-forward) | 120 | 3600 m |
    ///
    /// # The cap is `puff_duration / puff_dt`, not that plus one -- a finding
    ///
    /// **This test failed on first run**, asserting a settled population of
    /// 121 against a measured 120. The prediction was wrong, not the model:
    /// `[oldest_alive, now]` does span 121 emission times, but the newest of
    /// them has age zero, so it has travelled nowhere, `pasquill_gifford_sigmas`
    /// returns `None` for it -- upstream's NA path -- and it contributes
    /// nothing. The one at age exactly `puff_duration` is kept, so the count
    /// is 120 at every clock past one lifetime. The assertion was corrected
    /// to the derived number rather than loosened to an inequality, which
    /// would have hidden the same error next time.
    ///
    /// **Interpretation.** The plume reaches its settled population after one
    /// puff lifetime and does not change afterwards unless the wind does --
    /// which is the honest limit of the fast-forward button and is said on
    /// the Map tab rather than left for a user to infer from a picture that
    /// stops moving.
    #[test]
    fn the_plume_grows_out_of_the_stack_and_then_settles() {
        let channel = AtmosphericDispersionChannel::new();
        let config = channel.run_config();
        let puff_dt = config.puff_dt.get::<second>();
        let lifetime = config.puff_duration.get::<second>();
        // `puff_duration / puff_dt`, NOT that plus one: the just-emitted puff
        // has age zero, so it has no sigma and contributes nothing. See the
        // finding in this test's doc comment.
        let cap = (lifetime / puff_dt) as usize;

        let mut previous = 0usize;
        for clock in [0.0, 60.0, 600.0, 1200.0, 3600.0, 7200.0] {
            let states = channel.field_states_at(&config, clock);
            let reach = states
                .iter()
                .map(|s| ((s.x * s.x + s.y * s.y).sqrt()) as f64)
                .fold(0.0_f64, f64::max);
            println!(
                "plume clock {clock:>7.0} s -> {:>4} live puffs, furthest reach {reach:>7.0} m",
                states.len()
            );

            if clock == 0.0 {
                assert!(
                    states.is_empty(),
                    "a puff that has not travelled has no sigma and must contribute nothing"
                );
            }
            assert!(
                states.len() <= cap,
                "plume clock {clock} s carries {} puffs, above the {cap} the lifetime allows",
                states.len()
            );
            if clock <= lifetime {
                assert!(
                    states.len() >= previous,
                    "the population must grow while the clock is inside one puff lifetime"
                );
            } else {
                assert_eq!(
                    states.len(),
                    cap,
                    "past one puff lifetime the population must be settled at the cap"
                );
            }
            previous = states.len();
        }
    }

    /// **The field is a dilution factor, so the plume clock is exact at any
    /// offset** -- a two-hour fast-forward must give the same field as
    /// actually running the plant for two hours would.
    ///
    /// That equality is the whole justification for the fast-forward button
    /// driving the plume clock rather than the plant (maintainer, 2026-09-25),
    /// so it is asserted rather than argued: the state population is a closed
    /// form in elapsed time, and it carries no history, so `plant t + offset`
    /// and `plant t` with the same total are the same argument.
    #[test]
    fn a_plume_clock_jump_equals_having_run_the_clock_there() {
        let mut jumped = AtmosphericDispersionChannel::new();
        jumped.set_field_request(MapFieldRequest {
            plume_clock_offset: Time::new::<second>(7200.0),
            ..MapFieldRequest::default()
        });
        let config = jumped.run_config();

        let ran = AtmosphericDispersionChannel::new();
        let plant_t = 300.0;

        assert!((jumped.plume_time_s(plant_t) - 7500.0).abs() < 1e-9);
        let a = jumped.field_states_at(&config, jumped.plume_time_s(plant_t));
        let b = ran.field_states_at(&config, 7500.0);
        assert_eq!(a.len(), b.len(), "the populations must be the same size");
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x, y, "a jumped clock must be bit-identical to a run one");
        }
    }

    /// **The resolution the map asks for is honoured, and capped.**
    ///
    /// The cap is a cost ceiling and must bind on the *evaluated* grid, not
    /// merely be documented -- a map that asked for 4 096 cells would
    /// otherwise put ~2 billion kernel evaluations inside a 100 ms tick.
    #[test]
    fn the_requested_resolution_is_honoured_up_to_the_hosts_ceiling() {
        let ceiling = max_grid_cells();
        let mut channel = AtmosphericDispersionChannel::new();

        channel.set_field_request(MapFieldRequest {
            cells: 4096,
            ..MapFieldRequest::default()
        });
        assert_eq!(channel.field_cells(), ceiling, "the ceiling must bind");

        channel.set_field_request(MapFieldRequest {
            cells: 1,
            ..MapFieldRequest::default()
        });
        assert_eq!(channel.field_cells(), MIN_GRID_CELLS, "the floor must bind");

        let wanted = MIN_GRID_CELLS + (ceiling - MIN_GRID_CELLS) / 2;
        channel.set_field_request(MapFieldRequest {
            cells: wanted,
            ..MapFieldRequest::default()
        });
        assert_eq!(channel.field_cells(), wanted, "a request inside the range must pass through");

        channel.refresh_field(600.0);
        let grid = channel.grid_cache.as_ref().expect("a field was computed");
        assert_eq!(grid.cells, wanted);
        assert_eq!(grid.chi_over_q.len(), wanted * wanted);
    }

    /// **The instantaneous field puts the plume downwind and dilutes it with
    /// distance** -- the same two sign traps the receptor ring's own V&V
    /// test guards, checked on the picture the map actually paints.
    ///
    /// **Methodology.** Default meteorology (3 m/s **from the north**, so the
    /// plume travels south) at a plume clock of 600 s, on a 64-cell grid.
    /// Compare the column through the release point at one quarter of the way
    /// south against the mirrored cell to the north. Pass criterion: the
    /// southern cell exceeds the northern one, and the peak of the whole
    /// field lies in the southern half.
    ///
    /// **Results (2026-09-25)**: printed below on every run. A mirrored field
    /// still looks like a plume, which is why the direction is asserted and
    /// not eyeballed.
    #[test]
    fn the_instantaneous_field_puts_the_plume_downwind() {
        let mut channel = AtmosphericDispersionChannel::new();
        channel.set_field_request(MapFieldRequest {
            cells: MIN_GRID_CELLS,
            ..MapFieldRequest::default()
        });
        assert!(channel.refresh_field(600.0));
        let grid = channel.grid_cache.as_ref().expect("a field was computed");

        let n = grid.cells;
        let centre = n / 2;
        let quarter = n / 4;
        let south = grid.at(centre, centre + quarter).expect("inside the grid");
        let north = grid.at(centre, centre - quarter).expect("inside the grid");
        println!(
            "wind from the north: chi/Q south {south:.4e} s/m^3 against north {north:.4e} s/m^3"
        );
        assert!(
            south > north,
            "a wind FROM the north must carry the plume SOUTH; got south {south:.4e} \
             against north {north:.4e}"
        );

        let (peak_index, peak) = grid
            .chi_over_q
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .expect("the field is populated");
        let peak_row = peak_index / n;
        println!("peak chi/Q {peak:.4e} s/m^3 at row {peak_row} of {n} (row 0 is north)");
        assert!(
            peak_row > centre,
            "the peak must lie south of the release point, not at row {peak_row}"
        );
    }
}
