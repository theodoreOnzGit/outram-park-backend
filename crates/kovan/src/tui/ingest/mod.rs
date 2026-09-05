//! Ingest tab — interactive literature ingestion (`kovan-literature`).
//!
//! This is the TUI equivalent of `kovan lit import <PDF> [--json-out <p>]
//! [--markdown-out <p>]`, and it calls exactly the same library entry points the
//! CLI does ([`kovan_literature::extract_metadata`] and
//! [`kovan_literature::to_bibtex`]) — no reimplementation of the pipeline.
//!
//! # The flow
//!
//! ```text
//! Picking ──Enter──▶ Running ──ok──▶ Review ──s──▶ (files written)
//!    ▲                  │              │
//!    └──────x───────────┴──error──▶ Failed ──x──┘
//! ```
//!
//! 1. **Picking** — browse a directory for PDFs (the same `.gitignore`-aware
//!    walk the Browser tab uses, `kovan_discovery::discover_kind`), narrowed by
//!    a substring filter, so no absolute path has to be typed by hand.
//! 2. **Running** — extraction runs on a worker thread; the draw loop keeps
//!    running and shows elapsed time. See "Why a thread" below.
//! 3. **Review** — the extracted metadata is shown as an editable form with
//!    advisories, because `extract_metadata` is best-effort (see
//!    [`review`]'s module docs for the real-world failure that motivated it).
//! 4. **Save** — writes Markdown, `KovanDocument` JSON, and BibTeX to the chosen
//!    paths, from the **corrected** record.
//!
//! # Why a thread (and why a channel is right here)
//!
//! `extract_metadata` is one opaque, unbounded, blocking call over a
//! user-supplied file. Running it on the draw-loop thread would freeze the UI
//! for however long it takes, with no indication that anything is happening —
//! exactly the failure this tab exists to avoid. So the call is moved to a
//! worker thread and its one result comes back over a `std::sync::mpsc` channel.
//!
//! Measured cost (release build, developer desktop, 2026-08-05): a 12 MB /
//! 447-page scanned report extracted in **0.3 s**, a 1.4 MB / 103-page one in
//! 0.1 s. Faster than assumed — but the cost is a property of the file and the
//! machine (a debug build, an Android device, or a pathological PDF are all far
//! slower), and it is unbounded in principle, so the UI must not depend on it
//! being quick.
//!
//! The workspace rule "no channels for simulation state, use `Arc<RwLock<T>>`"
//! (root `CLAUDE.md`, "Shared state") is about threads computing over *shared
//! mutable* fields in a timestep loop. This is the other pattern that rule
//! contrasts with: a produce-once/consume-once pipeline with no shared state at
//! all — the worker owns its `PathBuf`, the UI owns the result, and nothing is
//! ever mutated from two threads. A lock here would add ceremony and no safety.
//!
//! Honesty about progress: `kovan-literature` exposes no progress callback, so
//! this tab shows **elapsed time and a liveness spinner, never a fabricated
//! percentage**.
//!
//! # Robustness
//!
//! - The worker wraps the library call in [`std::panic::catch_unwind`], so a
//!   panic inside PDF parsing becomes a normal `Failed` phase rather than
//!   unwinding the process and leaving the terminal in raw mode.
//! - A worker that dies without sending (channel disconnect) is reported too.
//! - Every save error is reported in-pane; nothing here panics or `unwrap`s on
//!   user-supplied paths.
//!
//! This is the one tab that **writes** files, and only when the user presses
//! `s` on paths they can see and edit.

mod draw;
mod metadata;
mod review;

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Instant;

use kovan_common::KovanDocument;
use kovan_discovery::{discover_kind, FileKind};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::ListState;

pub use draw::draw;
pub use review::{ReviewField, ReviewState};

use super::text_input::TextInput;

/// Frames of the liveness spinner shown while extraction runs.
const SPINNER: [char; 4] = ['|', '/', '-', '\\'];

/// Which picker field the keyboard types into while editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerField {
    /// The directory that is walked for PDFs.
    Root,
    /// A case-insensitive substring the PDF path must contain.
    Filter,
}

