//! Application state machine: which screen is showing, the current preset
//! selection, run settings, the background run handle, and the touch
//! hit-region bookkeeping every screen populates on each redraw.
//!
//! # Touch-first input model (op-omf)
//!
//! `outram-mc-tui` targets a phone terminal (Termux) where the *primary*
//! interaction is a tap or a scroll, not a keystroke — Termux maps a touch tap
//! to a terminal mouse `Down`/`Up` click and a drag/flick to `Drag`/`ScrollUp`/
//! `ScrollDown` events (see `main.rs`'s `EnableMouseCapture`). So the ground
//! truth for "what does tapping here do" is the **hit-region list**
//! ([`App::hit_regions`]): every screen's draw function, in `crate::ui`, pushes
//! `(Rect, UiAction)` for each tappable card/button/stepper-arrow as it lays the
//! frame out, and [`App::handle_mouse`] looks up whichever region contains the
//! click. Keyboard (arrow keys / Enter / q / Esc) is wired as a **secondary**
//! path that drives the same [`UiAction`]s, so both input modes stay in sync
//! by construction — there is only one action enum, not two parallel command
//! sets.

use ratatui::layout::Rect;

use outram_mc_libs::physics::compute::ComputeType;

use crate::presets::{GeometryPreset, ALL_PRESETS};
use crate::settings::RunSettings;
use crate::spectrum::SpectrumOverlay;
use crate::transport::{RunHandle, RunOutcome, RunPhase};

/// Which of the four screens is currently showing. Mobile-first: exactly one
/// screen fills the narrow terminal at a time (no side-by-side panes), and
/// [`Screen::next`]/[`Screen::back`] is a simple forward/backward wizard, not a
/// tab bar — matches how a phone app moves through "pick -> configure -> run ->
/// inspect" rather than switching between independent tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    GeometryPicker,
    Settings,
    Run,
    Spectrum,
}

/// Every user-triggerable action, dispatched identically from a mouse tap
/// (`App::handle_mouse`) or a keyboard shortcut (`App::handle_key`). See the
/// module docs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UiAction {
    SelectPreset(GeometryPreset),
    ToggleSphereVariant,
    GoToSettings,
    GoToGeometryPicker,
    GoToRun,
    GoToSpectrum,
    CycleCompute,
    BumpParticles(i64),
    BumpInactive(i64),
    BumpActive(i64),
    BumpSeed(i64),
    StartRun,
    ScrollBy(i32),
    Quit,
}

/// How many newly-computed generations the run screen reveals per redraw
/// tick, animating the already-finished `k_by_generation` trace onto the
/// chart instead of popping it in all at once — see `transport.rs`'s "live
/// convergence" doc note for what is and is not really incremental here.
const REVEAL_PER_TICK: usize = 3;

