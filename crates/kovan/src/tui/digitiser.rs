//! Digitiser tab — interactive graph digitisation: a Setup form, an
//! automatic pass on a worker thread, then a terminal review screen.
//!
//! **Absorbed the standalone `kovan-digitise-tui` binary into this crate's
//! TUI on 2026-08-21**, per GitHub issue #30's final 3-binary spec (`kovan`
//! GUI, `kovan-cli` agent CLI, `kovan-tui` terminal UI — see the workspace
//! root `CLAUDE.md` and this crate's `NOTICE`). The automatic-pipeline
//! arguments ([`AutoArgs`]) and the review mechanics (nudge / delete /
//! duplicate / mark-reviewed / save) are unchanged from that binary; only the
//! phase machine and key/draw dispatch were adapted to this tab's shape,
//! mirroring the [`super::ingest`] tab's `Picking/Setup -> Running -> Review`
//! pattern (worker thread + `mpsc` channel so a slow trace never blocks the
//! draw loop; see that module's docs for why a thread is the right tool
//! here too).
//!
//! # The flow
//!
//! ```text
//! Setup ──Enter (on last field)──▶ Running ──ok──▶ Review ──S/s──▶ (saved)
//!   ▲                                 │              │
//!   └────────────x────────────────────┴──error──▶ Failed ──x/Enter──┘
//! ```
//!
//! Only frame-edge calibration (`--x-range`/`--y-range`) is exposed here —
//! the explicit-pixel-reference form (`--x-ref`/`--y-ref`) needs mouse
//! clicks on the image and stays a `kovan` (GUI)-only capability, matching
//! this tab's terminal-cell resolution limits (see the module docs on the
//! review screen below).
//!
//! Terminal cells are coarse (each cell is at best 1x2 image pixels, usually
//! far less), so this reviewer is for *sanity checking and coarse fixes* on a
//! headless box or over SSH — Termux included; fine-grained editing belongs
//! to `kovan`, the GUI. Point-adding is limited to duplicating an existing
//! point and nudging it; free placement needs the GUI's mouse.

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Instant;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::digitiser::dataset::{
    utc_now_iso8601, xy_uncertainty_interval, DigitisedDataset, PointOrigin, ReviewInterface,
    ReviewStatus,
};
use crate::digitiser::frontend::AutoArgs;
use crate::digitiser::raster::PlotRaster;
use crate::digitiser::DigitiserError;

use super::text_input::TextInput;

/// Frames of the liveness spinner shown while the automatic pass runs.
const SPINNER: [char; 4] = ['|', '/', '-', '\\'];

/// One field of the Setup form. Order matches [`SETUP_FIELDS`], which is
/// also the order Up/Down/Tab cycles through them and the order the form is
/// drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupField {
    /// Path to the plot image (PNG or JPEG).
    Image,
    /// `linear` or `log`.
    XScale,
    /// `min,max` at the detected frame's left/right edges.
    XRange,
    /// `linear` or `log`.
    YScale,
    /// `min,max` at the detected frame's bottom/top edges.
    YRange,
    /// Figure designation as printed, e.g. `"Fig. 7"`. Required provenance.
    Figure,
    /// x-axis label as printed (units included).
    XLabel,
    /// y-axis label as printed (units included).
    YLabel,
    /// Recorded as `digitised_by` / the reviewer name on edits.
    Operator,
}

const SETUP_FIELDS: [SetupField; 9] = [
    SetupField::Image,
    SetupField::XScale,
    SetupField::XRange,
    SetupField::YScale,
    SetupField::YRange,
    SetupField::Figure,
    SetupField::XLabel,
    SetupField::YLabel,
    SetupField::Operator,
];

impl SetupField {
    fn label(self) -> &'static str {
        match self {
            SetupField::Image => "Image path",
            SetupField::XScale => "X scale (linear/log)",
            SetupField::XRange => "X range (min,max)",
            SetupField::YScale => "Y scale (linear/log)",
            SetupField::YRange => "Y range (min,max)",
            SetupField::Figure => "Figure (e.g. \"Fig. 7\")",
            SetupField::XLabel => "X label",
            SetupField::YLabel => "Y label",
            SetupField::Operator => "Operator",
        }
    }

    fn step(self, delta: i32) -> Self {
        let i = SETUP_FIELDS.iter().position(|f| *f == self).unwrap_or(0) as i32;
        let n = SETUP_FIELDS.len() as i32;
        let j = ((i + delta) % n + n) % n;
        SETUP_FIELDS[j as usize]
    }
}

