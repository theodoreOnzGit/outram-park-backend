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

use crate::digitiser::raster::{PlotRaster, Quarter};

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
#[derive(Debug, Clone)]
pub struct PlotSetup {
    /// Which stage is showing.
    pub stage: Stage,
    /// Quarter-turn applied to the crop before digitising.
    ///
    /// Scanned reports set a wide plot sideways on the page often enough
    /// that it is worth a button (maintainer, 2026-09-24: "the last 3 plots
    /// are rotated 90 degrees"). Digitising a sideways figure would transpose
    /// every axis -- the calibration entered against the wrong axis and every
    /// exported point swapped -- so this is fixed here, before any of that is
    /// entered, rather than left for the operator to compensate for mentally.
    pub turn: Quarter,
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
    /// Whether to overlay a true-horizontal/vertical grid on the preview.
    ///
    /// Judging a 2-degree skew by eye against nothing is hopeless; against a
    /// straight line it is easy. The grid is drawn in **screen space**, so
    /// its lines are exactly horizontal and vertical whatever the image is
    /// doing — which is the point, since the figure's own axes are what is
    /// being compared to them.
    pub show_grid: bool,
    /// Grid spacing in screen pixels.
    pub grid_spacing: f32,
    /// Fine deskew in degrees, positive clockwise, for a scan that went
    /// through the feeder crooked. Clamped to
    /// ±[`PlotRaster::MAX_DESKEW_DEGREES`].
    ///
    /// Unlike [`Self::turn`] this **resamples**, so the host re-derives it
    /// from the untouched original every time rather than applying it on
    /// top of the last result — otherwise nudging the slider ten times
    /// would blur the figure ten times over.
    pub skew_degrees: f64,
}

/// Hand-written rather than derived: a derived `Default` would leave
/// `grid_spacing` at 0.0, which the painter clamps to a mesh so dense it is
/// useless. Every other field's zero value is the right one.
impl Default for PlotSetup {
    fn default() -> Self {
        Self {
            stage: Stage::default(),
            turn: Quarter::default(),
            show_grid: false,
            grid_spacing: 40.0,
            skew_degrees: 0.0,
            figure: String::new(),
            document_title: String::new(),
            page: String::new(),
            notes: String::new(),
            x_min: String::new(),
            x_max: String::new(),
            y_min: String::new(),
            y_max: String::new(),
            x_log: false,
            y_log: false,
            x_label: String::new(),
            y_label: String::new(),
        }
    }
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