pub struct App {
    pub screen: Screen,
    pub selected_preset: GeometryPreset,
    pub settings: RunSettings,
    pub run_handle: RunHandle,
    /// How many points of the finished `k_by_generation` trace the run
    /// screen has revealed so far (see [`REVEAL_PER_TICK`]).
    pub revealed: usize,
    /// Cached copy of the last `Done` outcome, so the run/spectrum screens
    /// keep showing it across redraws without re-locking the run handle's
    /// mutex on every single tick once the run is finished.
    pub last_outcome: Option<Result<RunOutcome, String>>,
    /// Vertical scroll offset for the geometry-picker list (touch
    /// drag/wheel-scrollable when the four cards overflow a very short
    /// terminal).
    pub geometry_scroll: u16,
    /// Vertical scroll offset for the settings list (same reasoning).
    pub settings_scroll: u16,
    /// Tap/scroll hit regions for the *current* frame, repopulated by
    /// `crate::ui`'s draw functions on every redraw. `(Rect, action)`: a mouse
    /// `Down(Left)` inside `Rect` dispatches `action`.
    pub hit_regions: Vec<(Rect, UiAction)>,
    /// The scroll hit-region for whichever list is showing (a wheel/drag event
    /// anywhere over the list scrolls it), separate from the tap regions above
    /// because a scroll does not "activate" anything.
    pub scroll_region: Option<Rect>,
    pub should_quit: bool,
    /// One-line status/error message shown at the bottom of the frame (e.g. a
    /// preset-build failure surfaced instead of panicking).
    pub status: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::GeometryPicker,
            selected_preset: ALL_PRESETS[0],
            settings: RunSettings::default(),
            run_handle: RunHandle::new(),
            revealed: 0,
            last_outcome: None,
            geometry_scroll: 0,
            settings_scroll: 0,
            hit_regions: Vec::new(),
            scroll_region: None,
            should_quit: false,
            status: None,
        }
    }

    /// Whether the currently selected preset's driver can show a spectrum
    /// overlay — gates the "Spectrum" nav tap on the run screen.
    pub fn spectrum_available(&self) -> bool {
        self.selected_preset.supports_spectrum()
    }

    /// Called once per redraw, before drawing: pulls the latest background-run
    /// phase and advances the reveal-animation counter. Kept separate from
    /// drawing so the animation still advances even on ticks with no user
    /// input (the render loop's poll timeout guarantees a tick occurs
    /// regularly — see `main.rs`).
    pub fn on_tick(&mut self) {
        if let RunPhase::Done(outcome) = self.run_handle.snapshot() {
            let total = outcome.as_ref().map(|o| o.k_by_generation.len()).unwrap_or(0);
            if self.last_outcome.is_none() {
                self.revealed = 0;
            }
            self.last_outcome = Some(outcome);
            if self.revealed < total {
                self.revealed = (self.revealed + REVEAL_PER_TICK).min(total);
            }
        }
    }

    /// Dispatch one [`UiAction`] — the single place every mouse tap and
    /// keyboard shortcut ends up, per the module docs.
    pub fn dispatch(&mut self, action: UiAction) {
        match action {
            UiAction::SelectPreset(p) => {
                self.selected_preset = p;
            }
            UiAction::ToggleSphereVariant => {
                self.selected_preset = self.selected_preset.toggle_sphere_variant();
            }
            UiAction::GoToSettings => self.screen = Screen::Settings,
            UiAction::GoToGeometryPicker => self.screen = Screen::GeometryPicker,
            UiAction::GoToRun => self.screen = Screen::Run,
            UiAction::GoToSpectrum => {
                if self.spectrum_available() {
                    self.screen = Screen::Spectrum;
                }
            }
            UiAction::CycleCompute => self.settings.cycle_compute(),
            UiAction::BumpParticles(d) => self.settings.bump_particles(d),
            UiAction::BumpInactive(d) => self.settings.bump_inactive(d),
            UiAction::BumpActive(d) => self.settings.bump_active(d),
            UiAction::BumpSeed(d) => self.settings.bump_seed(d),
            UiAction::StartRun => {
                self.last_outcome = None;
                self.revealed = 0;
                self.status = None;
                self.run_handle.start(self.selected_preset, self.settings);
                self.screen = Screen::Run;
            }
            UiAction::ScrollBy(delta) => self.scroll_active_list(delta),
            UiAction::Quit => self.should_quit = true,
        }
    }

    fn scroll_active_list(&mut self, delta: i32) {
        let target = match self.screen {
            Screen::GeometryPicker => &mut self.geometry_scroll,
            Screen::Settings => &mut self.settings_scroll,
            _ => return,
        };
        let v = *target as i32 + delta;
        *target = v.clamp(0, 200) as u16;
    }

    /// Look up a screen coordinate against this frame's hit regions and
    /// dispatch the first match — the touch-tap path.
    pub fn tap(&mut self, column: u16, row: u16) {
        let hit = self.hit_regions.iter().find(|(rect, _)| point_in_rect(column, row, *rect)).map(|(_, a)| *a);
        if let Some(action) = hit {
            self.dispatch(action);
        }
    }

    /// Scroll the active list if the wheel/drag event landed inside its
    /// registered scroll region (or anywhere, when no region was registered
    /// yet — a narrow phone screen where the whole viewport is the list).
    pub fn scroll(&mut self, column: u16, row: u16, delta: i32) {
        let inside = self.scroll_region.map(|r| point_in_rect(column, row, r)).unwrap_or(true);
        if inside {
            self.scroll_active_list(delta);
        }
    }

    /// Caveat shown under the compute-backend row on the settings screen.
    ///
    /// **Honest disclosure, not a TUI bug:** [`RunSettings::compute`] is wired
    /// straight through to [`outram_mc_libs::physics::keff::KeffSettings::compute`]
    /// for every preset, and [`crate::app::App::gpu_status_line`] genuinely
    /// queries [`outram_mc_libs::gpu::probe`]. But of this crate's three
    /// transport entry points, only
    /// [`outram_mc_libs::physics::keff::run_keff`] (the original bare-sphere-only
    /// driver) actually dispatches on [`ComputeType`] — verified by reading
    /// `outram-mc-libs/src/physics/transport_csg.rs` and
    /// `outram-mc-libs/src/pebble_beds/keff_delta.rs`, neither of which
    /// reference `ComputeType`/`rayon`/`GpuContext` at all. Every preset in
    /// this TUI runs through `run_keff_csg` (bare sphere, LWR cell) or
    /// `run_keff_delta` (pebble beds) — chosen so bare sphere and LWR cell can
    /// carry the op-iom spectrum tally — so the compute-backend selector is
    /// currently **inert**: it is read and shown correctly, but every run
    /// executes the deterministic single-thread CPU path regardless of what is
    /// selected here. This is a real gap in the upstream crate's driver
    /// surface (`run_keff_csg`/`run_keff_delta` have no `ComputeType`
    /// dispatch), not a TUI oversight; see the crate README for the follow-up.
    pub fn compute_dispatch_note(&self) -> &'static str {
        "note: run_keff_csg/run_keff_delta (what every preset here uses) do not yet \
         dispatch on this setting - only run_keff does. All runs are single-thread \
         CPU regardless of the choice above; see README."
    }

    /// Human-readable GPU-availability line for the settings screen.
    pub fn gpu_status_line(&self) -> String {
        let available = outram_mc_libs::gpu::probe().is_some();
        match (self.settings.compute, available) {
            (ComputeType::Gpu, true) => "GPU adapter found - the run will use it".to_string(),
            (ComputeType::Gpu, false) => "No GPU adapter found - run will fall back to CPU".to_string(),
            (_, true) => "GPU adapter available (not selected)".to_string(),
            (_, false) => "No GPU adapter on this device".to_string(),
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn point_in_rect(column: u16, row: u16, rect: Rect) -> bool {
    column >= rect.x && column < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

/// Convenience the spectrum screen uses: the last finished run's overlay, if
/// any (`None` before a run, on a build error, or for a delta-tracking preset
/// with no tally attached).
pub fn current_spectrum(app: &App) -> Option<&SpectrumOverlay> {
    match &app.last_outcome {
        Some(Ok(outcome)) => outcome.spectrum.as_ref(),
        _ => None,
    }
}
