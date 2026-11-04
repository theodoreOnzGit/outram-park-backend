// SPDX-License-Identifier: AGPL-3.0-only
//! **Bookmarks** (GitHub #1210): a browser-style list of places in kovan,
//! each user's own, kept in the Kovan folder. GUI-free; the menu is
//! `app/bookmarks_menu.rs`.
//!
//! A bookmark is a name and a [`BookmarkTarget`]: a page (view, concept,
//! paper), a paper's PDF at a page, or a function in the Code Review. The
//! list adds, renames, deletes and ranks by fuzzy match
//! ([`crate::fuzzy::fuzzy_score`], the finders' matcher, over the name and
//! the target's description).
//!
//! # Where it is kept, and whose it is
//!
//! `<kovan folder>/bookmarks/<user>.toml`, one file per user (maintainer,
//! 2026-11-04: "keep kovan folder, but tag to user"). One file each, so two
//! people sharing a folder through git never edit the same file. Not in
//! `.kovan/`, which is "derived, safe to delete": a bookmark is the user's
//! own work.
//!
//! **The user** ([`resolve_user`]) is the reviewer id of the first key in
//! this machine's kovan keystore (`<kovan config dir>/keys`, sorted by file
//! name), e.g. `github:theodoreOnzGit`: kovan's own identity for a person,
//! the same on every machine the key is carried to, and present whether the
//! key is locked or not. Without a key it is the operating-system login,
//! `login:<name>`. Not the key session's reviewer (it is gone when the key
//! locks, and the bookmarks with it) and not `git config user.email` (a
//! process per lookup, and a Kovan folder need not be a repository). The
//! file name is the id with every character outside `A-Z a-z 0-9 . _ -`
//! made `_`; the id itself is written in the file and checked on reading.
//! One known cost: a user who creates their first key moves from the login
//! file to the reviewer file, and sees the login file's bookmarks no more
//! (the file is kept).
//!
//! # A new file, never a changed one
//!
//! Nothing else in the folder is read or written (kovan schemas never
//! break). A folder without the file has no bookmarks. A file that cannot
//! be read, does not parse, is of a later format or belongs to another
//! user is **reported and kept**: the store refuses changes until it reads
//! cleanly, so a bad file is never overwritten silently. Writes go through
//! a temporary file renamed into place, as `stamping/message_store.rs` and
//! `recent.rs` do.
//!
//! ```text
//!  # kovan bookmarks of github:someone. ...
//!  format = 1
//!  user = "github:someone"
//!
//!  [[bookmark]]
//!  name = "HTR-10 core, p. 12"
//!  [bookmark.target]
//!  kind = "pdf_page"        page | pdf_page | function
//!  paper = "wu2002htr10"
//!  page = 12                1-based, as the reader shows it
//! ```
//!
//! **No lag.** Reading (the keystore and the file) and writing run on a
//! [`Worker`]; [`BookmarkStore::poll`] never blocks.

use std::path::{Path, PathBuf};

use kovan_common::review::signing::keystore::Keystore;
use serde::{Deserialize, Serialize};

use crate::stamping::flow::Worker;

/// The folder, under the Kovan folder, holding one file per user.
pub const DIR: &str = "bookmarks";
/// The file format this version writes and reads.
pub const FORMAT: u32 = 1;

/// Who the bookmarks belong to (module doc): a reviewer id or
/// `login:<name>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserTag(String);

impl UserTag {
    /// The tag as written in the file.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The file name: the tag, every character outside `A-Z a-z 0-9 . _ -`
    /// made `_`, then `.toml`.
    pub fn file_name(&self) -> String {
        let slug: String = self
            .0
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || "._-".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        format!("{slug}.toml")
    }
}

/// The user (module doc): the first of `reviewers` (the keystore's, in its
/// order), else `login:<login>`, else `login:unknown`.
pub fn resolve_user(reviewers: &[String], login: Option<&str>) -> UserTag {
    if let Some(r) = reviewers.iter().find(|r| !r.trim().is_empty()) {
        return UserTag(r.trim().to_string());
    }
    let login = login
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .unwrap_or("unknown");
    UserTag(format!("login:{login}"))
}

/// The reviewers of `keystore`'s keys, in its order, each once; none when
/// it cannot be listed (the login is used instead, so a broken keystore
/// costs only the tag).
pub fn keystore_reviewers(keystore: &Keystore) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for k in keystore.list().unwrap_or_default() {
        if !out.contains(&k.reviewer) {
            out.push(k.reviewer);
        }
    }
    out
}

