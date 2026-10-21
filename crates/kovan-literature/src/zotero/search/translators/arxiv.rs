// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "arXiv.org.js" (translatorID
//   ecddda2e-4fc6-4aea-9f17-ef3b56d7377a, lastUpdated 2026-05-19 15:28:10):
//   the search half only: `arXivCategories` :38-234, `version` :236,
//   `detectSearch` :240-242, `doSearch` :244-248, `requestAtom` :351-354,
//   `parseAtom` :356-359, `parseSingleEntry` :361-465; the sandbox helpers
//   `text`/`attr` (translate, commit dd524aea9a55, translate.js
//   :1902-1934). The web half (detectWeb, doWeb, search-result scraping)
//   is not ported: kovan runs only identifier lookup.
// Copyright (c) 2019 Sean Takats and Michael Berkowitz.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The arXiv.org search translator: the arXiv API's Atom feed for one ID,
//! each entry a preprint; an entry with a DOI is completed from DOI
//! Content Negotiation (a child search).

use crate::zotero::framework::item::{
    JsObject, TranslatorCreator, TranslatorItem, TranslatorNote, TranslatorTag,
};
use crate::zotero::framework::js;
use crate::zotero::framework::options::TranslatorMetadata;
use crate::zotero::framework::utilities::{clean_author, trim_internal};
use crate::zotero::framework::xml::{NodeId, XmlDocument};
use crate::zotero::search::http::{encode_uri_component, RequestOptions};
use crate::zotero::search::{SearchContext, SearchError, SearchTranslator};
use serde_json::Value;

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "ecddda2e-4fc6-4aea-9f17-ef3b56d7377a",
    label: "arXiv.org",
    creator: "Sean Takats and Michael Berkowitz",
    target: "^https?://([^\\.]+\\.)?(arxiv\\.org|xxx\\.lanl\\.gov)/(search|find|catchup|list/\\w|abs/|pdf/)",
    min_version: "6.0",
    priority: 100,
    translator_type: 12,
    config_options: &[],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2026-05-19 15:28:10",
};

