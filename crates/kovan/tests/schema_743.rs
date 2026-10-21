//! The GitHub issue #743 schema additions, and backwards compatibility with
//! notes written before them.
//!
//! # Methodology
//!
//! Three synthetic fixtures under `tests/fixtures/schema_743/` (nothing is
//! copied from a real Kovan folder):
//!
//! - `legacy_paper.md`: a paper header with its BibTeX ```latex fence and a
//!   `## Summary`, an annotation with `[source] page`/`region`, a
//!   `connections` back-pointer and a `##` sub-heading, a digitised graph
//!   with a `### start of data series` CSV body, and a reviewed
//!   `pages = [..]` source reference;
//! - `legacy_mindmap.md`: the mindmap header and one `[relation]` artifact;
//! - `new_kinds.md`: the four new kinds, `origin`, `[[relation]]` anchors
//!   with `code:` targets and `page`/`quote`/`commit`, a `###` code block and
//!   a `## Review:` sign-off line.
//!
//! Each is parsed with [`kovan::artifact::parse_document`]. The pass
//! criteria: no problems; the fields the old schema had read back as
//! written; an absent `origin` reads as human and is not written back; and
//! every artifact's block re-renders with
//! [`kovan::artifact::render_artifact_block`] to **the same bytes** as the
//! file. The examples on the published schema page
//! (`docs/site/kovan-schema/index.html`) are parsed by the same rules, so
//! the page cannot show an example the parser rejects.
//!
//! # Results
//!
//! 2026-10-06: all pass. The read-only check of the maintainer's own folder
//! (`tests/local_library_compat.rs`) gave 39 files, 234 artifacts,
//! 0 problems and 234/234 identical re-renders, with the same digest
//! (`43960595adad4454`), before and after this change.

use kovan::artifact::{
    block_span, parse_document, parse_series_blocks, render_artifact_block, ArtifactKind, Origin,
    ParsedDocument,
};
use kovan::relation::{CodeTarget, RelationKind, Relations};

const LEGACY_PAPER: &str = include_str!("fixtures/schema_743/legacy_paper.md");
const LEGACY_MINDMAP: &str = include_str!("fixtures/schema_743/legacy_mindmap.md");
const NEW_KINDS: &str = include_str!("fixtures/schema_743/new_kinds.md");

/// Parse `md`, require no problems, and require every artifact block to
/// re-render byte for byte.
fn parse_and_round_trip(name: &str, md: &str) -> ParsedDocument {
    let parsed = parse_document(md);
    assert!(parsed.problems.is_empty(), "{name}: {:?}", parsed.problems);
    assert!(!parsed.artifacts.is_empty(), "{name}: no artifacts");
    let lines: Vec<&str> = md.lines().collect();
    for a in &parsed.artifacts {
        let span = block_span(md, a);
        let original = lines[span].join("\n");
        let rendered = render_artifact_block(a.level, &a.heading, &a.toml, &a.body).unwrap();
        assert_eq!(
            rendered.trim_end(),
            original.trim_end(),
            "{name}: artifact {:?} does not round-trip",
            a.id()
        );
    }
    parsed
}

