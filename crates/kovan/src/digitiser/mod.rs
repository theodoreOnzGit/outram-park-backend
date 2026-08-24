//! # Graph digitiser — extract `(x, y)` data points from plot images
//!
//! Several validation targets in this project exist **only as figures in
//! papers** (HTR-10 safety-demonstration transients, MSRE reactivity-insertion
//! curves, the Tobias decay-heat plots). This module turns a raster image of a
//! published plot into numeric data points *with the provenance record that
//! makes them usable as validation evidence* (`DATA_POLICY.md`: digitisation
//! is a processing step and must be documented as one).
//!
//! ## What belongs in this module
//!
//! - [`raster`] — loading a plot image into an owned RGB buffer (pure-Rust
//!   decoding via the `image` crate; PNG and JPEG).
//! - [`calibration`] — mapping pixel coordinates to data coordinates.
//!   [`calibration::PlotCalibration`] is enum-dispatched over two shapes
//!   (op-vyb9): [`calibration::PlotCalibration::AxisAligned`] (the
//!   original — **linear and logarithmic axes independently per axis**,
//!   with log axes calibrated in log10 space, never by linear pixel
//!   interpolation) and [`calibration::PlotCalibration::Parallelogram`] (a
//!   skewed pixel-space quadrilateral mapped onto a rectilinear data
//!   rectangle via a 2D projective transform, for a plot photographed or
//!   scanned at an angle — GUI-interactive only, see [`auto`]'s doc for why
//!   the automatic pipeline stays axis-aligned-only).
//! - [`detect`] — automatic detection of the plot frame (axis box) from dark
//!   line runs. Deterministic; no ML, no OCR (unlike [`table_ocr`] below,
//!   whose OCR use is a deliberate, separately-decided exception — see its
//!   own module doc).
//! - [`trace`] — automatic curve tracing by column scan, with enum-dispatched
//!   strategies ([`trace::TraceStrategy`]) and colour selectors
//!   ([`trace::CurveSelector`]).
//! - [`dataset`] — the output types. [`dataset::DigitisedDataset`] is
//!   deliberately impossible to construct or export without its
//!   [`calibration::PlotCalibration`] and [`dataset::FigureSource`] attached.
//! - [`auto`] — the one-shot automatic pipeline shared by all front ends.
//! - [`synthetic`] — deterministic rendering of known curves to images, used
//!   as self-consistency test fixtures (and later to cross-check the
//!   maintainer-supplied golden oracle, bead `op-amfh`).
//! - [`frontend`] — the shared `clap` argument surface used by `kovan-cli
//!   digitise` (the automatic-only path) and `kovan-tui`'s Digitiser tab
//!   (automatic pass, then interactive review). Compiled unconditionally:
//!   `clap` is already a hard dependency of this crate's own `kovan-cli`, so
//!   — unlike when this module lived in `kovan-literature`, where `clap` was
//!   optional — there is nothing left to gate.
//! - [`table_ocr`] — table digitisation (op-hnhp): OCR text recognition
//!   over a cropped table region via `kopitiam_ocr` (op-9bvi's engine
//!   decision), split into cells by a whitespace-run heuristic, with the
//!   same [`dataset::ReviewStatus`] human-review gate the plot digitiser
//!   uses. Compiled unconditionally — like `frontend`, it needs no GUI, so
//!   `kovan-cli`/`kovan-tui` could drive it too even though only the GUI
//!   does today.
//! - [`gui`] *(behind this crate's `gui` feature, default except on
//!   Android)* — the egui app powering the `kovan` binary, exposed as a
//!   library function (`gui::run`). Its `desktop` submodule also carries
//!   GitHub issue #30's file picker (`egui-file-dialog`, op-689u), Gruvbox
//!   theming (op-t5sq), and integrated PDF reader panel (op-95x6, over
//!   `kopitiam_pdf::mupdf` — see the next bullet).
//!
//! ## What does not belong here
//!
//! - OCR / reading printed tick labels. KOVAN is deterministic and offline
//!   (no ML), so **numeric axis values must be supplied by the caller** (they
//!   are stated in the figure's caption/axes and are facts, not guesses); the
//!   pixel geometry is what gets automated. (GitHub issue #30 has since asked
//!   for OCR specifically for *table* digitisation, which is new ground for
//!   this crate and needs an explicit decision — tracked as bead `op-9bvi`,
//!   not yet made.)
//! - Network access of any kind.
//! - PDF *parsing* (text/metadata extraction) — that stays
//!   `kovan_literature::extract_metadata`'s job. This module's own PDF
//!   involvement is display-only: `gui`'s private `desktop::pdf_reader`
//!   submodule opens a PDF with `kopitiam_pdf::mupdf::PdfDocument` and
//!   rasterizes the current page
//!   with `kopitiam_pdf::mupdf::rasterize_page` (op-6ez3's rendering-engine
//!   decision) so it can be shown as a `kovan` GUI panel. It does not (yet)
//!   feed a rasterized page into the digitiser as a plot-image source —
//!   that's the draw-box-then-digitise interaction, a separate bead
//!   (op-p17q) this panel is built to support but does not itself implement.
//!
//! ## Units and `uom`
//!
//! Digitised axes carry whatever units the source figure printed — often
//! non-SI, arbitrary, or normalised (e.g. "% of operating power",
//! "MeV/fission·s"). The engine therefore works in plain `f64` *document
//! units* and records the axis label text verbatim in
//! [`dataset::DigitisedDataset::x_label`]/`y_label`; converting into `uom`
//! quantities is the consumer's job, at the point where the unit is actually
//! interpreted. Forcing `uom` here would require inventing dimensions for
//! axes the engine cannot know.
//!
//! ## Verification status (honest limits)
//!
//! The engine is verified by **synthetic self-consistency tests only**
//! (`tests/digitiser_synthetic.rs`): known curves are rendered to images at
//! known pixel positions, digitised, and compared against the analytic
//! values, for linear-linear, log-linear and log-log axes. Measured accuracy
//! figures live in that test file's doc comments. **No accuracy claim is made
//! against real published figures** — the hand-digitised golden oracle
//! (Tobias decay-heat points, bead `op-amfh`) does not exist yet. When it
//! lands, compare with [`synthetic`]-style tolerance checks against
//! [`dataset::DigitisedDataset`] output over the real scans.