/// `arXivCategories` (:38-234).
const CATEGORIES: &[(&str, &str)] = &[
    ("cs", "Computer Science"),
    ("econ", "Economics"),
    ("eess", "Electrical Engineering and Systems Science"),
    ("math", "Mathematics"),
    ("nlin", "Nonlinear Sciences"),
    ("physics", "Physics"),
    ("q-fin", "Quantitative Finance"),
    ("stat", "Statistics"),
    ("acc-phys", "Accelerator Physics"),
    ("adap-org", "Adaptation, Noise, and Self-Organizing Systems"),
    ("alg-geom", "Algebraic Geometry"),
    ("ao-sci", "Atmospheric-Oceanic Sciences"),
    ("astro-ph", "Astrophysics"),
    ("astro-ph.CO", "Cosmology and Nongalactic Astrophysics"),
    ("astro-ph.EP", "Earth and Planetary Astrophysics"),
    ("astro-ph.GA", "Astrophysics of Galaxies"),
    ("astro-ph.HE", "High Energy Astrophysical Phenomena"),
    (
        "astro-ph.IM",
        "Instrumentation and Methods for Astrophysics",
    ),
    ("astro-ph.SR", "Solar and Stellar Astrophysics"),
    ("atom-ph", "Atomic, Molecular and Optical Physics"),
    ("bayes-an", "Bayesian Analysis"),
    ("chao-dyn", "Chaotic Dynamics"),
    ("chem-ph", "Chemical Physics"),
    ("cmp-lg", "Computation and Language"),
    ("comp-gas", "Cellular Automata and Lattice Gases"),
    ("cond-mat", "Condensed Matter"),
    ("cond-mat.dis-nn", "Disordered Systems and Neural Networks"),
    ("cond-mat.mes-hall", "Mesoscale and Nanoscale Physics"),
    ("cond-mat.mtrl-sci", "Materials Science"),
    ("cond-mat.other", "Other Condensed Matter"),
    ("cond-mat.quant-gas", "Quantum Gases"),
    ("cond-mat.soft", "Soft Condensed Matter"),
    ("cond-mat.stat-mech", "Statistical Mechanics"),
    ("cond-mat.str-el", "Strongly Correlated Electrons"),
    ("cond-mat.supr-con", "Superconductivity"),
    ("cs.AI", "Artificial Intelligence"),
    ("cs.AR", "Hardware Architecture"),
    ("cs.CC", "Computational Complexity"),
    ("cs.CE", "Computational Engineering, Finance, and Science"),
    ("cs.CG", "Computational Geometry"),
    ("cs.CL", "Computation and Language"),
    ("cs.CR", "Cryptography and Security"),
    ("cs.CV", "Computer Vision and Pattern Recognition"),
    ("cs.CY", "Computers and Society"),
    ("cs.DB", "Databases"),
    ("cs.DC", "Distributed, Parallel, and Cluster Computing"),
    ("cs.DL", "Digital Libraries"),
    ("cs.DM", "Discrete Mathematics"),
    ("cs.DS", "Data Structures and Algorithms"),
    ("cs.ET", "Emerging Technologies"),
    ("cs.FL", "Formal Languages and Automata Theory"),
    ("cs.GL", "General Literature"),
    ("cs.GR", "Graphics"),
    ("cs.GT", "Computer Science and Game Theory"),
    ("cs.HC", "Human-Computer Interaction"),
    ("cs.IR", "Information Retrieval"),
    ("cs.IT", "Information Theory"),
    ("cs.LG", "Machine Learning"),
    ("cs.LO", "Logic in Computer Science"),
    ("cs.MA", "Multiagent Systems"),
    ("cs.MM", "Multimedia"),
    ("cs.MS", "Mathematical Software"),
    ("cs.NA", "Numerical Analysis"),
    ("cs.NE", "Neural and Evolutionary Computing"),
    ("cs.NI", "Networking and Internet Architecture"),
    ("cs.OH", "Other Computer Science"),
    ("cs.OS", "Operating Systems"),
    ("cs.PF", "Performance"),
    ("cs.PL", "Programming Languages"),
    ("cs.RO", "Robotics"),
    ("cs.SC", "Symbolic Computation"),
    ("cs.SD", "Sound"),
    ("cs.SE", "Software Engineering"),
    ("cs.SI", "Social and Information Networks"),
    ("cs.SY", "Systems and Control"),
    ("dg-ga", "Differential Geometry"),
    ("econ.EM", "Econometrics"),
    ("econ.GN", "General Economics"),
    ("econ.TH", "Theoretical Economics"),
    ("eess.AS", "Audio and Speech Processing"),
    ("eess.IV", "Image and Video Processing"),
    ("eess.SP", "Signal Processing"),
    ("eess.SY", "Systems and Control"),
    ("funct-an", "Functional Analysis"),
    ("gr-qc", "General Relativity and Quantum Cosmology"),
    ("hep-ex", "High Energy Physics - Experiment"),
    ("hep-lat", "High Energy Physics - Lattice"),
    ("hep-ph", "High Energy Physics - Phenomenology"),
    ("hep-th", "High Energy Physics - Theory"),
    ("math-ph", "Mathematical Physics"),
    ("math.AC", "Commutative Algebra"),
    ("math.AG", "Algebraic Geometry"),
    ("math.AP", "Analysis of PDEs"),
    ("math.AT", "Algebraic Topology"),
    ("math.CA", "Classical Analysis and ODEs"),
    ("math.CO", "Combinatorics"),
    ("math.CT", "Category Theory"),
    ("math.CV", "Complex Variables"),
    ("math.DG", "Differential Geometry"),
    ("math.DS", "Dynamical Systems"),
    ("math.FA", "Functional Analysis"),
    ("math.GM", "General Mathematics"),
    ("math.GN", "General Topology"),
    ("math.GR", "Group Theory"),
    ("math.GT", "Geometric Topology"),
    ("math.HO", "History and Overview"),
    ("math.IT", "Information Theory"),
    ("math.KT", "K-Theory and Homology"),
    ("math.LO", "Logic"),
    ("math.MG", "Metric Geometry"),
    ("math.MP", "Mathematical Physics"),
    ("math.NA", "Numerical Analysis"),
    ("math.NT", "Number Theory"),
    ("math.OA", "Operator Algebras"),
    ("math.OC", "Optimization and Control"),
    ("math.PR", "Probability"),
    ("math.QA", "Quantum Algebra"),
    ("math.RA", "Rings and Algebras"),
    ("math.RT", "Representation Theory"),
    ("math.SG", "Symplectic Geometry"),
    ("math.SP", "Spectral Theory"),
    ("math.ST", "Statistics Theory"),
    ("mtrl-th", "Materials Theory"),
    ("nlin.AO", "Adaptation and Self-Organizing Systems"),
    ("nlin.CD", "Chaotic Dynamics"),
    ("nlin.CG", "Cellular Automata and Lattice Gases"),
    ("nlin.PS", "Pattern Formation and Solitons"),
    ("nlin.SI", "Exactly Solvable and Integrable Systems"),
    ("nucl-ex", "Nuclear Experiment"),
    ("nucl-th", "Nuclear Theory"),
    ("patt-sol", "Pattern Formation and Solitons"),
    ("physics.acc-ph", "Accelerator Physics"),
    ("physics.ao-ph", "Atmospheric and Oceanic Physics"),
    ("physics.app-ph", "Applied Physics"),
    ("physics.atm-clus", "Atomic and Molecular Clusters"),
    ("physics.atom-ph", "Atomic Physics"),
    ("physics.bio-ph", "Biological Physics"),
    ("physics.chem-ph", "Chemical Physics"),
    ("physics.class-ph", "Classical Physics"),
    ("physics.comp-ph", "Computational Physics"),
    (
        "physics.data-an",
        "Data Analysis, Statistics and Probability",
    ),
    ("physics.ed-ph", "Physics Education"),
    ("physics.flu-dyn", "Fluid Dynamics"),
    ("physics.gen-ph", "General Physics"),
    ("physics.geo-ph", "Geophysics"),
    ("physics.hist-ph", "History and Philosophy of Physics"),
    ("physics.ins-det", "Instrumentation and Detectors"),
    ("physics.med-ph", "Medical Physics"),
    ("physics.optics", "Optics"),
    ("physics.plasm-ph", "Plasma Physics"),
    ("physics.pop-ph", "Popular Physics"),
    ("physics.soc-ph", "Physics and Society"),
    ("physics.space-ph", "Space Physics"),
    ("plasm-ph", "Plasma Physics"),
    ("q-alg", "Quantum Algebra and Topology"),
    ("q-bio", "Quantitative Biology"),
    ("q-bio.BM", "Biomolecules"),
    ("q-bio.CB", "Cell Behavior"),
    ("q-bio.GN", "Genomics"),
    ("q-bio.MN", "Molecular Networks"),
    ("q-bio.NC", "Neurons and Cognition"),
    ("q-bio.OT", "Other Quantitative Biology"),
    ("q-bio.PE", "Populations and Evolution"),
    ("q-bio.QM", "Quantitative Methods"),
    ("q-bio.SC", "Subcellular Processes"),
    ("q-bio.TO", "Tissues and Organs"),
    ("q-fin.CP", "Computational Finance"),
    ("q-fin.EC", "Economics"),
    ("q-fin.GN", "General Finance"),
    ("q-fin.MF", "Mathematical Finance"),
    ("q-fin.PM", "Portfolio Management"),
    ("q-fin.PR", "Pricing of Securities"),
    ("q-fin.RM", "Risk Management"),
    ("q-fin.ST", "Statistical Finance"),
    ("q-fin.TR", "Trading and Market Microstructure"),
    ("quant-ph", "Quantum Physics"),
    ("solv-int", "Exactly Solvable and Integrable Systems"),
    ("stat.AP", "Applications"),
    ("stat.CO", "Computation"),
    ("stat.ME", "Methodology"),
    ("stat.ML", "Machine Learning"),
    ("stat.OT", "Other Statistics"),
    ("stat.TH", "Statistics Theory"),
    ("supr-con", "Superconductivity"),
    ("test", "Test"),
    ("test.dis-nn", "Test Disruptive Networks"),
    ("test.mes-hall", "Test Hall"),
    ("test.mtrl-sci", "Test Mtrl-Sci"),
    ("test.soft", "Test Soft"),
    ("test.stat-mech", "Test Mechanics"),
    ("test.str-el", "Test Electrons"),
    ("test.supr-con", "Test Superconductivity"),
    ("bad-arch.bad-cat", "Invalid Category"),
];

