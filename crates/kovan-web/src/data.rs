//! Loading the page's data in the background, never on a frame
//! (no-lag rule): every file is fetched (web) or read (native) off the
//! frame, parsed, and dropped into the shared [`Shared`] state; the UI only
//! takes `try_read` snapshots and asks for a repaint when something lands.
//!
//! What is loaded, from the data folder the site build writes
//! (`code-review/data/`, see `web/build.sh`):
//!
//! ```text
//! build.json            {"commit": "<sha>", "repo": "<owner/name>",
//!                        "call_graph": {...}} (CallGraphStatus, #772)
//! code_map.json         kovan-cli code-map --format json
//! graph/index.json      kovan-cli call-graph --split-dir (SplitIndex)
//! graph/search.json     the same run's SearchIndex, for the search bar
//! graph/<crate>.json    one CrateSlice, fetched when the crate is opened
//! api/<crate>.json      the rustdoc pages the site has for that crate
//! site_links.json       the deep dives, tutorials and rustdoc the site has
//!                       (the navbar's file, scripts/build-pages.sh); optional
//! ```
//!
//! and source files themselves, fetched on demand from
//! `raw.githubusercontent.com/<repo>/<commit>/<file>` at the commit the site
//! was built from (maintainer, 2026-10-06: source text is not copied into
//! Pages); natively, read from the workspace checkout.
//!
//! On the web the fetch runs as a future on the page's event loop
//! (`wasm_bindgen_futures::spawn_local`), so the frame never waits for the
//! network; the JSON is parsed when the response arrives, between frames.
//! The parse time is measured and shown in the side panel, so a parse long
//! enough to be felt is visible rather than hidden. Natively each load is a
//! thread.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use kovan_common::call_graph::split::{CrateSlice, SearchIndex, SplitIndex};
use kovan_common::code_map::layout::Layout;
use kovan_common::code_map::CodeMap;
use serde::Deserialize;

/// The default repository the source is fetched from.
pub const DEFAULT_REPO: &str = "theodoreOnzGit/outram-park-backend";

/// The published site, for links when the page is not served from it
/// (natively).
pub const SITE_URL: &str = "https://theodoreonzgit.github.io/outram-park-backend/";

/// One book in `site_links.json`: its URL relative to the site root and the
/// mdBook folder it is built from.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Book {
    pub url: String,
    pub dir: String,
}

/// `site_links.json`, the same file the navbar strip reads
/// (`docs/site/code-map-bar.js`).
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct SiteLinks {
    #[serde(default)]
    pub deep_dives: Vec<Book>,
    #[serde(default)]
    pub tutorials: Vec<Book>,
}

impl SiteLinks {
    /// The deep dives built from a folder inside crate folder `dir` (the
    /// navbar's `ownsDir`).
    pub fn deep_dives_of(&self, dir: &str) -> Vec<&Book> {
        let prefix = format!("{}/", dir.trim_end_matches('/'));
        self.deep_dives.iter().filter(|b| b.dir.starts_with(&prefix)).collect()
    }
}

/// One thing being loaded.
#[derive(Debug, Clone)]
pub enum Load<T> {
    Pending,
    Ready(Arc<T>),
    Failed(String),
}

impl<T> Load<T> {
    pub fn ready(&self) -> Option<&Arc<T>> {
        match self {
            Load::Ready(v) => Some(v),
            _ => None,
        }
    }
}

/// `build.json`: what the site was built from.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BuildInfo {
    pub commit: String,
    #[serde(default = "default_repo")]
    pub repo: String,
    /// How the call graph was built and which crates it lacks (#772);
    /// absent in data written before it.
    #[serde(default)]
    pub call_graph: Option<CallGraphStatus>,
}

/// `build.json`'s `call_graph`, written by `web/data.sh` so a partial call
/// graph is shown, never silent (Leak Before Break, `docs/kovan.md`).
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct CallGraphStatus {
    /// `scip` or `lsp`.
    #[serde(default)]
    pub backend: String,
    /// `rust-analyzer --version` of the build.
    #[serde(default)]
    pub rust_analyzer: String,
    /// Crates whose graph is the last cached one, built from older source
    /// (rust-analyzer or that crate's run failed this time).
    #[serde(default)]
    pub stale: Vec<String>,
    /// Crates with no call graph at all in this build.
    #[serde(default)]
    pub missing: Vec<String>,
}

