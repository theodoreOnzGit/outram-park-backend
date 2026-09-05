//! Application state: the two screens (finder, plot) and the input that
//! drives them. Kept as plain data + enums (no trait objects, per the
//! workspace's "no `dyn`, use enums for dispatch" rule) so `main`'s event
//! loop can pattern-match on `app.screen` directly.

use std::sync::Arc;

use crate::nuclides::NuclideCatalog;
use crate::temperature::TempUnit;
use crate::xsdata::{MtChannel, NuclideXs};

/// Which screen is currently on top. [`Screen::Finder`] is the fuzzy nuclide
/// picker (op-ixe); [`Screen::Plot`] is the log-log cross-section view
/// (op-1lj) with the temperature control (op-dzl).
pub enum Screen {
    Finder(FinderState),
    Plot(PlotState),
}

/// State for the fuzzy nuclide finder screen.
pub struct FinderState {
    /// The text the user has typed so far.
    pub query: String,
    /// Index into the *filtered* result list (not the full catalog) of the
    /// currently highlighted row.
    pub selected: usize,
    /// First visible row index, for scrolling a list longer than the screen.
    pub scroll: usize,
    /// Row of the previous `Drag` mouse event, used to turn a touch-drag
    /// into a scroll delta (see `ui::finder::handle_mouse`). `None` when no
    /// drag is in progress.
    pub last_drag_row: Option<u16>,
}

impl Default for FinderState {
    fn default() -> Self {
        FinderState {
            query: String::new(),
            selected: 0,
            scroll: 0,
            last_drag_row: None,
        }
    }
}

/// State for the cross-section plot screen.
pub struct PlotState {
    pub nuclide: NuclideXs,
    pub mt: MtChannel,
    pub temp_unit: TempUnit,
    /// The temperature number as currently displayed, **in `temp_unit`**.
    /// Room temperature (25 degC) by default.
    pub temp_value: f64,
    /// Cache key for `cached_points`: recomputed only when
    /// `(nuclide name, mt, temp in kelvin)` actually changes, so dragging /
    /// tapping elsewhere on the screen doesn't re-run the Doppler evaluation
    /// every frame for no reason.
    cache_key: Option<(String, MtChannel, u64)>,
    pub cached_points: Vec<(f64, f64)>,
}

/// Number of log-spaced energy samples per redraw of the plot — see
/// [`crate::xsdata::sample_log_log`] for why a fixed grid this size is an
/// appropriate resolution for a terminal braille canvas.
const PLOT_SAMPLES: usize = 400;

impl PlotState {
    pub fn new(nuclide: NuclideXs) -> Self {
        PlotState {
            nuclide,
            mt: MtChannel::Total,
            temp_unit: TempUnit::Celsius,
            temp_value: 25.0,
            cache_key: None,
            cached_points: Vec::new(),
        }
    }

    /// Temperature in kelvin, clamped to `>= 0 K` (see
    /// [`crate::temperature::to_kelvin_clamped`]).
    pub fn temp_kelvin(&self) -> f64 {
        crate::temperature::to_kelvin_clamped(self.temp_value, self.temp_unit)
    }

    /// Recompute [`Self::cached_points`] if the nuclide/channel/temperature
    /// changed since the last call. Cheap no-op otherwise.
    pub fn refresh(&mut self) {
        let temp_bits = self.temp_kelvin().to_bits();
        let key = (self.nuclide.name().to_string(), self.mt, temp_bits);
        if self.cache_key.as_ref() != Some(&key) {
            self.cached_points = crate::xsdata::sample_log_log(
                &self.nuclide,
                self.mt,
                self.temp_kelvin(),
                PLOT_SAMPLES,
            );
            self.cache_key = Some(key);
        }
    }

    /// Toggle degC/K, converting the displayed number so the underlying
    /// physical temperature is preserved (op-dzl "auto-convert").
    pub fn toggle_unit(&mut self) {
        let kelvin = self.temp_kelvin();
        self.temp_unit = self.temp_unit.toggled();
        self.temp_value = match self.temp_unit {
            TempUnit::Kelvin => kelvin,
            TempUnit::Celsius => crate::temperature::kelvin_to_celsius(kelvin),
        };
    }

    /// Nudge the temperature by `delta`, **in the currently displayed
    /// unit**, clamped so it never reads below absolute zero in that unit.
    pub fn nudge_temp(&mut self, delta: f64) {
        let floor = match self.temp_unit {
            TempUnit::Celsius => -273.15,
            TempUnit::Kelvin => 0.0,
        };
        self.temp_value = (self.temp_value + delta).max(floor);
    }
}

/// Top-level application state.
pub struct App {
    pub catalog: Arc<NuclideCatalog>,
    pub screen: Screen,
    /// Set by an input handler to request the event loop exit.
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        App {
            catalog: Arc::new(NuclideCatalog::build()),
            screen: Screen::Finder(FinderState::default()),
            should_quit: false,
        }
    }

    /// Navigate from the finder to the plot screen for one nuclide.
    ///
    /// Silently stays on the finder if the nuclide fails to load (defensive:
    /// every catalog entry comes from the same embedded library it is looked
    /// up in, so this should not happen in practice).
    pub fn open_nuclide(&mut self, name: &str) {
        if let Ok(nx) = NuclideXs::load(name) {
            self.screen = Screen::Plot(PlotState::new(nx));
        }
    }

    pub fn back_to_finder(&mut self) {
        self.screen = Screen::Finder(FinderState::default());
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
