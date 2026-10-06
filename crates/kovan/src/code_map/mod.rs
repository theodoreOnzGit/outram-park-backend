//! The **code map** of a Cargo workspace: every member crate placed by the
//! `[package.metadata.kovan]` tag in its `Cargo.toml`, with its required
//! dependency edges (GitHub #734, step 2 of #729; the tags are #733).
//!
//! # What it shows
//!
//! The maintainer's layout, decided 2026-10-06 on #729:
//!
//! ```text
//!                                outram-park                        (root card)
//!  row 4   [dhoby-ghaut] [digital-twin-engine] [dover] [pasir-ris]  (each app its own box)
//!          NEUTRONICS | THERMAL-HYDRAULICS | ... | RISK              ┌ KNOWLEDGE MGMT ┐
//!  row 3   crates     | ...   (topic boxes are columns, rows 3-2)    │ kovan family   │
//!  row 2   crates     | ...   highest fidelity left, lowest right    │ by their rows  │
//!  rows 1-0 ── shared utilities base ──                              └────────────────┘
//! ```
//!
//! A card carries the crate's name, maturity (0 concept .. 4 human V&V, the
//! crate's lowest part) and fidelity (0 lumped .. 4 brute force, or a range).
//! The meaning of every tag is documented, and checked against the
//! dependency graph, in `crates/kovan/tests/code_map_tags.rs`.
//!
//! # Where the data comes from: `cargo metadata`, not rust-analyzer
//!
//! [`CodeMap::from_cargo_metadata`] reads the JSON of
//! `cargo metadata --format-version 1 --no-deps`: the package list, each
//! package's tag and its declared dependencies. Nothing is inferred from
//! source code and rust-analyzer is never run, so the map is cheap to build
//! on a desktop or in CI and is shipped to the web as static data (the SVG and
//! JSON `kovan-cli code-map` writes). Phones only ever render it.
//!
//! # Edges
//!
//! Only **required normal** dependencies between workspace members: not
//! optional ones, not dev- or build-dependencies. That is what a crate needs
//! in order to build, the same rule the row check uses.
//!
//! # Determinism
//!
//! Same `Cargo.toml`s, same map: crates are sorted by name, edges by
//! `(from, to)`, and the layout ([`layout::layout`]) uses only sorted
//! vectors and `BTreeMap`s, never a `HashMap`'s iteration order. The SVG
//! ([`svg::render`]) is plain string building with fixed number formatting,
//! so the same input gives byte-identical output (tested).
//!
//! Plain `serde` + `std` only, no GUI and no I/O except
//! [`run_cargo_metadata`], so the module can move to a wasm-clean crate
//! unchanged apart from that one function.

pub mod layout;
pub mod svg;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The title of the root card at the top of the map.
pub const ROOT_TITLE: &str = "outram-park";

/// The box a crate is drawn in (`topic` in the tag).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Topic {
    App,
    Neutronics,
    ThermalHydraulics,
    FuelPerformance,
    StructuralMechanics,
    Chemistry,
    FuelCycle,
    GranularDem,
    Risk,
    Utility,
    KnowledgeManagement,
}

impl Topic {
    /// Every topic, in the order the map draws them left to right.
    pub const ALL: [Topic; 11] = [
        Topic::App,
        Topic::Neutronics,
        Topic::ThermalHydraulics,
        Topic::FuelPerformance,
        Topic::StructuralMechanics,
        Topic::Chemistry,
        Topic::FuelCycle,
        Topic::GranularDem,
        Topic::Risk,
        Topic::Utility,
        Topic::KnowledgeManagement,
    ];

    /// The topic columns of the pyramid (rows 3 and 2), left to right.
    pub const COLUMNS: [Topic; 8] = [
        Topic::Neutronics,
        Topic::ThermalHydraulics,
        Topic::FuelPerformance,
        Topic::StructuralMechanics,
        Topic::Chemistry,
        Topic::FuelCycle,
        Topic::GranularDem,
        Topic::Risk,
    ];

