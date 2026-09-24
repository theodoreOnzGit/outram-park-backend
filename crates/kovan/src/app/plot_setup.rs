//! # The three-stage plot-setup form, shown before digitising a figure
//!
//! Cropping a figure out of the PDF reader used to drop the operator straight
//! into the digitiser with an empty panel: the four reference values blank,
//! both log checkboxes unticked, the axis labels empty, and the provenance
//! half-filled. Everything a figure's *caption and axes* already tell you had
//! to be typed in afterwards, interleaved with the actual pixel work, and the
//! two are different kinds of attention.
//!
//! This asks for them up front, in the order a reader's eye takes them off the
//! page (maintainer, 2026-09-24):
//!
//! | Stage | Asks for | Fills |
//! |---|---|---|
//! | 1. Figure | designation, document, page, notes | [`super::DigitiseApp`]'s provenance fields |
//! | 2. Axis ranges | min/max x and y, and whether either axis is logarithmic | `ref_val[0..4]`, `x_log`, `y_log` |
//! | 3. Axis labels | axis name and units, per axis | `x_label`, `y_label` |
//!
//! Afterwards the digitiser opens with all of it already in place, so the only
//! thing left is the part that genuinely needs the image: dragging the four
//! reference lines onto the axes and tracing the curve.
//!
//! ## Why the ranges are asked for, and the pixels are not
//!
//! A calibration is four `(pixel, value)` pairs. The **values** come off the
//! printed axis and can be typed while looking at the caption; the **pixels**
//! can only be placed against the image. Splitting them is the whole point:
//! this form takes the half that does not need the picture.
//!
//! ## What this does NOT do
//!
//! It does not validate that the figure's axes really are the range given, and
//! it cannot — that is what dragging the reference lines onto the tick marks
//! establishes. A wrong number typed here produces a wrong calibration exactly
//! as it would if typed in the digitiser; this changes *when* it is asked for,
//! not how much it is trusted.

use eframe::egui;

/// Which stage of the form is showing.
///
/// Ordered, and the order is the point — the figure identifies what is being
/// read, the ranges make the numbers meaningful, and the labels say what they
/// are. Each stage is answerable from the page without touching the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stage {
    /// Which figure, in which document, on which page.
    #[default]
    Figure,
    /// Axis ranges, and whether either axis is logarithmic.
    Ranges,
    /// Axis names and units.
    Labels,
}

impl Stage {
    /// `1`, `2` or `3`, for display.
    pub fn number(self) -> u8 {
        match self {
            Stage::Figure => 1,
            Stage::Ranges => 2,
            Stage::Labels => 3,
        }
    }

    /// The stage's own heading.
    pub fn title(self) -> &'static str {
        match self {
            Stage::Figure => "Figure",
            Stage::Ranges => "Axis ranges",
            Stage::Labels => "Axis labels and units",
        }
    }

    /// The next stage, or `None` on the last one (where the button is Finish).
    pub fn next(self) -> Option<Stage> {
        match self {
            Stage::Figure => Some(Stage::Ranges),
            Stage::Ranges => Some(Stage::Labels),
            Stage::Labels => None,
        }
    }

    /// The previous stage, or `None` on the first.
    pub fn previous(self) -> Option<Stage> {
        match self {
            Stage::Figure => None,
            Stage::Ranges => Some(Stage::Figure),
            Stage::Labels => Some(Stage::Ranges),
        }
    }
}

/// What the operator pressed on the form this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Still on the form.
    Continue,
    /// Finished — the caller should apply the answers and open the digitiser.
    Finish,
    /// Backed out of stage 1; the caller should return to the PDF reader and
    /// discard the crop.
    Cancel,
}