/// The operating-system login (`USER`, else `USERNAME` on Windows).
pub fn os_login() -> Option<String> {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
        .filter(|s| !s.trim().is_empty())
}

/// Where a bookmark goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BookmarkTarget {
    /// A page of the app: its view (the app's stable key for it), the
    /// concept the Wiki or Mindmap shows (a [`crate::node_id::NodeId`] as
    /// text) and the paper open on a paper page.
    Page {
        view: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        concept: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        paper: Option<String>,
    },
    /// A paper's PDF at a page, 1-based as the reader shows it.
    PdfPage { paper: String, page: usize },
    /// A function in the Code Review: the workspace folder, its call-graph
    /// id (`crates/x/src/a.rs::T::f`) and the name the review bar shows.
    Function {
        workspace: String,
        id: String,
        name: String,
    },
}

impl BookmarkTarget {
    /// One line saying where it goes, for the list and the fuzzy find.
    pub fn describe(&self) -> String {
        match self {
            BookmarkTarget::Page {
                view,
                concept,
                paper,
            } => match (paper, concept) {
                (Some(p), _) => format!("{view}: {p}"),
                (None, Some(c)) => format!("{view}: {c}"),
                (None, None) => view.clone(),
            },
            BookmarkTarget::PdfPage { paper, page } => format!("{paper}, page {page}"),
            BookmarkTarget::Function { workspace, id, .. } => {
                let ws = Path::new(workspace)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned());
                format!("{id} ({})", ws.unwrap_or_else(|| workspace.clone()))
            }
        }
    }
}

/// One bookmark.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bookmark {
    pub name: String,
    pub target: BookmarkTarget,
}

/// A user's bookmarks, in the order they were added.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bookmarks {
    pub items: Vec<Bookmark>,
}

impl Bookmarks {
    /// The bookmark of `target`, if there is one.
    pub fn find(&self, target: &BookmarkTarget) -> Option<usize> {
        self.items.iter().position(|b| &b.target == target)
    }

    /// Add `target` as `name` (trimmed; the target's description when
    /// empty) and return its index. A target already bookmarked is not
    /// added twice: its index is returned and its name kept.
    pub fn add(&mut self, name: &str, target: BookmarkTarget) -> usize {
        if let Some(i) = self.find(&target) {
            return i;
        }
        let name = match name.trim() {
            "" => target.describe(),
            n => n.to_string(),
        };
        self.items.push(Bookmark { name, target });
        self.items.len() - 1
    }

    /// Rename bookmark `i`; refused (`false`) for an empty name or no such
    /// bookmark.
    pub fn rename(&mut self, i: usize, name: &str) -> bool {
        let name = name.trim();
        match self.items.get_mut(i) {
            Some(b) if !name.is_empty() => {
                b.name = name.to_string();
                true
            }
            _ => false,
        }
    }

    /// Delete bookmark `i`, returning it.
    pub fn delete(&mut self, i: usize) -> Option<Bookmark> {
        (i < self.items.len()).then(|| self.items.remove(i))
    }

    /// The indices of the bookmarks matching `query`, best first (a tie
    /// keeps the order added): the better of the name's and the
    /// description's fuzzy score. An empty query gives them all, in order.
    pub fn ranked(&self, query: &str) -> Vec<usize> {
        let mut scored: Vec<(i32, usize)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                let by_name = crate::fuzzy::fuzzy_score(query, &b.name);
                let by_target = crate::fuzzy::fuzzy_score(query, &b.target.describe());
                by_name.max(by_target).map(|s| (s, i))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.into_iter().map(|(_, i)| i).collect()
    }
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    format: u32,
    user: String,
    #[serde(default, rename = "bookmark")]
    bookmarks: Vec<Bookmark>,
}

/// What reading a user's file found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loaded {
    /// No file yet: no bookmarks.
    Missing,
    Read(Bookmarks),
    /// The file is there but cannot be used (why); it is kept as it is.
    Unreadable(String),
}

/// `<root>/bookmarks/<user file>`.
pub fn path(root: &Path, user: &UserTag) -> PathBuf {
    root.join(DIR).join(user.file_name())
}

