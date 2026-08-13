//! `kovan-digitise-gui` — hybrid graph digitiser (egui, graphreader-style).
//!
//! **Automatic pass first, then human verification — recorded, not
//! assumed.** The interaction model follows graphreader.com: load a plot
//! image; click two reference points per axis and type their values; choose
//! linear or log per axis; auto-trace the curve; then drag / add / delete
//! individual points with the mouse; finally mark the dataset reviewed and
//! export. Every hand edit is recorded per point (`HandPlaced` /
//! `HandCorrected` with the operator name), any edit after a review resets
//! the status to `UNREVIEWED`, and the export always carries the full
//! calibration + provenance record.
//!
//! Desktop-only by policy: this is the workspace's one GUI exemption shape —
//! the binary is behind the non-default `digitise-gui` feature, its
//! egui/eframe dependencies are target-gated off Android, and on Android this
//! file compiles to a stub `main` that redirects to the CLI/TUI.

/// Android stub: the GUI stack is not built for Android; the terminal tools
/// are the supported path there.
#[cfg(target_os = "android")]
fn main() {
    eprintln!(
        "kovan-digitise-gui is desktop-only; on Android/Termux use \
         kovan-digitise (automatic) or kovan-digitise-tui (review)."
    );
}

#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result {
    let image_arg = std::env::args().nth(1);
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "kovan-digitise-gui",
        options,
        Box::new(move |_cc| {
            let mut app = desktop::DigitiseApp::default();
            if let Some(path) = image_arg {
                app.load_image(&path);
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use eframe::egui::{
        self, Color32, ComboBox, Key, PointerButton, Pos2, Rect, Sense, Stroke, TextureHandle,
        TextureOptions, Vec2,
    };
    use kovan_literature::digitiser::auto::{
        auto_digitise, AutoDigitiseConfig, AxisPixelRefs, AxisValueSpec,
    };
    use kovan_literature::digitiser::calibration::{
        AxisCalibration, AxisRef, AxisScale, PlotCalibration,
    };
    use kovan_literature::digitiser::dataset::{
        uncertainty_interval, utc_now_iso8601, DigitisedDataset, DigitisedPoint, FigureSource,
        PointOrigin, ReviewInterface, ReviewStatus, DATASET_SCHEMA_VERSION,
    };
    use kovan_literature::digitiser::detect::DetectConfig;
    use kovan_literature::digitiser::raster::PlotRaster;
    use kovan_literature::digitiser::trace::{CurveSelector, TraceConfig, TraceStrategy};

    /// What a click on the image currently means. Closed set, enum-dispatched.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ClickMode {
        /// Record the x-axis reference 1 pixel column.
        SetXRef1,
        /// Record the x-axis reference 2 pixel column.
        SetXRef2,
        /// Record the y-axis reference 1 pixel row.
        SetYRef1,
        /// Record the y-axis reference 2 pixel row.
        SetYRef2,
        /// Select / drag existing points.
        EditPoints,
        /// Each click adds a hand-placed point.
        AddPoint,
    }

    /// All GUI state, owned by value (no lifetimes, no shared state).
    pub struct DigitiseApp {
        // image
        image_path: String,
        raster: Option<PlotRaster>,
        texture: Option<TextureHandle>,
        zoom: f32,
        // calibration input
        mode: ClickMode,
        ref_px: [Option<f64>; 4], // x1, x2 (columns); y1, y2 (rows)
        ref_val: [String; 4],
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
                image_path: String::new(),
                raster: None,
                texture: None,
                zoom: 1.0,
                mode: ClickMode::SetXRef1,
                ref_px: [None; 4],
                ref_val: Default::default(),
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
        pub fn load_image(&mut self, path: &str) {
            match PlotRaster::from_path(std::path::Path::new(path)) {
                Ok(r) => {
                    self.image_path = path.to_string();
                    if self.json_out.is_empty() {
                        self.json_out = format!("{path}.digitised.json");
                    }
                    self.raster = Some(r);
                    self.texture = None; // re-uploaded next frame
                    self.dataset = None;
                    self.selected = None;
                    self.message = format!("loaded {path}");
                }
                Err(e) => self.message = e.to_string(),
            }
        }

        /// Build the calibration the four reference points + values describe.
        fn calibration(&self) -> Result<PlotCalibration, String> {
            let px = |i: usize, what: &str| {
                self.ref_px[i].ok_or_else(|| format!("{what} pixel not set — click it"))
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
                format!("{} via kovan-digitise-gui", self.operator_name()),
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
                digitised_by: format!(
                    "{} via kovan-digitise-gui (hand-placed)",
                    self.operator_name()
                ),
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
            ui.heading("kovan-digitise");
            ui.horizontal(|ui| {
                ui.label("image:");
                ui.text_edit_singleline(&mut self.image_path);
            });
            if ui.button("Load image").clicked() {
                let p = self.image_path.clone();
                self.load_image(&p);
            }
            ui.add(egui::Slider::new(&mut self.zoom, 0.25..=4.0).text("zoom"));
            ui.separator();

            ui.label("1. Axis references — click the image in each mode:");
            let labels = ["X1 (column)", "X2 (column)", "Y1 (row)", "Y2 (row)"];
            let modes = [
                ClickMode::SetXRef1,
                ClickMode::SetXRef2,
                ClickMode::SetYRef1,
                ClickMode::SetYRef2,
            ];
            for i in 0..4 {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.mode, modes[i], labels[i]);
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
            });
            ui.separator();

            ui.label("4. Provenance (required to export):");
            let field = |ui: &mut egui::Ui, name: &str, s: &mut String| {
                ui.horizontal(|ui| {
                    ui.label(name);
                    ui.text_edit_singleline(s);
                });
            };
            field(ui, "figure*", &mut self.figure);
            field(ui, "document title", &mut self.document_title);
            field(ui, "document id", &mut self.document_id);
            field(ui, "page", &mut self.page);
            field(ui, "x label", &mut self.x_label);
            field(ui, "y label", &mut self.y_label);
            field(ui, "notes", &mut self.notes);
            field(ui, "operator*", &mut self.operator);
            ui.separator();

            ui.label("5. Export:");
            field(ui, "json path", &mut self.json_out);
            field(ui, "csv path", &mut self.csv_out);
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
            // Upload texture on first frame after load.
            if self.texture.is_none() {
                let (w, h) = (raster.width() as usize, raster.height() as usize);
                let mut rgb = Vec::with_capacity(w * h * 3);
                for y in 0..raster.height() {
                    for x in 0..raster.width() {
                        rgb.extend_from_slice(&raster.rgb(x, y));
                    }
                }
                let img = egui::ColorImage::from_rgb([w, h], &rgb);
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
                let click_pos = response
                    .clicked()
                    .then(|| response.interact_pointer_pos())
                    .flatten();
                if let Some(pos) = click_pos {
                    let (px, py) = to_image(pos);
                    match self.mode {
                        ClickMode::SetXRef1 => {
                            self.ref_px[0] = Some(px);
                            self.mode = ClickMode::SetXRef2;
                        }
                        ClickMode::SetXRef2 => {
                            self.ref_px[1] = Some(px);
                            self.mode = ClickMode::SetYRef1;
                        }
                        ClickMode::SetYRef1 => {
                            self.ref_px[2] = Some(py);
                            self.mode = ClickMode::SetYRef2;
                        }
                        ClickMode::SetYRef2 => {
                            self.ref_px[3] = Some(py);
                            self.mode = ClickMode::EditPoints;
                            self.message =
                                "references set — fill their values, then Auto-trace".to_string();
                        }
                        ClickMode::EditPoints => {
                            self.selected = self.nearest_point(px, py, 10.0 / zoom as f64);
                        }
                        ClickMode::AddPoint => self.add_point(px, py),
                    }
                }
                if self.mode == ClickMode::EditPoints {
                    if response.drag_started_by(PointerButton::Primary) {
                        if let Some(pos) = response.interact_pointer_pos() {
                            let (px, py) = to_image(pos);
                            self.dragging = self.nearest_point(px, py, 12.0 / zoom as f64);
                            self.selected = self.dragging;
                        }
                    }
                    if let (Some(i), Some(pos)) = (self.dragging, response.interact_pointer_pos()) {
                        if response.dragged_by(PointerButton::Primary) {
                            let (px, py) = to_image(pos);
                            self.set_point_pixels(i, px, py, false);
                            self.mark_edited();
                        }
                    }
                    if response.drag_stopped() {
                        self.dragging = None;
                    }
                }
                if ui.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
                    self.delete_selected();
                }

                // --- overlays: reference lines, then points ---
                let ref_stroke = Stroke::new(1.0_f32, Color32::from_rgb(60, 120, 255));
                if let Some(p) = self.ref_px[0] {
                    painter.vline(to_screen(p, 0.0).x, rect.y_range(), ref_stroke);
                }
                if let Some(p) = self.ref_px[1] {
                    painter.vline(to_screen(p, 0.0).x, rect.y_range(), ref_stroke);
                }
                if let Some(p) = self.ref_px[2] {
                    painter.hline(rect.x_range(), to_screen(0.0, p).y, ref_stroke);
                }
                if let Some(p) = self.ref_px[3] {
                    painter.hline(rect.x_range(), to_screen(0.0, p).y, ref_stroke);
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

    impl eframe::App for DigitiseApp {
        // eframe 0.34 hands the root `Ui`; panels nest with `show_inside`,
        // CentralPanel last (same pattern as the workspace's digital-twin GUIs).
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            egui::Panel::left("controls")
                .min_size(290.0)
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.side_panel(ui));
                });
            egui::CentralPanel::default().show_inside(ui, |ui| self.image_panel(ui));
        }
    }
}
