//! The **Code Review** view (GitHub #820): `kovan-web`'s Code Review UI
//! embedded in desktop kovan, over the workspace the Code Map view shows.
//!
//! ```text
//!  code map ─► crate (module tree) ─► module (function ring) ─► function:
//!                                     source panel and review bar
//! ```
//!
//! It is the same egui UI the site publishes
//! ([`kovan_web::ui::CodeReview`]), in [`kovan_web::Mode::Desktop`], reading
//! a data folder [`crate::code_review_data`] builds from the SCIP index
//! "Index fresh" leaves in the workspace, and source files from the
//! checkout. **Nothing is picked by hand**: opening the view, or finishing
//! an "Index fresh", checks the data and builds or rebuilds it when the
//! index is newer.
//!
//! ~~**Stamping: the hand-off is wired, the dialog is not.**~~ **CORRECTED
//! 2026-10-10** (#770): **Stamping is wired end to end.** The review bar's
//! Stamp and Needs fix (enabled in `Mode::Desktop`) queue a
//! [`kovan_web::ui::HostRequest`] naming the function
//! ([`kovan_web::ui::FunctionRef`]); this view takes it after each frame and
//! opens the stamp dialog over it (`super::stamp_dialog`, whose logic is
//! [`crate::stamping::flow::StampFlow`]): key set-up, the review wizard,
//! sign and write `review.md`, or a needs-fix. After a write the flow
//! recomputes the states from `review.md`, and
//! [`CodeReviewView::set_stamps`] recolours the bar and the cards without
//! reloading the data.
//!
//! **The ⚑ need-you queue, recently reviewed and commit-and-push**
//! (GitHub #771): the strip's "⚑ need you" button opens a right panel
//! (`super::need_you_view`) whose rows navigate the embedded UI to their
//! function ([`kovan_web::ui::CodeReview::navigate`]). Every file kovan
//! writes in this session (the stamp dialog's `wrote`, acknowledged moves,
//! recorded deletions) is collected in `session`; Commit and push, in the
//! panel and on the dialog's last step, commits only those
//! (`crate::stamping::commit_push`), never to `main`.
//!
//! **Review mode (2026-10-10, #770, #740).** The strip's "Review >" and
//! "Review this walk >" act on the function selected in the embedded UI
//! ([`kovan_web::ui::CodeReview::selected_function`]); review mode
//! (`super::review_mode_panel`, logic in
//! [`crate::stamping::review_mode`]) is then drawn in place of the map,
//! and its Stamp / Needs fix open the same stamp dialog.
//!
//! **Stamp states come from `review.md`.** Once the data is loaded, the
//! view recomputes the data folder's `stamps` with
//! [`crate::stamping::stamp_states`] on a worker (~~the legacy
//! `review/stamps.toml` path of `kovan-cli call-graph --split-dir`~~
//! **CORRECTED 2026-10-10**: `--split-dir` writes the same `review.md`
//! states since 1c2319ffda, and `stamps.toml` is gone, #825), so the
//! view stays current after each stamp.
//!
//! **Refreshes never block stamping (GitHub #1018, maintainer 2026-10-22).**
//! The states are recomputed by one background
//! [`crate::stamping::refresh::Refresher`]: after the data loads, on the
//! strip's "Refresh states" (never greyed out; it queues), and after each
//! stamp, debounced and coalesced, at most one at a time. A function just
//! stamped shows "stamped, updating…" at once. The stamp dialog's Done step
//! does not wait, a new dialog opens and prepares while a refresh runs,
//! and a finished refresh only replaces the states shown. Each refresh's
//! phase timings go to kovan's log and the button's tooltip.
//!
//! **The signing key, once per session (GitHub #1020).** The view owns the
//! app's key session ([`SharedKeySession`]), which outlives a change of
//! workspace: every signing window it opens (the stamp dialog,
//! Organisations & IV&V, the re-anchor window) signs through it, so the
//! passphrase is asked at the first signing only. The strip shows 🔓/🔒
//! with **Lock now**, and "Signing key…" opens the window that sets the
//! auto-lock and exports or imports the key (`super::key_window`, logic
//! [`crate::stamping::key_panel`]).
//!
//! **Run workspace test (2026-10-29).** The strip's "Run workspace test"
//! runs `kovan-cli test` on a thread (`super::workspace_test`), so the
//! "pending workspace test" a `Cargo.lock` change leaves on every stamp
//! clears without a terminal; the states are recomputed when it finishes.
//! "Test changed crates" runs only the crates whose dependencies moved
//! (#1049), and the progress line is fixed-format (#1050). The strip folds
//! to one row (▾/▸) so it does not jump while a test runs.
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** Checking and building the data
//! (cargo metadata, reading the SCIP index, resolving every call, writing)
//! run on a worker thread; the UI thread takes the result with `try_write`
//! and reads progress with `RunControl::snapshot`, neither of which blocks.
//! `kovan-web` loads the folder on its own background threads.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use kovan_common::call_graph::split::StampState;
use kovan_common::review::signing::keystore::Keystore;
use kovan_common::review::signing::session::{SessionStatus, SharedKeySession};
use kovan_web::ui::HostRequest;

use crate::code_review_data::{build, data_dir, data_state, BuildReport, DataState};
use crate::commands::index_control::{Progress, RunControl};
use crate::stamping::commit_push::CommitPushJob;
use super::review_mode_panel::PanelRequest;
use crate::stamping::flow::{Purpose, StampFlow, Target};
use crate::stamping::refresh::Refresher;
use crate::stamping::key_panel::{settings_file, status_line, try_lock, KeyPanel, KeySessionSettings};
use crate::stamping::organisations::panel::OrgPanel;
use crate::stamping::user_management::UserPanel;
use super::key_window::KeyPick;

/// Which of Code Review's tabs is shown (GitHub #1019).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ReviewTab {
    /// The code map and review mode.
    #[default]
    Map,
    /// Every reviewer, key and agent ([`UserPanel`]).
    Users,
}

/// What the worker found or built.
type Prepared = Result<Option<BuildReport>, String>;

struct Job {
    ctl: RunControl,
    slot: Arc<RwLock<Option<Prepared>>>,
    last: Progress,
}

