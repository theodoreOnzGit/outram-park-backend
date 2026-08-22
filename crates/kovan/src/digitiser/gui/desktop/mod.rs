mod csv_preview;
mod pdf_reader;
mod theme;

use eframe::egui::{
    self, Color32, ComboBox, Key, PointerButton, Pos2, Rect, Sense, Stroke, TextureHandle,
    TextureOptions, Vec2,
};
use egui_file_dialog::FileDialog;

use crate::digitiser::auto::{auto_digitise, AutoDigitiseConfig, AxisPixelRefs, AxisValueSpec};
use crate::digitiser::calibration::{AxisCalibration, AxisRef, AxisScale, PlotCalibration};
use crate::digitiser::dataset::{
    uncertainty_interval, utc_now_iso8601, DigitisedDataset, DigitisedPoint, FigureSource,
    PointOrigin, ReviewInterface, ReviewStatus, DATASET_SCHEMA_VERSION,
};
use crate::digitiser::detect::DetectConfig;
use crate::digitiser::raster::PlotRaster;
use crate::digitiser::trace::{CurveSelector, TraceConfig, TraceStrategy};

use csv_preview::draw_csv_preview;
use pdf_reader::PdfReaderState;
use theme::GuiTheme;

/// Which top-level panel is showing — the plot digitiser (the window's
/// original purpose) or the integrated PDF reader (op-95x6). A closed set,
/// switched with a top-bar button row rather than a popup/new-window (the
/// window itself already is the "new tab" GitHub issue #30 asked for the
/// plot-digitiser popup to attach to — see op-p17q, not yet implemented).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum View {
    Digitiser,
    #[default]
    PdfReader,
}

/// Which action a pending file-dialog pick should feed into. One
/// [`FileDialog`] instance is shared by every "open a file" button in this
/// window (op-689u: "file picker for the digitiser (and PDF reader once it
/// exists)") rather than one picker per action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileDialogTarget {
    /// Picked path becomes the digitiser's plot image ([`DigitiseApp::load_image`]).
    Image,
    /// Picked path is opened in the PDF reader ([`PdfReaderState::open`]).
    Pdf,
    /// Picked path becomes the dataset JSON export path (op-jtna).
    JsonExport,
    /// Picked path becomes the dataset CSV export path (op-jtna).
    CsvExport,
}

impl FileDialogTarget {
    /// Whether this target opens an existing file (`pick_file`) or names a
    /// new one to write (`save_file`) — see [`DigitiseApp::open_picker`].
    fn is_save(self) -> bool {
        matches!(self, Self::JsonExport | Self::CsvExport)
    }

    /// The file-filter name (matching one of the names registered on
    /// [`FileDialog`] in [`DigitiseApp::default`]) this target should default
    /// to, so "Open PDF…" doesn't come up filtered to "Images" and
    /// vice versa (op-nje6).
    fn default_filter(self) -> &'static str {
        match self {
            Self::Image => "Images",
            Self::Pdf => "PDF",
            Self::JsonExport => "JSON",
            Self::CsvExport => "CSV",
        }
    }
}

/// What a click on the image currently means. Closed set, enum-dispatched.
///
/// **The four axis-reference lines are no longer a `ClickMode` step
/// (op-zfnh).** Previously calibrating meant cycling through
/// `SetXRef1 -> SetXRef2 -> SetYRef1 -> SetYRef2`, one click each, with only
/// the already-set lines drawn. Per GitHub issue #30 ("graphReader uses a
/// persistent box rather than manually clicking the four coordinates"), all
/// four lines now appear together as soon as an image loads (seeded at
/// 10%/90% of the image extent — see [`DigitiseApp::load_image`]) and are
/// draggable at any time, independent of `mode` — see
/// [`DigitiseApp::ref_dragging`] and `image_panel`'s reference-line hit test.
/// This keeps the existing axis-aligned [`crate::digitiser::calibration::PlotCalibration`]
/// model (columns for x, rows for y); a parallelogram/skewed variant for
/// off-centre plots is a separate, schema-affecting decision (tracked as
/// op-vyb9), not implemented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClickMode {
    /// Select / drag existing points.
    EditPoints,
    /// Double-click adds a hand-placed point (op-8ixa).
    AddPoint,
}

