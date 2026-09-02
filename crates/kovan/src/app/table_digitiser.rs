//! Table digitiser GUI tab (op-hnhp): the cropped-region hand-off from the
//! PDF reader's "Digitise table" tool lands here, gets OCR'd via
//! [`crate::digitiser::table_ocr`], and is reviewed/corrected/exported —
//! the same automatic-pass-then-mandatory-review shape the plot digitiser
//! (`super::DigitiseApp`) already uses, over a different engine (OCR
//! instead of curve tracing).
//!
//! This file owns no OCR or cell-splitting logic of its own — see
//! [`table_ocr`] for that and for the "no table-structure-detection, just a
//! whitespace-run heuristic" limitation this UI inherits.

use eframe::egui::{self, Color32};

use crate::digitiser::dataset::{utc_now_iso8601, ReviewInterface, ReviewStatus};
use crate::digitiser::raster::PlotRaster;
use crate::digitiser::table_ocr::{self, RecognizedTable};
use crate::project;
use crate::session::PaperSession;

use super::csv_preview::draw_csv_preview;
use super::pdf_reader::CropProvenance;

/// State for the table digitiser tab.
pub struct TableDigitiserState {
    crop: Option<PlotRaster>,
    /// Path to a `.traineddata` OCR model — supplied by the operator; this
    /// module does not download one (see `table_ocr`'s module doc).
    model_path: String,
    operator: String,
    table: Option<RecognizedTable>,
    json_out: String,
    csv_out: String,
    /// Provenance carried from the PDF reader's "Read table" crop
    /// (op-hnhp), if that's how the current region was loaded — see
    /// `super::DigitiseApp::crop_provenance` for the same idea on the plot
    /// digitiser side.
    crop_provenance: Option<CropProvenance>,
    project_root: String,
    project_markdown_rel: String,
    message: String,
    message_is_error: bool,
}

impl Default for TableDigitiserState {
    fn default() -> Self {
        Self {
            crop: None,
            model_path: String::new(),
            // op-n0kz: pre-fill from the OS login name where available,
            // same as the graph digitiser's own "your name" field.
            operator: super::default_operator_name(),
            table: None,
            json_out: String::new(),
            csv_out: String::new(),
            crop_provenance: None,
            project_root: String::new(),
            project_markdown_rel: String::new(),
            message: String::new(),
            message_is_error: false,
        }
    }
}

/// What kind of export path the table digitiser's "Browse…" buttons ask the
/// caller to open a native file picker for (`op-jfc3`) — `TableDigitiserState`
/// owns no [`egui_file_dialog::FileDialog`] of its own, the same "hand the
/// request up" shape [`super::pdf_reader::PdfReaderState`]'s `on_open_clicked`
/// callback already uses, since [`super::DigitiseApp`]'s single shared
/// dialog instance is what actually opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerRequest {
    Json,
    Csv,
}

impl TableDigitiserState {
    fn set_status(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.message_is_error = false;
    }

    fn set_error(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.message_is_error = true;
    }

    /// Set by [`super::DigitiseApp::handle_picked_file`] once the shared
    /// file dialog a [`PickerRequest`] triggered returns a path.
    pub(crate) fn set_json_out(&mut self, path: impl Into<String>) {
        self.json_out = path.into();
    }

    /// See [`Self::set_json_out`].
    pub(crate) fn set_csv_out(&mut self, path: impl Into<String>) {
        self.csv_out = path.into();
    }

    /// Receive a crop from the PDF reader's "Read table" menu action
    /// (op-hnhp/op-x9qn) — replaces whatever was previously being reviewed.
    pub fn load_crop(&mut self, raster: PlotRaster, provenance: Option<CropProvenance>) {
        self.crop = Some(raster);
        self.table = None;
        self.crop_provenance = provenance;
        self.set_status("region loaded — set the OCR model path, then Run OCR");
    }

    fn operator_name(&self) -> String {
        let t = self.operator.trim();
        if t.is_empty() {
            "unnamed operator".to_string()
        } else {
            t.to_string()
        }
    }

