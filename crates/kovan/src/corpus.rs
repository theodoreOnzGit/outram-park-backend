//! The built-in nuclear-engineering corpus (GitHub issue #248, epic #247).
//!
//! What belongs here: the curated knowledge Kovan ships with, compiled into
//! the binary so the mind map always has a nuclear-engineering backbone, with
//! no Kovan folder open, offline, and with no PDFs (maintainer brief,
//! 2026-09-22). That is:
//!
//! - ~~[`TOPICS`]: the topic hierarchy, as data (44 hand-written
//!   `nuclear-engineering/...` rows).~~ **REPLACED 2026-10-06** (maintainer,
//!   GitHub #724/#727: "Levels 1–3 should be in the standard kovan mindmap
//!   (replacing the existing one)"): [`topics`], the concept tree's levels
//!   1–3 read from `kovan_literature::concept_tree` (the 19 IAEA Milestones
//!   issues, the regulatory review categories under them, and the approved
//!   and deferred concepts), under one virtual root, [`ROOT_TOPIC`]. Paths
//!   are the concept paths themselves (`02-nuclear-safety/nuclear-design/…`),
//!   so a `kovan.toml` classification naming one resolves directly. The old
//!   44 paths are no longer corpus topics; a user library that has folders
//!   under `topics/nuclear-engineering/` keeps them as its own (library
//!   namespace) topics, exactly as any other user topic.
//! - [`LITERATURE`]: curated literature identities and metadata, supplied by
//!   the maintainer (#250, 2026-09-22) and read from the documents
//!   themselves; nothing bibliographic is invented here. The PDFs are in
//!   [`CORPUS_REPOSITORY_URL`]. Since 2026-10-06 it also holds every
//!   `[[document]]` the concept tree cites ([`CorpusLiterature::concept_document`]):
//!   the standard-tier ones with their corpus file, the two private-tier
//!   IAEA ones as citation-only entries, so Kovan never needs the private
//!   repository.
//! - [`CONNECTIONS`]: curated relationships between corpus nodes, carrying
//!   [`ConnectionOrigin::KovanCorpus`] so the GUI can refuse to edit them;
//!   [`curated_connections`] adds the concept tree's cross-links to them.
//!
//! What does not belong here: PDFs (never shipped in the crate; they live in
//! ~~a library's `literature/kovan-open-corpus/`~~ **CORRECTED 2026-09-22**:
//! [`STANDARD_CORPUS_FOLDER`] of [`CORPUS_REPOSITORY_URL`], mounted in every
//! Kovan folder at `literature/standard-corpus/`), user annotations, and user
//! connections (the user's own `mindmap/connections.toml`, #252).
//!
//! # Topics are a browsing hierarchy, not an ontology
//!
//! `kovan_semantics::ontology` already compiles a curated core of concepts
//! into Rust ([`Reactor`], [`Neutronics`], [`ThermalHydraulics`]), and its
//! edges assert meaning: `SpecializationOf` says one thing *is a* kind of
//! another. The topic tree here says only where to find things: Critical
//! Heat Flux sits under Thermal Hydraulics without being a kind of it. So the
//! tree is its own structure, and a topic that *is* an ontology concept links
//! to it through [`OntologyLink`], a typed value the compiler checks, so the
//! two cannot drift and search can use the ontology's aliases.
//!
//! The same holds for the concept tree that replaced the old topics: its
//! nesting is "where a regulator reviews this", not "is a kind of". Only
//! the three tree nodes that plainly *are* an ontology concept carry a link
//! ([`ONTOLOGY_LINKS`]).

use crate::node_id::{Namespace, NodeId};
use crate::relation::RelationKind;
use kovan_literature::{concept_tree, ConceptNode, ConceptStatus};
use kovan_semantics::{CoreConcept, Neutronics, Reactor, ThermalHydraulics};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// A corpus topic: one node of the browsing hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorpusTopic {
    /// Slash-separated path, parent first; the last segment is this topic's
    /// slug. Since 2026-10-06 this is the concept path itself, with no root
    /// prefix (`02-nuclear-safety/nuclear-design`), or [`ROOT_TOPIC`] for the
    /// virtual root.
    pub path: &'static str,
    /// Display name.
    pub title: &'static str,
    /// The ontology concept this topic is, if it is one.
    pub ontology: Option<OntologyLink>,
    /// The concept-tree node behind this topic: its level, sources,
    /// cross-links, origin and status. `None` only for [`ROOT_TOPIC`].
    pub concept: Option<&'static ConceptNode>,
}

impl CorpusTopic {
    /// This topic's node id.
    pub fn id(&self) -> NodeId {
        NodeId::concept(Namespace::Corpus, self.path)
    }

    /// The parent topic's path, or `None` for the root. A level-1 issue's
    /// parent is [`ROOT_TOPIC`], although its path carries no prefix.
    pub fn parent_path(&self) -> Option<&'static str> {
        if self.path == ROOT_TOPIC {
            return None;
        }
        Some(
            self.path
                .rsplit_once('/')
                .map(|(p, _)| p)
                .unwrap_or(ROOT_TOPIC),
        )
    }

    /// The concept-tree level: 0 for the virtual root, 1 for an IAEA issue,
    /// 2 for a review category, 3 and deeper for a concept.
    pub fn level(&self) -> usize {
        self.concept.map(|c| c.level).unwrap_or(0)
    }

    /// Whether the concept is `deferred` (placed, awaiting sources): a map
    /// should grey it.
    pub fn deferred(&self) -> bool {
        self.concept
            .is_some_and(|c| c.status == ConceptStatus::Deferred)
    }

    /// Other names search should match: the linked ontology concept's name
    /// and aliases.
    pub fn aliases(&self) -> Vec<&'static str> {
        match self.ontology {
            Some(link) => {
                let mut v = vec![link.name()];
                v.extend_from_slice(link.aliases());
                v
            }
            None => Vec::new(),
        }
    }
}

/// A link from a corpus topic to a `kovan_semantics::ontology` core concept.
/// A typed value rather than an id string, so a link to a concept that does
/// not exist does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OntologyLink {
    Reactor(Reactor),
    Neutronics(Neutronics),
    ThermalHydraulics(ThermalHydraulics),
}

impl OntologyLink {
    /// The ontology concept's id.
    pub fn id(self) -> &'static str {
        match self {
            Self::Reactor(c) => c.id(),
            Self::Neutronics(c) => c.id(),
            Self::ThermalHydraulics(c) => c.id(),
        }
    }

    /// The ontology concept's full name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Reactor(c) => c.name(),
            Self::Neutronics(c) => c.name(),
            Self::ThermalHydraulics(c) => c.name(),
        }
    }

    /// The ontology concept's aliases.
    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Reactor(c) => c.aliases(),
            Self::Neutronics(c) => c.aliases(),
            Self::ThermalHydraulics(c) => c.aliases(),
        }
    }
}

/// What kind of source a literature entry is. A PDF is optional for every
/// kind; a physical book or a web page has a research record all the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteratureKind {
    Paper,
    Report,
    Book,
    Webpage,
    Standard,
    Thesis,
    /// Anything the other kinds do not describe.
    Other,
}

/// Whether a source may be redistributed, decided from the document's own
/// licence or copyright page (workspace `DATA_POLICY.md`). **Never inferred**
/// from NRC or OSTI hosting, a `.gov` address, a NUREG/DOE report number or
/// a free download: those are
/// [`SourceStatus::PubliclyAccessibleUnverified`] until checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStatus {
    /// Checked: public domain (for example, authored by a U.S. government
    /// employee as part of their duties, per the document).
    VerifiedPublicDomain,
    /// Checked: under an open licence permitting redistribution (CC0,
    /// CC BY, CC BY-SA, ...), or under a publisher's written redistribution
    /// grant. The grant may be narrower than an open licence: the EPA
    /// Federal Guidance Reports (2026-09-28) are redistributable for
    /// non-commercial, scientific and educational purposes only, and their
    /// [`CorpusLiterature::status_basis`] says so.
    VerifiedOpenLicence,
    /// Freely readable, but redistribution has not been verified. The entry
    /// and its URL ship; the PDF does not.
    PubliclyAccessibleUnverified,
    /// Restricted or local-only.
    Restricted,
}

impl SourceStatus {
    /// Whether the PDF may be placed in the standard corpus
    /// ([`STANDARD_CORPUS_FOLDER`]; ~~`literature/kovan-open-corpus/`~~,
    /// corrected 2026-09-22).
    pub fn redistributable(self) -> bool {
        matches!(self, Self::VerifiedPublicDomain | Self::VerifiedOpenLicence)
    }
}

/// A curated literature entry. Every field is metadata; the PDF, if any, is
/// found separately (#253), and its absence is normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorpusLiterature {
    /// Corpus-unique id, e.g. a report number in lower case. Used as the
    /// literature node's path.
    pub id: &'static str,
    pub kind: LiteratureKind,
    pub title: &'static str,
    /// Authors or issuing organisation, in citation order.
    pub authors: &'static [&'static str],
    pub year: Option<u16>,
    /// Concept paths ([`topics`]) this entry is filed under by hand, from
    /// its abstract and contents. It appears as a citation of each. (Until
    /// 2026-10-06 these were paths of the old `TOPICS` table, all under
    /// `nuclear-engineering/`; every one was re-filed under the concept
    /// tree that day.) [`Self::filed_under`] adds the nodes that cite the
    /// entry as a source.
    pub topics: &'static [&'static str],
    /// The concept-tree `[[document]]` id this entry is, when the tree cites
    /// it (`concept_skeleton.toml`). Every node whose sources name that
    /// document files the entry too, so a node's sources are among its
    /// citations on the map.
    pub concept_document: Option<&'static str>,
    /// Where the source can be read or obtained, if known: a DOI link, or
    /// the publisher's own copy.
    pub source_url: Option<&'static str>,
    /// The PDF's path inside [`CORPUS_REPOSITORY_URL`], for a
    /// [`SourceStatus::redistributable`] entry whose PDF is held there.
    pub corpus_file: Option<&'static str>,
    pub status: SourceStatus,
    /// Why [`Self::status`] is what it is: the statement it rests on and
    /// where that statement is. Required for every entry (workspace
    /// `DATA_POLICY.md`: provenance for anything the code depends on).
    pub status_basis: &'static str,
}

impl CorpusLiterature {
    /// This entry's node id.
    pub fn id(&self) -> NodeId {
        NodeId::literature(Namespace::Corpus, self.id)
    }

    /// Every concept path this entry is filed under: [`Self::topics`], then
    /// each node that cites [`Self::concept_document`], in tree order, each
    /// once.
    pub fn filed_under(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = self.topics.to_vec();
        if let Some(doc) = self.concept_document {
            for n in concept_tree().nodes_citing(doc) {
                if !out.contains(&n.path.as_str()) {
                    out.push(n.path.as_str());
                }
            }
        }
        out
    }

    /// Where a user's notes paper for this entry is classified
    /// (`standard_corpus::ensure_paper`): the hand-filed [`Self::topics`],
    /// or, for an entry with none (a tree document cited by dozens of
    /// nodes, such as the SRP table of contents), the distinct level-1 issues
    /// of the nodes that cite it. Kept short on purpose: every path here
    /// becomes a folder in the user's library.
    pub fn classification_topics(&self) -> Vec<&'static str> {
        if !self.topics.is_empty() {
            return self.topics.to_vec();
        }
        let mut out: Vec<&'static str> = Vec::new();
        for p in self.filed_under() {
            let l1 = p.split('/').next().unwrap_or(p);
            if !out.contains(&l1) {
                out.push(l1);
            }
        }
        out
    }
}

/// Where a connection comes from. Corpus connections are defaults shipped
/// with Kovan and are not edited or deleted by the user; user connections
/// are the user's own (#252). Mirrors the concept-level split in
/// `kovan_semantics::ontology::Origin` (`Core` versus `User`/`Literature`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionOrigin {
    KovanCorpus,
    User,
}

impl ConnectionOrigin {
    /// Whether the user may edit or delete a connection of this origin.
    pub fn user_editable(self) -> bool {
        self == Self::User
    }
}

/// A curated relationship between two corpus nodes. Topic-to-literature
/// filing is not listed here: it is [`CorpusLiterature::topics`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorpusConnection {
    /// Node id strings, parsed and checked by the tests.
    pub source: &'static str,
    pub target: &'static str,
    pub relation: RelationKind,
    /// Where the source document states the relationship (a reference-list
    /// entry, a page). Required: a corpus connection is added only on a
    /// document's own evidence.
    pub basis: &'static str,
}

/// The Git repository holding the corpus PDFs (maintainer direction,
/// 2026-09-22), under `kovan-standard-open-corpus/`, with a README recording each
/// document's licence basis. Kovan mounts it in every Kovan folder as the
/// submodule `literature/standard-corpus/` (~~`literature/kovan-open-corpus/`~~,
/// corrected 2026-09-22); the map never depends on it (#253). In this
/// workspace the same repository is checked out as the Git submodule
/// `crates/kovan-literature/reactor-literature/`.
pub const CORPUS_REPOSITORY_URL: &str = "https://github.com/theodoreOnzGit/reactor-literature.git";

/// The branch of [`CORPUS_REPOSITORY_URL`] to use.
pub const CORPUS_REPOSITORY_BRANCH: &str = "main";

/// The folder of [`CORPUS_REPOSITORY_URL`] holding the hardcoded documents.
/// Kovan never writes into it (it is read-only to everyone but the corpus
/// maintainer, who edits it with Git directly).
pub const STANDARD_CORPUS_FOLDER: &str = "kovan-standard-open-corpus";

/// The virtual root's path: the single centre of the standard map, whose
/// children are the 19 IAEA Milestones issues in IAEA order.
///
/// It is not a concept of the tree (the tree has 19 roots) and it can never
/// collide with one: every concept path starts with a numbered level-1
/// segment (`NN-…`, enforced by `kovan-literature`'s
/// `tests/concept_skeleton.rs`), and the underscore keeps it out of reach of
/// any user topic too, since Kovan's slugs are lower-case letters, digits and
/// hyphens only (`classify::slugify`). The level-1 issues do **not** carry it
/// as a prefix: their paths are exactly the concept paths, and
/// [`CorpusTopic::parent_path`] supplies the root as their parent.
///
/// ~~`"nuclear-engineering"`~~ until 2026-10-06, when the concept tree
/// replaced the old topics.
pub const ROOT_TOPIC: &str = "iaea_milestones";

/// The virtual root's title.
pub const ROOT_TITLE: &str = "Nuclear knowledge (IAEA Milestones)";

/// The concept-tree nodes that **are** a `kovan_semantics` ontology concept,
/// so search can use the ontology's names and aliases. Only exact matches:
/// the tree's nesting is a review structure, and a node that merely belongs
/// to a reactor type (a pebble-bed core configuration under HTGR, say) is
/// not linked. The old topics' links (Transport, Diffusion, Natural
/// Circulation, HTGR, FHR, MSR) were re-pointed here where the tree has the
/// same concept; Diffusion, HTGR and FHR have no node that is that concept,
/// so they have no link.
pub const ONTOLOGY_LINKS: &[(&str, OntologyLink)] = &[
    (
        "02-nuclear-safety/nuclear-design/neutron-transport",
        OntologyLink::Neutronics(Neutronics::Transport),
    ),
    (
        "02-nuclear-safety/thermal-hydraulic-design/natural-convection-cooling",
        OntologyLink::ThermalHydraulics(ThermalHydraulics::NaturalCirculation),
    ),
    (
        "02-nuclear-safety/nuclear-design/core-configurations/liquid-fuelled",
        OntologyLink::Reactor(Reactor::Msr),
    ),
];