    /// The tag's spelling, e.g. `"thermal-hydraulics"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Topic::App => "app",
            Topic::Neutronics => "neutronics",
            Topic::ThermalHydraulics => "thermal-hydraulics",
            Topic::FuelPerformance => "fuel-performance",
            Topic::StructuralMechanics => "structural-mechanics",
            Topic::Chemistry => "chemistry",
            Topic::FuelCycle => "fuel-cycle",
            Topic::GranularDem => "granular-dem",
            Topic::Risk => "risk",
            Topic::Utility => "utility",
            Topic::KnowledgeManagement => "knowledge-management",
        }
    }

    /// Parse the tag's spelling.
    pub fn parse(s: &str) -> Option<Topic> {
        Topic::ALL.into_iter().find(|t| t.as_str() == s)
    }

    /// The box title drawn on the map.
    pub fn title(self) -> &'static str {
        match self {
            Topic::App => "APP",
            Topic::Neutronics => "NEUTRONICS",
            Topic::ThermalHydraulics => "THERMAL-HYDRAULICS",
            Topic::FuelPerformance => "FUEL PERFORMANCE",
            Topic::StructuralMechanics => "STRUCTURAL MECHANICS",
            Topic::Chemistry => "CHEMISTRY",
            Topic::FuelCycle => "FUEL CYCLE",
            Topic::GranularDem => "GRANULAR / DEM",
            Topic::Risk => "RISK / UQ / CONSEQUENCE",
            Topic::Utility => "SHARED UTILITIES",
            Topic::KnowledgeManagement => "KNOWLEDGE MANAGEMENT",
        }
    }

    /// Whether a crate of this topic carries no fidelity (utilities and
    /// knowledge management).
    pub fn has_no_fidelity(self) -> bool {
        matches!(self, Topic::Utility | Topic::KnowledgeManagement)
    }
}

/// A crate's fidelity on the absolute 0..=4 scale (maintainer,
/// 2026-10-06): 0 lumped, 1 one resolved dimension, 2 two (subchannel),
/// 3 three (CFD, diffusion), 4 brute force (Monte Carlo, first-principles
/// data). Serialised as the tag spells it: `3` or `[1, 3]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Fidelity {
    Level(u8),
    /// `(lo, hi)`, `lo < hi`: the crate spans these levels.
    Range(u8, u8),
}

impl Fidelity {
    /// Highest level the crate reaches.
    pub fn hi(self) -> u8 {
        match self {
            Fidelity::Level(l) => l,
            Fidelity::Range(_, hi) => hi,
        }
    }

    /// Lowest level the crate reaches.
    pub fn lo(self) -> u8 {
        match self {
            Fidelity::Level(l) => l,
            Fidelity::Range(lo, _) => lo,
        }
    }

    /// Short label, `"F3"` or `"F1–3"`.
    pub fn label(self) -> String {
        match self {
            Fidelity::Level(l) => format!("F{l}"),
            Fidelity::Range(lo, hi) => format!("F{lo}\u{2013}{hi}"),
        }
    }

    /// What a level means, from the tag documentation.
    pub fn level_meaning(level: u8) -> &'static str {
        match level {
            0 => "lumped",
            1 => "one resolved dimension",
            2 => "two dimensions (subchannel)",
            3 => "three dimensions (CFD, diffusion)",
            4 => "brute force (Monte Carlo, first-principles data)",
            _ => "unknown",
        }
    }
}

/// The maturity rung's short label (#729, maintainer 2026-10-06).
pub fn maturity_label(level: u8) -> &'static str {
    match level {
        0 => "concept",
        1 => "AI draft",
        2 => "AI V&V",
        3 => "human reviewed",
        4 => "human V&V",
        _ => "unknown",
    }
}

/// Approximate width, in px, of `s` set in a bold sans-serif at `size` px
/// (0.62 em per character, a deliberate over-estimate for crate names).
pub fn label_width(s: &str, size: f64) -> f64 {
    s.chars().count() as f64 * 0.62 * size
}

/// Fit `s` into `avail` px: the largest font size from `max` down to `min`
/// that fits, else `min` and the text cut with an ellipsis. The full text
/// stays in the tooltip and the detail panel. Used by the SVG and the GUI
/// so both cut the same names.
pub fn fit_label(s: &str, avail: f64, max: f64, min: f64) -> (String, f64) {
    let fits = avail / label_width(s, 1.0).max(1e-9);
    if fits >= min {
        return (s.to_string(), fits.min(max));
    }
    let keep = ((avail / (0.62 * min)).floor() as usize).saturating_sub(1).max(1);
    let mut t: String = s.chars().take(keep).collect();
    t.push('\u{2026}');
    (t, min)
}

