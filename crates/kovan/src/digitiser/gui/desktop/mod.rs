mod advanced_git_view;
mod bibliography;
mod csv_preview;
mod home;
mod kvim_editor;
mod pdf_reader;
mod table_digitiser;
mod theme;
mod wiki;

use eframe::egui::{
    self, Color32, ComboBox, Key, PointerButton, Pos2, Rect, Sense, Stroke, TextureHandle,
    TextureOptions, Vec2,
};
use egui_file_dialog::FileDialog;

use crate::digitiser::auto::{auto_digitise, AutoDigitiseConfig, AxisPixelRefs, AxisValueSpec};
use crate::entity::EntityConfig;
use crate::session::PaperSession;
use crate::digitiser::calibration::{
    AxisCalibration, AxisRef, AxisScale, ParallelogramCalibration, PixelPoint, PlotCalibration,
};
use crate::digitiser::dataset::{
    utc_now_iso8601, xy_uncertainty_interval, DigitisedDataset, DigitisedPoint, FigureSource,
    PointOrigin, ReviewInterface, ReviewStatus, DATASET_SCHEMA_VERSION,
};
use crate::digitiser::detect::DetectConfig;
use crate::digitiser::raster::PlotRaster;
use crate::digitiser::trace::{CurveSelector, TraceConfig, TraceStrategy};
use crate::project;

use advanced_git_view::AdvancedGitState;
use bibliography::{BibliographyAction, BibliographyState};
use csv_preview::draw_csv_preview;
use home::{HomeAction, HomeState};
use kvim_editor::KvimEditorState;
use pdf_reader::{CropProvenance, PdfReaderState};
use table_digitiser::TableDigitiserState;
use theme::GuiTheme;
use wiki::{WikiAction, WikiState};

use crate::mindmap::{MindmapAction, MindmapState};

/// Which top-level panel is showing — the plot digitiser (the window's
/// original purpose), the integrated PDF reader (op-95x6), or the
/// structured markdown editor (op-wr08). A closed set, switched with a
/// top-bar button row rather than a popup/new-window (the window itself
/// already is the "new tab" GitHub issue #30 asked for the plot-digitiser
/// popup to attach to — see `op-p17q`, wired the same way).
///
/// `Home` and `Wiki` are the Kovan redesign's startup/landing screens
/// (GitHub issue #35 §2, §8, `op-9vo6.3`/`.8`) — `Home` is now the
/// `#[default]`, replacing the previous default of launching straight into
/// `PdfReader`. `DigitiseApp::ui` auto-transitions `Home` -> `Wiki` the
/// frame a root is opened/created (§8: "after opening a root, land in the
/// Wiki, not the PDF reader"); the other variants stay reachable from the
/// top bar exactly as before this redesign — removing them is `op-9vo6.25`
/// (Research workspace)'s job, not this pass's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum View {
    #[default]
    Home,
    Wiki,
    Digitiser,
    PdfReader,
    /// The `kopitiam-neovim`-backed editor (§26/§27, `op-9vo6.17`) — the
    /// one user-facing paper-markdown editor (op-shjn, GH issue #35
    /// 2026-09-01: "the markdown editor should use the kvim editor"). The
    /// older hand-rolled `markdown_editor.rs` (the pre-redesign
    /// `crate::project` model) was retired along with `Bibliography`'s
    /// migration onto `KovanRoot` (op-9r26) — nothing constructs that view
    /// any more.
    KvimEditor,
    Bibliography,
    TableDigitiser,
    /// The interactive mindmap (§8, §9, `op-9vo6.21`), built on top of the
    /// `Wiki` view's collection model.
    Mindmap,
    /// The Advanced Git tab (§38, `op-9vo6.20`) — a separate area, per that
    /// section's own wording, from ordinary Save Document/Save Repository.
    AdvancedGit,
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
    /// Picked directory is opened (or discovered from) as a Kovan root
    /// (op-9vo6.3, §2's "Open Kovan Folder…").
    KovanRootOpen,
    /// Picked directory becomes a new Kovan root (op-9vo6.3, §2's
    /// "+ Create Kovan Folder…").
    KovanRootCreate,
    /// Picked file is previewed for ingestion into the open root
    /// (op-9vo6.9, §22's "+ Ingest Literature").
    PdfIngest,
    /// Picked file is loaded into the `kopitiam-neovim`-backed editor
    /// (op-9vo6.17). No extension filter — any text file is fair game for
    /// a general-purpose text editor.
    KvimFile,
}

impl FileDialogTarget {
    /// Whether this target picks a directory (`pick_directory`) rather than
    /// a file — see [`DigitiseApp::open_picker`].
    fn is_directory(self) -> bool {
        matches!(self, Self::KovanRootOpen | Self::KovanRootCreate)
    }

    /// Whether this target opens an existing file (`pick_file`) or names a
    /// new one to write (`save_file`) — see [`DigitiseApp::open_picker`].
    /// Meaningless for a directory target; checked after `is_directory`.
    fn is_save(self) -> bool {
        matches!(self, Self::JsonExport | Self::CsvExport)
    }

    /// The file-filter name (matching one of the names registered on
    /// [`FileDialog`] in [`DigitiseApp::default`]) this target should default
    /// to, so "Open PDF…" doesn't come up filtered to "Images" and
    /// vice versa (op-nje6). `None` for a directory target, which has no
    /// file-extension filter to select.
    fn default_filter(self) -> Option<&'static str> {
        match self {
            Self::Image => Some("Images"),
            Self::Pdf | Self::PdfIngest => Some("PDF"),
            Self::JsonExport => Some("JSON"),
            Self::CsvExport => Some("CSV"),
            Self::KovanRootOpen | Self::KovanRootCreate | Self::KvimFile => None,
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
/// model (columns for x, rows for y) as the default. A parallelogram/skewed
/// variant for off-centre plots (op-vyb9) now also exists, selectable via
/// [`CalibrationShape`] — the two shapes are enum-dispatched siblings, not
/// one replacing the other, so nothing about the axis-aligned path above
/// changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClickMode {
    /// Select / drag existing points.
    EditPoints,
    /// Double-click adds a hand-placed point (op-8ixa).
    AddPoint,
}

/// Which shape the calibration reference box is (op-vyb9): the original
/// axis-aligned box (4 independently draggable lines), or a freely-skewed
/// parallelogram (4 independently draggable corners, no rectilinear
/// constraint) for a plot photographed or scanned at an angle. Selectable in
/// the side panel; switching shape does not lose the other shape's own
/// reference positions (`ref_px` and `para_corners` are both seeded on
/// image load and kept independently — see [`DigitiseApp::set_raster`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum CalibrationShape {
    #[default]
    AxisAligned,
    Parallelogram,
}