    /// Draw the form beside the cropped figure. Returns what the operator
    /// pressed.
    ///
    /// `figure` is the crop's texture and its pixel size. Showing it is not
    /// decoration: **every answer on stages 2 and 3 is read off the image**
    /// -- the axis ranges from its tick labels, the axis names and units from
    /// its axis captions. A form that asks for them without showing the
    /// figure is asking the operator to remember a picture they were looking
    /// at a moment ago (maintainer, 2026-09-24).
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        figure: Option<(egui::TextureId, egui::Vec2)>,
    ) -> Outcome {
        match figure {
            Some((texture, size)) => {
                // The figure takes the larger share and is resizable: reading
                // a log axis's minor ticks sometimes needs most of the window.
                let width = (ui.available_width() * 0.55).max(240.0);
                egui::Panel::left("plot-setup-figure-preview")
                    .resizable(true)
                    .default_size(width)
                    .show(ui, |ui| self.figure_preview(ui, texture, size));
                self.form_ui(ui)
            }
            // No raster (the form reached from somewhere that has no crop):
            // the form still works, it is just harder to fill in.
            None => self.form_ui(ui),
        }
    }

    /// The cropped figure, scaled to fit the panel width and scrollable when
    /// the operator resizes past it.
    fn figure_preview(&mut self, ui: &mut egui::Ui, texture: egui::TextureId, size: egui::Vec2) {
        ui.horizontal(|ui| {
            ui.strong("Cropped figure");
            ui.weak(format!("{} x {} px", size.x as u32, size.y as u32));
            if ui
                .button("\u{21BB} Rotate")
                .on_hover_text("quarter-turn clockwise, for a figure printed sideways")
                .clicked()
            {
                self.turn = self.turn.next_clockwise();
            }
            if self.turn != Quarter::None {
                ui.weak(format!("{}\u{00B0}", self.turn.degrees()));
            }
        });
        ui.horizontal(|ui| {
            ui.label("skew");
            ui.add(
                egui::Slider::new(
                    &mut self.skew_degrees,
                    -PlotRaster::MAX_DESKEW_DEGREES..=PlotRaster::MAX_DESKEW_DEGREES,
                )
                .suffix("\u{00B0}")
                .step_by(0.1),
            )
            .on_hover_text(
                "straighten a crooked scan. This RESAMPLES the image, unlike the \
                 quarter turns — for the most accurate result on a badly skewed \
                 plot, leave this at 0 and use the Parallelogram calibration in \
                 the digitiser instead, which corrects skew without touching a \
                 single pixel.",
            );
            if self.skew_degrees != 0.0 && ui.small_button("reset").clicked() {
                self.skew_degrees = 0.0;
            }
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.show_grid, "grid")
                .on_hover_text("true horizontal/vertical lines to judge the skew against");
            if self.show_grid {
                ui.add(
                    egui::Slider::new(&mut self.grid_spacing, 10.0..=120.0)
                        .text("spacing")
                        .step_by(1.0),
                );
            }
        });
        if self.skew_degrees != 0.0 {
            ui.weak("resampled — the Parallelogram calibration corrects skew losslessly");
        }
        ui.separator();
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Fit to the panel width, but never enlarge past 1:1 -- a
                // scanned figure upscaled past its own resolution just looks
                // blurry and reads no better.
                let avail = ui.available_width().max(1.0);
                let scale = (avail / size.x.max(1.0)).min(1.0);
                let shown = size * scale;
                let (rect, _) = ui.allocate_exact_size(shown, egui::Sense::hover());
                let painter = ui.painter_at(rect);
                painter.image(
                    texture,
                    rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                if self.show_grid {
                    Self::paint_grid(&painter, rect, self.grid_spacing);
                }
            });
    }

    /// Draw the reference grid over `rect`.
    ///
    /// Translucent, and every fourth line is stronger — a uniform mesh is
    /// hard to track across a busy figure, whereas a coarse line every few
    /// gives the eye something to follow along a plot axis. Drawn **after**
    /// the image so it is visible over dark ink, and clipped to the image so
    /// it cannot spill into the form.
    fn paint_grid(painter: &egui::Painter, rect: egui::Rect, spacing: f32) {
        let spacing = spacing.max(4.0);
        let fine = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(0, 170, 255, 70));
        let bold = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(0, 170, 255, 150));

        let mut i = 0;
        let mut x = rect.min.x;
        while x <= rect.max.x {
            let s = if i % 4 == 0 { bold } else { fine };
            painter.line_segment([egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)], s);
            x += spacing;
            i += 1;
        }
        let mut j = 0;
        let mut y = rect.min.y;
        while y <= rect.max.y {
            let s = if j % 4 == 0 { bold } else { fine };
            painter.line_segment([egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)], s);
            y += spacing;
            j += 1;
        }
    }

    /// The three-stage form itself.
    fn form_ui(&mut self, ui: &mut egui::Ui) -> Outcome {
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
                Self::decade_readout(ui, self.x_log, &self.x_min, &self.x_max);
                ui.end_row();

                ui.label("y axis");
                ui.add(egui::TextEdit::singleline(&mut self.y_min).desired_width(90.0));
                ui.add(egui::TextEdit::singleline(&mut self.y_max).desired_width(90.0));
                ui.checkbox(&mut self.y_log, "logarithmic");
                Self::decade_readout(ui, self.y_log, &self.y_min, &self.y_max);
                ui.end_row();
            });
        if self.x_log || self.y_log {
            ui.add_space(4.0);
            ui.weak(
                "A logarithmic axis is calibrated in log10 space, so both ends must be > 0. \
                 Enter the VALUE, not the exponent: the bottom of a 10^-6 gridline is 1e-6, \
                 and the 10^0 gridline is 1.",
            );
        }
    }

    /// How many decades a logarithmic axis spans, shown beside it.
    ///
    /// # Why this is worth a widget
    ///
    /// On a log axis people think in powers of ten, and the natural thing to
    /// type for the `10^0` gridline is `10^0` — which Rust's `f64` parser
    /// rejects outright, so the only thing that *will* parse is the wrong
    /// number, `10`. That is exactly what happened to Figs. 6, 7 and 8 of the
    /// PANAMA report (maintainer, 2026-09-24: "it was supposed to be 1, not
    /// 10, like 10^0 was what i entered"): three figures digitised over
    /// **seven** decades where **six** are plotted, stretching every ordinate
    /// in the log by 7/6.
    ///
    /// It went unnoticed because nothing on screen ever said how many decades
    /// the axis covered, and a wrong-by-one-decade calibration produces a
    /// perfectly plausible-looking curve. A reader who can see "7 decades"
    /// against a plot showing six catches it in a second; the residuals took
    /// a model-free consistency argument across three figures to find.
    ///
    /// Shown for the log case only — a linear axis has no decades and the
    /// readout would be noise.
    fn decade_readout(ui: &mut egui::Ui, is_log: bool, min: &str, max: &str) {
        if !is_log {
            ui.label("");
            return;
        }
        match Self::decades(min, max) {
            Some(decades) => {
                ui.weak(format!("{decades:.3} decades")).on_hover_text(
                    "count the gridlines on the figure: this must match. A \
                         calibration one decade out still draws a plausible curve.",
                );
            }
            None => {
                ui.label("");
            }
        }
    }

    /// The decades a logarithmic axis spans, or `None` if the ends are not
    /// two positive numbers with `max > min`.
    ///
    /// Split out from [`Self::decade_readout`] so the arithmetic is testable
    /// without standing up a `Ui` — the value of this feature is entirely in
    /// the number being right.
    fn decades(min: &str, max: &str) -> Option<f64> {
        let parse = |s: &str| s.trim().parse::<f64>().ok().filter(|v| *v > 0.0);
        match (parse(min), parse(max)) {
            (Some(lo), Some(hi)) if hi > lo => Some((hi / lo).log10()),
            _ => None,
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
            turn: Quarter::None,
            skew_degrees: 0.0,
            show_grid: false,
            grid_spacing: 40.0,
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

    /// The form must work with no raster -- reached from anywhere that has no
    /// crop, it is only harder to fill in, not broken. Pinning the `None` arm
    /// keeps the preview optional rather than load-bearing.
    #[test]
    fn the_form_does_not_require_a_figure_to_exist() {
        let s = PlotSetup::begin(Some("Fig. 1".into()), None, None);
        // Nothing about validity depends on a texture being present.
        assert!(s.figure_error().is_none());
        assert_eq!(s.stage, Stage::Figure);
    }

    /// The grid must start at a usable spacing. A derived `Default` would
    /// leave it at 0.0, which the painter clamps to a 4 px mesh -- dense
    /// enough to obscure the figure it exists to help judge.
    #[test]
    fn the_grid_defaults_to_a_usable_spacing() {
        let s = PlotSetup::default();
        assert!(!s.show_grid, "off until asked for");
        assert!(
            (20.0..=80.0).contains(&s.grid_spacing),
            "grid spacing {} is not a usable default",
            s.grid_spacing
        );
        // `begin` inherits it rather than resetting to zero.
        let b = PlotSetup::begin(Some("Fig. 8".into()), None, None);
        assert_eq!(b.grid_spacing, s.grid_spacing);
    }

    /// The deskew starts at zero: a wizard that silently rotated a figure
    /// the operator had not asked to rotate would be worse than no feature.
    #[test]
    fn the_transforms_start_as_no_ops() {
        let s = PlotSetup::default();
        assert_eq!(s.turn, Quarter::None);
        assert_eq!(s.skew_degrees, 0.0);
        let b = PlotSetup::begin(None, None, None);
        assert_eq!(b.turn, Quarter::None);
        assert_eq!(b.skew_degrees, 0.0);
    }

    /// **The readout that would have caught the PANAMA digitisation slip.**
    ///
    /// Figs. 6, 7 and 8 plot six decades, `10^-6` to `10^0`. Entering `10`
    /// for the top gridline instead of `1` gives SEVEN — and a wrong-by-one-
    /// decade calibration still draws a perfectly plausible curve, which is
    /// why it went unnoticed until a model-free consistency argument across
    /// three figures found it.
    #[test]
    fn the_decade_count_distinguishes_the_panama_slip() {
        let correct = PlotSetup::decades("1e-6", "1").expect("both positive");
        let slipped = PlotSetup::decades("1e-6", "10").expect("both positive");
        assert!(
            (correct - 6.0).abs() < 1e-9,
            "1e-6 to 1 is six decades, got {correct}"
        );
        assert!(
            (slipped - 7.0).abs() < 1e-9,
            "1e-6 to 10 is seven decades, got {slipped}"
        );
        assert!(
            (slipped - correct - 1.0).abs() < 1e-9,
            "the slip must read as exactly one decade more"
        );
    }

    /// A log axis needs two positive numbers in increasing order; anything
    /// else shows nothing rather than a misleading figure.
    #[test]
    fn the_decade_count_refuses_what_it_cannot_measure() {
        assert_eq!(PlotSetup::decades("0", "10"), None, "zero has no logarithm");
        assert_eq!(PlotSetup::decades("-1", "10"), None, "nor does a negative");
        assert_eq!(PlotSetup::decades("1", "1"), None, "a zero-width axis");
        assert_eq!(PlotSetup::decades("10", "1"), None, "reversed ends");
        assert_eq!(PlotSetup::decades("", "10"), None);
        assert_eq!(PlotSetup::decades("10^0", "10"), None, "^ does not parse");
    }

    /// Scientific notation is what a log axis is usually typed in, and it
    /// must parse — `1e-6` is the form the hint text recommends.
    #[test]
    fn scientific_notation_parses() {
        for (lo, hi, want) in [
            ("1e-6", "1e0", 6.0),
            ("1E-6", "1E1", 7.0),
            ("0.001", "1000", 6.0),
        ] {
            let got = PlotSetup::decades(lo, hi).expect("parses");
            assert!(
                (got - want).abs() < 1e-9,
                "{lo}..{hi}: got {got}, want {want}"
            );
        }
    }
}
