//! The concept-tree PROPOSALS (`src/concept_proposals.toml`, GitHub #724,
//! #726, #727, #729): level-3 concepts and the seed set of level-4
//! outram-park implementation leaves, awaiting the maintainer's approval.
//!
//! What is checked:
//! - every proposed concept path is unique, collides with no skeleton node,
//!   sits under an L2 node (depth >= 3), and its parent exists in the
//!   skeleton or among the proposals; segments are plain lower-case words;
//! - `origin` is `nrc` or `outram-park`, and an `outram-park` concept says
//!   `why`; every source names a skeleton `[[document]]`; cross-links resolve;
//! - every implementation names an existing concept, a crate that is a
//!   workspace member directory under `crates/`, and a module that exists as
//!   `crates/<crate>/src/<module>.rs` or `crates/<crate>/src/<module>/`.
//!   **This is the "tracked as develop evolves" gate**: a refactor that moves
//!   or renames a module fails here until the proposal is updated;
//! - `docs/concept-proposals.md` is the current rendering of the TOML.
//!   Regenerate it with
//!   `KOVAN_REGENERATE_PROPOSALS=1 cargo test --release -p kovan-literature --test concept_proposals`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
struct Skeleton {
    document: Vec<Document>,
    node: Vec<Node>,
}

#[derive(Deserialize)]
struct Document {
    id: String,
}

#[derive(Deserialize)]
struct Node {
    path: String,
    title: String,
}

#[derive(Deserialize)]
struct Proposals {
    concept: Vec<Concept>,
    implementation: Vec<Implementation>,
}

#[derive(Deserialize)]
struct Concept {
    path: String,
    title: String,
    origin: String,
    sources: Vec<Source>,
    #[serde(default)]
    why: Option<String>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    cross_links: Vec<String>,
    status: String,
}

#[derive(Deserialize)]
struct Source {
    document: String,
    section: String,
}

#[derive(Deserialize)]
struct Implementation {
    concept: String,
    #[serde(rename = "crate")]
    krate: String,
    module: String,
    title: String,
    what: String,
    kind: String,
    #[serde(default)]
    upstream: Option<String>,
    #[serde(default)]
    cross_links: Vec<String>,
    /// Concepts the work aims to support but is not validated, risk-assessed
    /// or licensed for (an "aspiration" link: shown dashed, never as a home).
    #[serde(default)]
    aspirations: Vec<String>,
    #[serde(default)]
    aspiration_prerequisites: Vec<String>,
    status: String,
}

fn skeleton() -> Skeleton {
    toml::from_str(include_str!("../src/concept_skeleton.toml")).expect("concept_skeleton.toml parses")
}