/// The paper the operator is currently working on, shared across every
/// paper-aware view — GitHub issue #35's 2026-09-01 "unify Kovan root and
/// active-paper context" comment (op-sr4n). Constructed only by
/// [`DigitiseApp::activate_paper`]; nothing else builds one, so there is
/// exactly one place a paper becomes "the" active paper.
struct ActivePaper {
    session: PaperSession,
    /// The paper's source PDF, resolved from its `kovan.toml`
    /// (`[source].pdf`, relative to the paper's own directory) to an
    /// absolute path and confirmed to exist on disk. `None` when the paper
    /// has no recorded source, or the recorded path is missing locally
    /// (the GH comment's own "Missing PDF" acceptance scenario: activation
    /// still succeeds, the PDF state just reports unavailable).
    pdf_path: Option<std::path::PathBuf>,
}

/// All GUI state, owned by value (no lifetimes, no shared state).
pub struct DigitiseApp {
    // chrome
    view: View,
    theme: GuiTheme,
    file_dialog: FileDialog,
    file_dialog_target: Option<FileDialogTarget>,
    // Kovan root startup/landing screens (op-9vo6.3/.8, GitHub issue #35 §2/§8)
    home: HomeState,
    wiki: Option<WikiState>,
    kvim_editor: KvimEditorState,
    mindmap: MindmapState,
    advanced_git: AdvancedGitState,
    /// The paper currently in focus, if any — GitHub issue #35's
    /// "unify root and active-paper context" comment (op-sr4n). Set only by
    /// [`Self::activate_paper`]; consumed by every paper-aware view instead
    /// of each independently prompting for a root/path.
    active_paper: Option<ActivePaper>,
    /// A just-opened PDF that is not yet part of the open Kovan library,
    /// awaiting an Ingest/Skip decision (op-9sc7, GH issue #35 2026-09-01
    /// 05:22: "opening new pdfs, kovan should ask if you want to ingest
    /// it"). Absolute path, as a `String` to match [`FileDialogTarget`]'s
    /// own picked-path convention.
    pending_ingest_prompt: Option<String>,
    /// "Always ingest automatically" (op-9sc7's own "checkbox to
    /// auto-ingest opened pdfs") — when on, a newly opened PDF outside the
    /// library is ingested immediately with no prompt, using its own
    /// preview-suggested citekey/topics.
    auto_ingest_opened_pdfs: bool,
    // PDF reader (op-95x6)
    pdf_reader: PdfReaderState,
    // bibliography (op-9vml, migrated onto KovanRoot by op-9r26)
    bibliography: BibliographyState,
    // table digitiser (op-hnhp)
    table_digitiser: TableDigitiserState,
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
    /// Which corner of the reference box (as `(x_index, y_index)` into
    /// `ref_px`/`ref_val`) is currently being dragged, if any — corner-drag
    /// support for GitHub issue #30's "I also want to be able to drag
    /// corners of the box for the digitiser ui to adjust the min and max x
    /// and y" (op-zfnh's original persistent box only let each of the four
    /// *lines* be dragged independently; this drags an x-line and a y-line
    /// together in one gesture). `None` when the pointer isn't holding a
    /// corner. Takes priority over `ref_dragging` when a drag starts on a
    /// corner (see `image_panel`'s hit test).
    ref_dragging_corner: Option<(usize, usize)>,
    /// Which calibration shape is active (op-vyb9) — see [`CalibrationShape`].
    calibration_shape: CalibrationShape,
    /// Parallelogram corner pixel positions, order `[top_left, top_right,
    /// bottom_right, bottom_left]` — matches
    /// `ParallelogramCalibration::pixel_corners`'s convention. Independent
    /// of `ref_px`; seeded alongside it in `set_raster` so switching
    /// `calibration_shape` mid-session always has something sensible to
    /// show.
    para_corners: [Option<(f64, f64)>; 4],
    /// Which parallelogram corner (index into `para_corners`) is currently
    /// being dragged, if any.
    para_dragging: Option<usize>,
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
    /// Provenance carried from the PDF reader's "Digitise graph" crop
    /// (op-p17q), if that's how the current image was loaded — used by
    /// [`Self::save_into_project`] (op-96am) to record page/pixel/date/
    /// author on the CSV block it appends into a project's `graph_csvs`
    /// section. `None` when the image was loaded directly (no PDF-reader
    /// crop in this session).
    crop_provenance: Option<CropProvenance>,
    /// Root of the "kovan folder" project (op-63u0) to save into, and the
    /// markdown file (relative to that root) the CSV belongs to — both
    /// operator-supplied, same as `json_out`/`csv_out`, since a crop has no
    /// way to know which project/document it came from on its own.
    project_root: String,
    project_markdown_rel: String,
    message: String,
    /// `true` when `message` reports a failure — op-fueb: a calibration
    /// failure used to be indistinguishable from an ordinary status update
    /// (both were the same plain label buried at the bottom of a long
    /// scrollable panel), so "Auto-trace" clicked with an incomplete
    /// calibration appeared to silently do nothing. Drives both the colour
    /// and the top-of-panel duplicate of the message — see `side_panel`.
    message_is_error: bool,
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
            home: HomeState::default(),
            wiki: None,
            kvim_editor: KvimEditorState::default(),
            mindmap: MindmapState::default(),
            advanced_git: AdvancedGitState::default(),
            active_paper: None,
            pending_ingest_prompt: None,
            auto_ingest_opened_pdfs: false,
            pdf_reader: PdfReaderState::new(),
            bibliography: BibliographyState::default(),
            table_digitiser: TableDigitiserState::default(),
            image_path: String::new(),
            raster: None,
            texture: None,
            zoom: 1.0,
            mode: ClickMode::EditPoints,
            ref_px: [None; 4],
            ref_val: Default::default(),
            ref_dragging: None,
            ref_dragging_corner: None,
            calibration_shape: CalibrationShape::default(),
            para_corners: [None; 4],
            para_dragging: None,
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
            crop_provenance: None,
            project_root: String::new(),
            project_markdown_rel: String::new(),
            message: "load an image, then click the four axis reference points".to_string(),
            message_is_error: false,
        }
    }
}

impl DigitiseApp {
    /// Set an ordinary status message (op-fueb: explicitly the "not an
    /// error" half of `message`/`message_is_error`, so a later action can't
    /// leave a stale error's red styling on screen after it succeeds).
    fn set_status(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.message_is_error = false;
    }

    /// Make `citekey` the active paper (op-sr4n, GitHub issue #35's
    /// 2026-09-01 "unify Kovan root and active-paper context" comment):
    /// opens its [`PaperSession`], resolves its source PDF from
    /// `kovan.toml`, and pushes both into the PDF reader and the kvim
    /// editor.
    ///
    /// Requires a Kovan root to already be open. A missing/unreadable
    /// source PDF is not an error — the GH comment's own "Missing PDF"
    /// acceptance scenario says activation must still succeed with the PDF
    /// state simply reporting unavailable — so [`ActivePaper::pdf_path`]
    /// is `None` in that case rather than this method failing.
    fn activate_paper(&mut self, citekey: &str) -> Result<(), String> {
        let root = self.home.root().cloned().ok_or_else(|| "no Kovan folder open".to_string())?;
        let session = PaperSession::open(&root, citekey).map_err(|e| e.to_string())?;

        let pdf_path = EntityConfig::load(&root.paper_dir(citekey))
            .ok()
            .and_then(|cfg| cfg.source)
            .and_then(|source| source.pdf)
            .map(|rel| root.paper_dir(citekey).join(rel))
            .filter(|path| path.is_file());

        if let Some(pdf) = &pdf_path {
            self.pdf_reader.open(&pdf.to_string_lossy());
        }
        self.kvim_editor.load_text(session.markdown());

        self.active_paper = Some(ActivePaper { session, pdf_path });
        Ok(())
    }