/// All GUI state, owned by value (no lifetimes, no shared state).
pub struct DigitiseApp {
    // chrome
    view: View,
    theme: GuiTheme,
    file_dialog: FileDialog,
    file_dialog_target: Option<FileDialogTarget>,
    // PDF reader (op-95x6)
    pdf_reader: PdfReaderState,
    // image
    image_path: String,
    raster: Option<PlotRaster>,
    texture: Option<TextureHandle>,
    zoom: f32,
    // calibration input
    mode: ClickMode,
    ref_px: [Option<f64>; 4], // x1, x2 (columns); y1, y2 (rows)
    ref_val: [String; 4],
    /// Which of the four reference lines (indices into `ref_px`/`ref_val`,
    /// same order) is currently being dragged, if any — op-zfnh's persistent
    /// draggable box. `None` when the pointer isn't holding a reference line.
    ref_dragging: Option<usize>,
    x_log: bool,
    y_log: bool,
    // trace tuning
    threshold: u8,
    step: u32,
    strategy: TraceStrategy,
    // provenance input
    figure: String,
    document_title: String,
    document_id: String,
    page: String,
    notes: String,
    x_label: String,
    y_label: String,
    operator: String,
    // result
    dataset: Option<DigitisedDataset>,
    selected: Option<usize>,
    dragging: Option<usize>,
    json_out: String,
    csv_out: String,
    message: String,
}

impl Default for DigitiseApp {
    fn default() -> Self {
        Self {
            view: View::default(),
            theme: GuiTheme::default(),
            file_dialog: FileDialog::new()
                .add_file_filter_extensions("Images", vec!["png", "jpg", "jpeg"])
                .add_file_filter_extensions("PDF", vec!["pdf"])
                .default_file_filter("Images"),
            file_dialog_target: None,
            pdf_reader: PdfReaderState::default(),
            image_path: String::new(),
            raster: None,
            texture: None,
            zoom: 1.0,
            mode: ClickMode::EditPoints,
            ref_px: [None; 4],
            ref_val: Default::default(),
            ref_dragging: None,
            x_log: false,
            y_log: false,
            threshold: 128,
            step: 1,
            strategy: TraceStrategy::ContinuityNearest,
            figure: String::new(),
            document_title: String::new(),
            document_id: String::new(),
            page: String::new(),
            notes: String::new(),
            x_label: "x".to_string(),
            y_label: "y".to_string(),
            operator: String::new(),
            dataset: None,
            selected: None,
            dragging: None,
            json_out: String::new(),
            csv_out: String::new(),
            message: "load an image, then click the four axis reference points".to_string(),
        }
    }
}

impl DigitiseApp {
    /// Load `path` as the working plot image (PNG/JPEG).
    ///
    /// Seeds the four axis-reference lines at 10%/90% of the new image's
    /// extent (op-zfnh's persistent box, replacing the old one-click-per-line
    /// flow) — a plot's axes are rarely at the very edge of the figure, so
    /// this typically starts closer to correct than the previous "nothing
    /// set yet" state, and every line is immediately visible and draggable
    /// regardless.
    pub fn load_image(&mut self, path: &str) {
        match PlotRaster::from_path(std::path::Path::new(path)) {
            Ok(r) => {
                self.image_path = path.to_string();
                if self.json_out.is_empty() {
                    self.json_out = format!("{path}.digitised.json");
                }
                let (w, h) = (r.width() as f64, r.height() as f64);
                self.ref_px = [w * 0.1, w * 0.9, h * 0.9, h * 0.1].map(Some);
                self.raster = Some(r);
                self.texture = None; // re-uploaded next frame
                self.dataset = None;
                self.selected = None;
                self.ref_dragging = None;
                self.message = format!(
                    "loaded {path} — drag the four reference lines into place, fill their values"
                );
            }
            Err(e) => self.message = e.to_string(),
        }
    }

