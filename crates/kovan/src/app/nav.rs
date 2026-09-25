//! The app's back/forward navigation (GitHub issue #242, epic #241).
//!
//! What belongs here: what a location *is* for this app ([`AppLocation`]),
//! reading the current one from the app's state, applying a stored one back,
//! the ← / → buttons, and the keyboard/mouse shortcuts. The history rules
//! themselves (a new visit clears Forward, and so on) live in
//! [`crate::navigation::NavHistory`], which has no GUI dependency.
//!
//! # How navigation is recorded: observed, not instrumented
//!
//! Pages change in many places: the top-bar tabs, the Wiki and Mindmap
//! breadcrumbs and drill-ins, opening a paper from either, the Home screen's
//! jump to the Wiki once a folder opens. Rather than touching every one of
//! them, the app reads where it ended up at the end of each frame
//! ([`DigitiseApp::record_location`]) and records a history step when that
//! differs from the last one. A new way to navigate is therefore recorded
//! without any extra code, and a missed call site cannot exist.
//!
//! # The Wiki and the Mindmap share one location
//!
//! They are two views of the same place (maintainer direction, 2026-09-22):
//! switching from one to the other keeps the concept you were on, and moving
//! to a different concept in either moves both.

use super::{DigitiseApp, View};
use crate::node_id::NodeId;
use eframe::egui;

/// One page in the history: which page, and what it was showing.
///
/// `concept` is the concept shown on the Wiki and Mindmap pages, from either
/// the built-in corpus or the user's library (`None` is the top, and it is
/// always `None` on other pages). `paper` is the citekey
/// open on the paper pages (PDF reader, Kvim editor) and `None` elsewhere, so
/// going back to a paper page reopens the paper that was on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AppLocation {
    pub(super) view: View,
    pub(super) concept: Option<NodeId>,
    pub(super) paper: Option<String>,
}

impl AppLocation {
    /// Where the app starts: the Mindmap on the corpus root, so a fresh
    /// Kovan opens on its nuclear-engineering map (maintainer brief,
    /// 2026-09-22).
    pub(super) fn start() -> Self {
        Self {
            view: View::default(),
            concept: crate::mindmap::MindmapState::default().current().cloned(),
            paper: None,
        }
    }

    /// A short human-readable name, for the ← / → tooltips.
    pub(super) fn label(&self) -> String {
        let page = match self.view {
            View::Home => "Home",
            View::Wiki => "Wiki",
            View::Mindmap => "Mindmap",
            View::AdvancedGit => "Save Repository",
            View::Digitiser => "Digitiser",
            View::PdfReader => "PDF Reader",
            View::KvimEditor => "Kvim Editor",
            View::Bibliography => "Bibliography",
            View::TableDigitiser => "Table Digitiser",
            View::PlotSetup => "Digitiser setup",
        };
        match (&self.paper, is_concept_page(self.view), &self.concept) {
            (Some(citekey), _, _) => format!("{page}: {citekey}"),
            (None, true, Some(id)) => format!("{page}: {}", id.path),
            (None, true, None) => format!("{page}: top"),
            (None, false, _) => page.to_string(),
        }
    }
}

/// Pages that show a concept (and so share one location).
fn is_concept_page(view: View) -> bool {
    matches!(view, View::Wiki | View::Mindmap)
}

/// Pages that show the active paper.
fn is_paper_page(view: View) -> bool {
    matches!(view, View::PdfReader | View::KvimEditor)
}

impl DigitiseApp {
    /// The concept the Wiki or Mindmap is showing, if `view` is one of them
    /// (the inner `None` is the top).
    fn concept_on(&self, view: View) -> Option<Option<NodeId>> {
        match view {
            View::Wiki => self.wiki.as_ref().map(|w| w.current().cloned()),
            View::Mindmap => Some(self.mindmap.current().cloned()),
            _ => None,
        }
    }

    /// Show `concept` on both concept pages.
    fn set_shared_concept(&mut self, concept: Option<NodeId>) {
        if let Some(wiki) = self.wiki.as_mut() {
            wiki.set_current(concept.clone());
        }
        self.mindmap.set_current(concept);
    }