#[test]
fn a_legacy_paper_parses_as_before_and_round_trips() {
    let doc = parse_and_round_trip("legacy_paper", LEGACY_PAPER);
    let kinds: Vec<_> = doc.artifacts.iter().map(|a| a.kind()).collect();
    assert_eq!(
        kinds,
        [
            ArtifactKind::Paper,
            ArtifactKind::Annotation,
            ArtifactKind::DigitisedGraph,
            ArtifactKind::SourceReference
        ]
    );
    for a in &doc.artifacts {
        assert_eq!(a.toml.kovan.origin, None, "no origin key was written");
        assert_eq!(a.origin(), Origin::Human, "absent origin reads as human");
        assert!(a.relations().is_empty());
    }

    let paper = &doc.artifacts[0];
    assert!(paper
        .body
        .starts_with("```latex\n@article{synthetic2020example,"));
    assert!(paper.body.contains("## Summary"));

    let note = doc.get("graphite-temperature-assumption").unwrap();
    let source = note.toml.source.as_ref().unwrap();
    assert_eq!(source.page, Some(87));
    assert_eq!(
        <[f64; 4]>::from(source.region.unwrap()),
        [0.214, 0.341, 0.721, 0.508]
    );
    assert_eq!(note.toml.connections, ["a1b2c3d4e5f6"]);
    assert!(note.body.contains("## A sub-heading inside the annotation"));

    let graph = doc.get("fig-3-specific-heat").unwrap();
    let series = parse_series_blocks(&graph.body);
    assert_eq!(series.len(), 2);
    assert_eq!(series[0].name, "sample A");
    assert_eq!(series[1].csv, "T (K),cp (J/kg/K)\n300,700\n600,1160");
    assert_eq!(
        graph.toml.extraction.as_ref().unwrap().figure.as_deref(),
        Some("Fig. 3")
    );

    let reference = doc.get("pages-42-48").unwrap();
    assert!(reference.is_reviewed());
    assert_eq!(
        reference.toml.source.as_ref().unwrap().pages,
        Some([42, 48])
    );
}

#[test]
fn a_legacy_relation_artifact_keeps_its_single_table_shape() {
    let doc = parse_and_round_trip("legacy_mindmap", LEGACY_MINDMAP);
    let rel = doc.get("a1b2c3d4e5f6").unwrap();
    assert_eq!(rel.kind(), ArtifactKind::Relation);
    let Some(Relations::One(record)) = &rel.toml.relation else {
        panic!("a [relation] table must read as Relations::One");
    };
    assert_eq!(
        record.source,
        "artifact:synthetic2020example#graphite-temperature-assumption"
    );
    assert_eq!(record.target, "collection:htgrs/materials");
    assert_eq!(record.kind, RelationKind::Supports);
    assert_eq!(
        (record.page, &record.quote, &record.commit),
        (None, &None, &None)
    );
}

#[test]
fn the_new_kinds_origin_and_anchors_parse_and_round_trip() {
    let doc = parse_and_round_trip("new_kinds", NEW_KINDS);
    let kinds: Vec<_> = doc.artifacts.iter().map(|a| a.kind()).collect();
    assert_eq!(
        kinds,
        [
            ArtifactKind::LessonSection,
            ArtifactKind::WalkStep,
            ArtifactKind::CodeWalk,
            ArtifactKind::RecipeStep
        ]
    );
    let origins: Vec<_> = doc.artifacts.iter().map(|a| a.toml.kovan.origin).collect();
    assert_eq!(
        origins,
        [
            Some(Origin::Ai),
            Some(Origin::Ai),
            Some(Origin::Human),
            None
        ]
    );

    let step = doc.get("step-1-fence-hides-heading").unwrap();
    let anchors = step.relations();
    assert_eq!(anchors.len(), 2);
    assert!(matches!(step.toml.relation, Some(Relations::Many(_))));
    assert!(
        anchors.iter().all(|r| r.source.is_empty()),
        "source is implicit"
    );

    let code = anchors[0].code_target().expect("a code: target");
    assert_eq!(code.file, "crates/kovan/src/artifact.rs");
    assert_eq!(code.item, "heading_span");
    assert!(code.line.is_some());
    assert_eq!(anchors[0].commit.as_deref(), Some("2f2cf7d599"));
    assert_eq!(anchors[0].kind, RelationKind::Implements);

    assert_eq!(anchors[1].target, "paper:synthetic2020example");
    assert_eq!(anchors[1].page, Some(87));
    assert_eq!(
        anchors[1].quote.as_deref(),
        Some("nominal operating conditions")
    );
    assert!(anchors[1].code_target().is_none());

    // `###` data (an embedded code block) and the `## Review:` line stay in
    // the body; neither starts an artifact.
    assert!(step.body.contains("### Code\n\n```rust\n"));
    assert!(step
        .body
        .trim_end()
        .ends_with("## Review: ⚠ AI draft, not reviewed"));
}

