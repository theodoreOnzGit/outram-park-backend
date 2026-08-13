//! Visual instrumentation (gauge/readout).
//!
//! **No backing physics type yet.** `nee_soon` (the intended source for an
//! instrumentation physics type) is currently a scaffold with a single
//! empty `NeeSoon {}` struct -- no dedicated instrumentation type exists
//! there to wrap. This is a visual-only placeholder that displays a caller-
//! supplied label and value; once `nee_soon` exposes a real type, this
//! struct should gain a `physics` field the same way every other visual
//! component here does.

use egui::{Pos2, Response, Ui, Widget};

/// Visual placeholder for an instrument readout (no `nee_soon` physics type
/// to wrap yet -- see this module's doc). Displays a fixed label and value
/// string supplied by the caller, rather than reading live physics state.
pub struct InstrumentationVisual {
    /// On-screen anchor position.
    pub screen_position: Pos2,
    /// Instrument label (e.g. `"T_core"`).
    pub label: String,
    /// Instrument value, pre-formatted by the caller (e.g. `"573.15 K"`).
    pub value: String,
}

impl InstrumentationVisual {
    /// Construct a placeholder instrumentation visual with the given label
    /// and pre-formatted value.
    pub fn new(screen_position: Pos2, label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            screen_position,
            label: label.into(),
            value: value.into(),
        }
    }
}

impl Widget for InstrumentationVisual {
    /// Minimal-static rendering: a text label showing `"{label}: {value}"`.
    fn ui(self, ui: &mut Ui) -> Response {
        ui.put(
            egui::Rect::from_min_size(self.screen_position, egui::vec2(120.0, 20.0)),
            egui::Label::new(format!("{}: {}", self.label, self.value)),
        )
    }
}