fn category(term: &str) -> Option<&'static str> {
    CATEGORIES.iter().find(|(k, _)| *k == term).map(|(_, v)| *v)
}

/// `detectSearch` (:240-242): `!!item.arXiv`.
pub fn detect_search(search: &JsObject) -> bool {
    search.truthy("arXiv")
}

/// `doSearch` (:244-248).
pub fn do_search(ctx: &mut SearchContext, search: &JsObject) -> Result<(), SearchError> {
    let id = search
        .get("arXiv")
        .map(js::to_js_string)
        .unwrap_or_default();
    let url = format!(
        "https://export.arxiv.org/api/query?id_list={}&max_results=1",
        encode_uri_component(&id)
    );
    let doc = request_atom(ctx, &url)?;
    parse_atom(ctx, &doc)
}

/// `requestAtom` (:351-354): `requestText`, then `DOMParser` as XML.
fn request_atom(ctx: &mut SearchContext, url: &str) -> Result<XmlDocument, SearchError> {
    let text = ctx.request_text(url, &RequestOptions::default())?;
    Ok(XmlDocument::parse_from_string(&text))
}

/// `root.querySelectorAll("parent > local")`: elements named `local`
/// below `root` whose parent element is named `parent`.
fn child_selector(doc: &XmlDocument, root: NodeId, parent: &str, local: &str) -> Vec<NodeId> {
    doc.query_selector_all(root, local)
        .into_iter()
        .filter(|&n| {
            doc.parent(n)
                .and_then(|p| doc.element(p))
                .is_some_and(|e| e.local == parent)
        })
        .collect()
}