/// A module rated above its crate (`[[package.metadata.kovan.maturity_modules]]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaturityModule {
    /// Path from the library root, `a::b`.
    pub module: String,
    pub level: u8,
    pub why: String,
}

/// One workspace member and its tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrateNode {
    pub name: String,
    /// The `description` from `Cargo.toml`, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Degree of integration: 0-1 utilities, 2 domain solvers, 3 coupled
    /// multiphysics, 4 integrated GUI apps.
    pub row: u8,
    pub topic: Topic,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fidelity: Option<Fidelity>,
    /// The crate's lowest part, 0..=4.
    pub maturity: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub maturity_modules: Vec<MaturityModule>,
    /// Directory of the library's root source file, for checking that
    /// `maturity_modules` exist. Machine-specific, so never serialised.
    #[serde(skip)]
    pub lib_dir: Option<String>,
    /// The crate's folder relative to the workspace root, `/`-separated
    /// (e.g. `crates/kovan`), for links into the repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
}

impl CrateNode {
    /// `"2 · parts at 4"` when modules are rated higher, else `"2"`.
    pub fn maturity_summary(&self) -> String {
        match self.maturity_modules.iter().map(|m| m.level).max() {
            Some(top) => format!("{} \u{b7} parts at {top}", self.maturity),
            None => self.maturity.to_string(),
        }
    }
}

/// `from` needs `to` to build (a required normal dependency).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
}

/// The whole map's data: crates sorted by name, edges sorted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeMap {
    pub root: String,
    pub crates: Vec<CrateNode>,
    pub edges: Vec<Edge>,
}

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    #[serde(default)]
    workspace_root: Option<String>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    dependencies: Vec<Dependency>,
    #[serde(default)]
    targets: Vec<Target>,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
    #[serde(default)]
    manifest_path: Option<String>,
}

#[derive(Deserialize)]
struct Dependency {
    name: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    optional: bool,
}

#[derive(Deserialize)]
struct Target {
    kind: Vec<String>,
    src_path: String,
}

fn level(v: &serde_json::Value) -> Option<u8> {
    v.as_u64().filter(|l| *l <= 4).map(|l| l as u8)
}

