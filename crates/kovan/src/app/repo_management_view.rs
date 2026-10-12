//! The **Repository management** controls on the page of that name (GitHub
//! #840, #841; maintainer, 2026-10-12): the Kovan folder's repositories by
//! kind, "Add repository…" for an open corpus, a proprietary corpus or a
//! Rust code repository, and, for a code repository only, **Force add,
//! commit and push**, which asks first.
//!
//! ```text
//!  [ Add repository… ]                      (a window: kind, remote, name)
//!  Open corpus         open            literature/open-corpus
//!  Proprietary corpus  proprietary     literature/proprietary
//!  Code repository     my-code         code/my-code   [ Force add, commit and push… ]
//!                                                       └ a window: the warning, the
//!                                                         branch, what would be added,
//!                                                         a note, "Yes, …" / Cancel
//! ```
//!
//! Save Repository (the rest of the page) never commits or pushes a code
//! repository; the forced action here is the only way kovan does. The
//! logic is [`crate::repo_management`]; adding and pushing run on worker
//! threads (network git), and the frame only polls them.

use std::path::PathBuf;

use egui::RichText;

use crate::repo_management::{
    add_repository, default_path, force_add_commit_push, force_preview, name_from_remote,
    AddedRepository, ForcePushed, NewRepository, RepoKind,
};
use crate::root::KovanRoot;
use crate::stamping::flow::Worker;

/// The "Add repository" window's fields.
struct AddForm {
    kind: RepoKind,
    remote: String,
    name: String,
    /// The name still follows the remote (the user has not typed one).
    name_follows: bool,
    branch: String,
}

/// The "Force add, commit and push" window of one code repository.
struct ForcePrompt {
    name: String,
    dir: PathBuf,
    /// The branch and the changes, or why it cannot be done.
    preview: Option<Result<(String, Vec<String>), String>>,
    note: String,
}

enum Job {
    Add(Worker<Result<AddedRepository, String>>),
    Preview(Worker<Result<(String, Vec<String>), String>>),
    Force(Worker<Result<ForcePushed, String>>),
}

/// The controls' state; owned by the app beside the Save page's.
#[derive(Default)]
pub(crate) struct RepoManagementView {
    add: Option<AddForm>,
    force: Option<ForcePrompt>,
    job: Option<Job>,
    /// What the last action did, or why it was refused.
    pub message: Option<Result<String, String>>,
}

impl RepoManagementView {
    pub(crate) fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// Open the "Add repository" window.
    pub(crate) fn open_add(&mut self) {
        self.add = Some(AddForm {
            kind: RepoKind::Code,
            remote: String::new(),
            name: String::new(),
            name_follows: true,
            branch: String::new(),
        });
    }

    /// Start adding the form's repository to the Kovan folder (worker).
    fn start_add(&mut self, root: &KovanRoot) {
        let (Some(f), None) = (&self.add, &self.job) else { return };
        let new = NewRepository {
            kind: f.kind,
            name: f.name.trim().to_string(),
            remote: f.remote.trim().to_string(),
            path: None,
            branch: Some(f.branch.trim().to_string()).filter(|b| !b.is_empty()),
        };
        let dir = root.path().to_path_buf();
        self.job = Some(Job::Add(Worker::spawn(move || add_repository(&dir, &new))));
    }

    /// Open the forced-push prompt for the code repository `name` at `dir`;
    /// what would be added is read on a worker.
    pub(crate) fn open_force(&mut self, name: &str, dir: PathBuf) {
        if self.job.is_some() {
            return;
        }
        let d = dir.clone();
        self.job = Some(Job::Preview(Worker::spawn(move || force_preview(&d))));
        self.force = Some(ForcePrompt {
            name: name.to_string(),
            dir,
            preview: None,
            note: String::new(),
        });
    }

    /// The user confirmed: force add, commit and push (worker).
    fn confirm_force(&mut self) {
        let (Some(p), None) = (&self.force, &self.job) else { return };
        let (dir, note) = (p.dir.clone(), p.note.clone());
        self.job = Some(Job::Force(Worker::spawn(move || force_add_commit_push(&dir, &note))));
    }