/// The automatic pass running on a worker thread.
pub struct RunningDigitise {
    image: String,
    started: Instant,
    frame: usize,
    receiver: Receiver<Result<(PlotRaster, DigitisedDataset), String>>,
}

impl RunningDigitise {
    fn spinner(&self) -> char {
        SPINNER[self.frame % SPINNER.len()]
    }
}

/// A failed automatic pass, kept on screen until dismissed.
pub struct FailureReport {
    message: String,
}

/// Review-screen state: the traced dataset, ready for the operator to nudge,
/// delete, duplicate, and mark reviewed. Field-for-field the same session
/// state the former standalone `kovan-digitise-tui` binary's `App` held.
pub struct ReviewState {
    raster: PlotRaster,
    dataset: DigitisedDataset,
    operator: String,
    json_path: TextInput,
    csv_path: TextInput,
    selected: usize,
    dirty: bool,
    message: String,
}

impl ReviewState {
    fn new(raster: PlotRaster, dataset: DigitisedDataset, operator: String, image: &str) -> Self {
        Self {
            raster,
            dataset,
            operator,
            json_path: TextInput::new(format!("{image}.digitised.json")),
            csv_path: TextInput::default(),
            selected: 0,
            dirty: false,
            message: "automatic pass loaded — verify the overlay, then `v` + `S`".to_string(),
        }
    }

    fn select(&mut self, delta: i64) {
        let n = self.dataset.points.len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected as i64 + delta).rem_euclid(n as i64) as usize;
    }

    /// Move the selected point by whole image pixels and recompute its data
    /// coordinates + reading uncertainty through the dataset's calibration.
    fn nudge(&mut self, dx: f64, dy: f64) {
        let cal = self.dataset.calibration;
        let Some(p) = self.dataset.points.get_mut(self.selected) else {
            return;
        };
        // Fallback pixel position when one/both of `x_px`/`y_px` weren't
        // recorded — needs both data values at once, not one axis at a
        // time, since a parallelogram calibration's inverse map is coupled
        // (op-vyb9; `AxisAligned`'s own per-axis independence still holds,
        // this is just the shared entry point).
        let (fallback_x_px, fallback_y_px) = cal.pixel_at(p.x, p.y).unwrap_or((0.0, 0.0));
        let x_px = p.x_px.unwrap_or(fallback_x_px) + dx;
        let y_px = p.y_px.unwrap_or(fallback_y_px) + dy;
        p.x_px = Some(x_px);
        p.y_px = Some(y_px);
        (p.x, p.y) = cal.point_at(x_px, y_px);
        // Hand precision: half-pixel reading error on both axes.
        let ((x_minus, x_plus), (y_minus, y_plus)) =
            xy_uncertainty_interval(&cal, x_px, y_px, 0.5, 0.5);
        (p.x_minus, p.x_plus) = (x_minus, x_plus);
        (p.y_minus, p.y_plus) = (y_minus, y_plus);
        if !matches!(p.origin, PointOrigin::HandPlaced { .. }) {
            p.origin = PointOrigin::HandCorrected {
                by: self.operator.clone(),
            };
        }
        self.mark_edited();
    }

    fn delete_selected(&mut self) {
        if self.dataset.points.is_empty() {
            return;
        }
        self.dataset.points.remove(self.selected);
        if self.selected >= self.dataset.points.len() && self.selected > 0 {
            self.selected -= 1;
        }
        self.mark_edited();
        self.message = "point deleted".to_string();
    }

    /// Insert a `HandPlaced` copy of the selected point right after it (then
    /// nudge it into place). The TUI's substitute for mouse placement.
    fn duplicate_selected(&mut self) {
        let Some(p) = self.dataset.points.get(self.selected) else {
            return;
        };
        let mut q = p.clone();
        q.origin = PointOrigin::HandPlaced {
            by: self.operator.clone(),
        };
        self.dataset.points.insert(self.selected + 1, q);
        self.selected += 1;
        self.mark_edited();
        self.message = "hand-placed copy inserted — nudge it into position".to_string();
    }

    /// Any edit invalidates a previously recorded review: reviewing covers
    /// the points as they were, so the status drops back to Unreviewed.
    fn mark_edited(&mut self) {
        if matches!(self.dataset.review, ReviewStatus::Reviewed { .. }) {
            self.dataset.review = ReviewStatus::Unreviewed;
            self.message = "edited after review — status reset to UNREVIEWED".to_string();
        }
        self.dirty = true;
    }

    fn save(&mut self) {
        if let Err(e) = self
            .dataset
            .write_json(std::path::Path::new(self.json_path.value()))
        {
            self.message = format!("save failed: {e}");
            return;
        }
        if !self.csv_path.value().is_empty() {
            if let Err(e) = self
                .dataset
                .write_csv(std::path::Path::new(self.csv_path.value()))
            {
                self.message = format!("json saved, csv failed: {e}");
                return;
            }
        }
        self.dirty = false;
        self.message = format!("saved to {}", self.json_path.value());
    }
}

