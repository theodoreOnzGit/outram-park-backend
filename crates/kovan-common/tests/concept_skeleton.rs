//! The concept-tree skeleton (`src/concept_skeleton.toml`, GitHub #724/#726/#727;
//! in kovan-common because Code Review must build for wasm32, and kovan does not)
//! is well formed, and every source links to a real document.
//!
//! What is checked:
//! - every node path is unique; L1 paths are exactly the 19 IAEA issues as
//!   `NN-segment`, `NN` = 01..=19 in order (IAEA NG-G-3.1 Rev. 1, §3.NN);
//! - every deeper node's parent exists; L2 segments carry no number;
//! - every source names a declared `[[document]]`, and every document is used;
//! - every `standard`-tier document's file exists in the `reactor-literature`
//!   submodule when that submodule is checked out (skipped, with a note, when
//!   it is empty). `private`-tier files live outside this repository and are
//!   not checked here.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize)]
struct Skeleton {
    document: Vec<Document>,
    node: Vec<Node>,
}

#[derive(Deserialize)]
struct Document {
    id: String,
    tier: String,
    file: String,
}

#[derive(Deserialize)]
struct Node {
    path: String,
    title: String,
    sources: Vec<Source>,
}

#[derive(Deserialize)]
struct Source {
    document: String,
}

fn skeleton() -> Skeleton {
    toml::from_str(include_str!("../src/concept_skeleton.toml")).expect("concept_skeleton.toml parses")
}

#[test]
fn level_one_is_the_nineteen_iaea_issues_in_order() {
    let s = skeleton();
    let l1: Vec<&str> = s.node.iter().map(|n| n.path.as_str()).filter(|p| !p.contains('/')).collect();
    assert_eq!(l1.len(), 19, "{l1:?}");
    for (i, p) in l1.iter().enumerate() {
        let (num, rest) = p.split_once('-').expect("NN-segment");
        assert_eq!(num, format!("{:02}", i + 1), "{p}");
        assert!(!rest.is_empty() && rest.chars().all(|c| c.is_ascii_lowercase() || c == '-'), "{p}");
    }
}

#[test]
fn paths_are_unique_parents_exist_and_level_two_is_unnumbered() {
    let s = skeleton();
    let mut seen = BTreeSet::new();
    for n in &s.node {
        assert!(seen.insert(n.path.as_str()), "duplicate path {}", n.path);
        assert!(!n.title.is_empty(), "{} has no title", n.path);
        assert!(!n.sources.is_empty(), "{} has no source", n.path);
    }
    for n in &s.node {
        if let Some((parent, seg)) = n.path.rsplit_once('/') {
            assert!(seen.contains(parent), "{}: parent {parent} missing", n.path);
            assert!(!seg.starts_with(|c: char| c.is_ascii_digit()), "{}: L2 segment is numbered", n.path);
            assert!(seg.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'), "{}", n.path);
        }
    }
}

#[test]
fn every_source_links_to_a_declared_document_and_every_document_is_used() {
    let s = skeleton();
    let docs: BTreeMap<&str, &Document> = s.document.iter().map(|d| (d.id.as_str(), d)).collect();
    assert_eq!(docs.len(), s.document.len(), "duplicate document id");
    let mut used = BTreeSet::new();
    for n in &s.node {
        for src in &n.sources {
            assert!(docs.contains_key(src.document.as_str()), "{}: unknown document {}", n.path, src.document);
            used.insert(src.document.as_str());
        }
    }
    for d in &s.document {
        assert!(matches!(d.tier.as_str(), "standard" | "private"), "{}: tier {}", d.id, d.tier);
        assert!(used.contains(d.id.as_str()), "document {} is never cited", d.id);
    }
}

#[test]
fn standard_tier_files_exist_in_the_corpus_checkout() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../kovan-literature/reactor-literature");
    if !corpus.join("kovan-standard-open-corpus").is_dir() {
        eprintln!("skipped: reactor-literature submodule not checked out at {}", corpus.display());
        return;
    }
    for d in skeleton().document.iter().filter(|d| d.tier == "standard") {
        assert!(corpus.join(&d.file).is_file(), "{}: {} not in the corpus", d.id, d.file);
    }
}