/// The form's own state: everything asked across the three stages.
///
/// Deliberately all `String`, including the numbers. A half-typed `-` or `1e`
/// is not an `f64` and must still be allowed to sit in the box while the rest
/// is typed; parsing happens at the boundary, where
/// [`Self::range_error`] reports it. This is the same choice the digitiser's
/// own `ref_val` makes, and it is why the two can be copied across directly.
#[derive(Debug, Clone, Default)]
pub struct PlotSetup {
    /// Which stage is showing.
    pub stage: Stage,
    // --- stage 1 ---
    /// Figure designation as printed, e.g. `"Fig. 7"`. Required.
    pub figure: String,
    /// Source document title or citation.
    pub document_title: String,
    /// Page the figure sits on.
    pub page: String,
    /// Free-text notes: which curve, any crop or skew worth recording.
    pub notes: String,
    // --- stage 2 ---
    /// Minimum x, at the left reference line.
    pub x_min: String,
    /// Maximum x, at the right reference line.
    pub x_max: String,
    /// Minimum y, at the bottom reference line.
    pub y_min: String,
    /// Maximum y, at the top reference line.
    pub y_max: String,
    /// Whether the x axis is decade-ruled.
    pub x_log: bool,
    /// Whether the y axis is decade-ruled.
    pub y_log: bool,
    // --- stage 3 ---
    /// x-axis label as printed, units included.
    pub x_label: String,
    /// y-axis label as printed, units included.
    pub y_label: String,
}

impl PlotSetup {
    /// Seed the form from what the crop already knows, and start at stage 1.
    ///
    /// The PDF reader can supply the page (and sometimes the figure) from the
    /// crop itself; re-asking for what is already known is how a form earns a
    /// reputation for wasting time.
    pub fn begin(
        figure: Option<String>,
        page: Option<u32>,
        document_title: Option<String>,
    ) -> Self {
        Self {
            stage: Stage::Figure,
            figure: figure.unwrap_or_default(),
            page: page.map(|p| p.to_string()).unwrap_or_default(),
            document_title: document_title.unwrap_or_default(),
            ..Self::default()
        }
    }