/// The tab's state machine. Enum dispatch, no trait objects — matches the
/// [`super::ingest::IngestPhase`] shape.
#[allow(clippy::large_enum_variant)]
pub enum DigitiserPhase {
    Setup,
    Running(RunningDigitise),
    Review(ReviewState),
    Failed(FailureReport),
}

/// State for the Digitiser tab.
pub struct DigitiserState {
    pub phase: DigitiserPhase,
    field: SetupField,
    image: TextInput,
    x_scale: TextInput,
    x_range: TextInput,
    y_scale: TextInput,
    y_range: TextInput,
    figure: TextInput,
    x_label: TextInput,
    y_label: TextInput,
    operator: TextInput,
    pub status: String,
    edit_backup: String,
}

impl Default for DigitiserState {
    fn default() -> Self {
        Self {
            phase: DigitiserPhase::Setup,
            field: SetupField::Image,
            image: TextInput::default(),
            x_scale: TextInput::new("linear"),
            x_range: TextInput::default(),
            y_scale: TextInput::new("linear"),
            y_range: TextInput::default(),
            figure: TextInput::default(),
            x_label: TextInput::new("x"),
            y_label: TextInput::new("y"),
            operator: TextInput::new("kovan-tui (interactive)"),
            status: "Up/Down: field  e: edit  Enter (on last field): run  q: quit".to_string(),
            edit_backup: String::new(),
        }
    }
}

impl DigitiserState {
    /// `true` while the automatic pass is running — the draw loop polls
    /// faster then, matching the Ingest tab's [`super::ingest::IngestState::is_busy`].
    pub fn is_busy(&self) -> bool {
        matches!(self.phase, DigitiserPhase::Running(_))
    }

    /// `true` when a global `q`/`Esc` would throw away work in progress.
    pub fn blocks_quit(&self) -> bool {
        matches!(
            self.phase,
            DigitiserPhase::Running(_) | DigitiserPhase::Review(_)
        )
    }

    pub fn note_blocked_quit(&mut self) {
        self.status = match self.phase {
            DigitiserPhase::Running(_) => {
                "automatic pass still running — press 'x' to abandon it, then 'q'".to_string()
            }
            _ => "review not saved — press 'S' to save or 'x' to discard, then 'q'".to_string(),
        };
    }

