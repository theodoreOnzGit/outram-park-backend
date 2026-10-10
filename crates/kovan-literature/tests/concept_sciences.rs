//! The SCIENCES tree proposals (`src/concept_sciences_proposals.toml`,
//! GitHub #727, maintainer direction 2026-10-10): the second tab of the
//! concept map, beside the IAEA/NRC application tree, which references it.
//!
//! What is checked:
//! - every Sciences path is unique, starts `sciences/`, is made of plain
//!   lower-case hyphenated words with no digits, names no workspace crate,
//!   and its parent exists (a two-segment path is a top-level branch);
//! - every source names a document declared in the skeleton or in the
//!   sciences file (ids may not collide); a node with no source says
//!   "source not yet read"; every document declared here is cited; a
//!   corpus document's file exists when the `reactor-literature` submodule is
//!   checked out (skipped when it is empty);
//! - every cross-link resolves to another Sciences node;
//! - a `kind = "method"` node sits under `sciences/computation/problems/`, is
//!   dated, and its theorems, aims and `superseded_by` resolve; no other node
//!   carries method fields;
//! - every `[[reference]]` names an existing skeleton node, concept or
//!   implementation leaf, and every Sciences path it lists resolves; so does
//!   every optional `sciences = [...]` field in `concept_proposals.toml`, and
//!   the tree reader accepts that field (the schema change is additive);
//! - **approved ids are permanent**: every path in
//!   `src/concept_sciences_approved.txt` exists and is `approved`, and every
//!   `approved` node is listed there;
//! - `docs/concept-sciences-proposals.md` is the current rendering. Regenerate
//!   it with
//!   `KOVAN_REGENERATE_SCIENCES=1 cargo test --release -p kovan-literature --test concept_sciences`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize)]
struct Skeleton {
    document: Vec<SkelDocument>,
    node: Vec<SkelNode>,
}

#[derive(Deserialize)]
struct SkelDocument {
    id: String,
    licence: String,
}

#[derive(Deserialize)]
struct SkelNode {
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
    #[serde(default)]
    sciences: Vec<String>,
}