    /// Build the calibration the four reference points + values describe.
    fn calibration(&self) -> Result<PlotCalibration, String> {
        let px = |i: usize, what: &str| {
            self.ref_px[i].ok_or_else(|| format!("{what} pixel not set — drag its line into place"))
        };
        let val = |i: usize, what: &str| {
            self.ref_val[i]
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("{what} value {:?} is not a number", self.ref_val[i]))
        };
        let scale = |log: bool| {
            if log {
                AxisScale::Logarithmic
            } else {
                AxisScale::Linear
            }
        };
        let x = AxisCalibration::new(
            scale(self.x_log),
            AxisRef {
                pixel: px(0, "X1")?,
                value: val(0, "X1")?,
            },
            AxisRef {
                pixel: px(1, "X2")?,
                value: val(1, "X2")?,
            },
        )
        .map_err(|e| e.to_string())?;
        let y = AxisCalibration::new(
            scale(self.y_log),
            AxisRef {
                pixel: px(2, "Y1")?,
                value: val(2, "Y1")?,
            },
            AxisRef {
                pixel: px(3, "Y2")?,
                value: val(3, "Y2")?,
            },
        )
        .map_err(|e| e.to_string())?;
        Ok(PlotCalibration { x, y })
    }

    fn source(&self, raster: &PlotRaster) -> Result<FigureSource, String> {
        let mut s = FigureSource::new(self.figure.clone()).map_err(|e| e.to_string())?;
        s.document_title =
            (!self.document_title.trim().is_empty()).then(|| self.document_title.clone());
        s.document_id = (!self.document_id.trim().is_empty()).then(|| self.document_id.clone());
        s.page = self.page.trim().parse::<u32>().ok();
        s.notes = (!self.notes.trim().is_empty()).then(|| self.notes.clone());
        s.image_path = Some(self.image_path.clone());
        s.image_sha256 = raster.source_sha256().map(str::to_string);
        Ok(s)
    }

    fn operator_name(&self) -> String {
        let t = self.operator.trim();
        if t.is_empty() {
            "unnamed operator".to_string()
        } else {
            t.to_string()
        }
    }

    /// The automatic pass: trace with the current calibration and tuning.
    fn auto_trace(&mut self) {
        let Some(raster) = &self.raster else {
            self.message = "load an image first".to_string();
            return;
        };
        let cal = match self.calibration() {
            Ok(c) => c,
            Err(e) => {
                self.message = e;
                return;
            }
        };
        let source = match self.source(raster) {
            Ok(s) => s,
            Err(e) => {
                self.message = e;
                return;
            }
        };
        let config = AutoDigitiseConfig {
            x: AxisValueSpec {
                scale: cal.x.scale,
                refs: AxisPixelRefs::Explicit {
                    r1: cal.x.r1,
                    r2: cal.x.r2,
                },
            },
            y: AxisValueSpec {
                scale: cal.y.scale,
                refs: AxisPixelRefs::Explicit {
                    r1: cal.y.r1,
                    r2: cal.y.r2,
                },
            },
            detect: DetectConfig::default(),
            trace: TraceConfig {
                selector: CurveSelector::DarkestBand {
                    max_luminance: self.threshold,
                },
                strategy: self.strategy,
                column_step: self.step,
                inset: 3,
                max_column_fill: 0.6,
            },
        };
        match auto_digitise(
            raster,
            &config,
            source,
            self.x_label.clone(),
            self.y_label.clone(),
            format!("{} via kovan (gui)", self.operator_name()),
            utc_now_iso8601(),
        ) {
            Ok(d) => {
                self.message = format!(
                    "auto pass traced {} points — verify, correct, then mark reviewed",
                    d.points.len()
                );
                self.dataset = Some(d);
                self.selected = None;
                self.mode = ClickMode::EditPoints;
            }
            Err(e) => self.message = e.to_string(),
        }
    }

    /// Start an empty dataset from the calibration alone, for figures
    /// digitised entirely by hand-placed points.
    fn start_empty(&mut self) {
        let Some(raster) = &self.raster else {
            self.message = "load an image first".to_string();
            return;
        };
        let cal = match self.calibration() {
            Ok(c) => c,
            Err(e) => {
                self.message = e;
                return;
            }
        };
        let source = match self.source(raster) {
            Ok(s) => s,
            Err(e) => {
                self.message = e;
                return;
            }
        };
        self.dataset = Some(DigitisedDataset {
            schema_version: DATASET_SCHEMA_VERSION,
            source,
            calibration: cal,
            x_label: self.x_label.clone(),
            y_label: self.y_label.clone(),
            digitised_by: format!("{} via kovan (gui, hand-placed)", self.operator_name()),
            digitised_at: utc_now_iso8601(),
            trace: None,
            review: ReviewStatus::Unreviewed,
            points: Vec::new(),
        });
        self.selected = None;
        self.mode = ClickMode::AddPoint;
        self.message = "empty dataset started — click to place points".to_string();
    }

    /// Any edit invalidates a recorded review.
    fn mark_edited(&mut self) {
        if let Some(d) = &mut self.dataset {
            if matches!(d.review, ReviewStatus::Reviewed { .. }) {
                d.review = ReviewStatus::Unreviewed;
                self.message = "edited after review — status reset to UNREVIEWED".to_string();
            }
        }
    }

    fn set_point_pixels(&mut self, idx: usize, px: f64, py: f64, hand_placed: bool) {
        let by = self.operator_name();
        let Some(d) = &mut self.dataset else { return };
        let cal = d.calibration;
        let Some(p) = d.points.get_mut(idx) else {
            return;
        };
        p.x_px = Some(px);
        p.y_px = Some(py);
        (p.x, p.y) = cal.point_at(px, py);
        (p.x_minus, p.x_plus) = uncertainty_interval(&cal.x, px, 0.5);
        (p.y_minus, p.y_plus) = uncertainty_interval(&cal.y, py, 0.5);
        p.origin = if hand_placed || matches!(p.origin, PointOrigin::HandPlaced { .. }) {
            PointOrigin::HandPlaced { by }
        } else {
            PointOrigin::HandCorrected { by }
        };
    }

    fn add_point(&mut self, px: f64, py: f64) {
        let Some(d) = &mut self.dataset else {
            self.message = "run the auto pass or Start empty first".to_string();
            return;
        };
        let idx = d
            .points
            .iter()
            .position(|p| p.x_px.unwrap_or(f64::MAX) > px)
            .unwrap_or(d.points.len());
        d.points.insert(
            idx,
            DigitisedPoint {
                x: 0.0,
                y: 0.0,
                x_minus: 0.0,
                x_plus: 0.0,
                y_minus: 0.0,
                y_plus: 0.0,
                x_px: Some(px),
                y_px: Some(py),
                origin: PointOrigin::HandPlaced { by: String::new() },
            },
        );
        self.set_point_pixels(idx, px, py, true);
        self.selected = Some(idx);
        self.mark_edited();
    }

    fn delete_selected(&mut self) {
        let Some(i) = self.selected else { return };
        if let Some(d) = &mut self.dataset {
            if i < d.points.len() {
                d.points.remove(i);
                self.selected = None;
                self.mark_edited();
                self.message = "point deleted".to_string();
            }
        }
    }

    /// Remove every marker, keeping the calibration/provenance already
    /// entered — op-8ixa's "clear all markers button".
    fn clear_all_points(&mut self) {
        if let Some(d) = &mut self.dataset {
            let n = d.points.len();
            d.points.clear();
            self.selected = None;
            self.mark_edited();
            self.message = format!("cleared {n} marker(s)");
        }
    }

    fn save(&mut self, reviewed: bool) {
        if reviewed {
            let by = self.operator_name();
            if let Some(d) = &mut self.dataset {
                d.record_review(by, utc_now_iso8601(), ReviewInterface::Gui);
            }
        }
        let Some(d) = &self.dataset else {
            self.message = "nothing to save".to_string();
            return;
        };
        if self.json_out.trim().is_empty() {
            self.message = "set a JSON output path".to_string();
            return;
        }
        if let Err(e) = d.write_json(std::path::Path::new(self.json_out.trim())) {
            self.message = e.to_string();
            return;
        }
        let mut saved = format!("saved {}", self.json_out.trim());
        if !self.csv_out.trim().is_empty() {
            match d.write_csv(std::path::Path::new(self.csv_out.trim())) {
                Ok(()) => saved.push_str(&format!(" and {}", self.csv_out.trim())),
                Err(e) => {
                    self.message = format!("json saved, csv failed: {e}");
                    return;
                }
            }
        }
        self.message = saved;
    }

    /// Nearest point (index) to image-pixel position, within `max_px`.
    fn nearest_point(&self, px: f64, py: f64, max_px: f64) -> Option<usize> {
        let d = self.dataset.as_ref()?;
        let mut best: Option<(usize, f64)> = None;
        for (i, p) in d.points.iter().enumerate() {
            let (Some(x), Some(y)) = (p.x_px, p.y_px) else {
                continue;
            };
            let dist = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            if dist <= max_px && best.is_none_or(|(_, bd)| dist < bd) {
                best = Some((i, dist));
            }
        }
        best.map(|(i, _)| i)
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("kovan — graph digitiser");
        ui.horizontal(|ui| {
            ui.label("image:");
            ui.text_edit_singleline(&mut self.image_path);
        });
        ui.horizontal(|ui| {
            if ui.button("Load image").clicked() {
                let p = self.image_path.clone();
                self.load_image(&p);
            }
            if ui.button("Browse…").clicked() {
                self.open_picker(FileDialogTarget::Image);
            }
        });
        ui.add(egui::Slider::new(&mut self.zoom, 0.25..=4.0).text("zoom"));
        ui.separator();

        ui.label("1. Axis references — drag the 4 lines on the image into place:");
        let labels = ["X1 (column)", "X2 (column)", "Y1 (row)", "Y2 (row)"];
        for i in 0..4 {
            ui.horizontal(|ui| {
                ui.label(labels[i]);
                ui.label(match self.ref_px[i] {
                    Some(p) => format!("px {p:.0}"),
                    None => "px —".to_string(),
                });
                ui.label("=");
                ui.add(egui::TextEdit::singleline(&mut self.ref_val[i]).desired_width(70.0));
            });
        }
        ui.checkbox(&mut self.x_log, "x axis logarithmic");
        ui.checkbox(&mut self.y_log, "y axis logarithmic");
        ui.separator();

        ui.label("2. Automatic pass:");
        ui.add(egui::Slider::new(&mut self.threshold, 1..=254).text("ink threshold"));
        ui.add(egui::Slider::new(&mut self.step, 1..=20).text("column step"));
        ComboBox::from_label("strategy")
            .selected_text(format!("{:?}", self.strategy))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.strategy,
                    TraceStrategy::ContinuityNearest,
                    "ContinuityNearest",
                );
                ui.selectable_value(
                    &mut self.strategy,
                    TraceStrategy::LargestRun,
                    "LargestRun",
                );
                ui.selectable_value(
                    &mut self.strategy,
                    TraceStrategy::ColumnCentroid,
                    "ColumnCentroid",
                );
            });
        ui.horizontal(|ui| {
            if ui.button("Auto-trace").clicked() {
                self.auto_trace();
            }
            if ui.button("Start empty (hand-place)").clicked() {
                self.start_empty();
            }
        });
        ui.separator();

        ui.label("3. Verify & correct:");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.mode, ClickMode::EditPoints, "Edit/drag");
            ui.selectable_value(&mut self.mode, ClickMode::AddPoint, "Add points");
            if ui.button("Delete selected").clicked() {
                self.delete_selected();
            }
            if ui.button("Clear all").clicked() {
                self.clear_all_points();
            }
        });
        ui.small(
            "double-click adds a marker (Add points mode) · right-click removes the nearest one",
        );
        ui.separator();

        ui.label("4. Provenance (required to export):");
        let field = |ui: &mut egui::Ui, name: &str, s: &mut String| {
            ui.horizontal(|ui| {
                ui.label(name);
                ui.text_edit_singleline(s);
            });
        };
        // op-5ecn: hover tooltips on the fields the maintainer asked what they
        // mean — figure, document title/id, page, operator. Each tooltip sits
        // on the field's own label via `.on_hover_text`, not a separate `?`
        // icon, so there is nothing extra to click.
        let field_tip = |ui: &mut egui::Ui, name: &str, tip: &str, s: &mut String| {
            ui.horizontal(|ui| {
                ui.label(name).on_hover_text(tip);
                ui.text_edit_singleline(s);
            });
        };
        field_tip(
            ui,
            "figure*",
            "The figure's own identifier/caption in the source document \
             (e.g. \"Figure 4\" or \"Fig. 4.2\") — becomes FigureSource::figure. \
             Required: every dataset must say which figure it came from.",
            &mut self.figure,
        );
        field_tip(
            ui,
            "document title",
            "Title of the document the figure was taken from, for provenance \
             cross-reference — optional, but strongly recommended when the \
             figure isn't self-explanatory.",
            &mut self.document_title,
        );
        field_tip(
            ui,
            "document id",
            "This document's identifier in the kovan-literature archive \
             (e.g. its BibTeX key or KovanDocument id) — lets a reader trace \
             the digitised dataset back to its source document via `kovan lit`.",
            &mut self.document_id,
        );
        field_tip(
            ui,
            "page",
            "Page number the figure appears on in the source document.",
            &mut self.page,
        );
        field(ui, "x label", &mut self.x_label);
        field(ui, "y label", &mut self.y_label);
        field(ui, "notes", &mut self.notes);
        field_tip(
            ui,
            "operator*",
            "Who is running this digitisation — your name or handle. Recorded \
             on every hand-placed/corrected point and on the review record \
             (a KOVAN dataset can only be marked Reviewed by a human — see \
             this crate's dogfooding rule); an edit after review resets the \
             dataset back to Unreviewed. Required: every dataset must say who \
             digitised it.",
            &mut self.operator,
        );
        ui.separator();

        ui.label("5. Export:");
        // op-jtna: a "Browse…" button beside each export path, using the same
        // shared FileDialog (in save-file mode) rather than only a typed
        // path. Written inline rather than as a `field`-style closure because
        // it needs `&mut self` (to stash which target the dialog is for) at
        // the same time as `&mut self.json_out`/`&mut self.csv_out` — two
        // disjoint-field closure arguments the borrow checker can't verify
        // through a shared helper.
        let mut open_json_picker = false;
        ui.horizontal(|ui| {
            ui.label("json path");
            ui.text_edit_singleline(&mut self.json_out);
            open_json_picker = ui.button("Browse…").clicked();
        });
        if open_json_picker {
            self.open_picker(FileDialogTarget::JsonExport);
        }
        let mut open_csv_picker = false;
        ui.horizontal(|ui| {
            ui.label("csv path");
            ui.text_edit_singleline(&mut self.csv_out);
            open_csv_picker = ui.button("Browse…").clicked();
        });
        if open_csv_picker {
            self.open_picker(FileDialogTarget::CsvExport);
        }
        ui.horizontal(|ui| {
            if ui.button("Save (unreviewed)").clicked() {
                self.save(false);
            }
            if ui.button("Mark reviewed + save").clicked() {
                self.save(true);
            }
        });
        if let Some(d) = &self.dataset {
            let review = match &d.review {
                ReviewStatus::Unreviewed => "UNREVIEWED".to_string(),
                ReviewStatus::Reviewed { by, at, .. } => {
                    format!("reviewed by {by} at {at}")
                }
            };
            ui.label(format!("{} points · {review}", d.points.len()));
            if let Some(i) = self.selected {
                if let Some(p) = d.points.get(i) {
                    ui.label(format!(
                        "sel: x={:.6e} y={:.6e} (+{:.1e}/-{:.1e})",
                        p.x, p.y, p.y_plus, p.y_minus
                    ));
                }
            }
        }
        ui.separator();
        ui.label(&self.message);
    }

    fn image_panel(&mut self, ui: &mut egui::Ui) {
        let Some(raster) = &self.raster else {
            ui.centered_and_justified(|ui| {
                ui.label("no image loaded");
            });
            return;
        };
        // Upload texture on first frame after load — shares its
        // PlotRaster-to-ColorImage conversion with the PDF reader's own
        // image-viewing path (op-wojr) via `raster_to_color_image`.
        if self.texture.is_none() {
            let img = pdf_reader::raster_to_color_image(raster);
            self.texture = Some(ui.ctx().load_texture("plot", img, TextureOptions::NEAREST));
        }
        let texture = self.texture.as_ref().expect("just set").clone();
        let size = Vec2::new(
            raster.width() as f32 * self.zoom,
            raster.height() as f32 * self.zoom,
        );

        let zoom = self.zoom;
        egui::ScrollArea::both().show(ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
            let painter = ui.painter_at(rect);
            painter.image(
                texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );

            let to_image = move |pos: Pos2| -> (f64, f64) {
                (
                    ((pos.x - rect.min.x) / zoom) as f64,
                    ((pos.y - rect.min.y) / zoom) as f64,
                )
            };
            let to_screen = move |px: f64, py: f64| -> Pos2 {
                Pos2::new(rect.min.x + px as f32 * zoom, rect.min.y + py as f32 * zoom)
            };

            // --- interactions ---
            // op-zfnh: hit-test the four persistent reference lines — column
            // position for the two x lines (indices 0/1), row position for
            // the two y lines (indices 2/3) — within a fixed *screen*-space
            // tolerance (matches the point-drag tolerances below, which are
            // already written as `N / zoom` image-space to hold N screen px
            // at any zoom level).
            let ref_tol = 6.0 / zoom as f64;
            fn hit_ref_line(ref_px: &[Option<f64>; 4], tol: f64, px: f64, py: f64) -> Option<usize> {
                for (i, coord) in [px, px, py, py].into_iter().enumerate() {
                    if let Some(r) = ref_px[i] {
                        if (coord - r).abs() < tol {
                            return Some(i);
                        }
                    }
                }
                None
            }

            // op-8ixa: right-click removes the nearest marker under the
            // cursor regardless of mode (graphReader precedent), checked
            // before the mode-dispatched left-click handling below so a
            // stray left click from the same gesture can't also fire.
            if response.secondary_clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let (px, py) = to_image(pos);
                    if let Some(i) = self.nearest_point(px, py, 12.0 / zoom as f64) {
                        self.selected = Some(i);
                        self.delete_selected();
                    }
                }
            }
            // Adding a point is a double left-click (graphReader precedent) —
            // a single click in AddPoint mode is reserved for future
            // click-drag box-select, so it deliberately does not add here.
            if self.mode == ClickMode::AddPoint && response.double_clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let (px, py) = to_image(pos);
                    self.add_point(px, py);
                }
            }
            if self.mode == ClickMode::EditPoints {
                if let Some(pos) = response
                    .clicked()
                    .then(|| response.interact_pointer_pos())
                    .flatten()
                {
                    let (px, py) = to_image(pos);
                    self.selected = self.nearest_point(px, py, 10.0 / zoom as f64);
                }
            }

            // Reference-line dragging (op-zfnh) takes priority over marker
            // dragging when a drag starts on top of a line — it is checked
            // first and, if it claims the gesture, marker-drag start below is
            // skipped for that same drag via the `else`.
            if response.drag_started_by(PointerButton::Primary) {
                if let Some(pos) = response.interact_pointer_pos() {
                    let (px, py) = to_image(pos);
                    self.ref_dragging = hit_ref_line(&self.ref_px, ref_tol, px, py);
                    if self.ref_dragging.is_none() && self.mode == ClickMode::EditPoints {
                        self.dragging = self.nearest_point(px, py, 12.0 / zoom as f64);
                        self.selected = self.dragging;
                    }
                }
            }
            if let (Some(i), Some(pos)) = (self.ref_dragging, response.interact_pointer_pos()) {
                if response.dragged_by(PointerButton::Primary) {
                    let (px, py) = to_image(pos);
                    self.ref_px[i] = Some(if i < 2 { px } else { py });
                }
            }
            if self.mode == ClickMode::EditPoints && self.ref_dragging.is_none() {
                if let (Some(i), Some(pos)) = (self.dragging, response.interact_pointer_pos()) {
                    if response.dragged_by(PointerButton::Primary) {
                        let (px, py) = to_image(pos);
                        self.set_point_pixels(i, px, py, false);
                        self.mark_edited();
                    }
                }
            }
            if response.drag_stopped() {
                self.ref_dragging = None;
                self.dragging = None;
            }
            if ui.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
                self.delete_selected();
            }

            // --- overlays: reference lines, then points ---
            let ref_stroke = Stroke::new(1.0_f32, Color32::from_rgb(60, 120, 255));
            let ref_stroke_active = Stroke::new(2.5_f32, Color32::from_rgb(255, 210, 60));
            let stroke_for = |i: usize| {
                if self.ref_dragging == Some(i) {
                    ref_stroke_active
                } else {
                    ref_stroke
                }
            };
            if let Some(p) = self.ref_px[0] {
                painter.vline(to_screen(p, 0.0).x, rect.y_range(), stroke_for(0));
            }
            if let Some(p) = self.ref_px[1] {
                painter.vline(to_screen(p, 0.0).x, rect.y_range(), stroke_for(1));
            }
            if let Some(p) = self.ref_px[2] {
                painter.hline(rect.x_range(), to_screen(0.0, p).y, stroke_for(2));
            }
            if let Some(p) = self.ref_px[3] {
                painter.hline(rect.x_range(), to_screen(0.0, p).y, stroke_for(3));
            }
            if let Some(d) = &self.dataset {
                for (i, p) in d.points.iter().enumerate() {
                    let (Some(x), Some(y)) = (p.x_px, p.y_px) else {
                        continue;
                    };
                    let pos = to_screen(x, y);
                    let colour = match p.origin {
                        PointOrigin::AutoTraced => Color32::from_rgb(220, 40, 40),
                        PointOrigin::HandPlaced { .. } => Color32::from_rgb(40, 160, 40),
                        PointOrigin::HandCorrected { .. } => Color32::from_rgb(230, 140, 20),
                    };
                    if Some(i) == self.selected {
                        painter.circle_stroke(pos, 6.0, Stroke::new(2.0_f32, Color32::YELLOW));
                    }
                    painter.circle_filled(pos, 2.5, colour);
                }
            }
        });
    }
}