    /// Whether `path` is already one of the open library's stored source
    /// PDFs — i.e. lives directly under `root.open_sources_dir()` or
    /// `root.restricted_sources_dir()`, exactly where `ingest::ingest`
    /// (§23 step 3) copies a paper's PDF to. Cheap prefix check rather than
    /// scanning every paper's `kovan.toml`, and correct as long as nothing
    /// else writes into those two directories — which nothing in this
    /// crate does.
    fn already_ingested(&self, path: &std::path::Path) -> bool {
        let Some(root) = self.home.root() else { return false };
        let Ok(canon) = path.canonicalize() else { return false };
        [root.open_sources_dir(), root.restricted_sources_dir()]
            .into_iter()
            .any(|dir| canon.starts_with(dir.canonicalize().unwrap_or(dir)))
    }

    /// A PDF was just opened (op-9sc7, GH issue #35 2026-09-01 05:22:
    /// "opening new pdfs, kovan should ask if you want to ingest it") — if
    /// it is not already part of the open library, either ingest it
    /// immediately ([`Self::auto_ingest_opened_pdfs`] on) or queue the
    /// Ingest/Skip prompt [`Self::ingest_prompt_ui`] draws. No-op with no
    /// root open — nothing to ingest into.
    fn offer_ingest_if_new(&mut self, path: &str) {
        let Some(root) = self.home.root().cloned() else { return };
        if self.already_ingested(std::path::Path::new(path)) {
            return;
        }
        if self.auto_ingest_opened_pdfs {
            let wiki = self.wiki.get_or_insert_with(|| WikiState::new(&root));
            if let Err(e) = wiki.begin_ingest(&root, std::path::Path::new(path)) {
                self.set_error(e);
            }
        } else {
            self.pending_ingest_prompt = Some(path.to_string());
        }
    }