/// The view's state; owned by the app.
#[derive(Default)]
pub(crate) struct CodeReviewView {
    workspace: Option<PathBuf>,
    /// An entry to open as soon as the embedded UI exists
    /// ([`Self::open_entry`]).
    open_at: Option<String>,
    /// The embedded UI, once the data is ready.
    review: Option<kovan_web::ui::CodeReview>,
    job: Option<Job>,
    /// Whether the data of `workspace` has been checked since it was set.
    checked: bool,
    /// The last build's report, or why there is nothing to show.
    note: Option<Result<String, String>>,
    /// A Stamp / Needs-fix press taken from the embedded UI, until its
    /// dialog is closed.
    pending: Option<HostRequest>,
    /// The stamp dialog of `pending`.
    dialog: Option<StampFlow>,
    /// The review states, recomputed in the background (GitHub #1018):
    /// after the data loads, on "Refresh states", and debounced after
    /// each stamp. ~~`states_job`, one full refresh per stamp, handed over
    /// by the dialog~~ **CORRECTED 2026-10-22**: one coalesced refresher;
    /// the dialog never waits on it.
    refresher: Refresher,
    /// Why the states could not be computed, or the dialog not opened.
    states_note: Option<String>,
    /// The keystore the dialog reads (tests inject a temporary one; `None`
    /// is kovan's default location, never read under `cargo test`).
    keystore: Option<Keystore>,
    /// The ⚑ need-you panel (#771).
    need_you: super::need_you_view::NeedYouView,
    /// Files kovan wrote this session, workspace-relative: what Commit and
    /// push may commit (#771).
    session: BTreeSet<String>,
    /// The commit-and-push run (one at a time, shared by the panel and the
    /// dialog).
    commit: CommitPushJob,
    /// The "Organisations & IV&V" window, while open (GitHub #810).
    organisations: Option<OrgPanel>,
    /// Review mode and the review walk (#770, #740), drawn in place of the
    /// embedded map while a function is under review.
    review_mode: super::review_mode_panel::ReviewModePanel,
    /// "Sync down": pull the code repository, then rebuild (2026-10-12).
    sync: super::code_sync::CodeSync,
    /// "Run workspace test" / "Test changed crates": `kovan-cli test` on a
    /// thread (2026-10-29, #1049, #1050).
    workspace_test: super::workspace_test::WorkspaceTest,
    /// The top strip folded to one row (maintainer, 2026-10-29: the
    /// wrapping strip made the view jump while a test ran).
    strip_collapsed: bool,
    /// A reviewed function's "Add connection" window, while open (#839).
    connection: Option<super::connection_picker::ConnectionPicker>,
    /// The app's key session (GitHub #1020): kept across a change of
    /// workspace, dropped (and zeroised) on Lock now, auto-lock and quit.
    key_session: SharedKeySession,
    /// The auto-lock setting has been read from kovan's config folder.
    key_settings_loaded: bool,
    /// "Lock now" pressed while a signing job held the session.
    lock_pending: bool,
    /// The session as last read (`try_read` may miss a frame).
    key_status: Option<SessionStatus>,
    /// The "Signing key" window, while open.
    key_panel: Option<KeyPanel>,
    /// A file picker the window asked for, until the app takes it.
    key_pick: Option<KeyPick>,
    /// The tab shown (GitHub #1019).
    tab: ReviewTab,
    /// Code Review ▸ User management, once opened.
    users: Option<UserPanel>,
}

/// The test's progress (fixed width, so the strip does not re-wrap as it
/// changes), its Cancel, and the last run's outcome, truncated with the
/// full text on hover. GUI drawing code (exempt from the test rule): the
/// text is `workspace_test::progress_text`, tested there.
fn test_status(ui: &mut egui::Ui, t: &super::workspace_test::WorkspaceTest) {
    let size = [300.0, ui.spacing().interact_size.y];
    if let Some(line) = t.progress_line() {
        ui.spinner();
        ui.add_sized(size, egui::Label::new(egui::RichText::new(line).weak()).truncate())
            .on_hover_text(t.latest().unwrap_or("starting"));
        if ui.small_button("Cancel test").clicked() {
            t.cancel();
        }
    }
    let (text, colour) = match &t.message {
        Some(Ok(m)) => (m, ui.visuals().weak_text_color()),
        Some(Err(e)) => (e, ui.visuals().error_fg_color),
        None => return,
    };
    ui.add_sized(size, egui::Label::new(egui::RichText::new(text).color(colour)).truncate())
        .on_hover_text(text);
}

/// Check the data of `workspace` and build it when missing or stale (or
/// always, with `force`), on a worker thread. GUI glue: the work is
/// `code_review_data::{data_state, build}`, tested there.
fn start(workspace: PathBuf, force: bool) -> Job {
    let ctl = RunControl::for_background();
    let slot: Arc<RwLock<Option<Prepared>>> = Arc::new(RwLock::new(None));
    let (worker_ctl, worker_slot) = (ctl.clone(), slot.clone());
    std::thread::spawn(move || {
        let result = match data_state(&workspace) {
            DataState::Current if !force => Ok(None),
            _ => build(&workspace, &worker_ctl).map(Some),
        };
        if let Ok(mut s) = worker_slot.write() {
            *s = Some(result);
        }
    });
    Job {
        ctl,
        slot,
        last: Progress::default(),
    }
}

impl CodeReviewView {
    /// Show `workspace` (the Code Map view's). A change drops the UI of the
    /// previous one; the new one's data is checked on the next frame shown.
    pub(crate) fn set_workspace(&mut self, workspace: Option<&Path>) {
        if self.workspace.as_deref() != workspace {
            // The key session belongs to the app, not to the workspace.
            *self = Self {
                workspace: workspace.map(Path::to_path_buf),
                key_session: self.key_session.clone(),
                key_settings_loaded: self.key_settings_loaded,
                key_status: self.key_status.take(),
                tab: self.tab,
                ..Self::default()
            };
        }
    }

    /// Open `entry` (`path/to/file.rs::qual`) of `workspace`, with its
    /// source, once the workspace's data is loaded (GitHub #844: a code
    /// review chosen on the mindmap).
    pub(crate) fn open_entry(&mut self, workspace: &Path, entry: &str) {
        self.set_workspace(Some(workspace));
        self.open_at = Some(entry.to_string());
    }

    /// An "Index fresh" of `dir` finished: rebuild its data now, in the
    /// background, so the view is ready when it is opened.
    pub(crate) fn index_finished(&mut self, dir: &Path) {
        self.set_workspace(Some(dir));
        self.rebuild(true);
    }

    /// Push fresh stamp states into the embedded UI (after the stamp dialog
    /// wrote one), so it recolours without reloading. A no-op while there is
    /// no UI: the next data build carries the stamps.
    pub(crate) fn set_stamps(&mut self, stamps: Vec<StampState>) {
        if let Some(review) = &mut self.review {
            review.set_stamps(stamps);
        }
    }

    /// Take the embedded UI's Stamp / Needs-fix press, if any, and open its
    /// dialog. A press while a dialog is open is dropped (since 2026-10-12
    /// the dialog is a movable window, no longer a modal that blocks the
    /// bar, so this drop is what keeps one dialog at a time).
    fn take_request(&mut self) {
        match self.review.as_mut().and_then(|r| r.take_host_request()) {
            Some(HostRequest::AddConnection(f)) => {
                if let Some(root) = &self.workspace {
                    self.connection = Some(super::connection_picker::ConnectionPicker::new(root, f));
                }
            }
            Some(r) if self.pending.is_none() => self.open(r),
            _ => {}
        }
    }

    /// The keystore the dialog uses: the injected one, else kovan's default
    /// location; under `cargo test` never the user's.
    fn keystore(&self) -> Result<Keystore, String> {
        match &self.keystore {
            Some(k) => Ok(k.clone()),
            None if cfg!(test) => Err("tests never read the real keystore".into()),
            None => Keystore::default_location().map_err(|e| e.to_string()),
        }
    }

    /// Open the stamp dialog for `req` (its first job starts on a worker).
    fn open(&mut self, req: HostRequest) {
        let purpose = match &req {
            HostRequest::Stamp(_) => Purpose::Stamp,
            HostRequest::NeedsFix(_) => Purpose::NeedsFix,
            // "Quick stamp" (#857): the re-confirm step, held as a Stamp
            // like review mode's own Re-confirm.
            HostRequest::Reconfirm(f) => return self.open_flow(Purpose::Reconfirm, HostRequest::Stamp(f.clone())),
            // Not a stamp dialog: `take_request` opens its own window.
            HostRequest::AddConnection(_) => return,
        };
        self.open_flow(purpose, req);
    }

    /// Open the stamp dialog for `purpose` on the function `req` names
    /// (added 2026-10-10 for review mode's Re-confirm, #770: `req` is then
    /// held as a Stamp, the request kovan-web knows).
    fn open_flow(&mut self, purpose: Purpose, req: HostRequest) {
        let f = match &req {
            HostRequest::Stamp(f)
            | HostRequest::NeedsFix(f)
            | HostRequest::AddConnection(f)
            | HostRequest::Reconfirm(f) => f,
        };
        let target = Target {
            function: f.id.clone(),
            name: f.name.clone(),
        };
        let (Some(root), ks) = (self.workspace.clone(), self.keystore()) else {
            return;
        };
        match ks {
            Ok(ks) => {
                self.dialog = Some(
                    StampFlow::new(root, ks, purpose, target).with_session(self.key_session.clone()),
                );
                self.pending = Some(req);
            }
            Err(e) => self.states_note = Some(format!("Cannot open the stamp dialog: {e}")),
        }
    }