impl DigitiseApp {
    /// Top bar: switch between the Digitiser and PDF Reader panels
    /// (op-95x6), and the Gruvbox theme selector (op-t5sq).
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.view, View::Digitiser, "Digitiser");
            ui.selectable_value(&mut self.view, View::PdfReader, "PDF Reader");
            ui.separator();
            ComboBox::from_id_salt("gui-theme")
                .selected_text(self.theme.label())
                .show_ui(ui, |ui| {
                    for t in GuiTheme::ALL {
                        ui.selectable_value(&mut self.theme, t, t.label());
                    }
                });
        });
    }

    /// Open the shared [`FileDialog`] for `target`, selecting the filter (or
    /// save extension) that matches it first — op-nje6's fix for the picker
    /// always coming up filtered to "Images" regardless of what was actually
    /// being opened.
    fn open_picker(&mut self, target: FileDialogTarget) {
        self.file_dialog_target = Some(target);
        self.file_dialog.config_mut().default_file_filter = Some(target.default_filter().to_string());
        if target.is_save() {
            self.file_dialog.save_file();
        } else {
            self.file_dialog.pick_file();
        }
    }

    /// Route a just-picked file path to whichever action requested it.
    fn handle_picked_file(&mut self, path: &std::path::Path) {
        let Some(target) = self.file_dialog_target.take() else {
            return;
        };
        let path = path.to_string_lossy().into_owned();
        match target {
            FileDialogTarget::Image => self.load_image(&path),
            FileDialogTarget::Pdf => self.pdf_reader.open(&path),
            FileDialogTarget::JsonExport => self.json_out = path,
            FileDialogTarget::CsvExport => self.csv_out = path,
        }
    }
}