/// Read one package's tag; every problem found is pushed to `errors`.
fn read_tag(p: &Package, root: Option<&str>, errors: &mut Vec<String>) -> Option<CrateNode> {
    let name = &p.name;
    let Some(t) = p.metadata.as_ref().map(|m| &m["kovan"]).filter(|t| t.is_object()) else {
        errors.push(format!("{name}: no [package.metadata.kovan] tag"));
        return None;
    };
    let start = errors.len();
    let row = level(&t["row"]);
    if row.is_none() {
        errors.push(format!("{name}: row must be 0..=4, found {}", t["row"]));
    }
    let topic = t["topic"].as_str().and_then(Topic::parse);
    if topic.is_none() {
        errors.push(format!("{name}: unknown topic {}", t["topic"]));
    }
    let fidelity = match &t["fidelity"] {
        serde_json::Value::Null => None,
        serde_json::Value::Array(r) => match r.as_slice() {
            [lo, hi] => match (level(lo), level(hi)) {
                (Some(lo), Some(hi)) if lo < hi => Some(Fidelity::Range(lo, hi)),
                _ => {
                    errors.push(format!("{name}: fidelity range {r:?} must be [lo, hi], 0 <= lo < hi <= 4"));
                    None
                }
            },
            _ => {
                errors.push(format!("{name}: fidelity range {r:?} must be [lo, hi], 0 <= lo < hi <= 4"));
                None
            }
        },
        v => match level(v) {
            Some(l) => Some(Fidelity::Level(l)),
            None => {
                errors.push(format!("{name}: fidelity {v} must be 0..=4"));
                None
            }
        },
    };
    if let Some(topic) = topic {
        if topic.has_no_fidelity() && fidelity.is_some() {
            errors.push(format!("{name}: a {} crate carries no fidelity", topic.as_str()));
        }
        if !topic.has_no_fidelity() && t["fidelity"].is_null() {
            errors.push(format!(
                "{name}: fidelity is required outside utilities and knowledge management"
            ));
        }
    }
    let maturity = level(&t["maturity"]);
    if maturity.is_none() {
        errors.push(format!("{name}: maturity must be 0..=4, found {}", t["maturity"]));
    }
    let mut modules = Vec::new();
    if let Some(list) = t.get("maturity_modules") {
        for m in list.as_array().into_iter().flatten() {
            let module = m["module"].as_str().unwrap_or_default().to_string();
            let why = m["why"].as_str().unwrap_or_default().to_string();
            let lvl = level(&m["level"]);
            if module.is_empty() {
                errors.push(format!("{name}: a maturity module has no module path"));
            }
            if why.trim().is_empty() {
                errors.push(format!("{name}: {module} needs a why"));
            }
            match (lvl, maturity) {
                (Some(l), Some(c)) if l > c => {}
                _ => errors.push(format!(
                    "{name}: {module} level {} must be above the crate's {}",
                    m["level"], t["maturity"]
                )),
            }
            modules.push(MaturityModule { module, level: lvl.unwrap_or(0), why });
        }
    }
    if errors.len() > start {
        return None;
    }
    let lib_dir = p
        .targets
        .iter()
        .find(|t| t.kind.iter().any(|k| k == "lib"))
        .and_then(|t| Path::new(&t.src_path).parent())
        .map(|d| d.to_string_lossy().into_owned());
    let dir = match (root, p.manifest_path.as_deref()) {
        (Some(root), Some(manifest)) => Path::new(manifest)
            .parent()
            .and_then(|d| d.strip_prefix(root).ok())
            .map(|d| d.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")),
        _ => None,
    };
    Some(CrateNode {
        dir,
        name: name.clone(),
        description: p.description.clone().filter(|d| !d.trim().is_empty()),
        row: row?,
        topic: topic?,
        fidelity,
        maturity: maturity?,
        maturity_modules: modules,
        lib_dir,
    })
}

impl CodeMap {
    /// Build the map from `cargo metadata --format-version 1 --no-deps`
    /// output. `Err` lists every malformed or missing tag, one line each.
    pub fn from_cargo_metadata(json: &str) -> Result<CodeMap, Vec<String>> {
        let meta: Metadata =
            serde_json::from_str(json).map_err(|e| vec![format!("cargo metadata is not the expected JSON: {e}")])?;
        let mut errors = Vec::new();
        let names: BTreeSet<&str> = meta.packages.iter().map(|p| p.name.as_str()).collect();
        let mut crates: Vec<CrateNode> =
            meta.packages.iter().filter_map(|p| read_tag(p, meta.workspace_root.as_deref(), &mut errors)).collect();
        if !errors.is_empty() {
            return Err(errors);
        }
        crates.sort_by(|a, b| a.name.cmp(&b.name));
        let mut edges = BTreeSet::new();
        for p in &meta.packages {
            for d in &p.dependencies {
                let required = d.kind.is_none() && !d.optional;
                if required && names.contains(d.name.as_str()) && d.name != p.name {
                    edges.insert(Edge { from: p.name.clone(), to: d.name.clone() });
                }
            }
        }
        Ok(CodeMap {
            root: ROOT_TITLE.to_string(),
            crates,
            edges: edges.into_iter().collect(),
        })
    }

    /// The crate called `name`.
    pub fn get(&self, name: &str) -> Option<&CrateNode> {
        self.crates
            .binary_search_by(|c| c.name.as_str().cmp(name))
            .ok()
            .map(|i| &self.crates[i])
    }

    /// What `name` depends on (required), sorted.
    pub fn dependencies(&self, name: &str) -> Vec<&str> {
        self.edges.iter().filter(|e| e.from == name).map(|e| e.to.as_str()).collect()
    }

    /// What depends on `name` (required), sorted.
    pub fn dependents(&self, name: &str) -> Vec<&str> {
        let mut v: Vec<&str> =
            self.edges.iter().filter(|e| e.to == name).map(|e| e.from.as_str()).collect();
        v.sort_unstable();
        v
    }

    /// Placement rules the tags must obey, against each other and the
    /// dependency graph (#733): row 4 is exactly the app topic, utilities
    /// sit in rows 0-1, and no crate sits below a required dependency.
    /// Empty when all hold.
    pub fn placement_problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for c in &self.crates {
            if (c.row == 4) != (c.topic == Topic::App) {
                problems.push(format!("{}: row 4 is the app row, and only it", c.name));
            }
            if c.topic == Topic::Utility && c.row > 1 {
                problems.push(format!("{}: a utility sits in row 0 or 1", c.name));
            }
        }
        let rows: BTreeMap<&str, u8> = self.crates.iter().map(|c| (c.name.as_str(), c.row)).collect();
        for e in &self.edges {
            if let (Some(&a), Some(&b)) = (rows.get(e.from.as_str()), rows.get(e.to.as_str())) {
                if a < b {
                    problems.push(format!(
                        "{} (row {a}) sits below {} (row {b}), which it depends on",
                        e.from, e.to
                    ));
                }
            }
        }
        problems
    }
}

