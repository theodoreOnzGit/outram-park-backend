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
    /// the release began.
    ///
    /// ~~"So evaluating it at `t + offset` is not an extrapolation and not a
    /// skipped calculation -- it is the same closed form at a later argument,
    /// exact at any offset."~~ **CORRECTED 2026-09-27.** That was true of the
    /// closed-form field and is **false** now that the puff population is
    /// marched ([`AtmosphericDispersionChannel::advance_population`], and see
    /// [`FieldPuff`] for why it had to be). Two things a reader must not carry
    /// forward from the struck sentence:
    ///
    /// 1. **A forward jump IS an extrapolation.** The catch-up march applies
    ///    the **current** wind to the whole jumped interval, because this
    ///    channel holds one wind and no history of where it has been. So the
    ///    jumped field is "the plume this wind would build if it held that
    ///    long", not a forecast across a wind change.
    /// 2. **A rewind CLEARS the population.** The trajectory integral is not
    ///    invertible and no per-step history is kept, so the plume restarts
    ///    from the stack and any bend from an earlier wind change is lost.
    ///
    /// The source-independence of `chi/Q` is unaffected and still holds --
    /// it was never the part that made the jump exact; the closed form was.
    /// `tests::a_plume_clock_jump_equals_having_run_the_clock_there_on_a_steady_wind`
    /// still passes, and its `_on_a_steady_wind` suffix is precisely this
    /// correction: the equality holds because both sides march the *same
    /// constant wind*, not because the field can be evaluated anywhere.
    ///
    /// This claim was found by a review of the Map tab's on-screen text, which
    /// had inherited it; the tab now says both caveats at the control that
    /// causes them.
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
    /// **Instantaneous** `chi/Q` \[s/m^3\], sampled from the marched population
    /// at this receptor — see
    /// [`AtmosphericDispersionChannel::instantaneous_chi_over_q_at_ring`]. A
    /// different quantity from [`Self::chi_over_q`] above, which is
    /// time-integrated; this is the one that agrees with the map field.
    pub instantaneous_chi_over_q: f64,
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
/// # ~~Why this is much slower than everything else in the plant~~
/// # MEASURED 2026-09-27: ~10 % of the plant step, not "far and away the most"
/// # (and the first attempt at this measurement said 0.00 % and was WRONG)
///
/// ~~"A puff run is `O(steps * puffs_alive * receptors)`, and with 24 receptors
/// over a 20-minute puff lifetime it is **far and away the most expensive thing
/// this simulator would do per step**."~~ **CORRECTED 2026-09-27 — overstated by
/// about an order of magnitude.** It is ~10 % of the plant step against the steam
/// generator's ~100 %. This correction was itself wrong on its first attempt, which
/// claimed 0.00 %; see the table's second column and the note below it.
///
/// `tests::where_the_plant_step_spends_its_time`, over 20 s of plant time:
///
/// | Component | ~~First reading~~ | **CORRECTED, evaluations verified** |
/// |---|---|---|
/// | **Gaussian puff dispersion** | ~~0.00 %~~ | **10.26 %** (54.3 ms/call, 10/10 evaluated) |
/// | TRISO-ATOPS release channel | ~~0.00 %~~ | **0.09 %** |
/// | primary: steam generator | 100.12 % (26.46 ms/call) | **102.23 %** (27.05 ms/call) |
/// | primary: hot leg + core | 0.03 % | 0.03 % |
/// | secondary loop (IF97) | 0.40 % | 0.36 % |
///
/// # The first column was measured WRONG, and finding out why mattered
///
/// Both `0.00 %` readings were the cost of an **early return**, not of the model.
/// `plant.core.peak_kernel_temperature()` returns `None` once the bed passes the
/// nuclear-graphite correlations' 2000 K ceiling (gh:#350, gh:#351) — which it does
/// within ~4 s at the shipped opening condition — a `None` kernel leaves the release
/// channel empty, and [`AtmosphericDispersionChannel::update`] returns early on an
/// empty release channel. The breakdown charged 0.0000 s for work that never ran,
/// and **nothing checked**.
///
/// `tests::where_the_plant_step_spends_its_time` now counts the calls that actually
/// evaluated, **asserts the count is non-zero**, and prints `N/M calls that
/// EVALUATED`, so this cannot recur silently. It charges a representative 1100 K
/// kernel, because the plant's own is `None`.
///
/// The honest conclusion is narrower than either earlier one: the steam generator
/// dominates by roughly a factor of **10**, not by everything — and the dispersion
/// is not free either. ~10 % of the plant step at the 2 s interval, ~0.3 % at 60 s.
///
/// The originally struck claim was true when written and has been overtaken twice:
/// the map field moved to a WGSL kernel on the GPU (0.67 ms at the 64-cell
/// default, 9.09 ms even at 512 cells — `cargo plume-timing`), and the population
/// the ring walks fell from ~7 260 puffs to the 120 alive at an instant when the
/// field became instantaneous on 2026-09-25. What actually dominates is the
/// steam generator, at 26.46 ms per plant step — a quarter of the 100 ms tick on
/// its own — which is what `super::primary_loop`'s "~96 % of this plant's
/// compute" has said all along. The two claims contradicted each other and this
/// is the one that was wrong.
///
/// **This matters beyond tidiness.** The interval below is justified by the
/// struck sentence, so the justification is gone even though the interval may
/// still be defensible on the quasi-steady argument that follows. And because the
/// ring's time-integrated `chi/Q` refreshes only on this throttle, a claim about
/// cost that is off by everything is the reason that column cannot be live —
/// see [`ReceptorResult::instantaneous_chi_over_q`], which had to be added
/// alongside it rather than replacing it. **Whether this throttle should exist
/// at all is now an open question and a maintainer decision**, not something to
/// infer from a cost that has been measured at zero.
///
/// The dispersion run remains *quasi-steady* in exactly the sense
/// [`super::fission_product_release`] is: it carries no state between calls, so
/// running it more often integrates nothing more accurately.
///
/// # Then MEASURED PROPERLY, 2026-09-27: 26.8 ms per call, and it STAYS
///
/// The maintainer asked for this throttle to come out. It has been carried out
/// for **half** of the dispersion side and refused for the other half, on a
/// measurement rather than on the struck claim above.
///
/// `tests::what_one_dispersion_evaluation_costs` times one full
/// [`AtmosphericDispersionChannel::evaluate`] with a **populated** release
/// channel — populated deliberately, because `update` returns early on an empty
/// one and that early return is what the 0.00 % reading above actually measured:
///
/// | | Value |
/// |---|---|
/// | one full evaluation | **26.819 ms** (median of 5, after a warm-up) |
/// | steam generator, for scale | 26.46 ms/call = 100.12 % of the plant step |
/// | as a share of a 100 ms tick | **27 %** |
/// | as a share of a 60 fps frame | **161 % — does not fit** |
///
/// So the two halves of "the dispersion" differ by a factor of **37**: the map
/// field is 0.731 ms on the GPU and now runs at 60 fps
/// ([`FIELD_REFRESH_INTERVAL_S`]), while the receptor ring's time-integrated
/// `chi/Q` plus the activity survey is 26.8 ms and would cost real-time ratio
/// directly at any interactive rate. Lumping them under one word is what made the
/// original claim above look plausible for as long as it did.
///
/// **What this throttle still governs**, therefore, is only the expensive half:
/// two `dilution_factors` runs (air at receptor height, ground at `z = 0`) and
/// the activity survey. The live column a reader watches is **not** on it — see
/// [`ReceptorResult::instantaneous_chi_over_q`], refreshed beside the field.
///
/// **Not claimed:** that 60 s is the *right* interval. It is justified by the
/// quasi-steady argument, and 26.8 ms is cheap enough that a much shorter one
/// would also be affordable — 1 s would cost 2.7 % of plant-time compute. Nobody
/// has measured whether the activity columns visibly lag at 60 s, and that is the
/// question to ask if they feel stale, rather than removing the throttle outright.
///
/// 60 s is chosen against the physics: the dispersion result depends on the
/// wind and the release rate, the wind is an operator input that does not
/// change on its own, and the release rate follows the kernel temperature,
/// which moves on the bed's ~184 s time constant. Nothing the model reads can
/// meaningfully change faster than this.
pub const DISPERSION_EVALUATION_INTERVAL_S: f64 = 2.0;