impl eframe::App for DigitiseApp {
    // eframe 0.34 hands the root `Ui`; panels nest with `show_inside`,
    // CentralPanel last (same pattern as the workspace's digital-twin GUIs).
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.theme.apply(ui.ctx());

        egui::Panel::top("topbar")
            .show_inside(ui, |ui| self.top_bar(ui));

        self.file_dialog.update(ui.ctx());
        if let Some(path) = self.file_dialog.take_picked() {
            self.handle_picked_file(&path);
        }

        match self.view {
            View::Digitiser => {
                egui::Panel::left("controls")
                    .min_size(290.0)
                    .show_inside(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| self.side_panel(ui));
                    });
                // op-5sdc: CSV preview + copy button, right-hand side,
                // htgr_sim_v1-style — see csv_preview.rs.
                egui::Panel::right("csv_preview")
                    .min_size(260.0)
                    .show_inside(ui, |ui| {
                        if let Some(d) = &self.dataset {
                            draw_csv_preview(ui, &d.to_csv_string());
                        } else {
                            ui.centered_and_justified(|ui| {
                                ui.label("no dataset yet — run the auto pass or Start empty");
                            });
                        }
                    });
                egui::CentralPanel::default().show_inside(ui, |ui| self.image_panel(ui));
            }
            View::PdfReader => {
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    let mut open_clicked = false;
                    self.pdf_reader.ui(ui, || open_clicked = true);
                    if open_clicked {
                        self.open_picker(FileDialogTarget::Pdf);
                    }
                });
            }
        }
    }
}
