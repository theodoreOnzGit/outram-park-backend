//! The GUI: panel set, the always-on security banner, and shared formatting.
//!
//! ## The `--` rule
//!
//! [`UNREAD_PLACEHOLDER`] is the only thing this client ever shows for a value it
//! has not read. There is no zero default, no "last known" number carried over
//! from a previous session, and no interpolation. [`format_numeric`] and
//! [`format_boolean`] take an `Option` and return the placeholder for `None`, so
//! the honest display is the *easy* one to write and a fabricated value would
//! have to be introduced deliberately.
//!
//! ## The banner is not optional
//!
//! [`security_banner`] draws on every frame while a session is up. The intent,
//! from the maintainer: people should try the demo *knowing* the connection has no
//! authentication and no encryption. Tucking that into an about box would let them
//! miss it, so it is a permanent coloured strip instead. Do not make it
//! dismissible, and do not move it behind a tab.

use egui::{Color32, RichText, Ui};

use crate::shared_state::{BooleanSample, ClientSharedState, ConnectionState, NumericSample};

pub mod about_panel;
pub mod controls_panel;
pub mod discovery_panel;
pub mod outputs_panel;
pub mod trends_panel;

/// What is shown for any value this client has not read from the server.
///
/// Two ASCII hyphens, chosen so it is obviously not a number and cannot be
/// mistaken for `0`, `0.0` or an em-dash used as a minus sign.
pub const UNREAD_PLACEHOLDER: &str = "--";

/// Fill colour of the security banner: a muted amber that reads as "be aware"
/// rather than "something has gone wrong".
pub const BANNER_FILL: Color32 = Color32::from_rgb(90, 62, 12);

/// Text colour on the security banner.
pub const BANNER_TEXT: Color32 = Color32::from_rgb(255, 226, 150);

/// Colour for a failed write, a failed connection, or a bad status code.
pub const ERROR_TEXT: Color32 = Color32::from_rgb(255, 138, 128);

/// Colour for a caveat that is not a failure — a clamped set point, a fallback to
/// polling, a non-`Good` but usable status.
pub const WARNING_TEXT: Color32 = Color32::from_rgb(255, 205, 120);

/// Colour for a healthy, confirmed value.
pub const OK_TEXT: Color32 = Color32::from_rgb(150, 220, 160);

/// Colour for the `--` placeholder and other "no data" text: deliberately dim, so
/// an unread cell recedes instead of competing with real readings.
pub const MUTED_TEXT: Color32 = Color32::from_rgb(150, 150, 150);

/// The client's top-level tabs.
///
/// An enum with an exhaustive `match` in [`Panel::show`], so a new panel cannot be
/// added without being wired into the dispatch (workspace Rust design rules).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// Find a simulator and connect to it.
    Connect,
    /// The live output grid, grouped by kind.
    Outputs,
    /// Time trends against simulated time.
    Trends,
    /// Writable controls and switches.
    Controls,
    /// Scope, security and provenance.
    About,
}

impl Panel {
    /// Every panel, in tab order. `Connect` is first because it is where a new
    /// user has to start.
    pub const ALL: &'static [Panel] = &[
        Self::Connect,
        Self::Outputs,
        Self::Trends,
        Self::Controls,
        Self::About,
    ];

    /// Tab label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Connect => "Connect",
            Self::Outputs => "Live outputs",
            Self::Trends => "Trends",
            Self::Controls => "Controls",
            Self::About => "About & scope",
        }
    }
}

/// Format a numeric reading with its unit, or the `--` placeholder.
///
/// # Arguments
///
/// * `sample` — the reading, or `None` if this client has not read the node.
/// * `unit` — the node's unit string from the node map, e.g. `"degC"`.
/// * `decimals` — digits after the point.
///
/// Returns `"--"` for `None`, with **no unit appended** — `"-- degC"` would imply
/// a temperature was measured and merely not displayed.
pub fn format_numeric(sample: Option<&NumericSample>, unit: &str, decimals: usize) -> String {
    match sample {
        None => UNREAD_PLACEHOLDER.to_string(),
        Some(sample) if !sample.value.is_finite() => {
            format!("{} (non-finite)", sample.value)
        }
        Some(sample) => format!("{:.*} {}", decimals, sample.value, unit),
    }
}

/// Format a boolean read-back, or the `--` placeholder.
pub fn format_boolean(sample: Option<&BooleanSample>) -> String {
    match sample {
        None => UNREAD_PLACEHOLDER.to_string(),
        Some(sample) if sample.value => "on".to_string(),
        Some(_) => "off".to_string(),
    }
}