/// Run `cargo metadata --format-version 1 --no-deps` in `workspace` and
/// return its JSON. Uses `$CARGO` when set (inside `cargo test`), else
/// `cargo` from `PATH`. The one function here that does I/O.
pub fn run_cargo_metadata(workspace: &Path, offline: bool) -> Result<String, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = std::process::Command::new(cargo);
    cmd.current_dir(workspace)
        .args(["metadata", "--format-version", "1", "--no-deps"]);
    if offline {
        cmd.arg("--offline");
    }
    let out = cmd.output().map_err(|e| format!("could not run cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata failed in {}: {}",
            workspace.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("cargo metadata output is not UTF-8: {e}"))
}

/// Build the map of the workspace at `workspace` (cargo metadata, then
/// [`CodeMap::from_cargo_metadata`]), errors joined one per line.
pub fn load_workspace(workspace: &Path) -> Result<CodeMap, String> {
    let json = run_cargo_metadata(workspace, false)?;
    CodeMap::from_cargo_metadata(&json).map_err(|e| e.join("\n"))
}

#[cfg(test)]
pub(crate) mod fixture {
    //! A small synthetic workspace in `cargo metadata` form, shaped like the
    //! real one: apps, two topic boxes with ranges, utilities, kovan family.

    fn pkg(name: &str, tag: &str, deps: &[(&str, Option<&str>, bool)]) -> String {
        let deps: Vec<String> = deps
            .iter()
            .map(|(n, kind, optional)| {
                let kind = kind.map(|k| format!("\"{k}\"")).unwrap_or_else(|| "null".into());
                format!(r#"{{"name":"{n}","kind":{kind},"optional":{optional}}}"#)
            })
            .collect();
        format!(
            r#"{{"name":"{name}","description":"the {name} crate","dependencies":[{}],
               "targets":[{{"kind":["lib"],"src_path":"/w/{name}/src/lib.rs"}}],
               "manifest_path":"/w/crates/{name}/Cargo.toml",
               "metadata":{{"kovan":{tag}}}}}"#,
            deps.join(",")
        )
    }

    pub(crate) fn json() -> String {
        let p = [
            pkg("app-a", r#"{"row":4,"topic":"app","fidelity":[0,1],"maturity":1}"#, &[("mc", None, false), ("kv", None, false)]),
            pkg("app-b", r#"{"row":4,"topic":"app","fidelity":0,"maturity":0}"#, &[("util", None, false)]),
            pkg("mc", r#"{"row":2,"topic":"neutronics","fidelity":4,"maturity":2}"#, &[("util", None, false), ("kv", Some("dev"), false)]),
            pkg("diff", r#"{"row":3,"topic":"neutronics","fidelity":3,"maturity":2}"#, &[("mc", None, false), ("util", None, true)]),
            pkg("pk", r#"{"row":2,"topic":"neutronics","fidelity":0,"maturity":1}"#, &[]),
            pkg("raffles", r#"{"row":2,"topic":"risk","fidelity":[0,4],"maturity":2}"#, &[("util", None, false)]),
            pkg("changi", r#"{"row":2,"topic":"risk","fidelity":[1,3],"maturity":2}"#, &[]),
            pkg("pflotran", r#"{"row":2,"topic":"risk","fidelity":3,"maturity":1}"#, &[]),
            pkg("redhill", r#"{"row":2,"topic":"risk","fidelity":3,"maturity":0}"#, &[]),
            pkg("buangkok", r#"{"row":2,"topic":"risk","fidelity":1,"maturity":2}"#, &[]),
            pkg("bishan", r#"{"row":2,"topic":"risk","fidelity":0,"maturity":1}"#, &[]),
            pkg("sembawang", r#"{"row":3,"topic":"risk","fidelity":0,"maturity":1}"#, &[("buangkok", None, false)]),
            pkg("steam", r#"{"row":2,"topic":"thermal-hydraulics","fidelity":0,"maturity":2,"maturity_modules":[{"module":"region_1","level":3,"why":"reviewed"}]}"#, &[]),
            pkg("util", r#"{"row":0,"topic":"utility","maturity":2}"#, &[]),
            pkg("mesh", r#"{"row":1,"topic":"utility","maturity":2}"#, &[("util", None, false)]),
            pkg("kv", r#"{"row":3,"topic":"knowledge-management","maturity":1}"#, &[("kv-common", None, false)]),
            pkg("kv-common", r#"{"row":0,"topic":"knowledge-management","maturity":1}"#, &[]),
        ];
        format!(r#"{{"workspace_root":"/w","packages":[{}]}}"#, p.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tags_and_keeps_only_required_internal_edges() {
        let map = CodeMap::from_cargo_metadata(&fixture::json()).unwrap();
        assert_eq!(map.crates.len(), 17);
        assert!(map.crates.windows(2).all(|w| w[0].name < w[1].name));
        // dev (mc -> kv) and optional (diff -> util) edges are dropped.
        assert!(!map.edges.contains(&Edge { from: "mc".into(), to: "kv".into() }));
        assert!(!map.edges.contains(&Edge { from: "diff".into(), to: "util".into() }));
        assert!(map.edges.contains(&Edge { from: "app-a".into(), to: "kv".into() }));
        assert_eq!(map.get("raffles").unwrap().fidelity, Some(Fidelity::Range(0, 4)));
        assert_eq!(map.get("steam").unwrap().maturity_summary(), "2 \u{b7} parts at 3");
        assert_eq!(map.dependents("util"), vec!["app-b", "mc", "mesh", "raffles"]);
        assert_eq!(map.get("mc").unwrap().dir.as_deref(), Some("crates/mc"));
        assert!(map.placement_problems().is_empty(), "{:?}", map.placement_problems());
    }

    #[test]
    fn json_round_trips_and_spells_fidelity_as_the_tag_does() {
        let map = CodeMap::from_cargo_metadata(&fixture::json()).unwrap();
        let s = serde_json::to_string(&map).unwrap();
        assert!(s.contains(r#""fidelity":[0,4]"#) && s.contains(r#""fidelity":4"#));
        let back: CodeMap = serde_json::from_str(&s).unwrap();
        let mut expected = map.clone();
        expected.crates.iter_mut().for_each(|c| c.lib_dir = None);
        assert_eq!(back, expected);
    }

    #[test]
    fn malformed_tags_are_all_reported() {
        let json = r#"{"packages":[
            {"name":"a","metadata":null},
            {"name":"b","metadata":{"kovan":{"row":7,"topic":"risk","fidelity":[3,1],"maturity":1}}},
            {"name":"c","metadata":{"kovan":{"row":1,"topic":"utility","maturity":2,"maturity_modules":[{"module":"x","level":1,"why":""}]}}}
        ]}"#;
        let errors = CodeMap::from_cargo_metadata(json).unwrap_err();
        let all = errors.join("\n");
        for needle in ["a: no [package.metadata.kovan]", "b: row", "b: fidelity range", "c: x needs a why", "c: x level 1"] {
            assert!(all.contains(needle), "missing {needle:?} in\n{all}");
        }
    }
}