    /// Open the "Organisations & IV&V" window (GitHub #810) over the
    /// workspace, with the same keystore rule as the stamp dialog.
    fn open_organisations(&mut self) {
        let Some(root) = self.workspace.clone() else {
            return;
        };
        match self.keystore() {
            Ok(ks) => {
                self.organisations = Some(OrgPanel::new(root, ks).with_session(self.key_session.clone()))
            }
            Err(e) => {
                self.states_note = Some(format!("Cannot open Organisations & IV&V: {e}"));
            }
        }
    }

    /// Open the "Signing key" window (GitHub #1020) over the workspace,
    /// with the same keystore rule as the stamp dialog.
    fn open_key_panel(&mut self) {
        let Some(root) = self.workspace.clone() else {
            return;
        };
        match self.keystore() {
            Ok(ks) => {
                self.key_panel = Some(KeyPanel::new(root, ks, self.key_session.clone(), settings_file()))
            }
            Err(e) => self.states_note = Some(format!("Cannot open the signing key window: {e}")),
        }
    }

    /// Open Code Review ▸ User management (GitHub #1019) over the
    /// workspace, with the same keystore rule as the stamp dialog, and the
    /// Signing-key panel its "My keys on this machine" draws.
    fn open_users(&mut self) {
        let Some(root) = self.workspace.clone() else {
            return;
        };
        match self.keystore() {
            Ok(ks) => {
                self.users = Some(UserPanel::new(root, ks, self.key_session.clone()));
                if self.key_panel.is_none() {
                    self.open_key_panel();
                }
            }
            Err(e) => self.states_note = Some(format!("Cannot open User management: {e}")),
        }
    }

    /// Once per frame (GitHub #1020): read the auto-lock setting the first
    /// time, drop a key idle past the timeout, apply a pending Lock now,
    /// and cache the status for the strip. Never blocks. Returns how long
    /// until the key would lock, so the caller repaints by then.
    fn key_frame(&mut self) -> Option<std::time::Duration> {
        if !self.key_settings_loaded {
            if let Ok(mut s) = self.key_session.try_write() {
                if let Some(path) = settings_file() {
                    s.set_auto_lock(KeySessionSettings::read(&path).auto_lock());
                }
                self.key_settings_loaded = true;
            }
        }
        if self.lock_pending && try_lock(&self.key_session).is_some() {
            self.lock_pending = false;
        }
        if let Ok(mut s) = self.key_session.try_write() {
            s.expire_if_idle();
        }
        if let Ok(s) = self.key_session.try_read() {
            self.key_status = s.status();
        }
        self.key_status.as_ref().map(|s| s.locks_in)
    }

    /// "Lock now" from the strip or the window: at once, or as soon as a
    /// signing job lets go of the session.
    pub(crate) fn lock_key(&mut self) {
        if try_lock(&self.key_session).is_none() {
            self.lock_pending = true;
        }
        self.key_status = None;
    }

    /// The file picker the "Signing key" window asked for, once.
    pub(crate) fn take_key_pick(&mut self) -> Option<KeyPick> {
        self.key_pick.take()
    }

    /// A path was picked for the window: the export destination
    /// (`export`), else the key file to import.
    pub(crate) fn key_picked(&mut self, export: bool, path: PathBuf) {
        let Some(p) = self.key_panel.as_mut() else { return };
        if export {
            p.export_to(path);
        } else {
            p.import_source = Some(path);
            p.error = None;
        }
    }

    /// Close the dialog, keeping a refresh it started.
    fn close_dialog(&mut self) {
        if let Some(mut d) = self.dialog.take() {
            self.session.extend(d.wrote.iter().cloned());
            if let crate::stamping::flow::Step::Done(o) = &d.step {
                self.review_mode.outcome(o.what);
            } else {
                // A refusal (the code changed while reviewed, #740
                // decision 7): show the new diff first.
                self.review_mode.reload();
            }
            self.take_stamped(&mut d);
        }
        self.pending = None;
    }

    /// A write the dialog made (GitHub #1018): its function shows
    /// "stamped, updating…" at once, and a background refresh is asked
    /// for (debounced, coalesced with other stamps).
    fn take_stamped(&mut self, d: &mut StampFlow) {
        if let Some(st) = d.take_stamped() {
            self.refresher.set_root(self.workspace.clone());
            if let Some(s) = self.refresher.stamped(&st.function, &st.reviewer, &st.date) {
                self.set_stamps(s);
            }
        }
    }

    /// Recompute the stamp states from `review.md` in the background, now
    /// (queued behind one running, never blocking).
    fn load_states(&mut self) {
        self.refresher.set_root(self.workspace.clone());
        self.refresher.request_now();
    }

    /// Take a finished refresh and start a due one, without blocking.
    fn poll_states(&mut self) {
        self.refresher.set_root(self.workspace.clone());
        if let Some(s) = self.refresher.poll() {
            self.states_note = None;
            self.set_stamps(s);
        }
        if let Some(e) = self.refresher.error.take() {
            self.states_note = Some(format!("Review states from review.md: {e}"));
        }
    }

    fn rebuild(&mut self, force: bool) {
        if let (Some(dir), None) = (self.workspace.clone(), &self.job) {
            self.review = None;
            self.checked = true;
            self.job = Some(start(dir, force));
        }
    }

    /// Take the worker's result, if it has one, without blocking.
    fn poll(&mut self) {
        let Some(job) = &mut self.job else { return };
        if let Some(p) = job.ctl.snapshot() {
            job.last = p;
        }
        let Some(result) = job.slot.try_write().ok().and_then(|mut s| s.take()) else {
            return;
        };
        self.job = None;
        let Some(dir) = self.workspace.clone() else {
            return;
        };
        match result {
            Ok(report) => {
                self.note = report.map(|r| Ok(r.describe()));
                let source = kovan_web::data::DataSource::Dir {
                    data: data_dir(&dir),
                    workspace: dir,
                };
                self.review = Some(kovan_web::ui::CodeReview::new(
                    kovan_web::Mode::Desktop,
                    source,
                ));
                self.load_states();
            }
            Err(e) => self.note = Some(Err(e)),
        }
    }