    pub fn help_line(&self) -> &'static str {
        match self.phase {
            DigitiserPhase::Setup => {
                "Up/Down: field  e: edit  Enter: run digitiser  1-7: tabs  q: quit"
            }
            DigitiserPhase::Running(_) => "tracing… x: abandon (q/Esc will not quit while running)",
            DigitiserPhase::Review(_) => {
                "Tab/←→ select · ↑↓ nudge y · h/l nudge x (Shift=5px) · d delete · \
                 a duplicate · v mark reviewed · e: edit output path · S save · x discard · q quit"
            }
            DigitiserPhase::Failed(_) => "x / Enter: back to setup  1-7: tabs  q: quit",
        }
    }

    fn focused_input_mut(&mut self) -> Option<&mut TextInput> {
        if matches!(self.phase, DigitiserPhase::Setup) {
            return Some(match self.field {
                SetupField::Image => &mut self.image,
                SetupField::XScale => &mut self.x_scale,
                SetupField::XRange => &mut self.x_range,
                SetupField::YScale => &mut self.y_scale,
                SetupField::YRange => &mut self.y_range,
                SetupField::Figure => &mut self.figure,
                SetupField::XLabel => &mut self.x_label,
                SetupField::YLabel => &mut self.y_label,
                SetupField::Operator => &mut self.operator,
            });
        }
        if let DigitiserPhase::Review(review) = &mut self.phase {
            // Only the JSON path is edited in place here; the review screen
            // has no field cursor of its own beyond the point selection, so
            // `e` always targets the save path — the one thing worth
            // retyping in a terminal review session.
            return Some(&mut review.json_path);
        }
        None
    }

    fn begin_edit(&mut self, editing: &mut bool) {
        match self.focused_input_mut() {
            Some(input) => {
                self.edit_backup = input.value().to_string();
                *editing = true;
            }
            None => self.status = "nothing editable here".to_string(),
        }
    }

    /// Build the [`AutoArgs`] the Setup form describes, filling every
    /// non-form field with the same default `clap` would give it.
    fn build_auto_args(&self) -> Result<AutoArgs, String> {
        if self.image.value().trim().is_empty() {
            return Err("image path is required".to_string());
        }
        if self.figure.value().trim().is_empty() {
            return Err("figure designation is required".to_string());
        }
        if self.x_range.value().trim().is_empty() || self.y_range.value().trim().is_empty() {
            return Err(
                "x range and y range are both required (min,max off the frame edges)".to_string(),
            );
        }
        Ok(AutoArgs {
            image: self.image.value().trim().to_string(),
            x_scale: self.x_scale.value().trim().to_string(),
            y_scale: self.y_scale.value().trim().to_string(),
            x_range: Some(self.x_range.value().trim().to_string()),
            y_range: Some(self.y_range.value().trim().to_string()),
            x_ref: Vec::new(),
            y_ref: Vec::new(),
            figure: self.figure.value().trim().to_string(),
            document_id: None,
            document_title: None,
            page: None,
            notes: None,
            x_label: self.x_label.value().trim().to_string(),
            y_label: self.y_label.value().trim().to_string(),
            operator: self.operator.value().trim().to_string(),
            timestamp: None,
            strategy: "continuity".to_string(),
            step: 1,
            threshold: 128,
            curve_rgb: None,
            curve_tolerance: 60,
            inset: 3,
            max_column_fill: 0.6,
            dark_threshold: 128,
            min_line_fraction: 0.4,
        })
    }

    fn start_run(&mut self) {
        let args = match self.build_auto_args() {
            Ok(a) => a,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        self.status = format!("tracing {} …", args.image);
        self.phase = DigitiserPhase::Running(spawn_digitise(args));
    }

    fn abandon(&mut self) {
        self.status = match self.phase {
            DigitiserPhase::Running(_) => {
                "abandoned — the worker finishes in the background and its result is discarded"
                    .to_string()
            }
            DigitiserPhase::Review(_) => "review discarded — nothing was written".to_string(),
            _ => "back to setup".to_string(),
        };
        self.phase = DigitiserPhase::Setup;
    }

    /// Advance animation and collect a finished worker result. Returns
    /// `true` when the phase changed (caller repaints, matching the Ingest
    /// tab's [`super::ingest::IngestState::tick`]).
    pub fn tick(&mut self) -> bool {
        let mut next: Option<DigitiserPhase> = None;
        let mut next_status = String::new();

        if let DigitiserPhase::Running(job) = &mut self.phase {
            job.frame = job.frame.wrapping_add(1);
            match job.receiver.try_recv() {
                Ok(Ok((raster, dataset))) => {
                    let elapsed = job.started.elapsed();
                    next_status = format!(
                        "{} points traced in {:.1}s — verify the overlay, then v + S",
                        dataset.points.len(),
                        elapsed.as_secs_f64()
                    );
                    next = Some(DigitiserPhase::Review(ReviewState::new(
                        raster,
                        dataset,
                        self.operator.value().trim().to_string(),
                        &job.image,
                    )));
                }
                Ok(Err(message)) => {
                    next_status = "automatic pass failed".to_string();
                    next = Some(DigitiserPhase::Failed(FailureReport { message }));
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    next_status = "automatic pass failed".to_string();
                    next = Some(DigitiserPhase::Failed(FailureReport {
                        message: "the worker thread ended without returning a result".to_string(),
                    }));
                }
            }
        }

        match next {
            Some(phase) => {
                self.phase = phase;
                self.status = next_status;
                true
            }
            None => false,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, editing: &mut bool) {
        if *editing {
            match key.code {
                KeyCode::Enter => *editing = false,
                KeyCode::Esc => {
                    let backup = self.edit_backup.clone();
                    if let Some(input) = self.focused_input_mut() {
                        input.set(backup);
                    }
                    *editing = false;
                }
                KeyCode::Backspace => {
                    if let Some(input) = self.focused_input_mut() {
                        input.backspace();
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(input) = self.focused_input_mut() {
                        input.push_char(c);
                    }
                }
                _ => {}
            }
            return;
        }

        match self.phase {
            DigitiserPhase::Setup => self.handle_setup_key(key, editing),
            DigitiserPhase::Running(_) => {
                if key.code == KeyCode::Char('x') {
                    self.abandon();
                }
            }
            DigitiserPhase::Review(_) => self.handle_review_key(key, editing),
            DigitiserPhase::Failed(_) => {
                if matches!(key.code, KeyCode::Char('x') | KeyCode::Enter) {
                    self.abandon();
                }
            }
        }
    }

    fn handle_setup_key(&mut self, key: KeyEvent, editing: &mut bool) {
        match key.code {
            KeyCode::Down | KeyCode::Tab => self.field = self.field.step(1),
            KeyCode::Up | KeyCode::BackTab => self.field = self.field.step(-1),
            KeyCode::Char('e') => self.begin_edit(editing),
            KeyCode::Enter => self.start_run(),
            _ => {}
        }
    }

    fn handle_review_key(&mut self, key: KeyEvent, editing: &mut bool) {
        match key.code {
            KeyCode::Char('x') => self.abandon(),
            KeyCode::Char('e') => self.begin_edit(editing),
            KeyCode::Char('S') | KeyCode::Char('s') => {
                if let DigitiserPhase::Review(review) = &mut self.phase {
                    review.save();
                }
            }
            _ => {
                let DigitiserPhase::Review(review) = &mut self.phase else {
                    return;
                };
                let step = if key.modifiers.contains(KeyModifiers::SHIFT) {
                    5.0
                } else {
                    1.0
                };
                match key.code {
                    KeyCode::Right | KeyCode::Tab => review.select(1),
                    KeyCode::Left | KeyCode::BackTab => review.select(-1),
                    KeyCode::Up => review.nudge(0.0, -step),
                    KeyCode::Down => review.nudge(0.0, step),
                    KeyCode::Char('h') => review.nudge(-step, 0.0),
                    KeyCode::Char('l') => review.nudge(step, 0.0),
                    KeyCode::Char('d') => review.delete_selected(),
                    KeyCode::Char('a') => review.duplicate_selected(),
                    KeyCode::Char('v') => {
                        review.dataset.record_review(
                            review.operator.clone(),
                            utc_now_iso8601(),
                            ReviewInterface::Tui,
                        );
                        review.dirty = true;
                        review.message =
                            format!("marked reviewed by {} — save with S", review.operator);
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Spawn the worker thread that runs [`AutoArgs::run`]. Wrapped in
/// `catch_unwind` for the same reason the Ingest tab's extraction worker is:
/// the pipeline runs over untrusted image bytes, and a panic there must
/// surface as a `Failed` phase, not a dead process with the terminal left in
/// raw mode.
fn spawn_digitise(args: AutoArgs) -> RunningDigitise {
    let (sender, receiver) = std::sync::mpsc::channel();
    let image = args.image.clone();
    std::thread::spawn(move || {
        let result: Result<(PlotRaster, DigitisedDataset), DigitiserError> =
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| args.run())) {
                Ok(r) => r,
                Err(_) => Err(DigitiserError::Trace(
                    "the automatic pass panicked (the image is probably malformed)".to_string(),
                )),
            };
        let _ = sender.send(result.map_err(|e| e.to_string()));
    });
    RunningDigitise {
        image,
        started: Instant::now(),
        frame: 0,
        receiver,
    }
}

/// Render the tab: the Setup form, the Running spinner, the review screen
/// (image as ratatui half-blocks with a point overlay — unchanged from the
/// former standalone `kovan-digitise-tui` binary), or the failure message.
pub fn draw(f: &mut Frame, area: ratatui::layout::Rect, state: &mut DigitiserState, editing: bool) {
    match &state.phase {
        DigitiserPhase::Setup => draw_setup(f, area, state, editing),
        DigitiserPhase::Running(job) => draw_running(f, area, job),
        DigitiserPhase::Review(review) => draw_review(f, area, review),
        DigitiserPhase::Failed(failure) => draw_failed(f, area, failure),
    }
}

fn draw_setup(f: &mut Frame, area: ratatui::layout::Rect, state: &DigitiserState, editing: bool) {
    let mut lines: Vec<Line> = Vec::with_capacity(SETUP_FIELDS.len() + 2);
    for field in SETUP_FIELDS {
        let value = match field {
            SetupField::Image => state.image.value(),
            SetupField::XScale => state.x_scale.value(),
            SetupField::XRange => state.x_range.value(),
            SetupField::YScale => state.y_scale.value(),
            SetupField::YRange => state.y_range.value(),
            SetupField::Figure => state.figure.value(),
            SetupField::XLabel => state.x_label.value(),
            SetupField::YLabel => state.y_label.value(),
            SetupField::Operator => state.operator.value(),
        };
        let marker = if field == state.field {
            if editing {
                "> [editing] "
            } else {
                "> "
            }
        } else {
            "  "
        };
        lines.push(Line::from(format!("{marker}{}: {value}", field.label())));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(state.status.clone()));
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" digitiser setup — automatic pass ");
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_running(f: &mut Frame, area: ratatui::layout::Rect, job: &RunningDigitise) {
    let text = vec![Line::from(format!(
        "{} tracing {} — {:.1}s elapsed",
        job.spinner(),
        job.image,
        job.started.elapsed().as_secs_f64()
    ))];
    let block = Block::default().borders(Borders::ALL).title(" running ");
    f.render_widget(Paragraph::new(text).block(block), area);
}

fn draw_failed(f: &mut Frame, area: ratatui::layout::Rect, failure: &FailureReport) {
    let text = vec![Line::from(failure.message.clone())];
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" automatic pass failed ");
    f.render_widget(Paragraph::new(text).block(block), area);
}

fn draw_review(f: &mut Frame, area: ratatui::layout::Rect, review: &ReviewState) {
    let [image_area, status_area] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(6)]).areas(area);

    // --- image as half-blocks, nearest-neighbour fit ---
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", review.dataset.source.figure));
    let inner = block.inner(image_area);
    f.render_widget(block, image_area);
    let (iw, ih) = (review.raster.width(), review.raster.height());
    if inner.width > 0 && inner.height > 0 && iw > 0 && ih > 0 {
        let buf = f.buffer_mut();
        for cy in 0..inner.height {
            for cx in 0..inner.width {
                let sample = |sub: u32| -> Color {
                    let px = (cx as u64 * iw as u64 / inner.width as u64) as u32;
                    let py = (((cy as u64 * 2 + sub as u64) * ih as u64)
                        / (inner.height as u64 * 2)) as u32;
                    let [r, g, b] = review.raster.rgb(px.min(iw - 1), py.min(ih - 1));
                    Color::Rgb(r, g, b)
                };
                let cell = &mut buf[(inner.x + cx, inner.y + cy)];
                cell.set_symbol("▀");
                cell.set_fg(sample(0));
                cell.set_bg(sample(1));
            }
        }
        // --- point overlay ---
        for (i, p) in review.dataset.points.iter().enumerate() {
            let (Some(x_px), Some(y_px)) = (p.x_px, p.y_px) else {
                continue;
            };
            let cx = (x_px * inner.width as f64 / iw as f64) as u16;
            let cy = (y_px * inner.height as f64 / ih as f64) as u16;
            if cx < inner.width && cy < inner.height {
                let cell = &mut buf[(inner.x + cx, inner.y + cy)];
                if i == review.selected {
                    cell.set_symbol("X");
                    cell.set_fg(Color::Yellow);
                    cell.set_bg(Color::Black);
                } else {
                    cell.set_symbol("o");
                    cell.set_fg(Color::Red);
                }
            }
        }
    }

    // --- status + help ---
    let review_status = match &review.dataset.review {
        ReviewStatus::Unreviewed => "UNREVIEWED".to_string(),
        ReviewStatus::Reviewed { by, at, .. } => format!("reviewed by {by} at {at}"),
    };
    let point_line = match review.dataset.points.get(review.selected) {
        Some(p) => format!(
            "point {}/{}: x = {:.6e} (-{:.1e}/+{:.1e})  y = {:.6e} (-{:.1e}/+{:.1e})  [{}]",
            review.selected + 1,
            review.dataset.points.len(),
            p.x,
            p.x_minus,
            p.x_plus,
            p.y,
            p.y_minus,
            p.y_plus,
            match &p.origin {
                PointOrigin::AutoTraced => "auto".to_string(),
                PointOrigin::HandPlaced { by } => format!("hand-placed by {by}"),
                PointOrigin::HandCorrected { by } => format!("hand-corrected by {by}"),
            },
        ),
        None => "no points".to_string(),
    };
    let status = Paragraph::new(vec![
        Line::from(point_line),
        Line::from(format!(
            "{review_status}{}   save: {}",
            if review.dirty { "  [unsaved]" } else { "" },
            review.json_path.value()
        )),
        Line::from(review.message.clone()),
    ])
    .style(Style::default())
    .block(Block::default().borders(Borders::ALL).title(" review "));
    f.render_widget(status, status_area);
}
