//! The egui Code Review UI, [`CodeReview`]: one UI for web-kovan and
//! desktop kovan, parameterised by [`Mode`].
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │ [←][→][↑] outram-park › boon-lay › … › live_pools   [search] │  top bar
//! ├─────────────┬──────────────────────────────┬─────────────────┤
//! │ side panel  │  canvas: map / crate / module│ source panel    │
//! │ (« Hide;    │  [+][−][Fit][100%]           │ (phone: full-   │
//! │ folded on a │                              │  screen sheet,  │
//! │ phone)      │                              │  ‹ Back)        │
//! ├─────────────┴──────────────────────────────┴─────────────────┤
//! │ review bar: name · unreviewed · M1 · ▸ blocked by N · Stamp  │
//! └──────────────────────────────────────────────────────────────┘
//! ```
//!
//! Mobile-first: zoom buttons on the canvas, a collapsible side panel
//! folded under 700 pt, finger-sized buttons, status drawn on the canvas.
//! No lag: every file loads in the background ([`crate::data`]); the frame
//! only reads snapshots.

mod bar;
mod camera;
mod canvas;
mod source;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use egui::{Color32, RichText};
use kovan_common::call_graph::split::{CrateSlice, SearchIndex, SplitIndex, StampState};
use kovan_common::code_map::layout::Layout;
use kovan_common::code_map::CodeMap;

pub use bar::{review_bar, BarAction, BarInfo};
pub use camera::Camera;

use crate::data::{BuildInfo, DataSource, Load, SiteLinks, Store, Timing};
use crate::model::{self, crate_of, file_of, DeepLink, Dir, Facts};
use crate::search::{search, Hit as SearchHit, SearchResult};
use crate::Mode;

/// Screens narrower than this open with the side panel folded, and show
/// the source as a full-screen sheet (points).
pub const NARROW: f32 = 700.0;

/// Which canvas is on screen.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Map,
    Crate(String),
    Module { krate: String, file: String },
}

/// A place in the back/forward history: the canvas, the selected function
/// and the unfolded call levels.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub level: Level,
    pub selected: Option<String>,
    pub expanded: BTreeSet<(String, Dir)>,
}

impl Place {
    fn at(level: Level) -> Place {
        Place { level, selected: None, expanded: BTreeSet::new() }
    }
}

/// What is under the pointer on a canvas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Hit {
    MapCrate(String),
    MapToggle(String),
    MapModule { krate: String, file: String },
    TreeModule(String),
    TreeToggle(String),
    Function(String),
    FnToggle(String, Dir),
    ModuleCard,
    /// A row or fidelity label on the map, with what it means.
    Label(String),
}

/// Everything loaded so far, taken once per frame.
pub(crate) struct Snap {
    pub map: Option<Arc<CodeMap>>,
    pub layout: Option<Arc<Layout>>,
    pub index: Option<Arc<SplitIndex>>,
    pub search: Option<Arc<SearchIndex>>,
    /// Missing in a local build: then the map offers no deep dives.
    pub site_links: Option<Arc<SiteLinks>>,
    pub build: Option<Arc<BuildInfo>>,
    pub crates: BTreeMap<String, Load<CrateSlice>>,
    pub api: BTreeMap<String, Arc<BTreeSet<String>>>,
    pub files: BTreeMap<String, Load<String>>,
    pub timings: Vec<Timing>,
    pub errors: Vec<String>,
    /// Stamp states the host pushed ([`CodeReview::set_stamps`]); when set
    /// they replace the index's `stamps`.
    pub stamps: Option<Arc<Vec<StampState>>>,
}

impl Snap {
    pub fn slice(&self, krate: &str) -> Option<&Arc<CrateSlice>> {
        self.crates.get(krate).and_then(|l| l.ready())
    }
    /// The slice holding function `id`'s crate.
    pub fn slice_of(&self, id: &str) -> Option<&Arc<CrateSlice>> {
        let map = self.map.as_ref()?;
        self.slice(crate_of(map, id)?)
    }
    pub fn facts(&self) -> Facts {
        match &self.stamps {
            Some(s) => Facts::new(s),
            None => Facts::new(self.index.as_ref().map(|i| i.stamps.as_slice()).unwrap_or(&[])),
        }
    }
}

/// The function a [`HostRequest`] is about: everything the host needs to
/// find it and draft a review entry, as the UI knows it when the button is
/// pressed (GitHub #740, #770).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionRef {
    /// The call-graph id, `crates/x/src/a.rs::T::f` (the key of
    /// [`StampState::function`]).
    pub id: String,
    /// The workspace crate holding it (code map name).
    pub krate: String,
    /// Its file, workspace-relative with `/` separators.
    pub file: String,
    /// The name the review bar shows (`T::f`).
    pub name: String,
    /// The workspace functions it calls (ids), as the "blocked by" list
    /// counts them; `None` while the crate's slice is still loading, never
    /// an empty list standing in for "not known yet".
    pub callees: Option<Vec<String>>,
    /// `(start_line, end_line)`, 1-based and inclusive (doc comment
    /// included), when the slice is loaded.
    pub lines: Option<(u32, u32)>,
}