// ── ~~60.0~~ ~~10 Hz~~ -> 2.0 s of plant time, CHANGED 2026-09-27 ────────────
//
// **Maintainer direction**, in two steps on the same day: *"Receptor ring should
// update at 10 Hz"*, then, on seeing what it cost, *"once every 2 s then"*.
//
// This is **plant** time, not wall clock — unlike [`FIELD_REFRESH_INTERVAL_S`],
// which is wall clock so the map stays responsive under a pause or a
// fast-forward. Plant time is right here because what the ring reports depends on
// the release rate, which is driven by the kernel temperature on the bed's
// ~184 s time constant.
//
// # What 10 Hz cost, measured
//
// Set to every plant tick and measured with
// `tests::where_the_plant_step_spends_its_time` over 20 s of plant time:
//
// |  | 60 s throttle | every tick |
// |---|---|---|
// | whole plant, wall | 5.2863 s | **6.0722 s** |
// | real-time ratio | 3.783 | **3.294** |
// | dispersion calls | 1 | 200 |
//
// So every-tick cost about **13 % of the real-time ratio** — 0.79 s over 200
// calls, ~3.9 ms per call. Note that is far LESS than the 26.819 ms
// `tests::what_one_dispersion_evaluation_costs` measures for a standalone
// `evaluate`, and the difference is not explained: in the plant many calls hit the
// grid cache, so they are not all doing the same work. **Both numbers stand as
// measured and the discrepancy is open** rather than being averaged into one
// figure that matches neither.
//
// 2.0 s of plant time is 20 plant ticks, so the ring costs ~5 % of what every
// tick did while still refreshing thirty times per minute — well inside any
// transient a reader is watching, and 30x more responsive than the 60 s it
// replaced.
//
// **If the speed is wanted back**, in increasing order of effort:
//
//  1. Note the ring's *live* column already updates at 60 fps independently of
//     this throttle ([`ReceptorResult::instantaneous_chi_over_q`]), so the
//     question is only how stale the TIME-INTEGRATED column and the activity
//     columns may be. An interval of 1 s costs 2.7 % of plant-time compute
//     instead of ~100 %, for a lag nobody has yet shown to be visible.
//  2. `dilution_factors` is called **twice** per evaluation (air at receptor
//     height, ground at `z = 0`) and the ground set exists only for deposition.
//     If deposition is not on screen, that is half the cost.
//  3. The ring walk is `O(steps x puffs x receptors)` of the same Gaussian kernel
//     the field already runs on the GPU at 83x the CPU rate. It has never been
//     dispatched there.

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
/// feels to a hand on the slider. ~~`0.1 s` is the 10 Hz the Map tab
/// targets.~~ **CHANGED 2026-09-27 to 60 fps** — see the section below.
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
///
/// # ~~0.1 s (10 Hz)~~ ~~60 fps~~ -> **back to 0.1 s (10 Hz)**, 2026-09-27
///
/// **Maintainer direction**, in two steps on the same day: *"I want you to do at
/// least a 30 fps, ideally 60 fps, otherwise update map at 10 Hz just like rest of
/// the plant"*, then, after seeing it run at 60 fps, *"can you change map field to
/// 10 Hz"*.
///
/// So 10 Hz is the shipped cadence — the fallback the first direction named, taken
/// deliberately rather than because 60 fps was unaffordable. **It is affordable**:
/// the measurements below stand, and the field uses 4.4 % of a 60 fps frame. The
/// choice is the maintainer's and is about how the map should feel next to a plant
/// that ticks at 10 Hz, not about cost.
///
/// Consequence worth knowing: the live ring column
/// ([`ReceptorResult::instantaneous_chi_over_q`]) is refreshed beside the field, so
/// it follows this constant too — it is now a 10 Hz readout, not a 60 fps one. It
/// remains far more responsive than the time-integrated column on
/// [`DISPERSION_EVALUATION_INTERVAL_S`]'s 2 s interval, which is the distinction
/// that matters.
///
/// 60 fps is affordable, and by a wide margin, because the field is on the GPU.
/// Measured with `cargo plume-timing` at the shipped 120-puff instantaneous
/// population:
///
/// | Cells | Evaluations | GPU | Share of a 16.7 ms frame |
/// |---|---|---|---|
/// | **64** (the default) | 0.49 M | **0.67 ms** | **4 %** |
/// | 192 | 4.4 M | 0.91 ms | 5 % |
/// | 256 | 7.9 M | 2.01 ms | 12 % |
/// | 512 | 31.5 M | 9.09 ms | 54 % |
///
/// So even the 512-cell ceiling fits a 60 fps frame, and the default fits it
/// twenty-five times over. The live ring sample refreshed alongside it adds
/// 2 880 kernel evaluations against the field's 491 520 — noise.
///
/// # What is NOT on this clock, and why the throttle next door stays
///
/// This constant governs the **field only** — [`Self::refresh_field`], which
/// calls [`AtmosphericDispersionChannel::compute_field`]. It does **not** govern
/// the receptor ring's time-integrated `chi/Q` or the activity survey: those live
/// in `evaluate` behind [`DISPERSION_EVALUATION_INTERVAL_S`], and
/// `tests::what_one_dispersion_evaluation_costs` measures one full evaluation at
/// **26.819 ms** — within noise of the steam generator's 26.46 ms, which is
/// 100.12 % of the plant step. At 60 fps that does not fit in a frame at all, and
/// at 10 Hz it would be 27 % of every tick.
///
/// That is why the maintainer's "take the throttle out" could only be carried out
/// for **half** of the dispersion side. The expensive half stays throttled, and
/// the reason is a measurement rather than the (now struck) claim that used to
/// justify it.
///
/// `0.1` matches [`crate::physics::PLANT_TIMESTEP_S`] in value but is deliberately
/// **not** written as that constant: this one is **wall-clock** and the plant's is
/// **plant time**. They coincide at a 1:1 real-time ratio and diverge the moment the
/// plant is paused or fast-forwarded, which is the whole reason the field is on its
/// own clock. Tying them together would silently freeze the map on a paused plant.
pub const FIELD_REFRESH_INTERVAL_S: f64 = 0.1;