#[derive(Deserialize)]
struct Implementation {
    #[serde(rename = "crate")]
    krate: String,
    module: String,
    #[serde(default)]
    sciences: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sciences {
    document: Vec<Document>,
    science: Vec<Science>,
    #[serde(default)]
    reference: Vec<Reference>,
    #[serde(default)]
    candidate: Vec<Candidate>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    id: String,
    title: String,
    publisher: String,
    date: String,
    holder: String,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    url: Option<String>,
    licence: String,
    redistributable: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Science {
    path: String,
    title: String,
    sources: Vec<Source>,
    #[serde(default)]
    cross_links: Vec<String>,
    status: String,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    dated: Option<String>,
    #[serde(default)]
    superseded_by: Option<String>,
    #[serde(default)]
    theorems: Vec<String>,
    #[serde(default)]
    aims: Vec<String>,
    #[serde(default)]
    order_of_accuracy: Option<String>,
    #[serde(default)]
    cost_scaling: Option<String>,
    #[serde(default)]
    memory: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    document: String,
    section: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    #[serde(default)]
    node: Option<String>,
    #[serde(default, rename = "crate")]
    krate: Option<String>,
    #[serde(default)]
    module: Option<String>,
    sciences: Vec<String>,
    status: String,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Candidate {
    title: String,
    url: String,
    licence: String,
    why: String,
}

const SCIENCES_TOML: &str = include_str!("../src/concept_sciences_proposals.toml");
const APPROVED_TXT: &str = include_str!("../src/concept_sciences_approved.txt");
const COMPUTATION_PROBLEMS: &str = "sciences/computation/problems/";
const THEOREMS: &str = "sciences/computation/governing-theorems/";
const AIMS: &str = "sciences/computation/aims/";
const NOT_READ: &str = "source not yet read";

fn skeleton() -> Skeleton {
    toml::from_str(include_str!("../src/concept_skeleton.toml"))
        .expect("concept_skeleton.toml parses")
}

fn proposals() -> Proposals {
    toml::from_str(include_str!("../src/concept_proposals.toml"))
        .expect("concept_proposals.toml parses")
}

fn sciences() -> Sciences {
    toml::from_str(SCIENCES_TOML).expect("concept_sciences_proposals.toml parses")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Workspace member directory names under `crates/`.
fn workspace_member_crates() -> BTreeSet<String> {
    #[derive(Deserialize)]
    struct Root {
        workspace: Ws,
    }
    #[derive(Deserialize)]
    struct Ws {
        members: Vec<String>,
    }
    let text =
        std::fs::read_to_string(workspace_root().join("Cargo.toml")).expect("root Cargo.toml");
    let root: Root = toml::from_str(&text).expect("root Cargo.toml parses");
    root.workspace
        .members
        .iter()
        .filter_map(|m| m.strip_prefix("crates/").map(str::to_owned))
        .collect()
}

fn science_paths(s: &Sciences) -> BTreeSet<&str> {
    s.science.iter().map(|n| n.path.as_str()).collect()
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map(|(p, _)| p).unwrap_or("")
}

fn approved_ledger() -> Vec<&'static str> {
    APPROVED_TXT
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

#[test]
fn paths_are_permanent_shaped_unique_and_parented() {
    let s = sciences();
    let crates = workspace_member_crates();
    let paths = science_paths(&s);
    assert_eq!(paths.len(), s.science.len(), "duplicate Sciences path");
    for n in &s.science {
        let rest = n
            .path
            .strip_prefix("sciences/")
            .unwrap_or_else(|| panic!("{}: must start with sciences/", n.path));
        for seg in rest.split('/') {
            assert!(
                !seg.is_empty()
                    && seg
                        .split('-')
                        .all(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase())),
                "{}: segment {seg:?} is not plain lower-case hyphenated words without digits",
                n.path
            );
            assert!(
                !crates.contains(seg),
                "{}: segment {seg:?} names a crate; ids name the physics",
                n.path
            );
        }
        let p = parent(&n.path);
        assert!(
            p == "sciences" || paths.contains(p),
            "{}: parent {p} missing",
            n.path
        );
        assert!(!n.title.is_empty(), "{}: no title", n.path);
        assert!(
            matches!(n.status.as_str(), "proposed" | "approved" | "deferred"),
            "{}: status {}",
            n.path,
            n.status
        );
    }
    for root in ["sciences/physical-principles", "sciences/computation"] {
        assert!(paths.contains(root), "top-level branch {root} missing");
    }
}

#[test]
fn sources_name_declared_documents_and_every_document_is_cited() {
    let sk = skeleton();
    let s = sciences();
    let skel_docs: BTreeSet<&str> = sk.document.iter().map(|d| d.id.as_str()).collect();
    let mut local = BTreeSet::new();
    for d in &s.document {
        assert!(local.insert(d.id.as_str()), "duplicate document {}", d.id);
        assert!(
            !skel_docs.contains(d.id.as_str()),
            "{}: already declared in concept_skeleton.toml",
            d.id
        );
        assert!(
            !d.title.is_empty()
                && !d.publisher.is_empty()
                && !d.date.is_empty()
                && !d.licence.is_empty(),
            "{}: incomplete",
            d.id
        );
        match d.holder.as_str() {
            "standard" => assert!(
                d.file
                    .as_deref()
                    .is_some_and(|f| f.starts_with("kovan-standard-open-corpus/")),
                "{}: standard needs a file there",
                d.id
            ),
            "theodore-open" => assert!(
                d.file
                    .as_deref()
                    .is_some_and(|f| f.starts_with("theodore-open-corpus/")),
                "{}: theodore-open needs a file there",
                d.id
            ),
            "web" => assert!(
                d.url.as_deref().is_some_and(|u| u.starts_with("https://")) && d.file.is_none(),
                "{}: web needs an https url and no file",
                d.id
            ),
            h => panic!("{}: unknown holder {h}", d.id),
        }
    }
    let mut cited = BTreeSet::new();
    for n in &s.science {
        if n.sources.is_empty() {
            assert!(
                n.note.as_deref().is_some_and(|x| x.contains(NOT_READ)),
                "{}: no source and no '{NOT_READ}' note",
                n.path
            );
        }
        for src in &n.sources {
            assert!(
                local.contains(src.document.as_str()) || skel_docs.contains(src.document.as_str()),
                "{}: unknown document {}",
                n.path,
                src.document
            );
            assert!(
                !src.section.is_empty(),
                "{}: source without a section",
                n.path
            );
            cited.insert(src.document.as_str());
        }
    }
    for d in &local {
        assert!(
            cited.contains(d),
            "document {d} is declared but never cited"
        );
    }
    // Files exist when the submodule is checked out.
    let lit = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("reactor-literature");
    if lit.join("kovan-standard-open-corpus").is_dir() {
        for d in &s.document {
            if let Some(f) = &d.file {
                assert!(
                    lit.join(f).is_file(),
                    "{}: {f} not in reactor-literature",
                    d.id
                );
            }
        }
    } else {
        eprintln!("note: reactor-literature submodule is empty; document files not checked");
    }
}

#[test]
fn cross_links_resolve_within_sciences() {
    let s = sciences();
    let paths = science_paths(&s);
    for n in &s.science {
        for x in &n.cross_links {
            assert!(
                paths.contains(x.as_str()),
                "{}: cross-link {x} unknown",
                n.path
            );
            assert_ne!(x, &n.path, "{}: links to itself", n.path);
        }
    }
}

#[test]
fn method_leaves_sit_under_problems_and_name_theorems_and_aims() {
    let s = sciences();
    let paths = science_paths(&s);
    let methods: BTreeSet<&str> = s
        .science
        .iter()
        .filter(|n| n.kind.as_deref() == Some("method"))
        .map(|n| n.path.as_str())
        .collect();
    for n in &s.science {
        match n.kind.as_deref() {
            Some("method") => {
                assert!(
                    n.path.starts_with(COMPUTATION_PROBLEMS),
                    "{}: a method sits under a Problem",
                    n.path
                );
                let d = n
                    .dated
                    .as_deref()
                    .unwrap_or_else(|| panic!("{}: a method is dated", n.path));
                assert!(
                    d.len() == 10
                        && d.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 {
                            c == '-'
                        } else {
                            c.is_ascii_digit()
                        }),
                    "{}: dated {d:?} is not YYYY-MM-DD",
                    n.path
                );
                for t in &n.theorems {
                    assert!(
                        t.starts_with(THEOREMS) && paths.contains(t.as_str()),
                        "{}: theorem {t} unknown",
                        n.path
                    );
                }
                for a in &n.aims {
                    assert!(
                        a.starts_with(AIMS) && paths.contains(a.as_str()),
                        "{}: aim {a} unknown",
                        n.path
                    );
                }
                if let Some(x) = &n.superseded_by {
                    assert!(
                        methods.contains(x.as_str()) && x != &n.path,
                        "{}: superseded_by {x} is not another method",
                        n.path
                    );
                }
            }
            None => assert!(
                n.dated.is_none()
                    && n.superseded_by.is_none()
                    && n.theorems.is_empty()
                    && n.aims.is_empty()
                    && n.order_of_accuracy.is_none()
                    && n.cost_scaling.is_none()
                    && n.memory.is_none(),
                "{}: method fields on a node that is not kind = \"method\"",
                n.path
            ),
            Some(k) => panic!("{}: unknown kind {k}", n.path),
        }
    }
}

#[test]
fn references_and_sciences_fields_resolve() {
    let sk = skeleton();
    let p = proposals();
    let s = sciences();
    let paths = science_paths(&s);
    let iaea: BTreeSet<&str> = sk
        .node
        .iter()
        .map(|n| n.path.as_str())
        .chain(p.concept.iter().map(|c| c.path.as_str()))
        .collect();
    let leaves: BTreeSet<(&str, &str)> = p
        .implementation
        .iter()
        .map(|i| (i.krate.as_str(), i.module.as_str()))
        .collect();
    let mut seen = BTreeSet::new();
    for r in &s.reference {
        let key = match (&r.node, &r.krate, &r.module) {
            (Some(n), None, None) => {
                assert!(
                    iaea.contains(n.as_str()),
                    "reference: node {n} is not a skeleton node or concept"
                );
                n.clone()
            }
            (None, Some(c), Some(m)) => {
                assert!(
                    leaves.contains(&(c.as_str(), m.as_str())),
                    "reference: {c}::{m} is not an [[implementation]] leaf"
                );
                format!("{c}::{m}")
            }
            _ => panic!("a reference names either `node` or `crate` + `module`"),
        };
        assert!(seen.insert(key.clone()), "duplicate reference for {key}");
        assert!(!r.sciences.is_empty(), "{key}: no Sciences paths");
        assert!(
            matches!(r.status.as_str(), "proposed" | "approved"),
            "{key}: status {}",
            r.status
        );
        let mut uniq = BTreeSet::new();
        for x in &r.sciences {
            assert!(
                paths.contains(x.as_str()),
                "{key}: Sciences path {x} unknown"
            );
            assert!(uniq.insert(x), "{key}: {x} listed twice");
        }
    }
    for c in &p.concept {
        for x in &c.sciences {
            assert!(
                paths.contains(x.as_str()),
                "{}: sciences {x} unknown",
                c.path
            );
        }
    }
    for i in &p.implementation {
        for x in &i.sciences {
            assert!(
                paths.contains(x.as_str()),
                "{}::{}: sciences {x} unknown",
                i.krate,
                i.module
            );
        }
    }
}

/// The optional `sciences` field is additive: the tree reader Kovan uses
/// accepts a concept carrying it.
#[test]
fn the_tree_reader_accepts_a_sciences_field() {
    let skeleton = "[[document]]\nid = \"d\"\ntitle = \"D\"\npublisher = \"P\"\ndate = \"2000\"\ntier = \"standard\"\nfile = \"f.pdf\"\nlicence = \"L\"\n\n\
                    [[node]]\npath = \"01-a\"\ntitle = \"A\"\nsources = [{ document = \"d\", section = \"1\" }]\n\n\
                    [[node]]\npath = \"01-a/b\"\ntitle = \"B\"\nsources = [{ document = \"d\", section = \"1\" }]\n";
    let proposals = "[[concept]]\npath = \"01-a/b/c\"\ntitle = \"C\"\norigin = \"nrc\"\nsources = [{ document = \"d\", section = \"1.1\" }]\n\
                     sciences = [\"sciences/physical-principles/conservation-laws\"]\nstatus = \"approved\"\n";
    let t =
        kovan_literature::ConceptTree::parse(skeleton, proposals).expect("a sciences field parses");
    assert!(t.node("01-a/b/c").is_some());
}

#[test]
fn approved_sciences_ids_never_disappear() {
    let s = sciences();
    let by_path: BTreeMap<&str, &Science> =
        s.science.iter().map(|n| (n.path.as_str(), n)).collect();
    let ledger = approved_ledger();
    let listed: BTreeSet<&str> = ledger.iter().copied().collect();
    assert_eq!(
        listed.len(),
        ledger.len(),
        "concept_sciences_approved.txt lists a path twice"
    );
    for id in &ledger {
        let n = by_path.get(id).unwrap_or_else(|| {
            panic!("approved Sciences id {id} has disappeared; approved ids are permanent")
        });
        assert_eq!(
            n.status, "approved",
            "approved Sciences id {id} is no longer approved"
        );
    }
    for n in &s.science {
        if n.status == "approved" {
            assert!(
                listed.contains(n.path.as_str()),
                "{} is approved but not recorded in concept_sciences_approved.txt",
                n.path
            );
        }
    }
}

fn md(s: &str) -> String {
    s.replace('|', "\\|")
}

/// Renders the review document from the TOML files.
fn render() -> String {
    let sk = skeleton();
    let s = sciences();
    let mut licences: BTreeMap<&str, &str> = sk
        .document
        .iter()
        .map(|d| (d.id.as_str(), d.licence.as_str()))
        .collect();
    for d in &s.document {
        licences.insert(d.id.as_str(), d.licence.as_str());
    }
    let mut kids: BTreeMap<&str, Vec<&Science>> = BTreeMap::new();
    for n in &s.science {
        kids.entry(parent(&n.path)).or_default().push(n);
    }
    let unread: Vec<&Science> = s
        .science
        .iter()
        .filter(|n| n.note.as_deref().is_some_and(|x| x.contains(NOT_READ)))
        .collect();
    let methods = s.science.iter().filter(|n| n.kind.is_some()).count();
    let short = |p: &str| p.strip_prefix("sciences/").unwrap_or(p).to_owned();

    let mut out = String::new();
    let w = &mut out;
    writeln!(w, "# Sciences tree proposals (concept map, second tab)").unwrap();
    writeln!(w).unwrap();
    writeln!(
        w,
        "> **Generated** from `src/concept_sciences_proposals.toml` by `tests/concept_sciences.rs` \
         (`KOVAN_REGENERATE_SCIENCES=1 cargo test --release -p kovan-literature --test concept_sciences`). \
         Edit the TOML, not this file."
    )
    .unwrap();
    writeln!(w).unwrap();
    writeln!(w, "**Status: DRAFT FOR MAINTAINER REVIEW.** Every node and reference here is `proposed` (GitHub #727, maintainer direction 2026-10-10). Nothing is in either tree until approved.").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "## Two tabs").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "1. **IAEA (application)**: the approved skeleton (19 IAEA NG-G-3.1 issues, then NRC L2 categories), the L3 engineering concepts and the L4 outram-park implementation leaves of `concept_proposals.toml`. Unchanged.").unwrap();
    writeln!(w, "2. **Sciences** (this draft): physical principles and computation, then the disciplines that branch out of them. The IAEA tab *references* it: an L2 node, an L3 concept or an L4 leaf names the Sciences nodes it rests on, as many as apply.").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "Software engineering and QA (the human and management side: management systems, quality assurance, the V&V process) belong to the IAEA tab, not here. Their nodes are a later pass; they will reference Computation's *trust* aims.").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "## Node ids are permanent").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "A Sciences node's id is its path, `sciences/<segment>/...`: plain lower-case words joined by `-`, no digits, naming the physics or the mathematics, never a source, a document number, a code or a crate. Function-level review stamps will link Sciences nodes (review.md `[[relation]] kind = \"implements\"`), so **once a node is approved its path never changes and never disappears**: the tree grows additively. Approval appends the path to `src/concept_sciences_approved.txt`, and the test fails if a listed id is missing or no longer approved. A computation method is never deleted; it is marked `superseded_by`.").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "## Counts").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "- **Nodes: {}** ({} under physical principles, {} under computation of which {} are example methods, {} in the disciplines).", s.science.len(),
        s.science.iter().filter(|n| n.path.starts_with("sciences/physical-principles")).count(),
        s.science.iter().filter(|n| n.path.starts_with("sciences/computation")).count(), methods,
        s.science.iter().filter(|n| !n.path.starts_with("sciences/physical-principles") && !n.path.starts_with("sciences/computation")).count()).unwrap();
    writeln!(
        w,
        "- **Nodes whose source is not yet read: {}**.",
        unread.len()
    )
    .unwrap();
    writeln!(
        w,
        "- **Documents cited: {}** declared here, plus skeleton documents.",
        s.document.len()
    )
    .unwrap();
    writeln!(w, "- **Proposed references: {}** ({} from skeleton L1/L2 nodes, the rest a sample of L3 concepts and L4 leaves).", s.reference.len(),
        s.reference.iter().filter(|r| r.node.as_deref().is_some_and(|n| n.split('/').count() <= 2)).count()).unwrap();
    writeln!(w).unwrap();

    writeln!(w, "## The tree").unwrap();
    writeln!(w).unwrap();
    writeln!(
        w,
        "`(method)` marks an example method leaf; `~>` lists cross-links (the graph)."
    )
    .unwrap();
    writeln!(w).unwrap();
    writeln!(w, "```text").unwrap();
    writeln!(w, "Sciences").unwrap();
    fn walk(w: &mut String, kids: &BTreeMap<&str, Vec<&Science>>, at: &str, prefix: &str) {
        let Some(list) = kids.get(at) else { return };
        for (i, n) in list.iter().enumerate() {
            let last = i + 1 == list.len();
            let seg = n.path.rsplit('/').next().unwrap();
            let tag = if n.kind.is_some() { " (method)" } else { "" };
            writeln!(
                w,
                "{prefix}{}{seg}{tag}",
                if last { "`-- " } else { "|-- " }
            )
            .unwrap();
            walk(
                w,
                kids,
                &n.path,
                &format!("{prefix}{}", if last { "    " } else { "|   " }),
            );
        }
    }
    walk(w, &kids, "sciences", "");
    writeln!(w, "```").unwrap();
    writeln!(w).unwrap();

    writeln!(w, "## Computation: why problems, theorems and aims").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "Algorithms evolve fast; the problems they solve and the theorems that bound them do not, so the permanent ids live on **Problems**, **Governing theorems** and **Aims**. **Methods** are leaves under the Problem they solve: each is dated, never deleted (marked `superseded_by`), and names the theorems and aims it trades on, with its accuracy-versus-speed properties recorded (order of accuracy, cost scaling, memory). The aims are the motivation: **accuracy** and **speed**, often orthogonal, and **trust**, because an irreproducible run that is fast and accurate is worthless for V&V (the HTR-10 bed that gave a different answer every run because a neighbour grid iterated a randomly seeded `HashMap`, root `CLAUDE.md`). Only a handful of example methods are drafted, to show the shape.").unwrap();
    writeln!(w).unwrap();

    writeln!(w, "## Nodes").unwrap();
    writeln!(w).unwrap();
    writeln!(
        w,
        "| Node | Title | Parent; cross-links | Sources (document, section) | Licence | Note |"
    )
    .unwrap();
    writeln!(w, "|---|---|---|---|---|---|").unwrap();
    for n in &s.science {
        let mut rel = format!("`{}`", short(parent(&n.path)));
        if !n.cross_links.is_empty() {
            rel.push_str("; ~> ");
            rel.push_str(
                &n.cross_links
                    .iter()
                    .map(|x| format!("`{}`", short(x)))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        let srcs = if n.sources.is_empty() {
            "(none read)".to_owned()
        } else {
            n.sources
                .iter()
                .map(|x| format!("`{}` {}", x.document, md(&x.section)))
                .collect::<Vec<_>>()
                .join("; ")
        };
        let mut lic: Vec<&str> = n
            .sources
            .iter()
            .filter_map(|x| licences.get(x.document.as_str()).copied())
            .collect();
        lic.dedup();
        let lic: BTreeSet<&str> = lic.into_iter().collect();
        let mut note = n.note.clone().unwrap_or_default();
        if n.kind.is_some() {
            let mut extra = vec![format!(
                "method, dated {}",
                n.dated.as_deref().unwrap_or("?")
            )];
            if !n.theorems.is_empty() {
                extra.push(format!(
                    "theorems: {}",
                    n.theorems
                        .iter()
                        .map(|x| format!("`{}`", short(x)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if !n.aims.is_empty() {
                extra.push(format!(
                    "aims: {}",
                    n.aims
                        .iter()
                        .map(|x| format!("`{}`", short(x)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            for (k, v) in [
                ("order of accuracy", &n.order_of_accuracy),
                ("cost", &n.cost_scaling),
                ("memory", &n.memory),
            ] {
                if let Some(v) = v {
                    extra.push(format!("{k}: {v}"));
                }
            }
            if let Some(x) = &n.superseded_by {
                extra.push(format!("superseded by `{}`", short(x)));
            }
            if !note.is_empty() {
                extra.push(note);
            }
            note = extra.join("; ");
        }
        writeln!(
            w,
            "| `{}` | {} | {} | {} | {} | {} |",
            short(&n.path),
            md(&n.title),
            rel,
            srcs,
            md(&lic.into_iter().collect::<Vec<_>>().join("; ")),
            md(&note)
        )
        .unwrap();
    }
    writeln!(w).unwrap();

    writeln!(w, "## Documents declared by the Sciences file").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "Skeleton documents (NJOY2016 manual, NUREG/KM-0006, SAND2011-2195, the Brown slides, OpenMC's depletion and eigenvalue pages, NASA-STD/HDBK-7009B, and others) are cited by id and listed in `concept_skeleton.toml`.").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "| Id | Document | Where | Licence | Redistributable |").unwrap();
    writeln!(w, "|---|---|---|---|---|").unwrap();
    for d in &s.document {
        let wh = match (d.holder.as_str(), &d.file, &d.url) {
            ("web", _, Some(u)) => format!("web: <{u}>"),
            (h, Some(f), _) => format!("{h}: `{f}`"),
            (h, _, _) => h.to_owned(),
        };
        writeln!(
            w,
            "| `{}` | {} ({}, {}) | {} | {} | {} |",
            d.id,
            md(&d.title),
            md(&d.publisher),
            md(&d.date),
            wh,
            md(&d.licence),
            if d.redistributable { "yes" } else { "no" }
        )
        .unwrap();
    }
    writeln!(w).unwrap();

    let refs_by_node: BTreeMap<&str, &Reference> = s
        .reference
        .iter()
        .filter_map(|r| r.node.as_deref().map(|n| (n, r)))
        .collect();
    writeln!(w, "## IAEA/NRC L2 to Sciences (proposed)").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "Every L2 node of the approved skeleton, with the Sciences nodes it would reference. A dash means no physics or computation reference is proposed (management, legal, financial and stakeholder categories). Software engineering and QA nodes will live on this IAEA side and reference Computation's trust aims.").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "| L2 node | Title | Sciences references | Note |").unwrap();
    writeln!(w, "|---|---|---|---|").unwrap();
    for n in sk.node.iter().filter(|n| n.path.split('/').count() == 2) {
        let (refs, note) = match refs_by_node.get(n.path.as_str()) {
            Some(r) => (
                r.sciences
                    .iter()
                    .map(|x| format!("`{}`", short(x)))
                    .collect::<Vec<_>>()
                    .join(", "),
                r.note.clone().unwrap_or_default(),
            ),
            None => ("-".to_owned(), String::new()),
        };
        writeln!(
            w,
            "| `{}` | {} | {} | {} |",
            n.path,
            md(&n.title),
            refs,
            md(&note)
        )
        .unwrap();
    }
    writeln!(w).unwrap();

    writeln!(
        w,
        "## Sample: L3 concepts and L4 implementation leaves to Sciences (proposed)"
    )
    .unwrap();
    writeln!(w).unwrap();
    writeln!(w, "A sample, not all 309 concepts and 192 leaves, to show the shape: an implementation leaf rests on many Sciences nodes. Once approved, each list moves into the optional `sciences = [...]` field of that `[[concept]]` or `[[implementation]]` in `concept_proposals.toml`.").unwrap();
    writeln!(w).unwrap();
    writeln!(w, "| IAEA-side entry | Sciences references |").unwrap();
    writeln!(w, "|---|---|").unwrap();
    for r in &s.reference {
        let who = match (&r.node, &r.krate, &r.module) {
            (Some(n), _, _) if n.split('/').count() <= 2 => continue,
            (Some(n), _, _) => format!("L3 `{n}`"),
            (None, Some(c), Some(m)) => format!("L4 `{c}` `{m}`"),
            _ => unreachable!(),
        };
        writeln!(
            w,
            "| {} | {} |",
            who,
            r.sciences
                .iter()
                .map(|x| format!("`{}`", short(x)))
                .collect::<Vec<_>>()
                .join(", ")
        )
        .unwrap();
    }
    writeln!(w).unwrap();
    writeln!(w, "**How the existing implementation leaves sit.** They keep their home under the IAEA tab (an L3 concept, as today). The mapping rule for the Sciences references: a leaf names (1) every governing equation or physical law its code evaluates, at the finest Sciences node that names it; (2) the Computation Problem it solves and, where one is drafted, the Method it uses; (3) the Computation Aims its tests pin (for example determinism where a test asserts identical reruns); and (4) the discipline node it belongs to. Function-level review stamps then link the same ids through `implements` relations.").unwrap();
    writeln!(w).unwrap();

    writeln!(
        w,
        "## Nodes whose source is not yet read ({})",
        unread.len()
    )
    .unwrap();
    writeln!(w).unwrap();
    for n in &unread {
        writeln!(
            w,
            "- `{}` ({}): {}",
            short(&n.path),
            md(&n.title),
            md(n.note.as_deref().unwrap_or(""))
        )
        .unwrap();
    }
    writeln!(w).unwrap();

    writeln!(w, "## Candidate documents for the corpus").unwrap();
    writeln!(w).unwrap();
    writeln!(
        w,
        "Nothing was downloaded or committed; the maintainer supplies the corpus."
    )
    .unwrap();
    writeln!(w).unwrap();
    for c in &s.candidate {
        writeln!(
            w,
            "- **{}** <{}>. Licence: {}. Why: {}",
            md(&c.title),
            c.url,
            md(&c.licence),
            md(&c.why)
        )
        .unwrap();
    }
    out
}

#[test]
fn review_document_is_current() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("concept-sciences-proposals.md");
    let fresh = render();
    if std::env::var_os("KOVAN_REGENERATE_SCIENCES").is_some() {
        std::fs::write(&path, &fresh).expect("write docs/concept-sciences-proposals.md");
        return;
    }
    let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        on_disk == fresh,
        "docs/concept-sciences-proposals.md is stale; regenerate with KOVAN_REGENERATE_SCIENCES=1 cargo test --release -p kovan-literature --test concept_sciences"
    );
}
