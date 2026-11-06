//! The **Bookmarks** menu at the top right of the top bar (GitHub #1210),
//! beside "⚙ Setup": "Bookmark this page", a fuzzy-find box, and the list,
//! each row a jump with rename and delete.
//!
//! The list, its file and the fuzzy ranking are [`crate::bookmarks`]
//! (GUI-free). What belongs here: what "this page" is
//! ([`DigitiseApp::bookmark_here`]: the function selected in the Code
//! Review, the open paper's PDF at the page in view, or the current
//! [`AppLocation`]), turning a bookmark back into a place
//! ([`target_location`], [`DigitiseApp::jump_to_bookmark`]), and the
//! window.
//!
//! **A jump is a Back/Forward step.** It goes through nav's
//! `apply_location`, and the history records where the frame ended up
//! ([`super::nav`]), so ← returns to where you were. A function jump to the
//! Code Review while already on it moves inside the Code Review, whose own
//! history holds that step.
//!
//! **Mobile-first.** A window anchored under the top bar's right end, no
//! wider than the screen less a 16 px gutter each side; rows wrap, and a
//! jump closes it so a phone shows the page.

use std::path::Path;

use eframe::egui;
use kovan_common::review::signing::keystore::Keystore;

use super::nav::AppLocation;
use super::{DigitiseApp, View};
use crate::bookmarks::{BookmarkStore, BookmarkTarget, StoreState};
use crate::node_id::NodeId;

/// Every view, for the round-trip test of [`view_key`].
#[cfg(test)]
const VIEWS: [View; 12] = [
    View::Home,
    View::Wiki,
    View::Digitiser,
    View::PdfReader,
    View::KvimEditor,
    View::Bibliography,
    View::TableDigitiser,
    View::PlotSetup,
    View::Mindmap,
    View::AdvancedGit,
    View::CodeMap,
    View::CodeReview,
];

/// The stable name of `view` in a bookmark file (never a Rust name, so a
/// renamed variant keeps reading old files).
pub(super) fn view_key(view: View) -> &'static str {
    match view {
        View::Home => "home",
        View::Wiki => "wiki",
        View::Digitiser => "digitiser",
        View::PdfReader => "pdf_reader",
        View::KvimEditor => "kvim_editor",
        View::Bibliography => "bibliography",
        View::TableDigitiser => "table_digitiser",
        View::PlotSetup => "plot_setup",
        View::Mindmap => "mindmap",
        View::AdvancedGit => "advanced_git",
        View::CodeMap => "code_map",
        View::CodeReview => "code_review",
    }
}

/// The view named `key` ([`view_key`]); `None` for a name this version
/// does not know.
pub(super) fn view_from_key(key: &str) -> Option<View> {
    Some(match key {
        "home" => View::Home,
        "wiki" => View::Wiki,
        "digitiser" => View::Digitiser,
        "pdf_reader" => View::PdfReader,
        "kvim_editor" => View::KvimEditor,
        "bibliography" => View::Bibliography,
        "table_digitiser" => View::TableDigitiser,
        "plot_setup" => View::PlotSetup,
        "mindmap" => View::Mindmap,
        "advanced_git" => View::AdvancedGit,
        "code_map" => View::CodeMap,
        "code_review" => View::CodeReview,
        _ => return None,
    })
}

/// A page bookmark of `location`.
pub(super) fn location_target(location: &AppLocation) -> BookmarkTarget {
    BookmarkTarget::Page {
        view: view_key(location.view).to_string(),
        concept: location.concept.as_ref().map(NodeId::to_string),
        paper: location.paper.clone(),
    }
}

/// Where `target` goes, as a location (the page a PDF page or a function
/// is on); `None` for a view this version does not know. A concept that
/// no longer parses opens the top of the map.
pub(super) fn target_location(target: &BookmarkTarget) -> Option<AppLocation> {
    Some(match target {
        BookmarkTarget::Page {
            view,
            concept,
            paper,
        } => AppLocation {
            view: view_from_key(view)?,
            concept: concept.as_deref().and_then(|c| NodeId::parse(c).ok()),
            paper: paper.clone(),
        },
        BookmarkTarget::PdfPage { paper, .. } => AppLocation {
            view: View::PdfReader,
            concept: None,
            paper: Some(paper.clone()),
        },
        BookmarkTarget::Function { .. } => AppLocation {
            view: View::CodeReview,
            concept: None,
            paper: None,
        },
    })
}

/// The window's egui id.
const WINDOW_ID: &str = "bookmarks-window";

