//! The "Add connection" window of a reviewed function (GitHub #839;
//! maintainer, 2026-10-12): search the standard corpus concepts, click one,
//! and it is recorded in the code repository's
//! `docs/mindmap/connections.toml` ([`crate::code_connections`]).
//!
//! ```text
//!  right-click a function with a valid stamp ── "Add connection…"
//!        │  kovan_web::ui::HostRequest::AddConnection
//!        v
//!  ConnectionPicker ── search ──> standard concepts (stamping::concepts)
//!        │ click a concept
//!        v
//!  worker: code_connections::add_for_reviewed (loads the workspace, checks
//!          the stamp is valid, writes the file)
//!        v
//!  the file joins the session's files, for Commit and push
//! ```
//!
//! The write runs on a worker (it runs the staleness engine); the frame only
//! polls it.

use std::path::{Path, PathBuf};

use kovan_web::ui::FunctionRef;

use crate::code_connections::{self, AddedForFunction, CodeConnection, REL_PATH};
use crate::node_id::{Namespace, NodeId};
use crate::stamping::concepts::{rank, standard_concepts, ConceptChoice};
use crate::stamping::flow::Worker;

type Added = Result<(String, AddedForFunction), String>;

/// The window's state.
pub(crate) struct ConnectionPicker {
    root: PathBuf,
    /// The function the connections are from.
    pub function: FunctionRef,
    pub query: String,
    /// Every standard corpus concept (the choices).
    concepts: Vec<ConceptChoice>,
    /// The function's connections, as last read.
    pub existing: Vec<CodeConnection>,
    /// What the last add did, or why it was refused.
    pub note: Option<Result<String, String>>,
    job: Option<Worker<Added>>,
}

impl ConnectionPicker {
    /// Open the window for `function` in the code repository at `root`.
    pub(crate) fn new(root: &Path, function: FunctionRef) -> ConnectionPicker {
        let mut p = ConnectionPicker {
            root: root.to_path_buf(),
            function,
            query: String::new(),
            concepts: standard_concepts(),
            existing: Vec::new(),
            note: None,
            job: None,
        };
        p.reload();
        p
    }

    /// Read the function's connections again (a small file).
    fn reload(&mut self) {
        self.existing = code_connections::read(&self.root)
            .connections()
            .into_iter()
            .filter(|c| c.path == self.function.id)
            .collect();
    }

    pub(crate) fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// The concepts matching the search, best first.
    pub(crate) fn matches(&self) -> Vec<&ConceptChoice> {
        rank(&self.concepts, &self.query)
    }

    /// Connect the function to the standard concept at `path` (worker).
    pub(crate) fn add(&mut self, path: &str) {
        if self.job.is_some() {
            return;
        }
        let (root, function) = (self.root.clone(), self.function.id.clone());
        let target = NodeId::concept(Namespace::Corpus, path);
        self.job = Some(Worker::spawn(move || {
            code_connections::add_for_reviewed(&root, &function, &target)
                .map(|a| (target.to_string(), a))
                .map_err(|e| e.to_string())
        }));
    }

    /// Take a finished add; true when the file was written.
    pub(crate) fn poll(&mut self) -> bool {
        let Some(r) = self.job.as_ref().and_then(Worker::try_take) else {
            return false;
        };
        self.job = None;
        let wrote = r.is_ok();
        self.note = Some(r.map(|(target, a)| match a.added.replaced_corrupted {
            Some(why) => format!(
                "Connected to {target}. {REL_PATH} was corrupted ({why}) and was created fresh."
            ),
            None if a.added.created => format!("Connected to {target}. Created {REL_PATH}."),
            None => format!("Connected to {target}."),
        }));
        if wrote {
            self.reload();
        }
        wrote
    }

    /// Poll until no job runs (tests only; the UI never waits).
    #[cfg(test)]
    pub(crate) fn wait(&mut self) -> bool {
        let start = std::time::Instant::now();
        let mut wrote = false;
        while self.busy() {
            assert!(start.elapsed() < std::time::Duration::from_secs(120));
            std::thread::sleep(std::time::Duration::from_millis(5));
            wrote |= self.poll();
        }
        wrote
    }
}

