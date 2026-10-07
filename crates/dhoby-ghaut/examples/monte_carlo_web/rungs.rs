//! **The rung table** of the Monte Carlo demo (gh:#520, #521): which rungs
//! the one app has, and the contract each rung's module fulfils.
//!
//! # Adding a rung is additive
//!
//! A rung is a directory `examples/monte_carlo_web/<rung>/` whose `mod.rs`
//! defines a marker type implementing [`McRung`] (its [`RungInfo`], tapes,
//! picture, notes, and the [`RungBuilder`] / [`LoadedRung`] that process the
//! data and run the neutrons), plus **one line** in the `rung_table!`
//! invocation in `main.rs`:
//!
//! ```text
//! rung_table! {
//!     godiva: Godiva,
//!     triso: Triso,
//!     ugraphite: Ugraphite,   // <- the new rung's module and marker type
//! }
//! ```
//!
//! The macro declares the module and generates the `Rung` enum, its URL
//! names, and the enum dispatch (`Builder`, `Loaded`) that the engine and the
//! app use, so no other file changes. (Enums, not trait objects: the
//! workspace's Rust rules.) `godiva/` is the worked example of a rung with a
//! Run k_eff mode, `triso/` of one without.
//!
//! Two optional pieces any rung can add (gh:#549), with no change outside its
//! directory: a **small `k_inf` case** at a parameter the reader picks
//! ([`McRung::kinf_case`] with [`LoadedRung::kinf_start`] /
//! [`LoadedRung::kinf_step`]; the engine streams it one generation per
//! request and the app gives it a slider and a plot against the recorded
//! curve), and the **σ(E) panel** beside the geometry
//! ([`LoadedRung::xs_curves`]).
//!
//! `scripts/build-pages.sh` reads every rung's `name:` and `lesson:` lines
//! (in `<rung>/mod.rs`, one line each) and fails the site build if the lesson
//! page is missing or a page links to a rung that does not exist.

use crate::anim::Spectrum;
use crate::engine::Tier;
use crate::history::History;
use crate::keff::{Generation, KeffConfig, KinfCase, KinfGeneration, Reference};
use crate::raster::{RasterInfo, RasterReq};
use crate::sweep::RecordedSweep;
use crate::xs::XsCurve;
use dhoby_ghaut::web_demo::view::View;

/// What the demo shows for a rung.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Illustration: neutrons animated one at a time (and, where the rung has
    /// one, whole generations of the fission source).
    Watch,
    /// True Monte Carlo: a real power iteration, one console line per generation.
    Run,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Watch => "watch",
            Mode::Run => "run",
        }
    }
    pub fn parse(s: &str) -> Option<Mode> {
        match s {
            "watch" => Some(Mode::Watch),
            "run" => Some(Mode::Run),
            _ => None,
        }
    }
}

/// One rung's row of the table.
pub struct RungInfo {
    /// Stable short name, used in `?rung=`.
    pub name: &'static str,
    /// Title shown in the panel.
    pub title: &'static str,
    /// The page explaining this rung, relative to the site root.
    pub lesson: &'static str,
    /// Sets the animation-speed default ([`Spectrum::default_speed_at_1ev`]).
    pub spectrum: Spectrum,
}

/// A rung's data processing, one tape at a time (so the UI can show
/// progress between the long, blocking jobs).
pub trait RungBuilder: Sized {
    type Loaded: LoadedRung;
    fn new(tier: Tier) -> Self;
    /// Process the next tape from its (covariance-stripped) bytes.
    fn step(&mut self, bytes: &[u8]) -> Result<(), String>;
    fn finish(self) -> Result<Self::Loaded, String>;
}

/// A rung with its data processed: what the engine serves.
pub trait LoadedRung {
    /// The next traced history of the Watch mode's chain.
    fn run_next(&mut self) -> History;
    /// Start a power iteration; returns the initial source sites when
    /// `cfg.want_sites`. A rung without one returns an error.
    fn keff_start(&mut self, cfg: KeffConfig) -> Result<Vec<[f32; 2]>, String>;
    /// Run the next generation; `None` once the run is over.
    fn keff_step(&mut self) -> Option<Generation>;
    fn keff_finished(&self) -> bool;
    /// Start a small `k_inf` case at parameter `param` (the rung's
    /// [`McRung::kinf_case`]), replacing any running one. A rung without one
    /// returns an error.
    fn kinf_start(&mut self, _param: f64, _cfg: KeffConfig) -> Result<(), String> {
        Err("this rung has no k_inf case".into())
    }
    /// Run the case's next generation; `None` once it is over.
    fn kinf_step(&mut self) -> Option<KinfGeneration> {
        None
    }
    /// The σ(E) panel's curves from the processed data, decimated
    /// ([`crate::xs::curves`]); empty for a rung without the panel.
    fn xs_curves(&self) -> Vec<XsCurve> {
        Vec::new()
    }
    /// Rasterise a window of the assembled geometry ([`crate::raster`]);
    /// only for a rung with [`McRung::raster_info`].
    fn raster(&mut self, _req: &RasterReq) -> Result<Vec<u8>, String> {
        Err("this rung has no live geometry slice".into())
    }
}