    /// Take a finished job; true when a repository was added (the app then
    /// reloads the Kovan folder). Never blocks.
    pub(crate) fn poll(&mut self) -> bool {
        let mut added = false;
        match self.job.take() {
            None => {}
            Some(Job::Add(w)) => match w.try_take() {
                None => self.job = Some(Job::Add(w)),
                Some(Ok(a)) => {
                    let mut m = format!(
                        "Added the {} {:?} at {}.",
                        a.kind.label().to_lowercase(),
                        a.name,
                        a.path.display()
                    );
                    for n in &a.notes {
                        m.push(' ');
                        m.push_str(n);
                    }
                    self.message = Some(Ok(m));
                    self.add = None;
                    added = true;
                }
                Some(Err(e)) => self.message = Some(Err(e)),
            },
            Some(Job::Preview(w)) => match w.try_take() {
                None => self.job = Some(Job::Preview(w)),
                Some(r) => {
                    if let Some(p) = &mut self.force {
                        p.preview = Some(r);
                    }
                }
            },
            Some(Job::Force(w)) => match w.try_take() {
                None => self.job = Some(Job::Force(w)),
                Some(Ok(done)) => {
                    let what = match &done.commit {
                        Some(c) => format!("committed {}", c.chars().take(10).collect::<String>()),
                        None => "nothing to commit".to_string(),
                    };
                    self.message = Some(Ok(format!(
                        "Force add, commit and push: {what}; pushed {} to {}.",
                        done.branch, done.remote
                    )));
                    self.force = None;
                }
                Some(Err(e)) => {
                    self.message = Some(Err(e));
                    self.force = None;
                }
            },
        }
        added
    }

    /// Poll until no job runs (tests only; the UI never waits).
    #[cfg(test)]
    fn wait(&mut self) -> bool {
        let start = std::time::Instant::now();
        let mut added = false;
        while self.busy() {
            assert!(start.elapsed() < std::time::Duration::from_secs(120));
            std::thread::sleep(std::time::Duration::from_millis(5));
            added |= self.poll();
        }
        added
    }
}

/// Draw the controls and their windows; true when a repository was added.
/// GUI drawing code (exempt from the test rule; the state above and
/// `crate::repo_management` are tested).
pub(crate) fn show(ui: &mut egui::Ui, v: &mut RepoManagementView, root: &KovanRoot) -> bool {
    let added = v.poll();
    if v.busy() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
    }
    let mut force = None;
    egui::CollapsingHeader::new(RichText::new("Repositories").strong())
        .id_salt("repo-management-list")
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button("Add repository\u{2026}")
                    .on_hover_text("Add an open corpus, a proprietary corpus or a Rust code repository as a submodule of this Kovan folder")
                    .clicked()
                {
                    v.open_add();
                }
                if v.busy() {
                    ui.spinner();
                }
            });
            match &v.message {
                Some(Ok(m)) => {
                    ui.weak(m);
                }
                Some(Err(e)) => {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
                None => {}
            }
            egui::Grid::new("repo-management-grid").num_columns(4).striped(true).show(ui, |ui| {
                for r in root.corpus_repos() {
                    ui.label(r.tier.label());
                    ui.monospace(&r.name);
                    ui.weak(r.rel.display().to_string());
                    ui.weak(if r.is_downloaded() { "" } else { "not downloaded" });
                    ui.end_row();
                }
                for c in root.code_repos() {
                    ui.label("Code repository");
                    ui.monospace(&c.name);
                    ui.weak(c.rel.display().to_string());
                    if ui
                        .add_enabled(c.is_downloaded() && !v.busy(), egui::Button::new("Force add, commit and push\u{2026}").small())
                        .on_hover_text("Save Repository never commits or pushes a code repository. This adds EVERY change in it, commits and pushes; it asks first.")
                        .on_disabled_hover_text("Not downloaded, or another action is running")
                        .clicked()
                    {
                        force = Some((c.name.clone(), c.dir.clone()));
                    }
                    ui.end_row();
                }
            });
            ui.weak("Save Repository below never commits or pushes a code repository.");
        });
    if let Some((name, dir)) = force {
        v.open_force(&name, dir);
    }
    add_window(ui.ctx(), v, root);
    force_window(ui.ctx(), v);
    added
}