/// Draw the window; false once it is closed. GUI drawing code (exempt from
/// the test rule; the state above is tested headless).
pub(crate) fn show(ctx: &egui::Context, p: &mut ConnectionPicker) -> bool {
    let mut open = true;
    let mut chosen = None;
    crate::mindmap::picker_window("Add connection\u{2026}", ctx)
        .id(egui::Id::new("code-review-add-connection"))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(egui::RichText::new(&p.function.id).monospace().strong());
            ui.weak("Connects this reviewed function to a concept of the standard corpus. Not part of its stamp.");
            if !p.existing.is_empty() {
                ui.label("Connected to:");
                for c in &p.existing {
                    ui.monospace(&c.target.path);
                }
                ui.separator();
            }
            ui.horizontal(|ui| {
                ui.label("Search");
                ui.add(egui::TextEdit::singleline(&mut p.query).desired_width(f32::INFINITY));
            });
            match &p.note {
                Some(Ok(m)) => {
                    ui.weak(m);
                }
                Some(Err(e)) => {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
                None => {}
            }
            if p.busy() {
                ui.spinner();
            }
            egui::ScrollArea::vertical()
                .max_height(crate::mindmap::picker_list_height(ui, 8.0))
                .auto_shrink(false)
                .show(ui, |ui| {
                    for c in p.matches() {
                        let linked = p.existing.iter().any(|e| e.target.path == c.path);
                        let label = format!("{}\n{}", c.title, c.path);
                        if ui
                            .add_enabled(!linked && !p.busy(), egui::Button::new(label).wrap())
                            .on_disabled_hover_text("Already connected, or a write is running")
                            .clicked()
                        {
                            chosen = Some(c.path.clone());
                        }
                    }
                });
        });
    if let Some(path) = chosen {
        p.add(&path);
    }
    open
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stamping::tests::{founder_key, Repo, LIB};

    fn function(qual: &str) -> FunctionRef {
        FunctionRef {
            id: format!("{LIB}::{qual}"),
            krate: "demo".into(),
            file: LIB.into(),
            name: qual.into(),
            callees: None,
            lines: None,
        }
    }

    /// Methodology: on the stamping fixture, open the window's state for
    /// `leaf` before it is reviewed and choose the first concept the search
    /// offers; stamp `leaf`, choose again, then draw the window headless.
    ///
    /// Pass: every concept the finder lists is one the connections file
    /// accepts (the two share the standard corpus); the unreviewed function
    /// is refused with the reason and nothing is written; once reviewed the
    /// add writes `docs/mindmap/connections.toml`, the window lists the
    /// connection, and it draws without a display. After `Cargo.lock`
    /// changes the function is "pending workspace test" and a second
    /// connection is still accepted (corrected 2026-10-13: it was refused).
    ///
    /// Result (2026-10-12): passes.
    #[test]
    fn a_reviewed_function_is_connected_to_a_standard_concept() {
        for c in standard_concepts() {
            assert!(crate::corpus::topic_at(&c.path).is_some(), "{} is not a corpus topic", c.path);
        }
        let r = Repo::new();
        let store = tempfile::tempdir().unwrap();
        let key = founder_key(&r, store.path());
        let mut p = ConnectionPicker::new(r.path(), function("leaf"));
        let first = p.matches().first().expect("the corpus has concepts").path.clone();
        p.add(&first);
        assert!(!p.wait());
        assert!(matches!(&p.note, Some(Err(e)) if e.contains("once the function is reviewed")), "{:?}", p.note);
        assert!(!r.path().join(REL_PATH).exists());

        r.stamp(&key, "leaf", "");
        p.add(&first);
        assert!(p.wait());
        assert!(matches!(&p.note, Some(Ok(m)) if m.contains("Created")), "{:?}", p.note);
        assert_eq!(p.existing.len(), 1);
        assert_eq!(p.existing[0].target.path, first);
        assert!(r.path().join(REL_PATH).is_file());

        // `Cargo.lock` changes (an unrelated dependency): the review now
        // awaits a workspace test, and still stands for a connection.
        r.write("Cargo.lock", "version = 4\n");
        r.commit("a dependency changed");
        assert_eq!(
            r.state("leaf").unwrap().kind(),
            kovan_common::review::state::StateKind::PendingWorkspaceTest
        );
        let second = p.matches().iter().map(|c| c.path.clone()).find(|c| *c != first).unwrap();
        p.add(&second);
        assert!(p.wait(), "{:?}", p.note);
        assert_eq!(p.existing.len(), 2);

        let ctx = egui::Context::default();
        let _ = ctx.run_ui(Default::default(), |ui| {
            assert!(show(ui.ctx(), &mut p));
        });
    }
}