    fn run_ocr(&mut self) {
        let Some(raster) = &self.crop else {
            self.set_error("no region loaded — crop one from the PDF reader first");
            return;
        };
        if self.model_path.trim().is_empty() {
            self.set_error("set the .traineddata model path first");
            return;
        }
        let image = raster_to_ocr_rgb(raster);
        match table_ocr::recognize_table(
            std::path::Path::new(self.model_path.trim()),
            &image,
            format!("{} via kovan (gui)", self.operator_name()),
        ) {
            Ok(table) => {
                let n = table.rows.len();
                self.table = Some(table);
                self.set_status(format!(
                    "OCR found {n} line(s) — check every cell, then mark reviewed"
                ));
            }
            Err(e) => self.set_error(e.to_string()),
        }
    }

    /// Any cell edit invalidates a previously recorded review — the plot
    /// digitiser's `mark_edited` rule, reused for the same reason.
    fn mark_edited(&mut self) {
        if let Some(t) = &mut self.table {
            if matches!(t.review, ReviewStatus::Reviewed { .. }) {
                t.review = ReviewStatus::Unreviewed;
                self.set_status("edited after review — status reset to UNREVIEWED");
            }
        }
    }

    fn mark_reviewed(&mut self) {
        let by = self.operator_name();
        if let Some(t) = &mut self.table {
            t.record_review(by, utc_now_iso8601(), ReviewInterface::Gui);
            self.set_status("marked reviewed — save to export");
        }
    }

    fn save(&mut self) {
        let Some(t) = &self.table else {
            self.set_error("nothing to save — run OCR first");
            return;
        };
        if self.json_out.trim().is_empty() && self.csv_out.trim().is_empty() {
            self.set_error("set a JSON or CSV output path");
            return;
        }
        if !self.json_out.trim().is_empty() {
            if let Err(e) = t.write_json(std::path::Path::new(self.json_out.trim())) {
                self.set_error(e.to_string());
                return;
            }
        }
        if !self.csv_out.trim().is_empty() {
            if let Err(e) = t.write_csv(std::path::Path::new(self.csv_out.trim())) {
                self.set_error(e.to_string());
                return;
            }
        }
        self.set_status("saved");
    }

    /// Append this table's CSV into the active paper's own canonical
    /// Markdown when one is open (`op-bd8p`, same reasoning as
    /// `super::DigitiseApp::save_into_project` — see its doc), falling back
    /// to the manual `project_root`/`project_markdown_rel` fields +
    /// [`project::append_to_section`] (op-96am/op-x9qn's original design)
    /// only when no paper is active.
    fn save_into_project(&mut self, active_paper: Option<&mut PaperSession>) {
        let Some(t) = &self.table else {
            self.set_error("nothing to save — run OCR first");
            return;
        };
        let csv_body = format!("```csv\n{}```\n", t.to_csv_string());

        // GH issue #35 2026-09-02: save as a real `[kovan]` artifact so the
        // page-context panel can re-open it; a re-digitise replaces the
        // source block in place.
        if let Some(session) = active_paper {
            let prov = self.crop_provenance.clone();
            let heading = prov
                .as_ref()
                .filter(|p| !p.figure.is_empty())
                .map(|p| p.figure.clone())
                .unwrap_or_else(|| "Digitised table".to_string());
            let anchor = prov.as_ref().map(|p| crate::artifact::SourceAnchor {
                page: Some((p.page_index + 1) as u32),
                pages: None,
                region: p.region(),
            });
            let replace_id = prov.as_ref().and_then(|p| p.source_artifact_id.clone());
            let citekey = session.citekey().to_string();
            let result: Result<String, String> = crate::classify::save_digitised_csv(
                session,
                crate::artifact::ArtifactKind::DigitisedTable,
                &heading,
                anchor,
                Some("kopitiam-ocr".to_string()),
                replace_id.as_deref(),
                &csv_body,
            )
            .map_err(|e| e.to_string())
            .and_then(|_| {
                session.save_document().map(|()| format!("saved into {citekey}'s notes")).map_err(|e| e.to_string())
            });
            match result {
                Ok(m) => self.set_status(m),
                Err(e) => self.set_error(e),
            }
            return;
        }

        // --- no active paper: the legacy plain-text section path ---
        let mut block = "### Digitised table".to_string();
        if let Some(prov) = &self.crop_provenance {
            block.push_str(&format!(
                " — page {}, pixel bbox [{:.1}, {:.1}, {:.1}, {:.1}], {}, {}",
                prov.page_index + 1, prov.min.x, prov.min.y, prov.max.x, prov.max.y, prov.created_at, prov.author
            ));
        }
        block.push_str("\n\n");
        block.push_str(&csv_body);
        if self.project_root.trim().is_empty() || self.project_markdown_rel.trim().is_empty() {
            self.set_error("set the project root and markdown path first");
            return;
        }
        match project::append_to_section(
            std::path::Path::new(self.project_root.trim()),
            self.project_markdown_rel.trim(),
            "table_csvs",
            &block,
        ) {
            Ok(_) => self.set_status("saved into project markdown (table_csvs)"),
            Err(e) => self.set_error(e.to_string()),
        }
    }