    /// Why stage 1 cannot be left, or `None` if it can.
    ///
    /// Only the figure designation is required, matching
    /// [`crate::digitiser::dataset::FigureSource::new`]'s own rule: a
    /// digitisation that cannot say which figure it read is not usable as
    /// evidence. Everything else on this stage is optional provenance.
    pub fn figure_error(&self) -> Option<&'static str> {
        if self.figure.trim().is_empty() {
            Some("a figure designation is required — a digitisation that cannot say which figure it read is not evidence")
        } else {
            None
        }
    }

    /// Why stage 2 cannot be left, or `None` if it can.
    ///
    /// Checks each field parses, that min and max differ (a zero-width axis
    /// has no calibration), and — for a logarithmic axis — that both ends are
    /// strictly positive, since the calibration is affine in `log10(value)`
    /// and `log10` of a non-positive number is not a number. Catching that
    /// here rather than at calibration time means the operator fixes it while
    /// still looking at the axis.
    pub fn range_error(&self) -> Option<String> {
        for (name, raw) in [
            ("x min", &self.x_min),
            ("x max", &self.x_max),
            ("y min", &self.y_min),
            ("y max", &self.y_max),
        ] {
            if raw.trim().is_empty() {
                return Some(format!("{name} is empty"));
            }
            if raw.trim().parse::<f64>().is_err() {
                return Some(format!("{name} is not a number: {:?}", raw.trim()));
            }
        }
        let parse = |s: &String| s.trim().parse::<f64>().unwrap_or(f64::NAN);
        let (x0, x1) = (parse(&self.x_min), parse(&self.x_max));
        let (y0, y1) = (parse(&self.y_min), parse(&self.y_max));
        if x0 == x1 {
            return Some("x min and x max are equal — the axis would have no extent".into());
        }
        if y0 == y1 {
            return Some("y min and y max are equal — the axis would have no extent".into());
        }
        if self.x_log && (x0 <= 0.0 || x1 <= 0.0) {
            return Some("a logarithmic x axis needs both ends strictly positive".into());
        }
        if self.y_log && (y0 <= 0.0 || y1 <= 0.0) {
            return Some("a logarithmic y axis needs both ends strictly positive".into());
        }
        None
    }

    /// Why stage 3 cannot be left, or `None` if it can.
    ///
    /// Both labels are required: an axis with no name is a column of numbers
    /// whose meaning lives only in whoever digitised it, which defeats the
    /// point of recording provenance at all.
    pub fn label_error(&self) -> Option<&'static str> {
        if self.x_label.trim().is_empty() {
            Some("the x-axis label is required — include its units")
        } else if self.y_label.trim().is_empty() {
            Some("the y-axis label is required — include its units")
        } else {
            None
        }
    }

    /// Whatever currently blocks leaving the showing stage.
    pub fn blocking_error(&self) -> Option<String> {
        match self.stage {
            Stage::Figure => self.figure_error().map(str::to_string),
            Stage::Ranges => self.range_error(),
            Stage::Labels => self.label_error().map(str::to_string),
        }
    }

    /// The four reference values in the digitiser's own `ref_val` order:
    /// `[X1, X2, Y1, Y2]`, i.e. `[x_min, x_max, y_min, y_max]`.
    ///
    /// Kept as one function so the ordering is stated once. Getting it wrong
    /// transposes an axis silently — the calibration still builds, and every
    /// digitised point is mirrored.
    pub fn reference_values(&self) -> [String; 4] {
        [
            self.x_min.trim().to_string(),
            self.x_max.trim().to_string(),
            self.y_min.trim().to_string(),
            self.y_max.trim().to_string(),
        ]
    }

    /// Draw the form. Returns what the operator pressed.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Outcome {
        let mut outcome = Outcome::Continue;

        ui.heading(format!(
            "Set up digitisation — step {} of 3: {}",
            self.stage.number(),
            self.stage.title()
        ));
        ui.label("Answer these off the figure's caption and axes; the digitiser opens with them filled in.");
        ui.separator();

        match self.stage {
            Stage::Figure => self.figure_stage(ui),
            Stage::Ranges => self.ranges_stage(ui),
            Stage::Labels => self.labels_stage(ui),
        }

        ui.separator();
        let blocking = self.blocking_error();
        if let Some(err) = &blocking {
            ui.colored_label(egui::Color32::from_rgb(230, 160, 60), err);
        }

        ui.horizontal(|ui| {
            match self.stage.previous() {
                Some(prev) => {
                    if ui.button("\u{2190} Back").clicked() {
                        self.stage = prev;
                    }
                }
                None => {
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Cancel;
                    }
                }
            }
            // The forward button is disabled, not hidden, while the stage is
            // incomplete: a button that vanishes reads as a bug, and the
            // message above says why it is unavailable.
            let last = self.stage.next().is_none();
            let label = if last {
                "Start digitising \u{2192}"
            } else {
                "Next \u{2192}"
            };
            let enabled = blocking.is_none();
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                match self.stage.next() {
                    Some(next) => self.stage = next,
                    None => outcome = Outcome::Finish,
                }
            }
        });

        outcome
    }

    fn figure_stage(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("plot-setup-figure")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("Figure *");
                ui.add(
                    egui::TextEdit::singleline(&mut self.figure)
                        .hint_text("Fig. 7  /  Figure 3(b)")
                        .desired_width(260.0),
                );
                ui.end_row();

                ui.label("Document");
                ui.add(
                    egui::TextEdit::singleline(&mut self.document_title)
                        .hint_text("title or citation")
                        .desired_width(260.0),
                );
                ui.end_row();

                ui.label("Page");
                ui.add(
                    egui::TextEdit::singleline(&mut self.page)
                        .hint_text("page number")
                        .desired_width(80.0),
                );
                ui.end_row();

                ui.label("Notes");
                ui.add(
                    egui::TextEdit::multiline(&mut self.notes)
                        .hint_text("which curve, any crop or known skew")
                        .desired_rows(2)
                        .desired_width(260.0),
                );
                ui.end_row();
            });
    }

    fn ranges_stage(&mut self, ui: &mut egui::Ui) {
        ui.label("The values at the ends of each axis, as printed. You will place the reference lines on the image next.");
        ui.add_space(4.0);
        egui::Grid::new("plot-setup-ranges")
            .num_columns(5)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label("");
                ui.label("min");
                ui.label("max");
                ui.label("");
                ui.label("");
                ui.end_row();

                ui.label("x axis");
                ui.add(egui::TextEdit::singleline(&mut self.x_min).desired_width(90.0));
                ui.add(egui::TextEdit::singleline(&mut self.x_max).desired_width(90.0));
                ui.checkbox(&mut self.x_log, "logarithmic");
                ui.label("");
                ui.end_row();

                ui.label("y axis");
                ui.add(egui::TextEdit::singleline(&mut self.y_min).desired_width(90.0));
                ui.add(egui::TextEdit::singleline(&mut self.y_max).desired_width(90.0));
                ui.checkbox(&mut self.y_log, "logarithmic");
                ui.label("");
                ui.end_row();
            });
        if self.x_log || self.y_log {
            ui.add_space(4.0);
            ui.weak("A logarithmic axis is calibrated in log10 space, so both ends must be > 0.");
        }
    }

    fn labels_stage(&mut self, ui: &mut egui::Ui) {
        ui.label("As printed on the figure, units included — these travel with the exported data.");
        ui.add_space(4.0);
        egui::Grid::new("plot-setup-labels")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label("x axis *");
                ui.add(
                    egui::TextEdit::singleline(&mut self.x_label)
                        .hint_text("Time (s)")
                        .desired_width(300.0),
                );
                ui.end_row();

                ui.label("y axis *");
                ui.add(
                    egui::TextEdit::singleline(&mut self.y_label)
                        .hint_text("Temperature (degC)")
                        .desired_width(300.0),
                );
                ui.end_row();
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled() -> PlotSetup {
        PlotSetup {
            stage: Stage::Figure,
            figure: "Fig. 7".into(),
            document_title: "Verfondern 1990".into(),
            page: "12".into(),
            notes: "upper curve".into(),
            x_min: "0".into(),
            x_max: "100".into(),
            y_min: "1".into(),
            y_max: "1000".into(),
            x_log: false,
            y_log: true,
            x_label: "Time (h)".into(),
            y_label: "Activity (Bq)".into(),
        }
    }

    #[test]
    fn the_stages_run_forward_and_back_in_order() {
        assert_eq!(Stage::Figure.next(), Some(Stage::Ranges));
        assert_eq!(Stage::Ranges.next(), Some(Stage::Labels));
        assert_eq!(Stage::Labels.next(), None, "the last stage finishes");
        assert_eq!(Stage::Figure.previous(), None, "the first stage cancels");
        assert_eq!(Stage::Labels.previous(), Some(Stage::Ranges));
        assert_eq!(
            (
                Stage::Figure.number(),
                Stage::Ranges.number(),
                Stage::Labels.number()
            ),
            (1, 2, 3)
        );
    }

    /// The figure designation is the one required field on stage 1, matching
    /// `FigureSource::new`'s own rule.
    #[test]
    fn stage_one_requires_only_the_figure() {
        let mut s = PlotSetup::default();
        assert!(s.figure_error().is_some(), "an empty figure blocks");
        s.figure = "   ".into();
        assert!(s.figure_error().is_some(), "whitespace is still empty");
        s.figure = "Fig. 3(b)".into();
        assert!(s.figure_error().is_none());
        // Everything else on the stage stays optional.
        assert!(s.document_title.is_empty() && s.page.is_empty() && s.notes.is_empty());
    }

    #[test]
    fn a_range_must_parse_and_have_extent() {
        let mut s = filled();
        s.stage = Stage::Ranges;
        assert!(s.range_error().is_none(), "{:?}", s.range_error());

        s.x_max = String::new();
        assert!(s.range_error().unwrap().contains("x max is empty"));

        s.x_max = "abc".into();
        assert!(s.range_error().unwrap().contains("not a number"));

        s.x_max = "0".into(); // equal to x_min
        assert!(
            s.range_error().unwrap().contains("no extent"),
            "a zero-width axis has no calibration"
        );
    }

    /// A logarithmic axis is calibrated in log10 space, so a non-positive end
    /// is not merely unusual -- `log10` of it is not a number. Catching it on
    /// the form means it is fixed while the operator still has the axis in
    /// front of them.
    #[test]
    fn a_logarithmic_axis_rejects_a_non_positive_end() {
        let mut s = filled();
        s.stage = Stage::Ranges;

        s.y_min = "0".into();
        assert!(
            s.range_error().unwrap().contains("strictly positive"),
            "log y with a zero end must be refused"
        );

        s.y_min = "-5".into();
        assert!(s.range_error().unwrap().contains("strictly positive"));

        // The same values are fine on a LINEAR axis.
        s.y_log = false;
        assert!(s.range_error().is_none(), "linear axes may cross zero");

        // And the check is per-axis: x is linear here and starts at 0.
        s.y_log = true;
        s.y_min = "1".into();
        assert_eq!(s.x_min, "0");
        assert!(
            s.range_error().is_none(),
            "a linear x may start at 0 while y is logarithmic"
        );
    }

    #[test]
    fn both_axis_labels_are_required() {
        let mut s = filled();
        s.stage = Stage::Labels;
        assert!(s.label_error().is_none());
        s.x_label = "  ".into();
        assert!(s.label_error().unwrap().contains("x-axis"));
        s.x_label = "Time (h)".into();
        s.y_label = String::new();
        assert!(s.label_error().unwrap().contains("y-axis"));
    }

    /// `blocking_error` must report the SHOWING stage, not the first
    /// incomplete one -- otherwise stage 1 would refuse to advance because
    /// stage 3 is empty, which is every stage's starting state.
    #[test]
    fn blocking_error_reports_only_the_showing_stage() {
        let mut s = PlotSetup::default();
        s.figure = "Fig. 1".into();
        s.stage = Stage::Figure;
        assert!(
            s.blocking_error().is_none(),
            "stage 1 is satisfied even though stages 2 and 3 are empty"
        );
        s.stage = Stage::Ranges;
        assert!(s.blocking_error().is_some(), "stage 2 is genuinely empty");
    }

    /// The digitiser's `ref_val` is `[X1, X2, Y1, Y2]`. Transposing it
    /// silently mirrors every digitised point -- the calibration still
    /// builds, so nothing else would catch it.
    #[test]
    fn reference_values_are_in_the_digitisers_order() {
        let s = filled();
        assert_eq!(
            s.reference_values(),
            [
                "0".to_string(),
                "100".to_string(),
                "1".to_string(),
                "1000".to_string()
            ],
            "order is [x_min, x_max, y_min, y_max]"
        );
    }

    #[test]
    fn begin_seeds_what_the_crop_already_knew() {
        let s = PlotSetup::begin(
            Some("Fig. 4".into()),
            Some(31),
            Some("Nabielek 1984".into()),
        );
        assert_eq!(s.stage, Stage::Figure);
        assert_eq!(s.figure, "Fig. 4");
        assert_eq!(
            s.page, "31",
            "page arrives as text, as the digitiser holds it"
        );
        assert_eq!(s.document_title, "Nabielek 1984");

        // Nothing known is nothing seeded, not a panic.
        let blank = PlotSetup::begin(None, None, None);
        assert!(blank.figure.is_empty() && blank.page.is_empty());
        assert!(blank.figure_error().is_some());
    }
}