    /// Where the app is now, read from its state.
    pub(super) fn observed_location(&self) -> AppLocation {
        AppLocation {
            view: self.view,
            concept: self.concept_on(self.view).flatten(),
            paper: if is_paper_page(self.view) {
                self.active_paper
                    .as_ref()
                    .map(|p| p.session.citekey().to_string())
            } else {
                None
            },
        }
    }

    /// The most recent concept either concept page showed, from the history.
    fn last_shared_concept(&self) -> Option<Option<NodeId>> {
        std::iter::once(self.history.current())
            .chain(self.history.back_entries().iter().rev())
            .find(|l| is_concept_page(l.view))
            .map(|l| l.concept.clone())
    }

    /// Keep the Wiki and Mindmap on one concept when the user switches
    /// between them: a concept page entered this frame from another page
    /// opens on the last concept either of them showed. Called after the top
    /// bar, before the page draws, so the first frame is already right.
    pub(super) fn sync_shared_concept(&mut self) {
        if is_concept_page(self.view) && self.history.current().view != self.view {
            if let Some(concept) = self.last_shared_concept() {
                self.set_shared_concept(concept);
            }
        }
    }

    /// Record where this frame ended up as a history step, if it moved. A
    /// concept change on one concept page is copied to the other first, so
    /// the two never disagree.
    pub(super) fn record_location(&mut self) {
        if let Some(concept) = self.concept_on(self.view) {
            self.set_shared_concept(concept);
        }
        let here = self.observed_location();
        self.history.visit(here);
    }

    /// Show `location`: its page, its concept, and its paper.
    fn apply_location(&mut self, location: AppLocation) {
        if is_concept_page(location.view) {
            self.set_shared_concept(location.concept.clone());
        }
        if let Some(citekey) = &location.paper {
            let already_open = self
                .active_paper
                .as_ref()
                .is_some_and(|p| p.session.citekey() == citekey);
            if !already_open {
                if let Err(e) = self.activate_paper(citekey) {
                    self.set_error(format!("{citekey}: {e}"));
                }
            }
        }
        self.view = location.view;
    }

    /// Step back one page, if there is one.
    pub(super) fn go_back(&mut self) {
        if let Some(location) = self.history.back().cloned() {
            self.apply_location(location);
        }
    }

    /// Step forward one page, if there is one.
    pub(super) fn go_forward(&mut self) {
        if let Some(location) = self.history.forward().cloned() {
            self.apply_location(location);
        }
    }

    /// The ← / → buttons, at the left of the top bar on every page. Each
    /// tooltip names the page it goes to.
    pub(super) fn nav_buttons(&mut self, ui: &mut egui::Ui) {
        let back_to = self.history.back_entries().last().map(AppLocation::label);
        let forward_to = self
            .history
            .forward_entries()
            .last()
            .map(AppLocation::label);
        let back = ui
            .add_enabled(back_to.is_some(), egui::Button::new("⬅"))
            .on_hover_text(format!(
                "Back (Alt+Left){}",
                back_to.map(|l| format!(" to {l}")).unwrap_or_default()
            ));
        let forward = ui
            .add_enabled(forward_to.is_some(), egui::Button::new("➡"))
            .on_hover_text(format!(
                "Forward (Alt+Right){}",
                forward_to.map(|l| format!(" to {l}")).unwrap_or_default()
            ));
        if back.clicked() {
            self.go_back();
        }
        if forward.clicked() {
            self.go_forward();
        }
    }