/// ~~Everything a cached field depends on.~~ **What has MOVED since the field
/// was last drawn** -- CORRECTED 2026-09-27.
///
/// Equality on this is the cache test: unequal means redraw.
///
/// # It is no longer a content key, and the distinction now matters
///
/// ~~"Everything a cached field depends on."~~ That was true while the field
/// was a closed form in `(meteorology, clock)`. It is **false** now that the
/// puff population is marched: the field depends on the whole **history** of
/// the wind, which no fixed-size key can carry. Two runs reaching plume time
/// `t` with the same current wind through *different* wind histories have
/// genuinely different plumes, and that is the point of the change (see
/// [`AtmosphericDispersionChannel::advance_population`]).
///
/// So this struct's job narrowed: it answers *"has anything changed that should
/// trigger a redraw?"*, not *"what field does this key produce?"*. It is still
/// sufficient for that, because the population's own clock is one of its
/// fields and the clock advances whenever the population does.
///
/// **Do not reintroduce a cache that reuses a field across a key match as
/// though the key determined it** -- with a marched population that would be
/// reusing a plume from a different history.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FieldKey {
    /// The wind and stability in force. In the key because a slider move must
    /// redraw, not because it determines the field.
    meteorology: Meteorology,
    /// Cells per side actually evaluated (after the [`max_grid_cells`] clamp,
    /// not as requested -- two requests that clamp to the same number share a
    /// field, correctly).
    cells: usize,
    /// The clock the **marched population** stands at \[s\]; see
    /// [`AtmosphericDispersionChannel::population_clock_s`].
    plume_time_s: f64,
}

/// One live puff of the map field's **Lagrangian** population.
///
/// # Why the channel carries puff state at all
///
/// Reported by the maintainer 2026-09-27: *"when puff particles go around, and
/// the wind direction changes, the puffs don't seem to remember their last
/// known location. You should treat the puff particles like in a lagrangian
/// fashion, where the particles DO remember their last position."*
///
/// They did not, and the reason was structural. The field used to be built from
/// a closed form that read the **current** wind and applied it to every puff's
/// **whole age**: `x = u_now * age`. So turning the wind slider did not turn the
/// plume, it *rewrote the plume's history* -- a 1190 s-old puff 3.5 km downwind
/// was instantly repositioned as though it had flown the new bearing since
/// birth, and the entire plume snapped rigidly about the stack. Nothing was
/// remembered because nothing was stored; position was a function of the
/// present.
///
/// A puff is now a parcel with its own state, marched forward one step at a
/// time. A wind change alters only what happens next. This is the same fix, and
/// the same reasoning, as `changi::puff::simulate::AdvectionPolicy`, whose
/// default this mirrors -- see that enum's documentation for the physics,
/// including why the dispersion distance must be **path length** and not the
/// straight-line distance from the stack.
///
/// # What it cost
///
/// The closed form had two properties this does not, and both are genuinely
/// gone rather than worked around:
///
/// - ~~"the plume clock may be jumped an hour ahead and still be exact rather
///   than extrapolated"~~ -- a model with history cannot be evaluated at an
///   arbitrary time without running the history. A forward jump is now marched
///   under the **current** wind, which is exact only if the wind held over the
///   jumped interval. Stated where the operator meets it.
/// - ~~"a two-hour fast-forward costs exactly what the first minute does"~~ --
///   it now costs the march. Measured cheap: the population is capped at
///   `puff_duration / puff_dt` = 120 puffs and a step is six flops per puff, so
///   an hour of catch-up at `sim_dt` = 10 s is 360 steps x 120 puffs ~ 260 k
///   flops -- far below the field evaluation it feeds, which is 65 536 cells x
///   120 puffs.
#[derive(Debug, Clone, Copy)]
struct FieldPuff {
    /// Eastward displacement from the stack \[m\], integrated step by step.
    dx_m: f64,
    /// Northward displacement from the stack \[m\], integrated step by step.
    dy_m: f64,
    /// **Path length** travelled \[m\] -- the Pasquill-Gifford argument. Not
    /// `hypot(dx, dy)`: on a bent trajectory they differ, and the chord
    /// under-reports how far the puff has dispersed.
    path_m: f64,
    /// Age \[s\], for retirement at `puff_duration`.
    age_s: f64,
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
    /// The live **Lagrangian** puff population the map field is drawn from.
    ///
    /// This is the channel's only genuinely path-dependent state: everything
    /// else here is either an input or a cache of a pure function. See
    /// [`FieldPuff`] for why it exists and [`Self::advance_population`] for how
    /// it is marched.
    puffs: Vec<FieldPuff>,
    /// The plume clock \[s\] the population in [`Self::puffs`] stands at.
    population_clock_s: f64,
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
            puffs: Vec::new(),
            population_clock_s: 0.0,
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
        // Bring the population onto the clock before the field is drawn from
        // it. `evaluate` is `&self` and deliberately stays so -- it renders the
        // population, it does not advance it.
        let config = self.run_config();
        self.advance_population(&config, self.plume_time_s(sim_time_s));
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
        // The rate limit is checked FIRST, before the population is marched.
        // Skipping a refresh must not skip plume time: the next call marches the
        // whole interval instead, so the plume stays on the clock even when the
        // field is drawn less often than 10 Hz. See
        // `Self::advance_population` on the wind this catch-up uses.
        let now = std::time::Instant::now();
        if let Some(last) = self.last_field_refresh {
            if now.duration_since(last).as_secs_f64() < FIELD_REFRESH_INTERVAL_S {
                return false;
            }
        }
        let config = self.run_config();
        self.advance_population(&config, self.plume_time_s(sim_time_s));
        // Keyed on the population's own clock, not the requested plume time:
        // the march lands on a multiple of `sim_dt`, and it is what was drawn.
        let key = FieldKey {
            meteorology: self.meteorology,
            cells: self.field_cells(),
            plume_time_s: self.population_clock_s,
        };
        if self.grid_is_current_for(&key) {
            return false;
        }