/// Everything about one rung that the engine and the app need. Implemented
/// by a marker type in the rung's `mod.rs`.
pub trait McRung {
    const INFO: RungInfo;
    type Builder: RungBuilder;
    /// The tapes to process, `(label shown, file in reference-data/endf/)`.
    fn jobs() -> &'static [(&'static str, &'static str)];
    /// The tier the tapes are processed at, given what the mode asked for.
    fn tier(requested: Tier) -> Tier;
    /// Rough cost of each job, for the progress bar only.
    fn job_weights(tier: Tier) -> Vec<f64>;
    /// Half-width of the picture, cm (what Reset fits).
    fn half_extent() -> f64;
    /// Draw the geometry under the tracks.
    fn draw(painter: &egui::Painter, rect: egui::Rect, view: &View);
    /// "What this is — and is not", one bullet per line.
    fn notes() -> &'static [&'static str];
    /// Run k_eff's default settings; `None` means the rung has no Run mode.
    fn run_default() -> Option<KeffConfig> {
        None
    }
    /// Watch mode's "whole generations" settings; `None` means no such view.
    fn watch_generations() -> Option<KeffConfig> {
        None
    }
    /// The recorded result Run k_eff compares with.
    fn reference() -> Option<Reference> {
        None
    }
    /// Shown on the loading card, if anything (e.g. why it takes long).
    fn loading_note(_tier: Tier) -> Option<&'static str> {
        None
    }
    /// The rung's own legend in the side panel (materials, say), under the
    /// shared energy colour bar and track markers.
    fn legend(_ui: &mut egui::Ui) {}
    /// A small `k_inf` case the reader can run at a parameter of their
    /// choice (gh:#549); `None` means the rung has none. Its engine side is
    /// [`LoadedRung::kinf_start`] / [`LoadedRung::kinf_step`].
    fn kinf_case() -> Option<KinfCase> {
        None
    }
    /// The tapes a load at `tier` processes. A rung whose Watch views need
    /// no nuclear data (the `htr10` geometry and recorded sweep) loads none
    /// until a view that computes asks for the full tier. Defaults to
    /// [`McRung::jobs`].
    fn jobs_for(_tier: Tier) -> &'static [(&'static str, &'static str)] {
        Self::jobs()
    }
    /// Whether Watch has "one neutron at a time" (default yes).
    fn has_tracks() -> bool {
        true
    }
    /// A live slice of the assembled geometry (the zoom ladder), gh:#528.
    fn raster_info() -> Option<RasterInfo> {
        None
    }
    /// A recorded sweep read with a slider, gh:#528.
    fn sweep() -> Option<RecordedSweep> {
        None
    }
    /// Whether the rung has the lattice-against-random-bed view
    /// ([`crate::beds`], gh:#787).
    fn beds() -> bool {
        false
    }
}