    /// Alt+Left / Alt+Right and the mouse's back/forward side buttons, as in
    /// a web browser.
    pub(super) fn handle_nav_input(&mut self, ctx: &egui::Context) {
        let (back, forward) = ctx.input(|i| {
            (
                (i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft))
                    || i.pointer.button_pressed(egui::PointerButton::Extra1),
                (i.modifiers.alt && i.key_pressed(egui::Key::ArrowRight))
                    || i.pointer.button_pressed(egui::PointerButton::Extra2),
            )
        });
        if back {
            self.go_back();
        } else if forward {
            self.go_forward();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::wiki::WikiState;
    use super::*;
    use crate::node_id::Namespace;

    /// One frame's worth of navigation bookkeeping, without drawing: the
    /// sync that runs before a page draws and the record that runs after.
    fn frame(app: &mut DigitiseApp) {
        app.sync_shared_concept();
        app.record_location();
    }

    #[test]
    fn switching_pages_records_steps_that_back_and_forward_retrace() {
        let mut app = DigitiseApp::default();
        let start = app.view;
        frame(&mut app);
        app.view = View::Digitiser;
        frame(&mut app);
        app.view = View::Bibliography;
        frame(&mut app);

        app.go_back();
        assert_eq!(app.view, View::Digitiser);
        app.go_back();
        assert_eq!(app.view, start);
        assert!(!app.history.can_go_back());
        app.go_forward();
        assert_eq!(app.view, View::Digitiser);
    }

    /// Kovan opens on the Mindmap, centred on the built-in corpus root, with
    /// no folder open (maintainer brief, 2026-09-22).
    #[test]
    fn kovan_opens_on_the_corpus_map() {
        let app = DigitiseApp::default();
        assert_eq!(app.view, View::Mindmap);
        assert_eq!(
            app.mindmap.current(),
            Some(&NodeId::concept(
                Namespace::Corpus,
                crate::corpus::ROOT_TOPIC
            ))
        );
        assert_eq!(app.history.current().view, View::Mindmap);
    }

    /// Going back and then to a new page discards the pages gone back from,
    /// and redrawing the same page records nothing.
    #[test]
    fn a_new_page_after_back_clears_forward_and_redraws_record_nothing() {
        let mut app = DigitiseApp::default();
        app.view = View::Digitiser;
        frame(&mut app);
        app.view = View::Bibliography;
        frame(&mut app);
        app.go_back();
        frame(&mut app);
        assert!(app.history.can_go_forward());

        app.view = View::TableDigitiser;
        frame(&mut app);
        frame(&mut app);
        frame(&mut app);
        assert!(!app.history.can_go_forward());
        assert_eq!(app.history.back_entries().len(), 2, "Mindmap, Digitiser");
    }

    /// The Wiki and Mindmap are two views of one place, across the corpus
    /// and the user's library: switching between them keeps the concept,
    /// moving in either moves both, and back/forward restores the concept as
    /// well as the page.
    #[test]
    fn the_wiki_and_mindmap_share_one_concept_through_history() {
        let reactors = NodeId::concept(Namespace::Library, "reactors");
        let htgr = NodeId::concept(Namespace::Library, "reactors/htgr");
        let th = NodeId::concept(Namespace::Corpus, "nuclear-engineering/thermal-hydraulics");
        let mut app = DigitiseApp::default();
        app.wiki = Some(WikiState::new());

        // Enter the Wiki (it opens on the shared concept), then move to a
        // library concept there, as a click would.
        app.view = View::Wiki;
        frame(&mut app);
        app.wiki
            .as_mut()
            .unwrap()
            .set_current(Some(reactors.clone()));
        frame(&mut app);

        app.view = View::Mindmap;
        frame(&mut app);
        assert_eq!(
            app.mindmap.current(),
            Some(&reactors),
            "the map opens where the wiki was"
        );

        app.mindmap.set_current(Some(htgr.clone()));
        frame(&mut app);
        assert_eq!(
            app.wiki.as_ref().unwrap().current(),
            Some(&htgr),
            "moving on the map moves the wiki too"
        );

        app.mindmap.set_current(Some(th.clone()));
        frame(&mut app);
        assert_eq!(
            app.wiki.as_ref().unwrap().current(),
            Some(&th),
            "corpus concepts are shared too"
        );

        app.go_back();
        assert_eq!(app.mindmap.current(), Some(&htgr));
        app.go_back();
        assert_eq!(app.view, View::Mindmap);
        assert_eq!(app.mindmap.current(), Some(&reactors));
        assert_eq!(app.wiki.as_ref().unwrap().current(), Some(&reactors));

        app.go_back();
        assert_eq!(app.view, View::Wiki);
        assert_eq!(app.wiki.as_ref().unwrap().current(), Some(&reactors));

        app.go_forward();
        app.go_forward();
        app.go_forward();
        assert_eq!(app.view, View::Mindmap);
        assert_eq!(app.mindmap.current(), Some(&th));
    }
}