/// Colour a reading by its status: dim when unread, error when the server marked
/// it bad, warning when uncertain, normal when good.
pub fn numeric_colour(sample: Option<&NumericSample>) -> Color32 {
    match sample {
        None => MUTED_TEXT,
        Some(sample) if sample.is_good() => OK_TEXT,
        Some(sample) if sample.status.is_uncertain() => WARNING_TEXT,
        Some(_) => ERROR_TEXT,
    }
}

/// The permanent security banner, drawn while a session is up.
///
/// Says in plain words that the link carries no authentication and no encryption
/// and that anyone on the network can read and write these values. Present and
/// legible by design — see this module's header.
pub fn security_banner(ui: &mut Ui) {
    egui::Frame::new()
        .fill(BANNER_FILL)
        .inner_margin(6)
        .corner_radius(4)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new("UNSECURED LINK")
                        .color(BANNER_TEXT)
                        .strong()
                        .monospace(),
                );
                ui.label(
                    RichText::new(
                        "No authentication, no encryption -- anyone on this network can read \
                         and write these values. Educational demonstration only; never point \
                         this at a real plant or an institutional production system.",
                    )
                    .color(BANNER_TEXT),
                );
            });
        });
}

/// The one-line status strip: connection state, endpoint, namespace index,
/// transport mode, value count, and any worker note.
///
/// Matches [`ConnectionState`] exhaustively, so an in-flight or failed attempt can
/// never be drawn as a live session.
pub fn status_strip(ui: &mut Ui, state: &ClientSharedState) {
    ui.horizontal_wrapped(|ui| {
        // The state's own label and endpoint, so the strip cannot disagree with
        // the state machine about which state it is in.
        let colour = match &state.connection {
            ConnectionState::Disconnected => MUTED_TEXT,
            ConnectionState::Connecting { .. } => WARNING_TEXT,
            ConnectionState::Connected { .. } => OK_TEXT,
            ConnectionState::Failed { .. } => ERROR_TEXT,
        };
        ui.label(
            RichText::new(state.connection.label())
                .color(colour)
                .strong(),
        );
        if let Some(endpoint_url) = state.connection.endpoint_url() {
            ui.label(RichText::new(endpoint_url).monospace());
        }
        if let Some(age) = state.connection.age() {
            ui.label(RichText::new(format!("({:.0} s)", age.as_secs_f64())).color(MUTED_TEXT));
        }

        match &state.connection {
            ConnectionState::Disconnected => {
                ui.label(
                    RichText::new("-- open the Connect tab to find a simulator").color(MUTED_TEXT),
                );
            }
            ConnectionState::Connecting { .. } => {}
            ConnectionState::Connected {
                namespace_index,
                transport,
                ..
            } => {
                ui.separator();
                ui.label(
                    RichText::new(format!("ns={namespace_index} (read from the server)"))
                        .color(MUTED_TEXT),
                );
                ui.separator();
                ui.label(RichText::new(transport.label()).color(MUTED_TEXT));
                ui.separator();
                ui.label(
                    RichText::new(format!("{} values received", state.values_received))
                        .color(MUTED_TEXT),
                );
            }
            ConnectionState::Failed { cause, .. } => {
                ui.separator();
                ui.label(RichText::new(cause.summary()).color(ERROR_TEXT));
            }
        }

        if state.has_failed_write() {
            ui.separator();
            ui.label(
                RichText::new("a write was refused -- see the Controls tab").color(ERROR_TEXT),
            );
        }
    });

    if let Some(note) = &state.worker_note {
        ui.label(RichText::new(note).color(WARNING_TEXT).small());
    }
}