/// The menu's state: the store and what is being typed.
pub(super) struct BookmarksMenu {
    pub(super) store: BookmarkStore,
    pub(super) open: bool,
    query: String,
    /// The bookmark being renamed, and the name typed so far.
    renaming: Option<(usize, String)>,
    /// The bookmark whose delete awaits "Yes".
    confirm_delete: Option<usize>,
}

impl Default for BookmarksMenu {
    /// Reads the user from kovan's keystore; under `cargo test` never the
    /// user's (the login is used).
    fn default() -> Self {
        let keystore = if cfg!(test) {
            None
        } else {
            Keystore::default_location().ok()
        };
        BookmarksMenu {
            store: BookmarkStore::new(keystore),
            open: false,
            query: String::new(),
            renaming: None,
            confirm_delete: None,
        }
    }
}

/// What a click in the window asked for.
enum MenuAction {
    Add,
    Jump(BookmarkTarget),
    Rename(usize, String),
    Delete(usize),
    Reload,
}

impl DigitiseApp {
    /// What "Bookmark this page" adds, and its default name: the function
    /// selected in the Code Review, the open paper's PDF at the page in
    /// view, or the current location.
    pub(super) fn bookmark_here(&self) -> (String, BookmarkTarget) {
        let here = self.observed_location();
        if here.view == View::CodeReview {
            if let (Some(ws), Some(f)) = (
                self.code_review.workspace(),
                self.code_review.selected_function(),
            ) {
                return (
                    format!("Code Review: {}", f.name),
                    BookmarkTarget::Function {
                        workspace: ws.display().to_string(),
                        id: f.id,
                        name: f.name,
                    },
                );
            }
        }
        if here.view == View::PdfReader {
            if let Some(paper) = here.paper.clone() {
                let page = self.pdf_reader.page_index() + 1;
                return (
                    format!("{paper}, p. {page}"),
                    BookmarkTarget::PdfPage { paper, page },
                );
            }
        }
        (here.label(), location_target(&here))
    }

    /// Go where `target` points, through nav's `apply_location` (so it is a
    /// Back/Forward step); a PDF page scrolls there, a function opens in
    /// the Code Review of its workspace.
    pub(super) fn jump_to_bookmark(&mut self, target: &BookmarkTarget) {
        let Some(location) = target_location(target) else {
            self.set_error(format!(
                "this bookmark's page is not one this kovan knows: {}",
                target.describe()
            ));
            return;
        };
        if let BookmarkTarget::Function { workspace, id, .. } = target {
            let ws = Path::new(workspace).to_path_buf();
            if self.code_map.workspace() != Some(ws.as_path()) {
                self.code_map.load_workspace(ws.clone());
            }
            self.code_review.open_entry(&ws, id);
        }
        self.apply_location(location);
        if let BookmarkTarget::PdfPage { page, .. } = target {
            self.pdf_reader.go_to_page(page.saturating_sub(1));
        }
    }

    /// Follow the open Kovan folder and take finished reads and writes;
    /// every frame, cheap.
    pub(super) fn poll_bookmarks(&mut self, ctx: &egui::Context) {
        let root = self.home.root().map(|r| r.path().to_path_buf());
        self.bookmarks.store.set_root(root.as_deref());
        self.bookmarks.store.poll();
        if self.bookmarks.store.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    /// The "🔖 Bookmarks" button on the top bar. GUI drawing code (exempt
    /// from the test rule; the list is `crate::bookmarks`, tested there).
    pub(super) fn bookmarks_button(&mut self, ui: &mut egui::Ui) {
        if ui
            .selectable_label(self.bookmarks.open, "\u{1F516} Bookmarks")
            .on_hover_text("Bookmark this page, find a bookmark, jump to it")
            .clicked()
        {
            self.bookmarks.open = !self.bookmarks.open;
            self.bookmarks.renaming = None;
            self.bookmarks.confirm_delete = None;
        }
    }

    /// The Bookmarks window, while open. GUI drawing code (exempt from the
    /// test rule): what it does is `bookmark_here`, `jump_to_bookmark` and
    /// the store's add, rename and delete, each tested. `ui` is the page
    /// under the top bar: the window opens just below the bar.
    pub(super) fn bookmarks_window(&mut self, ui: &egui::Ui) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        self.poll_bookmarks(ctx);
        if !self.bookmarks.open {
            return;
        }
        let (here_name, here) = self.bookmark_here();
        let width = (ctx.content_rect().width() - 32.0).clamp(160.0, 380.0);
        let below_bar = (ui.available_rect_before_wrap().top() - ctx.content_rect().top()).max(0.0);
        let mut open = true;
        let mut action = None;
        let menu = &mut self.bookmarks;
        egui::Window::new("\u{1F516} Bookmarks")
            .id(egui::Id::new(WINDOW_ID))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(width)
            .max_width(width)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, below_bar + 4.0))
            .show(ctx, |ui| {
                ui.set_max_width(width);
                action = menu_ui(ui, menu, &here_name, &here);
            });
        self.bookmarks.open &= open;
        match action {
            Some(MenuAction::Add) => {
                self.bookmarks.store.add(&here_name, here);
            }
            Some(MenuAction::Jump(t)) => {
                self.bookmarks.open = false;
                self.jump_to_bookmark(&t);
            }
            Some(MenuAction::Rename(i, name)) => {
                self.bookmarks.store.rename(i, &name);
            }
            Some(MenuAction::Delete(i)) => {
                self.bookmarks.store.delete(i);
            }
            Some(MenuAction::Reload) => self.bookmarks.store.reload(),
            None => {}
        }
    }
}