fn proposals() -> Proposals {
    toml::from_str(include_str!("../src/concept_proposals.toml")).expect("concept_proposals.toml parses")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Workspace member directories named `crates/<name>` in the root `Cargo.toml`.
fn workspace_member_crates() -> BTreeSet<String> {
    #[derive(Deserialize)]
    struct Root {
        workspace: Ws,
    }
    #[derive(Deserialize)]
    struct Ws {
        members: Vec<String>,
    }
    let text = std::fs::read_to_string(workspace_root().join("Cargo.toml")).expect("root Cargo.toml");
    let root: Root = toml::from_str(&text).expect("root Cargo.toml parses");
    root.workspace.members.iter().filter_map(|m| m.strip_prefix("crates/").map(str::to_owned)).collect()
}

/// The source file a `//! kovan-concept:` tag would go in, relative to the
/// workspace root, or `None` when the module does not exist.
fn module_file(krate: &str, module: &str) -> Option<String> {
    let src = workspace_root().join("crates").join(krate).join("src");
    let rel = |p: &Path| p.strip_prefix(workspace_root()).unwrap().to_string_lossy().replace('\\', "/");
    let file = src.join(format!("{module}.rs"));
    if file.is_file() {
        return Some(rel(&file));
    }
    let dir = src.join(module);
    if dir.join("mod.rs").is_file() {
        return Some(rel(&dir.join("mod.rs")));
    }
    if dir.is_dir() {
        return Some(format!("{}/", rel(&dir)));
    }
    None
}

#[test]
fn concepts_are_unique_and_hang_under_existing_nodes() {
    let s = skeleton();
    let p = proposals();
    let skel: BTreeSet<&str> = s.node.iter().map(|n| n.path.as_str()).collect();
    let mut seen = BTreeSet::new();
    for c in &p.concept {
        assert!(!skel.contains(c.path.as_str()), "{} is already a skeleton node", c.path);
        assert!(seen.insert(c.path.as_str()), "duplicate concept path {}", c.path);
        assert!(c.path.split('/').count() >= 3, "{}: a concept sits below an L2 node", c.path);
        assert!(!c.title.is_empty(), "{}: no title", c.path);
        assert!(matches!(c.status.as_str(), "proposed" | "approved" | "deferred"), "{}: status {}", c.path, c.status);
        for seg in c.path.split('/').skip(1) {
            assert!(
                !seg.is_empty() && seg.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'),
                "{}: segment {seg:?} is not plain lower-case words",
                c.path
            );
            assert!(!seg.starts_with(|ch: char| ch.is_ascii_digit()), "{}: numbered segment", c.path);
        }
    }
    for c in &p.concept {
        let (parent, _) = c.path.rsplit_once('/').unwrap();
        assert!(skel.contains(parent) || seen.contains(parent), "{}: parent {parent} missing", c.path);
        if skel.contains(parent) {
            assert_eq!(parent.split('/').count(), 2, "{}: parent {parent} is not an L2 node", c.path);
        }
        for x in &c.cross_links {
            assert!(skel.contains(x.as_str()) || seen.contains(x.as_str()), "{}: cross-link {x} unknown", c.path);
        }
    }
}

#[test]
fn origins_and_sources_are_declared() {
    let s = skeleton();
    let p = proposals();
    let docs: BTreeSet<&str> = s.document.iter().map(|d| d.id.as_str()).collect();
    for c in &p.concept {
        match c.origin.as_str() {
            "nrc" => {}
            "outram-park" => assert!(c.why.as_deref().is_some_and(|w| !w.is_empty()), "{}: outram-park concept needs `why`", c.path),
            o => panic!("{}: unknown origin {o}", c.path),
        }
        assert!(!c.sources.is_empty() || c.status == "deferred", "{}: no source (only a deferred concept may wait for one)", c.path);
        for src in &c.sources {
            assert!(docs.contains(src.document.as_str()), "{}: unknown document {}", c.path, src.document);
            assert!(!src.section.is_empty(), "{}: source without a section", c.path);
        }
    }
}

#[test]
fn implementations_name_existing_concepts_crates_and_modules() {
    let s = skeleton();
    let p = proposals();
    let concepts: BTreeSet<&str> =
        p.concept.iter().map(|c| c.path.as_str()).chain(s.node.iter().map(|n| n.path.as_str()).filter(|q| q.contains('/'))).collect();
    let members = workspace_member_crates();
    let mut seen = BTreeSet::new();
    let mut missing = Vec::new();
    for i in &p.implementation {
        assert!(concepts.contains(i.concept.as_str()), "{}::{}: concept {} does not exist", i.krate, i.module, i.concept);
        for x in &i.aspirations {
            assert!(concepts.contains(x.as_str()), "{}::{}: aspiration {x} does not exist", i.krate, i.module);
        }
        assert_eq!(i.aspirations.is_empty(), i.aspiration_prerequisites.is_empty(), "{}::{}: an aspiration must state its prerequisites", i.krate, i.module);
        for x in &i.cross_links {
            assert!(concepts.contains(x.as_str()), "{}::{}: cross-link {x} does not exist", i.krate, i.module);
        }
        assert!(members.contains(&i.krate), "{}: not a workspace member under crates/", i.krate);
        assert!(seen.insert((i.concept.as_str(), i.krate.as_str(), i.module.as_str())), "duplicate leaf {} {}::{}", i.concept, i.krate, i.module);
        assert!(!i.title.is_empty() && !i.what.is_empty(), "{}::{}: title and what are required", i.krate, i.module);
        assert!(matches!(i.status.as_str(), "proposed" | "approved" | "deferred"), "{}::{}: status {}", i.krate, i.module, i.status);
        match i.kind.as_str() {
            "port" => assert!(i.upstream.is_some(), "{}::{}: a port names its upstream", i.krate, i.module),
            "new-work" => {}
            k => panic!("{}::{}: unknown kind {k}", i.krate, i.module),
        }
        if module_file(&i.krate, &i.module).is_none() {
            missing.push(format!("crates/{}/src/{}(.rs|/)", i.krate, i.module));
        }
    }
    assert!(missing.is_empty(), "modules no longer on this branch; update concept_proposals.toml:\n{}", missing.join("\n"));
}

/// Renders the review document from the two TOML files.
fn render() -> String {
    let s = skeleton();
    let p = proposals();
    let titles: BTreeMap<&str, &str> = s.node.iter().map(|n| (n.path.as_str(), n.title.as_str())).collect();
    let mut leaves: BTreeMap<&str, Vec<&Implementation>> = BTreeMap::new();
    for i in &p.implementation {
        leaves.entry(i.concept.as_str()).or_default().push(i);
    }
    let n_nrc = p.concept.iter().filter(|c| c.origin == "nrc").count();
    let n_op = p.concept.len() - n_nrc;
    let mut per_crate: BTreeMap<&str, usize> = BTreeMap::new();
    for i in &p.implementation {
        *per_crate.entry(i.krate.as_str()).or_default() += 1;
    }

    let mut out = String::new();
    let w = &mut out;
    writeln!(w, "# Concept-tree proposals: L3 concepts and L4 seed tags").unwrap();
    writeln!(w).unwrap();
    writeln!(
        w,
        "> **Generated** from `src/concept_proposals.toml` by `tests/concept_proposals.rs` \
         (`KOVAN_REGENERATE_PROPOSALS=1 cargo test --release -p kovan-literature --test concept_proposals`). \
         Edit the TOML, not this file."
    )
    .unwrap();
    writeln!(w).unwrap();
    writeln!(
        w,
        "**Status: every line is a PROPOSAL** for the maintainer to approve or reject (GitHub #724, #726, #727, #729). \
         Nothing here is in the concept tree yet. Tick a box to approve; strike a line to reject."
    )
    .unwrap();
    writeln!(w).unwrap();
    writeln!(w, "## Counts").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "- **L3 concepts: {}** ({} from the NRC/ORNL text, `nrc`; {} needed by outram-park and named only implicitly, `outram-park`).", p.concept.len(), n_nrc, n_op).unwrap();
    writeln!(w, "- **L4 seed tags: {}** across {} crates.", p.implementation.len(), per_crate.len()).unwrap();
    writeln!(w).unwrap();
    writeln!(w, "| Crate | Seed tags |").unwrap();
    writeln!(w, "|---|---|").unwrap();
    for (k, n) in &per_crate {
        writeln!(w, "| `{k}` | {n} |").unwrap();
    }
    writeln!(w).unwrap();
    writeln!(w, "## How to read this").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "- **L3** concepts come from the subsections of the documents the skeleton already cites, read from the PDFs on 2026-10-06. Titles are the documents' own words, verbatim or lightly shortened. `origin: nrc` means the cited regulatory text names the concept itself (ORNL/TM-2018/976 counts here); `origin: outram-park` means the outram-park code needs it and the text names it only implicitly (the `why` line says why).").unwrap();
    writeln!(w, "- **L4** will be regenerated by kovan from `//! kovan-concept: <L3 path>` lines in each module's own doc comment (maintainer, 2026-10-06). The lines below are the **seed set** of those tags: each says which file the tag would be added to. No source file has been tagged yet. The test checks every named module still exists on `develop`.").unwrap();
    writeln!(w, "- World implementations (OpenMC, NJOY2016, GeN-Foam, FLEXPART, …) come first in Code Review and are the maintainer's to add; the `upstream` named on a leaf is what it ports or builds on.").unwrap();
    writeln!(w).unwrap();

    let l1s: Vec<&Node> = s.node.iter().filter(|n| !n.path.contains('/')).collect();
    let mut empty_l2 = Vec::new();
    for l1 in l1s {
        let l2s: Vec<&Node> = s.node.iter().filter(|n| n.path.rsplit_once('/').is_some_and(|(par, _)| par == l1.path)).collect();
        let has_any = l2s.iter().any(|l2| p.concept.iter().any(|c| c.path.starts_with(&format!("{}/", l2.path))));
        if !has_any {
            for l2 in &l2s {
                empty_l2.push(l2.path.as_str());
            }
            continue;
        }
        writeln!(w, "## {} (`{}`)", l1.title, l1.path).unwrap();
        writeln!(w).unwrap();
        for l2 in l2s {
            let prefix = format!("{}/", l2.path);
            let cs: Vec<&Concept> = p.concept.iter().filter(|c| c.path.starts_with(&prefix)).collect();
            if cs.is_empty() {
                empty_l2.push(l2.path.as_str());
                continue;
            }
            writeln!(w, "### {} (`{}`)", l2.title, l2.path).unwrap();
            writeln!(w).unwrap();
            for c in cs {
                let depth = c.path.split('/').count() - 3;
                let ind = "  ".repeat(depth);
                let rel = &c.path[prefix.len()..];
                let srcs: Vec<String> = c.sources.iter().map(|x| format!("`{}` {}", x.document, x.section)).collect();
                writeln!(w, "{ind}- [ ] **{}** · `{}` · origin `{}`", c.title, rel, c.origin).unwrap();
                writeln!(w, "{ind}  - sources: {}", srcs.join("; ")).unwrap();
                if let Some(why) = &c.why {
                    writeln!(w, "{ind}  - why: {why}").unwrap();
                }
                if let Some(note) = &c.note {
                    writeln!(w, "{ind}  - **note for review:** {note}").unwrap();
                }
                if !c.cross_links.is_empty() {
                    writeln!(w, "{ind}  - cross-links: {}", c.cross_links.iter().map(|x| format!("`{x}`")).collect::<Vec<_>>().join(", ")).unwrap();
                }
                if let Some(ls) = leaves.get(c.path.as_str()) {
                    writeln!(w, "{ind}  - proposed kovan-concept tags:").unwrap();
                    for i in ls {
                        let file = module_file(&i.krate, &i.module).unwrap_or_else(|| format!("crates/{}/src/{} (MISSING)", i.krate, i.module));
                        let up = match (&i.kind[..], &i.upstream) {
                            ("port", Some(u)) => format!("port of {u}"),
                            (_, Some(u)) => format!("new work, from {u}"),
                            _ => "new work".to_owned(),
                        };
                        writeln!(w, "{ind}    - [ ] add `//! kovan-concept: {}` to `{file}`: {}. {} ({up})", c.path, i.title, i.what).unwrap();
                    }
                }
            }
            writeln!(w).unwrap();
        }
    }
    let unplaced: Vec<&Implementation> = p.implementation.iter().filter(|i| titles.contains_key(i.concept.as_str())).collect();
    if !unplaced.is_empty() {
        writeln!(w, "## Leaves attached directly to skeleton nodes").unwrap();
        writeln!(w).unwrap();
        for i in unplaced {
            writeln!(w, "- [ ] `{}`: `{}` `{}`", i.concept, i.krate, i.module).unwrap();
        }
        writeln!(w).unwrap();
    }
    writeln!(w, "## L2 nodes with no proposed concept ({})", empty_l2.len()).unwrap();
    writeln!(w).unwrap();
    writeln!(w, "Left empty on purpose: a single short source chapter, a fuel-cycle-facility or environmental-review chapter with no outram-park work and no subsection grain worth splitting, or an L2 whose only source is the IAEA text. They stay greyed until content arrives.").unwrap();
    writeln!(w).unwrap();
    for path in empty_l2 {
        writeln!(w, "- `{path}` ({})", titles[path]).unwrap();
    }
    out
}

#[test]
fn review_document_is_current() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs").join("concept-proposals.md");
    let fresh = render();
    if std::env::var_os("KOVAN_REGENERATE_PROPOSALS").is_some() {
        std::fs::write(&path, &fresh).expect("write docs/concept-proposals.md");
        return;
    }
    let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        on_disk == fresh,
        "docs/concept-proposals.md is stale; regenerate with KOVAN_REGENERATE_PROPOSALS=1 cargo test --release -p kovan-literature --test concept_proposals"
    );
}