/// Read `user`'s bookmarks in the Kovan folder `root` (module doc).
pub fn read(root: &Path, user: &UserTag) -> Loaded {
    let p = path(root, user);
    let text = match std::fs::read_to_string(&p) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Loaded::Missing,
        Err(e) => return Loaded::Unreadable(format!("{}: {e}", p.display())),
    };
    let d: OnDisk = match toml::from_str(&text) {
        Ok(d) => d,
        Err(e) => return Loaded::Unreadable(format!("{}: {e}", p.display())),
    };
    if d.format > FORMAT {
        return Loaded::Unreadable(format!(
            "{}: format {} is from a later kovan (this one reads {FORMAT})",
            p.display(),
            d.format
        ));
    }
    if d.user != user.as_str() {
        return Loaded::Unreadable(format!(
            "{}: belongs to {}, not {}",
            p.display(),
            d.user,
            user.as_str()
        ));
    }
    Loaded::Read(Bookmarks { items: d.bookmarks })
}

/// Write `user`'s bookmarks, through a temporary file renamed into place.
pub fn write(root: &Path, user: &UserTag, bookmarks: &Bookmarks) -> Result<(), String> {
    let p = path(root, user);
    let dir = root.join(DIR);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let body = toml::to_string_pretty(&OnDisk {
        format: FORMAT,
        user: user.as_str().to_string(),
        bookmarks: bookmarks.items.clone(),
    })
    .map_err(|e| e.to_string())?;
    let header = format!(
        "# kovan bookmarks of {}. Written by kovan's Bookmarks menu (GitHub #1210).\n",
        user.as_str()
    );
    let tmp = p.with_extension("toml.tmp");
    std::fs::write(&tmp, format!("{header}{body}"))
        .map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &p).map_err(|e| format!("{}: {e}", p.display()))
}

/// What the store can do now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreState {
    /// No Kovan folder is open.
    NoFolder,
    Loading,
    Ready,
    /// The file cannot be used (why); changes are refused until a reload
    /// reads it cleanly.
    Unreadable(String),
}

/// The bookmarks of the open Kovan folder's user, read and written off the
/// UI thread (module doc).
pub struct BookmarkStore {
    /// The keystore the user is read from; `None` uses the login only (as
    /// under `cargo test`, which never reads the user's keystore).
    keystore: Option<Keystore>,
    root: Option<PathBuf>,
    user: Option<UserTag>,
    bookmarks: Bookmarks,
    state: StoreState,
    load: Option<(PathBuf, Worker<(UserTag, Loaded)>)>,
    save: Option<Worker<Result<(), String>>>,
    /// Changed since the last write started.
    dirty: bool,
    /// Why the last write failed, until one succeeds.
    pub save_error: Option<String>,
}

impl BookmarkStore {
    /// A store with no folder, reading the user from `keystore`.
    pub fn new(keystore: Option<Keystore>) -> BookmarkStore {
        BookmarkStore {
            keystore,
            root: None,
            user: None,
            bookmarks: Bookmarks::default(),
            state: StoreState::NoFolder,
            load: None,
            save: None,
            dirty: false,
            save_error: None,
        }
    }

    /// Follow the open Kovan folder: a different one (or none) drops the
    /// bookmarks shown and starts reading its own. Cheap when unchanged, so
    /// the app calls it every frame.
    pub fn set_root(&mut self, root: Option<&Path>) {
        if self.root.as_deref() == root {
            return;
        }
        self.root = root.map(Path::to_path_buf);
        self.reload();
    }

    /// Read the file again (after fixing an unreadable one).
    pub fn reload(&mut self) {
        self.bookmarks = Bookmarks::default();
        self.user = None;
        self.dirty = false;
        let Some(root) = self.root.clone() else {
            self.state = StoreState::NoFolder;
            self.load = None;
            return;
        };
        self.state = StoreState::Loading;
        let keystore = self.keystore.clone();
        let dir = root.clone();
        self.load = Some((
            root,
            Worker::spawn(move || {
                let reviewers = keystore
                    .as_ref()
                    .map(keystore_reviewers)
                    .unwrap_or_default();
                let user = resolve_user(&reviewers, os_login().as_deref());
                let loaded = read(&dir, &user);
                (user, loaded)
            }),
        ));
    }

    /// Take finished reads and writes, and start the next write when
    /// something changed meanwhile. Never blocks.
    pub fn poll(&mut self) {
        if let Some((root, w)) = &self.load {
            if let Some((user, loaded)) = w.try_take() {
                let current = self.root.as_ref() == Some(root);
                self.load = None;
                if current {
                    self.user = Some(user);
                    match loaded {
                        Loaded::Missing => self.state = StoreState::Ready,
                        Loaded::Read(b) => {
                            self.bookmarks = b;
                            self.state = StoreState::Ready;
                        }
                        Loaded::Unreadable(why) => self.state = StoreState::Unreadable(why),
                    }
                }
            }
        }
        if let Some(w) = &self.save {
            if let Some(result) = w.try_take() {
                self.save = None;
                self.save_error = result.err();
            }
        }
        if self.dirty && self.save.is_none() {
            self.start_save();
        }
    }