        let grid = self.compute_field(&config, &key);
        self.grid_cache = Some(grid.clone());
        self.grid_key = Some(key);
        // Stamped AFTER the computation, not before -- CORRECTED 2026-09-27.
        //
        // ~~`self.last_field_refresh = Some(now)`~~ where `now` was captured
        // before `compute_field`. That made the interval a "no more than one
        // field STARTED per 100 ms" rule, which degenerates to back-to-back
        // computation as soon as one takes longer than the interval -- and one
        // does: the GPU adapter probe plus the first field was **measured at
        // 423 ms** on 2026-09-27 (once per process,
        // `changi::puff::wgsl::has_gpu_field`). The limit then failed to limit
        // exactly when it mattered most.
        //
        // It also made `tests::changing_the_meteorology_forces_one_refresh_then_the_rate_limit_holds`
        // **order-dependent**: whichever test in the binary paid the probe had
        // its rate limit already expired by the following call, so the test
        // passed or failed on which tests ran before it. It failed when run
        // alone and passed in a full run.
        //
        // Measuring from COMPLETION gives the genuine frame-interval gap the
        // constant's doc describes -- "keeps a dragged slider from triggering a
        // field evaluation per frame" -- and makes the test independent of host
        // speed. Since 2026-09-27 that interval is one 60 fps frame, not 100 ms.
        self.last_field_refresh = Some(std::time::Instant::now());
        // So a snapshot written before the next `update()` picks up the fresh
        // field rather than the one `latest` was built with.
        //
        // The LIVE ring sample is refreshed here TOO, and that is a bug fix --
        // CORRECTED 2026-09-27. `instantaneous_chi_over_q_at_ring` was computed
        // only inside `collect`, which runs in `evaluate`, which is on
        // `DISPERSION_EVALUATION_INTERVAL_S`'s 60 s throttle. So the column
        // documented and advertised as *live* refreshed once a plant minute --
        // the very thing it was added to stop. Recomputing it beside the field is
        // what actually makes it live, because the field's clock is the fast one.
        //
        // It costs 24 receptors x at most 120 puffs = 2 880 kernel evaluations
        // against the field's 491 520, so it rides along for free.
        //
        // Sampled BEFORE taking the `&mut` on `latest`: it reads `self`
        // immutably, and the borrow checker will not allow both at once.
        let live = self.instantaneous_chi_over_q_at_ring(&config);
        if let Some(latest) = &mut self.latest {
            latest.grid = grid;
            for (slot, value) in latest.receptors.iter_mut().zip(live.iter()) {
                slot.instantaneous_chi_over_q = *value;
            }
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
            plume_time_s: self.population_clock_s,
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
            // The Lagrangian default, explicitly. The receptor ring goes
            // through `changi::activity::dilution_factors`, which reads this;
            // the map field is marched by `Self::advance_population`, which
            // implements the same policy on this channel's own population. The
            // two must not disagree about whether a puff turns.
            advection: changi::puff::simulate::AdvectionPolicy::LagrangianTrajectory,
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

    /// March the Lagrangian puff population forward to `target_s` on the plume
    /// clock, emitting and retiring as it goes.
    ///
    /// # The ordering, and why it is advect-then-emit
    ///
    /// Each step advects every live puff by `sim_dt` on the wind currently in
    /// force, **then** emits any puff due at the new clock (at the stack, with
    /// zero travel), **then** retires anything past `puff_duration`. A puff
    /// emitted `k` steps ago has therefore been advected exactly `k` times, so
    /// on a steady wind its displacement is `u * age` -- identical to the closed
    /// form this replaced, and to `changi::puff::simulate`'s own loop, which
    /// uses the same ordering for the same reason.
    ///
    /// # The wind used is the CURRENT wind, and that is a real approximation
    ///
    /// This channel holds one wind, not a time series: the operator's slider is
    /// a step function and nothing here records where it has been. So a catch-up
    /// march covering an interval during which the wind moved applies the
    /// *latest* wind to the whole of it. At the 10 Hz refresh
    /// ([`FIELD_REFRESH_INTERVAL_S`]) that interval is a tenth of a second and
    /// the error is negligible; across a deliberate plume-clock jump (see
    /// [`MapFieldRequest::plume_clock_offset`]) it is not, and the jump is
    /// therefore an extrapolation under "the wind holds", not a prediction.
    /// **Said plainly rather than hidden**, because the closed form it replaced
    /// was exact for the jump and a reader may remember that it was.
    ///
    /// # Rewinding CLEARS the population, because history is not stored
    ///
    /// If `target_s` is behind the population's clock the march cannot be
    /// undone -- the trajectory integral is not invertible and no per-step
    /// history is kept. The population is reset to empty at `t = 0` and
    /// re-marched. That is honest: after a rewind there is no remembered past,
    /// and re-marching under the current wind reproduces exactly what the old
    /// closed form would have drawn.
    ///
    /// Marching is *not* rate-limited here; the caller's rate limit decides how
    /// often this runs, and skipping a call only makes the next march longer.
    /// See [`FieldPuff`] for the measured cost.
    fn advance_population(&mut self, config: &RunConfig, target_s: f64) {
        let dt = config.sim_dt.get::<second>();
        let puff_dt = config.puff_dt.get::<second>();
        let lifetime = config.puff_duration.get::<second>();
        if dt <= 0.0 || puff_dt <= 0.0 {
            return;
        }
        let target = target_s.max(0.0);

        // A rewind is a reset -- see the doc comment.
        if target + 1e-9 < self.population_clock_s {
            self.puffs.clear();
            self.population_clock_s = 0.0;
        }
        // Seed the t = 0 emission once, so the population is never empty at a
        // clock the plume has reached.
        if self.puffs.is_empty() && self.population_clock_s == 0.0 {
            self.puffs.push(FieldPuff {
                dx_m: 0.0,
                dy_m: 0.0,
                path_m: 0.0,
                age_s: 0.0,
            });
        }

        let components =
            wind_vector_convert(self.meteorology.speed, self.meteorology.direction_from);
        let u = components.u.get::<meter_per_second>();
        let v = components.v.get::<meter_per_second>();
        let step_dx = u * dt;
        let step_dy = v * dt;
        // The length of THIS leg, accumulated. Recomputing it from the endpoints
        // is what would turn the path back into a chord.
        let leg = step_dx.hypot(step_dy);

        while self.population_clock_s + dt <= target + 1e-9 {
            for p in self.puffs.iter_mut() {
                p.dx_m += step_dx;
                p.dy_m += step_dy;
                p.path_m += leg;
                p.age_s += dt;
            }
            self.population_clock_s += dt;
            if (self.population_clock_s % puff_dt).abs() < 1e-9
                || (self.population_clock_s % puff_dt - puff_dt).abs() < 1e-9
            {
                self.puffs.push(FieldPuff {
                    dx_m: 0.0,
                    dy_m: 0.0,
                    path_m: 0.0,
                    age_s: 0.0,
                });
            }
            // `<=`, not `<`: the puff at age exactly `puff_duration` is kept,
            // which is what makes the settled count `puff_duration / puff_dt`
            // rather than that plus one. See
            // `tests::the_instantaneous_population_is_capped_by_the_puff_lifetime`.
            self.puffs.retain(|p| p.age_s <= lifetime);
        }
    }

    /// The flattened puff states the map field sums over -- **the marched
    /// Lagrangian population as it stands.**
    ///
    /// # Read from state, no longer written down in closed form
    ///
    /// ~~"For a **constant** wind -- which is what this simulator has, and says
    /// so -- a puff's whole history is closed form: a puff emitted at `t_j` is,
    /// at time `t`, at `(u, v) * (t - t_j)` with dispersion set by the distance
    /// `|U| * (t - t_j)` it has travelled. So the population can be written down
    /// directly at any `t`, with no time-marching and no state carried between
    /// calls -- which is also why the plume clock may be jumped an hour ahead
    /// and still be exact rather than extrapolated."~~
    /// **CORRECTED 2026-09-27.** The premise was the problem: the wind is *not*
    /// constant, it is an operator input that moves, and reading the current
    /// wind into every puff's whole age is what made the plume snap rigidly
    /// around the stack when the slider turned. The population is now marched
    /// ([`Self::advance_population`]) and this function only *reads* it. See
    /// [`FieldPuff`] for the full account and for what the closed form bought
    /// that this does not.
    ///
    /// What is unchanged: the expensive per-puff work -- the branchy
    /// Pasquill-Gifford table walk -- still happens **once per state** rather
    /// than once per (state, cell), and what crosses to the GPU is still pure
    /// arithmetic.
    ///
    /// The population is capped at `puff_duration / puff_dt` = **120** states at
    /// the shipped configuration; the newest emission has zero travel, so it has
    /// no sigma and is dropped here rather than being retired from the
    /// population.
    ///
    /// Weights are per **unit release rate**: a puff carries `Q * puff_dt` of
    /// mass, so at unit `Q` the weight is `puff_dt` \[s\] and the sum is
    /// `chi/Q` in s/m^3 -- independent of the source, exactly as the receptor
    /// ring's `chi/Q` is.
    fn field_states(&self, config: &RunConfig) -> Vec<changi::puff::wgsl::PuffState> {
        use changi::puff::wgsl::PuffState;

        let puff_dt = config.puff_dt.get::<second>();
        let class = self.stability_for_field();
        let mut states = Vec::with_capacity(self.puffs.len());
        for p in &self.puffs {
            // Zero travel is upstream's NA path: a puff that has not moved has
            // no sigma and contributes nothing. The argument is the accumulated
            // PATH, not `hypot(dx, dy)` -- see `FieldPuff::path_m`.
            let Some(sig) = pasquill_gifford_sigmas(class, Length::new::<meter>(p.path_m)) else {
                continue;
            };
            states.push(PuffState {
                x: p.dx_m as f32,
                y: p.dy_m as f32,
                sigma_y: sig.sigma_y.get::<meter>() as f32,
                sigma_z: sig.sigma_z.get::<meter>() as f32,
                // Mass per unit release RATE in one puff, so the sum is
                // `chi/Q` in s/m^3.
                weight: puff_dt as f32,
            });
        }
        states
    }

    /// **Instantaneous `chi/Q` sampled at the receptor-ring positions**, from the
    /// same marched population and the same kernel the map field uses \[s/m^3\].
    ///
    /// Returned in [`Self::receptor_ring`]'s order, so index `i` is the same
    /// receptor the ring's row `i` describes.
    ///
    /// # Why this exists (maintainer direction, 2026-09-27)
    ///
    /// *"Sampling datapoints at those positions in the ring is useful and needs a
    /// live update."*
    ///
    /// The ring's [`ReceptorResult::chi_over_q`] is the **time-integrated**
    /// dilution factor from `changi::activity::dilution_factors`. That is the
    /// right quantity for the activity and deposition columns, but it is
    /// expensive — `O(steps x puffs x receptors)`, run twice (air and ground) —
    /// which is why it sits behind [`DISPERSION_EVALUATION_INTERVAL_S`]'s 60 s
    /// throttle. A number that refreshes once a plant minute is not a live
    /// readout.
    ///
    /// This is the live one. It sums the **instantaneous** field at each
    /// receptor, so it refreshes with the field at
    /// [`FIELD_REFRESH_INTERVAL_S`] (10 Hz of wall clock) rather than on the
    /// ring's throttle.
    ///
    /// # It agrees with the map BY CONSTRUCTION, and that is new
    ///
    /// It calls `changi::puff::wgsl::contribution` over
    /// [`Self::field_states`] — **the same kernel and the same puff population**
    /// the grid is painted from. So a sampled value and the map cell under it are
    /// the same number, computed the same way.
    ///
    /// That matters because [`DispersionGrid`]'s own doc has to warn that a cell
    /// must **not** be compared against a table row: the two carried different
    /// quantities. That warning still stands for the time-integrated column, and
    /// is now **lifted for this one** — which is the point of adding it rather
    /// than speeding the other one up.
    ///
    /// # Cost
    ///
    /// 24 receptors x at most 120 live puffs = **2 880 kernel evaluations**,
    /// against the 0.49 M the 64-cell field already does. It is free at this
    /// scale, so it is computed on the CPU rather than dispatched to the GPU —
    /// a buffer upload for 24 points would cost more than the arithmetic.
    pub fn instantaneous_chi_over_q_at_ring(&self, config: &RunConfig) -> Vec<f64> {
        let states = self.field_states(config);
        let height = self.site.release_height.get::<meter>() as f32;
        self.receptor_ring()
            .iter()
            .map(|r| {
                let (x, y) = (r.x.get::<meter>() as f32, r.y.get::<meter>() as f32);
                states
                    .iter()
                    .map(|st| changi::puff::wgsl::contribution(st, x, y, height) as f64)
                    .sum()
            })
            .collect()
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

        let states = self.field_states(config);
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
        // The LIVE column, sampled from the marched population at the same
        // points. Computed once for the whole ring rather than per row -- it
        // walks `field_states` and there is no reason to rebuild that 24 times.
        let instantaneous = self.instantaneous_chi_over_q_at_ring(&self.run_config());
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
                    instantaneous_chi_over_q: instantaneous.get(index).copied().unwrap_or(0.0),
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

        // Times are DERIVED from the interval, not written as literals --
        // CORRECTED 2026-09-27. This test hardcoded 30 s and 60 s, which encoded
        // the old 60 s throttle, so it went red the moment
        // `DISPERSION_EVALUATION_INTERVAL_S` was changed to 2 s on maintainer
        // direction. What the test means is "half an interval" and "a full
        // interval"; saying so makes it survive the next cadence change, and a
        // failure then would be a real one.
        let interval = DISPERSION_EVALUATION_INTERVAL_S;

        assert!(
            channel.update(0.0, &release),
            "the first call must evaluate"
        );
        assert!(channel.latest().is_some());
        assert!(
            !channel.update(0.5 * interval, &release),
            "half an interval later ({} s) must NOT re-evaluate",
            0.5 * interval
        );
        assert!(
            channel.update(interval, &release),
            "a full interval later ({interval} s) must re-evaluate"
        );

        // Moving the wind forces a re-evaluation, because it is the input the
        // result is most sensitive to. Just after a full interval, so the only
        // thing that could allow it is the meteorology change itself.
        channel.set_meteorology(Meteorology {
            direction_from: Angle::new::<degree>(90.0),
            ..Meteorology::default()
        });
        assert!(
            channel.update(interval + 0.1 * interval, &release),
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
        let mut channel = AtmosphericDispersionChannel::new();
        let config = channel.run_config();
        let puff_dt = config.puff_dt.get::<second>();
        let lifetime = config.puff_duration.get::<second>();
        // `puff_duration / puff_dt`, NOT that plus one: the just-emitted puff
        // has age zero, so it has no sigma and contributes nothing. See the
        // finding in this test's doc comment.
        let cap = (lifetime / puff_dt) as usize;

        let mut previous = 0usize;
        for clock in [0.0, 60.0, 600.0, 1200.0, 3600.0, 7200.0] {
            // Marched, not written down in closed form (2026-09-27). The
            // clocks below ascend, so each is a forward march from the last and
            // no reset is triggered.
            channel.advance_population(&config, clock);
            let states = channel.field_states(&config);
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

    /// **A plume-clock jump equals having run the clock there -- ON A WIND THAT
    /// DID NOT MOVE**, which is now a condition and not a theorem.
    ///
    /// # ~~"the field is a closed form in elapsed time and carries no history"~~
    /// # -- CORRECTED 2026-09-27
    ///
    /// ~~"That equality is the whole justification for the fast-forward button
    /// driving the plume clock rather than the plant (maintainer, 2026-09-25),
    /// so it is asserted rather than argued: the state population is a closed
    /// form in elapsed time, and it carries no history, so `plant t + offset`
    /// and `plant t` with the same total are the same argument."~~
    ///
    /// **The population now carries history** (see [`FieldPuff`]), so that
    /// argument no longer holds in general: two runs reaching plume time 7500 s
    /// through different wind histories have different plumes, correctly. The
    /// equality survives here for a narrower and honestly weaker reason -- both
    /// channels are marched under the **same constant wind** from the same empty
    /// initial population, so their histories are identical and so are their
    /// populations.
    ///
    /// # What this now does and does not license
    ///
    /// It still justifies the fast-forward button *as a display control*: at a
    /// steady wind, jumping the plume clock shows what running the plant there
    /// would have shown. It does **not** license reading a jumped field as a
    /// prediction across a wind change, because the jumped march applies the
    /// current wind to the whole jumped interval -- stated on
    /// [`AtmosphericDispersionChannel::advance_population`] and surfaced to the
    /// operator on the Map tab.
    ///
    /// # Results (2026-09-27)
    ///
    /// Jumped (`plant 300 s` + `7200 s` offset) against run (`7500 s`), default
    /// meteorology: populations equal in length and **bit-identical** state for
    /// state. The equality is exact rather than tolerant because both sides
    /// execute the identical sequence of additions.
    #[test]
    fn a_plume_clock_jump_equals_having_run_the_clock_there_on_a_steady_wind() {
        let mut jumped = AtmosphericDispersionChannel::new();
        jumped.set_field_request(MapFieldRequest {
            plume_clock_offset: Time::new::<second>(7200.0),
            ..MapFieldRequest::default()
        });
        let config = jumped.run_config();

        let mut ran = AtmosphericDispersionChannel::new();
        let plant_t = 300.0;

        assert!((jumped.plume_time_s(plant_t) - 7500.0).abs() < 1e-9);
        jumped.advance_population(&config, jumped.plume_time_s(plant_t));
        ran.advance_population(&config, 7500.0);
        let a = jumped.field_states(&config);
        let b = ran.field_states(&config);
        assert_eq!(a.len(), b.len(), "the populations must be the same size");
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x, y, "a jumped clock must be bit-identical to a run one");
        }
    }

    /// **V&V, and the regression this whole change exists for: turning the wind
    /// must BEND the plume, leaving the puffs already aloft where they are.**
    ///
    /// # What was wrong
    ///
    /// Reported by the maintainer 2026-09-27: *"when puff particles go around,
    /// and the wind direction changes, the puffs don't seem to remember their
    /// last known location."* They did not. The field was built from a closed
    /// form reading the **current** wind and applying it to every puff's whole
    /// age, so a slider move rewrote the plume's history and the entire plume
    /// snapped rigidly about the stack. See [`FieldPuff`].
    ///
    /// # Methodology
    ///
    /// One channel at 3 m/s, `sim_dt` 10 s, class held fixed so the stability
    /// lookup cannot confound the geometry.
    ///
    /// 1. Wind **from the north** for 600 s. A met-convention north wind blows
    ///    *towards* the south, so the plume must run south: `dy < 0`, `dx = 0`.
    /// 2. Wind **from the west** for a further 600 s. Now the plume must run
    ///    east: everything emitted from here on goes `+x`.
    ///
    /// The puff of interest is the **oldest survivor** -- the one emitted at
    /// `t = 0`, aged 1200 s at the end, exactly at the retirement bound.
    ///
    /// Predicted before measuring: at 3 m/s each leg is `3 * 600 = 1800 m`, so a
    /// Lagrangian parcel ends at `(dx, dy) = (+1800, -1800)` with a **path of
    /// 3600 m** and a net displacement of 2546 m. The old behaviour would have
    /// put it at `(+3600, 0)` -- 1800 m due east of where it is, with the entire
    /// southward leg erased.
    ///
    /// # Results (measured 2026-09-27)
    ///
    /// | Quantity | Measured | Old behaviour |
    /// |---|---|---|
    /// | oldest puff `dx` | **+1800 m** | +3600 m |
    /// | oldest puff `dy` | **-1800 m** | 0 m |
    /// | its path length | **3600 m** | 3600 m (chord) |
    /// | its net displacement | 2546 m | 3600 m |
    ///
    /// The two differ by **1800 m**, a quarter of the grid's full width at the
    /// shipped [`GRID_HALF_WIDTH_M`].
    ///
    /// **Interpretation.** The plume is a dog-leg, which is what a veering wind
    /// physically produces, and the southward leg is *remembered* rather than
    /// recomputed. The dispersion distance is the 3600 m path, so the bend does
    /// not reset the spread -- a puff that has been aloft for 1200 s is drawn as
    /// widely dispersed wherever it happens to be. This checks the **wiring and
    /// the memory**, not the dispersion coefficients, which are `changi`'s and
    /// are verified against upstream R separately.
    #[test]
    fn turning_the_wind_bends_the_plume_and_the_puffs_keep_their_positions() {
        let mut channel = AtmosphericDispersionChannel::new();
        channel.set_meteorology(Meteorology {
            speed: Velocity::new::<meter_per_second>(3.0),
            direction_from: Angle::new::<degree>(0.0),
            stability: StabilitySource::Fixed(StabilityClass::B),
            hour: 12,
        });
        let config = channel.run_config();

        // Leg 1: wind FROM the north, so the plume runs SOUTH.
        channel.advance_population(&config, 600.0);
        let after_leg_1 = *channel
            .puffs
            .iter()
            .max_by(|a, b| a.age_s.partial_cmp(&b.age_s).unwrap())
            .expect("the population must not be empty after 600 s");
        println!(
            "after leg 1 (wind FROM north, 600 s): oldest puff dx = {:.1} m, dy = {:.1} m, \\
             path = {:.1} m, age = {:.0} s",
            after_leg_1.dx_m, after_leg_1.dy_m, after_leg_1.path_m, after_leg_1.age_s
        );
        assert!(
            after_leg_1.dy_m < -1.0 && after_leg_1.dx_m.abs() < 1e-9,
            "a wind FROM the north must carry the plume due SOUTH; got dx = {}, dy = {}",
            after_leg_1.dx_m,
            after_leg_1.dy_m
        );
        let southward = after_leg_1.dy_m;

        // Leg 2: wind FROM the west, so new travel runs EAST.
        channel.set_meteorology(Meteorology {
            direction_from: Angle::new::<degree>(270.0),
            ..channel.meteorology()
        });
        channel.advance_population(&config, 1200.0);
        let oldest = *channel
            .puffs
            .iter()
            .max_by(|a, b| a.age_s.partial_cmp(&b.age_s).unwrap())
            .expect("the population must not be empty after 1200 s");
        println!(
            "after leg 2 (wind FROM west,  600 s): oldest puff dx = {:.1} m, dy = {:.1} m, \\
             path = {:.1} m, net = {:.1} m, age = {:.0} s\\n  \\
             the OLD behaviour would have put it at dx = 3600.0 m, dy = 0.0 m",
            oldest.dx_m,
            oldest.dy_m,
            oldest.path_m,
            oldest.dx_m.hypot(oldest.dy_m),
            oldest.age_s
        );

        // THE assertion: the southward leg is remembered, not rewritten.
        assert!(
            (oldest.dy_m - southward).abs() < 1e-9,
            "the puff must KEEP the southward displacement it had when the wind turned: \\
             was {southward} m, is now {} m",
            oldest.dy_m
        );
        assert!((oldest.dx_m - 1800.0).abs() < 1e-6, "dx = {}", oldest.dx_m);
        assert!((oldest.dy_m + 1800.0).abs() < 1e-6, "dy = {}", oldest.dy_m);
        // Path, not chord: the bend does not shorten how far it has dispersed.
        assert!(
            (oldest.path_m - 3600.0).abs() < 1e-6,
            "path = {}, wanted 3600 m",
            oldest.path_m
        );
        assert!(
            oldest.path_m > oldest.dx_m.hypot(oldest.dy_m) + 1000.0,
            "on a right-angle dog-leg the path {} must exceed the net displacement {} \\
             by a wide margin",
            oldest.path_m,
            oldest.dx_m.hypot(oldest.dy_m)
        );
        // And it is nowhere near where the old code would have drawn it.
        assert!(
            (oldest.dx_m - 3600.0).abs() > 1000.0,
            "the puff must not have been swept onto the new bearing for its whole life"
        );
    }

    /// **V&V: the LIVE ring sample agrees with the map cell under the same
    /// point** — which is the whole reason the live column exists.
    ///
    /// # Methodology
    ///
    /// Maintainer direction 2026-09-27: *"sampling datapoints at those positions
    /// in the ring is useful and needs a live update."* The live column is
    /// [`AtmosphericDispersionChannel::instantaneous_chi_over_q_at_ring`], which
    /// sums `changi::puff::wgsl::contribution` over
    /// [`AtmosphericDispersionChannel::field_states`] — the same kernel and the
    /// same marched population the grid is painted from. If that is true, a
    /// sampled receptor and the grid cell containing it must be the same number
    /// up to the grid's own discretisation.
    ///
    /// Wind from the north at 3 m/s, class B held fixed, population marched to
    /// 600 s. For the receptor at bearing 180 (due south, straight downwind) at
    /// the innermost ring distance: take the live sample, take the grid cell
    /// whose centre is nearest that receptor, and compare.
    ///
    /// Pass criteria:
    /// - the downwind sample is strictly positive (the plume is there at all);
    /// - the upwind sample at bearing 0 is **orders below** it, so the sampler is
    ///   reading a direction and not a constant;
    /// - the downwind sample agrees with the nearest cell **within a factor of
    ///   3**. Not tighter, and the reason is physical rather than a fudge: at the
    ///   64-cell default a cell is ~39 m across, `sigma_y` near the stack is
    ///   smaller than that, and `chi/Q` falls steeply across one cell — so cell
    ///   centre and receptor position are genuinely different points on a steep
    ///   function. A tight tolerance here would be asserting the grid is finer
    ///   than it is.
    ///
    /// # Results (measured 2026-09-27)
    ///
    /// | Quantity | Value |
    /// |---|---|
    /// | downwind receptor | bearing 180, 100 m, at `(0.0, -100.0) m` |
    /// | live sample | **2.112412e-6 s/m^3** |
    /// | nearest grid cell | **1.395244e-6 s/m^3** (64 cells, ~39.1 m per cell) |
    /// | ratio | **1.5140** |
    /// | upwind receptor, bearing 0 | **2.132944e-16 s/m^3** |
    ///
    /// The ratio of 1.51 across one 39 m cell is what a steep function sampled at
    /// two nearby-but-different points looks like, not a disagreement about the
    /// quantity — and the upwind sample being **ten orders** below the downwind
    /// one is what shows the sampler reads a direction rather than a constant.
    ///
    /// What this establishes is the *agreement*, not the values: the point is that
    /// the table and the map stopped being two different quantities for this
    /// column.
    ///
    /// **Interpretation.** `DispersionGrid`'s standing warning that a cell must
    /// not be compared against a table row applies to the **time-integrated**
    /// column and still does. It is lifted for this one, and this test is what
    /// makes that claim checkable rather than asserted.
    #[test]
    fn the_live_ring_sample_agrees_with_the_field_cell_under_it() {
        let mut channel = AtmosphericDispersionChannel::new();
        channel.set_meteorology(Meteorology {
            speed: Velocity::new::<meter_per_second>(3.0),
            direction_from: Angle::new::<degree>(0.0),
            stability: StabilitySource::Fixed(StabilityClass::B),
            hour: 12,
        });
        let config = channel.run_config();
        channel.advance_population(&config, 600.0);

        let ring = channel.receptor_ring();
        let live = channel.instantaneous_chi_over_q_at_ring(&config);
        assert_eq!(live.len(), ring.len());

        let key = FieldKey {
            meteorology: channel.meteorology(),
            cells: channel.field_cells(),
            plume_time_s: 600.0,
        };
        let grid = channel.compute_field(&config, &key);

        // Nearest-cell lookup, from the grid's own geometry: row 0 is the
        // NORTHERNMOST row (north-up), so northing decreases with row.
        let cell_at = |x: f64, y: f64| -> f64 {
            let span = 2.0 * grid.half_width_m;
            let fx = (x + grid.half_width_m) / span;
            let fy = (grid.half_width_m - y) / span;
            let col = ((fx * grid.cells as f64).floor() as isize).clamp(0, grid.cells as isize - 1);
            let row = ((fy * grid.cells as f64).floor() as isize).clamp(0, grid.cells as isize - 1);
            grid.at(col as usize, row as usize).unwrap_or(0.0)
        };

        // The innermost ring, due south (downwind of a north wind) and due north.
        //
        // The index is DERIVED from the ring's own construction order rather than
        // searched for: `receptor_ring` loops `for distance in
        // RECEPTOR_DISTANCES_M { for sector in 0..RECEPTOR_SECTORS }`, so
        // `index = distance_index * RECEPTOR_SECTORS + sector` and
        // `bearing = 360 * sector / RECEPTOR_SECTORS`. Deriving it keeps this test
        // honest if the ring is ever renumbered -- it would fail to compile or
        // fail loudly rather than silently sample a different point.
        let innermost = RECEPTOR_DISTANCES_M[0];
        assert!(
            RECEPTOR_SECTORS % 2 == 0,
            "this test picks the due-south sector as RECEPTOR_SECTORS/2, which needs an even count"
        );
        let up = 0; // distance index 0, sector 0 -> bearing 0, due north
        let down = RECEPTOR_SECTORS / 2; // sector at bearing 180, due south

        let (dx, dy) = (
            ring[down].x.get::<meter>(),
            ring[down].y.get::<meter>(),
        );
        let cell = cell_at(dx, dy);
        println!(
            "LIVE RING SAMPLE vs FIELD CELL (wind from north 3 m/s, class B, t = 600 s)\n  \
             downwind receptor  bearing 180, {innermost:.0} m at ({dx:.1}, {dy:.1}) m\n    \
             live sample = {:.6e} s/m^3\n    \
             nearest cell = {:.6e} s/m^3   (grid {} cells, ~{:.1} m per cell)\n    \
             ratio        = {:.4}\n  \
             upwind receptor    bearing 0\n    \
             live sample = {:.6e} s/m^3",
            live[down],
            cell,
            grid.cells,
            2.0 * grid.half_width_m / grid.cells as f64,
            if cell > 0.0 { live[down] / cell } else { f64::INFINITY },
            live[up],
        );

        assert!(
            live[down] > 0.0,
            "the downwind receptor must see something; got {}",
            live[down]
        );
        assert!(
            live[up] < live[down],
            "a north wind must give less upwind ({}) than downwind ({})",
            live[up],
            live[down]
        );
        assert!(
            cell > 0.0,
            "the grid cell containing the downwind receptor must be populated"
        );
        let ratio = live[down] / cell;
        assert!(
            (1.0 / 3.0..=3.0).contains(&ratio),
            "the live sample and the cell under it must be the same quantity: ratio {ratio:.4} \
             (live {:.4e} against cell {:.4e}). See this test's doc on why the bound is a \
             factor of 3 and not tighter.",
            live[down],
            cell
        );
    }

    /// **V&V: the map field, plus the live ring sample refreshed with it, fits
    /// inside one 60 fps frame** — the gate for
    /// [`FIELD_REFRESH_INTERVAL_S`]'s cadence.
    ///
    /// # This measures HEADROOM, not the shipped cadence
    ///
    /// [`FIELD_REFRESH_INTERVAL_S`] is **0.1 s (10 Hz)** as shipped — the
    /// maintainer set it to 60 fps and then back to 10 Hz on 2026-09-27, the
    /// fallback their own direction named. So this test no longer gates the
    /// cadence; it gates the *cost*, against frame budgets the field is not
    /// currently asked to meet.
    ///
    /// That is still worth having, and is the reason it was not deleted with the
    /// 60 fps setting: it is the evidence that raising the cadence is a free
    /// decision rather than a performance question, and it is what would catch a
    /// future change making the field expensive enough that 10 Hz stops being
    /// comfortable. A test that only checked the shipped 100 ms budget would pass
    /// with a 99 ms field and tell nobody anything.
    ///
    /// # Methodology
    ///
    /// Times [`AtmosphericDispersionChannel::refresh_field`]'s actual work — the
    /// population march, [`AtmosphericDispersionChannel::compute_field`], and the
    /// live ring sample that now rides along with it — at the shipped resolution.
    /// Five samples, median, after a warm-up absorbing the one-off GPU adapter
    /// probe (423 ms, once per process, not a per-frame cost).
    ///
    /// Pass criterion: the median is under **one 30 fps frame (33.3 ms)**, which
    /// is the maintainer's stated *minimum*. The 60 fps target (16.7 ms) is
    /// reported and not asserted, deliberately: this runs on whatever host CI or
    /// a laptop provides, and a hard 60 fps gate would be the same
    /// load-sensitive mistake already corrected once today in
    /// `tampines`' exchanger timing test. The fallback the maintainer named —
    /// 10 Hz — is what a host that misses 60 fps degrades to, and it degrades by
    /// simply refreshing less often, with the last field left on screen.
    ///
    /// # Results
    ///
    /// Printed. Compare against the 26.8 ms of a FULL evaluation
    /// ([`tests::what_one_dispersion_evaluation_costs`]), which is what stays
    /// behind [`DISPERSION_EVALUATION_INTERVAL_S`] precisely because it does not
    /// fit a frame.
    #[test]
    fn the_map_field_fits_inside_a_sixty_fps_frame() {
        const FRAME_60_MS: f64 = 1000.0 / 60.0;
        const FRAME_30_MS: f64 = 1000.0 / 30.0;

        let mut channel = AtmosphericDispersionChannel::new();
        let config = channel.run_config();
        let key = FieldKey {
            meteorology: channel.meteorology(),
            cells: channel.field_cells(),
            plume_time_s: 600.0,
        };
        channel.advance_population(&config, 600.0);
        // Warm-up: the GPU adapter probe and the first allocation.
        let _ = channel.compute_field(&config, &key);

        let mut samples = Vec::new();
        for i in 0..5 {
            let target = 600.0 + i as f64 * FIELD_REFRESH_INTERVAL_S;
            let t = std::time::Instant::now();
            channel.advance_population(&config, target);
            let grid = channel.compute_field(&config, &key);
            let live = channel.instantaneous_chi_over_q_at_ring(&config);
            samples.push(t.elapsed().as_secs_f64() * 1.0e3);
            assert_eq!(grid.cells, key.cells);
            assert_eq!(live.len(), RECEPTOR_COUNT);
        }
        samples.sort_by(f64::total_cmp);
        let median = samples[samples.len() / 2];

        println!(
            "MAP FIELD FRAME BUDGET ({} cells, GPU path: {})\n  \
             samples (ms) = {:?}\n  \
             median       = {median:.3} ms  =  {:.1} % of a 60 fps frame ({FRAME_60_MS:.1} ms), \
             {:.1} % of a 30 fps frame\n  \
             for contrast, ONE FULL evaluation (ring + activity + field) = ~26.8 ms, \
             which is {:.0} % of a 60 fps frame -- it does NOT fit, and stays throttled",
            key.cells,
            changi::puff::wgsl::has_gpu_field(),
            samples
                .iter()
                .map(|v| (v * 1000.0).round() / 1000.0)
                .collect::<Vec<_>>(),
            100.0 * median / FRAME_60_MS,
            100.0 * median / FRAME_30_MS,
            100.0 * 26.8 / FRAME_60_MS,
        );

        assert!(
            median < FRAME_30_MS,
            "the map field takes {median:.3} ms, over one 30 fps frame ({FRAME_30_MS:.1} ms), \
             which is the maintainer's stated MINIMUM. FIELD_REFRESH_INTERVAL_S is set to \
             60 fps and cannot be honoured on this host -- either lower the resolution \
             ceiling (max_grid_cells) or set the interval back to the 10 Hz fallback."
        );
    }

    /// **MEASUREMENT: what one full dispersion evaluation actually costs**, so the
    /// 60 s throttle is a decision about a measured number rather than a
    /// recollection.
    ///
    /// # Why this had to be measured before the throttle could be touched
    ///
    /// Three numbers were in circulation on 2026-09-27 and no two agreed:
    ///
    /// - `DISPERSION_EVALUATION_INTERVAL_S`'s own (now struck) doc: the ring is
    ///   *"far and away the most expensive thing this simulator would do per
    ///   step"*.
    /// - `tests::where_the_plant_step_spends_its_time`'s 2026-09-25 record:
    ///   **0.61 %** of the plant step.
    /// - The same test re-run 2026-09-27: **0.00 %**, but *"over 1 calls"* — and
    ///   `update` returns early when the release channel is empty, so that may
    ///   have measured a call that never ran the model at all.
    ///
    /// 0.61 % of a plant step, at one call per 60 s of plant time, back-solves to
    /// roughly **96 ms per call** — which, run every 0.1 s tick instead, would
    /// more than double the plant step. Or it is ~0 and the throttle costs
    /// nothing to remove. Those are opposite conclusions from the same evidence,
    /// so neither can be acted on.
    ///
    /// # Methodology
    ///
    /// Time [`AtmosphericDispersionChannel::evaluate`] directly, with a
    /// **populated** release channel (so the early return cannot be mistaken for
    /// a cheap evaluation), at the shipped configuration. Five calls, median
    /// reported, after one warm-up that absorbs the one-off GPU adapter probe —
    /// measured separately at 423 ms and a startup cost, not a per-call one.
    ///
    /// `evaluate` is the whole job: two `dilution_factors` runs (air at receptor
    /// height, ground at `z = 0`), the activity survey, and the map field.
    ///
    /// Asserts only that the call is **under 100 ms**, the plant tick — the
    /// threshold above which removing the throttle would visibly cost real-time
    /// ratio. It is a decision input, not a performance target, and it is
    /// deliberately loose because the absolute number is host-specific while the
    /// comparison against the tick is what the decision turns on.
    ///
    /// # Results
    ///
    /// Printed by this test. Compare against the steam generator's measured
    /// **26.46 ms per call**, which is 100.12 % of the plant step.
    #[test]
    fn what_one_dispersion_evaluation_costs() {
        let mut channel = AtmosphericDispersionChannel::new();
        let release = release_at(1100.0);
        assert!(
            !release.latest().is_empty(),
            "the release channel must be POPULATED, or `evaluate` measures an early return"
        );

        // Warm-up: absorbs the one-off GPU adapter probe (423 ms, once per
        // process) and the first field allocation.
        let _ = channel.evaluate(0.0, &release);

        let mut samples = Vec::new();
        for i in 0..5 {
            let t = std::time::Instant::now();
            let result = channel.evaluate(60.0 * (i + 1) as f64, &release);
            samples.push(t.elapsed().as_secs_f64() * 1.0e3);
            // Use the result so nothing can be optimised away.
            assert!(!result.receptors.is_empty());
        }
        samples.sort_by(f64::total_cmp);
        let median = samples[samples.len() / 2];

        println!(
            "ONE DISPERSION EVALUATION (populated release, shipped config)\n  \
             samples (ms) = {:?}\n  \
             median       = {median:.3} ms per call\n  \
             plant tick   = {} ms; steam generator = 26.46 ms/call (100.12 % of the step)\n  \
             at 10 Hz this would be {:.1} % of a tick; on the 60 s throttle it is {:.4} % \
             of plant-time compute",
            samples
                .iter()
                .map(|v| (v * 1000.0).round() / 1000.0)
                .collect::<Vec<_>>(),
            crate::physics::PLANT_TIMESTEP_S * 1.0e3,
            100.0 * median / (crate::physics::PLANT_TIMESTEP_S * 1.0e3),
            100.0 * median / (DISPERSION_EVALUATION_INTERVAL_S * 1.0e3),
        );

        assert!(
            median < 100.0,
            "one dispersion evaluation costs {median:.3} ms, at or above the \
             {} ms plant tick. Removing DISPERSION_EVALUATION_INTERVAL_S's throttle would \
             then cost real-time ratio directly, so the throttle is doing real work and the \
             decision to remove it must be revisited.",
            crate::physics::PLANT_TIMESTEP_S * 1.0e3
        );
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