/// The "Add repository" window. GUI drawing code (exempt).
fn add_window(ctx: &egui::Context, v: &mut RepoManagementView, root: &KovanRoot) {
    let busy = v.busy();
    let Some(f) = &mut v.add else { return };
    let (mut open, mut go, mut cancel) = (true, false, false);
    egui::Window::new("Add repository")
        .id(egui::Id::new("repo-management-add"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(egui::vec2(460.0, 260.0))
        .constrain(true)
        .show(ctx, |ui| {
            ui.label("Added as a git submodule of this Kovan folder, if it is kovan compatible.");
            ui.horizontal_wrapped(|ui| {
                for k in RepoKind::ALL {
                    ui.selectable_value(&mut f.kind, k, k.label());
                }
            });
            ui.weak(match f.kind {
                RepoKind::OpenCorpus => "Openly licensed literature. Save Repository commits and pushes it.",
                RepoKind::ProprietaryCorpus => "Proprietary literature: the remote must be private. Save Repository commits and pushes it.",
                RepoKind::Code => "A Rust source code repository (a Cargo.toml at its root). Save Repository never commits or pushes it.",
            });
            egui::Grid::new("repo-management-add-grid").num_columns(2).show(ui, |ui| {
                ui.label("Remote URL");
                if ui.add(egui::TextEdit::singleline(&mut f.remote).desired_width(320.0)).changed() && f.name_follows {
                    f.name = name_from_remote(&f.remote);
                }
                ui.end_row();
                ui.label("Name");
                if ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(320.0)).changed() {
                    f.name_follows = false;
                }
                ui.end_row();
                ui.label("Branch (optional)");
                ui.add(egui::TextEdit::singleline(&mut f.branch).desired_width(320.0));
                ui.end_row();
                ui.label("Mounted at");
                ui.monospace(default_path(f.kind, f.name.trim()).display().to_string());
                ui.end_row();
            });
            ui.horizontal(|ui| {
                let ready = !busy && !f.remote.trim().is_empty() && !f.name.trim().is_empty();
                if ui.add_enabled(ready, egui::Button::new("Add")).clicked() {
                    go = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
                if busy {
                    ui.spinner();
                    ui.weak("Checking and cloning\u{2026}");
                }
            });
        });
    if go {
        v.start_add(root);
    } else if cancel || !open {
        v.add = None;
    }
}

/// The confirmation before a forced add, commit and push. GUI drawing code
/// (exempt).
fn force_window(ctx: &egui::Context, v: &mut RepoManagementView) {
    let busy = v.busy();
    let Some(p) = &mut v.force else { return };
    let (mut open, mut yes, mut cancel) = (true, false, false);
    egui::Window::new("Force add, commit and push?")
        .id(egui::Id::new("repo-management-force"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(egui::vec2(480.0, 360.0))
        .constrain(true)
        .show(ctx, |ui| {
            ui.label(RichText::new(format!("Code repository {:?}", p.name)).strong());
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "This adds EVERY change in the code repository (new, edited and deleted files, \
                 whoever made them), commits them and pushes the branch. It is not Save \
                 Repository, and it cannot be undone from kovan. Do you really want to?",
            );
            match &p.preview {
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.weak("Reading what would be added\u{2026}");
                    });
                }
                Some(Err(e)) => {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
                Some(Ok((branch, changes))) => {
                    ui.label(format!("Branch: {branch}. {} change(s) would be added:", changes.len()));
                    egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                        for c in changes.iter().take(500) {
                            ui.monospace(c);
                        }
                        if changes.len() > 500 {
                            ui.weak(format!("\u{2026} and {} more", changes.len() - 500));
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Commit note");
                        ui.add(egui::TextEdit::singleline(&mut p.note).desired_width(f32::INFINITY));
                    });
                }
            }
            ui.horizontal(|ui| {
                let ready = !busy && matches!(p.preview, Some(Ok(_)));
                if ui.add_enabled(ready, egui::Button::new("Yes, add, commit and push everything")).clicked() {
                    yes = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if yes {
        v.confirm_force();
    } else if (cancel || !open) && !matches!(v.job, Some(Job::Force(_))) {
        v.force = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::root::RootConfig;

    /// Headless: on a temporary Kovan folder the controls draw with the
    /// "Add repository" window open, and adding from a remote that does not
    /// exist is refused with the reason on a worker, records nothing and
    /// reports no repository added. The forced push prompt on a folder that
    /// is not a repository shows why and, confirmed or not, changes nothing.
    /// No network: the remote is a local path that does not exist.
    #[test]
    fn the_controls_draw_and_report_refusals_headless() {
        let d = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(d.path(), RootConfig::new("t", "t"), true).unwrap();
        let mut v = RepoManagementView::default();
        let ctx = egui::Context::default();
        let draw = |v: &mut RepoManagementView| {
            let mut added = false;
            let _ = ctx.run_ui(Default::default(), |ui| {
                added = show(ui, v, &root);
            });
            added
        };
        assert!(!draw(&mut v));
        v.open_add();
        let f = v.add.as_mut().unwrap();
        f.remote = d.path().join("no-such-remote.git").to_string_lossy().into_owned();
        f.name = name_from_remote(&f.remote);
        assert_eq!(f.name, "no-such-remote");
        v.start_add(&root);
        assert!(v.busy());
        assert!(!draw(&mut v));
        assert!(!v.wait());
        assert!(matches!(&v.message, Some(Err(e)) if e.contains("could not be read")), "{:?}", v.message);
        assert!(KovanRoot::open(d.path()).unwrap().code_repos().is_empty());

        let not_a_repo = tempfile::tempdir().unwrap();
        v.open_force("x", not_a_repo.path().to_path_buf());
        v.wait();
        assert!(matches!(v.force.as_ref().unwrap().preview, Some(Err(_))));
        assert!(!draw(&mut v));
        v.confirm_force();
        v.wait();
        assert!(v.force.is_none() && matches!(v.message, Some(Err(_))));
    }
}