impl CallGraphStatus {
    /// One line for the page when the graph is partial, else `None`.
    pub fn warning(&self) -> Option<String> {
        if self.stale.is_empty() && self.missing.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        if !self.stale.is_empty() {
            parts.push(format!(
                "STALE (graph from an older commit, lines may be off): {}",
                self.stale.join(", ")
            ));
        }
        if !self.missing.is_empty() {
            parts.push(format!("MISSING: {}", self.missing.join(", ")));
        }
        Some(format!("Call graph incomplete in this build. {}.", parts.join(". ")))
    }
}

fn default_repo() -> String {
    DEFAULT_REPO.to_string()
}

/// How long one load took, for the side panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Timing {
    pub what: String,
    pub bytes: usize,
    pub fetch_ms: f64,
    pub parse_ms: f64,
}

/// Everything loaded so far.
#[derive(Default)]
pub struct Shared {
    pub build: Option<Load<BuildInfo>>,
    pub code_map: Option<Load<CodeMap>>,
    /// The code map laid out, once it has loaded.
    pub layout: Option<Arc<Layout>>,
    pub index: Option<Load<SplitIndex>>,
    pub search: Option<Load<SearchIndex>>,
    pub site_links: Option<Load<SiteLinks>>,
    pub crates: BTreeMap<String, Load<CrateSlice>>,
    /// Per crate, the rustdoc pages that exist (paths under `api/`).
    pub api: BTreeMap<String, Load<BTreeSet<String>>>,
    /// Source files by workspace-relative path.
    pub files: BTreeMap<String, Load<String>>,
    pub timings: Vec<Timing>,
}

/// Where the data comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataSource {
    /// Over HTTP: `data` is the data folder's URL (`"data/"` on the page),
    /// `api` the rustdoc folder's (`"../api/"`).
    Http { data: String, api: String },
    /// A local folder laid out as the site's data folder, and the workspace
    /// checkout source files are read from. Native only.
    Dir { data: std::path::PathBuf, workspace: std::path::PathBuf },
}

/// The shared state and how to fill it.
#[derive(Clone)]
pub struct Store {
    pub shared: Arc<RwLock<Shared>>,
    pub source: DataSource,
}

impl Store {
    pub fn new(source: DataSource) -> Store {
        Store { shared: Arc::default(), source }
    }

    /// The rustdoc folder's URL as links on the page use it (`None` natively).
    pub fn api_url(&self) -> Option<&str> {
        match &self.source {
            DataSource::Http { api, .. } => Some(api),
            DataSource::Dir { .. } => None,
        }
    }

    /// The site root as links on the page use it: next to `api/` on the
    /// web, the published site natively.
    pub fn site_root(&self) -> String {
        match &self.source {
            DataSource::Http { api, .. } => api.strip_suffix("api/").unwrap_or("../").to_string(),
            DataSource::Dir { .. } => SITE_URL.to_string(),
        }
    }

    /// Start the first loads: build info, code map, call-graph index.
    pub fn start(&self, repaint: impl Fn() + Clone + Send + 'static) {
        {
            let mut s = self.shared.write().unwrap();
            s.build = Some(Load::Pending);
            s.code_map = Some(Load::Pending);
            s.index = Some(Load::Pending);
            s.search = Some(Load::Pending);
        }
        self.load_json::<BuildInfo>("build.json", repaint.clone(), |s, v| s.build = Some(v));
        self.load_json::<CodeMap>("code_map.json", repaint.clone(), |s, v| {
            if let Load::Ready(m) = &v {
                s.layout = Some(Arc::new(kovan_common::code_map::layout::layout(m)));
            }
            s.code_map = Some(v);
        });
        self.load_json::<SplitIndex>("graph/index.json", repaint.clone(), |s, v| s.index = Some(v));
        self.load_json::<SearchIndex>("graph/search.json", repaint.clone(), |s, v| s.search = Some(v));
        self.load_json::<SiteLinks>("site_links.json", repaint, |s, v| s.site_links = Some(v));
    }