/// The window's contents. GUI drawing code (exempt from the test rule; see
/// [`DigitiseApp::bookmarks_window`]).
fn menu_ui(
    ui: &mut egui::Ui,
    menu: &mut BookmarksMenu,
    here_name: &str,
    here: &BookmarkTarget,
) -> Option<MenuAction> {
    let mut action = None;
    match menu.store.state().clone() {
        StoreState::NoFolder => {
            ui.label(
                "Open a Kovan folder to keep bookmarks: they are kept in it, one file per user.",
            );
            return None;
        }
        StoreState::Loading => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Reading bookmarks…");
            });
            return None;
        }
        StoreState::Unreadable(why) => {
            ui.colored_label(
                ui.visuals().error_fg_color,
                "The bookmarks file cannot be read, so it is left as it is:",
            );
            ui.add(egui::Label::new(egui::RichText::new(why).monospace()).wrap());
            ui.label("Fix or move the file, then reload.");
            if ui.button("Reload").clicked() {
                action = Some(MenuAction::Reload);
            }
            return action;
        }
        StoreState::Ready => {}
    }
    if let Some(user) = menu.store.user() {
        ui.weak(format!("Bookmarks of {}", user.as_str()));
    }
    let already = menu.store.bookmarks().find(here);
    ui.horizontal_wrapped(|ui| {
        let add = ui
            .add_enabled(
                already.is_none(),
                egui::Button::new("\u{2795} Bookmark this page"),
            )
            .on_hover_text(format!("Add \u{201C}{here_name}\u{201D}"))
            .on_disabled_hover_text("This page is bookmarked already");
        if add.clicked() {
            action = Some(MenuAction::Add);
        }
    });
    if let Some(e) = &menu.store.save_error {
        ui.colored_label(ui.visuals().error_fg_color, format!("Not saved: {e}"));
    }
    ui.add(
        egui::TextEdit::singleline(&mut menu.query)
            .hint_text("find a bookmark")
            .desired_width(f32::INFINITY),
    );
    ui.separator();
    let bookmarks = menu.store.bookmarks();
    let ranked = bookmarks.ranked(&menu.query);
    let mut cancel = false;
    if bookmarks.items.is_empty() {
        ui.weak("No bookmarks yet.");
    } else if ranked.is_empty() {
        ui.weak("No bookmark matches.");
    }
    egui::ScrollArea::vertical()
        .id_salt("bookmarks-list")
        .max_height((ui.ctx().content_rect().height() * 0.6).max(120.0))
        .show(ui, |ui| {
            for i in ranked {
                let b = &bookmarks.items[i];
                if let Some((n, name)) = &mut menu.renaming {
                    if *n == i {
                        ui.horizontal_wrapped(|ui| {
                            let edit =
                                ui.add(egui::TextEdit::singleline(name).desired_width(180.0));
                            let enter =
                                edit.lost_focus() && ui.input(|k| k.key_pressed(egui::Key::Enter));
                            if ui.button("Save").clicked() || enter {
                                action = Some(MenuAction::Rename(i, name.clone()));
                            }
                            if ui.button("Cancel").clicked() {
                                cancel = true;
                            }
                        });
                        continue;
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    let row = ui
                        .add(egui::Button::new(&b.name).truncate())
                        .on_hover_text(b.target.describe());
                    if row.clicked() {
                        action = Some(MenuAction::Jump(b.target.clone()));
                    }
                    if menu.confirm_delete == Some(i) {
                        ui.label("Delete?");
                        if ui.button("Yes").clicked() {
                            action = Some(MenuAction::Delete(i));
                        }
                        if ui.button("No").clicked() {
                            menu.confirm_delete = None;
                        }
                    } else {
                        if ui
                            .small_button("\u{270F}")
                            .on_hover_text("Rename")
                            .clicked()
                        {
                            menu.renaming = Some((i, b.name.clone()));
                        }
                        if ui
                            .small_button("\u{1F5D1}")
                            .on_hover_text("Delete")
                            .clicked()
                        {
                            menu.confirm_delete = Some(i);
                        }
                    }
                });
            }
        });
    // A rename or delete ends the row's editing; the index changes after.
    if cancel
        || matches!(
            action,
            Some(MenuAction::Rename(..)) | Some(MenuAction::Delete(_))
        )
    {
        menu.renaming = None;
        menu.confirm_delete = None;
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_id::Namespace;

    /// Every view's key reads back as that view, the keys are distinct,
    /// and an unknown key is no view.
    #[test]
    fn view_keys_round_trip() {
        let mut seen = std::collections::BTreeSet::new();
        for v in VIEWS {
            assert_eq!(view_from_key(view_key(v)), Some(v));
            assert!(seen.insert(view_key(v)), "{v:?} shares a key");
        }
        assert_eq!(view_from_key("spaceship"), None);
    }

    /// A location becomes a page bookmark and back unchanged, concept and
    /// paper included; a PDF page and a function go to their pages; a
    /// concept that no longer parses opens the top.
    #[test]
    fn locations_and_bookmarks_convert_both_ways() {
        let loc = AppLocation {
            view: View::Wiki,
            concept: Some(NodeId::concept(Namespace::Library, "reactors/htgr")),
            paper: None,
        };
        assert_eq!(target_location(&location_target(&loc)), Some(loc));
        let paper = AppLocation {
            view: View::KvimEditor,
            concept: None,
            paper: Some("k".into()),
        };
        assert_eq!(target_location(&location_target(&paper)), Some(paper));
        let pdf = BookmarkTarget::PdfPage {
            paper: "k".into(),
            page: 3,
        };
        assert_eq!(
            target_location(&pdf).map(|l| (l.view, l.paper)),
            Some((View::PdfReader, Some("k".into())))
        );
        let f = BookmarkTarget::Function {
            workspace: "/w".into(),
            id: "a.rs::f".into(),
            name: "f".into(),
        };
        assert_eq!(target_location(&f).map(|l| l.view), Some(View::CodeReview));
        let bad = BookmarkTarget::Page {
            view: "mindmap".into(),
            concept: Some("not an id".into()),
            paper: None,
        };
        assert_eq!(target_location(&bad).map(|l| l.concept), Some(None));
        let unknown = BookmarkTarget::Page {
            view: "spaceship".into(),
            concept: None,
            paper: None,
        };
        assert_eq!(target_location(&unknown), None);
    }

    /// "Bookmark this page" on the start page is the Mindmap at its
    /// concept; on the Code Review with nothing selected it is the page;
    /// on the PDF Reader without a paper, the page too.
    #[test]
    fn bookmark_here_is_the_current_location() {
        let mut app = DigitiseApp::default();
        let (name, t) = app.bookmark_here();
        assert_eq!(target_location(&t), Some(app.observed_location()));
        assert!(name.starts_with("Mindmap"), "{name}");
        app.view = View::CodeReview;
        assert_eq!(app.code_review.selected_function(), None);
        assert_eq!(
            app.bookmark_here().1,
            location_target(&app.observed_location())
        );
        app.view = View::PdfReader;
        assert_eq!(app.pdf_reader.page_index(), 0);
        assert!(matches!(app.bookmark_here().1, BookmarkTarget::Page { .. }));
    }

    /// Jumps (the nav test's frame bookkeeping, without drawing): a page
    /// bookmark lands there and is a Back step; a function opens the Code
    /// Review on its workspace and entry; a PDF page asks the reader for
    /// that page; an unknown view is reported and goes nowhere.
    #[test]
    fn a_jump_lands_there_and_back_returns() {
        let frame = |app: &mut DigitiseApp| {
            app.sync_shared_concept();
            app.record_location();
        };
        let mut app = DigitiseApp::default();
        frame(&mut app);
        let start = app.view;
        app.jump_to_bookmark(&BookmarkTarget::Page {
            view: "bibliography".into(),
            concept: None,
            paper: None,
        });
        frame(&mut app);
        assert_eq!(app.view, View::Bibliography);
        app.go_back();
        assert_eq!(app.view, start, "a jump is a Back step");

        let d = tempfile::tempdir().unwrap();
        let ws = d.path().display().to_string();
        app.jump_to_bookmark(&BookmarkTarget::Function {
            workspace: ws.clone(),
            id: "src/lib.rs::f".into(),
            name: "f".into(),
        });
        frame(&mut app);
        assert_eq!(app.view, View::CodeReview);
        assert_eq!(app.code_review.workspace(), Some(d.path()));
        assert_eq!(app.code_map.workspace(), Some(d.path()));

        // No Kovan folder, so the paper cannot open (reported); the reader
        // is still asked for the page.
        app.jump_to_bookmark(&BookmarkTarget::PdfPage {
            paper: "k".into(),
            page: 4,
        });
        assert_eq!(app.view, View::PdfReader);
        assert_eq!(app.pdf_reader.page_index(), 3, "1-based page 4");
        assert!(app.message_is_error);

        app.view = View::Mindmap;
        app.message_is_error = false;
        app.jump_to_bookmark(&BookmarkTarget::Page {
            view: "spaceship".into(),
            concept: None,
            paper: None,
        });
        assert_eq!(app.view, View::Mindmap);
        assert!(app.message_is_error);
    }

    /// Headless: the button and the window draw in every store state
    /// (no folder, then a folder, ready with a bookmark) without a display
    /// or a panic, and the store follows the open folder.
    #[test]
    fn the_menu_draws_headless() {
        let mut app = DigitiseApp::default();
        app.bookmarks.open = true;
        let ctx = egui::Context::default();
        let draw = |app: &mut DigitiseApp| {
            let _ = ctx.run_ui(Default::default(), |ui| {
                app.bookmarks_button(ui);
                app.bookmarks_window(ui);
            });
        };
        draw(&mut app);
        assert_eq!(app.bookmarks.store.state(), &StoreState::NoFolder);

        let d = tempfile::tempdir().unwrap();
        let root =
            crate::root::KovanRoot::create(d.path(), crate::root::RootConfig::new("t", "T"), false)
                .unwrap();
        app.home.open_dir(root.path());
        for _ in 0..2000 {
            draw(&mut app);
            if app.bookmarks.store.state() == &StoreState::Ready {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(app.bookmarks.store.state(), &StoreState::Ready);
        let (name, here) = app.bookmark_here();
        app.bookmarks.store.add(&name, here);
        app.bookmarks.store.add(
            "HTR-10 core, a long name that has to be cut short on a phone",
            BookmarkTarget::PdfPage {
                paper: "wu2002htr10".into(),
                page: 12,
            },
        );
        app.bookmarks.store.add(
            "Pool::rate",
            BookmarkTarget::Function {
                workspace: "/w/ws".into(),
                id: "crates/a/src/lib.rs::Pool::rate".into(),
                name: "Pool::rate".into(),
            },
        );
        app.bookmarks.renaming = Some((0, "x".into()));
        draw(&mut app);
        app.bookmarks.renaming = None;
        app.bookmarks.confirm_delete = Some(0);
        draw(&mut app);
        app.bookmarks.confirm_delete = None;
        assert!(app.bookmarks.open);

        // Pictures at phone and desktop width (KOVAN_SHOT_DIR keeps them),
        // the top bar above; the window fits inside the 16 px gutters.
        let mut shots = super::super::headless_shot::Shooter::default();
        let dir = std::env::var_os("KOVAN_SHOT_DIR").map(std::path::PathBuf::from);
        let tmp = tempfile::tempdir().unwrap();
        let out_dir = dir.unwrap_or_else(|| tmp.path().to_path_buf());
        let shot_ctx = egui::Context::default();
        let bg = shot_ctx.global_style().visuals.panel_fill;
        for (file, size) in [
            ("bookmarks-phone.png", egui::vec2(390.0, 800.0)),
            ("bookmarks-desktop.png", egui::vec2(1100.0, 600.0)),
        ] {
            let mut frame = |app: &mut DigitiseApp| {
                shots.frame(&shot_ctx, size, |ui| {
                    egui::Panel::top("topbar").show(ui, |ui| app.top_bar(ui));
                    app.bookmarks_window(ui);
                })
            };
            for _ in 0..20 {
                let _ = frame(&mut app);
            }
            let out = frame(&mut app);
            shots.save(&shot_ctx, out, size, bg, &out_dir.join(file));
            let rect = shot_ctx
                .memory(|m| m.area_rect(egui::Id::new(WINDOW_ID)))
                .expect("the window was drawn");
            assert!(
                rect.left() >= 0.0 && rect.right() <= size.x,
                "{file}: {rect:?} inside {size:?}"
            );
        }
    }
}