/// A running extraction: the worker thread's handle to its result plus what the
/// UI needs to show that work is happening.
pub struct RunningJob {
    /// The PDF being extracted.
    pub pdf: PathBuf,
    /// Size of the source PDF in bytes (0 if it could not be stat'd) — shown so
    /// a long wait on a large scan is understandable rather than alarming.
    pub bytes: u64,
    /// When the worker was spawned; drives the elapsed-time display.
    pub started: Instant,
    /// Spinner frame counter, advanced once per [`IngestState::tick`].
    pub frame: usize,
    /// One-shot result channel from the worker thread.
    receiver: Receiver<Result<KovanDocument, String>>,
}

impl RunningJob {
    /// The current spinner character.
    pub fn spinner(&self) -> char {
        SPINNER[self.frame % SPINNER.len()]
    }
}

/// A failed extraction, kept so the message stays on screen until dismissed.
pub struct FailureReport {
    /// The PDF that failed.
    pub pdf: PathBuf,
    /// What went wrong, as reported by `kovan-literature` (or by the worker
    /// wrapper for a panic / dead thread).
    pub message: String,
}

/// The tab's state machine. Enum dispatch, no trait objects — every phase is a
/// known variant and every `match` over it is exhaustive.
///
/// The `Review` variant is much larger than the others (it carries the whole
/// extracted [`KovanDocument`]). Clippy's usual remedy — boxing the large field
/// — is not available here: the workspace forbids `Box<T>` (root `CLAUDE.md`,
/// "Rust design rules"), and it would buy nothing, since exactly one
/// `IngestPhase` exists per running program.
#[allow(clippy::large_enum_variant)]
pub enum IngestPhase {
    /// Choosing a PDF to import.
    Picking,
    /// Extraction is running on a worker thread.
    Running(RunningJob),
    /// Extraction finished; the metadata is under human review.
    Review(ReviewState),
    /// Extraction failed; showing why.
    Failed(FailureReport),
}

/// State for the Ingest tab.
pub struct IngestState {
    /// Directory walked for candidate PDFs.
    pub root: TextInput,
    /// Case-insensitive substring filter over the discovered paths.
    pub filter: TextInput,
    /// PDFs found by the last scan, after filtering.
    pub candidates: Vec<PathBuf>,
    /// Selection into [`IngestState::candidates`].
    pub list_state: ListState,
    /// One-line status message for the header.
    pub status: String,
    /// Current phase.
    pub phase: IngestPhase,
    /// Which picker field `e`/`f` edits.
    pub picker_field: PickerField,
    /// Value of the focused field when the current edit began, restored on Esc.
    edit_backup: String,
}

impl Default for IngestState {
    fn default() -> Self {
        Self {
            root: TextInput::new("."),
            filter: TextInput::default(),
            candidates: Vec::new(),
            list_state: ListState::default(),
            status: "'e' root, 'f' filter, 'r' scan, Enter imports the selected PDF".to_string(),
            phase: IngestPhase::Picking,
            picker_field: PickerField::Root,
            edit_backup: String::new(),
        }
    }
}

impl IngestState {
    /// `true` while an extraction is running — the draw loop polls faster then,
    /// so the elapsed time and spinner stay live.
    pub fn is_busy(&self) -> bool {
        matches!(self.phase, IngestPhase::Running(_))
    }

    /// `true` when a global `q`/`Esc` would throw away work in progress (a
    /// running extraction, or an unsaved review). [`super::App`] uses this to
    /// require an explicit `x` first, so a reflexive `q` cannot silently discard
    /// a corrected record.
    pub fn blocks_quit(&self) -> bool {
        matches!(self.phase, IngestPhase::Running(_) | IngestPhase::Review(_))
    }