/// What the reader asked the host to do ([`Mode::Desktop`] only); taken
/// with [`CodeReview::take_host_request`] after [`CodeReview::show`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostRequest {
    /// The review bar's Stamp.
    Stamp(FunctionRef),
    /// The review bar's Needs fix.
    NeedsFix(FunctionRef),
}

/// The Code Review UI.
pub struct CodeReview {
    pub mode: Mode,
    store: Store,
    started: bool,
    place: Place,
    history: Vec<Place>,
    cursor: usize,
    /// The crate view's selected module (file).
    selected_module: Option<String>,
    /// Crates whose top-level modules are shown on the map.
    map_expanded: BTreeSet<String>,
    /// Modules whose subtree is folded in the crate view (files).
    collapsed: BTreeSet<String>,
    show_tests: bool,
    cams: BTreeMap<Level, Camera>,
    panel_open: bool,
    panel_decided: bool,
    /// The function whose source is shown.
    source: Option<String>,
    bar_expanded: bool,
    menu: Option<Hit>,
    /// The code map's left-click choice (2026-10-06): the crate, where the
    /// popup opens, and the pass it opened on.
    crate_choice: Option<(String, egui::Pos2, u64)>,
    query: String,
    results: Vec<SearchResult>,
    results_for: String,
    focus_search: bool,
    status: Option<String>,
    last_hash: String,
    pending: Option<DeepLink>,
    /// The last Stamp / Needs-fix press, until the host takes it.
    host_request: Option<HostRequest>,
    /// Stamp states the host pushed ([`Self::set_stamps`]).
    stamps: Option<Arc<Vec<StampState>>>,
}

impl CodeReview {
    /// The UI over `source`'s data, opening at the page's deep link.
    pub fn new(mode: Mode, source: DataSource) -> CodeReview {
        let link = DeepLink::parse(&crate::platform::hash());
        CodeReview {
            mode,
            store: Store::new(source),
            started: false,
            place: Place::at(Level::Map),
            history: vec![Place::at(Level::Map)],
            cursor: 0,
            selected_module: None,
            map_expanded: BTreeSet::new(),
            collapsed: BTreeSet::new(),
            show_tests: false,
            cams: BTreeMap::new(),
            panel_open: true,
            panel_decided: false,
            source: None,
            bar_expanded: false,
            menu: None,
            crate_choice: None,
            query: String::new(),
            results: Vec::new(),
            results_for: String::new(),
            focus_search: false,
            status: None,
            last_hash: crate::platform::hash(),
            pending: (link != DeepLink::Map).then_some(link),
            host_request: None,
            stamps: None,
        }
    }

    /// The Stamp or Needs-fix press since the last call ([`Mode::Desktop`];
    /// always `None` in [`Mode::Web`]). The host calls this after
    /// [`Self::show`] each frame and opens its dialog.
    pub fn take_host_request(&mut self) -> Option<HostRequest> {
        self.host_request.take()
    }

    /// Replace the stamp states shown (the data's `stamps` until now), e.g.
    /// after the host wrote a stamp, so the bar and the cards recolour
    /// without reloading the data.
    pub fn set_stamps(&mut self, stamps: Vec<StampState>) {
        self.stamps = Some(Arc::new(stamps));
    }