/// A short, dim line telling the reader that a panel has nothing yet, and why.
pub fn awaiting_data_note(ui: &mut Ui, state: &ClientSharedState) {
    let text = match &state.connection {
        ConnectionState::Connected { .. } => "Connected, waiting for the first values to arrive.",
        ConnectionState::Connecting { .. } => "Connecting -- no values yet.",
        ConnectionState::Failed { .. } => {
            "Not connected. The Connect tab explains what went wrong."
        }
        ConnectionState::Disconnected => "Not connected. Use the Connect tab to find a simulator.",
    };
    ui.label(RichText::new(text).color(MUTED_TEXT));
    ui.label(
        RichText::new(format!(
            "Values this client has not read are shown as \"{UNREAD_PLACEHOLDER}\" -- never as \
             a placeholder number."
        ))
        .color(MUTED_TEXT)
        .small(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use opcua::types::StatusCode;
    use std::time::Instant;

    /// Verifies an unread value formats as the bare `--` placeholder with no unit
    /// and no number, for every published node.
    ///
    /// **Methodology.** Call [`format_numeric`] with `None` for each of the 21
    /// signals' and 8 controls' units, and [`format_boolean`] with `None`, and
    /// assert the output is exactly `"--"` — in particular that it contains no
    /// digit and does not carry the unit. Appending the unit would imply a
    /// measurement existed; a digit would be a fabricated value. This is the
    /// display half of the anti-fabrication rule whose storage half is checked in
    /// `shared_state::tests::unread_nodes_are_absent_rather_than_zero`. Pass
    /// criterion: exact equality with `"--"` in all 30 cases.
    ///
    /// **Results (2026-07-28).** 29 numeric cases (21 signal units + 8 control
    /// units) and 1 boolean case all produced exactly `"--"`, containing no ASCII
    /// digit and none of the unit strings `degC`, `kg/s`, `kW`, `Pa`, `s`, `ms`,
    /// `rad/s`, `W/(m^2 K)`. Interpretation: an empty grid is unambiguously
    /// empty.
    #[test]
    fn unread_values_format_as_a_bare_placeholder() {
        use outram_park_digital_twin_engine::ciet_opcua::node_map::{CietControl, CietSignal};

        for signal in CietSignal::ALL {
            let text = format_numeric(None, signal.unit(), 2);
            assert_eq!(text, UNREAD_PLACEHOLDER, "signal {signal:?}");
            assert!(!text.chars().any(|c| c.is_ascii_digit()));
            assert!(!text.contains(signal.unit()));
        }
        for control in CietControl::ALL {
            let text = format_numeric(None, control.unit(), 2);
            assert_eq!(text, UNREAD_PLACEHOLDER, "control {control:?}");
        }
        assert_eq!(format_boolean(None), UNREAD_PLACEHOLDER);
    }

    /// Verifies a real reading formats with its value and unit at the requested
    /// precision, and that a non-finite value is labelled rather than drawn as a
    /// number.
    ///
    /// **Methodology.** Format a `Good` sample of 86.5 degC at 2 decimals, a flow
    /// of -0.0378 kg/s at 4 decimals (negative, since FM-40 is signed and reverse
    /// flow must display), and a `NaN`. Pass criterion: `"86.50 degC"`,
    /// `"-0.0378 kg/s"`, and a `NaN` rendering that carries the word
    /// "non-finite".
    ///
    /// **Results (2026-07-28).** Measured `"86.50 degC"`, `"-0.0378 kg/s"`, and
    /// `"NaN (non-finite)"`. Interpretation: a signed flowrate keeps its sign,
    /// and a `NaN` arriving from a diverging solver is shown as broken rather
    /// than as a plausible number.
    #[test]
    fn real_readings_format_with_value_unit_and_sign() {
        let good = NumericSample {
            value: 86.5,
            status: StatusCode::Good,
            received_at: Instant::now(),
        };
        assert_eq!(format_numeric(Some(&good), "degC", 2), "86.50 degC");

        let reverse_flow = NumericSample {
            value: -0.0378,
            status: StatusCode::Good,
            received_at: Instant::now(),
        };
        assert_eq!(
            format_numeric(Some(&reverse_flow), "kg/s", 4),
            "-0.0378 kg/s"
        );

        let broken = NumericSample {
            value: f64::NAN,
            status: StatusCode::Good,
            received_at: Instant::now(),
        };
        assert!(format_numeric(Some(&broken), "degC", 2).contains("non-finite"));

        let on = BooleanSample {
            value: true,
            status: StatusCode::Good,
            received_at: Instant::now(),
        };
        assert_eq!(format_boolean(Some(&on)), "on");
    }

    /// Verifies an unread cell and a bad-status cell are coloured differently
    /// from a good one, so status is visible without reading the text.
    ///
    /// **Methodology.** Compare [`numeric_colour`] for `None`, a `Good` sample
    /// and a `BadDeviceFailure` sample. Pass criterion: three distinct colours,
    /// with the unread case equal to [`MUTED_TEXT`] and the bad case equal to
    /// [`ERROR_TEXT`].
    ///
    /// **Results (2026-07-28).** `None` → `MUTED_TEXT` (150,150,150); `Good` →
    /// `OK_TEXT` (150,220,160); `BadDeviceFailure` → `ERROR_TEXT`
    /// (255,138,128) — three distinct values. Interpretation: a column of dim
    /// `--` cells is visibly different from a column of live readings.
    #[test]
    fn reading_colour_distinguishes_unread_good_and_bad() {
        let good = NumericSample {
            value: 1.0,
            status: StatusCode::Good,
            received_at: Instant::now(),
        };
        let bad = NumericSample {
            value: 1.0,
            status: StatusCode::BadDeviceFailure,
            received_at: Instant::now(),
        };

        assert_eq!(numeric_colour(None), MUTED_TEXT);
        assert_eq!(numeric_colour(Some(&good)), OK_TEXT);
        assert_eq!(numeric_colour(Some(&bad)), ERROR_TEXT);
        assert_ne!(numeric_colour(None), numeric_colour(Some(&good)));
        assert_ne!(numeric_colour(Some(&good)), numeric_colour(Some(&bad)));
    }
}