pub mod auto;
pub mod calibration;
pub mod dataset;
pub mod detect;
pub mod frontend;
#[cfg(feature = "gui")]
pub mod gui;
pub mod raster;
pub mod synthetic;
pub mod table_ocr;
pub mod trace;

/// Errors produced by the graph digitiser.
///
/// Enum-dispatched per the workspace Rust design rules (no trait objects).
/// Every variant carries a human-readable message describing what failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DigitiserError {
    /// The image file could not be read or decoded (bad path, unsupported
    /// format, corrupt data).
    Image(String),
    /// Axis calibration is invalid — coincident reference pixels, coincident
    /// reference values, or non-positive values on a logarithmic axis.
    Calibration(String),
    /// The plot frame (axis box) could not be detected automatically.
    Detection(String),
    /// Curve tracing failed (e.g. no curve pixels found inside the frame).
    Trace(String),
    /// A dataset file could not be read, written, or parsed.
    Io(String),
    /// Table OCR (`table_ocr` — op-hnhp) failed: the `.traineddata` model
    /// could not be loaded, or line recognition itself failed.
    Ocr(String),
}

impl std::fmt::Display for DigitiserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DigitiserError::Image(m) => write!(f, "image error: {m}"),
            DigitiserError::Calibration(m) => write!(f, "calibration error: {m}"),
            DigitiserError::Detection(m) => write!(f, "axis detection error: {m}"),
            DigitiserError::Trace(m) => write!(f, "trace error: {m}"),
            DigitiserError::Io(m) => write!(f, "dataset io error: {m}"),
            DigitiserError::Ocr(m) => write!(f, "OCR error: {m}"),
        }
    }
}

impl std::error::Error for DigitiserError {}