    fn repaint(ctx: &egui::Context) -> impl Fn() + Clone + Send + 'static {
        let ctx = ctx.clone();
        move || ctx.request_repaint()
    }

    fn snapshot(&self) -> Option<Snap> {
        let s = self.store.shared.try_read().ok()?;
        fn ready<T>(l: &Option<Load<T>>) -> Option<Arc<T>> {
            l.as_ref().and_then(|l| l.ready()).cloned()
        }
        let mut errors = Vec::new();
        if let Some(Load::Failed(e)) = &s.code_map {
            errors.push(e.clone());
        }
        if let Some(Load::Failed(e)) = &s.index {
            errors.push(format!("no call graph in this build ({e})"));
        }
        Some(Snap {
            map: ready(&s.code_map),
            layout: s.layout.clone(),
            index: ready(&s.index),
            search: ready(&s.search),
            site_links: ready(&s.site_links),
            build: ready(&s.build),
            crates: s.crates.clone(),
            api: s.api.iter().filter_map(|(k, v)| v.ready().map(|v| (k.clone(), v.clone()))).collect(),
            files: s.files.clone(),
            timings: s.timings.clone(),
            errors,
            stamps: self.stamps.clone(),
        })
    }

    // ---- navigation -------------------------------------------------------

    /// Go to `place`, as a new history entry.
    pub(crate) fn go(&mut self, place: Place) {
        if place == self.place {
            return;
        }
        self.history.truncate(self.cursor + 1);
        self.history.push(place.clone());
        self.cursor = self.history.len() - 1;
        self.enter(place);
    }

    /// Change the current place without a new history entry (selection).
    pub(crate) fn replace(&mut self, place: Place) {
        self.history[self.cursor] = place.clone();
        self.enter(place);
    }

    fn enter(&mut self, place: Place) {
        if let Level::Module { file, .. } = &place.level {
            self.selected_module = Some(file.clone());
        }
        self.place = place;
        self.menu = None;
        self.sync_hash();
    }

    fn back(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            let p = self.history[self.cursor].clone();
            self.enter(p);
        }
    }

    fn forward(&mut self) {
        if self.cursor + 1 < self.history.len() {
            self.cursor += 1;
            let p = self.history[self.cursor].clone();
            self.enter(p);
        }
    }

    fn up(&mut self) {
        let level = match &self.place.level {
            Level::Map => return,
            Level::Crate(_) => Level::Map,
            Level::Module { krate, .. } => Level::Crate(krate.clone()),
        };
        self.go(Place::at(level));
    }

    fn link(&self) -> DeepLink {
        match (&self.place.level, &self.place.selected) {
            (Level::Map, _) => DeepLink::Map,
            (Level::Crate(c), _) => DeepLink::Crate(c.clone()),
            (Level::Module { .. }, Some(id)) => DeepLink::Function { id: id.clone(), source: self.source.as_deref() == Some(id) },
            (Level::Module { file, .. }, None) => DeepLink::Module(file.clone()),
        }
    }

    fn sync_hash(&mut self) {
        let h = self.link().to_hash();
        if h != self.last_hash {
            crate::platform::set_hash(&h);
            self.last_hash = h;
        }
    }

    /// Open a deep link (needs the code map to find a file's crate).
    fn open_link(&mut self, link: DeepLink, map: &CodeMap) {
        match link {
            DeepLink::Map => self.go(Place::at(Level::Map)),
            DeepLink::Crate(c) => self.go(Place::at(Level::Crate(c))),
            DeepLink::Module(file) => {
                if let Some(k) = crate_of(map, &file) {
                    self.go(Place::at(Level::Module { krate: k.to_string(), file }));
                }
            }
            DeepLink::Function { id, source } => self.open_function(&id, source, map),
        }
    }

    /// Go into function `id`: its module, with it selected.
    pub(crate) fn open_function(&mut self, id: &str, source: bool, map: &CodeMap) {
        let Some(k) = crate_of(map, id) else {
            self.status = Some(format!("{id}: not in a workspace crate"));
            return;
        };
        let level = Level::Module { krate: k.to_string(), file: file_of(id).to_string() };
        let keep = if self.place.level == level { self.place.expanded.clone() } else { BTreeSet::new() };
        self.go(Place { level, selected: Some(id.to_string()), expanded: keep });
        if source {
            self.source = Some(id.to_string());
            self.sync_hash();
        }
    }

    fn choose(&mut self, r: &SearchResult, map: &CodeMap) {
        match &r.hit {
            SearchHit::Crate(c) => self.go(Place::at(Level::Crate(c.clone()))),
            SearchHit::Module { krate, file } => self.go(Place::at(Level::Module { krate: krate.clone(), file: file.clone() })),
            SearchHit::Function { id, .. } => self.open_function(id, false, map),
        }
        self.query.clear();
        self.results.clear();
    }

    // ---- the frame --------------------------------------------------------

    /// Draw the whole UI into `ui` (an eframe app's root `Ui`).
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let resizing = camera::keep_canvas_at_device_pixels(&ctx);
        if !self.started {
            // Dark, like the site's other demos (the cards are drawn for it).
            // Not inside desktop kovan, which has its own theme for the
            // whole window (GitHub #820).
            if self.mode == Mode::Web {
                ctx.set_visuals(egui::Visuals::dark());
            }
            self.store.start(Self::repaint(&ctx));
            self.started = true;
        }
        let Some(snap) = self.snapshot() else {
            ctx.request_repaint();
            return;
        };
        let width = ui.max_rect().width();
        let narrow = width < NARROW;
        if !self.panel_decided && !resizing {
            self.panel_open = !narrow;
            self.panel_decided = true;
        }
        // A hash the reader typed (or the browser's own back button).
        let h = crate::platform::hash();
        if h != self.last_hash {
            self.last_hash = h.clone();
            self.pending = Some(DeepLink::parse(&h));
        }
        if let (Some(link), Some(map)) = (self.pending.clone(), snap.map.clone()) {
            self.pending = None;
            self.open_link(link, &map);
        }
        self.request_data(&ctx, &snap);
        self.keys(&ctx);

        egui::Panel::top("kw_top").show(ui, |ui| self.top_bar(ui, &snap, narrow));
        if let Some(info) = self.bar_info(&snap) {
            egui::Panel::bottom("kw_bar").show(ui, |ui| {
                let mut expanded = self.bar_expanded;
                let act = review_bar(ui, self.mode, &info, &mut expanded, narrow);
                self.bar_expanded = expanded;
                if let Some(a) = act {
                    self.bar_action(a, &snap);
                }
            });
        }
        if narrow && self.source.is_some() {
            egui::CentralPanel::default().show(ui, |ui| self.source_panel(ui, &snap, true));
            return;
        }
        let mut open = self.panel_open;
        let mut hide = false;
        let pw = (width * 0.85).min(360.0);
        egui::Panel::left("kw_side").default_size(pw).resizable(true).show_collapsible(ui, &mut open, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.site_buttons(ui);
                ui.horizontal(|ui| {
                    ui.heading(self.panel_title());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("« Hide").on_hover_text("Fold the panel away to see the whole picture").clicked() {
                            hide = true;
                        }
                    });
                });
                self.side_panel(ui, &snap);
            });
        });
        self.panel_open = if hide { false } else { open };
        if self.source.is_some() {
            egui::Panel::right("kw_source").default_size((width * 0.4).clamp(320.0, 640.0)).min_size(320.0).resizable(true).show(ui, |ui| {
                self.source_panel(ui, &snap, false)
            });
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(ui.visuals().extreme_bg_color)).show(ui, |ui| {
            self.canvas(ui, &snap);
        });
    }

    fn panel_title(&self) -> String {
        match &self.place.level {
            Level::Map => "Code map".into(),
            Level::Crate(c) => c.clone(),
            Level::Module { file, .. } => file.rsplit('/').next().unwrap_or(file).to_string(),
        }
    }

    /// Ask for whatever the current place needs, in the background.
    fn request_data(&self, ctx: &egui::Context, snap: &Snap) {
        if snap.index.is_none() {
            return;
        }
        let r = Self::repaint(ctx);
        let mut want: BTreeSet<String> = self.map_expanded.clone();
        match &self.place.level {
            Level::Map => {}
            Level::Crate(c) | Level::Module { krate: c, .. } => {
                want.insert(c.clone());
            }
        }
        if let (Some(map), Some(id)) = (&snap.map, &self.place.selected) {
            // The selected function's callees, for the bar.
            if let Some(slice) = snap.slice_of(id) {
                for c in snap.facts().callees(map, slice, id) {
                    if let Some(k) = crate_of(map, &c) {
                        want.insert(k.to_string());
                    }
                }
            }
        }
        if let Some(map) = &snap.map {
            for (id, _) in &self.place.expanded {
                if let Some(k) = crate_of(map, id) {
                    want.insert(k.to_string());
                }
            }
        }
        for c in want {
            self.store.want_crate(&c, r.clone());
        }
        if let Some(id) = &self.source {
            self.store.want_file(file_of(id), r);
        }
    }

    fn keys(&mut self, ctx: &egui::Context) {
        let typing = ctx.memory(|m| m.focused().is_some());
        let (back, fwd, find) = ctx.input(|i| {
            (
                i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft),
                i.modifiers.alt && i.key_pressed(egui::Key::ArrowRight),
                (i.modifiers.command && i.key_pressed(egui::Key::P)) || (!typing && i.key_pressed(egui::Key::Slash)),
            )
        });
        if back {
            self.back();
        }
        if fwd {
            self.forward();
        }
        if find {
            self.focus_search = true;
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui, snap: &Snap, narrow: bool) {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            let b = |s: &str| egui::Button::new(RichText::new(s).size(16.0)).min_size(egui::vec2(36.0, 32.0));
            if ui.add_enabled(self.cursor > 0, b("<")).on_hover_text("Back (Alt+Left)").clicked() {
                self.back();
            }
            if ui.add_enabled(self.cursor + 1 < self.history.len(), b(">")).on_hover_text("Forward (Alt+Right)").clicked() {
                self.forward();
            }
            if ui.add_enabled(self.place.level != Level::Map, b("Up")).on_hover_text("Up one level").clicked() {
                self.up();
            }
            self.breadcrumb(ui, snap);
        });
        // The search bar, always visible (Ctrl+P or / focuses it).
        let id = egui::Id::new("kw_search");
        let resp = ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .id(id)
                .hint_text("Search crates, modules, functions  (Ctrl+P or /)")
                .desired_width(if narrow { f32::INFINITY } else { 520.0 })
                .min_size(egui::vec2(0.0, 30.0)),
        );
        if self.focus_search {
            resp.request_focus();
            self.focus_search = false;
        }
        if self.query != self.results_for {
            self.results_for = self.query.clone();
            self.results = match &snap.map {
                Some(m) => search(&self.query, m, snap.search.as_deref(), 12),
                None => Vec::new(),
            };
        }
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if !self.query.trim().is_empty() {
            let mut chosen = None;
            if enter {
                chosen = self.results.first().cloned();
            }
            egui::Area::new(egui::Id::new("kw_results"))
                .order(egui::Order::Foreground)
                .fixed_pos(resp.rect.left_bottom() + egui::vec2(0.0, 4.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_max_width(resp.rect.width().max(320.0));
                        if self.results.is_empty() {
                            ui.weak(if snap.search.is_none() { "only crate names: no search index in this build" } else { "no match" });
                        }
                        let facts = snap.facts();
                        for r in &self.results {
                            let extra = match &r.hit {
                                SearchHit::Function { id, .. } => format!(" · {}", facts.review(id).label()),
                                SearchHit::Crate(c) => snap.map.as_ref().and_then(|m| m.get(c)).map(|c| format!(" · M{}", c.maturity)).unwrap_or_default(),
                                SearchHit::Module { .. } => String::new(),
                            };
                            let text = format!("{:6} {}{extra}\n       {}", r.kind, r.label, r.detail);
                            if ui.add(egui::Button::new(RichText::new(text).monospace().size(12.0)).frame(false).min_size(egui::vec2(0.0, 32.0))).clicked() {
                                chosen = Some(r.clone());
                            }
                        }
                    });
                });
            if let (Some(r), Some(map)) = (chosen, snap.map.clone()) {
                self.choose(&r, &map);
            }
        }
        ui.add_space(2.0);
    }

    fn breadcrumb(&mut self, ui: &mut egui::Ui, snap: &Snap) {
        let mut go: Option<Place> = None;
        if ui.link("outram-park").clicked() {
            go = Some(Place::at(Level::Map));
        }
        let (krate, file) = match &self.place.level {
            Level::Map => (None, None),
            Level::Crate(c) => (Some(c.clone()), None),
            Level::Module { krate, file } => (Some(krate.clone()), Some(file.clone())),
        };
        if let Some(k) = &krate {
            ui.weak(">");
            if ui.link(k).clicked() {
                go = Some(Place::at(Level::Crate(k.clone())));
            }
        }
        if let (Some(k), Some(f)) = (&krate, &file) {
            if let Some(slice) = snap.slice(k) {
                let tree = model::ModTree::build(&slice.krate, true);
                if let Some(n) = tree.find(f) {
                    let chain = tree.ancestry(n);
                    for (i, &a) in chain.iter().enumerate() {
                        // The library root is the crate itself.
                        if i == 0 && tree.lib_root == Some(a) && chain.len() > 1 {
                            continue;
                        }
                        ui.weak(">");
                        let node = &tree.nodes[a];
                        if i + 1 == chain.len() {
                            ui.label(RichText::new(&node.name).strong());
                        } else if ui.link(&node.name).clicked() {
                            go = Some(Place::at(Level::Module { krate: k.clone(), file: node.file.clone() }));
                        }
                    }
                }
            }
        }
        if let Some(p) = go {
            self.go(p);
        }
    }

    fn bar_info(&self, snap: &Snap) -> Option<BarInfo> {
        let id = self.place.selected.as_ref()?;
        let map = snap.map.as_ref()?;
        let facts = snap.facts();
        let slice = snap.slice_of(id);
        let (maturity, name) = match slice.and_then(|s| model::find_function(&s.krate, id)) {
            Some((m, f, _)) => (model::function_maturity(m, map, crate_of(map, id).unwrap_or("")), f.owner.as_ref().map(|o| format!("{o}::{}", f.name)).unwrap_or(f.name.clone())),
            None => (None, model::short_name(id).to_string()),
        };
        let (blocked, callees, loading) = match slice {
            Some(s) => {
                let c = facts.callees(map, s, id);
                let b: Vec<(String, model::Review)> = c.iter().filter(|x| !facts.review(x).is_valid()).map(|x| (x.clone(), facts.review(x))).collect();
                (b, c.len(), false)
            }
            None => (Vec::new(), 0, true),
        };
        let reach = slice.and_then(|s| model::find_function(&s.krate, id)).and_then(|(_, f, _)| f.reached_by.clone());
        let (tests, tests_total, examples) = match reach {
            Some(r) => (
                r.tests.iter().map(|t| (t.id.clone(), t.hops)).collect(),
                r.tests_total,
                r.examples.iter().map(|e| (format!("{}::{}", e.krate, e.example), e.hops)).collect(),
            ),
            None => (Vec::new(), 0, Vec::new()),
        };
        Some(BarInfo { name, id: id.clone(), review: facts.review(id), maturity, blocked, callees, loading, tests, tests_total, examples })
    }

    fn bar_action(&mut self, a: BarAction, snap: &Snap) {
        match a {
            BarAction::Goto(id) => {
                if let Some(map) = snap.map.clone() {
                    self.open_function(&id, false, &map);
                }
            }
            BarAction::ViewSource => {
                self.source = self.place.selected.clone();
                self.sync_hash();
            }
            BarAction::Stamp | BarAction::NeedsFix => {
                if !self.mode.can_stamp() {
                    self.status = Some("Stamping is desktop-only: stamps are made in desktop kovan.".into());
                    return;
                }
                let Some(f) = self.place.selected.as_deref().and_then(|id| self.function_ref(snap, id)) else {
                    self.status = Some("Nothing to stamp: the function is not in a loaded workspace crate.".into());
                    return;
                };
                self.host_request = Some(match a {
                    BarAction::Stamp => HostRequest::Stamp(f),
                    _ => HostRequest::NeedsFix(f),
                });
            }
        }
    }

    /// What the host is told about function `id` ([`FunctionRef`]);
    /// `None` when the code map is not loaded or `id` is in no workspace
    /// crate.
    fn function_ref(&self, snap: &Snap, id: &str) -> Option<FunctionRef> {
        let map = snap.map.as_ref()?;
        let krate = crate_of(map, id)?.to_string();
        let slice = snap.slice(&krate);
        let found = slice.and_then(|s| model::find_function(&s.krate, id)).map(|(_, f, _)| f);
        let name = match found {
            Some(f) => f.owner.as_ref().map(|o| format!("{o}::{}", f.name)).unwrap_or(f.name.clone()),
            None => model::short_name(id).to_string(),
        };
        Some(FunctionRef {
            id: id.to_string(),
            krate,
            file: file_of(id).to_string(),
            name,
            callees: slice.map(|s| snap.facts().callees(map, s, id)),
            lines: found.map(|f| (f.start_line, f.end_line)),
        })
    }

    // ---- side panel -------------------------------------------------------

    /// The crate on screen: the crate view's, or the module view's crate.
    /// `None` on the code map.
    pub(crate) fn focused_crate(&self) -> Option<&str> {
        match &self.place.level {
            Level::Map => None,
            Level::Crate(c) | Level::Module { krate: c, .. } => Some(c),
        }
    }

    /// The two big buttons at the top of the side panel (maintainer,
    /// 2026-10-07): back to the site's JavaScript code map, on the crate on
    /// screen, and to the home page. Web only: desktop kovan is not a page
    /// of the site. GUI drawing; the URLs are `model::js_map_url` /
    /// `model::home_url`, tested there.
    fn site_buttons(&self, ui: &mut egui::Ui) {
        if self.mode != Mode::Web {
            return;
        }
        let root = self.store.site_root();
        let w = ui.available_width();
        let big = |ui: &mut egui::Ui, text: &str, tip: String| {
            ui.add_sized([w, 44.0], egui::Button::new(RichText::new(text).size(17.0).strong()).fill(Color32::from_rgb(31, 92, 100)))
                .on_hover_text(tip)
                .clicked()
        };
        let map = model::js_map_url(&root, self.focused_crate());
        let tip = match self.focused_crate() {
            Some(c) => format!("The site's JavaScript code map, on {c}"),
            None => "The site's JavaScript code map".to_string(),
        };
        if big(ui, "Go back to JavaScript map", tip) {
            ui.ctx().open_url(egui::OpenUrl::same_tab(map));
        }
        if big(ui, "Go to homepage", "The OUTRAM PARK site's home page".into()) {
            ui.ctx().open_url(egui::OpenUrl::same_tab(model::home_url(&root)));
        }
        ui.add_space(6.0);
    }

    fn side_panel(&mut self, ui: &mut egui::Ui, snap: &Snap) {
        if let Some(s) = self.status.clone() {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(Color32::from_rgb(250, 200, 80), s);
                if ui.small_button("×").clicked() {
                    self.status = None;
                }
            });
        }
        match self.place.level.clone() {
            Level::Map => self.map_panel(ui, snap),
            Level::Crate(c) => self.crate_panel(ui, snap, &c),
            Level::Module { krate, file } => self.module_panel(ui, snap, &krate, &file),
        }
        ui.separator();
        ui.collapsing("Data loaded", |ui| {
            for t in &snap.timings {
                ui.label(format!("{}: {:.0} kB, fetched in {:.0} ms, parsed in {:.0} ms", t.what, t.bytes as f64 / 1e3, t.fetch_ms, t.parse_ms));
            }
            if let Some(b) = &snap.build {
                ui.label(format!("built from {} @ {}", b.repo, &b.commit[..b.commit.len().min(10)]));
                if let Some(cg) = &b.call_graph {
                    ui.label(format!("call graph: {} backend, {}", cg.backend, cg.rust_analyzer));
                }
            }
            for e in &snap.errors {
                ui.colored_label(Color32::from_rgb(255, 130, 130), e);
            }
        });
        ui.add_space(8.0);
        ui.weak("Research, education and V&V only. Read-only: stamps are made in desktop kovan.");
    }

    fn map_panel(&mut self, ui: &mut egui::Ui, snap: &Snap) {
        ui.label("Every crate of the workspace in its topic box. Tap a crate to open its module tree; the triangle on a card shows its top-level modules in place.");
        ui.label(match self.mode {
            Mode::Web => "Web: read-only.",
            Mode::Desktop => "Desktop: Stamp and Needs fix open desktop kovan's stamp dialog.",
        });
        if let Some(w) = snap.build.as_ref().and_then(|b| b.call_graph.as_ref()).and_then(|c| c.warning()) {
            ui.colored_label(Color32::from_rgb(255, 130, 130), w);
        }
        if let Some(ix) = &snap.index {
            ui.label(format!("Call graph in this build: {} crates, {} functions.", ix.crates.len(), ix.totals.functions));
            ui.horizontal_wrapped(|ui| {
                for c in &ix.crates {
                    if ui.link(&c.name).clicked() {
                        self.go(Place::at(Level::Crate(c.name.clone())));
                    }
                }
            });
        }
        ui.separator();
        ui.label(RichText::new("Stamp colours").strong());
        use kovan_common::review::state::Tone;
        for (t, c) in [
            ("unreviewed", bar::tone_colour(Tone::Neutral)),
            ("valid", bar::tone_colour(Tone::Good)),
            ("needs a person: changed, a callee changed, moved, doc changed, fixed, pending test", bar::tone_colour(Tone::Attention)),
            ("does not count: needs fix, unverified, outside scope, unreadable", bar::tone_colour(Tone::Blocked)),
        ] {
            ui.colored_label(c, format!("{t}"));
        }
        ui.label("Maturity: M0 concept, M1 AI draft, M2 AI V&V, M3 human reviewed, M4 human V&V.");
    }

    fn crate_panel(&mut self, ui: &mut egui::Ui, snap: &Snap, krate: &str) {
        if let Some(c) = snap.map.as_ref().and_then(|m| m.get(krate)) {
            if let Some(b) = &c.backronym {
                ui.label(egui::RichText::new(b).italics());
            }
            if let Some(d) = &c.description {
                ui.label(d);
            }
            ui.label(format!("Maturity M{} ({}), {}", c.maturity, kovan_common::code_map::maturity_label(c.maturity), c.topic.title()));
        }
        ui.checkbox(&mut self.show_tests, "Show test modules");
        let Some(slice) = snap.slice(krate).cloned() else {
            ui.weak(self.load_state(snap, krate));
            return;
        };
        let tree = model::ModTree::build(&slice.krate, self.show_tests);
        let api = snap.api.get(krate).cloned();
        ui.separator();
        ui.label(RichText::new("Modules").strong());
        let mut open = None;
        for (n, depth) in tree.depth_first() {
            let node = &tree.nodes[n];
            ui.horizontal(|ui| {
                ui.add_space(14.0 * depth as f32);
                let label = if node.path.is_empty() { format!("{} ({})", node.name, if node.kind == kovan_common::call_graph::TargetKind::Lib { "lib" } else { "example" }) } else { node.name.clone() };
                let sel = self.selected_module.as_deref() == Some(node.file.as_str());
                if ui.selectable_label(sel, RichText::new(label).monospace()).on_hover_text(&node.file).clicked() {
                    open = Some(node.file.clone());
                }
                ui.weak(format!("M{} · {} fn", node.maturity.or(slice.krate.maturity).unwrap_or(0), node.functions));
                if node.kind == kovan_common::call_graph::TargetKind::Lib {
                    self.api_link(ui, snap, &api, &model::rustdoc_module_page(krate, &node.path), "");
                }
            });
        }
        if let Some(f) = open {
            self.go(Place::at(Level::Module { krate: krate.to_string(), file: f }));
        }
    }

    fn module_panel(&mut self, ui: &mut egui::Ui, snap: &Snap, krate: &str, file: &str) {
        ui.label(RichText::new(file).monospace().small());
        if ui.button("Collapse all").on_hover_text("Fold every unfolded caller and callee").clicked() && !self.place.expanded.is_empty() {
            let mut p = self.place.clone();
            p.expanded.clear();
            self.go(p);
        }
        ui.checkbox(&mut self.show_tests, "Show test functions");
        let Some(slice) = snap.slice(krate).cloned() else {
            ui.weak(self.load_state(snap, krate));
            return;
        };
        let Some((module, kind, _)) = model::find_module(&slice.krate, file) else {
            ui.weak("module not in this crate's data");
            return;
        };
        let api = snap.api.get(krate).cloned();
        let lib = kind == kovan_common::call_graph::TargetKind::Lib;
        if lib {
            ui.horizontal(|ui| {
                ui.label(RichText::new(if module.path.is_empty() { "crate root" } else { module.path.as_str() }).monospace());
                self.api_link(ui, snap, &api, &model::rustdoc_module_page(krate, &module.path), "");
            });
        }
        if let Some(u) = &module.upstream {
            ui.horizontal_wrapped(|ui| {
                ui.weak(format!("ported from {}", u.project.clone().unwrap_or_default()));
                if let Some(url) = &u.url {
                    ui.add(egui::Hyperlink::from_label_and_url("Upstream", url).open_in_new_tab(true));
                }
            });
        }
        ui.separator();
        ui.label(RichText::new("Functions").strong());
        let facts = snap.facts();
        let map = snap.map.clone();
        let mut pick = None;
        for f in module.functions.iter().filter(|f| self.show_tests || !f.test) {
            let sel = self.place.selected.as_deref() == Some(f.id.as_str());
            ui.horizontal_wrapped(|ui| {
                let name = f.owner.as_ref().map(|o| format!("{o}::{}", f.name)).unwrap_or(f.name.clone());
                if ui.selectable_label(sel, RichText::new(name).monospace().strong()).clicked() {
                    pick = Some(f.id.clone());
                }
                let r = facts.review(&f.id);
                ui.colored_label(bar::review_colour(&r), r.label());
                if let Some(m) = map.as_ref().and_then(|m| model::function_maturity(module, m, krate)) {
                    ui.weak(format!("M{m}"));
                }
                ui.weak(format!("L{}", f.line));
                if lib && !f.test {
                    if let Some((page, frag)) = model::rustdoc_fn_pages(krate, &module.path, f).into_iter().find(|(p, _)| api.as_ref().is_some_and(|a| a.contains(p))) {
                        self.api_link(ui, snap, &api, &page, &frag);
                    }
                }
            });
            ui.label(RichText::new(&f.signature).monospace().small().weak());
            if !f.doc.is_empty() {
                ui.label(RichText::new(&f.doc).small());
            }
            ui.add_space(4.0);
        }
        if let Some(id) = pick {
            let mut p = self.place.clone();
            p.selected = Some(id);
            self.replace(p);
        }
    }

    fn load_state(&self, snap: &Snap, krate: &str) -> String {
        match snap.crates.get(krate) {
            Some(Load::Failed(e)) => format!("No call-graph data: {e}."),
            Some(Load::Pending) => "Loading the crate's call graph…".into(),
            _ if snap.index.is_none() => "Loading the index…".into(),
            _ => "Not in this build's call graph.".into(),
        }
    }

    /// "API" to a rustdoc page, only when the site has that page.
    fn api_link(&self, ui: &mut egui::Ui, _snap: &Snap, api: &Option<Arc<BTreeSet<String>>>, page: &str, frag: &str) {
        let (Some(base), Some(set)) = (self.store.api_url(), api) else { return };
        if set.contains(page) {
            ui.add(egui::Hyperlink::from_label_and_url("API", format!("{base}{page}{frag}")).open_in_new_tab(true));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The crate the side panel's "Go back to JavaScript map" opens on.
    #[test]
    fn focused_crate_follows_the_view() {
        let src = DataSource::Dir { data: std::env::temp_dir(), workspace: ".".into() };
        let mut r = CodeReview::new(Mode::Web, src);
        assert_eq!(r.focused_crate(), None);
        r.go(Place::at(Level::Crate("boon-lay".into())));
        assert_eq!(r.focused_crate(), Some("boon-lay"));
        r.go(Place::at(Level::Module { krate: "petir".into(), file: "crates/petir/src/lib.rs".into() }));
        assert_eq!(r.focused_crate(), Some("petir"));
        assert_eq!(model::js_map_url(&r.store.site_root(), r.focused_crate()), format!("{}code-map/#petir", crate::data::SITE_URL));
    }

    /// A snapshot with a one-crate code map (`bl` at `crates/bl`) and no
    /// slice loaded.
    fn snap_with_map() -> Snap {
        let map = CodeMap {
            root: "r".into(),
            crates: vec![kovan_common::code_map::CrateNode {
                name: "bl".into(),
                description: None,
                backronym: None,
                row: 2,
                topic: kovan_common::code_map::Topic::Risk,
                fidelity: None,
                maturity: 1,
                maturity_modules: vec![],
                lib_dir: None,
                dir: Some("crates/bl".into()),
            }],
            edges: vec![],
        };
        Snap {
            map: Some(Arc::new(map)),
            layout: None,
            index: None,
            search: None,
            site_links: None,
            build: None,
            crates: BTreeMap::new(),
            api: BTreeMap::new(),
            files: BTreeMap::new(),
            timings: vec![],
            errors: vec![],
            stamps: None,
        }
    }

    /// Desktop: Stamp and Needs fix queue one request for the host, taken
    /// once, naming the selected function; with the crate's slice not yet
    /// loaded the callees are `None` (unknown), not empty. Web: no request,
    /// a "desktop-only" status instead.
    #[test]
    fn stamp_queues_a_host_request_in_desktop_mode_only() {
        let id = "crates/bl/src/chem.rs::Pool::rate";
        let src = || DataSource::Dir { data: std::env::temp_dir(), workspace: ".".into() };
        let snap = snap_with_map();

        let mut d = CodeReview::new(Mode::Desktop, src());
        d.place.selected = Some(id.into());
        d.bar_action(BarAction::Stamp, &snap);
        let want = FunctionRef {
            id: id.into(),
            krate: "bl".into(),
            file: "crates/bl/src/chem.rs".into(),
            name: "Pool::rate".into(),
            callees: None,
            lines: None,
        };
        assert_eq!(d.take_host_request(), Some(HostRequest::Stamp(want.clone())));
        assert_eq!(d.take_host_request(), None, "taken once");
        d.bar_action(BarAction::NeedsFix, &snap);
        assert_eq!(d.take_host_request(), Some(HostRequest::NeedsFix(want)));
        // A function outside every workspace crate: nothing for the host.
        d.place.selected = Some("vendor/x.rs::f".into());
        d.bar_action(BarAction::Stamp, &snap);
        assert_eq!(d.take_host_request(), None);
        assert!(d.status.as_deref().unwrap_or("").contains("Nothing to stamp"));

        let mut w = CodeReview::new(Mode::Web, src());
        w.place.selected = Some(id.into());
        w.bar_action(BarAction::Stamp, &snap);
        w.bar_action(BarAction::NeedsFix, &snap);
        assert_eq!(w.take_host_request(), None);
        assert!(w.status.as_deref().unwrap_or("").contains("desktop-only"));
    }

    /// Stamps the host pushes replace the data's in every snapshot, so the
    /// bar and the cards read them on the next frame.
    #[test]
    fn set_stamps_replaces_the_stamp_states_read_by_the_views() {
        use kovan_common::call_graph::split::StampVerdict;
        let id = "crates/bl/src/chem.rs::rate";
        let mut r = CodeReview::new(Mode::Desktop, DataSource::Dir { data: std::env::temp_dir(), workspace: ".".into() });
        assert!(!r.snapshot().unwrap().facts().review(id).is_valid());
        r.set_stamps(vec![StampState {
            function: id.into(),
            verdict: StampVerdict::Valid,
            reason: String::new(),
            rung: 2,
            reviewer: "R".into(),
            date: "2026-10-10".into(),
            note: String::new(),
            permalink: String::new(),
            state: None,
        }]);
        assert!(r.snapshot().unwrap().facts().review(id).is_valid());
        r.set_stamps(vec![]);
        assert!(!r.snapshot().unwrap().facts().review(id).is_valid());
    }
}