    /// Returns `Some(request)` the frame a "Browse…" button is clicked
    /// (`op-jfc3`) — the caller opens its shared file dialog for that
    /// request and later routes the picked path back in via
    /// [`Self::set_json_out`]/[`Self::set_csv_out`].
    pub fn ui(&mut self, ui: &mut egui::Ui, active_paper: Option<&mut PaperSession>) -> Option<PickerRequest> {
        ui.heading("kovan — table digitiser (OCR)");
        ui.small(
            "OCR recognises text lines, not table structure — cells are split on runs of \
             2+ spaces (see table_ocr's module doc). Check every row before exporting.",
        );
        if self.message_is_error {
            egui::Frame::new()
                .fill(Color32::from_rgb(120, 30, 30))
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.colored_label(Color32::WHITE, format!("⚠ {}", self.message));
                });
        }
        ui.separator();

        if self.crop.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    "no region loaded — go to PDF Reader, pick \"Digitise table\", \
                     draw a box, right-click it",
                );
            });
            return None;
        }

        ui.horizontal(|ui| {
            ui.label("OCR model (.traineddata):");
            ui.text_edit_singleline(&mut self.model_path);
        });
        ui.horizontal(|ui| {
            ui.label("your name*:").on_hover_text(
                "Recorded as who ran this OCR pass and, later, who reviewed \
                 it. Required. Pre-filled from your OS login name where \
                 available — feel free to change it.",
            );
            ui.text_edit_singleline(&mut self.operator);
        });
        if ui.button("Run OCR").clicked() {
            self.run_ocr();
        }
        ui.separator();

        if self.table.is_none() {
            ui.label(&self.message);
            return None;
        }

        // Each block below re-borrows `self.table` rather than holding one
        // borrow across the whole function — several branches call back into
        // `self.mark_edited()`/`mark_reviewed()`/`save()`, which need their
        // own `&mut self`, so the table borrow can't live that long.
        let mut edited = false;
        egui::ScrollArea::vertical()
            .id_salt("table_ocr_grid")
            .max_height(300.0)
            .show(ui, |ui| {
                egui::Grid::new("table_ocr_cells").striped(true).show(ui, |ui| {
                    for row in self.table.as_mut().unwrap().rows.iter_mut() {
                        for cell in row.iter_mut() {
                            if ui
                                .add(egui::TextEdit::singleline(cell).desired_width(120.0))
                                .changed()
                            {
                                edited = true;
                            }
                        }
                        ui.end_row();
                    }
                });
            });
        if edited {
            self.mark_edited();
        }

        let table = self.table.as_ref().unwrap();
        let review = match &table.review {
            ReviewStatus::Unreviewed => "UNREVIEWED".to_string(),
            ReviewStatus::Reviewed { by, at, .. } => format!("reviewed by {by} at {at}"),
        };
        let row_count = table.rows.len();
        ui.horizontal(|ui| {
            ui.label(format!("{row_count} row(s) · {review}"));
            if ui.button("Mark reviewed").clicked() {
                self.mark_reviewed();
            }
        });

        ui.separator();
        // op-jfc3: "Browse…" opens the app's shared native file picker,
        // same as the graph digitiser already had (op-jtna) — this tab had
        // only a typed path before.
        let mut request = None;
        ui.horizontal(|ui| {
            ui.label("json path");
            ui.text_edit_singleline(&mut self.json_out);
            if ui.button("Browse…").clicked() {
                request = Some(PickerRequest::Json);
            }
        });
        ui.horizontal(|ui| {
            ui.label("csv path");
            ui.text_edit_singleline(&mut self.csv_out);
            if ui.button("Browse…").clicked() {
                request = Some(PickerRequest::Csv);
            }
        });
        if ui.button("Save").clicked() {
            self.save();
        }
        ui.separator();
        ui.label("Save into project markdown:");
        // op-bd8p: mirrors the graph digitiser's own fix -- an active
        // paper already tells us where this belongs.
        match active_paper.as_ref().map(|s| s.citekey().to_string()) {
            Some(citekey) => {
                ui.label(format!("saving into {citekey}'s notes"));
            }
            None => {
                ui.horizontal(|ui| {
                    ui.label("project root");
                    ui.text_edit_singleline(&mut self.project_root);
                });
                ui.horizontal(|ui| {
                    ui.label("markdown path (relative)");
                    ui.text_edit_singleline(&mut self.project_markdown_rel);
                });
            }
        }
        if let Some(prov) = &self.crop_provenance {
            ui.label(format!(
                "from PDF reader: page {}, bbox [{:.0}, {:.0}, {:.0}, {:.0}], {}",
                prov.page_index + 1,
                prov.min.x,
                prov.min.y,
                prov.max.x,
                prov.max.y,
                prov.author
            ));
        } else {
            ui.small("(no PDF-reader crop provenance for this region)");
        }
        if ui.button("Save CSV into project markdown").clicked() {
            self.save_into_project(active_paper);
        }
        ui.separator();
        let csv_string = self.table.as_ref().unwrap().to_csv_string();
        draw_csv_preview(ui, &csv_string);
        ui.label(&self.message);
        request
    }
}