    /// Load crate `name`'s call graph (and its rustdoc page list) unless it
    /// is loaded or loading. A crate absent from the index is not fetched.
    pub fn want_crate(&self, name: &str, repaint: impl Fn() + Clone + Send + 'static) {
        let file = {
            let mut s = self.shared.write().unwrap();
            if s.crates.contains_key(name) {
                return;
            }
            let entry = s.index.as_ref().and_then(|i| i.ready()).and_then(|i| i.crates.iter().find(|c| c.name == name).cloned());
            let Some(entry) = entry else {
                if matches!(s.index, Some(Load::Ready(_))) {
                    s.crates.insert(name.to_string(), Load::Failed("not in this build's call graph".into()));
                }
                return;
            };
            s.crates.insert(name.to_string(), Load::Pending);
            s.api.insert(name.to_string(), Load::Pending);
            entry.file
        };
        let key = name.to_string();
        self.load_json::<CrateSlice>(&format!("graph/{file}"), repaint.clone(), move |s, v| {
            s.crates.insert(key, v);
        });
        let key = name.to_string();
        self.load_json::<Vec<String>>(&format!("api/{name}.json"), repaint, move |s, v| {
            let v = match v {
                Load::Ready(list) => Load::Ready(Arc::new(list.iter().cloned().collect())),
                // No rustdoc for this crate: no links, not an error.
                _ => Load::Ready(Arc::new(BTreeSet::new())),
            };
            s.api.insert(key, v);
        });
    }

    /// Load source file `file` unless it is loaded or loading.
    pub fn want_file(&self, file: &str, repaint: impl Fn() + Clone + Send + 'static) {
        let url = {
            let s = self.shared.write().unwrap();
            if s.files.contains_key(file) {
                return;
            }
            let build = s.build.as_ref().and_then(|b| b.ready()).cloned();
            match (&self.source, build) {
                (DataSource::Http { .. }, Some(b)) => {
                    format!("https://raw.githubusercontent.com/{}/{}/{}", b.repo, b.commit, file)
                }
                (DataSource::Http { .. }, None) => return, // retried once build.json lands
                (DataSource::Dir { workspace, .. }, _) => workspace.join(file).to_string_lossy().into_owned(),
            }
        };
        self.shared.write().unwrap().files.insert(file.to_string(), Load::Pending);
        let key = file.to_string();
        let shared = self.shared.clone();
        fetch_text(url, move |r, _fetch_ms| {
            let v = match r {
                Ok(t) => Load::Ready(Arc::new(t)),
                Err(e) => Load::Failed(e),
            };
            shared.write().unwrap().files.insert(key, v);
            repaint();
        });
    }

    fn load_json<T: for<'de> Deserialize<'de> + Send + Sync + 'static>(
        &self,
        rel: &str,
        repaint: impl Fn() + Send + 'static,
        put: impl FnOnce(&mut Shared, Load<T>) + Send + 'static,
    ) {
        let url = match &self.source {
            DataSource::Http { data, .. } => format!("{data}{rel}"),
            DataSource::Dir { data, .. } => data.join(rel).to_string_lossy().into_owned(),
        };
        let shared = self.shared.clone();
        let what = rel.to_string();
        fetch_text(url, move |r, fetch_ms| {
            let t0 = crate::platform::now_ms();
            let (v, bytes) = match r {
                Ok(text) => {
                    let n = text.len();
                    match serde_json::from_str::<T>(&text) {
                        Ok(v) => (Load::Ready(Arc::new(v)), n),
                        Err(e) => (Load::Failed(format!("{what}: {e}")), n),
                    }
                }
                Err(e) => (Load::Failed(e), 0),
            };
            let parse_ms = crate::platform::now_ms() - t0;
            let mut s = shared.write().unwrap();
            s.timings.push(Timing { what, bytes, fetch_ms, parse_ms });
            put(&mut s, v);
            drop(s);
            repaint();
        });
    }
}

/// Fetch `url` (or read the file at that path natively) as text, then call
/// `done(result, milliseconds taken)` off the frame.
fn fetch_text(url: String, done: impl FnOnce(Result<String, String>, f64) + Send + 'static) {
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async move {
        let t0 = crate::platform::now_ms();
        let r = web::fetch_text(&url).await;
        done(r, crate::platform::now_ms() - t0);
    });
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || {
        let t0 = crate::platform::now_ms();
        let r = std::fs::read_to_string(&url).map_err(|e| format!("{url}: {e}"));
        done(r, crate::platform::now_ms() - t0);
    });
}

#[cfg(target_arch = "wasm32")]
mod web {
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;