    fn start_save(&mut self) {
        let (Some(root), Some(user)) = (self.root.clone(), self.user.clone()) else {
            return;
        };
        self.dirty = false;
        let snapshot = self.bookmarks.clone();
        self.save = Some(Worker::spawn(move || write(&root, &user, &snapshot)));
    }

    /// A read or write is running (the app keeps redrawing until done).
    pub fn busy(&self) -> bool {
        self.load.is_some() || self.save.is_some() || self.dirty
    }

    pub fn state(&self) -> &StoreState {
        &self.state
    }

    /// The user the bookmarks belong to, once read.
    pub fn user(&self) -> Option<&UserTag> {
        self.user.as_ref()
    }

    pub fn bookmarks(&self) -> &Bookmarks {
        &self.bookmarks
    }

    /// Change the bookmarks with `f` and write them, only while
    /// [`StoreState::Ready`]; `None` otherwise (nothing changed).
    fn change<T>(&mut self, f: impl FnOnce(&mut Bookmarks) -> T) -> Option<T> {
        if self.state != StoreState::Ready {
            return None;
        }
        let out = f(&mut self.bookmarks);
        self.dirty = true;
        self.poll();
        Some(out)
    }

    /// [`Bookmarks::add`], then write.
    pub fn add(&mut self, name: &str, target: BookmarkTarget) -> Option<usize> {
        self.change(|b| b.add(name, target))
    }

    /// [`Bookmarks::rename`], then write.
    pub fn rename(&mut self, i: usize, name: &str) -> bool {
        self.change(|b| b.rename(i, name)).unwrap_or(false)
    }