/// Convert a [`PlotRaster`] into the `kopitiam_ocr::RgbImage` OCR wants —
/// the same per-pixel `rgb()` read the digitiser's own texture-upload path
/// (`mod.rs::image_panel`) and the PDF reader's `raster_to_color_image`
/// use, just targeting a different crate's image type.
fn raster_to_ocr_rgb(raster: &PlotRaster) -> kopitiam_ocr::RgbImage {
    let (w, h) = (raster.width() as usize, raster.height() as usize);
    let mut pixels = Vec::with_capacity(w * h * 3);
    for y in 0..raster.height() {
        for x in 0..raster.width() {
            pixels.extend_from_slice(&raster.rgb(x, y));
        }
    }
    kopitiam_ocr::RgbImage::new(w, h, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digitiser::dataset::ReviewStatus;
    use crate::digitiser::table_ocr::RecognizedTable;
    use crate::entity::{Access, CiteKey, EntityConfig};
    use crate::root::RootConfig;

    fn make_root() -> (tempfile::TempDir, crate::root::KovanRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = crate::root::KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        (dir, root)
    }

    fn table() -> RecognizedTable {
        RecognizedTable {
            schema_version: 1,
            source_image_sha256: None,
            source_note: None,
            engine: "test".to_string(),
            recognized_by: "unit test".to_string(),
            recognized_at: "2026-09-02T00:00:00Z".to_string(),
            review: ReviewStatus::Unreviewed,
            rows: vec![vec!["a".to_string(), "b".to_string()]],
        }
    }

    /// `op-bd8p`: mirrors the graph digitiser's own fix — an active paper
    /// already tells the table digitiser where to save; no manual project
    /// root/markdown path needed.
    #[test]
    fn save_into_project_writes_into_the_active_papers_session_when_given_one() {
        let (_dir, root) = make_root();
        EntityConfig::paper(CiteKey::parse("wang2018multiphysics").unwrap(), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("wang2018multiphysics"))
            .unwrap();
        let mut session = PaperSession::open(&root, "wang2018multiphysics").unwrap();

        let mut state = TableDigitiserState { table: Some(table()), ..Default::default() };
        state.save_into_project(Some(&mut session));

        assert!(!state.message_is_error, "{}", state.message);
        let reopened = PaperSession::open(&root, "wang2018multiphysics").unwrap();
        let md = reopened.markdown();
        assert!(md.contains("Digitised table"), "{md}");
        // GH issue #35 2026-09-02: it is a real fenced-TOML artifact now.
        assert!(md.contains("kind = \"digitised_table\""), "{md}");
        assert!(md.contains("method = \"manual_digitisation\""), "{md}");
        let idx = crate::research_record::ResearchRecordIndex::from_session(&reopened);
        assert_eq!(idx.artifacts().len(), 1);
        assert!(idx.artifacts()[0].csv_block().is_some());
    }

    #[test]
    fn save_into_project_falls_back_to_manual_project_fields_with_no_active_paper() {
        let mut state = TableDigitiserState { table: Some(table()), ..Default::default() };
        state.save_into_project(None);
        assert!(state.message_is_error, "{}", state.message);
        assert!(state.message.contains("project root"), "{}", state.message);
    }
}