/// The sandbox's `text(root, selector)`: the first match's trimmed
/// `textContent`, or "".
fn text(doc: &XmlDocument, root: NodeId, selector: &str) -> String {
    doc.query_selector(root, selector)
        .map(|n| js::trim(&doc.text(n)).to_owned())
        .unwrap_or_default()
}

/// The sandbox's `attr(root, selector, name)`.
fn attr(doc: &XmlDocument, root: NodeId, selector: &str, name: &str) -> String {
    doc.query_selector(root, selector)
        .and_then(|n| doc.get_attribute(n, name).map(|v| js::trim(v).to_owned()))
        .unwrap_or_default()
}

/// `parseAtom` (:356-359).
fn parse_atom(ctx: &mut SearchContext, doc: &XmlDocument) -> Result<(), SearchError> {
    for entry in child_selector(doc, doc.document(), "feed", "entry") {
        parse_single_entry(ctx, doc, entry)?;
    }
    Ok(())
}

/// `str.replace(/v\d+/, '')`: the first `v` followed by digits removed.
fn strip_first_version(s: &str) -> String {
    let b = s.as_bytes();
    for i in 0..b.len() {
        if b[i] == b'v' && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
            let mut j = i + 1;
            while b.get(j).is_some_and(u8::is_ascii_digit) {
                j += 1;
            }
            return format!("{}{}", &s[..i], &s[j..]);
        }
    }
    s.to_owned()
}