/// Generates, from a list `module: Marker`, the `mod` declarations, the
/// `Rung` enum with its dispatch to each marker's [`McRung`] methods, and the
/// `Builder` / `Loaded` enums the engine holds. See the module docs.
#[macro_export]
macro_rules! rung_table {
    ($($m:ident : $t:ident),+ $(,)?) => {
        $( #[cfg(not(target_os = "android"))] mod $m; )+

        #[cfg(not(target_os = "android"))]
        pub mod table {
            #![allow(dead_code)]
            use $crate::rungs::{McRung, RungBuilder, LoadedRung, RungInfo};
            use $crate::engine::Tier;
            use $crate::history::History;
            use $crate::keff::{Generation, KeffConfig, KinfCase, KinfGeneration, Reference};
            use $crate::xs::XsCurve;
            use $crate::raster::{RasterInfo, RasterReq};
            use $crate::sweep::RecordedSweep;

            /// The rungs, in ladder order (the order of `rung_table!`).
            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            pub enum Rung { $( $t, )+ }

            pub const ALL: &[Rung] = &[ $( Rung::$t, )+ ];

            impl dhoby_ghaut::web_demo::lesson::Rung for Rung {
                fn all() -> &'static [Self] { ALL }
                fn name(self) -> &'static str { self.info().name }
                fn title(self) -> &'static str { self.info().title }
                fn lesson(self) -> &'static str { self.info().lesson }
            }

            impl Rung {
                pub fn info(self) -> &'static RungInfo {
                    match self { $( Rung::$t => &<$crate::$m::$t as McRung>::INFO, )+ }
                }
                pub fn jobs(self) -> &'static [(&'static str, &'static str)] {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::jobs(), )+ }
                }
                pub fn tier(self, requested: Tier) -> Tier {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::tier(requested), )+ }
                }
                pub fn job_weights(self, tier: Tier) -> Vec<f64> {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::job_weights(tier), )+ }
                }
                pub fn half_extent(self) -> f64 {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::half_extent(), )+ }
                }
                pub fn draw(self, painter: &egui::Painter, rect: egui::Rect, view: &dhoby_ghaut::web_demo::view::View) {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::draw(painter, rect, view), )+ }
                }
                pub fn notes(self) -> &'static [&'static str] {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::notes(), )+ }
                }
                pub fn run_default(self) -> Option<KeffConfig> {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::run_default(), )+ }
                }
                pub fn watch_generations(self) -> Option<KeffConfig> {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::watch_generations(), )+ }
                }
                pub fn reference(self) -> Option<Reference> {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::reference(), )+ }
                }
                pub fn loading_note(self, tier: Tier) -> Option<&'static str> {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::loading_note(tier), )+ }
                }
                pub fn legend(self, ui: &mut egui::Ui) {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::legend(ui), )+ }
                }
                pub fn jobs_for(self, tier: Tier) -> &'static [(&'static str, &'static str)] {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::jobs_for(tier), )+ }
                }
                pub fn has_tracks(self) -> bool {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::has_tracks(), )+ }
                }
                pub fn raster_info(self) -> Option<RasterInfo> {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::raster_info(), )+ }
                }
                pub fn sweep(self) -> Option<RecordedSweep> {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::sweep(), )+ }
                }
                pub fn kinf_case(self) -> Option<KinfCase> {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::kinf_case(), )+ }
                }
                pub fn beds(self) -> bool {
                    match self { $( Rung::$t => <$crate::$m::$t as McRung>::beds(), )+ }
                }
                pub fn has_run(self) -> bool {
                    self.run_default().is_some()
                }
                pub fn builder(self, requested: Tier) -> Builder {
                    let tier = self.tier(requested);
                    match self { $( Rung::$t => Builder::$t(<<$crate::$m::$t as McRung>::Builder as RungBuilder>::new(tier)), )+ }
                }
            }

            /// A rung's data processing in progress.
            #[allow(clippy::large_enum_variant)]
            pub enum Builder { $( $t(<$crate::$m::$t as McRung>::Builder), )+ }

            impl Builder {
                pub fn step(&mut self, bytes: &[u8]) -> Result<(), String> {
                    match self { $( Builder::$t(b) => RungBuilder::step(b, bytes), )+ }
                }
                pub fn finish(self) -> Result<Loaded, String> {
                    Ok(match self { $( Builder::$t(b) => Loaded::$t(RungBuilder::finish(b)?), )+ })
                }
            }

            /// A rung with its data processed.
            #[allow(clippy::large_enum_variant)]
            pub enum Loaded { $( $t(<<$crate::$m::$t as McRung>::Builder as RungBuilder>::Loaded), )+ }

            impl Loaded {
                pub fn run_next(&mut self) -> History {
                    match self { $( Loaded::$t(l) => l.run_next(), )+ }
                }
                pub fn keff_start(&mut self, cfg: KeffConfig) -> Result<Vec<[f32; 2]>, String> {
                    match self { $( Loaded::$t(l) => l.keff_start(cfg), )+ }
                }
                pub fn keff_step(&mut self) -> Option<Generation> {
                    match self { $( Loaded::$t(l) => l.keff_step(), )+ }
                }
                pub fn keff_finished(&self) -> bool {
                    match self { $( Loaded::$t(l) => l.keff_finished(), )+ }
                }
                pub fn kinf_start(&mut self, param: f64, cfg: KeffConfig) -> Result<(), String> {
                    match self { $( Loaded::$t(l) => l.kinf_start(param, cfg), )+ }
                }
                pub fn kinf_step(&mut self) -> Option<KinfGeneration> {
                    match self { $( Loaded::$t(l) => l.kinf_step(), )+ }
                }
                pub fn xs_curves(&self) -> Vec<XsCurve> {
                    match self { $( Loaded::$t(l) => l.xs_curves(), )+ }
                }
                pub fn raster(&mut self, req: &RasterReq) -> Result<Vec<u8>, String> {
                    match self { $( Loaded::$t(l) => l.raster(req), )+ }
                }
            }
        }
    };
}

/// What the app opens on: `?rung=…&mode=…` in the browser, `--rung …
/// --mode …` natively. No rung given opens the first rung; a rung without a
/// Run mode ignores `mode=run`. (The old `demos/triso-pebble/` URL
/// redirects to `?rung=triso`.)
pub fn start(query: &[(String, String)]) -> (crate::table::Rung, Mode) {
    use dhoby_ghaut::web_demo::{lesson, platform::query_value};
    let rung: crate::table::Rung = lesson::from_query(query);
    let mode = query_value(query, "mode").and_then(Mode::parse).unwrap_or(Mode::Watch);
    let mode = if rung.has_run() { mode } else { Mode::Watch };
    (rung, mode)
}