/// The topics, built once: [`ROOT_TOPIC`] first, then every concept-tree
/// node (levels 1–3, parents before children), and an index by path.
struct Topics {
    all: Vec<CorpusTopic>,
    by_path: HashMap<&'static str, usize>,
    /// Paths with literature filed at or below them (ancestors included,
    /// and the root), counting every filing.
    with_literature: HashSet<&'static str>,
    /// The same, counting only hand filings ([`CorpusLiterature::topics`]),
    /// not a node's citation of its own sources.
    with_classified_literature: HashSet<&'static str>,
}

fn mark_with_ancestors(set: &mut HashSet<&'static str>, path: &'static str) {
    let mut at = path;
    set.insert(ROOT_TOPIC);
    loop {
        set.insert(at);
        match at.rsplit_once('/') {
            Some((p, _)) => at = p,
            None => break,
        }
    }
}

/// A node's title as the map shows it. A level-1 node is one of the 19 IAEA
/// Milestones issues and carries its issue number, "2. Nuclear safety", taken
/// from its `NN-` path segment (maintainer direction, 2026-10-06: "the 19
/// milestones need to have their number in the mindmap"). Deeper nodes are
/// unnumbered, like their paths. Built once, inside [`built`]'s `OnceLock`.
fn numbered_title(n: &'static ConceptNode) -> &'static str {
    match n.path.split_once('-') {
        Some((num, _)) if n.level == 1 => match num.parse::<u32>() {
            Ok(k) => format!("{k}. {}", n.title).leak(),
            Err(_) => n.title.as_str(),
        },
        _ => n.title.as_str(),
    }
}

fn built() -> &'static Topics {
    static TOPICS: OnceLock<Topics> = OnceLock::new();
    TOPICS.get_or_init(|| {
        let tree = concept_tree();
        let mut all = vec![CorpusTopic {
            path: ROOT_TOPIC,
            title: ROOT_TITLE,
            ontology: None,
            concept: None,
        }];
        all.extend(tree.nodes().iter().map(|n| {
            CorpusTopic {
                path: n.path.as_str(),
                title: numbered_title(n),
                ontology: ONTOLOGY_LINKS
                    .iter()
                    .find(|(p, _)| *p == n.path)
                    .map(|(_, l)| *l),
                concept: Some(n),
            }
        }));
        let by_path = all.iter().enumerate().map(|(i, t)| (t.path, i)).collect();
        let mut with_literature = HashSet::new();
        let mut with_classified_literature = HashSet::new();
        for l in LITERATURE {
            for p in l.filed_under() {
                mark_with_ancestors(&mut with_literature, p);
            }
            for p in l.topics {
                mark_with_ancestors(&mut with_classified_literature, p);
            }
        }
        Topics {
            all,
            by_path,
            with_literature,
            with_classified_literature,
        }
    })
}

/// Every standard-map topic: [`ROOT_TOPIC`], then levels 1–3 of the concept
/// tree, parents before children. Replaces the old `TOPICS` table
/// (2026-10-06).
pub fn topics() -> &'static [CorpusTopic] {
    &built().all
}

/// The basis for every U.S. NRC entry: the NRC Site Disclaimer
/// (<https://www.nrc.gov/about-nrc/site-disclaimer>, accessed 2026-09-22),
/// quoted verbatim in the corpus repository's README.
const NRC_BASIS: &str = "U.S. Government Work, not subject to copyright: NRC Site Disclaimer \
(https://www.nrc.gov/about-nrc/site-disclaimer); 17 U.S.C. 105";

/// The basis for the Code of Federal Regulations printouts (added
/// 2026-10-06): federal regulations are U.S. Government works; the eCFR
/// prints on every page that it is "authoritative but unofficial". Quoted
/// in the corpus repository's README, section 6.
const CFR_BASIS: &str = "U.S. federal regulation, a U.S. Government Work not subject to copyright \
(17 U.S.C. 105); eCFR printout (https://www.ecfr.gov), \"authoritative but unofficial\"";

/// The record for the NRC contractor reports (NUREG/CR) moved out of the
/// standard corpus on 2026-10-07: they carry no redistribution statement of
/// their own, and the corpus holds only what is explicitly redistributable
/// (owner's rule). Cited only, held in the maintainer's private corpus. See
/// the corpus README, section 7.
const NUREG_CR_MOVED_BASIS: &str = "NRC contractor report (written by Oak Ridge National Laboratory) with \
no redistribution statement of its own; moved from the standard corpus to the maintainer's private \
corpus on 2026-10-07 (owner: \"as long as it is available for redistribution, it is safe in \
standard corpus\"). Cited only, never redistributed; publicly available from the NRC";

/// The basis for the PHYSOR 2026 papers.
const PHYSOR_2026_BASIS: &str =
    "CC BY 4.0, as recorded on the paper's Zenodo DOI record (checked 2026-09-22)";

/// The basis for the U.S. EPA Federal Guidance Reports: the EPA disclaimers
/// page's "Copyright Status" section
/// (<https://www.epa.gov/web-policies-and-procedures/epa-disclaimers>,
/// accessed 2026-09-28), quoted verbatim in the corpus repository's README.
/// Neither report notes a copyright condition of its own. The grant is for
/// non-commercial, scientific and educational use only, so it is recorded as
/// a (non-commercial) open licence, not as public domain: both reports were
/// prepared jointly with Oak Ridge National Laboratory, a DOE contractor
/// laboratory, so 17 U.S.C. 105 does not cover them wholly.
const EPA_BASIS: &str = "EPA Copyright Status statement: \"may be freely distributed and used for \
non-commercial, scientific and educational purposes\" \
(https://www.epa.gov/web-policies-and-procedures/epa-disclaimers, accessed 2026-09-28); \
the report notes no copyright condition of its own. Commercial use is not granted";

/// The basis for the Congressional Research Service reports (added
/// 2026-10-07, corpus README section 9, ground 8): the notice printed on the
/// last page of every CRS report.
const CRS_BASIS: &str = "U.S. Government Work: \"CRS Reports, as a work of the United States \
Government, are not subject to copyright protection in the United States. Any CRS Report may be \
reproduced and distributed in its entirety without permission from CRS.\" (last page of the report, \
checked 2026-10-07; corpus README section 9). Third-party material inside keeps its holder's copyright";

/// The basis for the DOE Fundamentals Handbooks (added 2026-10-07, corpus
/// README section 4): each cover's distribution statement.
const DOE_DIST_A_BASIS: &str = "DOE handbook marked \"Distribution Statement A. Approved for public \
release; distribution is unlimited.\" (cover, PDF page 1, checked 2026-10-07; corpus README \
section 4)";

/// The same basis for the DOE Fundamentals Handbooks filed on 2026-10-10
/// (Material Science DOE-HDBK-1017, Chemistry DOE-HDBK-1015, Mathematics
/// DOE-HDBK-1014; OUTRAM PARK #829), whose covers were read that day.
const DOE_DIST_A_BASIS_2026_10_10: &str = "DOE handbook marked \"Distribution Statement A. \
Approved for public release; distribution is unlimited.\" (cover, PDF page 1, checked \
2026-10-10; corpus README section 4)";

/// The basis for the OpenStax *University Physics* volumes filed on
/// 2026-10-10 (corpus README section 10, ground 9 extended that day to
/// CC BY-NC-SA by the maintainer; OUTRAM PARK #829): each PDF's copyright
/// page, read that day.
const OPENSTAX_CC_BY_NC_SA_BASIS: &str = "CC BY-NC-SA 4.0: \"Textbook content produced by \
OpenStax is licensed under a Creative Commons Attribution Non-Commercial ShareAlike 4.0 \
International License (CC BY-NC-SA 4.0).\" (copyright page, PDF page 4, checked 2026-10-10; \
corpus README section 10, ground 9). Non-commercial, share-alike; attribution \"Access for free \
at openstax.org.\" kept on the unmodified PDF";

/// The basis for DOE laboratory reports whose cover or release form reads
/// "Approved for public release; distribution is unlimited." (added
/// 2026-10-07, corpus README section 4). Contractor-written, so not public
/// domain; the marking is the basis.
const DOE_LAB_UNLIMITED_BASIS: &str = "DOE laboratory report marked \"Approved for public release; \
distribution is unlimited.\" (PDF page 1, checked 2026-10-07; corpus README section 4). \
Contractor-written, so not claimed as public domain under 17 U.S.C. 105";

/// The basis for the two Frontiers in Energy Research articles (added
/// 2026-10-07, corpus README section 2).
const FRONTIERS_CC_BY_BASIS: &str = "CC BY 4.0: \"This is an open-access article distributed under \
the terms of the Creative Commons Attribution License (CC BY).\" (PDF page 1, COPYRIGHT box; the \
article page links CC BY 4.0; checked 2026-10-07; corpus README section 2)";

/// The basis for the OpenMC documentation pages (added 2026-10-07, corpus
/// README section 8, ground 7 as extended that day).
const OPENMC_MIT_BASIS: &str = "OpenMC's MIT licence, which grants rights to \"this software and \
associated documentation files\" including to \"distribute\", provided the copyright and permission \
notice are included (LICENSE at openmc-dev/openmc commit a5bc348, checked 2026-10-07; kept beside the \
file as openmc-docs/LICENSE; corpus README section 8)";

/// The basis for the MOOSE documentation pages (added 2026-10-07, corpus
/// README section 8, ground 7 as extended that day: the owner accepts the
/// repository's LGPL 2.1 as covering its documentation).
const MOOSE_LGPL_BASIS: &str = "MOOSE's GNU LGPL 2.1 (section 1: verbatim copies may be distributed \
with the notices and a copy of the licence), owner-accepted as covering the repository's \
documentation (#760, 2026-10-07); LICENSE and COPYRIGHT at idaholab/moose commit 21efd28 kept \
beside the file (corpus README section 8)";

/// The basis for the NSA Cybersecurity Information Sheets (added
/// 2026-10-07, corpus README section 11, ground 10).
const NSA_SHARE_BASIS: &str = "U.S. Government Work (17 U.S.C. 105), and the sheet's \"Purpose\" \
paragraph states \"This information may be shared broadly to reach all appropriate stakeholders.\" \
(last page, checked 2026-10-07; corpus README section 11, ground 10)";

/// The basis for NASA Technical Memoranda held on the NASA Technical Reports
/// Server (added 2026-10-07, corpus README section 13, ground 12).
const NTRS_BASIS: &str = "NTRS record: \"Distribution Limits: Public. Copyright: Work of the US Gov. \
Public Use Permitted.\" (checked 2026-10-07; corpus README section 13, ground 12). Some authors were \
NASA contractors; the basis is NASA's statement for the report as a whole";

/// As [`NTRS_BASIS`], for the MC/DC tutorial, which quotes RTCA material
/// with RTCA's permission (PDF page 12).
const NTRS_RTCA_BASIS: &str = "NTRS record: \"Distribution Limits: Public. Copyright: Work of the US \
Gov. Public Use Permitted.\" (checked 2026-10-07; corpus README section 13, ground 12). Two authors \
were from industry; quotations of RTCA/DO-248A (\"with permission from the RTCA\", PDF page 12) \
keep RTCA's copyright";

/// The basis for FAA advisory circulars and orders (added 2026-10-07, corpus
/// README section 14, ground 13).
const FAA_BASIS: &str = "U.S. Government Work (17 U.S.C. 105): issued by the Federal Aviation \
Administration (office named under \"Initiated by\", PDF page 1); no copyright notice (checked \
2026-10-07; corpus README section 14, ground 13). The RTCA/EUROCAE documents it cites stay \
copyrighted and are not reproduced";

/// The basis for the arXiv preprints under CC BY 4.0 (added 2026-10-07,
/// corpus README section 2, ground 2 extended to arXiv).
const ARXIV_CC_BY_BASIS: &str = "CC BY 4.0: the arXiv record links its licence to \
http://creativecommons.org/licenses/by/4.0/ (checked 2026-10-07; corpus README section 2, ground 2 \
extended to arXiv preprints, owner on #760)";