    /// `fetch(url).text()`, failing on a non-2xx status.
    pub async fn fetch_text(url: &str) -> Result<String, String> {
        let err = |e: JsValue| format!("{url}: {e:?}");
        let w = web_sys::window().ok_or("no window")?;
        let resp: web_sys::Response =
            JsFuture::from(w.fetch_with_str(url)).await.map_err(err)?.dyn_into().map_err(err)?;
        if !resp.ok() {
            return Err(format!("{url}: HTTP {}", resp.status()));
        }
        let text = JsFuture::from(resp.text().map_err(err)?).await.map_err(err)?;
        text.as_string().ok_or_else(|| format!("{url}: not text"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: a data folder with a code map, an index of one crate and
    /// that crate's slice, read natively. Every load lands as Ready without
    /// the caller waiting on it, and a crate missing from the index is
    /// reported as such rather than fetched.
    ///
    /// Result (2026-10-06): passes.
    #[test]
    fn loads_land_in_the_background() {
        let dir = std::env::temp_dir().join(format!("kovan-web-data-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("graph")).unwrap();
        std::fs::write(dir.join("build.json"), r#"{"commit":"abc"}"#).unwrap();
        std::fs::write(
            dir.join("code_map.json"),
            serde_json::to_string(&CodeMap { root: "r".into(), crates: vec![], edges: vec![] }).unwrap(),
        )
        .unwrap();
        let slice = r#"{"schema":1,"crate":{"name":"a","dir":"crates/a","targets":[]},"calls":[],"module_calls":[],"crate_calls":[],"outside":[],"files":[]}"#;
        std::fs::write(dir.join("graph/a.json"), slice).unwrap();
        let index = r#"{"schema":1,"graph_schema":1,"crates":[{"name":"a","dir":"crates/a","file":"a.json","bytes":1,"modules":0,"functions":0}],"totals":{"crates":1,"targets":0,"modules":0,"functions":0,"calls":0,"call_sites":0,"unresolved":0,"unresolved_by_kind":{},"outside":0,"external_calls":0},"stamps":[]}"#;
        std::fs::write(dir.join("graph/index.json"), index).unwrap();
        let store = Store::new(DataSource::Dir { data: dir.clone(), workspace: dir.clone() });
        store.start(|| {});
        let wait = |f: &dyn Fn(&Shared) -> bool| {
            for _ in 0..500 {
                if f(&store.shared.read().unwrap()) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            panic!("timed out");
        };
        wait(&|s| matches!(s.index, Some(Load::Ready(_))) && s.layout.is_some());
        assert_eq!(store.shared.read().unwrap().build.as_ref().unwrap().ready().unwrap().repo, DEFAULT_REPO);
        store.want_crate("a", || {});
        store.want_crate("zz", || {});
        wait(&|s| matches!(s.crates.get("a"), Some(Load::Ready(_))) && matches!(s.api.get("a"), Some(Load::Ready(_))));
        assert!(matches!(store.shared.read().unwrap().crates.get("zz"), Some(Load::Failed(_))));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A crate owns the deep dives built from a folder inside its own, the
    /// navbar's rule (`ownsDir` in docs/site/code-map-bar.js): `crates/kovan`
    /// must not claim `crates/kovan-web`'s book.
    #[test]
    fn deep_dives_belong_to_the_crate_folder_they_live_in() {
        let l: SiteLinks = serde_json::from_str(
            r#"{"deep_dives":[{"url":"deep-dives/mc/","dir":"crates/outram-mc-libs/docs/lessons"},
                {"url":"deep-dives/w/","dir":"crates/kovan-web/docs/lessons"}],"tutorials":[],"api":["x"]}"#,
        )
        .unwrap();
        assert_eq!(l.deep_dives_of("crates/outram-mc-libs").len(), 1);
        assert_eq!(l.deep_dives_of("crates/outram-mc-libs/").len(), 1);
        assert!(l.deep_dives_of("crates/kovan").is_empty());
        assert!(l.deep_dives_of("crates/outram-mc").is_empty());
    }

    /// Leak Before Break (#772): a `build.json` naming stale or missing
    /// crates gives a warning line; a complete one, or an old `build.json`
    /// without `call_graph`, gives none.
    #[test]
    fn a_partial_call_graph_is_shown() {
        let old: BuildInfo = serde_json::from_str(r#"{"commit":"abc"}"#).unwrap();
        assert!(old.call_graph.is_none());
        let full: BuildInfo = serde_json::from_str(
            r#"{"commit":"abc","repo":"o/r","call_graph":{"backend":"scip","rust_analyzer":"rust-analyzer 1.98.0","crates":2,"reindexed":2,"stale":[],"missing":[]}}"#,
        )
        .unwrap();
        assert_eq!(full.call_graph.as_ref().unwrap().warning(), None);
        let part: BuildInfo = serde_json::from_str(
            r#"{"commit":"abc","call_graph":{"backend":"scip","rust_analyzer":"x","stale":["a","b"],"missing":["c"]}}"#,
        )
        .unwrap();
        let w = part.call_graph.unwrap().warning().unwrap();
        assert!(w.contains("STALE") && w.contains("a, b") && w.contains("MISSING: c"), "{w}");
    }
}