    /// Draw the view. GUI drawing code (exempt from the test rule): its
    /// logic is `code_review_data`, and the UI drawn is `kovan-web`'s.
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
        if !self.checked {
            self.rebuild(false);
        }
        self.poll();
        self.poll_states();
        if let Some(left) = self.key_frame() {
            // Repaint by the auto-lock, so the key is dropped on time.
            ui.ctx().request_repaint_after(left + std::time::Duration::from_secs(1));
        }
        if self.refresher.outstanding() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
        self.need_you.set_root(self.workspace.as_deref());
        self.need_you.key_session = self.key_session.clone();
        if self.commit.poll() {
            // Committed: the stamps may now count, and the files are in git.
            if let Some(Ok(p)) = &self.commit.last {
                for f in &p.files {
                    self.session.remove(f);
                }
            }
            self.need_you.dirty = true;
            self.load_states();
        }
        if self.workspace_test.poll() {
            // A counted run clears "pending workspace test".
            self.need_you.dirty = true;
            self.load_states();
        }
        if self.sync.poll() {
            // Pulled: the stamps and the code may both have moved. The same
            // as pressing "Rebuild data" after a `git pull`.
            self.need_you.dirty = true;
            self.rebuild(true);
        }
        let mut rebuild = false;
        let mut open_orgs = false;
        let mut open_key = false;
        let mut lock_now = false;
        let mut sync_now = false;
        let mut test_now: Option<super::workspace_test::Scope> = None;
        let mut refresh_now = false;
        // Review mode (#770): its workspace and keystore, and the function
        // selected on the embedded map, which [Review >] starts from.
        self.review_mode
            .set_root(self.workspace.clone(), self.keystore().ok());
        let selected = self.review.as_ref().and_then(|r| r.selected_function());
        let mut review_now = None;
        let mut walk_now = None;
        egui::Panel::top("code-review-strip").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let fold = if self.strip_collapsed { "\u{25B8}" } else { "\u{25BE}" };
                if ui
                    .small_button(fold)
                    .on_hover_text("Fold this bar to one row, or unfold it")
                    .clicked()
                {
                    self.strip_collapsed = !self.strip_collapsed;
                }
                ui.selectable_value(&mut self.tab, ReviewTab::Map, "Code map");
                ui.selectable_value(&mut self.tab, ReviewTab::Users, "User management")
                    .on_hover_text("Every reviewer and key, and the agents' justifications (GitHub #1019)");
                if self.strip_collapsed {
                    // One row: the test's progress only (#1050).
                    test_status(ui, &self.workspace_test);
                    return;
                }
                match &self.workspace {
                    Some(dir) => ui.weak(dir.display().to_string()),
                    None => ui.weak("No workspace"),
                };
                if let (Some(f), true) = (&selected, self.review_mode.active.is_none()) {
                    if ui
                        .button(format!("Review {} >", f.name))
                        .on_hover_text("Review mode: its source or diff since the last review, and this folder's review.md beside it")
                        .clicked()
                    {
                        review_now = Some(f.clone());
                    }
                    if ui
                        .button("Review this walk >")
                        .on_hover_text("Every function under this one, bottom-up, with the walk's size shown first")
                        .clicked()
                    {
                        walk_now = Some(f.clone());
                    }
                }
                if self.workspace.is_some()
                    && ui
                        .add_enabled(self.job.is_none(), egui::Button::new("Rebuild data"))
                        .on_hover_text(
                            "Build this view's data again from the workspace's saved SCIP index \
                             (target/kovan-scip/index.scip). rust-analyzer is not run.",
                        )
                        .clicked()
                {
                    rebuild = true;
                }
                if self.workspace.is_some() {
                    let free = self.job.is_none()
                        && !self.sync.busy()
                        && !self.workspace_test.busy()
                        && self.dialog.is_none()
                        && self.review_mode.active.is_none();
                    if ui
                        .add_enabled(free, egui::Button::new("\u{2B07} Sync down"))
                        .on_hover_text(
                            "Pull this repository's branch (stamps and fixes pushed from \
                             elsewhere), then rebuild this view. If the folder and the remote \
                             disagree you are asked before anything is forced.",
                        )
                        .on_disabled_hover_text(
                            "Wait for the running job or workspace test, and close the stamp dialog or review mode",
                        )
                        .clicked()
                    {
                        sync_now = true;
                    }
                    if let Some(w) = self.sync.working() {
                        ui.spinner();
                        ui.weak(w);
                    }
                    match &self.sync.message {
                        Some(Ok(t)) => {
                            ui.weak(t);
                        }
                        Some(Err(e)) => {
                            ui.colored_label(ui.visuals().error_fg_color, e);
                        }
                        None => {}
                    }
                    if !self.workspace_test.busy() {
                        use super::workspace_test::Scope;
                        if ui
                            .add_enabled(!self.sync.busy(), egui::Button::new("Test changed crates"))
                            .on_hover_text(
                                "kovan-cli test -p, for each crate whose dependencies changed in \
                                 Cargo.lock since its last recorded run (or that has none): clears \
                                 those crates' \"pending workspace test\". -p resolves features \
                                 for those crates alone, not unified across the workspace (#1049).",
                            )
                            .on_disabled_hover_text("Wait for Sync down")
                            .clicked()
                        {
                            test_now = Some(Scope::ChangedCrates);
                        }
                        if ui
                            .add_enabled(!self.sync.busy(), egui::Button::new("Run workspace test"))
                            .on_hover_text(
                                "kovan-cli test: the full release suite at this commit (hours). \
                                 A counted run clears \"pending workspace test\" after a Cargo.lock \
                                 change. Refused while .rs, Cargo.toml or Cargo.lock edits are \
                                 uncommitted.",
                            )
                            .on_disabled_hover_text("Wait for Sync down")
                            .clicked()
                        {
                            test_now = Some(Scope::Workspace);
                        }
                    }
                    test_status(ui, &self.workspace_test);
                }
                match &self.note {
                    Some(Ok(text)) => {
                        ui.weak(text);
                    }
                    Some(Err(e)) => {
                        ui.colored_label(ui.visuals().error_fg_color, e);
                    }
                    None => {}
                }
                if self.workspace.is_some()
                    && ui
                        .add_enabled(
                            self.organisations.is_none(),
                            egui::Button::new("Organisations & IV&V"),
                        )
                        .on_hover_text(
                            "Rung 5: sign developing and reviewer organisations (maintainer) \
                             or a separation attestation (independent reviewer).",
                        )
                        .clicked()
                {
                    open_orgs = true;
                }
                if self.workspace.is_some() {
                    ui.weak(status_line(self.key_status.as_ref()));
                    if self.key_status.is_some() && ui.small_button("Lock now").clicked() {
                        lock_now = true;
                    }
                    if ui
                        .add_enabled(self.key_panel.is_none(), egui::Button::new("Signing key\u{2026}"))
                        .on_hover_text(
                            "Lock state and auto-lock; export your key to another machine, or \
                             import it here",
                        )
                        .clicked()
                    {
                        open_key = true;
                    }
                }
                if let Some(st) = self.refresher.status() {
                    ui.spinner();
                    ui.weak(st);
                }
                if self.review.is_some() {
                    let timing = self.refresher.last_timings.map(|t| t.line());
                    let b = ui
                        .small_button("Refresh states")
                        .on_hover_text(timing.unwrap_or_else(|| {
                            "Recompute every function's state from review.md in the background".into()
                        }));
                    if b.clicked() {
                        // Never greyed out: queued behind a running one.
                        refresh_now = true;
                    }
                }
                if self.workspace.is_some() {
                    let label = match self.need_you.count() {
                        Some(n) => format!("\u{2691} {n} need you"),
                        None => "\u{2691} need you".to_string(),
                    };
                    if ui
                        .selectable_label(self.need_you.open, label)
                        .on_hover_text(
                            "What needs you: stale stamps, re-confirms, fixes, moves, \
                             deletions and new code; recently reviewed; commit and push",
                        )
                        .clicked()
                    {
                        self.need_you.open = !self.need_you.open;
                    }
                }
                if let Some(e) = &self.states_note {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
            });
        });
        if rebuild {
            self.rebuild(true);
        }
        if refresh_now {
            self.load_states();
        }
        if let (Some(scope), Some(dir)) = (test_now, self.workspace.clone()) {
            self.workspace_test.start(dir, scope);
        }
        if sync_now {
            if let Some(dir) = self.workspace.clone() {
                self.sync.start(dir);
            }
        }
        if self.sync.working().is_some() || self.workspace_test.busy() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
        let unsaved = self.session.len();
        self.sync.prompt_ui(&ui.ctx().clone(), unsaved);
        if open_orgs {
            self.open_organisations();
        }
        if lock_now {
            self.lock_key();
        }
        if open_key {
            self.open_key_panel();
        }
        if self.dialog.as_ref().is_some_and(|d| d.wants_key_window) {
            if let Some(d) = self.dialog.as_mut() {
                d.wants_key_window = false;
            }
            if self.key_panel.is_none() {
                self.open_key_panel();
            }
        }
        let users_tab = self.tab == ReviewTab::Users && self.workspace.is_some();
        if users_tab && self.users.is_none() {
            self.open_users();
        }
        if users_tab {
            // The tab draws the Signing-key panel's body inline: no window.
            if let Some(u) = self.users.as_mut() {
                let mut pick = None;
                let keys = self.key_panel.as_mut();
                egui::CentralPanel::default().show(ui, |ui| {
                    pick = super::user_management_view::show(ui, u, keys);
                });
                if pick.is_some() {
                    self.key_pick = pick;
                }
                if u.take_changed() && self.review.is_some() {
                    // Trust changed: every stamp may count differently.
                    self.load_states();
                }
            }
            if self.key_panel.as_mut().is_some_and(|p| p.take_imported()) {
                if let Some(u) = self.users.as_mut() {
                    u.load();
                }
            }
            return;
        }
        if let Some(p) = self.key_panel.as_mut() {
            let (open, pick) = super::key_window::show(&ui.ctx().clone(), p);
            if pick.is_some() {
                self.key_pick = pick;
            }
            if p.take_imported() {
                // A key arrived: an open stamp dialog looks again.
                if let Some(d) = self.dialog.as_mut() {
                    d.reload_keys();
                }
            }
            if !open {
                self.key_panel = None;
            }
        }
        if let Some(p) = self.organisations.as_mut() {
            if super::organisations_view::show(&ui.ctx().clone(), p) {
                self.organisations = None;
                // A record may change rung 5 on every function: recompute.
                if self.review.is_some() {
                    self.load_states();
                }
            }
        }
        if let Some(f) = review_now {
            self.review_mode.enter(f);
        }
        if let Some(f) = walk_now {
            self.review_mode.plan(&f);
        }
        if self.review_mode.walk.is_some() {
            egui::Panel::top("code-review-walk").show(ui, |ui| self.review_mode.walk_strip(ui));
        }
        if let Some(job) = &self.job {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
            egui::CentralPanel::default().show(ui, |ui| {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.spinner();
                    ui.label(if job.last.phase.is_empty() {
                        "Checking the code review data\u{2026}"
                    } else {
                        job.last.phase.as_str()
                    });
                    // The call graph counts the functions it has expanded;
                    // the other phases do not count.
                    if job.last.total > 0 {
                        ui.add(
                            egui::ProgressBar::new(job.last.done as f32 / job.last.total as f32)
                                .desired_width(360.0)
                                .text(format!("{} / {}", job.last.done, job.last.total)),
                        );
                    }
                    ui.weak("This runs in the background; the other tabs stay usable.");
                });
                // What the terminal prints, newest last (as "Index fresh" shows it).
                egui::CollapsingHeader::new("Log")
                    .id_salt("code-review-data-log")
                    .default_open(true)
                    .show(ui, |ui| {
                        for l in job.last.log.iter().rev().take(12).rev() {
                            ui.weak(l);
                        }
                    });
            });
            return;
        }
        match &mut self.review {
            Some(review) => {
                if let Some(link) = super::need_you_view::show(
                    ui,
                    &mut self.need_you,
                    &mut self.session,
                    &mut self.commit,
                ) {
                    review.navigate(link);
                }
                if let Some(id) = self.open_at.take() {
                    review.navigate(kovan_web::model::DeepLink::Function { id, source: true });
                }
                if self.review_mode.active.is_some() {
                    self.review_mode.ui(ui);
                } else {
                    review.show(ui);
                }
                let panel_requests = self.review_mode.take_requests();
                for r in &panel_requests {
                    if let PanelRequest::Goto(id) = r {
                        review.open_function_later(id);
                    }
                }
                self.take_request();
                for r in panel_requests {
                    if self.pending.is_some() {
                        break;
                    }
                    match r {
                        PanelRequest::Dialog(req, seed) => {
                            self.open(req);
                            if let Some(d) = self.dialog.as_mut() {
                                d.comments_seed = seed;
                            }
                        }
                        PanelRequest::Reconfirm(f) => {
                            self.open_flow(Purpose::Reconfirm, HostRequest::Stamp(f));
                        }
                        PanelRequest::Goto(_) => {}
                    }
                }
                let ctx = ui.ctx().clone();
                let mut closed = false;
                let mut fresh = None;
                if let Some(d) = self.dialog.as_mut() {
                    if !d.wrote.is_subset(&self.session) {
                        self.session.extend(d.wrote.iter().cloned());
                        self.need_you.dirty = true;
                    }
                    closed = super::stamp_dialog::show(
                        &ctx,
                        d,
                        &mut self.commit,
                        self.workspace.as_deref(),
                        &self.session,
                    );
                    fresh = d.take_states();
                }
                if let Some(mut d) = self.dialog.take() {
                    self.take_stamped(&mut d);
                    self.dialog = Some(d);
                }
                if let Some(s) = fresh {
                    self.set_stamps(s);
                }
                if closed {
                    self.close_dialog();
                }
                // The "Add connection" window (#839): the file it writes
                // joins the session's, for Commit and push.
                if let Some(p) = self.connection.as_mut() {
                    if p.poll() {
                        self.session.insert(crate::code_connections::REL_PATH.to_string());
                    }
                    if p.busy() {
                        ctx.request_repaint_after(std::time::Duration::from_millis(200));
                    }
                    if !super::connection_picker::show(&ctx, p) {
                        self.connection = None;
                    }
                }
            }
            None => {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.add_space(24.0);
                    ui.vertical_centered(|ui| {
                        if self.workspace.is_none() {
                            ui.label("No workspace is open.");
                            ui.weak("Choose a Rust workspace or crate in the Code Map tab.");
                        } else {
                            ui.label("There is no code review data for this workspace yet.");
                            ui.weak("Run \"Index fresh\" in the Code Map tab; this view then loads by itself.");
                        }
                    });
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_web::ui::FunctionRef;

    /// Headless: the view draws the embedded UI with a pending request (the
    /// placeholder modal on top) without a display or a panic, keeps the
    /// request until Cancel, and passes stamps through to the embedded UI.
    #[test]
    fn a_pending_request_is_held_and_drawn_headless() {
        let d = tempfile::tempdir().unwrap();
        let mut v = CodeReviewView {
            review: Some(kovan_web::ui::CodeReview::new(
                kovan_web::Mode::Desktop,
                kovan_web::data::DataSource::Dir {
                    data: d.path().to_path_buf(),
                    workspace: d.path().to_path_buf(),
                },
            )),
            ..CodeReviewView::default()
        };
        let req = HostRequest::Stamp(FunctionRef {
            id: "crates/a/src/lib.rs::f".into(),
            krate: "a".into(),
            file: "crates/a/src/lib.rs".into(),
            name: "f".into(),
            callees: None,
            lines: None,
        });
        v.pending = Some(req.clone());
        // The need-you panel open too (no workspace: it builds nothing).
        v.need_you.open = true;
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));
        }
        assert_eq!(v.pending, Some(req), "held until Cancel");
        // Nothing pressed in the embedded UI: nothing new taken.
        v.pending = None;
        v.take_request();
        assert_eq!(v.pending, None);
        v.set_stamps(Vec::new());
    }

    fn function() -> FunctionRef {
        FunctionRef {
            id: "crates/a/src/lib.rs::f".into(),
            krate: "a".into(),
            file: "crates/a/src/lib.rs".into(),
            name: "f".into(),
            callees: None,
            lines: None,
        }
    }

    /// Headless (#770, #740 decision 8), on the stamping fixture (temporary
    /// git repository and keystore, test passphrase): `twice` inherited
    /// stale after `leaf` changed and was re-stamped. Review mode's
    /// Re-confirm request opens the dialog on its re-confirm step, which
    /// draws `leaf`'s unified diff and the passphrase form without a
    /// display or a panic.
    ///
    /// Result (2026-10-10): passes.
    #[test]
    fn the_reconfirm_step_draws_the_callee_diff_headless() {
        use crate::stamping::flow::Step;
        use crate::stamping::tests::{founder_key, Repo, LIB};
        let r = Repo::new();
        let store = tempfile::tempdir().unwrap();
        let key = founder_key(&r, store.path());
        r.stamp(&key, "leaf", "");
        r.stamp(&key, "twice", "");
        r.edit("x * 2.0", "x * 3.0");
        r.commit("edit leaf");
        r.reindex();
        r.stamp(&key, "leaf", "");
        let mut v = CodeReviewView {
            workspace: Some(r.path().to_path_buf()),
            review: Some(kovan_web::ui::CodeReview::new(
                kovan_web::Mode::Desktop,
                kovan_web::data::DataSource::Dir {
                    data: r.path().to_path_buf(),
                    workspace: r.path().to_path_buf(),
                },
            )),
            checked: true,
            keystore: Some(Keystore::at(store.path())),
            ..CodeReviewView::default()
        };
        let twice = FunctionRef {
            id: format!("{LIB}::twice"),
            krate: "demo".into(),
            file: LIB.into(),
            name: "twice".into(),
            callees: None,
            lines: None,
        };
        // Through the canvas's "Quick stamp" request (#857), which must
        // reach the same re-confirm step.
        v.open(HostRequest::Reconfirm(twice));
        v.dialog.as_mut().unwrap().wait();
        let d = v.dialog.as_ref().unwrap();
        assert_eq!(d.step, Step::Reconfirm, "{:?}", d.step);
        assert_eq!(d.reconfirm.as_ref().unwrap().changed.len(), 1);
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));
        }
        assert_eq!(v.dialog.as_ref().unwrap().step, Step::Reconfirm, "drawn, not moved");
        v.close_dialog();
    }

    /// Headless (#770): with a function under review and a walk planned,
    /// the view draws review mode in place of the embedded map, and the walk
    /// card, while their workers run (and are refused over an empty
    /// folder), without a display or a panic. Temporary folders only.
    #[test]
    fn review_mode_replaces_the_map_headless() {
        let d = tempfile::tempdir().unwrap();
        let mut v = CodeReviewView {
            workspace: Some(d.path().to_path_buf()),
            review: Some(kovan_web::ui::CodeReview::new(
                kovan_web::Mode::Desktop,
                kovan_web::data::DataSource::Dir {
                    data: d.path().to_path_buf(),
                    workspace: d.path().to_path_buf(),
                },
            )),
            checked: true,
            ..CodeReviewView::default()
        };
        v.review_mode.set_root(Some(d.path().to_path_buf()), None);
        v.review_mode.enter(function());
        v.review_mode.plan(&function());
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));
        }
        assert!(v.review_mode.active.is_some(), "still in review mode");
        assert!(v.review_mode.walk.is_some());
    }

    /// Headless: a Stamp request opens the dialog only with a keystore
    /// (never the real one under `cargo test`); the dialog draws every
    /// step without a display or a panic; closing it clears the request.
    /// Temporary folders only.
    #[test]
    fn the_stamp_dialog_opens_and_draws_every_step_headless() {
        use crate::stamping::flow::{Outcome, Step, WizardForm};
        use crate::stamping::StampContext;

        let d = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        let mut v = CodeReviewView {
            workspace: Some(d.path().to_path_buf()),
            review: Some(kovan_web::ui::CodeReview::new(
                kovan_web::Mode::Desktop,
                kovan_web::data::DataSource::Dir {
                    data: d.path().to_path_buf(),
                    workspace: d.path().to_path_buf(),
                },
            )),
            checked: true,
            ..CodeReviewView::default()
        };
        v.open(HostRequest::Stamp(function()));
        assert!(v.dialog.is_none() && v.pending.is_none());
        assert!(v.states_note.as_deref().unwrap().contains("real keystore"));

        v.keystore = Some(Keystore::at(store.path()));
        v.open(HostRequest::NeedsFix(function()));
        assert!(v.dialog.is_some() && v.pending.is_some());
        v.dialog.as_mut().unwrap().wait();
        assert_eq!(v.dialog.as_ref().unwrap().step, Step::NoRoot);

        let ctx = egui::Context::default();
        let wizard = StampContext {
            fn_id: "fn:00".into(),
            call_graph_id: "crates/a/src/lib.rs::f".into(),
            path: "crates/a/src/lib.rs::f".into(),
            review_md: "crates/a/src/review.md".into(),
            applicability: Default::default(),
            answers: [("doc_matches_behaviour".to_string(), "partly".to_string())].into(),
            restamp: true,
            previous_comments: String::new(),
            hash: String::new(),
            no_concept: Some("other: a test".into()),
            suggested_architecture: None,
            concept: Some("concept:a/b".into()),
            concepts: crate::stamping::concepts::standard_concepts(),
        };
        let steps = [
            Step::Loading,
            Step::NoRoot,
            Step::SetupKey,
            Step::Generating,
            Step::PickKey,
            Step::Register,
            Step::Registering,
            Step::Preparing,
            Step::Refused {
                message: "a.rs has changes not committed".into(),
                hint: "Commit or stash".into(),
            },
            Step::Wizard,
            Step::Signing,
            Step::PreparingReconfirm,
            Step::Reconfirm,
            Step::NeedsFixForm,
            Step::WritingNeedsFix,
            Step::Done(Outcome {
                what: crate::stamping::flow::Purpose::Stamp,
                review_md: "crates/a/src/review.md".into(),
                replaced: true,
            }),
            Step::Failed("unreadable".into()),
        ];
        for step in steps {
            {
                let f = v.dialog.as_mut().unwrap();
                f.wizard = Some(WizardForm::new(wizard.clone()));
                f.step = step.clone();
            }
            for _ in 0..2 {
                let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));
            }
            assert_eq!(v.dialog.as_ref().unwrap().step, step, "drawn, not moved");
        }
        v.close_dialog();
        assert!(v.dialog.is_none() && v.pending.is_none());

        // Review mode's Re-confirm opens the same dialog on its re-confirm
        // purpose (#770); with no kovan_root.toml it stops there.
        v.open_flow(crate::stamping::flow::Purpose::Reconfirm, HostRequest::Stamp(function()));
        assert_eq!(
            v.dialog.as_ref().unwrap().purpose,
            crate::stamping::flow::Purpose::Reconfirm
        );
        v.dialog.as_mut().unwrap().wait();
        assert_eq!(v.dialog.as_ref().unwrap().step, Step::NoRoot);
        v.close_dialog();
    }

    /// Headless (GitHub #810): the Organisations & IV&V window opens only
    /// with a keystore (never the real one under `cargo test`) and draws
    /// its loading, error and loaded states, with a key, forms filled and
    /// notices, without a display or a panic; the stamp dialog draws its
    /// attestation picker with and without choices. Temporary folders only.
    #[test]
    fn organisations_window_and_attestation_picker_draw_headless() {
        use crate::stamping::flow::{KeyInfo, Step, WizardForm};
        use crate::stamping::organisations::Overview;
        use crate::stamping::StampContext;
        use kovan_common::review::ivv_view::AttestationChoice;
        use kovan_common::review::root::Role;

        let d = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        let mut v = CodeReviewView {
            workspace: Some(d.path().to_path_buf()),
            checked: true,
            ..CodeReviewView::default()
        };
        v.open_organisations();
        assert!(v.organisations.is_none());
        v.keystore = Some(Keystore::at(store.path()));
        v.open_organisations();
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));
        }
        v.organisations.as_mut().unwrap().wait();
        let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));
        {
            let p = v.organisations.as_mut().unwrap();
            assert!(matches!(&p.overview, Some(Err(_))), "no kovan_root.toml");
            p.overview = Some(Ok(Overview {
                warnings: vec!["developing organisation record 1: it is not signed".into()],
                reviewers: vec![("github:m".into(), Role::Maintainer)],
                ..Overview::default()
            }));
            p.keys = vec![KeyInfo {
                reviewer: "github:m".into(),
                key: "k1".into(),
                path: store.path().join("k"),
                registered: true,
                fingerprint: "SHA256:test".into(),
            }];
            p.forms.dev_name = "Outram Park project".into();
            p.forms.attestation.audit_record = "not a url".into();
            p.notices.push("Signed.".into());
            p.error = Some("Wrong passphrase. Nothing was written.".into());
        }
        for _ in 0..2 {
            let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));
        }
        assert!(v.organisations.is_some(), "open until closed");

        // The picker in the stamp dialog.
        v.review = Some(kovan_web::ui::CodeReview::new(
            kovan_web::Mode::Desktop,
            kovan_web::data::DataSource::Dir {
                data: d.path().to_path_buf(),
                workspace: d.path().to_path_buf(),
            },
        ));
        v.organisations = None;
        v.open(HostRequest::Stamp(function()));
        v.dialog.as_mut().unwrap().wait();
        let ctx_w = StampContext {
            fn_id: "fn:00".into(),
            call_graph_id: "crates/a/src/lib.rs::f".into(),
            path: "crates/a/src/lib.rs::f".into(),
            review_md: "crates/a/src/review.md".into(),
            applicability: Default::default(),
            answers: Default::default(),
            restamp: false,
            previous_comments: String::new(),
            hash: String::new(),
            no_concept: None,
            suggested_architecture: None,
            concept: None,
            concepts: Vec::new(),
        };
        for choices in [0, 1] {
            {
                let f = v.dialog.as_mut().unwrap();
                let mut w = WizardForm::new(ctx_w.clone());
                if choices == 1 {
                    w.attestations = vec![AttestationChoice {
                        id: "sep-1".into(),
                        organisation: "Example IV&V Ltd".into(),
                        developing_organisation: "Outram Park project".into(),
                        date: "2026-10-10".into(),
                        audit_record: "https://github.com/o/r/issues/1".into(),
                    }];
                    w.separation_attestation = Some("sep-1".into());
                }
                f.wizard = Some(w);
                f.step = Step::Wizard;
            }
            for _ in 0..2 {
                let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));
            }
            let w = v.dialog.as_ref().unwrap().wizard.as_ref().unwrap();
            assert_eq!(
                w.separation_attestation.is_some(),
                choices == 1,
                "the picker keeps a valid choice and clears one with no choices"
            );
        }
    }

    /// Methodology (#844): a code review chosen on the mindmap asks the
    /// view to open an entry of a mounted repository. Pass: the view takes
    /// that repository as its workspace and remembers the entry; showing
    /// the same workspace again (what the app does every frame) keeps it;
    /// switching to another workspace drops it with the rest of the view.
    ///
    /// Result (2026-10-13): passes.
    #[test]
    fn an_entry_chosen_on_the_mindmap_is_remembered_until_the_data_is_ready() {
        let mut v = CodeReviewView::default();
        let (a, b) = (Path::new("/tmp/kovan-a"), Path::new("/tmp/kovan-b"));
        v.open_entry(a, "src/lib.rs::leaf");
        assert_eq!(v.workspace.as_deref(), Some(a));
        assert_eq!(v.open_at.as_deref(), Some("src/lib.rs::leaf"));
        v.set_workspace(Some(a));
        assert_eq!(v.open_at.as_deref(), Some("src/lib.rs::leaf"));
        v.set_workspace(Some(b));
        assert_eq!(v.open_at, None);
    }

    /// The key session in the view (GitHub #1020): it survives a change of
    /// workspace; the frame step caches its status and says when it would
    /// lock; Lock now drops the key; the "Signing key" window opens over the
    /// injected keystore, takes a picked import file, and draws headless
    /// (locked and unlocked) with the stamp dialog's "Import a key…" request
    /// opening it.
    #[test]
    fn the_key_session_survives_and_the_key_window_draws_headless() {
        let d = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        let mut v = CodeReviewView::default();
        v.set_workspace(Some(d.path()));
        v.keystore = Some(Keystore::at(store.path()));
        assert_eq!(v.key_frame(), None, "locked");
        assert!(v.key_settings_loaded);
        let (_, key) = kovan_common::review::signing::keystore::generate("github:m", "k1", "2026-10-22", "p").unwrap();
        v.key_session.write().unwrap().hold(key);
        let session = v.key_session.clone();
        v.set_workspace(Some(store.path()));
        assert!(std::sync::Arc::ptr_eq(&session, &v.key_session), "kept across workspaces");
        v.set_workspace(Some(d.path()));
        v.keystore = Some(Keystore::at(store.path()));
        assert!(v.key_frame().is_some_and(|left| left <= kovan_common::review::signing::session::DEFAULT_AUTO_LOCK));
        assert!(v.key_status.is_some());

        let ctx = egui::Context::default();
        v.checked = true; // no data build in this test
        v.open_key_panel();
        assert!(v.key_panel.is_some());
        v.key_picked(false, d.path().join("k.kovankey"));
        assert_eq!(v.key_panel.as_ref().unwrap().import_source.as_deref(), Some(d.path().join("k.kovankey").as_path()));
        assert_eq!(v.take_key_pick(), None);
        for _ in 0..3 {
            let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));
        }
        v.key_picked(true, d.path().join("out.kovankey"));
        v.key_panel.as_mut().unwrap().wait();
        v.lock_key();
        assert!(v.key_status.is_none() && session.read().unwrap().status().is_none());
        let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));

        // The stamp dialog's "Import a key from another machine…".
        v.key_panel = None;
        let mut f = StampFlow::new(d.path().into(), Keystore::at(store.path()), Purpose::Stamp, Target {
            function: "crates/a/src/lib.rs::f".into(),
            name: "f".into(),
        });
        f.wait();
        f.wants_key_window = true;
        v.dialog = Some(f);
        let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));
        assert!(v.key_panel.is_some());
        assert!(!v.dialog.as_ref().unwrap().wants_key_window);
    }

    /// Headless (GitHub #1019): Code Review ▸ User management, on the
    /// stamp layer's git fixture with a temporary keystore. The founder
    /// has stamped `twice`, an agent justified `other`, and Bob registered
    /// a key and awaits admission. The tab opens its panel and the
    /// Signing-key panel (drawn inline, no window), loads on workers, and
    /// draws the list, the founder's page and Bob's page with the Admit
    /// form, at desktop and phone width, then (phase 2) Bob admitted as a
    /// novice with a widened scope, his standing history and the standing
    /// controls, without a display or a panic.
    /// Each frame is rasterised in software
    /// ([`super::super::headless_shot`]); with `KOVAN_SHOT_DIR` set the
    /// PNGs are kept there for review.
    ///
    /// Result (2026-10-22): passes.
    #[test]
    fn user_management_tab_draws_headless_and_screenshots() {
        use crate::stamping::justify::{justify, JustifyRequest};
        use crate::stamping::tests::{founder_key, Repo, BY, LIB};
        use crate::stamping::user_management::tests::{bob_registers, B};
        use kovan_common::review::review_md::Verdict;

        let r = Repo::new();
        let store = tempfile::tempdir().unwrap();
        let key = founder_key(&r, store.path());
        r.stamp(&key, "twice", "");
        justify(
            r.path(),
            &JustifyRequest {
                function: format!("{LIB}::other"),
                by: "agent:claude".into(),
                verdict: Verdict::Ok,
                evidence: vec!["cargo test --release: 3 passed".into()],
            },
        )
        .unwrap();
        r.commit("justify other");
        bob_registers(&r, store.path(), "b1");
        let mut v = CodeReviewView {
            workspace: Some(r.path().to_path_buf()),
            checked: true,
            keystore: Some(Keystore::at(store.path())),
            tab: ReviewTab::Users,
            ..CodeReviewView::default()
        };
        let ctx = egui::Context::default();
        let mut shots = super::super::headless_shot::Shooter::default();
        let desk = egui::vec2(1100.0, 1000.0);
        let _ = shots.frame(&ctx, desk, |ui| v.ui(ui));
        assert!(v.users.is_some() && v.key_panel.is_some());
        v.users.as_mut().unwrap().wait();
        v.key_panel.as_mut().unwrap().wait();
        let dir = std::env::var_os("KOVAN_SHOT_DIR").map(PathBuf::from);
        let tmp = tempfile::tempdir().unwrap();
        let out_dir = dir.unwrap_or_else(|| tmp.path().to_path_buf());
        let bg = ctx.global_style().visuals.panel_fill;
        for (name, sel, size) in [
            ("1-roster-founder", BY, desk),
            ("2-awaiting-admission", B, desk),
            ("3-phone-width", B, egui::vec2(390.0, 1400.0)),
        ] {
            v.users.as_mut().unwrap().selected = Some(sel.to_string());
            let _ = shots.frame(&ctx, size, |ui| v.ui(ui));
            let out = shots.frame(&ctx, size, |ui| v.ui(ui));
            shots.save(&ctx, out, size, bg, &out_dir.join(format!("{name}.png")));
            assert!(out_dir.join(format!("{name}.png")).exists());
        }
        // Phase 2: Bob admitted as a novice, then his scope widened; his
        // page shows the standing history and the standing controls.
        use crate::stamping::user_management::{apply_action, UserAction};
        use kovan_common::review::root::Role;
        let today = crate::stamping::user_management::today();
        apply_action(r.path(), &key, &UserAction::Admit { reviewer: B.into(), role: Role::Reviewer, scope: vec!["crates/demo/**".into()], novice: true }, &today).unwrap();
        apply_action(r.path(), &key, &UserAction::ChangeScope { reviewer: B.into(), scope: vec!["crates/demo/**".into(), "crates/njoy-*/**".into()] }, &today).unwrap();
        v.users.as_mut().unwrap().load();
        v.users.as_mut().unwrap().wait();
        v.users.as_mut().unwrap().selected = Some(B.to_string());
        let size = egui::vec2(1100.0, 1200.0);
        let _ = shots.frame(&ctx, size, |ui| v.ui(ui));
        let out = shots.frame(&ctx, size, |ui| v.ui(ui));
        shots.save(&ctx, out, size, bg, &out_dir.join("4-novice-standing.png"));
        let u = v.users.as_ref().unwrap();
        assert!(u.roster().unwrap().reviewer(B).unwrap().role_label.contains("novice"));
        assert_eq!(u.roster().unwrap().agents.len(), 1);
        assert!(v.key_panel.is_some(), "the inline Signing-key panel stays open");
        // Back to the map: the tab is kept across a workspace re-set.
        v.set_workspace(Some(r.path()));
        assert_eq!(v.tab, ReviewTab::Users);
    }

    /// Headless (GitHub #1018; maintainer: "refreshing UI doesn't block
    /// dialog", "starting a stamp should not wait on refresh"), on the
    /// stamp layer's git fixture with a temporary keystore. The view's
    /// refresher is given a manual clock and a gated (slow) runner. After a
    /// first refresh, a slow one is started and held. While it is still
    /// running: the stamp dialog for `leaf` opens and **prepares to the
    /// wizard**, signs, and its Done step is not busy; `leaf` shows
    /// "stamped, updating…" at once; then, back to back, the dialog for
    /// `twice` opens, prepares and signs the same way. Pass: both dialogs
    /// reached the wizard and Done with the slow refresh still running;
    /// the held refresh is superseded and exactly one more runs (three in
    /// all), nothing is outstanding at the end, and both functions end
    /// valid (rung 3), with no "updating" row left.
    ///
    /// Result (2026-10-22): passes.
    #[test]
    fn stamping_never_waits_on_the_background_refresh() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        use crate::stamping::flow::{Step, WizardForm};
        use crate::stamping::refresh::{RefreshRunner, Refresher};
        use crate::stamping::tests::{clean, founder_key, Repo, LIB, PASS};
        use crate::stamping::StampContext;
        use kovan_common::review::signing::session::SessionClock;
        use kovan_common::review::state::StateKind;

        let r = Repo::new();
        let store = tempfile::tempdir().unwrap();
        founder_key(&r, store.path());
        let clock = SessionClock::manual();
        let gate = Arc::new(AtomicBool::new(true));
        let mut v = CodeReviewView {
            workspace: Some(r.path().to_path_buf()),
            review: Some(kovan_web::ui::CodeReview::new(
                kovan_web::Mode::Desktop,
                kovan_web::data::DataSource::Dir { data: r.path().to_path_buf(), workspace: r.path().to_path_buf() },
            )),
            checked: true,
            keystore: Some(Keystore::at(store.path())),
            refresher: Refresher::new(clock.clone(), std::time::Duration::from_secs(1), RefreshRunner::Gated(gate.clone())),
            ..CodeReviewView::default()
        };
        v.load_states();
        let first = v.refresher.settle().expect("the first states");
        v.set_stamps(first);
        // A slow refresh, held running.
        gate.store(false, Ordering::SeqCst);
        v.load_states();
        v.poll_states();
        assert!(v.refresher.busy());
        let ctx = egui::Context::default();
        let fref = |q: &str| FunctionRef {
            id: format!("{LIB}::{q}"),
            krate: "demo".into(),
            file: LIB.into(),
            name: q.into(),
            callees: None,
            lines: None,
        };
        for q in ["leaf", "twice"] {
            v.open(HostRequest::Stamp(fref(q)));
            let d = v.dialog.as_mut().expect("the dialog opens at once");
            d.wait();
            assert_eq!(d.step, Step::Wizard, "{q}: {:?}", d.error);
            assert!(v.refresher.busy(), "{q} prepared while the slow refresh still runs");
            let d = v.dialog.as_mut().unwrap();
            let w = d.wizard.as_mut().unwrap();
            let c = StampContext { answers: clean(), ..w.ctx.clone() };
            *w = WizardForm::new(c);
            w.choose_reason(Some("plumbing"));
            w.passphrase = PASS.into();
            assert!(d.sign());
            d.wait();
            assert!(matches!(d.step, Step::Done(_)), "{q}: {:?} {:?}", d.step, d.error);
            assert!(!d.busy(), "the Done step never waits on a refresh");
            let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));
            let shown = v.refresher.shown().unwrap();
            let row = shown.iter().find(|s| s.function == format!("{LIB}::{q}")).expect("an optimistic row");
            assert_eq!(row.state, Some(StateKind::Updating));
            assert!(v.refresher.busy(), "{q}: the slow refresh is still the one running");
            v.close_dialog();
            r.commit(&format!("stamp {q}"));
        }
        assert_eq!(v.refresher.started, 2);
        gate.store(true, Ordering::SeqCst);
        let last = v.refresher.settle().expect("the final states");
        assert_eq!((v.refresher.started, v.refresher.superseded), (3, 1), "coalesced: one rerun");
        assert!(!v.refresher.outstanding());
        for q in ["leaf", "twice"] {
            let s = last.iter().find(|s| s.function == format!("{LIB}::{q}")).unwrap();
            assert_eq!((s.state, s.rung), (Some(StateKind::Valid), 3), "{q}: {}", s.reason);
        }
        assert!(!last.iter().any(|s| s.state == Some(StateKind::Updating)));
        // The strip draws its Refresh button and status headless.
        v.load_states();
        let _ = ctx.run_ui(Default::default(), |ui| v.ui(ui));
        let _ = v.refresher.settle();
    }
}