/// Curated literature (#250), supplied by the maintainer on 2026-09-22 and
/// held in [`CORPUS_REPOSITORY_URL`], plus the three EPA Federal Guidance
/// Reports (FGR-11, 13, 15) added on 2026-09-28 (maintainer request). **Only documents in that repository's
/// `kovan-standard-open-corpus/` folder are hardcoded** (maintainer direction,
/// 2026-09-22); the maintainer's own open literature (`theodore-open-corpus/`,
/// including the TUAS paper, which was listed here until then) is not. Titles, authors and years are read
/// from each document's own title and front-matter pages; topics from its
/// abstract and contents (re-filed under the concept tree on 2026-10-06).
/// Since 2026-10-06 the list also holds every document the concept tree
/// cites (see the block comment above those entries). Only the metadata is
/// compiled into Kovan; the PDFs stay in the corpus repository and open from
/// its checkout when one is present. The citations between entries are in
/// [`CONNECTIONS`] (NUREG-2201 and NUREG/KM-0006 also cite NUREG-0800, but
/// chapters 19.2 and 15.0.2, not the Section 4.2 held here, so those are not
/// connections).
pub const LITERATURE: &[CorpusLiterature] = &[
    CorpusLiterature {
        id: "wash-1400",
        kind: LiteratureKind::Report,
        title: "Reactor Safety Study: An Assessment of Accident Risks in U.S. Commercial Nuclear Power \
                Plants (WASH-1400, NUREG-75/014), Executive Summary and Main Report",
        authors: &["U.S. Nuclear Regulatory Commission"],
        year: Some(1975),
        topics: &[
            "02-nuclear-safety/severe-accidents/probabilistic-risk-assessment",
            "02-nuclear-safety/severe-accidents/severe-accident-evaluation",
            "02-nuclear-safety/source-terms/accident-source-terms",
        ],
        concept_document: Some("wash-1400"),
        source_url: Some("https://www.nrc.gov/docs/ML1533/ML15334A199.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML15334A199.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "nureg-0800-4.2",
        kind: LiteratureKind::Report,
        title: "Standard Review Plan, Section 4.2: Fuel System Design (NUREG-0800, Revision 3)",
        authors: &["U.S. Nuclear Regulatory Commission"],
        year: Some(2007),
        topics: &[
            "02-nuclear-safety/fuel-system-design",
            "02-nuclear-safety/fuel-system-design/fuel-system-damage",
        ],
        concept_document: Some("nureg-0800-4.2-rev3"),
        source_url: Some("https://www.nrc.gov/docs/ML0707/ML070740002.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML070740002.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "nureg-km-0004",
        kind: LiteratureKind::Report,
        title: "Fuel Behavior under Abnormal Conditions (NUREG/KM-0004)",
        authors: &["Meyer, R.O."],
        year: Some(2013),
        topics: &[
            "02-nuclear-safety/fuel-system-design/fuel-rod-failure",
            "02-nuclear-safety/fuel-system-design/reactivity-initiated-accident-criteria",
            "02-nuclear-safety/fuel-system-design/fuel-coolability",
            "02-nuclear-safety/accident-analysis/decrease-in-reactor-coolant-inventory",
        ],
        concept_document: None,
        source_url: Some("https://www.nrc.gov/docs/ML1302/ML13028A421.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML13028A421.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "nureg-km-0006",
        kind: LiteratureKind::Report,
        title: "Fundamental Theory of Scientific Computer Simulation Review (NUREG/KM-0006)",
        authors: &["Kaizer, J.S."],
        year: Some(2013),
        topics: &[
            "02-nuclear-safety/accident-analysis/transient-and-accident-analysis-methods/qualification-of-analytical-codes",
            "07-regulatory-framework/quality-assurance/software-quality-assurance",
        ],
        // A tree document since 2026-10-07: the generic design-element
        // concepts under SQA design and implementation cite its section 3.3
        // (#760).
        concept_document: Some("nureg-km-0006"),
        source_url: Some("https://www.nrc.gov/docs/ML1332/ML13325A086.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML13325A086.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "nureg-2201",
        kind: LiteratureKind::Report,
        title: "Probabilistic Risk Assessment and Regulatory Decisionmaking: Some Frequently Asked Questions (NUREG-2201)",
        authors: &["Siu, N.", "Stutzke, M.", "Dennis, S.", "Harrison, D."],
        year: Some(2016),
        topics: &[
            "02-nuclear-safety/severe-accidents/probabilistic-risk-assessment",
        ],
        concept_document: None,
        source_url: Some("https://www.nrc.gov/docs/ML1624/ML16245A032.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML16245A032.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "nureg-cr-7041",
        kind: LiteratureKind::Report,
        title: "SCALE/TRITON Primer: A Primer for Light Water Reactor Lattice Physics Calculations (NUREG/CR-7041)",
        authors: &["Ade, B.J."],
        year: Some(2012),
        topics: &[
            "02-nuclear-safety/nuclear-design/neutron-transport/deterministic-neutronics",
            "02-nuclear-safety/nuclear-design/neutron-transport/multigroup-cross-section-condensation",
            "16-nuclear-fuel-cycle/fuel-depletion",
        ],
        concept_document: None,
        source_url: Some("https://www.nrc.gov/docs/ML1233/ML12338A215.pdf"),
        corpus_file: None,
        status: SourceStatus::Restricted,
        status_basis: NUREG_CR_MOVED_BASIS,
    },
    CorpusLiterature {
        id: "nureg-cr-7289",
        kind: LiteratureKind::Report,
        title: "Nuclear Data Assessment for Advanced Reactors (NUREG/CR-7289)",
        authors: &[
            "Bostelmann, F.",
            "Ilas, G.",
            "Celik, C.",
            "Holcomb, A.M.",
            "Wieselquist, W.A.",
        ],
        year: Some(2022),
        topics: &[
            "02-nuclear-safety/nuclear-design/nuclear-data-processing/nuclear-data-covariances",
            "02-nuclear-safety/nuclear-design/nuclear-data-processing/thermal-scattering",
            "02-nuclear-safety/nuclear-design/core-configurations/pebble-bed",
            "02-nuclear-safety/nuclear-design/core-configurations/liquid-fuelled",
            "02-nuclear-safety/nuclear-design/core-configurations/liquid-metal-cooled",
        ],
        concept_document: None,
        source_url: Some("https://www.nrc.gov/docs/ML2206/ML22063A060.pdf"),
        corpus_file: None,
        status: SourceStatus::Restricted,
        status_basis: NUREG_CR_MOVED_BASIS,
    },
    CorpusLiterature {
        id: "hori2026physor",
        kind: LiteratureKind::Paper,
        title: "Burnup Calculation Using POD-Based Neutron Spectrum Reconstruction: Application to High-Temperature Gas-Cooled Reactor Core Analysis",
        authors: &["Hori, T.", "Chiba, G."],
        year: Some(2026),
        topics: &[
            "16-nuclear-fuel-cycle/fuel-depletion/depletion-solvers",
            "02-nuclear-safety/nuclear-design/core-configurations/prismatic",
        ],
        concept_document: None,
        source_url: Some("https://doi.org/10.5281/zenodo.20803716"),
        corpus_file: Some(
            "kovan-standard-open-corpus/physor-2026/physor2026-206-hori-pod-burnup-httr.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: PHYSOR_2026_BASIS,
    },
    CorpusLiterature {
        id: "bures2026physor",
        kind: LiteratureKind::Paper,
        title: "Design and Neutronics of a Physical Subcritical-Assembly Simulator for Reactor-Physics Education",
        authors: &["Bureš, L.", "Elter, Z."],
        year: Some(2026),
        topics: &[
            "02-nuclear-safety/nuclear-design/neutron-transport/eigenvalue-and-fixed-source-calculations",
            "02-nuclear-safety/nuclear-design/reactor-kinetic-behaviour",
            "02-nuclear-safety/nuclear-design/kinetics-parameters",
            "10-human-resource-development/knowledge-management-and-education/education-and-outreach",
        ],
        concept_document: None,
        source_url: Some("https://doi.org/10.5281/zenodo.20804104"),
        corpus_file: Some(
            "kovan-standard-open-corpus/physor-2026/physor2026-306-bures-subcritical-simulator.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: PHYSOR_2026_BASIS,
    },
    CorpusLiterature {
        id: "acierno2026physor",
        kind: LiteratureKind::Paper,
        title: "Preliminary Thermal-Hydraulics and Neutronics Studies on HEXANA Pool-Type Sodium-cooled Fast Reactor Concept",
        authors: &["Acierno, A.", "Politello, J."],
        year: Some(2026),
        topics: &[
            "02-nuclear-safety/nuclear-design/core-configurations/liquid-metal-cooled",
            "02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics",
            "02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/system-thermal-hydraulics",
            "02-nuclear-safety/nuclear-design/neutron-transport",
            "02-nuclear-safety/accident-analysis",
        ],
        concept_document: None,
        source_url: Some("https://doi.org/10.5281/zenodo.20803769"),
        corpus_file: Some(
            "kovan-standard-open-corpus/physor-2026/physor2026-343-acierno-hexana-sfr.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: PHYSOR_2026_BASIS,
    },
    CorpusLiterature {
        id: "krpan2026physor",
        kind: LiteratureKind::Paper,
        title: "A peek into the MSRE, six decades later: a hyper-fidelity simulation of the classical molten salt reactor",
        authors: &[
            "Krpan, R.",
            "Fiorina, C.",
            "Clarno, K.",
            "Genoni, C.",
            "Gentry, C.A.",
            "Park, S.M.",
            "Ragusa, J.",
        ],
        year: Some(2026),
        topics: &[
            "02-nuclear-safety/nuclear-design/core-configurations/liquid-fuelled",
            "02-nuclear-safety/nuclear-design/neutron-transport",
            "02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/computational-fluid-dynamics",
            "02-nuclear-safety/thermal-hydraulic-design/thermal-hydraulic-methods/neutronics-thermal-hydraulics-coupling",
            "02-nuclear-safety/accident-analysis/transient-and-accident-analysis-methods/qualification-of-analytical-codes",
        ],
        concept_document: None,
        source_url: Some("https://doi.org/10.5281/zenodo.20803785"),
        corpus_file: Some(
            "kovan-standard-open-corpus/physor-2026/physor2026-449-krpan-msre-hyper-fidelity.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: PHYSOR_2026_BASIS,
    },
    CorpusLiterature {
        id: "epa-fgr-11",
        kind: LiteratureKind::Report,
        title: "Limiting Values of Radionuclide Intake and Air Concentration and Dose Conversion \
                Factors for Inhalation, Submersion, and Ingestion (Federal Guidance Report No. 11, \
                EPA-520/1-88-020)",
        authors: &["Eckerman, K.F.", "Wolbarst, A.B.", "Richardson, A.C.B."],
        year: Some(1988),
        topics: &[
            "08-radiation-protection/radiation-protection/monitoring-exposure-control-and-dosimetry",
            "08-radiation-protection/radiation-protection/protection-of-plant-workers",
        ],
        concept_document: None,
        source_url: Some(
            "https://www.epa.gov/sites/default/files/2015-05/documents/520-1-88-020.pdf",
        ),
        corpus_file: Some("kovan-standard-open-corpus/epa/fgr-11-epa-520-1-88-020.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: EPA_BASIS,
    },
    CorpusLiterature {
        id: "epa-fgr-13",
        kind: LiteratureKind::Report,
        title: "Cancer Risk Coefficients for Environmental Exposure to Radionuclides \
                (Federal Guidance Report No. 13, EPA 402-R-99-001)",
        authors: &[
            "Eckerman, K.F.",
            "Leggett, R.W.",
            "Nelson, C.B.",
            "Puskin, J.S.",
            "Richardson, A.C.B.",
        ],
        year: Some(1999),
        topics: &[
            "08-radiation-protection/radiation-protection/monitoring-exposure-control-and-dosimetry",
            "13-environmental-protection/radiological-impacts-of-normal-operation/doses-to-members-of-the-public",
        ],
        concept_document: None,
        source_url: Some(
            "https://www.epa.gov/system/files/documents/2025-03/402-r-99-001_508-d_2.pdf",
        ),
        corpus_file: Some("kovan-standard-open-corpus/epa/fgr-13-epa-402-r-99-001.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: EPA_BASIS,
    },
    CorpusLiterature {
        id: "epa-fgr-15",
        kind: LiteratureKind::Report,
        title: "External Exposure to Radionuclides in Air, Water and Soil \
                (Federal Guidance Report No. 15, EPA 402-R-25-001, revised July 2025)",
        authors: &[
            "Bellamy, M.B.",
            "Samuels, C.E.",
            "Dewji, S.A.",
            "Leggett, R.W.",
            "Hiller, M.",
            "Veinot, K.",
            "Manger, R.P.",
            "Ryman, J.C.",
            "Easterly, C.E.",
            "Hertel, N.E.",
            "Stewart, D.J.",
            "Eckerman, K.F.",
        ],
        year: Some(2025),
        topics: &[
            "08-radiation-protection/radiation-protection/monitoring-exposure-control-and-dosimetry",
            "13-environmental-protection/radiological-impacts-of-normal-operation/exposure-pathways",
        ],
        concept_document: None,
        source_url: Some(
            "https://www.epa.gov/system/files/documents/2025-07/fgr15_rev2025july_final_508.pdf",
        ),
        corpus_file: Some("kovan-standard-open-corpus/epa/fgr-15-epa-402-r-25-001.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: EPA_BASIS,
    },
    // ── The concept tree's documents (added 2026-10-06, #724/#727) ──────
    // Every `[[document]]` of `kovan-literature/src/concept_skeleton.toml`
    // not already above. Titles, authors and dates read from each PDF's
    // title page (`pdftotext`, 2026-10-06) and the corpus README's rows for
    // them; filed under every node that cites them (`concept_document`), so
    // `topics` is empty. A `source_url` is given only where the document or
    // the README records one: the four `nureg-…` files were supplied without
    // their ADAMS accession numbers, so they have none yet.
    CorpusLiterature {
        id: "nureg-0800-toc-rev6",
        kind: LiteratureKind::Report,
        title: "Standard Review Plan (SRP) for the Review of Safety Analysis Reports for Nuclear \
                Power Plants, Table of Contents, Revision 6 (NUREG-0800)",
        authors: &["U.S. Nuclear Regulatory Commission"],
        year: Some(2007),
        topics: &[],
        concept_document: Some("nureg-0800-toc-rev6"),
        source_url: Some("https://www.nrc.gov/docs/ML0708/ML070810350.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML070810350.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "nureg-1537-part1",
        kind: LiteratureKind::Report,
        title: "Guidelines for Preparing and Reviewing Applications for the Licensing of \
                Non-Power Reactors: Format and Content (NUREG-1537, Part 1)",
        authors: &["U.S. Nuclear Regulatory Commission, Office of Nuclear Reactor Regulation"],
        year: Some(1996),
        topics: &[],
        concept_document: Some("nureg-1537-part1"),
        source_url: None,
        corpus_file: Some("kovan-standard-open-corpus/nrc/nureg-1537-part1-1996.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "rg-1.232-rev0",
        kind: LiteratureKind::Report,
        title: "Guidance for Developing Principal Design Criteria for Non-Light-Water Reactors \
                (Regulatory Guide 1.232, Revision 0)",
        authors: &["U.S. Nuclear Regulatory Commission"],
        year: Some(2018),
        topics: &[],
        concept_document: Some("rg-1.232-rev0"),
        source_url: Some("https://www.nrc.gov/docs/ML1732/ML17325A611.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML17325A611.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "ornl-tm-2018-976",
        kind: LiteratureKind::Report,
        title: "Regulatory Gap Analysis of Select NUREG-0800 Chapters for Applicability to \
                Molten Salt Reactors (ORNL/TM-2018/976)",
        authors: &["Belles, R.J.", "Flanagan, G.F."],
        year: Some(2018),
        topics: &[],
        concept_document: Some("ornl-tm-2018-976"),
        source_url: None,
        corpus_file: Some(
            "kovan-standard-open-corpus/us-doe/ornl-tm-2018-976-msr-nureg0800-gap-analysis.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: "DOE report marked for unlimited distribution: the cover (PDF page 1) \
                       states \"Approved for public release. Distribution is unlimited.\" \
                       (checked 2026-10-06; corpus README section 4). Contractor-written, so \
                       not claimed as public domain under 17 U.S.C. 105",
    },
    CorpusLiterature {
        id: "nureg-1520-rev2",
        kind: LiteratureKind::Report,
        title: "Standard Review Plan for Fuel Cycle Facilities License Applications, Final \
                Report (NUREG-1520, Revision 2)",
        authors: &["U.S. Nuclear Regulatory Commission, Office of Nuclear Material Safety and Safeguards"],
        year: Some(2015),
        topics: &[],
        concept_document: Some("nureg-1520-rev2"),
        source_url: None,
        corpus_file: Some("kovan-standard-open-corpus/nrc/nureg-1520-rev2-2015.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "nureg-1555",
        kind: LiteratureKind::Report,
        title: "Standard Review Plans for Environmental Reviews for Nuclear Power Plants \
                (Environmental Standard Review Plan, NUREG-1555)",
        authors: &["U.S. Nuclear Regulatory Commission, Office of Nuclear Reactor Regulation"],
        year: Some(1999),
        topics: &[],
        concept_document: Some("nureg-1555"),
        source_url: None,
        corpus_file: Some("kovan-standard-open-corpus/nrc/nureg-1555-1999.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "nureg-0654-rev2",
        kind: LiteratureKind::Report,
        title: "Criteria for Preparation and Evaluation of Radiological Emergency Response \
                Plans and Preparedness in Support of Nuclear Power Plants, Final Report \
                (NUREG-0654/FEMA-REP-1, Revision 2)",
        authors: &[
            "U.S. Nuclear Regulatory Commission",
            "Federal Emergency Management Agency",
        ],
        year: Some(2019),
        topics: &[],
        concept_document: Some("nureg-0654-rev2"),
        source_url: None,
        corpus_file: Some("kovan-standard-open-corpus/nrc/nureg-0654-fema-rep-1-rev2-2019.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: "U.S. Government Work, not subject to copyright: a joint publication of \
                       the NRC and FEMA, both U.S. Government agencies (17 U.S.C. 105); NRC Site \
                       Disclaimer (https://www.nrc.gov/about-nrc/site-disclaimer)",
    },
    CorpusLiterature {
        id: "jrc-eur-28712",
        kind: LiteratureKind::Report,
        title: "The High Temperature Gas-cooled Reactor: Safety considerations of the \
                (V)HTR-Modul (EUR 28712 EN)",
        authors: &["Kugeler, K.", "Nabielek, H.", "Buckthorpe, D."],
        year: Some(2017),
        topics: &[],
        concept_document: Some("jrc-eur-28712"),
        source_url: Some("https://doi.org/10.2760/270321"),
        corpus_file: Some("kovan-standard-open-corpus/eu-jrc/kjna28712enn.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: "European Commission reuse notice (PDF page 2, legal notice): \"Reuse is \
                       authorised provided the source is acknowledged. The reuse policy of \
                       European Commission documents is regulated by Decision 2011/833/EU\" \
                       (corpus README section 5)",
    },
    CorpusLiterature {
        id: "10cfr50",
        kind: LiteratureKind::Other,
        title: "10 CFR Part 50, Domestic Licensing of Production and Utilization Facilities \
                (eCFR, up to date as of 2 October 2026)",
        authors: &["U.S. Nuclear Regulatory Commission"],
        year: Some(2026),
        topics: &[],
        concept_document: Some("10cfr50"),
        source_url: Some("https://www.ecfr.gov/current/title-10/chapter-I/part-50"),
        corpus_file: Some("kovan-standard-open-corpus/cfr/10cfr50-ecfr-2026-10-02.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: CFR_BASIS,
    },
    CorpusLiterature {
        id: "10cfr52",
        kind: LiteratureKind::Other,
        title: "10 CFR Part 52, Licenses, Certifications, and Approvals for Nuclear Power Plants \
                (eCFR, up to date as of 2 October 2026)",
        authors: &["U.S. Nuclear Regulatory Commission"],
        year: Some(2026),
        topics: &[],
        concept_document: Some("10cfr52"),
        source_url: Some("https://www.ecfr.gov/current/title-10/chapter-I/part-52"),
        corpus_file: Some("kovan-standard-open-corpus/cfr/10cfr52-ecfr-2026-10-02.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: CFR_BASIS,
    },
    CorpusLiterature {
        id: "10cfr53",
        kind: LiteratureKind::Other,
        title: "10 CFR Part 53, Risk-Informed, Technology-Inclusive Regulatory Framework for \
                Commercial Nuclear Plants (eCFR, up to date as of 2 October 2026)",
        authors: &["U.S. Nuclear Regulatory Commission"],
        year: Some(2026),
        topics: &[],
        concept_document: Some("10cfr53"),
        source_url: Some("https://www.ecfr.gov/current/title-10/chapter-I/part-53"),
        corpus_file: Some("kovan-standard-open-corpus/cfr/10cfr53-ecfr-2026-10-02.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: CFR_BASIS,
    },
    // Software quality assurance (#760, 2026-10-07): filed under the
    // approved `software-quality-assurance` concept, which cites them.
    CorpusLiterature {
        id: "nureg-br-0167",
        kind: LiteratureKind::Report,
        title: "Software Quality Assurance Program and Guidelines (NUREG/BR-0167)",
        authors: &[
            "U.S. Nuclear Regulatory Commission, Office of Information Resources Management",
        ],
        year: Some(1993),
        topics: &["07-regulatory-framework/quality-assurance/software-quality-assurance"],
        concept_document: Some("nureg-br-0167"),
        source_url: None,
        corpus_file: Some(
            "kovan-standard-open-corpus/nrc/nureg-br-0167-1993-sqa-program-and-guidelines.pdf",
        ),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "doe-g-414.1-4",
        kind: LiteratureKind::Report,
        title: "Safety Software Guide for Use with 10 CFR 830 Subpart A, Quality Assurance \
                Requirements, and DOE O 414.1C, Quality Assurance (DOE G 414.1-4)",
        authors: &["U.S. Department of Energy, Office of Environment, Safety and Health"],
        year: Some(2005),
        topics: &["07-regulatory-framework/quality-assurance/software-quality-assurance"],
        concept_document: Some("doe-g-414.1-4"),
        source_url: Some(
            "https://www.energy.gov/sites/prod/files/hss/Enforcement%20and%20Oversight/Enforcement/docs/guides/DOE_Guide_414_1_4.pdf",
        ),
        corpus_file: Some(
            "kovan-standard-open-corpus/us-doe/doe-g-414-1-4-2005-safety-software-guide.pdf",
        ),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: "U.S. Government Work written by DOE itself (17 U.S.C. 105); DOE's Copyright, \
                       Restrictions and Permissions Notice (https://www.energy.gov/web-policies, \
                       accessed 2026-10-07): public domain, may be freely distributed, \
                       acknowledge DOE (corpus README section 4, extended)",
    },
    CorpusLiterature {
        id: "doe-std-1172-2003",
        kind: LiteratureKind::Report,
        title: "Safety Software Quality Assurance Functional Area Qualification Standard \
                (DOE-STD-1172-2003)",
        authors: &["U.S. Department of Energy"],
        year: Some(2003),
        topics: &["07-regulatory-framework/quality-assurance/software-quality-assurance"],
        concept_document: Some("doe-std-1172-2003"),
        source_url: None,
        corpus_file: Some(
            "kovan-standard-open-corpus/us-doe/doe-std-1172-2003-safety-software-qa-faqs.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: "DOE technical standard marked \"DISTRIBUTION STATEMENT A. Approved for \
                       public release; distribution is unlimited.\" (PDF page 1, checked \
                       2026-10-07; corpus README section 4). Superseded by DOE-STD-1172-2011",
    },
    // The NJOY2016 manual (#760, question 8, 2026-10-07): moved from the
    // maintainer's open corpus on the owner's decision ("it is the bedrock
    // of nuclear science"); corpus README ground 7, section 8. Metadata from
    // the PDF's title page (`pdftotext`, 2026-10-07).
    CorpusLiterature {
        id: "la-ur-17-20093",
        kind: LiteratureKind::Report,
        title: "The NJOY Nuclear Data Processing System, Version 2016 (LA-UR-17-20093), \
                updated for NJOY2016.53, November 7, 2019",
        authors: &[
            "MacFarlane, R.E.",
            "Muir, D.W.",
            "Boicourt, R.M.",
            "Kahler, A.C.",
            "Conlin, J.L.",
            "Haeck, W.",
        ],
        year: Some(2019),
        // Filed under each module's sub-concept too (maintainer,
        // 2026-10-07); the tree cites the manual chapter by chapter, with
        // printed and PDF pages read from the PDF (concept_proposals.toml):
        // RECONR ch. 3, BROADR 4, UNRESR 5 and PURR 23, THERMR 7 and LEAPR
        // 24, ACER 17, GROUPR 8, GAMINR 9, DTFR 13, CCCCR 14, MATXSR 15,
        // POWR 18 and WIMSR 19, ERRORR 10 and COVR 11, HEATR 6. No
        // sub-concept exists for NJOY itself (ch. 2), MODER (12), RESXSR
        // (16), PLOTR (20), VIEWR (21), MIXR (22) or GASPR (25); those stay
        // under the parent concept only.
        topics: &[
            "02-nuclear-safety/nuclear-design/nuclear-data-processing",
            "02-nuclear-safety/nuclear-design/nuclear-data-processing/resonance-reconstruction",
            "02-nuclear-safety/nuclear-design/nuclear-data-processing/doppler-broadening",
            "02-nuclear-safety/nuclear-design/nuclear-data-processing/unresolved-resonance-probability-tables",
            "02-nuclear-safety/nuclear-design/nuclear-data-processing/thermal-scattering",
            "02-nuclear-safety/nuclear-design/nuclear-data-processing/ace-library-generation",
            "02-nuclear-safety/nuclear-design/nuclear-data-processing/multigroup-data-generation",
            "02-nuclear-safety/nuclear-design/nuclear-data-processing/nuclear-data-covariances",
            "02-nuclear-safety/nuclear-design/nuclear-data-processing/heating-and-damage",
        ],
        concept_document: Some("la-ur-17-20093"),
        source_url: Some("https://github.com/njoy/NJOY2016-manual"),
        corpus_file: Some("kovan-standard-open-corpus/lanl/2022laur1720093.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: "Los Alamos National Security, LLC's BSD-3-style licence: \"redistribution \
                       and use in source and binary forms, with or without modification, are \
                       permitted provided that\" the notice is retained and reproduced, and no \
                       endorsement is implied. In the LICENSE of \
                       https://github.com/njoy/NJOY2016-manual (commit 9a2951f, checked \
                       2026-10-07) and printed on PDF page 2; the licence text is kept beside \
                       the PDF (corpus README ground 7, section 8). The PDF is byte-identical \
                       to that commit's njoy16.pdf",
    },
    // Bedrock documents for the IAEA Milestones issues (#760, 2026-10-07):
    // found by a search across the 19 issues, each re-read and its basis
    // quoted in the corpus README (NRC: section 1; DOE: section 4; CRS and
    // GAO: section 9, ground 8). Metadata from each document's own title
    // pages (or, where stated, its official landing page). Hand-filed under
    // the most specific existing concept for its issue.
    CorpusLiterature {
        id: "nureg-br-0500-rev4",
        kind: LiteratureKind::Report,
        title: "Safety Culture Policy Statement (NUREG/BR-0500, Revision 4)",
        authors: &["U.S. Nuclear Regulatory Commission"],
        year: Some(2018),
        topics: &["03-management"],
        concept_document: None,
        source_url: Some("https://www.nrc.gov/docs/ML1813/ML18137A389.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML18137A389.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "nureg-0980-v1-n12",
        kind: LiteratureKind::Report,
        title: "Nuclear Regulatory Legislation, 117th Congress; 2nd Session (NUREG-0980, Vol. 1, \
                No. 12)",
        authors: &["U.S. Nuclear Regulatory Commission, Office of the General Counsel"],
        year: Some(2025),
        topics: &["05-legal-framework/nuclear-legislation"],
        concept_document: None,
        source_url: Some("https://www.nrc.gov/docs/ML2512/ML25120A424.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML25120A424.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "nureg-2159-rev1",
        kind: LiteratureKind::Report,
        title: "Acceptable Standard Format and Content for the Fundamental Nuclear Material Control \
                Plan Required for Special Nuclear Material of Moderate Strategic Significance, \
                Final Report (NUREG-2159, Revision 1)",
        authors: &["Pham, T.", "Tuttle, G.", "Ani, S."],
        year: Some(2022),
        topics: &["06-safeguards/material-control-and-accounting"],
        concept_document: None,
        source_url: Some("https://www.nrc.gov/docs/ML2214/ML22143A963.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML22143A963.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "nureg-1736",
        kind: LiteratureKind::Report,
        title: "Consolidated Guidance: 10 CFR Part 20 - Standards for Protection Against Radiation, \
                Final Report (NUREG-1736)",
        authors: &[
            "Zelac, R.E.",
            "Cameron, J.L.",
            "Karagiannis, H.",
            "McGrath, J.R.",
            "Sherbini, S.S.",
            "Thomas, M.L.",
            "Wigginton, J.E.",
        ],
        year: Some(2001),
        topics: &["08-radiation-protection/radiation-protection/dose-limits-and-criteria"],
        concept_document: None,
        // ADAMS holds it in two parts (ML013330106, ML013330154); the
        // corpus file is the two joined, as the README records.
        source_url: Some("https://www.nrc.gov/reading-rm/doc-collections/nuregs/staff/sr1736/"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/nureg-1736-2001.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "nureg-1032",
        kind: LiteratureKind::Report,
        title: "Evaluation of Station Blackout Accidents at Nuclear Power Plants: Technical Findings \
                Related to Unresolved Safety Issue A-44, Final Report (NUREG-1032)",
        authors: &["Baranowsky, P.W."],
        year: Some(1988),
        topics: &["09-electrical-grid/electric-power/station-blackout"],
        concept_document: None,
        source_url: Some("https://www.osti.gov/servlets/purl/5122568"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/nureg-1032-1988.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: "U.S. Government Work written by NRC staff, not subject to copyright: NRC Site \
                       Disclaimer (https://www.nrc.gov/about-nrc/site-disclaimer); 17 U.S.C. 105. \
                       The OSTI copy's cover is also stamped \"DISTRIBUTION OF THIS DOCUMENT IS \
                       UNLIMITED\" (corpus README section 1)",
    },
    CorpusLiterature {
        id: "nureg-br-0215-rev2",
        kind: LiteratureKind::Report,
        title: "Public Involvement in the Nuclear Regulatory Process (NUREG/BR-0215, Revision 2)",
        authors: &["U.S. Nuclear Regulatory Commission, Office of Public Affairs"],
        year: Some(2004),
        topics: &["11-stakeholder-involvement/public-participation-in-licensing"],
        concept_document: None,
        source_url: Some(
            "https://www.nrc.gov/reading-rm/doc-collections/nuregs/brochures/br0215/",
        ),
        corpus_file: Some("kovan-standard-open-corpus/nrc/nureg-br-0215-rev2-2004.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "rg-4.7-rev3",
        kind: LiteratureKind::Report,
        title: "General Site Suitability Criteria for Nuclear Power Stations (Regulatory Guide 4.7, \
                Revision 3)",
        authors: &["U.S. Nuclear Regulatory Commission, Office of Nuclear Regulatory Research"],
        year: Some(2014),
        topics: &["12-site-and-supporting-facilities/site-characteristics"],
        concept_document: None,
        source_url: Some("https://www.nrc.gov/docs/ML1218/ML12188A053.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML12188A053.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "nureg-0396",
        kind: LiteratureKind::Report,
        title: "Planning Basis for the Development of State and Local Government Radiological \
                Emergency Response Plans in Support of Light Water Nuclear Power Plants \
                (NUREG-0396, EPA 520/1-78-016)",
        authors: &["Collins, H.E.", "Grimes, B.K.", "Galpin, F."],
        year: Some(1978),
        topics: &["14-emergency-planning/protective-response/plume-exposure-pathway-epz-size"],
        concept_document: None,
        source_url: Some("https://www.nrc.gov/docs/ML0513/ML051390356.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML051390356.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: "U.S. Government Work, not subject to copyright: prepared by a joint NRC and \
                       EPA task force, both U.S. Government agencies (17 U.S.C. 105); NRC Site \
                       Disclaimer (https://www.nrc.gov/about-nrc/site-disclaimer)",
    },
    CorpusLiterature {
        id: "rg-5.71-rev1",
        kind: LiteratureKind::Report,
        title: "Cybersecurity Programs for Nuclear Power Reactors (Regulatory Guide 5.71, \
                Revision 1)",
        authors: &["U.S. Nuclear Regulatory Commission"],
        year: Some(2023),
        topics: &["15-nuclear-security/cybersecurity/cybersecurity-program"],
        concept_document: None,
        source_url: Some("https://www.nrc.gov/docs/ML2225/ML22258A204.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML22258A204.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "nureg-1757-v2-rev2",
        kind: LiteratureKind::Report,
        title: "Consolidated Decommissioning Guidance: Characterization, Survey, and Determination \
                of Radiological Criteria, Final Report (NUREG-1757, Volume 2, Revision 2)",
        authors: &[
            "Barr, C.S.",
            "Clark, S.",
            "Chapman, G.C.",
            "Esh, D.W.",
            "Fedors, R.W.",
            "Huffert, A.M.",
            "Kauffman, L.A.",
            "LaFranzo, M.M.",
            "McKenney, C.A.",
            "Parks, L.L.",
            "Schmidt, D.W.",
            "Schwartzman, A.L.",
            "Watson, B.A.",
        ],
        year: Some(2022),
        topics: &["17-radioactive-waste-management/decommissioning/release-criteria-and-final-survey"],
        concept_document: None,
        source_url: Some("https://www.nrc.gov/docs/ML2219/ML22194A859.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML22194A859.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "rg-1.164-rev1",
        kind: LiteratureKind::Report,
        title: "Dedication of Commercial-Grade Items for Use in Nuclear Power Plants (Regulatory \
                Guide 1.164, Revision 1)",
        authors: &["U.S. Nuclear Regulatory Commission"],
        year: Some(2024),
        topics: &["19-procurement/supplier-quality-and-specifications"],
        concept_document: None,
        source_url: Some("https://www.nrc.gov/docs/ML2403/ML24038A310.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML24038A310.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "crs-r42853",
        kind: LiteratureKind::Report,
        title: "Nuclear Energy: Overview of Congressional Issues (CRS Report R42853, updated \
                December 3, 2024)",
        authors: &["Holt, M."],
        year: Some(2024),
        topics: &["01-national-position"],
        concept_document: None,
        source_url: Some("https://crsreports.congress.gov"),
        corpus_file: Some("kovan-standard-open-corpus/us-congress/crs-r42853-2024-12-03.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: CRS_BASIS,
    },
    CorpusLiterature {
        id: "crs-if10821",
        kind: LiteratureKind::Report,
        title: "Price-Anderson Act: Nuclear Power Industry Liability Limits and Compensation to the \
                Public After Radioactive Releases (CRS In Focus IF10821, updated February 28, 2025)",
        authors: &["Holt, M."],
        year: Some(2025),
        topics: &["05-legal-framework/civil-liability-for-nuclear-damage"],
        concept_document: None,
        source_url: Some("https://crsreports.congress.gov"),
        corpus_file: Some("kovan-standard-open-corpus/us-congress/crs-if10821-2025-02-28.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: CRS_BASIS,
    },
    CorpusLiterature {
        id: "gao-15-652",
        kind: LiteratureKind::Report,
        title: "Technology Assessment: Nuclear Reactors: Status and challenges in development and \
                deployment of new commercial concepts (GAO-15-652)",
        authors: &["U.S. Government Accountability Office"],
        year: Some(2015),
        topics: &["01-national-position"],
        concept_document: None,
        source_url: Some("https://www.gao.gov"),
        corpus_file: Some("kovan-standard-open-corpus/us-congress/gao-15-652.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: "U.S. Government Work: \"This is a work of the U.S. government and is not \
                       subject to copyright protection in the United States. The published \
                       product may be reproduced and distributed in its entirety without further \
                       permission from GAO.\" (PDF page 4, checked 2026-10-07; corpus README \
                       section 9). Third-party images inside keep their holders' copyright",
    },
    CorpusLiterature {
        id: "doe-hdbk-1019-1-93",
        kind: LiteratureKind::Report,
        title: "DOE Fundamentals Handbook: Nuclear Physics and Reactor Theory, Volume 1 of 2 \
                (DOE-HDBK-1019/1-93)",
        authors: &["U.S. Department of Energy"],
        year: Some(1993),
        topics: &[
            "10-human-resource-development/knowledge-management-and-education/education-and-outreach",
            "02-nuclear-safety/nuclear-design",
        ],
        concept_document: None,
        source_url: Some(
            "https://www.energy.gov/sites/default/files/2026-04/DOE-HDBK-1019-93_VOL1.pdf",
        ),
        corpus_file: Some(
            "kovan-standard-open-corpus/us-doe/doe-hdbk-1019-1-93-nuclear-physics-reactor-theory.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: DOE_DIST_A_BASIS,
    },
    CorpusLiterature {
        id: "doe-hdbk-1019-2-93",
        kind: LiteratureKind::Report,
        title: "DOE Fundamentals Handbook: Nuclear Physics and Reactor Theory, Volume 2 of 2 \
                (DOE-HDBK-1019/2-93)",
        authors: &["U.S. Department of Energy"],
        year: Some(1993),
        topics: &[
            "10-human-resource-development/knowledge-management-and-education/education-and-outreach",
            "02-nuclear-safety/nuclear-design",
        ],
        concept_document: None,
        source_url: Some(
            "https://www.energy.gov/sites/default/files/2026-04/DOE-HDBK-1019-93_VOL2.pdf",
        ),
        corpus_file: Some(
            "kovan-standard-open-corpus/us-doe/doe-hdbk-1019-2-93-nuclear-physics-reactor-theory.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: DOE_DIST_A_BASIS,
    },
    CorpusLiterature {
        id: "doe-hdbk-1012-1-92",
        kind: LiteratureKind::Report,
        title: "DOE Fundamentals Handbook: Thermodynamics, Heat Transfer, and Fluid Flow, Volume 1 \
                of 3 (DOE-HDBK-1012/1-92)",
        authors: &["U.S. Department of Energy"],
        year: Some(1992),
        topics: &[
            "10-human-resource-development/knowledge-management-and-education/education-and-outreach",
            "02-nuclear-safety/thermal-hydraulic-design",
        ],
        concept_document: None,
        source_url: Some(
            "https://www.energy.gov/sites/default/files/2026-04/DOE-HDBK-1012-92_VOL1.pdf",
        ),
        corpus_file: Some(
            "kovan-standard-open-corpus/us-doe/doe-hdbk-1012-1-92-thermo-heat-transfer-fluid-flow.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: DOE_DIST_A_BASIS,
    },
    CorpusLiterature {
        id: "doe-hdbk-1012-2-92",
        kind: LiteratureKind::Report,
        title: "DOE Fundamentals Handbook: Thermodynamics, Heat Transfer, and Fluid Flow, Volume 2 \
                of 3 (DOE-HDBK-1012/2-92)",
        authors: &["U.S. Department of Energy"],
        year: Some(1992),
        topics: &[
            "10-human-resource-development/knowledge-management-and-education/education-and-outreach",
            "02-nuclear-safety/thermal-hydraulic-design",
        ],
        concept_document: None,
        source_url: Some(
            "https://www.energy.gov/sites/default/files/2026-04/DOE-HDBK-1012-92_VOL2.pdf",
        ),
        corpus_file: Some(
            "kovan-standard-open-corpus/us-doe/doe-hdbk-1012-2-92-thermo-heat-transfer-fluid-flow.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: DOE_DIST_A_BASIS,
    },
    CorpusLiterature {
        id: "doe-hdbk-1012-3-92",
        kind: LiteratureKind::Report,
        title: "DOE Fundamentals Handbook: Thermodynamics, Heat Transfer, and Fluid Flow, Volume 3 \
                of 3 (DOE-HDBK-1012/3-92)",
        authors: &["U.S. Department of Energy"],
        year: Some(1992),
        topics: &[
            "10-human-resource-development/knowledge-management-and-education/education-and-outreach",
            "02-nuclear-safety/thermal-hydraulic-design",
        ],
        concept_document: None,
        source_url: Some(
            "https://www.energy.gov/sites/default/files/2026-04/DOE-HDBK-1012-92_VOL3.pdf",
        ),
        corpus_file: Some(
            "kovan-standard-open-corpus/us-doe/doe-hdbk-1012-3-92-thermo-heat-transfer-fluid-flow.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: DOE_DIST_A_BASIS,
    },
    CorpusLiterature {
        id: "doe-hdbk-1017-1-93",
        kind: LiteratureKind::Report,
        title: "DOE Fundamentals Handbook: Material Science, Volume 1 of 2 (DOE-HDBK-1017/1-93)",
        authors: &["U.S. Department of Energy"],
        year: Some(1993),
        topics: &[
            "10-human-resource-development/knowledge-management-and-education/education-and-outreach",
            "02-nuclear-safety/design-of-structures-systems-and-components",
        ],
        concept_document: None,
        source_url: Some(
            "https://www.energy.gov/sites/default/files/2026-04/DOE-HDBK-1017-93_VOL1.pdf",
        ),
        corpus_file: Some("kovan-standard-open-corpus/us-doe/doe-hdbk-1017-1-93-material-science.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: DOE_DIST_A_BASIS_2026_10_10,
    },
    CorpusLiterature {
        id: "doe-hdbk-1017-2-93",
        kind: LiteratureKind::Report,
        title: "DOE Fundamentals Handbook: Material Science, Volume 2 of 2 (DOE-HDBK-1017/2-93)",
        authors: &["U.S. Department of Energy"],
        year: Some(1993),
        topics: &[
            "10-human-resource-development/knowledge-management-and-education/education-and-outreach",
            "02-nuclear-safety/design-of-structures-systems-and-components",
        ],
        concept_document: None,
        source_url: Some(
            "https://www.energy.gov/sites/default/files/2026-04/DOE-HDBK-1017-93_VOL2.pdf",
        ),
        corpus_file: Some("kovan-standard-open-corpus/us-doe/doe-hdbk-1017-2-93-material-science.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: DOE_DIST_A_BASIS_2026_10_10,
    },
    CorpusLiterature {
        id: "doe-hdbk-1015-1-93",
        kind: LiteratureKind::Report,
        title: "DOE Fundamentals Handbook: Chemistry, Volume 1 of 2 (DOE-HDBK-1015/1-93)",
        authors: &["U.S. Department of Energy"],
        year: Some(1993),
        topics: &[
            "10-human-resource-development/knowledge-management-and-education/education-and-outreach",
            "02-nuclear-safety/reactor-coolant-system",
        ],
        concept_document: None,
        source_url: Some(
            "https://www.energy.gov/sites/default/files/2026-04/DOE-HDBK-1015-93_VOL1.pdf",
        ),
        corpus_file: Some("kovan-standard-open-corpus/us-doe/doe-hdbk-1015-1-93-chemistry.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: DOE_DIST_A_BASIS_2026_10_10,
    },
    CorpusLiterature {
        id: "doe-hdbk-1015-2-93",
        kind: LiteratureKind::Report,
        title: "DOE Fundamentals Handbook: Chemistry, Volume 2 of 2 (DOE-HDBK-1015/2-93)",
        authors: &["U.S. Department of Energy"],
        year: Some(1993),
        topics: &[
            "10-human-resource-development/knowledge-management-and-education/education-and-outreach",
            "02-nuclear-safety/reactor-coolant-system",
        ],
        concept_document: None,
        source_url: Some(
            "https://www.energy.gov/sites/default/files/2026-04/DOE-HDBK-1015-93_VOL2.pdf",
        ),
        corpus_file: Some("kovan-standard-open-corpus/us-doe/doe-hdbk-1015-2-93-chemistry.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: DOE_DIST_A_BASIS_2026_10_10,
    },
    CorpusLiterature {
        id: "doe-hdbk-1014-1-92",
        kind: LiteratureKind::Report,
        title: "DOE Fundamentals Handbook: Mathematics, Volume 1 of 2 (DOE-HDBK-1014/1-92)",
        authors: &["U.S. Department of Energy"],
        year: Some(1992),
        topics: &[
            "10-human-resource-development/knowledge-management-and-education/education-and-outreach",
        ],
        concept_document: None,
        source_url: Some(
            "https://www.energy.gov/sites/default/files/2026-04/DOE-HDBK-1014-92_VOL1.pdf",
        ),
        corpus_file: Some("kovan-standard-open-corpus/us-doe/doe-hdbk-1014-1-92-mathematics.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: DOE_DIST_A_BASIS_2026_10_10,
    },
    CorpusLiterature {
        id: "doe-hdbk-1014-2-92",
        kind: LiteratureKind::Report,
        title: "DOE Fundamentals Handbook: Mathematics, Volume 2 of 2 (DOE-HDBK-1014/2-92)",
        authors: &["U.S. Department of Energy"],
        year: Some(1992),
        topics: &[
            "10-human-resource-development/knowledge-management-and-education/education-and-outreach",
        ],
        concept_document: None,
        source_url: Some(
            "https://www.energy.gov/sites/default/files/2026-04/DOE-HDBK-1014-92_VOL2.pdf",
        ),
        corpus_file: Some("kovan-standard-open-corpus/us-doe/doe-hdbk-1014-2-92-mathematics.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: DOE_DIST_A_BASIS_2026_10_10,
    },
    CorpusLiterature {
        id: "nfwg-2020",
        kind: LiteratureKind::Report,
        title: "Restoring America's Competitive Nuclear Energy Advantage: A strategy to assure U.S. \
                national security (U.S. Nuclear Fuel Working Group)",
        authors: &["U.S. Department of Energy"],
        year: Some(2020),
        topics: &["16-nuclear-fuel-cycle"],
        concept_document: None,
        source_url: Some(
            "https://www.energy.gov/downloads/restoring-americas-competitive-nuclear-energy-advantage",
        ),
        corpus_file: Some(
            "kovan-standard-open-corpus/us-doe/nfwg-2020-restoring-competitive-nuclear-advantage.pdf",
        ),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: "U.S. Government Work issued by DOE itself (DOE seal and name on the cover; \
                       17 U.S.C. 105); DOE's Copyright, Restrictions and Permissions Notice \
                       (https://www.energy.gov/web-policies): public domain, may be freely \
                       distributed, acknowledge DOE (corpus README section 4, extended). No \
                       copyright notice in the document (checked 2026-10-07)",
    },
    CorpusLiterature {
        id: "ornl-tm-2020-1522",
        kind: LiteratureKind::Report,
        title: "Integrated Energy System Investigation for the Eastman Chemical Company Kingsport, \
                Tennessee, Facility (ORNL/TM-2020/1522)",
        authors: &[
            "Greenwood, M.S.",
            "Guler Yigitoglu, A.",
            "Rader, J.D.",
            "Tharp, W.",
            "Poore, M.",
            "Belles, R.",
            "Zhang, B.",
            "Cumberland, R.",
            "Muhlheim, M.",
        ],
        year: Some(2020),
        topics: &["18-industrial-involvement/process-heat-and-industrial-applications"],
        concept_document: None,
        source_url: Some("https://info.ornl.gov/sites/publications/Files/Pub139613.pdf"),
        corpus_file: Some(
            "kovan-standard-open-corpus/us-doe/ornl-tm-2020-1522-ies-eastman-kingsport.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: "DOE laboratory report marked \"Unlimited Release\" on its cover (PDF page 1, \
                       checked 2026-10-07; accepted by the owner as an unlimited-distribution \
                       marking; corpus README section 4). Contractor-written, so not claimed as \
                       public domain",
    },
    CorpusLiterature {
        id: "ornl-tm-2014-88",
        kind: LiteratureKind::Report,
        title: "Small, Modular Advanced High-Temperature Reactor-Carbonate Thermochemical Cycle \
                (ORNL/TM-2014/88)",
        authors: &["Holcomb, D.E.", "Ilas, D.", "Middleton, B.", "Arrieta, M."],
        year: Some(2014),
        topics: &["18-industrial-involvement/process-heat-and-industrial-applications"],
        concept_document: None,
        source_url: Some("https://info.ornl.gov/sites/publications/Files/Pub48861.pdf"),
        corpus_file: Some(
            "kovan-standard-open-corpus/us-doe/ornl-tm-2014-88-smahtr-carbonate-cycle.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: DOE_LAB_UNLIMITED_BASIS,
    },
    // Solver-pattern sources (#760, 2026-10-07): the concept tree cites
    // them (concept_skeleton.toml `[[document]]`s), so they are filed under
    // the nodes that cite them and carry no hand filing. Bases quoted in
    // the corpus README: sections 4 (DOE laboratories), 2 (CC BY), 10
    // (CC BY-SA, CC BY-NC-ND) and 8 (project documentation under its
    // software licence).
    CorpusLiterature {
        id: "sand2011-2195",
        kind: LiteratureKind::Report,
        title: "A Theory Manual for Multi-physics Code Coupling in LIME, Version 1.0 (SAND2011-2195)",
        authors: &[
            "Pawlowski, R.",
            "Bartlett, R.",
            "Belcourt, N.",
            "Hooper, R.",
            "Schmidt, R.",
        ],
        year: Some(2011),
        topics: &[],
        concept_document: Some("sand2011-2195"),
        source_url: Some("https://www.osti.gov/servlets/purl/1011710"),
        corpus_file: Some(
            "kovan-standard-open-corpus/us-doe/sand2011-2195-lime-coupling-theory-manual.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: "Sandia report marked \"Unlimited Release\" and \"Approved for public release; \
                       further dissemination unlimited.\" (PDF page 1, checked 2026-10-07; corpus \
                       README section 4). Figure 3.1(b) is reprinted with permission from a third \
                       party and keeps its holder's copyright",
    },
    CorpusLiterature {
        id: "la-ur-06-7094",
        kind: LiteratureKind::Report,
        title: "Monte Carlo Eigenvalue Calculations (LA-UR-06-7094, lecture slides)",
        authors: &["Brown, F."],
        year: Some(2006),
        topics: &[],
        concept_document: Some("la-ur-06-7094"),
        source_url: Some(
            "https://mcnp.lanl.gov/pdf_files/TechReport_2006_LANL_LA-UR-06-7094_Brown.pdf",
        ),
        corpus_file: Some("kovan-standard-open-corpus/us-doe/la-ur-06-7094-brown-mc-eigenvalue.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: DOE_LAB_UNLIMITED_BASIS,
    },
    CorpusLiterature {
        id: "la-ur-09-02377",
        kind: LiteratureKind::Report,
        title: "A Review of Monte Carlo Criticality Calculations - Convergence, Bias, Statistics \
                (LA-UR-09-02377, M&C 2009 slides)",
        authors: &["Brown, F.B."],
        year: Some(2009),
        topics: &[],
        concept_document: Some("la-ur-09-02377"),
        source_url: Some(
            "https://mcnp.lanl.gov/pdf_files/TechReport_2009_LANL_LA-UR-09-02377_Brown.pdf",
        ),
        corpus_file: Some(
            "kovan-standard-open-corpus/us-doe/la-ur-09-02377-brown-mc-criticality-review.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: DOE_LAB_UNLIMITED_BASIS,
    },
    CorpusLiterature {
        id: "kim-2022-fenrg-859622",
        kind: LiteratureKind::Paper,
        title: "An iDTMC-based Monte Carlo depletion of a 3D SMR with intra-pin flux renormalization",
        authors: &["Kim, I.", "Kim, I.", "Kim, Y."],
        year: Some(2022),
        topics: &[],
        concept_document: Some("kim-2022-fenrg-859622"),
        source_url: Some("https://doi.org/10.3389/fenrg.2022.859622"),
        corpus_file: Some("kovan-standard-open-corpus/cc-by/kim2022-fenrg-859622-idtmc-depletion.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: FRONTIERS_CC_BY_BASIS,
    },
    CorpusLiterature {
        id: "zhang-2023-fenrg-1101050",
        kind: LiteratureKind::Paper,
        title: "Parallel Jacobian-free Newton Krylov discrete ordinates method for pin-by-pin \
                neutron transport models",
        authors: &["Zhang, Y.", "Zhou, X."],
        year: Some(2023),
        topics: &[],
        concept_document: Some("zhang-2023-fenrg-1101050"),
        source_url: Some("https://doi.org/10.3389/fenrg.2022.1101050"),
        corpus_file: Some(
            "kovan-standard-open-corpus/cc-by/zhang2023-fenrg-1101050-parallel-jfnk-sn.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: FRONTIERS_CC_BY_BASIS,
    },
    CorpusLiterature {
        id: "arxiv-2306.01924v2",
        kind: LiteratureKind::Paper,
        title: "multiRegionFoam -- A Unified Multiphysics Framework for Multi-Region Coupled \
                Continuum-Physical Problems (arXiv:2306.01924v2)",
        authors: &[
            "Alkafri, H.",
            "Habes, C.",
            "Fadeli, M.E.",
            "Hess, S.",
            "Beale, S.B.",
            "Zhang, S.",
            "Jasak, H.",
            "Marschall, H.",
        ],
        year: Some(2023),
        topics: &[],
        concept_document: Some("arxiv-2306.01924v2"),
        source_url: Some("https://arxiv.org/abs/2306.01924v2"),
        corpus_file: Some("kovan-standard-open-corpus/cc-by-sa/arxiv-2306.01924v2-multiregionfoam.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: "CC BY-SA 4.0, as linked from the arXiv record of v2 \
                       (https://arxiv.org/abs/2306.01924v2, checked 2026-10-07; corpus README \
                       section 10, ground 9)",
    },
    CorpusLiterature {
        id: "arxiv-2301.00289v3",
        kind: LiteratureKind::Paper,
        title: "Stability Analysis of Picard Iteration for Coupled Neutronics/Thermal-Hydraulics \
                Simulations (arXiv:2301.00289v3)",
        authors: &["Wang, D."],
        year: Some(2023),
        topics: &[],
        concept_document: Some("arxiv-2301.00289v3"),
        source_url: Some("https://arxiv.org/abs/2301.00289v3"),
        corpus_file: Some(
            "kovan-standard-open-corpus/cc-by-nc-nd/arxiv-2301.00289v3-wang-picard-stability.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: "CC BY-NC-ND 4.0, as linked from the arXiv record of v3 \
                       (https://arxiv.org/abs/2301.00289v3, checked 2026-10-07; corpus README \
                       section 10, ground 9). Non-commercial, no derivatives",
    },
    CorpusLiterature {
        id: "cosgrove-2020-pc-stability",
        kind: LiteratureKind::Paper,
        title: "Stability analysis of predictor-corrector schemes for coupling neutronics and \
                depletion (accepted manuscript, Annals of Nuclear Energy)",
        authors: &["Cosgrove, P.", "Shwageraus, E.", "Parks, G.T."],
        year: Some(2020),
        topics: &[],
        concept_document: Some("cosgrove-2020-pc-stability"),
        source_url: Some("https://doi.org/10.17863/CAM.57013"),
        corpus_file: Some(
            "kovan-standard-open-corpus/cc-by-nc-nd/cosgrove2020-pc-stability-cam-309913.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: "CC BY-NC-ND 4.0: the University of Cambridge repository record \
                       (https://www.repository.cam.ac.uk/handle/1810/309913, checked 2026-10-07) \
                       states \"this item's license is described as \
                       Attribution-NonCommercial-NoDerivatives 4.0 International\" (corpus README \
                       section 10, ground 9). Non-commercial, no derivatives",
    },
    CorpusLiterature {
        id: "openstax-university-physics-1",
        kind: LiteratureKind::Book,
        title: "University Physics Volume 1 (OpenStax, Rice University)",
        authors: &["Ling, S.J.", "Sanny, J.", "Moebs, W."],
        year: Some(2016),
        topics: &[
            "10-human-resource-development/knowledge-management-and-education/education-and-outreach",
        ],
        concept_document: None,
        source_url: Some("https://openstax.org/details/books/university-physics-volume-1"),
        corpus_file: Some(
            "kovan-standard-open-corpus/cc-by-nc-sa/openstax2016-university-physics-volume-1.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: OPENSTAX_CC_BY_NC_SA_BASIS,
    },
    CorpusLiterature {
        id: "openstax-university-physics-2",
        kind: LiteratureKind::Book,
        title: "University Physics Volume 2 (OpenStax, Rice University)",
        authors: &["Ling, S.J.", "Sanny, J.", "Moebs, W."],
        year: Some(2016),
        topics: &[
            "10-human-resource-development/knowledge-management-and-education/education-and-outreach",
        ],
        concept_document: None,
        source_url: Some("https://openstax.org/details/books/university-physics-volume-2"),
        corpus_file: Some(
            "kovan-standard-open-corpus/cc-by-nc-sa/openstax2016-university-physics-volume-2.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: OPENSTAX_CC_BY_NC_SA_BASIS,
    },
    CorpusLiterature {
        id: "openstax-university-physics-3",
        kind: LiteratureKind::Book,
        title: "University Physics Volume 3 (OpenStax, Rice University)",
        authors: &["Ling, S.J.", "Sanny, J.", "Moebs, W."],
        year: Some(2016),
        topics: &[
            "10-human-resource-development/knowledge-management-and-education/education-and-outreach",
        ],
        concept_document: None,
        source_url: Some("https://openstax.org/details/books/university-physics-volume-3"),
        corpus_file: Some(
            "kovan-standard-open-corpus/cc-by-nc-sa/openstax2016-university-physics-volume-3.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: OPENSTAX_CC_BY_NC_SA_BASIS,
    },
    CorpusLiterature {
        id: "openmc-docs-depletion",
        kind: LiteratureKind::Other,
        title: "OpenMC documentation, Methods: Depletion (depletion.rst, openmc-dev/openmc commit \
                a5bc348)",
        authors: &["OpenMC contributors"],
        year: Some(2026),
        topics: &[],
        concept_document: Some("openmc-docs-depletion"),
        source_url: Some(
            "https://github.com/openmc-dev/openmc/blob/a5bc348a6ca2d2de49c8325dd3cc220e9fc55f83/docs/source/methods/depletion.rst",
        ),
        corpus_file: Some("kovan-standard-open-corpus/openmc-docs/depletion.rst"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: OPENMC_MIT_BASIS,
    },
    CorpusLiterature {
        id: "openmc-docs-eigenvalue",
        kind: LiteratureKind::Other,
        title: "OpenMC documentation, Methods: Eigenvalue Calculations (eigenvalue.rst, \
                openmc-dev/openmc commit a5bc348)",
        authors: &["OpenMC contributors"],
        year: Some(2026),
        topics: &[],
        concept_document: Some("openmc-docs-eigenvalue"),
        source_url: Some(
            "https://github.com/openmc-dev/openmc/blob/a5bc348a6ca2d2de49c8325dd3cc220e9fc55f83/docs/source/methods/eigenvalue.rst",
        ),
        corpus_file: Some("kovan-standard-open-corpus/openmc-docs/eigenvalue.rst"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: OPENMC_MIT_BASIS,
    },
    CorpusLiterature {
        id: "moose-docs-simple",
        kind: LiteratureKind::Other,
        title: "MOOSE Navier-Stokes module documentation: SIMPLE executioner (SIMPLE.md, \
                idaholab/moose commit 21efd28)",
        authors: &["MOOSE contributors (Battelle Energy Alliance, LLC, et al.)"],
        year: Some(2026),
        topics: &[],
        concept_document: Some("moose-docs-simple"),
        source_url: Some(
            "https://github.com/idaholab/moose/blob/21efd282277eb517ee85470371a037b89141e685/modules/navier_stokes/doc/content/source/executioners/SIMPLE.md",
        ),
        corpus_file: Some("kovan-standard-open-corpus/moose-docs/SIMPLE.md"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: MOOSE_LGPL_BASIS,
    },
    CorpusLiterature {
        id: "moose-docs-pimple",
        kind: LiteratureKind::Other,
        title: "MOOSE Navier-Stokes module documentation: PIMPLE executioner (PIMPLE.md, \
                idaholab/moose commit 21efd28)",
        authors: &["MOOSE contributors (Battelle Energy Alliance, LLC, et al.)"],
        year: Some(2026),
        topics: &[],
        concept_document: Some("moose-docs-pimple"),
        source_url: Some(
            "https://github.com/idaholab/moose/blob/21efd282277eb517ee85470371a037b89141e685/modules/navier_stokes/doc/content/source/executioners/PIMPLE.md",
        ),
        corpus_file: Some("kovan-standard-open-corpus/moose-docs/PIMPLE.md"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: MOOSE_LGPL_BASIS,
    },
    // Software-properties sources (#760, 2026-10-07): the 26 redistributable
    // documents cited by the timeless software-properties branch under SQA
    // (corpus README sections 1, 2 and 11 to 15). Metadata from each
    // document's own title page; each is a concept-tree document.
    CorpusLiterature {
        id: "nsa-csi-software-memory-safety",
        kind: LiteratureKind::Report,
        title: "Software Memory Safety (NSA Cybersecurity Information Sheet U/OO/219936-22, \
                Version 1.1)",
        authors: &["U.S. National Security Agency"],
        year: Some(2023),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("nsa-csi-software-memory-safety"),
        source_url: Some(
            "https://media.defense.gov/2022/Nov/10/2003112742/-1/-1/0/CSI_SOFTWARE_MEMORY_SAFETY.PDF",
        ),
        corpus_file: Some(
            "kovan-standard-open-corpus/nsa/nsa-csi-software-memory-safety-v1.1-2023.pdf",
        ),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NSA_SHARE_BASIS,
    },
    CorpusLiterature {
        id: "nsa-cisa-memory-safe-languages-2025",
        kind: LiteratureKind::Report,
        title: "Memory Safe Languages: Reducing Vulnerabilities in Modern Software Development \
                (NSA and CISA Cybersecurity Information Sheet U/OO/172709-25, Version 1.0)",
        authors: &[
            "U.S. National Security Agency",
            "Cybersecurity and Infrastructure Security Agency",
        ],
        year: Some(2025),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("nsa-cisa-memory-safe-languages-2025"),
        source_url: Some(
            "https://media.defense.gov/2025/Jun/23/2003742198/-1/-1/0/CSI_MEMORY_SAFE_LANGUAGES_REDUCING_VULNERABILITIES_IN_MODERN_SOFTWARE_DEVELOPMENT.PDF",
        ),
        corpus_file: Some(
            "kovan-standard-open-corpus/nsa/nsa-cisa-csi-memory-safe-languages-2025.pdf",
        ),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NSA_SHARE_BASIS,
    },
    CorpusLiterature {
        id: "cisa-case-for-memory-safe-roadmaps",
        kind: LiteratureKind::Report,
        title: "The Case for Memory Safe Roadmaps: Why Both C-Suite Executives and Technical \
                Experts Need to Take Memory Safe Coding Seriously",
        authors: &[
            "Cybersecurity and Infrastructure Security Agency",
            "U.S. National Security Agency",
            "Federal Bureau of Investigation",
            "Australian Signals Directorate's Australian Cyber Security Centre",
            "Canadian Centre for Cyber Security",
            "United Kingdom National Cyber Security Centre",
            "New Zealand National Cyber Security Centre",
            "Computer Emergency Response Team New Zealand",
        ],
        year: Some(2023),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("cisa-case-for-memory-safe-roadmaps"),
        source_url: Some(
            "https://www.cisa.gov/sites/default/files/2023-12/The-Case-for-Memory-Safe-Roadmaps-508c.pdf",
        ),
        corpus_file: Some(
            "kovan-standard-open-corpus/cisa/cisa-case-for-memory-safe-roadmaps-2023.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: "TLP:CLEAR: \"Subject to standard copyright rules, TLP:CLEAR information may \
                       be distributed without restriction.\" (PDF page 1, checked 2026-10-07; \
                       corpus README section 12, ground 11, owner's decision on #760). A \
                       disclosure marking, not a licence: five of the eight authoring agencies are \
                       non-U.S. governments, so not all of it is a U.S. Government Work; held \
                       whole and unmodified",
    },
    CorpusLiterature {
        id: "nasa-mco-mib-phase-1",
        kind: LiteratureKind::Report,
        title: "Mars Climate Orbiter Mishap Investigation Board Phase I Report",
        authors: &[
            "Stephenson, A.G.",
            "LaPiana, L.S.",
            "Mulville, D.R.",
            "Rutledge, P.J.",
            "Bauer, F.H.",
            "Folta, D.",
            "Dukeman, G.A.",
            "Sackheim, R.",
            "Norvig, P.",
        ],
        year: Some(1999),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("nasa-mco-mib-phase-1"),
        source_url: Some("https://llis.nasa.gov/llis_lib/pdf/1009464main1_0641-mr.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nasa/mco-mib-phase-i-report-1999.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: "U.S. Government Work (17 U.S.C. 105): every board member on the signature \
                       page (PDF page 3) signs as a NASA employee; no copyright notice (checked \
                       2026-10-07; corpus README section 13, ground 12)",
    },
    CorpusLiterature {
        id: "nasa-std-8739.8b",
        kind: LiteratureKind::Standard,
        title: "NASA-STD-8739.8B, Software Assurance and Software Safety Standard",
        authors: &["National Aeronautics and Space Administration"],
        year: Some(2022),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("nasa-std-8739.8b"),
        source_url: Some("https://standards.nasa.gov/standard/NASA/NASA-STD-87398"),
        corpus_file: Some("kovan-standard-open-corpus/nasa/nasa-std-8739.8b-2022.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: "NASA standard, cover marked \"APPROVED FOR PUBLIC RELEASE – DISTRIBUTION IS \
                       UNLIMITED\" (PDF page 1); NTSS: \"cleared for public accessibility on the \
                       internet\" (checked 2026-10-07; corpus README section 13, ground 12)",
    },
    CorpusLiterature {
        id: "npr-7150.2d",
        kind: LiteratureKind::Standard,
        title: "NPR 7150.2D, NASA Software Engineering Requirements",
        authors: &["National Aeronautics and Space Administration, Office of the Chief Engineer"],
        year: Some(2022),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("npr-7150.2d"),
        source_url: Some(
            "https://nodis3.gsfc.nasa.gov/npg_img/N_PR_7150_002D_/N_PR_7150_002D_.pdf",
        ),
        corpus_file: Some("kovan-standard-open-corpus/nasa/npr-7150.2d-2022.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: "U.S. Government Work (17 U.S.C. 105): a NASA directive issued by NASA and \
                       published in its public directives library (NODIS); no release marking or \
                       copyright notice (checked 2026-10-07; corpus README section 13, ground 12)",
    },
    CorpusLiterature {
        id: "nasa-std-7009b",
        kind: LiteratureKind::Standard,
        title: "NASA-STD-7009B, Standard for Models and Simulations",
        authors: &["National Aeronautics and Space Administration, Office of the Chief Engineer"],
        year: Some(2024),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("nasa-std-7009b"),
        source_url: Some("https://standards.nasa.gov/standard/NASA/NASA-STD-7009"),
        corpus_file: Some("kovan-standard-open-corpus/nasa/nasa-std-7009b-2024.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: "U.S. Government Work (17 U.S.C. 105), a NASA technical standard; the NASA \
                       Technical Standards System page states \"Internet Public -- Standard is \
                       cleared for public accessibility on the internet\" (checked 2026-10-07; \
                       corpus README section 13, ground 12)",
    },
    CorpusLiterature {
        id: "nasa-hdbk-7009b",
        kind: LiteratureKind::Standard,
        title: "NASA-HDBK-7009B, NASA Handbook for Models and Simulations: An Implementation Guide \
                for NASA-STD-7009B",
        authors: &["National Aeronautics and Space Administration, Office of the Chief Engineer"],
        year: Some(2026),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("nasa-hdbk-7009b"),
        source_url: Some("https://standards.nasa.gov/standard/NASA/NASA-HDBK-7009"),
        corpus_file: Some("kovan-standard-open-corpus/nasa/nasa-hdbk-7009b-2026.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: "NASA handbook, cover and every footer marked \"APPROVED FOR PUBLIC \
                       RELEASE—DISTRIBUTION IS UNLIMITED\" (checked 2026-10-07; corpus README \
                       section 13, ground 12). Filed as published, with its \"DRAFT: \
                       NASA-HDBK-7009B\" running header (owner, #760: \"DRAFT is ok\")",
    },
    CorpusLiterature {
        id: "nasa-tm-103863",
        kind: LiteratureKind::Report,
        title: "The NAS Parallel Benchmarks (NASA Technical Memorandum 103863)",
        authors: &["Bailey, D.", "Barton, J.", "Lasinski, T.", "Simon, H."],
        year: Some(1993),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("nasa-tm-103863"),
        source_url: Some("https://ntrs.nasa.gov/citations/19940008727"),
        corpus_file: Some(
            "kovan-standard-open-corpus/nasa/nasa-tm-103863-nas-parallel-benchmarks.pdf",
        ),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NTRS_BASIS,
    },
    CorpusLiterature {
        id: "nasa-tm-2001-210876",
        kind: LiteratureKind::Report,
        title: "A Practical Tutorial on Modified Condition/Decision Coverage (NASA/TM-2001-210876)",
        authors: &["Hayhurst, K.J.", "Veerhusen, D.S.", "Chilenski, J.J.", "Rierson, L.K."],
        year: Some(2001),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("nasa-tm-2001-210876"),
        source_url: Some("https://ntrs.nasa.gov/citations/20010057789"),
        corpus_file: Some("kovan-standard-open-corpus/nasa/nasa-tm-2001-210876-mcdc-tutorial.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NTRS_RTCA_BASIS,
    },
    CorpusLiterature {
        id: "btp-7-14-rev6",
        kind: LiteratureKind::Report,
        title: "Guidance on Software Reviews for Digital Computer-Based Instrumentation and \
                Control Systems (NUREG-0800, SRP Branch Technical Position 7-14, Revision 6)",
        authors: &["U.S. Nuclear Regulatory Commission"],
        year: Some(2016),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("btp-7-14-rev6"),
        source_url: Some("https://www.nrc.gov/docs/ML1601/ML16019A308.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML16019A308.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "rg-1.168-rev2",
        kind: LiteratureKind::Report,
        title: "Verification, Validation, Reviews, and Audits for Digital Computer Software Used \
                in Safety Systems of Nuclear Power Plants (Regulatory Guide 1.168, Revision 2)",
        authors: &["U.S. Nuclear Regulatory Commission, Office of Nuclear Regulatory Research"],
        year: Some(2013),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("rg-1.168-rev2"),
        source_url: Some("https://www.nrc.gov/docs/ML1307/ML13073A210.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML13073A210.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "rg-1.169-rev1",
        kind: LiteratureKind::Report,
        title: "Configuration Management Plans for Digital Computer Software Used in Safety \
                Systems of Nuclear Power Plants (Regulatory Guide 1.169, Revision 1)",
        authors: &["U.S. Nuclear Regulatory Commission, Office of Nuclear Regulatory Research"],
        year: Some(2013),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("rg-1.169-rev1"),
        source_url: Some("https://www.nrc.gov/docs/ML1235/ML12355A642.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML12355A642.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "rg-1.170-rev1",
        kind: LiteratureKind::Report,
        title: "Test Documentation for Digital Computer Software Used in Safety Systems of Nuclear \
                Power Plants (Regulatory Guide 1.170, Revision 1)",
        authors: &["U.S. Nuclear Regulatory Commission, Office of Nuclear Regulatory Research"],
        year: Some(2013),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("rg-1.170-rev1"),
        source_url: Some("https://www.nrc.gov/docs/ML1300/ML13003A216.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML13003A216.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "rg-1.171-rev1",
        kind: LiteratureKind::Report,
        title: "Software Unit Testing for Digital Computer Software Used in Safety Systems of \
                Nuclear Power Plants (Regulatory Guide 1.171, Revision 1)",
        authors: &["U.S. Nuclear Regulatory Commission, Office of Nuclear Regulatory Research"],
        year: Some(2013),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("rg-1.171-rev1"),
        source_url: Some("https://www.nrc.gov/docs/ML1300/ML13004A375.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML13004A375.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "rg-1.172-rev1",
        kind: LiteratureKind::Report,
        title: "Software Requirement Specifications for Digital Computer Software and Complex \
                Electronics Used in Safety Systems of Nuclear Power Plants (Regulatory Guide \
                1.172, Revision 1)",
        authors: &["U.S. Nuclear Regulatory Commission, Office of Nuclear Regulatory Research"],
        year: Some(2013),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("rg-1.172-rev1"),
        source_url: Some("https://www.nrc.gov/docs/ML1300/ML13007A173.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML13007A173.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "rg-1.173-rev1",
        kind: LiteratureKind::Report,
        title: "Developing Software Life-Cycle Processes for Digital Computer Software Used in \
                Safety Systems of Nuclear Power Plants (Regulatory Guide 1.173, Revision 1)",
        authors: &["U.S. Nuclear Regulatory Commission, Office of Nuclear Regulatory Research"],
        year: Some(2013),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("rg-1.173-rev1"),
        source_url: Some("https://www.nrc.gov/docs/ML1300/ML13009A190.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML13009A190.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "rg-1.152-rev4",
        kind: LiteratureKind::Report,
        title: "Criteria for Programmable Digital Devices in Safety-Related Systems of Nuclear \
                Power Plants (Regulatory Guide 1.152, Revision 4)",
        authors: &["U.S. Nuclear Regulatory Commission"],
        year: Some(2023),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("rg-1.152-rev4"),
        source_url: Some("https://www.nrc.gov/docs/ML2305/ML23054A463.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/nrc/ML23054A463.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "faa-ac-20-115d",
        kind: LiteratureKind::Report,
        title: "Airborne Software Development Assurance Using EUROCAE ED-12( ) and RTCA DO-178( ) \
                (FAA Advisory Circular 20-115D)",
        authors: &["Federal Aviation Administration"],
        year: Some(2017),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("faa-ac-20-115d"),
        source_url: Some(
            "https://www.faa.gov/documentLibrary/media/Advisory_Circular/AC_20-115D.pdf",
        ),
        corpus_file: Some("kovan-standard-open-corpus/faa/faa-ac-20-115d-2017.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: FAA_BASIS,
    },
    CorpusLiterature {
        id: "faa-order-8110.49a",
        kind: LiteratureKind::Report,
        title: "Software Approval Guidelines (FAA Order 8110.49A)",
        authors: &["Federal Aviation Administration"],
        year: Some(2018),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("faa-order-8110.49a"),
        source_url: Some("https://www.faa.gov/documentLibrary/media/Order/FAA_Order_8110.49A.pdf"),
        corpus_file: Some("kovan-standard-open-corpus/faa/faa-order-8110.49a-2018.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: FAA_BASIS,
    },
    CorpusLiterature {
        id: "faa-ac-00-69",
        kind: LiteratureKind::Report,
        title: "Best Practices for Airborne Software Development Assurance Using EUROCAE ED-12( ) \
                and RTCA DO-178( ) (FAA Advisory Circular 00-69)",
        authors: &["Federal Aviation Administration"],
        year: Some(2017),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("faa-ac-00-69"),
        source_url: Some(
            "https://www.faa.gov/documentLibrary/media/Advisory_Circular/AC_00-69.pdf",
        ),
        corpus_file: Some("kovan-standard-open-corpus/faa/faa-ac-00-69-2017.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: FAA_BASIS,
    },
    CorpusLiterature {
        id: "faa-ac-20-193",
        kind: LiteratureKind::Report,
        title: "Use of Multi-Core Processors (FAA Advisory Circular 20-193)",
        authors: &["Federal Aviation Administration"],
        year: Some(2024),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("faa-ac-20-193"),
        source_url: Some(
            "https://www.faa.gov/documentLibrary/media/Advisory_Circular/AC_20-193.pdf",
        ),
        corpus_file: Some("kovan-standard-open-corpus/faa/faa-ac-20-193-2024.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: FAA_BASIS,
    },
    CorpusLiterature {
        id: "faa-ac-25.1309-1b",
        kind: LiteratureKind::Report,
        title: "System Design and Analysis (FAA Advisory Circular 25.1309-1B)",
        authors: &["Federal Aviation Administration"],
        year: Some(2024),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("faa-ac-25.1309-1b"),
        source_url: Some(
            "https://www.faa.gov/documentLibrary/media/Advisory_Circular/AC_25.1309-1B.pdf",
        ),
        corpus_file: Some("kovan-standard-open-corpus/faa/faa-ac-25.1309-1b-2024.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: FAA_BASIS,
    },
    CorpusLiterature {
        id: "nistir-8397",
        kind: LiteratureKind::Report,
        title: "Guidelines on Minimum Standards for Developer Verification of Software (NISTIR \
                8397)",
        authors: &["Black, P.E.", "Guttman, B.", "Okun, V."],
        year: Some(2021),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("nistir-8397"),
        source_url: Some("https://doi.org/10.6028/NIST.IR.8397"),
        corpus_file: Some("kovan-standard-open-corpus/nist/nistir-8397-2021.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: "Public domain: \"Pursuant to Title 17, Section 105 of the United States \
                       Code, this is not subject to copyright protection and is in the public \
                       domain.\" (PDF page 4, checked 2026-10-07; corpus README section 15, ground \
                       14)",
    },
    CorpusLiterature {
        id: "arxiv-2505.01671v1",
        kind: LiteratureKind::Paper,
        title: "Report on Challenges of Practical Reproducibility for Systems and HPC Computer \
                Science (arXiv:2505.01671v1)",
        authors: &[
            "Keahey, K.",
            "Richardson, M.",
            "Tolosana Calasanz, R.",
            "Hunold, S.",
            "Lofstead, J.",
            "Malik, T.",
            "Perez, C.",
        ],
        year: Some(2025),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("arxiv-2505.01671v1"),
        source_url: Some("https://arxiv.org/abs/2505.01671v1"),
        corpus_file: Some(
            "kovan-standard-open-corpus/cc-by/arxiv-2505.01671v1-keahey-practical-reproducibility.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: ARXIV_CC_BY_BASIS,
    },
    CorpusLiterature {
        id: "arxiv-2311.06995v1",
        kind: LiteratureKind::Paper,
        title: "Scalable Delivery of Scalable Libraries and Tools: How ECP Delivered a Software \
                Ecosystem for Exascale and Beyond (arXiv:2311.06995v1)",
        authors: &["Heroux, M.A."],
        year: Some(2023),
        topics: &[
            "07-regulatory-framework/quality-assurance/software-quality-assurance/software-properties",
        ],
        concept_document: Some("arxiv-2311.06995v1"),
        source_url: Some("https://arxiv.org/abs/2311.06995v1"),
        corpus_file: Some(
            "kovan-standard-open-corpus/cc-by/arxiv-2311.06995v1-heroux-ecp-software-ecosystem.pdf",
        ),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: ARXIV_CC_BY_BASIS,
    },
    // Private tier: cited only. Their PDFs are in the maintainer's private
    // repository and may not be redistributed; no `corpus_file`, so Kovan
    // never looks for them and needs nothing but this metadata. Metadata
    // from the skeleton's `[[document]]` rows (which were read from the
    // documents when the maintainer supplied them), not re-read here.
    CorpusLiterature {
        id: "kendrick-2019-unqa",
        kind: LiteratureKind::Report,
        title: "Nuclear Quality Assurance in University Research with Control System Design \
                as a Case Study (PhD dissertation, UC Berkeley)",
        authors: &["Kendrick, J.C."],
        year: Some(2019),
        topics: &["07-regulatory-framework/quality-assurance/software-quality-assurance"],
        concept_document: Some("kendrick-2019-unqa"),
        source_url: Some("https://escholarship.org/uc/item/7936z4xq"),
        corpus_file: None,
        status: SourceStatus::Restricted,
        status_basis: "(c) J. C. Kendrick; open download from eScholarship but not for \
                       redistribution; held in the maintainer's private corpus and cited by \
                       section and page only (concept_skeleton.toml, tier \"private\")",
    },
    CorpusLiterature {
        id: "iaea-ng-g-3.1-rev1",
        kind: LiteratureKind::Report,
        title: "Milestones in the Development of a National Infrastructure for Nuclear Power \
                (IAEA Nuclear Energy Series No. NG-G-3.1 (Rev. 1))",
        authors: &["International Atomic Energy Agency"],
        year: Some(2015),
        topics: &[],
        concept_document: Some("iaea-ng-g-3.1-rev1"),
        source_url: None,
        corpus_file: None,
        status: SourceStatus::Restricted,
        status_basis: "(c) IAEA; held in the maintainer's private corpus and cited by section and \
                       page only (concept_skeleton.toml, tier \"private\")",
    },
    CorpusLiterature {
        id: "iaea-nutec-plastics",
        kind: LiteratureKind::Report,
        title: "NUTEC Plastics: Scaling up solutions and partnerships for global impact",
        authors: &["International Atomic Energy Agency"],
        year: Some(2026),
        topics: &[],
        concept_document: Some("iaea-nutec-plastics"),
        source_url: None,
        corpus_file: None,
        status: SourceStatus::Restricted,
        status_basis: "No reuse licence found; held in the maintainer's private corpus and cited \
                       only (concept_skeleton.toml, tier \"private\")",
    },
];

/// Curated corpus-level relationships, each on a document's own evidence
/// ([`CorpusConnection::basis`]). A citation is recorded as
/// [`RelationKind::RelatedTo`]: citing a work shows relevance, not agreement.
pub const CONNECTIONS: &[CorpusConnection] = &[
    CorpusConnection {
        source: "corpus:literature/nureg-2201",
        target: "corpus:literature/wash-1400",
        relation: RelationKind::RelatedTo,
        basis: "NUREG-2201 reference list: \"WASH-1400, (NUREG-75/014), October 1975\"; \
                discussed in its text as the first PRA",
    },
    CorpusConnection {
        source: "corpus:literature/epa-fgr-13",
        target: "corpus:literature/epa-fgr-11",
        relation: RelationKind::RelatedTo,
        basis: "FGR-13 preface: \"Although many of the biokinetic and dosimetric models used \
                here are updates of models used in Federal Guidance Report No. 11, the present \
                report does not replace either that document or Federal Guidance Report No. 12\"",
    },
];

/// A curated connection as typed node ids: [`CONNECTIONS`] parsed, plus one
/// per concept-tree cross-link. Every one is
/// [`ConnectionOrigin::KovanCorpus`]: shown on the map, never editable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CuratedConnection {
    pub source: NodeId,
    pub target: NodeId,
    pub relation: RelationKind,
    /// The evidence: a document's statement for [`CONNECTIONS`], the tree
    /// file for a cross-link.
    pub basis: String,
    pub origin: ConnectionOrigin,
}

/// Every curated connection: [`CONNECTIONS`], then the concept tree's
/// cross-links (`cross_links` in `concept_skeleton.toml` and
/// `concept_proposals.toml`; one home path, a dotted link to each other
/// node a concept belongs to), in tree order. Built once.
pub fn curated_connections() -> &'static [CuratedConnection] {
    static ALL: OnceLock<Vec<CuratedConnection>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut out: Vec<CuratedConnection> = CONNECTIONS
            .iter()
            .filter_map(|c| {
                Some(CuratedConnection {
                    source: NodeId::parse(c.source).ok()?,
                    target: NodeId::parse(c.target).ok()?,
                    relation: c.relation,
                    basis: c.basis.to_string(),
                    origin: ConnectionOrigin::KovanCorpus,
                })
            })
            .collect();
        let tree = concept_tree();
        for n in tree.nodes() {
            for x in tree.cross_links(&n.path) {
                out.push(CuratedConnection {
                    source: NodeId::concept(Namespace::Corpus, &n.path),
                    target: NodeId::concept(Namespace::Corpus, &x.path),
                    relation: RelationKind::RelatedTo,
                    basis: format!(
                        "concept-tree cross-link: {} also belongs under {} \
                         (kovan-literature concept_skeleton.toml / concept_proposals.toml)",
                        n.path, x.path
                    ),
                    origin: ConnectionOrigin::KovanCorpus,
                });
            }
        }
        out
    })
}

/// The curated connections touching `node`, at either end, each with its
/// **other** end: what a map star centred on `node` draws as read-only link
/// cards.
pub fn curated_connections_for(
    node: &NodeId,
) -> Vec<(&'static CuratedConnection, &'static NodeId)> {
    curated_connections()
        .iter()
        .filter_map(|c| {
            if &c.source == node {
                Some((c, &c.target))
            } else if &c.target == node {
                Some((c, &c.source))
            } else {
                None
            }
        })
        .collect()
}

/// The topic at `path`, if any.
pub fn topic_at(path: &str) -> Option<&'static CorpusTopic> {
    let t = built();
    t.by_path.get(path).map(|&i| &t.all[i])
}

/// The direct children of the topic at `path`, in tree order: the 19 IAEA
/// issues for [`ROOT_TOPIC`].
pub fn children_of(path: &str) -> impl Iterator<Item = &'static CorpusTopic> {
    let tree = concept_tree();
    let kids: Vec<&'static CorpusTopic> = if path == ROOT_TOPIC {
        tree.roots().filter_map(|n| topic_at(&n.path)).collect()
    } else {
        tree.children(path)
            .filter_map(|n| topic_at(&n.path))
            .collect()
    };
    kids.into_iter()
}

/// The literature filed under the topic at `path`
/// ([`CorpusLiterature::filed_under`]), in [`LITERATURE`] order.
pub fn literature_in(path: &str) -> impl Iterator<Item = &'static CorpusLiterature> {
    static BY_PATH: OnceLock<HashMap<&'static str, Vec<usize>>> = OnceLock::new();
    let map = BY_PATH.get_or_init(|| {
        let mut m: HashMap<&'static str, Vec<usize>> = HashMap::new();
        for (i, l) in LITERATURE.iter().enumerate() {
            for p in l.filed_under() {
                m.entry(p).or_default().push(i);
            }
        }
        m
    });
    map.get(path)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|i| &LITERATURE[i])
}

/// Whether any literature is filed at `path` or anywhere below it, counting
/// a node's citation of its own sources. The hook a "show empty nodes"
/// toggle reads (#724: "empty is greyed, not hidden"). Because nearly every
/// node cites a standard-corpus document, this is true almost everywhere;
/// [`has_classified_literature`] is the stricter reading.
pub fn has_literature(path: &str) -> bool {
    built().with_literature.contains(path)
}

/// Whether literature is filed **by hand** ([`CorpusLiterature::topics`])
/// at `path` or below: literature *about* the node, as opposed to the
/// regulatory text that defines it. The likelier meaning of "greyed" for
/// the Literature tab; the maintainer has not chosen between the two.
pub fn has_classified_literature(path: &str) -> bool {
    built().with_classified_literature.contains(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The standard map's top level: one virtual root whose children are the
    /// 19 IAEA Milestones issues, in IAEA order (NG-G-3.1 Rev. 1, §3.1–3.19).
    #[test]
    fn the_root_has_the_nineteen_iaea_issues_in_order() {
        let root = topic_at(ROOT_TOPIC).unwrap();
        assert_eq!(root.title, ROOT_TITLE);
        assert_eq!(root.level(), 0);
        assert!(root.parent_path().is_none());
        let issues: Vec<&str> = children_of(ROOT_TOPIC).map(|t| t.path).collect();
        assert_eq!(issues.len(), 19);
        for (i, p) in issues.iter().enumerate() {
            assert!(p.starts_with(&format!("{:02}-", i + 1)), "{p}");
            assert_eq!(topic_at(p).unwrap().parent_path(), Some(ROOT_TOPIC));
            assert_eq!(topic_at(p).unwrap().level(), 1);
        }
        assert_eq!(issues[1], "02-nuclear-safety");
        // Each issue's title carries its number; deeper nodes do not.
        assert_eq!(topic_at("02-nuclear-safety").unwrap().title, "2. Nuclear safety");
        assert_eq!(topic_at("19-procurement").unwrap().title, "19. Procurement");
        assert_eq!(
            topic_at("02-nuclear-safety/nuclear-design").unwrap().title,
            "Nuclear design and core physics"
        );
    }

    /// A known deep path resolves, with its sources and its ancestors.
    #[test]
    fn a_deep_concept_path_resolves_with_its_sources() {
        let p = "02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking/majorant";
        let t = topic_at(p).expect("majorant is in the standard map");
        assert_eq!(t.level(), 5);
        let concept = t.concept.unwrap();
        assert!(!concept.sources.is_empty());
        assert!(!t.deferred());
        let mut at = t.parent_path();
        let mut chain = Vec::new();
        while let Some(path) = at {
            chain.push(path);
            at = topic_at(path).unwrap().parent_path();
        }
        assert_eq!(
            chain,
            [
                "02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking",
                "02-nuclear-safety/nuclear-design/neutron-transport",
                "02-nuclear-safety/nuclear-design",
                "02-nuclear-safety",
                ROOT_TOPIC,
            ]
        );
        assert!(
            children_of("02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking")
                .any(|c| c.path == p)
        );
    }

    /// The map is exactly levels 1–3 of the tree plus the root; the old
    /// `nuclear-engineering/...` topics are gone from the corpus namespace.
    #[test]
    fn the_map_is_the_concept_tree_and_the_old_topics_are_gone() {
        assert_eq!(topics().len(), concept_tree().nodes().len() + 1);
        assert!(topic_at("nuclear-engineering").is_none());
        assert!(topic_at("nuclear-engineering/reactor-systems/htgr").is_none());
        assert!(topics()
            .iter()
            .all(|t| t.path == ROOT_TOPIC || t.concept.is_some()));
        // Deferred concepts are present, flagged.
        assert!(topics().iter().any(|t| t.deferred()));
        // The root id is no concept path and no possible user slug.
        assert!(concept_tree().node(ROOT_TOPIC).is_none());
        assert!(ROOT_TOPIC.contains('_'));
    }

    /// Every path is unique and a valid node id; every topic but the root
    /// has a parent that exists and comes earlier.
    #[test]
    fn the_taxonomy_is_a_well_formed_tree() {
        let mut seen = HashSet::new();
        for t in topics() {
            assert!(seen.insert(t.path), "duplicate topic {}", t.path);
            assert!(
                NodeId::parse(&t.id().to_string()).is_ok(),
                "{} is not a valid id",
                t.path
            );
            match t.parent_path() {
                None => assert_eq!(t.path, ROOT_TOPIC, "only the root has no parent"),
                Some(parent) => assert!(
                    seen.contains(parent),
                    "{}'s parent {parent} is missing or listed after it",
                    t.path
                ),
            }
        }
    }

    /// Literature is filed only under topics that exist, has a unique id,
    /// every concept-tree document is an entry, and corpus connections join
    /// nodes that exist.
    #[test]
    fn literature_and_connections_point_at_real_nodes() {
        assert_eq!(
            LITERATURE.len(),
            // ~~66~~ ~~92 since 2026-10-07: the 26 software-properties sources (#760).~~
            // ~~98 since 2026-10-10: DOE-HDBK-1017, 1015 and 1014, two volumes each (#829).~~
            // 101 since 2026-10-10: OpenStax University Physics Volumes 1-3, CC BY-NC-SA
            // (#829; College Physics 2e not filed, its PDF is over GitHub's 100 MB limit).
            101,
            "the maintainer's 2026-09-22 set, EPA FGR-11, FGR-13 and FGR-15 (2026-09-28), \
             the concept tree's 13 further documents (2026-10-06), and the software QA \
             set NUREG/BR-0167, DOE-STD-1172-2003 and DOE G 414.1-4 (2026-10-07, #760), Kendrick 2019 \
             (private, cited only), the NJOY2016 manual (2026-10-07, #760), the \
             22 IAEA-issue bedrock documents and 12 solver-pattern sources (2026-10-07, #760), \
             the 26 software-properties sources (2026-10-07, #760), and the DOE Material \
             Science, Chemistry and Mathematics handbooks (2026-10-10, #829), and OpenStax \
             University Physics Volumes 1-3 (2026-10-10, #829)"
        );
        let mut ids = HashSet::new();
        for l in LITERATURE {
            assert!(
                NodeId::parse(&l.id().to_string()).is_ok(),
                "{} is not a valid id",
                l.id
            );
            assert!(!l.status_basis.is_empty(), "{} has no licence basis", l.id);
            // Only a redistributable document may be held in the corpus
            // repository, and every one of these is.
            assert_eq!(
                l.corpus_file.is_some(),
                l.status.redistributable(),
                "{}: corpus_file must be set exactly when redistributable",
                l.id
            );
            assert!(ids.insert(l.id), "duplicate literature id {}", l.id);
            assert!(
                !l.filed_under().is_empty(),
                "{} is filed under no topic",
                l.id
            );
            assert!(!l.classification_topics().is_empty(), "{}", l.id);
            for t in l.filed_under() {
                assert!(
                    topic_at(t).is_some(),
                    "{} is filed under missing topic {t}",
                    l.id
                );
            }
        }
        // Every tree document is an entry; standard-tier ones carry their
        // corpus file, private-tier ones none (citation only).
        for d in concept_tree().documents() {
            let l = LITERATURE
                .iter()
                .find(|l| l.concept_document == Some(d.id.as_str()))
                .unwrap_or_else(|| panic!("tree document {} has no LITERATURE entry", d.id));
            match d.tier {
                kovan_literature::DocumentTier::Standard => {
                    assert_eq!(l.corpus_file, Some(d.file.as_str()), "{}", d.id)
                }
                kovan_literature::DocumentTier::Private => {
                    assert!(
                        l.corpus_file.is_none(),
                        "{}: private tier is cited only",
                        d.id
                    )
                }
            }
        }
        let exists = |id: &NodeId| {
            assert_eq!(id.namespace, Namespace::Corpus, "{id} is not a corpus node");
            topics().iter().any(|t| &t.id() == id) || LITERATURE.iter().any(|l| &l.id() == id)
        };
        for c in CONNECTIONS {
            assert!(
                !c.basis.is_empty(),
                "{} -> {} has no evidence",
                c.source,
                c.target
            );
        }
        assert!(curated_connections().len() > CONNECTIONS.len());
        for c in curated_connections() {
            assert!(exists(&c.source), "missing source {}", c.source);
            assert!(exists(&c.target), "missing target {}", c.target);
            assert!(!c.origin.user_editable());
        }
    }

    /// A node's sources are among its citations, and the hand filing is too.
    #[test]
    fn a_node_cites_its_sources_and_its_hand_filed_literature() {
        let pra = "02-nuclear-safety/severe-accidents/probabilistic-risk-assessment";
        let ids: Vec<&str> = literature_in(pra).map(|l| l.id).collect();
        assert!(ids.contains(&"wash-1400"), "{ids:?}");
        assert!(ids.contains(&"nureg-2201"), "{ids:?}");
        let fuel = "02-nuclear-safety/fuel-system-design";
        let ids: Vec<&str> = literature_in(fuel).map(|l| l.id).collect();
        assert!(
            ids.contains(&"nureg-0800-toc-rev6"),
            "a node's source: {ids:?}"
        );
        assert!(ids.contains(&"nureg-0800-4.2"), "{ids:?}");
        assert!(has_literature(fuel) && has_literature(ROOT_TOPIC));
        assert!(has_classified_literature("02-nuclear-safety"));
        // ~~19-procurement~~ until 2026-10-07, when RG 1.164 Rev. 1 was
        // hand-filed under 19-procurement/supplier-quality-and-specifications
        // (#760); issue 4 still has no hand-filed literature.
        assert!(has_classified_literature("19-procurement"));
        assert!(!has_classified_literature("04-funding-and-financing"));
        // The IAEA Milestones are cited (only) by the level-1 issues.
        let l1: Vec<&str> = literature_in("19-procurement").map(|l| l.id).collect();
        assert_eq!(l1, ["iaea-ng-g-3.1-rev1"]);
        // A document cited by dozens of nodes classifies a user's notes
        // paper under its level-1 issues only.
        let srp = LITERATURE
            .iter()
            .find(|l| l.id == "nureg-0800-toc-rev6")
            .unwrap();
        assert!(srp.filed_under().len() > 20);
        assert!(srp.classification_topics().iter().all(|p| !p.contains('/')));
    }

    /// The cross-links are curated, read-only connections, seen from both
    /// ends.
    #[test]
    fn cross_links_are_curated_connections_from_both_ends() {
        let st = NodeId::concept(Namespace::Corpus, "02-nuclear-safety/source-terms");
        let env = NodeId::concept(Namespace::Corpus, "13-environmental-protection");
        let from = curated_connections_for(&st);
        assert!(from
            .iter()
            .any(|(c, other)| **other == env && c.origin == ConnectionOrigin::KovanCorpus));
        assert!(curated_connections_for(&env)
            .iter()
            .any(|(_, other)| **other == st));
    }

    #[test]
    fn linked_topics_take_the_ontology_aliases() {
        let transport = topic_at("02-nuclear-safety/nuclear-design/neutron-transport").unwrap();
        assert_eq!(transport.ontology.unwrap().id(), "neutron-transport");
        assert!(transport.aliases().contains(&"Neutron Transport"));
        let msr = topic_at("02-nuclear-safety/nuclear-design/core-configurations/liquid-fuelled")
            .unwrap();
        assert!(msr.aliases().contains(&"MSR"));
        for (p, _) in ONTOLOGY_LINKS {
            assert!(topic_at(p).is_some(), "{p}");
        }
        assert!(topic_at("02-nuclear-safety/severe-accidents")
            .unwrap()
            .aliases()
            .is_empty());
    }

    #[test]
    fn only_verified_sources_are_redistributable_and_only_user_connections_editable() {
        assert!(SourceStatus::VerifiedPublicDomain.redistributable());
        assert!(SourceStatus::VerifiedOpenLicence.redistributable());
        assert!(!SourceStatus::PubliclyAccessibleUnverified.redistributable());
        assert!(!SourceStatus::Restricted.redistributable());
        assert!(ConnectionOrigin::User.user_editable());
        assert!(!ConnectionOrigin::KovanCorpus.user_editable());
    }
}