#[test]
fn every_kind_has_its_wire_name() {
    for kind in ArtifactKind::ALL {
        let toml = format!(
            "[kovan]\nid = \"x\"\nkind = \"{}\"\ncreated = \"c\"\nmodified = \"m\"\n",
            kind.as_str()
        );
        let md = format!("# x\n\n```toml\n{toml}```\n");
        let doc = parse_document(&md);
        assert!(
            doc.problems.is_empty(),
            "{}: {:?}",
            kind.as_str(),
            doc.problems
        );
        assert_eq!(doc.artifacts[0].kind(), kind);
    }
}

#[test]
fn an_unknown_origin_or_kind_is_a_problem_not_a_crash() {
    let md = "# x\n\n```toml\n[kovan]\nid = \"x\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\norigin = \"robot\"\n```\n\n# y\n\n```toml\n[kovan]\nid = \"y\"\nkind = \"walk_stepp\"\ncreated = \"c\"\nmodified = \"m\"\n```\n\n# z\n\n```toml\n[kovan]\nid = \"z\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n```\n";
    let doc = parse_document(md);
    assert_eq!(doc.problems.len(), 2);
    assert_eq!(doc.artifacts.len(), 1, "the good block still loads");
}

#[test]
fn code_targets_parse_and_print() {
    for s in [
        "code:crates/kovan/src/artifact.rs::parse_document",
        "code:crates/kovan/src/artifact.rs::Artifact::csv_block",
        "code:crates/boon-lay/src/lib.rs::Particle::release@L120",
    ] {
        let t = CodeTarget::parse(s).unwrap_or_else(|| panic!("{s}"));
        assert_eq!(t.to_string(), s);
    }
    assert_eq!(CodeTarget::parse("code:a.rs::f@L7").unwrap().line, Some(7));
    for bad in [
        "paper:x",
        "code:",
        "code:a.rs",
        "code:::f",
        "code:a.rs::",
        "code:a.rs::f@L0",
        "code:a.rs::f@Lx",
        "code:a b.rs::f",
    ] {
        assert_eq!(CodeTarget::parse(bad), None, "{bad}");
    }
}

/// A `code:` target is not a graph node: both node-id readers decline it,
/// which is what keeps the mindmap, the index's via-artifact pass and the
/// connection menu from treating it as one (they skip an end neither reads).
#[test]
fn a_code_target_is_not_a_graph_node() {
    let t = "code:crates/kovan/src/artifact.rs::parse_document@L640";
    assert!(kovan::node_id::NodeId::parse(t).is_err());
    assert!(kovan::node_id::NodeId::from_graph_id(t).is_none());
}

/// Every example on the published schema page parses with no problems, and
/// the ones marked `data-kovan-roundtrip` re-render byte for byte.
#[test]
fn the_schema_page_examples_parse() {
    let page = include_str!("../../../docs/site/kovan-schema/index.html");
    let mut seen = 0;
    let mut rest = page;
    while let Some(at) = rest.find("data-kovan-example=\"") {
        let tag_end = at + rest[at..].find('>').unwrap();
        let tag = &rest[at..tag_end];
        let name = tag["data-kovan-example=\"".len()..]
            .split('"')
            .next()
            .unwrap();
        let body_start = tag_end + 1;
        let body_start = body_start
            + rest[body_start..]
                .strip_prefix("<code>")
                .map_or(0, |_| "<code>".len());
        let body_end = body_start + rest[body_start..].find("</code>").unwrap();
        let md = unescape(&rest[body_start..body_end]);
        let doc = if tag.contains("data-kovan-roundtrip") {
            parse_and_round_trip(name, &md)
        } else {
            let doc = parse_document(&md);
            assert!(doc.problems.is_empty(), "{name}: {:?}", doc.problems);
            doc
        };
        assert!(!doc.artifacts.is_empty(), "{name}: no artifact");
        seen += 1;
        rest = &rest[body_end..];
    }
    assert!(seen >= 5, "only {seen} examples found on the schema page");
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}