    /// [`Bookmarks::delete`], then write.
    pub fn delete(&mut self, i: usize) -> Option<Bookmark> {
        self.change(|b| b.delete(i)).flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(view: &str) -> BookmarkTarget {
        BookmarkTarget::Page {
            view: view.into(),
            concept: None,
            paper: None,
        }
    }

    fn pdf(paper: &str, page: usize) -> BookmarkTarget {
        BookmarkTarget::PdfPage {
            paper: paper.into(),
            page,
        }
    }

    fn function() -> BookmarkTarget {
        BookmarkTarget::Function {
            workspace: "/w/outram-park-backend".into(),
            id: "crates/a/src/lib.rs::Pool::rate".into(),
            name: "Pool::rate".into(),
        }
    }

    /// Poll until the store's reads and writes have finished (each is a
    /// thread; a few milliseconds).
    fn settle(s: &mut BookmarkStore) {
        for _ in 0..2000 {
            s.poll();
            if !s.busy() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("store never settled");
    }

    /// The user is the keystore's first reviewer, else the login, else
    /// `login:unknown`; the file name keeps only safe characters.
    #[test]
    fn the_user_is_the_keystore_reviewer_then_the_login() {
        let r = vec!["".to_string(), "github:someone".to_string()];
        assert_eq!(resolve_user(&r, Some("teddy")).as_str(), "github:someone");
        assert_eq!(resolve_user(&[], Some(" teddy ")).as_str(), "login:teddy");
        assert_eq!(resolve_user(&[], Some("")).as_str(), "login:unknown");
        assert_eq!(resolve_user(&[], None).as_str(), "login:unknown");
        let odd = resolve_user(&["a b/c@x.org".to_string()], None);
        assert_eq!(odd.file_name(), "a_b_c_x.org.toml");
        assert_eq!(
            resolve_user(&["github:someone".into()], None).file_name(),
            "github_someone.toml"
        );
        // The login read never panics (it may be absent on a CI runner).
        let _ = os_login();
    }

    /// The keystore's reviewers, each once, in its order; an empty or
    /// missing keystore gives none.
    #[test]
    fn keystore_reviewers_come_from_the_key_files() {
        use kovan_common::review::signing::keystore::generate;
        let d = tempfile::tempdir().unwrap();
        let ks = Keystore::at(d.path().join("keys"));
        assert!(keystore_reviewers(&ks).is_empty(), "no folder yet");
        for (who, id) in [("github:b", "k1"), ("github:a", "k2"), ("github:b", "k3")] {
            let (file, _) = generate(who, id, "2026-11-04", "pass phrase for tests").unwrap();
            ks.save(&file).unwrap();
        }
        let r = keystore_reviewers(&ks);
        assert_eq!(r.len(), 2);
        assert!(r.contains(&"github:a".to_string()) && r.contains(&"github:b".to_string()));
        assert_eq!(resolve_user(&r, Some("teddy")).as_str(), r[0]);
    }

    /// Add, rename and delete: a target is bookmarked once, an empty name
    /// takes the description, an empty rename is refused, and a bad index
    /// changes nothing.
    #[test]
    fn add_rename_and_delete() {
        let mut b = Bookmarks::default();
        assert_eq!(b.add("  ", pdf("wu2002", 12)), 0);
        assert_eq!(b.items[0].name, "wu2002, page 12");
        assert_eq!(b.add("Mindmap", page("mindmap")), 1);
        assert_eq!(b.add("again", pdf("wu2002", 12)), 0, "not added twice");
        assert_eq!(b.items.len(), 2);
        assert_eq!(b.items[0].name, "wu2002, page 12", "name kept");
        assert!(b.rename(0, " HTR-10 core "));
        assert_eq!(b.items[0].name, "HTR-10 core");
        assert!(!b.rename(0, "   "));
        assert!(!b.rename(9, "x"));
        assert_eq!(b.delete(9), None);
        assert_eq!(b.delete(0).map(|x| x.name), Some("HTR-10 core".into()));
        assert_eq!(b.items.len(), 1);
        assert_eq!(b.find(&page("mindmap")), Some(0));
    }

    /// Every target describes itself in one line.
    #[test]
    fn targets_describe_themselves() {
        assert_eq!(page("Wiki").describe(), "Wiki");
        let c = BookmarkTarget::Page {
            view: "Wiki".into(),
            concept: Some("corpus:a/b".into()),
            paper: None,
        };
        assert_eq!(c.describe(), "Wiki: corpus:a/b");
        let p = BookmarkTarget::Page {
            view: "Kvim Editor".into(),
            concept: None,
            paper: Some("k".into()),
        };
        assert_eq!(p.describe(), "Kvim Editor: k");
        assert_eq!(
            function().describe(),
            "crates/a/src/lib.rs::Pool::rate (outram-park-backend)"
        );
    }

    /// Fuzzy find: matched on the name or the target, best first, a
    /// non-match left out, and an empty query keeps the order added.
    #[test]
    fn fuzzy_find_ranks_names_and_targets() {
        let mut b = Bookmarks::default();
        b.add("Reactivity notes", pdf("wu2002htr10", 3));
        b.add("Pool rate", function());
        b.add("Map top", page("Mindmap"));
        assert_eq!(b.ranked(""), [0, 1, 2]);
        assert_eq!(b.ranked("htr10"), [0], "matched on the paper");
        assert_eq!(b.ranked("rate")[0], 1, "a name prefix ranks first");
        assert_eq!(b.ranked("POOL"), [1], "case-insensitive");
        assert!(b.ranked("zzzz").is_empty());
        assert_eq!(b.ranked("mtop"), [2], "subsequence on the name");
    }

    /// Methodology: the load-old-data proof the issue asks for. A Kovan
    /// folder from before this change (no `bookmarks/`) loads with no
    /// bookmarks and nothing written; one bookmark of each kind written by
    /// this version reads back equal, and the file is the documented TOML.
    /// No temporary file is left behind.
    #[test]
    fn an_old_folder_loads_and_a_written_file_reads_back() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        std::fs::write(root.join("kovan_root.toml"), "schema_version = 1\n").unwrap();
        let user = resolve_user(&["github:someone".into()], None);
        assert_eq!(read(root, &user), Loaded::Missing);
        assert!(!root.join(DIR).exists(), "reading writes nothing");

        let mut b = Bookmarks::default();
        b.add(
            "Map",
            BookmarkTarget::Page {
                view: "mindmap".into(),
                concept: Some("corpus:02-nuclear-safety".into()),
                paper: None,
            },
        );
        b.add("HTR-10 core", pdf("wu2002htr10", 12));
        b.add("", function());
        write(root, &user, &b).unwrap();
        assert_eq!(read(root, &user), Loaded::Read(b.clone()));
        let text = std::fs::read_to_string(path(root, &user)).unwrap();
        assert!(text.starts_with("# kovan bookmarks of github:someone."));
        assert!(
            text.contains("format = 1") && text.contains("kind = \"pdf_page\""),
            "{text}"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("kovan_root.toml")).unwrap(),
            "schema_version = 1\n"
        );
        let names: Vec<_> = std::fs::read_dir(root.join(DIR))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, [std::ffi::OsString::from("github_someone.toml")]);
    }

    /// A file that does not parse, is of a later format, or belongs to
    /// another user is unreadable, never a panic; an unreadable path (a
    /// folder in its place) too.
    #[test]
    fn a_bad_file_is_unreadable_not_a_panic() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        let user = resolve_user(&[], Some("teddy"));
        std::fs::create_dir_all(root.join(DIR)).unwrap();
        for text in [
            "not = [toml",
            "format = 2\nuser = \"login:teddy\"\n",
            "format = 1\nuser = \"login:other\"\n",
            "format = 1\nuser = \"login:teddy\"\n[[bookmark]]\nname = \"x\"\n[bookmark.target]\nkind = \"spaceship\"\n",
        ] {
            std::fs::write(path(root, &user), text).unwrap();
            assert!(matches!(read(root, &user), Loaded::Unreadable(_)), "{text}");
        }
        std::fs::remove_file(path(root, &user)).unwrap();
        std::fs::create_dir(path(root, &user)).unwrap();
        assert!(matches!(read(root, &user), Loaded::Unreadable(_)));
    }

    /// The store, end to end on its workers: no folder, then a folder
    /// without the file (ready, empty); add, rename and delete are written
    /// and survive a new store; a corrupt file is reported, refuses changes
    /// and is kept byte for byte, until it is fixed and reloaded.
    #[test]
    fn the_store_reads_writes_and_keeps_a_corrupt_file() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        let mut s = BookmarkStore::new(None);
        assert_eq!(s.state(), &StoreState::NoFolder);
        assert_eq!(s.add("x", page("Home")), None, "no folder: refused");

        s.set_root(Some(root));
        assert_eq!(s.state(), &StoreState::Loading);
        settle(&mut s);
        assert_eq!(s.state(), &StoreState::Ready);
        let user = s.user().cloned().unwrap();
        assert!(
            user.as_str().starts_with("login:"),
            "no keystore: the login"
        );
        assert_eq!(s.add("Home", page("Home")), Some(0));
        assert_eq!(s.add("Paper", pdf("k", 2)), Some(1));
        assert!(s.rename(1, "Paper, p. 2"));
        assert!(!s.rename(1, ""));
        assert_eq!(s.delete(0).map(|b| b.name), Some("Home".into()));
        settle(&mut s);
        assert_eq!(s.save_error, None);

        let mut again = BookmarkStore::new(None);
        again.set_root(Some(root));
        again.set_root(Some(root));
        settle(&mut again);
        assert_eq!(again.bookmarks().items.len(), 1);
        assert_eq!(again.bookmarks().items[0].name, "Paper, p. 2");

        let corrupt = "format = 1\nuser = [broken\n";
        std::fs::write(path(root, &user), corrupt).unwrap();
        again.reload();
        settle(&mut again);
        assert!(matches!(again.state(), StoreState::Unreadable(_)));
        assert!(again.bookmarks().items.is_empty());
        assert_eq!(
            again.add("x", page("Home")),
            None,
            "refused while unreadable"
        );
        assert!(!again.rename(0, "x"));
        assert_eq!(again.delete(0), None);
        settle(&mut again);
        assert_eq!(
            std::fs::read_to_string(path(root, &user)).unwrap(),
            corrupt,
            "kept as it was"
        );

        std::fs::remove_file(path(root, &user)).unwrap();
        again.reload();
        settle(&mut again);
        assert_eq!(again.state(), &StoreState::Ready);

        again.set_root(None);
        assert_eq!(again.state(), &StoreState::NoFolder);
    }

    /// A write that cannot be made is reported, not a panic: the folder's
    /// `bookmarks` is a file, so the directory cannot be created.
    #[test]
    fn a_failed_write_is_reported() {
        let d = tempfile::tempdir().unwrap();
        let mut s = BookmarkStore::new(None);
        s.set_root(Some(d.path()));
        settle(&mut s);
        std::fs::write(d.path().join(DIR), "in the way").unwrap();
        s.add("Home", page("Home"));
        settle(&mut s);
        assert!(s.save_error.is_some());
    }
}