/// `parseSingleEntry` (:361-465).
fn parse_single_entry(
    ctx: &mut SearchContext,
    doc: &XmlDocument,
    entry: NodeId,
) -> Result<(), SearchError> {
    let mut item = TranslatorItem::new("preprint");
    item.set("title", trim_internal(&text(doc, entry, "title")));
    // ZU.strToISO returns false when it finds no date.
    let date = ctx
        .str_to_iso(&text(doc, entry, "updated"))
        .map_or(Value::Bool(false), Value::String);
    item.set("date", date);
    for n in child_selector(doc, entry, "author", "name") {
        let c = clean_author(&doc.text(n), "author", false);
        item.creators.push(TranslatorCreator {
            first_name: c.first_name,
            last_name: Some(c.last_name),
            creator_type: Some(c.creator_type),
            ..Default::default()
        });
    }
    item.set("abstractNote", trim_internal(&text(doc, entry, "summary")));

    for comment in doc.query_selector_all(entry, "comment") {
        let note = trim_internal(&doc.text(comment));
        item.notes
            .push(TranslatorNote::new(format!("Comment: {note}")));
    }

    for c in doc.query_selector_all(entry, "category") {
        let sub = doc.get_attribute(c, "term").unwrap_or("").to_owned();
        let main = sub.split('.').next().unwrap_or("");
        let tag = if main != sub && category(main).is_some() {
            // `arXivCategories[mainCat] + " - " + arXivCategories[sub]`.
            Some(format!(
                "{} - {}",
                category(main).unwrap_or(""),
                category(&sub).unwrap_or("undefined")
            ))
        } else {
            category(&sub).map(str::to_owned)
        };
        // `.filter(Boolean)`.
        if let Some(t) = tag.filter(|t| !t.is_empty()) {
            item.tags.push(TranslatorTag::new(t));
        }
    }

    let versioned_url = text(doc, entry, "id");
    let arxiv_url = strip_first_version(&versioned_url);
    let doi = text(doc, entry, "doi");
    if !doi.is_empty() {
        item.set("DOI", doi.clone());
    }
    item.set("url", arxiv_url.clone());

    // `arxivURL.match(/\/abs\/(.+)$/)[1]`: a TypeError when there is none.
    let article_id = arxiv_url
        .find("/abs/")
        .map(|i| &arxiv_url[i + 5..])
        .filter(|s| !s.is_empty() && !s.contains(['\n', '\r', '\u{2028}', '\u{2029}']))
        .map(str::to_owned)
        .ok_or_else(|| {
            SearchError::Translator("TypeError: arxivURL.match(...) is null".to_owned())
        })?;

    let mut article_field = attr(doc, entry, "primary_category", "term");
    if !article_field.is_empty() {
        article_field = format!("[{article_field}]");
    }
    let extra = if article_id.contains('/') {
        format!("arXiv:{article_id}")
    } else if !article_field.is_empty() {
        format!("arXiv:{article_id} {article_field}")
    } else {
        format!("arXiv:{article_id}")
    };
    item.set("extra", extra);

    let pdf_url = versioned_url.replacen("/abs/", "/pdf/", 1);
    let mut pdf = JsObject::new();
    pdf.set("title", "Preprint PDF");
    pdf.set("url", pdf_url);
    pdf.set("mimeType", "application/pdf");
    item.attachments.push(pdf);
    let mut snap = JsObject::new();
    snap.set("title", "Snapshot");
    snap.set("url", arxiv_url);
    snap.set("mimeType", "text/html");
    item.attachments.push(snap);

    if !doi.is_empty() {
        // DOI Content Negotiation as a child search (:428-453): its items
        // supplement the preprint, its errors are ignored (an empty error
        // handler), and the item completes in the `done` handler either way.
        let mut s = JsObject::new();
        s.set("itemType", "journalArticle");
        s.set("DOI", doi);
        let child = ctx.child_search(SearchTranslator::DoiContentNegotiation, &s)?;
        let get = |i: &TranslatorItem, k: &str| i.get(k).cloned().unwrap_or(Value::Null);
        for found in &child.items {
            item.item_type = found.item_type.clone();
            item.set("volume", get(found, "volume"));
            item.set("issue", get(found, "issue"));
            item.set("pages", get(found, "pages"));
            item.set("date", get(found, "date"));
            item.set("ISSN", get(found, "ISSN"));
            if found.truthy("publicationTitle") {
                item.set("publicationTitle", get(found, "publicationTitle"));
                item.set("journalAbbreviation", get(found, "journalAbbreviation"));
            }
            item.set("date", get(found, "date"));
        }
        ctx.complete(item)
    } else {
        item.set("publisher", "arXiv");
        item.set("number", format!("arXiv:{article_id}"));
        // `version` is set only by doWeb; in a search it is undefined.
        item.set("DOI", format!("10.48550/arXiv.{article_id}"));
        item.set("archiveID", format!("arXiv:{article_id}"));
        ctx.complete(item)
    }
}