    /// Draw the Ingest/Skip prompt from [`Self::offer_ingest_if_new`], if
    /// one is pending. Ingesting hands off to the same "+ Ingest
    /// Literature…" flow the Wiki's own button opens (§22's preview/
    /// classify form), rather than a second, competing ingestion path.
    fn ingest_prompt_ui(&mut self, ctx: &egui::Context) {
        let Some(path) = self.pending_ingest_prompt.clone() else { return };
        let Some(root) = self.home.root().cloned() else {
            self.pending_ingest_prompt = None;
            return;
        };
        let mut close = false;
        egui::Window::new("Ingest this PDF?").collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label(format!("{path} is not in your Kovan library yet."));
            ui.checkbox(&mut self.auto_ingest_opened_pdfs, "Always ingest opened PDFs automatically");
            ui.horizontal(|ui| {
                if ui.button("Ingest…").clicked() {
                    let wiki = self.wiki.get_or_insert_with(|| WikiState::new(&root));
                    if let Err(e) = wiki.begin_ingest(&root, std::path::Path::new(&path)) {
                        self.set_error(e);
                    }
                    close = true;
                }
                if ui.button("Skip").clicked() {
                    close = true;
                }
            });
        });
        if close {
            self.pending_ingest_prompt = None;
        }
    }

    /// [`Self::activate_paper`], plus switching to whichever view makes the
    /// newly active paper visible — the PDF reader when a source document
    /// is available, the kvim editor otherwise (a paper catalogued from
    /// metadata alone still has its research Markdown to read/edit). Shared
    /// by every call site that opens a paper (Wiki, Mindmap, ingestion) so
    /// they land in the same place rather than each picking its own view.
    fn activate_paper_and_navigate(&mut self, citekey: &str) {
        match self.activate_paper(citekey) {
            Ok(()) => {
                let has_pdf = self.active_paper.as_ref().is_some_and(|p| p.pdf_path.is_some());
                self.view = if has_pdf { View::PdfReader } else { View::KvimEditor };
                self.set_status(format!("opened {citekey}"));
            }
            Err(e) => self.set_error(format!("{citekey}: {e}")),
        }
    }

    /// Set a failure message — styled and duplicated at the top of
    /// `side_panel` so it can't be missed the way a plain `self.message`
    /// assignment buried at the bottom of a scrollable panel could be
    /// (op-fueb: this is exactly what made an incomplete calibration look
    /// like "Auto-trace" was silently doing nothing).
    fn set_error(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.message_is_error = true;
    }

    /// Load `path` as the working plot image (PNG/JPEG).
    pub fn load_image(&mut self, path: &str) {
        match PlotRaster::from_path(std::path::Path::new(path)) {
            Ok(r) => {
                self.image_path = path.to_string();
                if self.json_out.is_empty() {
                    self.json_out = format!("{path}.digitised.json");
                }
                self.set_raster(r, format!(
                    "loaded {path} — drag the four reference lines into place, fill their values"
                ));
            }
            Err(e) => self.set_error(e.to_string()),
        }
    }

    /// Load an already-decoded [`PlotRaster`] directly — the hand-off from
    /// the PDF reader's draw-box-then-right-click crop (op-p17q), which has
    /// no file path of its own (it's a region cropped out of a rendered
    /// page/image in memory). `image_path` and the JSON-export default stay
    /// whatever they were, since there's no source path to derive them from
    /// here; the operator fills in `json_out` before exporting, same as any
    /// other required-before-export field. `provenance`, when given, is
    /// carried into [`Self::crop_provenance`] for [`Self::save_into_project`]
    /// (op-96am) to record on the CSV block it later appends into the
    /// project's markdown.
    pub fn load_image_from_raster(&mut self, raster: PlotRaster, provenance: Option<CropProvenance>) {
        self.set_raster(
            raster,
            "cropped from the PDF reader — drag the four reference lines into place, \
             fill their values"
                .to_string(),
        );
        self.crop_provenance = provenance;
    }

    /// Shared reset-to-a-new-image logic between [`Self::load_image`] and
    /// [`Self::load_image_from_raster`]: seed the four axis-reference lines
    /// at 10%/90% of the new image's extent (op-zfnh's persistent box,
    /// replacing the old one-click-per-line flow — a plot's axes are rarely
    /// at the very edge of the figure, so this typically starts closer to
    /// correct than "nothing set yet", and every line is immediately visible
    /// and draggable regardless), and clear any previous dataset/selection.
    fn set_raster(&mut self, raster: PlotRaster, status: String) {
        let (w, h) = (raster.width() as f64, raster.height() as f64);
        self.ref_px = [w * 0.1, w * 0.9, h * 0.9, h * 0.1].map(Some);
        // Parallelogram corners seeded the same 10%/90% inset as the
        // axis-aligned lines (op-vyb9) — starts as a plain rectangle, drag
        // any corner to skew it.
        self.para_corners = [
            Some((w * 0.1, h * 0.1)), // top_left
            Some((w * 0.9, h * 0.1)), // top_right
            Some((w * 0.9, h * 0.9)), // bottom_right
            Some((w * 0.1, h * 0.9)), // bottom_left
        ];
        self.para_dragging = None;
        self.raster = Some(raster);
        self.texture = None; // re-uploaded next frame
        self.dataset = None;
        self.selected = None;
        self.ref_dragging = None;
        self.ref_dragging_corner = None;
        self.crop_provenance = None;
        self.set_status(status);
    }

    /// Build the calibration the four reference points/corners + values
    /// describe — [`CalibrationShape::AxisAligned`] or
    /// [`CalibrationShape::Parallelogram`] depending on
    /// `self.calibration_shape` (op-vyb9). Both branches reuse the same
    /// four `ref_val` text fields (X1/X2/Y1/Y2 — left/right/bottom/top data
    /// values), only the *pixel positions* they pair with differ (4 lines
    /// vs. 4 free corners).
    fn calibration(&self) -> Result<PlotCalibration, String> {
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
        match self.calibration_shape {
            CalibrationShape::AxisAligned => {
                let px = |i: usize, what: &str| {
                    self.ref_px[i]
                        .ok_or_else(|| format!("{what} pixel not set — drag its line into place"))
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
                Ok(PlotCalibration::AxisAligned { x, y })
            }
            CalibrationShape::Parallelogram => {
                let corner_labels = ["top-left", "top-right", "bottom-right", "bottom-left"];
                let corner = |i: usize| {
                    self.para_corners[i].ok_or_else(|| {
                        format!("{} corner not set — drag it into place", corner_labels[i])
                    })
                };
                let pixel_corners = [corner(0)?, corner(1)?, corner(2)?, corner(3)?]
                    .map(|(x, y)| PixelPoint { x, y });
                // Reuse the AxisAligned fields' data-value meaning: X1/X2 are
                // the left/right x values, Y1/Y2 are the bottom/top y values
                // (see `ref_val`'s seeding in `set_raster` and the AxisAligned
                // branch above — Y1 pairs with the larger-row/bottom line,
                // Y2 with the smaller-row/top line).
                let p = ParallelogramCalibration::new(
                    pixel_corners,
                    scale(self.x_log),
                    val(0, "X1 (left)")?,
                    val(1, "X2 (right)")?,
                    scale(self.y_log),
                    val(3, "Y2 (top)")?,
                    val(2, "Y1 (bottom)")?,
                )
                .map_err(|e| e.to_string())?;
                Ok(PlotCalibration::Parallelogram(p))
            }
        }
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
            self.set_error("load an image first");
            return;
        };
        let cal = match self.calibration() {
            Ok(c) => c,
            Err(e) => {
                self.set_error(e);
                return;
            }
        };
        // Auto-trace's column/row scan is inherently axis-aligned (same
        // boundary as `auto.rs`'s own automatic detection pipeline — see
        // its doc comment) — a Parallelogram calibration (op-vyb9) is a
        // hand-digitisation aid for a skewed photo, not something the
        // automatic tracer can drive.
        let PlotCalibration::AxisAligned { x: x_cal, y: y_cal } = cal else {
            self.set_error(
                "Auto-trace needs Rectangle calibration — switch back from Parallelogram, \
                 or hand-place points instead (double-click on the image)",
            );
            return;
        };
        let source = match self.source(raster) {
            Ok(s) => s,
            Err(e) => {
                self.set_error(e);
                return;
            }
        };
        let config = AutoDigitiseConfig {
            x: AxisValueSpec {
                scale: x_cal.scale,
                refs: AxisPixelRefs::Explicit {
                    r1: x_cal.r1,
                    r2: x_cal.r2,
                },
            },
            y: AxisValueSpec {
                scale: y_cal.scale,
                refs: AxisPixelRefs::Explicit {
                    r1: y_cal.r1,
                    r2: y_cal.r2,
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
                let n = d.points.len();
                self.dataset = Some(d);
                self.selected = None;
                self.mode = ClickMode::EditPoints;
                if n == 0 {
                    self.set_error(
                        "auto pass traced 0 points — check the ink threshold/curve colour, \
                         and that the reference box actually brackets the curve",
                    );
                } else {
                    self.set_status(format!(
                        "auto pass traced {n} points — verify, correct, then mark reviewed"
                    ));
                }
            }
            Err(e) => self.set_error(e.to_string()),
        }
    }

    /// Start an empty dataset from the calibration alone, for figures
    /// digitised entirely by hand-placed points.
    fn start_empty(&mut self) {
        let Some(raster) = &self.raster else {
            self.set_error("load an image first");
            return;
        };
        let cal = match self.calibration() {
            Ok(c) => c,
            Err(e) => {
                self.set_error(e);
                return;
            }
        };
        let source = match self.source(raster) {
            Ok(s) => s,
            Err(e) => {
                self.set_error(e);
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
        self.set_status("empty dataset started — click to place points");
    }

    /// Any edit invalidates a recorded review.
    fn mark_edited(&mut self) {
        if let Some(d) = &mut self.dataset {
            if matches!(d.review, ReviewStatus::Reviewed { .. }) {
                d.review = ReviewStatus::Unreviewed;
                self.set_status("edited after review — status reset to UNREVIEWED");
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
        let ((x_minus, x_plus), (y_minus, y_plus)) = xy_uncertainty_interval(&cal, px, py, 0.5, 0.5);
        (p.x_minus, p.x_plus) = (x_minus, x_plus);
        (p.y_minus, p.y_plus) = (y_minus, y_plus);
        p.origin = if hand_placed || matches!(p.origin, PointOrigin::HandPlaced { .. }) {
            PointOrigin::HandPlaced { by }
        } else {
            PointOrigin::HandCorrected { by }
        };
    }

    fn add_point(&mut self, px: f64, py: f64) {
        let Some(d) = &mut self.dataset else {
            self.set_error("run the auto pass or Start empty first");
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
                self.set_status("point deleted");
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
            self.set_status(format!("cleared {n} marker(s)"));
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
            self.set_error("nothing to save");
            return;
        };
        if self.json_out.trim().is_empty() {
            self.set_error("set a JSON output path");
            return;
        }
        if let Err(e) = d.write_json(std::path::Path::new(self.json_out.trim())) {
            self.set_error(e.to_string());
            return;
        }
        let mut saved = format!("saved {}", self.json_out.trim());
        if !self.csv_out.trim().is_empty() {
            match d.write_csv(std::path::Path::new(self.csv_out.trim())) {
                Ok(()) => saved.push_str(&format!(" and {}", self.csv_out.trim())),
                Err(e) => {
                    self.set_error(format!("json saved, csv failed: {e}"));
                    return;
                }
            }
        }
        self.set_status(saved);
    }

    /// Append this dataset's CSV into a "kovan folder" project's
    /// `graph_csvs` section (op-96am: "csvs go straight into markdown with
    /// date and time and author... metadata of which page and exact
    /// pixels"), via [`project::append_to_section`]. `project_root`/
    /// `project_markdown_rel` are operator-supplied (same reasoning as
    /// `json_out`/`csv_out`: a crop has no built-in way to know which
    /// project/document it belongs to). Distinct from [`Self::save`], which
    /// writes a standalone JSON/CSV file wherever asked — this instead
    /// folds the CSV into an existing project document's markdown, keeping
    /// the JSON/CSV export path available unchanged alongside it (op-x9qn's
    /// "CSV auto-saves into markdown, but retain csv export capability").
    fn save_into_project(&mut self) {
        let Some(d) = &self.dataset else {
            self.set_error("nothing to save");
            return;
        };
        if self.project_root.trim().is_empty() || self.project_markdown_rel.trim().is_empty() {
            self.set_error("set the project root and markdown path first");
            return;
        }
        let title = self.figure.trim();
        let title = if title.is_empty() { "Digitised graph" } else { title };
        let mut block = format!("### {title}");
        if let Some(prov) = &self.crop_provenance {
            block.push_str(&format!(
                " — page {}, pixel bbox [{:.1}, {:.1}, {:.1}, {:.1}], {}, {}",
                prov.page_index + 1,
                prov.min.x,
                prov.min.y,
                prov.max.x,
                prov.max.y,
                prov.created_at,
                prov.author
            ));
        }
        block.push_str("\n\n```csv\n");
        block.push_str(&d.to_csv_string());
        block.push_str("```\n");
        match project::append_to_section(
            std::path::Path::new(self.project_root.trim()),
            self.project_markdown_rel.trim(),
            "graph_csvs",
            &block,
        ) {
            Ok(_) => self.set_status("saved into project markdown (graph_csvs)"),
            Err(e) => self.set_error(e.to_string()),
        }
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
        // op-fueb: duplicate any failure at the very TOP of the panel, in a
        // coloured frame — the plain status label at the bottom (kept below,
        // for continuity) sits past four sections' worth of scrolling and is
        // easy to miss entirely, which is exactly what made an incomplete
        // calibration look like "Auto-trace" was silently doing nothing.
        if self.message_is_error {
            egui::Frame::new()
                .fill(Color32::from_rgb(120, 30, 30))
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.colored_label(Color32::WHITE, format!("⚠ {}", self.message));
                });
            ui.separator();
        }
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

        ui.label("1. Calibration shape (op-vyb9):");
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.calibration_shape,
                CalibrationShape::AxisAligned,
                "Rectangle",
            );
            ui.selectable_value(
                &mut self.calibration_shape,
                CalibrationShape::Parallelogram,
                "Parallelogram",
            );
        });
        match self.calibration_shape {
            CalibrationShape::AxisAligned => {
                ui.small("Drag the 4 lines on the image into place, then type what each is worth:");
            }
            CalibrationShape::Parallelogram => {
                ui.small(
                    "Drag the 4 corners on the image into place (for a plot photographed \
                     or scanned at an angle), then type what each edge is worth:",
                );
            }
        }
        let labels = match self.calibration_shape {
            CalibrationShape::AxisAligned => ["X1 (column)", "X2 (column)", "Y1 (row)", "Y2 (row)"],
            CalibrationShape::Parallelogram => {
                ["X1 (left)", "X2 (right)", "Y1 (bottom)", "Y2 (top)"]
            }
        };
        for i in 0..4 {
            ui.horizontal(|ui| {
                ui.label(labels[i]);
                let px_label = match self.calibration_shape {
                    CalibrationShape::AxisAligned => self.ref_px[i].map(|p| format!("px {p:.0}")),
                    CalibrationShape::Parallelogram => self.para_corners[i]
                        .map(|(x, y)| format!("px ({x:.0}, {y:.0})")),
                };
                ui.label(px_label.unwrap_or_else(|| "px —".to_string()));
                ui.label("=");
                ui.add(egui::TextEdit::singleline(&mut self.ref_val[i]).desired_width(70.0));
                // op-fueb: live per-field validity, so an empty/unparseable
                // value is visible the moment it's wrong rather than only
                // after clicking Auto-trace and hunting for the error.
                if self.ref_val[i].trim().parse::<f64>().is_ok() {
                    ui.colored_label(Color32::from_rgb(90, 200, 90), "✓");
                } else if self.ref_val[i].trim().is_empty() {
                    ui.colored_label(Color32::from_rgb(230, 160, 60), "not set");
                } else {
                    ui.colored_label(Color32::from_rgb(230, 90, 90), "not a number");
                }
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
        ui.separator();
        ui.label("Save into project markdown (op-96am):");
        ui.horizontal(|ui| {
            ui.label("project root");
            ui.text_edit_singleline(&mut self.project_root);
        });
        ui.horizontal(|ui| {
            ui.label("markdown path (relative)");
            ui.text_edit_singleline(&mut self.project_markdown_rel);
        });
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
            ui.small("(no PDF-reader crop provenance for this image)");
        }
        if ui.button("Save CSV into project markdown").clicked() {
            self.save_into_project();
        }
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
            // Corner hit test — a slightly larger tolerance than a bare line,
            // since a corner is a single point the pointer has to land near
            // in both axes at once, not a whole line's length.
            let corner_tol = 8.0 / zoom as f64;
            fn hit_ref_corner(
                ref_px: &[Option<f64>; 4],
                tol: f64,
                px: f64,
                py: f64,
            ) -> Option<(usize, usize)> {
                for xi in 0..2 {
                    for yi in 2..4 {
                        if let (Some(rx), Some(ry)) = (ref_px[xi], ref_px[yi]) {
                            if (px - rx).abs() < tol && (py - ry).abs() < tol {
                                return Some((xi, yi));
                            }
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

            // Parallelogram corner hit test (op-vyb9) — nearest of the 4
            // free corners within `corner_tol`, mirroring `hit_ref_corner`'s
            // tolerance but over independent points rather than line
            // intersections.
            fn hit_para_corner(
                corners: &[Option<(f64, f64)>; 4],
                tol: f64,
                px: f64,
                py: f64,
            ) -> Option<usize> {
                let mut best: Option<(usize, f64)> = None;
                for (i, c) in corners.iter().enumerate() {
                    if let Some((cx, cy)) = c {
                        let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                        if d < tol && best.is_none_or(|(_, bd)| d < bd) {
                            best = Some((i, d));
                        }
                    }
                }
                best.map(|(i, _)| i)
            }

            // Reference-line/corner dragging (op-zfnh/op-vyb9) takes priority
            // over marker dragging when a drag starts on top of one — it is
            // checked first and, if it claims the gesture, marker-drag start
            // below is skipped for that same drag via the `else`.
            if response.drag_started_by(PointerButton::Primary) {
                if let Some(pos) = response.interact_pointer_pos() {
                    let (px, py) = to_image(pos);
                    let claimed = match self.calibration_shape {
                        CalibrationShape::AxisAligned => {
                            self.ref_dragging_corner =
                                hit_ref_corner(&self.ref_px, corner_tol, px, py);
                            self.ref_dragging = if self.ref_dragging_corner.is_some() {
                                None
                            } else {
                                hit_ref_line(&self.ref_px, ref_tol, px, py)
                            };
                            self.ref_dragging_corner.is_some() || self.ref_dragging.is_some()
                        }
                        CalibrationShape::Parallelogram => {
                            self.para_dragging =
                                hit_para_corner(&self.para_corners, corner_tol, px, py);
                            self.para_dragging.is_some()
                        }
                    };
                    if !claimed && self.mode == ClickMode::EditPoints {
                        self.dragging = self.nearest_point(px, py, 12.0 / zoom as f64);
                        self.selected = self.dragging;
                    }
                }
            }
            if let (Some((xi, yi)), Some(pos)) =
                (self.ref_dragging_corner, response.interact_pointer_pos())
            {
                if response.dragged_by(PointerButton::Primary) {
                    let (px, py) = to_image(pos);
                    self.ref_px[xi] = Some(px);
                    self.ref_px[yi] = Some(py);
                }
            }
            if let (Some(i), Some(pos)) = (self.ref_dragging, response.interact_pointer_pos()) {
                if response.dragged_by(PointerButton::Primary) {
                    let (px, py) = to_image(pos);
                    self.ref_px[i] = Some(if i < 2 { px } else { py });
                }
            }
            if let (Some(i), Some(pos)) = (self.para_dragging, response.interact_pointer_pos()) {
                if response.dragged_by(PointerButton::Primary) {
                    let (px, py) = to_image(pos);
                    self.para_corners[i] = Some((px, py));
                }
            }
            if self.mode == ClickMode::EditPoints
                && self.ref_dragging.is_none()
                && self.ref_dragging_corner.is_none()
                && self.para_dragging.is_none()
            {
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
                self.ref_dragging_corner = None;
                self.para_dragging = None;
                self.dragging = None;
            }
            if ui.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
                self.delete_selected();
            }

            // --- overlays: reference lines/quad, then points ---
            match self.calibration_shape {
                CalibrationShape::AxisAligned => {
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
                    // Corner-drag handles — a small circle at each of the
                    // four reference-box corners, highlighted while being
                    // dragged, so a corner grab (moves both adjoining lines
                    // at once) is discoverable rather than a hidden
                    // hit-test-only gesture.
                    for (xi, yi) in [(0, 2), (0, 3), (1, 2), (1, 3)] {
                        let (Some(x), Some(y)) = (self.ref_px[xi], self.ref_px[yi]) else {
                            continue;
                        };
                        let pos = to_screen(x, y);
                        let active = self.ref_dragging_corner == Some((xi, yi));
                        painter.circle_filled(
                            pos,
                            if active { 6.0 } else { 4.0 },
                            if active {
                                Color32::from_rgb(255, 210, 60)
                            } else {
                                Color32::from_rgb(60, 120, 255)
                            },
                        );
                        painter.circle_stroke(
                            pos,
                            if active { 6.0 } else { 4.0 },
                            Stroke::new(1.0_f32, Color32::WHITE),
                        );
                    }
                }
                CalibrationShape::Parallelogram => {
                    // The quad's 4 edges, in corner order (top_left ->
                    // top_right -> bottom_right -> bottom_left -> back to
                    // top_left) — draws a skewed outline instead of the
                    // axis-aligned box's independent lines.
                    let screen_corners: Vec<Option<Pos2>> = self
                        .para_corners
                        .iter()
                        .map(|c| c.map(|(x, y)| to_screen(x, y)))
                        .collect();
                    for i in 0..4 {
                        if let (Some(a), Some(b)) =
                            (screen_corners[i], screen_corners[(i + 1) % 4])
                        {
                            painter.line_segment(
                                [a, b],
                                Stroke::new(1.5_f32, Color32::from_rgb(60, 120, 255)),
                            );
                        }
                    }
                    for (i, sc) in screen_corners.iter().enumerate() {
                        let Some(pos) = sc else { continue };
                        let active = self.para_dragging == Some(i);
                        painter.circle_filled(
                            *pos,
                            if active { 6.0 } else { 4.0 },
                            if active {
                                Color32::from_rgb(255, 210, 60)
                            } else {
                                Color32::from_rgb(60, 120, 255)
                            },
                        );
                        painter.circle_stroke(
                            *pos,
                            if active { 6.0 } else { 4.0 },
                            Stroke::new(1.0_f32, Color32::WHITE),
                        );
                    }
                }
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
    /// Top bar: switch between the Digitiser, PDF Reader (op-95x6) and
    /// Markdown Editor (op-wr08) panels, and the Gruvbox theme selector
    /// (op-t5sq).
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.view, View::Home, "Home");
            ui.selectable_value(&mut self.view, View::Wiki, "Wiki");
            ui.selectable_value(&mut self.view, View::Mindmap, "Mindmap");
            ui.selectable_value(&mut self.view, View::AdvancedGit, "Advanced Git");
            ui.selectable_value(&mut self.view, View::Digitiser, "Digitiser");
            ui.selectable_value(&mut self.view, View::PdfReader, "PDF Reader");
            // op-shjn (GH issue #35, "the markdown editor should use the
            // kvim editor"): Kvim is the one user-facing paper-markdown
            // editor. The older hand-rolled markdown_editor.rs was retired
            // (op-9r26) once Bibliography moved off the old crate::project
            // model, so there is nothing left for a second nav button to
            // point at.
            ui.selectable_value(&mut self.view, View::KvimEditor, "Kvim Editor");
            ui.selectable_value(&mut self.view, View::Bibliography, "Bibliography");
            ui.selectable_value(&mut self.view, View::TableDigitiser, "Table Digitiser");
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
        if target.is_directory() {
            self.file_dialog.pick_directory();
            return;
        }
        self.file_dialog.config_mut().default_file_filter =
            target.default_filter().map(str::to_string);
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
            FileDialogTarget::Pdf => {
                self.pdf_reader.open(&path);
                self.offer_ingest_if_new(&path);
            }
            FileDialogTarget::JsonExport => self.json_out = path,
            FileDialogTarget::CsvExport => self.csv_out = path,
            FileDialogTarget::KovanRootOpen => self.home.open_dir(std::path::Path::new(&path)),
            FileDialogTarget::KovanRootCreate => self.home.begin_create(std::path::Path::new(&path)),
            FileDialogTarget::PdfIngest => {
                if let (Some(root), Some(wiki)) = (self.home.root(), self.wiki.as_mut()) {
                    if let Err(message) = wiki.begin_ingest(root, std::path::Path::new(&path)) {
                        self.set_error(message);
                    }
                }
            }
            FileDialogTarget::KvimFile => match std::fs::read_to_string(&path) {
                Ok(text) => self.kvim_editor.load_text(&text),
                Err(e) => self.set_error(format!("{path}: {e}")),
            },
        }
    }
}

impl eframe::App for DigitiseApp {
    // eframe 0.34 hands the root `Ui`; panels nest with `show_inside`,
    // CentralPanel last (same pattern as the workspace's digital-twin GUIs).
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.theme.apply(ui.ctx());

        egui::Panel::top("topbar")
            .show(ui, |ui| self.top_bar(ui));

        self.file_dialog.update(ui.ctx());
        if let Some(path) = self.file_dialog.take_picked() {
            self.handle_picked_file(&path);
        }
        self.ingest_prompt_ui(ui.ctx());

        // §8: "after opening a root, land in the Wiki, not the PDF
        // reader." A root becomes available asynchronously (the directory
        // picker resolves on a later frame than the button click), so this
        // checks every frame rather than at the click site. Only fires from
        // `Home` — once the user has navigated elsewhere, opening a
        // *different* root later does not yank them away from what they
        // were doing.
        if self.view == View::Home {
            if let Some(root) = self.home.root() {
                if self.wiki.is_none() {
                    self.wiki = Some(WikiState::new(root));
                }
                self.view = View::Wiki;
            }
        }

        match self.view {
            View::Home => {
                egui::CentralPanel::default().show(ui, |ui| {
                    if let Some(action) = self.home.ui(ui) {
                        match action {
                            HomeAction::RequestOpenDialog => self.open_picker(FileDialogTarget::KovanRootOpen),
                            HomeAction::RequestCreateDialog => self.open_picker(FileDialogTarget::KovanRootCreate),
                        }
                    }
                });
            }
            View::Wiki => {
                let mut ingest_clicked = false;
                let mut opened_paper = None;
                if let Some(root) = self.home.root().cloned() {
                    egui::CentralPanel::default().show(ui, |ui| {
                        if let Some(wiki) = self.wiki.as_mut() {
                            match wiki.ui(ui, &root) {
                                Some(WikiAction::RequestIngestDialog) => ingest_clicked = true,
                                Some(WikiAction::OpenPaper(citekey)) => opened_paper = Some(citekey),
                                None => {}
                            }
                        }
                    });
                } else {
                    // No root open (e.g. the user navigated here directly
                    // via the top bar) — nothing to browse yet.
                    egui::CentralPanel::default().show(ui, |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.weak("no Kovan folder open — go to Home to open or create one");
                        });
                    });
                }
                if ingest_clicked {
                    self.open_picker(FileDialogTarget::PdfIngest);
                }
                // op-sr4n.2: a paper link was clicked, or "Ingest & Open"
                // just finished — activate it and jump to it, same as
                // Mindmap's own OpenPaper below.
                if let Some(citekey) = opened_paper {
                    self.activate_paper_and_navigate(&citekey);
                }
            }
            View::Mindmap => {
                if let Some(root) = self.home.root().cloned() {
                    let index = crate::index::KnowledgeIndex::load_or_rebuild(&root);
                    let graph = crate::graph::KnowledgeGraph::load_or_rebuild(&root, &index);
                    let mut opened_paper = None;
                    egui::CentralPanel::default().show(ui, |ui| {
                        if let Some(MindmapAction::OpenPaper(citekey)) = self.mindmap.ui(ui, &root, &index, &graph) {
                            opened_paper = Some(citekey);
                        }
                    });
                    if let Some(citekey) = opened_paper {
                        // op-sr4n.3: route through the same
                        // activate_paper/view-switch helper Wiki uses,
                        // rather than each view picking its own paper-
                        // opening behaviour.
                        self.activate_paper_and_navigate(&citekey);
                    }
                } else {
                    egui::CentralPanel::default().show(ui, |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.weak("no Kovan folder open — go to Home to open or create one");
                        });
                    });
                }
            }
            View::AdvancedGit => {
                if let Some(root) = self.home.root().cloned() {
                    egui::CentralPanel::default().show(ui, |ui| {
                        self.advanced_git.ui(ui, &root);
                    });
                } else {
                    egui::CentralPanel::default().show(ui, |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.weak("no Kovan folder open — go to Home to open or create one");
                        });
                    });
                }
            }
            View::Digitiser => {
                egui::Panel::left("controls")
                    .min_size(290.0)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| self.side_panel(ui));
                    });
                // op-5sdc: CSV preview + copy button, right-hand side,
                // htgr_sim_v1-style — see csv_preview.rs.
                egui::Panel::right("csv_preview")
                    .min_size(260.0)
                    .show(ui, |ui| {
                        if let Some(d) = &self.dataset {
                            draw_csv_preview(ui, &d.to_csv_string());
                        } else {
                            ui.centered_and_justified(|ui| {
                                ui.label("no dataset yet — run the auto pass or Start empty");
                            });
                        }
                    });
                egui::CentralPanel::default().show(ui, |ui| self.image_panel(ui));
            }
            View::PdfReader => {
                let mut open_clicked = false;
                let mut crop_result = None;
                egui::CentralPanel::default().show(ui, |ui| {
                    // op-q1qj: the active paper's session (if any) so the
                    // reader saves annotations straight into it instead of
                    // asking for a project root/markdown path.
                    let active_session = self.active_paper.as_mut().map(|p| &mut p.session);
                    crop_result = self.pdf_reader.ui(ui, || open_clicked = true, active_session);
                });
                if open_clicked {
                    self.open_picker(FileDialogTarget::Pdf);
                }
                // op-p17q/op-hnhp: the reader just completed a
                // crop-then-right-click gesture — load the cropped region
                // into the matching digitiser tab and switch to it, so
                // "close" (switching back to the reader) is the natural way
                // to return, per the issue's own "popup window? Or new
                // tab? ... after I'm done, I close" — this window's
                // existing view switch already fills that role, so no
                // separate popup/tab was added.
                match crop_result {
                    Some(pdf_reader::CropResult::Plot(raster, provenance)) => {
                        self.load_image_from_raster(raster, Some(provenance));
                        self.view = View::Digitiser;
                    }
                    Some(pdf_reader::CropResult::Table(raster, provenance)) => {
                        self.table_digitiser.load_crop(raster, Some(provenance));
                        self.view = View::TableDigitiser;
                    }
                    None => {}
                }
            }
            View::KvimEditor => {
                let mut open_clicked = false;
                let mut save_clicked = false;
                let root = self.home.root().cloned();
                let index = root.as_ref().map(crate::index::KnowledgeIndex::load_or_rebuild);
                let active_citekey = self.active_paper.as_ref().map(|p| p.session.citekey().to_string());
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        match &active_citekey {
                            // op-sr4n.5: following the active paper (GH
                            // issue #35's "unify root and active-paper
                            // context" comment) is the normal path; "Open…"
                            // stays available as the comment's own allowed
                            // "explicit secondary action" for a standalone
                            // file outside any paper.
                            Some(citekey) => {
                                ui.strong(format!("\u{1F4C4} {citekey}"));
                                if self.kvim_editor.is_modified() && ui.button("Save").clicked() {
                                    save_clicked = true;
                                }
                            }
                            None => {
                                ui.weak("no paper selected — pick one from Wiki or Mindmap");
                            }
                        }
                        if ui.button("Open external file…").clicked() {
                            open_clicked = true;
                        }
                    });
                    let completion = match (&root, &index) {
                        (Some(root), Some(index)) => Some(kvim_editor::CompletionSource { root, index }),
                        _ => None,
                    };
                    self.kvim_editor.ui(ui, completion);
                });
                if open_clicked {
                    self.open_picker(FileDialogTarget::KvimFile);
                }
                if save_clicked {
                    // §37 "Save Document": writes the buffer to disk,
                    // nothing more — no Git staging/commit (that is Save
                    // Repository, op-9vo6.19, not built yet).
                    let text = self.kvim_editor.text();
                    if let Some(active) = &mut self.active_paper {
                        active.session.set_markdown(text);
                        match active.session.save_document() {
                            Ok(()) => {
                                let citekey = active.session.citekey().to_string();
                                self.kvim_editor.load_text(active.session.markdown());
                                self.set_status(format!("saved {citekey}"));
                            }
                            Err(e) => self.set_error(e.to_string()),
                        }
                    }
                }
            }
            View::Bibliography => {
                // op-9r26: Bibliography now follows the already-open Kovan
                // root automatically — no project-folder picker in the
                // normal workflow (GH issue #35: "Do not ask the user to
                // select a project folder... Do not open a folder dialog").
                if let Some(root) = self.home.root().cloned() {
                    let index = crate::index::KnowledgeIndex::load_or_rebuild(&root);
                    let mut action = None;
                    egui::CentralPanel::default().show(ui, |ui| {
                        action = self.bibliography.ui(ui, &root, &index);
                    });
                    if let Some(BibliographyAction::OpenPaper(citekey)) = action {
                        self.activate_paper_and_navigate(&citekey);
                    }
                } else {
                    egui::CentralPanel::default().show(ui, |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.weak("no Kovan folder open — go to Home to open or create one");
                        });
                    });
                }
            }
            View::TableDigitiser => {
                egui::CentralPanel::default().show(ui, |ui| {
                    self.table_digitiser.ui(ui);
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::Access;
    use crate::ingest::{self, IngestChoice};
    use crate::root::{KovanRoot, RootConfig};

    fn make_root() -> (tempfile::TempDir, KovanRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        (dir, root)
    }

    /// A tiny, structurally valid one-page PDF with an `/Info` `/Title` —
    /// same fixture `ingest.rs`'s own tests use, duplicated per this
    /// workspace's existing per-file test-fixture convention (see
    /// `ingest.rs`/`mindmap.rs`/`autocomplete.rs`'s own copies) rather than
    /// adding cross-module test-only public surface.
    fn write_test_pdf(path: &std::path::Path, title: &str) {
        use lopdf::{dictionary, Document, Object};
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages_id });
        let pages = dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(page_id)], "Count" => 1 };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        let info_id = doc.add_object(dictionary! { "Title" => Object::string_literal(title) });
        doc.trailer.set("Info", info_id);
        doc.save(path).unwrap();
    }

    fn ingest_one(root: &KovanRoot, dir: &std::path::Path, title: &str, access: Access) -> String {
        let pdf_path = dir.join(format!("{title}.pdf"));
        write_test_pdf(&pdf_path, title);
        let preview = ingest::preview(root, &pdf_path).unwrap();
        let citekey = preview.suggested_citekey.clone();
        let choice =
            IngestChoice { citekey: citekey.clone(), access, topics: vec!["htgrs".to_string()], projects: vec![] };
        ingest::ingest(root, &preview, choice).unwrap();
        citekey
    }

    /// GH issue #35's 2026-09-01 "unify root and active-paper context"
    /// comment, "Testing requirements" section: "activate a known paper ->
    /// PaperSession opens -> Markdown path resolves -> source PDF path
    /// resolves when present".
    #[test]
    fn activate_paper_resolves_session_and_pdf_path() {
        let (dir, root) = make_root();
        let citekey = ingest_one(&root, dir.path(), "Coupled Neutronics Methodology", Access::Open);

        let mut app = DigitiseApp::default();
        app.home.open_dir(root.path());
        app.activate_paper(&citekey).unwrap();

        let active = app.active_paper.as_ref().expect("activation should have set active_paper");
        assert_eq!(active.session.citekey(), citekey);
        assert_eq!(active.session.markdown_path(), root.paper_markdown(&citekey));
        let pdf_path = active.pdf_path.as_ref().expect("an Open-access ingested paper keeps its PDF locally");
        assert!(pdf_path.is_file());
    }

    /// Same section, "Missing PDF": "activate a known paper with no local
    /// PDF -> activation succeeds -> PDF state reports unavailable ->
    /// paper/Markdown remain usable."
    #[test]
    fn activate_paper_succeeds_when_the_recorded_pdf_is_missing_locally() {
        let (dir, root) = make_root();
        let citekey = ingest_one(&root, dir.path(), "Graphite Thermal Conductivity", Access::Open);
        // Simulate the recorded PDF having gone missing from disk (e.g. a
        // clone of the repo without the submodule/proprietary content
        // pulled) without touching kovan.toml's own [source].pdf pointer.
        let stored_pdf = root.open_sources_dir().join(format!("{citekey}.pdf"));
        std::fs::remove_file(&stored_pdf).unwrap();

        let mut app = DigitiseApp::default();
        app.home.open_dir(root.path());
        app.activate_paper(&citekey).unwrap();

        let active = app.active_paper.as_ref().expect("activation should still succeed with no local PDF");
        assert_eq!(active.session.citekey(), citekey);
        assert!(active.pdf_path.is_none(), "a missing PDF must report unavailable, not a stale path");
    }

    #[test]
    fn activate_paper_fails_cleanly_with_no_root_open() {
        let mut app = DigitiseApp::default();
        assert!(app.activate_paper("does-not-matter").is_err());
        assert!(app.active_paper.is_none());
    }

    /// "Cross-view identity: ensure Wiki/Mindmap activation resolves to the
    /// same paper identity and paths rather than constructing independent
    /// representations" — both call sites route through
    /// `activate_paper_and_navigate`, so this pins that they land on the
    /// same view/state regardless of which one triggered it.
    #[test]
    fn activate_paper_and_navigate_lands_on_pdf_reader_when_a_pdf_is_present() {
        let (dir, root) = make_root();
        let citekey = ingest_one(&root, dir.path(), "Fuel Cladding Gap Conductance", Access::Open);

        let mut app = DigitiseApp::default();
        app.home.open_dir(root.path());
        app.activate_paper_and_navigate(&citekey);

        assert_eq!(app.view, View::PdfReader);
        assert!(app.active_paper.is_some());
    }

    #[test]
    fn activate_paper_and_navigate_falls_back_to_kvim_editor_with_no_pdf() {
        let (dir, root) = make_root();
        let citekey = ingest_one(&root, dir.path(), "Metadata Only Record", Access::Open);
        std::fs::remove_file(root.open_sources_dir().join(format!("{citekey}.pdf"))).unwrap();

        let mut app = DigitiseApp::default();
        app.home.open_dir(root.path());
        app.activate_paper_and_navigate(&citekey);

        assert_eq!(app.view, View::KvimEditor);
    }
}