    /// The key-binding help line for the current phase, shown in the app's
    /// footer. Phase-specific because the bindings genuinely differ between
    /// picking a file, waiting, and reviewing metadata.
    pub fn help_line(&self) -> &'static str {
        match self.phase {
            IngestPhase::Picking => {
                "e: directory  f: filter  r: scan  Up/Down: select  Enter: import  1-6: tabs  q: quit"
            }
            IngestPhase::Running(_) => "extracting… x: abandon (q/Esc will not quit while running)",
            IngestPhase::Review(_) => {
                "Up/Down: field  e: edit  Left/Right: type  s: save  x: discard  PgUp/PgDn: scroll"
            }
            IngestPhase::Failed(_) => "x / Enter: back to the picker  1-6: tabs  q: quit",
        }
    }

    /// Message shown when a quit was blocked by [`IngestState::blocks_quit`].
    pub fn note_blocked_quit(&mut self) {
        self.status = match self.phase {
            IngestPhase::Running(_) => {
                "extraction still running — press 'x' to abandon it, then 'q'".to_string()
            }
            _ => "review not saved — press 's' to save or 'x' to discard, then 'q'".to_string(),
        };
    }

    /// Re-run PDF discovery under the current root and filter.
    ///
    /// Uses the same `.gitignore`-aware walk as the Browser tab
    /// ([`kovan_discovery::discover_kind`] with [`FileKind::Pdf`]). A missing
    /// root is reported in the status line rather than shown as "0 files".
    pub fn run_scan(&mut self) {
        let root = PathBuf::from(self.root.value());
        if !root.exists() {
            self.candidates.clear();
            self.list_state.select(None);
            self.status = format!("root does not exist: {}", root.display());
            return;
        }
        let needle = self.filter.value().trim().to_ascii_lowercase();
        let mut found: Vec<PathBuf> = discover_kind(&root, FileKind::Pdf)
            .into_iter()
            .filter(|p| {
                needle.is_empty() || p.to_string_lossy().to_ascii_lowercase().contains(&needle)
            })
            .collect();
        found.sort();
        self.candidates = found;
        self.status = if self.filter.value().trim().is_empty() {
            format!("{} PDF(s) found", self.candidates.len())
        } else {
            format!(
                "{} PDF(s) matching '{}'",
                self.candidates.len(),
                self.filter.value().trim()
            )
        };
        self.list_state.select(if self.candidates.is_empty() {
            None
        } else {
            Some(0)
        });
    }

    /// The currently selected candidate PDF, if any.
    pub fn selected_pdf(&self) -> Option<PathBuf> {
        self.list_state
            .selected()
            .and_then(|i| self.candidates.get(i))
            .cloned()
    }

    /// Start extracting `pdf` on a worker thread and switch to the Running
    /// phase. Any previous phase (a finished review, a failure) is replaced.
    pub fn start_extraction(&mut self, pdf: PathBuf) {
        let bytes = std::fs::metadata(&pdf).map(|m| m.len()).unwrap_or(0);
        self.status = format!("extracting {} …", pdf.display());
        self.phase = IngestPhase::Running(spawn_extraction(pdf, bytes));
    }

    /// Hand-off entry point used by the Literature tab's `i` key: point the
    /// picker at the PDF's directory, select it, and start the import straight
    /// away (the user already chose the file over there).
    pub fn ingest_path(&mut self, pdf: PathBuf) {
        if let Some(parent) = pdf.parent() {
            if !parent.as_os_str().is_empty() {
                self.root.set(parent.to_string_lossy().as_ref());
            }
        }
        self.filter.clear();
        self.run_scan();
        if let Some(i) = self.candidates.iter().position(|p| *p == pdf) {
            self.list_state.select(Some(i));
        }
        self.start_extraction(pdf);
    }

    /// Advance animation and collect a finished worker result.
    ///
    /// Called once per draw-loop iteration. Returns `true` when the phase
    /// changed, which the caller uses to clear the terminal — a worker panic
    /// prints to stderr through the default hook and can smear the frame, so a
    /// full repaint on every transition out of Running keeps the screen clean.
    pub fn tick(&mut self) -> bool {
        let mut next: Option<IngestPhase> = None;
        let mut next_status = String::new();

        if let IngestPhase::Running(job) = &mut self.phase {
            job.frame = job.frame.wrapping_add(1);
            match job.receiver.try_recv() {
                Ok(Ok(doc)) => {
                    let elapsed = job.started.elapsed();
                    next_status = format!(
                        "extracted in {:.1}s — review the metadata before saving",
                        elapsed.as_secs_f64()
                    );
                    next = Some(IngestPhase::Review(ReviewState::new(
                        job.pdf.clone(),
                        doc,
                        elapsed,
                    )));
                }
                Ok(Err(message)) => {
                    next_status = "extraction failed".to_string();
                    next = Some(IngestPhase::Failed(FailureReport {
                        pdf: job.pdf.clone(),
                        message,
                    }));
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    next_status = "extraction failed".to_string();
                    next = Some(IngestPhase::Failed(FailureReport {
                        pdf: job.pdf.clone(),
                        message: "the extraction thread ended without returning a result"
                            .to_string(),
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

    /// The text field the keyboard types into, given the current phase and
    /// focus. `None` when the focused row is not typed (the cycled document
    /// type) or the phase has no fields (Running/Failed).
    fn focused_input_mut(&mut self) -> Option<&mut TextInput> {
        if matches!(self.phase, IngestPhase::Picking) {
            return Some(match self.picker_field {
                PickerField::Root => &mut self.root,
                PickerField::Filter => &mut self.filter,
            });
        }
        if let IngestPhase::Review(review) = &mut self.phase {
            return review.focused_input_mut();
        }
        None
    }

    /// Begin editing the focused field, remembering its value so `Esc` can put
    /// it back. No-op (with an explanatory status) when the focused row is not
    /// editable.
    fn begin_edit(&mut self, editing: &mut bool) {
        match self.focused_input_mut() {
            Some(input) => {
                self.edit_backup = input.value().to_string();
                *editing = true;
            }
            None => {
                self.status = "this field is chosen with Left/Right, not typed".to_string();
            }
        }
    }

    /// Finish an edit with `Enter`: rescan in the picker, or re-derive the
    /// output paths in the review form.
    fn commit_edit(&mut self, editing: &mut bool) {
        *editing = false;
        if matches!(self.phase, IngestPhase::Picking) {
            self.run_scan();
            return;
        }
        if let IngestPhase::Review(review) = &mut self.phase {
            match review.field {
                // Hand-editing an output path pins all three: the user has taken
                // over, so slug changes must stop moving their files.
                ReviewField::MarkdownOut | ReviewField::JsonOut | ReviewField::BibtexOut => {
                    review.outputs_pinned = true;
                }
                _ => review.refresh_output_defaults(),
            }
        }
    }

    /// Abandon whatever is in flight and return to the picker.
    ///
    /// A running extraction cannot be interrupted — `extract_metadata` is one
    /// blocking library call with no cancellation token — so "abandon" means the
    /// receiver is dropped and the worker's eventual result is discarded. The
    /// thread finishes on its own; nothing leaks but the CPU time already spent.
    fn abandon(&mut self) {
        self.status = match self.phase {
            IngestPhase::Running(_) => {
                "abandoned — the worker finishes in the background and its result is discarded"
                    .to_string()
            }
            IngestPhase::Review(_) => "review discarded — nothing was written".to_string(),
            _ => "back to the picker".to_string(),
        };
        self.phase = IngestPhase::Picking;
    }

    fn select_next(&mut self) {
        if self.candidates.is_empty() {
            return;
        }
        let i = self
            .list_state
            .selected()
            .map(|i| (i + 1) % self.candidates.len())
            .unwrap_or(0);
        self.list_state.select(Some(i));
    }

    fn select_prev(&mut self) {
        if self.candidates.is_empty() {
            return;
        }
        let i = self
            .list_state
            .selected()
            .map(|i| {
                if i == 0 {
                    self.candidates.len() - 1
                } else {
                    i - 1
                }
            })
            .unwrap_or(0);
        self.list_state.select(Some(i));
    }

    /// Handle one key event. `editing` is the shared edit-mode flag owned by
    /// [`super::App`]; while it is `true` every key types into the focused field
    /// instead of navigating.
    ///
    /// Pure state mutation plus the two explicit user-triggered actions (scan a
    /// directory, write the chosen files) — the extraction itself is handed to a
    /// worker thread, so no key press ever blocks the draw loop.
    pub fn handle_key(&mut self, key: KeyEvent, editing: &mut bool) {
        if *editing {
            match key.code {
                KeyCode::Enter => self.commit_edit(editing),
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

        // Dispatched on the phase discriminant (rather than a `match` that holds
        // a mutable borrow of `self.phase`) so each handler can freely call the
        // whole-state helpers: `begin_edit`, `abandon`, `start_extraction`.
        match self.phase {
            IngestPhase::Picking => self.handle_picking_key(key, editing),
            IngestPhase::Running(_) => {
                if key.code == KeyCode::Char('x') {
                    self.abandon();
                }
            }
            IngestPhase::Review(_) => self.handle_review_key(key, editing),
            IngestPhase::Failed(_) => {
                if matches!(key.code, KeyCode::Char('x') | KeyCode::Enter) {
                    self.abandon();
                }
            }
        }
    }

    /// Keys for the PDF picker: edit the root/filter, scan, move the selection,
    /// and start an import.
    fn handle_picking_key(&mut self, key: KeyEvent, editing: &mut bool) {
        match key.code {
            KeyCode::Char('e') => {
                self.picker_field = PickerField::Root;
                self.begin_edit(editing);
            }
            KeyCode::Char('f') => {
                self.picker_field = PickerField::Filter;
                self.begin_edit(editing);
            }
            KeyCode::Char('r') => self.run_scan(),
            KeyCode::Down => self.select_next(),
            KeyCode::Up => self.select_prev(),
            KeyCode::Enter => match self.selected_pdf() {
                Some(pdf) => self.start_extraction(pdf),
                None => {
                    self.status =
                        "no PDF selected — press 'r' to scan, Up/Down to choose".to_string()
                }
            },
            _ => {}
        }
    }

    /// Keys for the metadata review form: move between rows, edit a row, cycle
    /// the document type, save, or discard.
    fn handle_review_key(&mut self, key: KeyEvent, editing: &mut bool) {
        match key.code {
            KeyCode::Char('e') | KeyCode::Enter => self.begin_edit(editing),
            KeyCode::Char('x') => self.abandon(),
            KeyCode::Char('s') => {
                let saved = match &mut self.phase {
                    IngestPhase::Review(review) => review.save(),
                    _ => false,
                };
                self.status = if saved {
                    "saved — see the report pane for the exact paths".to_string()
                } else {
                    "nothing written — see the report pane".to_string()
                };
            }
            _ => {
                if let IngestPhase::Review(review) = &mut self.phase {
                    match key.code {
                        KeyCode::Down => review.field = review.field.step(1),
                        KeyCode::Up => review.field = review.field.step(-1),
                        KeyCode::Left | KeyCode::Right => {
                            if review.field == ReviewField::DocType {
                                let delta = if key.code == KeyCode::Left { -1 } else { 1 };
                                review.document_type =
                                    review::step_document_type(review.document_type, delta);
                                review.refresh_output_defaults();
                            }
                        }
                        KeyCode::PageDown => {
                            review.preview_scroll = review.preview_scroll.saturating_add(5)
                        }
                        KeyCode::PageUp => {
                            review.preview_scroll = review.preview_scroll.saturating_sub(5)
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Spawn the worker thread that runs `kovan_literature::extract_metadata`.
///
/// The library call is wrapped in [`std::panic::catch_unwind`] because PDF
/// parsing runs over untrusted third-party bytes: a panic there must surface as
/// a `Failed` phase in the UI, not as a dead process with the terminal left in
/// raw mode. The thread is detached — abandoning a job simply drops the
/// receiving end, and the worker's `send` then fails harmlessly.
fn spawn_extraction(pdf: PathBuf, bytes: u64) -> RunningJob {
    let (sender, receiver) = std::sync::mpsc::channel();
    let worker_pdf = pdf.clone();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            kovan_literature::extract_metadata(&worker_pdf).map_err(|e| e.to_string())
        }))
        .unwrap_or_else(|_| {
            Err(
                "PDF extraction panicked inside kovan-literature (the file is probably malformed)"
                    .to_string(),
            )
        });
        // A dropped receiver means the user abandoned the job; discarding the
        // result is the intended behaviour, not an error.
        let _ = sender.send(result);
    });
    RunningJob {
        pdf,
        bytes,
        started: Instant::now(),
        frame: 0,
        receiver,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;
    use std::time::{Duration, Instant as StdInstant};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// A directory with two PDF-named files (contents are not valid PDFs — the
    /// picker only walks and filters, it never parses).
    fn pdf_tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("reports")).unwrap();
        std::fs::write(dir.path().join("reports/anl-7416.pdf"), b"not a real pdf").unwrap();
        std::fs::write(dir.path().join("neutron-benchmarks.pdf"), b"not a real pdf").unwrap();
        std::fs::write(dir.path().join("notes.md"), b"# not a pdf").unwrap();
        dir
    }

    fn picker_at(dir: &tempfile::TempDir) -> IngestState {
        let mut state = IngestState::default();
        state.root.set(dir.path().to_str().expect("utf8 path"));
        state.run_scan();
        state
    }

    /// Block until the worker finishes, ticking as the draw loop would.
    /// Returns `false` if it never finished within the timeout.
    fn drain(state: &mut IngestState, timeout: Duration) -> bool {
        let start = StdInstant::now();
        while start.elapsed() < timeout {
            if state.tick() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    #[test]
    fn scan_finds_pdfs_only() {
        let dir = pdf_tree();
        let state = picker_at(&dir);
        assert_eq!(state.candidates.len(), 2, "{:?}", state.candidates);
        assert!(state
            .candidates
            .iter()
            .all(|p| p.extension().unwrap() == "pdf"));
        assert_eq!(state.list_state.selected(), Some(0));
    }

    #[test]
    fn filter_narrows_the_candidate_list_case_insensitively() {
        let dir = pdf_tree();
        let mut state = picker_at(&dir);
        state.filter.set("ANL");
        state.run_scan();
        assert_eq!(state.candidates.len(), 1);
        assert!(state.candidates[0].ends_with("anl-7416.pdf"));
        assert!(state.status.contains("matching"));
    }

    #[test]
    fn missing_root_is_reported_not_panicked() {
        let mut state = IngestState::default();
        state.root.set("/no/such/kovan-ingest-dir");
        state.run_scan();
        assert!(state.candidates.is_empty());
        assert!(state.status.contains("does not exist"));
    }

    #[test]
    fn typing_a_root_then_enter_scans_it() {
        let dir = pdf_tree();
        let mut state = IngestState::default();
        let mut editing = false;
        state.handle_key(key(KeyCode::Char('e')), &mut editing);
        assert!(editing);
        state.root.clear();
        for c in dir.path().to_str().unwrap().chars() {
            state.handle_key(key(KeyCode::Char(c)), &mut editing);
        }
        state.handle_key(key(KeyCode::Enter), &mut editing);
        assert!(!editing);
        assert_eq!(state.candidates.len(), 2);
    }

    #[test]
    fn esc_cancels_an_edit_and_restores_the_previous_value() {
        let mut state = IngestState::default();
        let mut editing = false;
        state.root.set("/original");
        state.handle_key(key(KeyCode::Char('e')), &mut editing);
        state.handle_key(key(KeyCode::Char('x')), &mut editing);
        assert_eq!(state.root.value(), "/originalx");
        state.handle_key(key(KeyCode::Esc), &mut editing);
        assert!(!editing);
        assert_eq!(state.root.value(), "/original");
    }

    #[test]
    fn enter_with_no_selection_reports_instead_of_starting_a_job() {
        let mut state = IngestState::default();
        let mut editing = false;
        state.handle_key(key(KeyCode::Enter), &mut editing);
        assert!(matches!(state.phase, IngestPhase::Picking));
        assert!(state.status.contains("no PDF selected"));
    }

    #[test]
    fn a_bad_pdf_ends_in_the_failed_phase_with_a_message() {
        let dir = pdf_tree();
        let mut state = picker_at(&dir);
        let mut editing = false;
        state.handle_key(key(KeyCode::Enter), &mut editing);
        assert!(matches!(state.phase, IngestPhase::Running(_)));
        assert!(state.is_busy());

        assert!(
            drain(&mut state, Duration::from_secs(20)),
            "worker must report back"
        );
        match &state.phase {
            IngestPhase::Failed(report) => {
                assert!(!report.message.is_empty());
                assert!(report.pdf.extension().unwrap() == "pdf");
            }
            _ => panic!("expected Failed for a non-PDF payload"),
        }
        assert!(!state.is_busy());
    }

    #[test]
    fn a_failed_job_is_dismissed_back_to_the_picker() {
        let dir = pdf_tree();
        let mut state = picker_at(&dir);
        let mut editing = false;
        state.handle_key(key(KeyCode::Enter), &mut editing);
        assert!(drain(&mut state, Duration::from_secs(20)));
        state.handle_key(key(KeyCode::Char('x')), &mut editing);
        assert!(matches!(state.phase, IngestPhase::Picking));
    }

    #[test]
    fn abandoning_a_running_job_returns_to_the_picker_immediately() {
        let dir = pdf_tree();
        let mut state = picker_at(&dir);
        let mut editing = false;
        state.handle_key(key(KeyCode::Enter), &mut editing);
        state.handle_key(key(KeyCode::Char('x')), &mut editing);
        assert!(matches!(state.phase, IngestPhase::Picking));
        assert!(state.status.contains("abandoned"));
    }

    #[test]
    fn tick_on_an_idle_picker_does_nothing() {
        let mut state = IngestState::default();
        assert!(!state.tick());
        assert!(matches!(state.phase, IngestPhase::Picking));
    }

    #[test]
    fn ingest_path_points_the_picker_at_the_files_directory() {
        let dir = pdf_tree();
        let pdf = dir.path().join("neutron-benchmarks.pdf");
        let mut state = IngestState::default();
        state.ingest_path(pdf.clone());
        assert_eq!(state.root.value(), dir.path().to_str().unwrap());
        assert_eq!(state.selected_pdf(), Some(pdf));
        assert!(matches!(state.phase, IngestPhase::Running(_)));
        // Do not leave a worker racing the test harness.
        assert!(drain(&mut state, Duration::from_secs(20)));
    }

    #[test]
    fn quit_is_blocked_while_work_is_in_flight() {
        let dir = pdf_tree();
        let mut state = picker_at(&dir);
        assert!(!state.blocks_quit(), "the picker never blocks quitting");
        let mut editing = false;
        state.handle_key(key(KeyCode::Enter), &mut editing);
        assert!(state.blocks_quit());
        state.note_blocked_quit();
        assert!(state.status.contains('x'));
        state.handle_key(key(KeyCode::Char('x')), &mut editing);
        assert!(!state.blocks_quit());
    }

    /// Drive the review phase without needing a real PDF: build the document the
    /// worker would have produced and enter the phase directly.
    fn state_in_review() -> IngestState {
        use kovan_common::{DocumentType, Visibility};
        let mut doc = KovanDocument::new(
            "kovan-1",
            "2004anl7416",
            Visibility::Open,
            DocumentType::Other,
            "ANL-7416 Supplement 2",
        );
        doc.year = Some(2004);
        doc.markdown_body = "Argonne Code Center, June 1977".to_string();
        IngestState {
            phase: IngestPhase::Review(ReviewState::new(
                PathBuf::from("/tmp/anl.pdf"),
                doc,
                Duration::from_secs(3),
            )),
            ..Default::default()
        }
    }

    fn review_of(state: &IngestState) -> &ReviewState {
        match &state.phase {
            IngestPhase::Review(r) => r,
            _ => panic!("expected the review phase"),
        }
    }

    #[test]
    fn review_navigation_moves_between_fields() {
        let mut state = state_in_review();
        let mut editing = false;
        assert_eq!(review_of(&state).field, ReviewField::Title);
        state.handle_key(key(KeyCode::Down), &mut editing);
        assert_eq!(review_of(&state).field, ReviewField::Authors);
        state.handle_key(key(KeyCode::Up), &mut editing);
        assert_eq!(review_of(&state).field, ReviewField::Title);
    }

    #[test]
    fn typing_into_the_year_field_corrects_the_record() {
        let mut state = state_in_review();
        let mut editing = false;
        state.handle_key(key(KeyCode::Down), &mut editing); // Authors
        state.handle_key(key(KeyCode::Char('e')), &mut editing);
        for c in "Argonne Code Center".chars() {
            state.handle_key(key(KeyCode::Char(c)), &mut editing);
        }
        state.handle_key(key(KeyCode::Enter), &mut editing);
        state.handle_key(key(KeyCode::Down), &mut editing); // Year
        state.handle_key(key(KeyCode::Char('e')), &mut editing);
        for _ in 0..4 {
            state.handle_key(key(KeyCode::Backspace), &mut editing);
        }
        for c in "1977".chars() {
            state.handle_key(key(KeyCode::Char(c)), &mut editing);
        }
        state.handle_key(key(KeyCode::Enter), &mut editing);

        let doc = review_of(&state)
            .corrected_document()
            .expect("form is valid");
        assert_eq!(doc.year, Some(1977));
        assert_eq!(doc.slug, "argonnecodecenter1977anl7416");
    }

    #[test]
    fn left_right_cycles_the_document_type_only_on_that_row() {
        let mut state = state_in_review();
        let mut editing = false;
        state.handle_key(key(KeyCode::Right), &mut editing);
        assert_eq!(
            review_of(&state).document_type,
            kovan_common::DocumentType::Other,
            "Left/Right on the Title row must not change the type"
        );
        for _ in 0..3 {
            state.handle_key(key(KeyCode::Down), &mut editing);
        }
        assert_eq!(review_of(&state).field, ReviewField::DocType);
        state.handle_key(key(KeyCode::Right), &mut editing);
        assert_eq!(
            review_of(&state).document_type,
            kovan_common::DocumentType::Paper
        );
    }

    #[test]
    fn pressing_e_on_the_type_row_explains_itself_instead_of_editing() {
        let mut state = state_in_review();
        let mut editing = false;
        for _ in 0..3 {
            state.handle_key(key(KeyCode::Down), &mut editing);
        }
        state.handle_key(key(KeyCode::Char('e')), &mut editing);
        assert!(!editing);
        assert!(state.status.contains("Left/Right"));
    }

    #[test]
    fn editing_an_output_path_pins_all_three() {
        let mut state = state_in_review();
        let mut editing = false;
        for _ in 0..5 {
            state.handle_key(key(KeyCode::Down), &mut editing);
        }
        assert_eq!(review_of(&state).field, ReviewField::MarkdownOut);
        state.handle_key(key(KeyCode::Char('e')), &mut editing);
        state.handle_key(key(KeyCode::Char('!')), &mut editing);
        state.handle_key(key(KeyCode::Enter), &mut editing);
        assert!(review_of(&state).outputs_pinned);
    }

    #[test]
    fn saving_from_the_review_phase_writes_and_reports() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut state = state_in_review();
        let mut editing = false;
        if let IngestPhase::Review(review) = &mut state.phase {
            review.outputs_pinned = true;
            review
                .markdown_out
                .set(dir.path().join("doc.md").to_string_lossy().as_ref());
            review
                .json_out
                .set(dir.path().join("doc.json").to_string_lossy().as_ref());
            review
                .bibtex_out
                .set(dir.path().join("doc.bib").to_string_lossy().as_ref());
        }
        state.handle_key(key(KeyCode::Char('s')), &mut editing);
        assert!(state.status.contains("saved"), "{}", state.status);
        assert!(dir.path().join("doc.bib").exists());
        assert!(!review_of(&state).save_report.is_empty());
    }

    #[test]
    fn discarding_a_review_writes_nothing_and_returns_to_the_picker() {
        let mut state = state_in_review();
        let mut editing = false;
        state.handle_key(key(KeyCode::Char('x')), &mut editing);
        assert!(matches!(state.phase, IngestPhase::Picking));
        assert!(state.status.contains("discarded"));
    }
}
