//! The built-in nuclear-engineering corpus (GitHub issue #248, epic #247).
//!
//! What belongs here: the curated knowledge Kovan ships with, compiled into
//! the binary so the mind map always has a nuclear-engineering backbone, with
//! no Kovan folder open, offline, and with no PDFs (maintainer brief,
//! 2026-09-22). That is:
//!
//! - [`TOPICS`]: the topic hierarchy, as data. The types are not specific to
//!   this taxonomy; extending it means adding rows.
//! - [`LITERATURE`]: curated literature identities and metadata, supplied by
//!   the maintainer (#250, 2026-09-22) and read from the documents
//!   themselves; nothing bibliographic is invented here. The PDFs are in
//!   [`CORPUS_REPOSITORY_URL`].
//! - [`CONNECTIONS`]: curated relationships between corpus nodes, carrying
//!   [`ConnectionOrigin::KovanCorpus`] so the GUI can refuse to edit them.
//!
//! What does not belong here: PDFs (never shipped in the crate; they live in
//! a library's `literature/kovan-open-corpus/`), user annotations, and user
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

use crate::node_id::{Namespace, NodeId};
use crate::relation::RelationKind;
use kovan_semantics::{CoreConcept, Neutronics, Reactor, ThermalHydraulics};

/// A corpus topic: one node of the browsing hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorpusTopic {
    /// Slash-separated path, parent first; the last segment is this topic's
    /// slug. The parent is the path with its last segment removed.
    pub path: &'static str,
    /// Display name.
    pub title: &'static str,
    /// The ontology concept this topic is, if it is one.
    pub ontology: Option<OntologyLink>,
}

impl CorpusTopic {
    /// This topic's node id.
    pub fn id(&self) -> NodeId {
        NodeId::concept(Namespace::Corpus, self.path)
    }

    /// The parent topic's path, or `None` for the root.
    pub fn parent_path(&self) -> Option<&'static str> {
        self.path.rsplit_once('/').map(|(p, _)| p)
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
    /// CC BY, CC BY-SA, ...).
    VerifiedOpenLicence,
    /// Freely readable, but redistribution has not been verified. The entry
    /// and its URL ship; the PDF does not.
    PubliclyAccessibleUnverified,
    /// Restricted or local-only.
    Restricted,
}

impl SourceStatus {
    /// Whether the PDF may be placed in `literature/kovan-open-corpus/`.
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
    /// Paths of the [`TOPICS`] this entry is filed under. It appears as a
    /// citation of each.
    pub topics: &'static [&'static str],
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
}

/// Shorthand for a topic with no ontology link.
const fn topic(path: &'static str, title: &'static str) -> CorpusTopic {
    CorpusTopic {
        path,
        title,
        ontology: None,
    }
}

/// Shorthand for a topic that is an ontology concept.
const fn linked(path: &'static str, title: &'static str, link: OntologyLink) -> CorpusTopic {
    CorpusTopic {
        path,
        title,
        ontology: Some(link),
    }
}

/// The root topic's path.
pub const ROOT_TOPIC: &str = "nuclear-engineering";

/// The Git repository holding the corpus PDFs (maintainer direction,
/// 2026-09-22), under `kovan-open-corpus/`, with a README recording each
/// document's licence basis. Kovan may clone it into a library's
/// `literature/kovan-open-corpus/`; the map never depends on it (#253).
pub const CORPUS_REPOSITORY_URL: &str = "https://github.com/theodoreOnzGit/reactor-literature.git";

/// The branch of [`CORPUS_REPOSITORY_URL`] to use.
pub const CORPUS_REPOSITORY_BRANCH: &str = "main";

/// The initial nuclear-engineering hierarchy (maintainer brief, 2026-09-22).
/// Parents come before children. Extend by adding rows.
pub const TOPICS: &[CorpusTopic] = &[
    topic("nuclear-engineering", "Nuclear Engineering"),
    topic("nuclear-engineering/reactor-physics", "Reactor Physics"),
    linked(
        "nuclear-engineering/reactor-physics/transport",
        "Transport",
        OntologyLink::Neutronics(Neutronics::Transport),
    ),
    linked(
        "nuclear-engineering/reactor-physics/diffusion",
        "Diffusion",
        OntologyLink::Neutronics(Neutronics::Diffusion),
    ),
    topic(
        "nuclear-engineering/reactor-physics/monte-carlo",
        "Monte Carlo",
    ),
    topic(
        "nuclear-engineering/reactor-physics/reactor-kinetics",
        "Reactor Kinetics",
    ),
    topic("nuclear-engineering/nuclear-data", "Nuclear Data"),
    topic(
        "nuclear-engineering/thermal-hydraulics",
        "Thermal Hydraulics",
    ),
    topic(
        "nuclear-engineering/thermal-hydraulics/single-phase-flow",
        "Single-Phase Flow",
    ),
    topic(
        "nuclear-engineering/thermal-hydraulics/two-phase-flow",
        "Two-Phase Flow",
    ),
    topic(
        "nuclear-engineering/thermal-hydraulics/critical-heat-flux",
        "Critical Heat Flux",
    ),
    linked(
        "nuclear-engineering/thermal-hydraulics/natural-circulation",
        "Natural Circulation",
        OntologyLink::ThermalHydraulics(ThermalHydraulics::NaturalCirculation),
    ),
    topic("nuclear-engineering/reactor-systems", "Reactor Systems"),
    topic("nuclear-engineering/reactor-systems/lwr", "LWR"),
    topic("nuclear-engineering/reactor-systems/lwr/pwr", "PWR"),
    topic("nuclear-engineering/reactor-systems/lwr/bwr", "BWR"),
    linked(
        "nuclear-engineering/reactor-systems/htgr",
        "HTGR",
        OntologyLink::Reactor(Reactor::Htgr),
    ),
    topic(
        "nuclear-engineering/reactor-systems/htgr/pebble-bed",
        "Pebble Bed",
    ),
    topic(
        "nuclear-engineering/reactor-systems/htgr/prismatic",
        "Prismatic",
    ),
    topic("nuclear-engineering/reactor-systems/sfr", "SFR"),
    linked(
        "nuclear-engineering/reactor-systems/fhr",
        "FHR",
        OntologyLink::Reactor(Reactor::Fhr),
    ),
    linked(
        "nuclear-engineering/reactor-systems/msr",
        "MSR",
        OntologyLink::Reactor(Reactor::Msr),
    ),
    topic(
        "nuclear-engineering/reactor-systems/research-reactors",
        "Research Reactors",
    ),
    topic("nuclear-engineering/fuel-and-materials", "Fuel & Materials"),
    topic("nuclear-engineering/fuel-and-materials/uo2", "UO2"),
    topic("nuclear-engineering/fuel-and-materials/triso", "TRISO"),
    topic(
        "nuclear-engineering/fuel-and-materials/metallic-fuel",
        "Metallic Fuel",
    ),
    topic(
        "nuclear-engineering/fuel-and-materials/graphite",
        "Graphite",
    ),
    topic(
        "nuclear-engineering/fuel-and-materials/cladding",
        "Cladding",
    ),
    topic(
        "nuclear-engineering/fuel-and-materials/structural-materials",
        "Structural Materials",
    ),
    topic("nuclear-engineering/safety", "Safety"),
    topic(
        "nuclear-engineering/safety/accident-analysis",
        "Accident Analysis",
    ),
    topic(
        "nuclear-engineering/safety/severe-accidents",
        "Severe Accidents",
    ),
    topic(
        "nuclear-engineering/safety/criticality-safety",
        "Criticality Safety",
    ),
    topic(
        "nuclear-engineering/safety/defense-in-depth",
        "Defense in Depth",
    ),
    topic("nuclear-engineering/safety/source-term", "Source Term"),
    topic("nuclear-engineering/pra", "PRA"),
    topic("nuclear-engineering/fuel-cycle", "Fuel Cycle"),
    topic(
        "nuclear-engineering/scientific-computing",
        "Scientific Computing",
    ),
    topic(
        "nuclear-engineering/scientific-computing/verification",
        "Verification",
    ),
    topic(
        "nuclear-engineering/scientific-computing/validation",
        "Validation",
    ),
];

/// The basis for every U.S. NRC entry: the NRC Site Disclaimer
/// (<https://www.nrc.gov/about-nrc/site-disclaimer>, accessed 2026-09-22),
/// quoted verbatim in the corpus repository's README.
const NRC_BASIS: &str = "U.S. Government Work, not subject to copyright: NRC Site Disclaimer \
(https://www.nrc.gov/about-nrc/site-disclaimer); 17 U.S.C. 105";

/// The basis for the PHYSOR 2026 papers.
const PHYSOR_2026_BASIS: &str =
    "CC BY 4.0, as recorded on the paper's Zenodo DOI record (checked 2026-09-22)";

/// Curated literature (#250), supplied by the maintainer on 2026-09-22 and
/// held in [`CORPUS_REPOSITORY_URL`]. Titles, authors and years are read
/// from each document's own title and front-matter pages; topics from its
/// abstract and contents. No corpus connection is listed between them: none
/// cites another entry here (NUREG-2201 and NUREG/KM-0006 cite NUREG-0800,
/// but chapters 19.2 and 15.0.2, not the Section 4.2 held here).
pub const LITERATURE: &[CorpusLiterature] = &[
    CorpusLiterature {
        id: "nureg-0800-4.2",
        kind: LiteratureKind::Report,
        title: "Standard Review Plan, Section 4.2: Fuel System Design (NUREG-0800, Revision 3)",
        authors: &["U.S. Nuclear Regulatory Commission"],
        year: Some(2007),
        topics: &[
            "nuclear-engineering/fuel-and-materials",
            "nuclear-engineering/fuel-and-materials/cladding",
            "nuclear-engineering/reactor-systems/lwr",
            "nuclear-engineering/safety",
        ],
        source_url: Some("https://www.nrc.gov/docs/ML0707/ML070740002.pdf"),
        corpus_file: Some("kovan-open-corpus/nrc/ML070740002.pdf"),
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
            "nuclear-engineering/fuel-and-materials/cladding",
            "nuclear-engineering/safety/accident-analysis",
        ],
        source_url: Some("https://www.nrc.gov/docs/ML1302/ML13028A421.pdf"),
        corpus_file: Some("kovan-open-corpus/nrc/ML13028A421.pdf"),
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
            "nuclear-engineering/scientific-computing",
            "nuclear-engineering/scientific-computing/verification",
            "nuclear-engineering/scientific-computing/validation",
        ],
        source_url: Some("https://www.nrc.gov/docs/ML1332/ML13325A086.pdf"),
        corpus_file: Some("kovan-open-corpus/nrc/ML13325A086.pdf"),
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
            "nuclear-engineering/pra",
        ],
        source_url: Some("https://www.nrc.gov/docs/ML1624/ML16245A032.pdf"),
        corpus_file: Some("kovan-open-corpus/nrc/ML16245A032.pdf"),
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
            "nuclear-engineering/reactor-physics/transport",
            "nuclear-engineering/reactor-systems/lwr",
            "nuclear-engineering/scientific-computing",
        ],
        source_url: Some("https://www.nrc.gov/docs/ML1233/ML12338A215.pdf"),
        corpus_file: Some("kovan-open-corpus/nrc/ML12338A215.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "nureg-cr-7289",
        kind: LiteratureKind::Report,
        title: "Nuclear Data Assessment for Advanced Reactors (NUREG/CR-7289)",
        authors: &["Bostelmann, F.", "Ilas, G.", "Celik, C.", "Holcomb, A.M.", "Wieselquist, W.A."],
        year: Some(2022),
        topics: &[
            "nuclear-engineering/nuclear-data",
            "nuclear-engineering/reactor-systems/htgr",
            "nuclear-engineering/reactor-systems/msr",
            "nuclear-engineering/reactor-systems/sfr",
        ],
        source_url: Some("https://www.nrc.gov/docs/ML2206/ML22063A060.pdf"),
        corpus_file: Some("kovan-open-corpus/nrc/ML22063A060.pdf"),
        status: SourceStatus::VerifiedPublicDomain,
        status_basis: NRC_BASIS,
    },
    CorpusLiterature {
        id: "ong2024tuas",
        kind: LiteratureKind::Paper,
        title: "An open-source Thermo-hydraulic Uniphase Advection and Convection Solver for Salt Flows (TUAS)",
        authors: &["Ong, T.K.C.", "Xiao, S.", "Peterson, P.F."],
        year: Some(2024),
        topics: &[
            "nuclear-engineering/thermal-hydraulics/single-phase-flow",
            "nuclear-engineering/thermal-hydraulics/natural-circulation",
            "nuclear-engineering/reactor-systems/fhr",
            "nuclear-engineering/scientific-computing/verification",
            "nuclear-engineering/scientific-computing/validation",
        ],
        source_url: Some("https://doi.org/10.1016/j.jandt.2025.03.006"),
        corpus_file: Some("kovan-open-corpus/ong/tuas-theodore-ong.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: "CC BY 4.0, stated in the article itself; authored by the corpus maintainer",
    },
    CorpusLiterature {
        id: "hori2026physor",
        kind: LiteratureKind::Paper,
        title: "Burnup Calculation Using POD-Based Neutron Spectrum Reconstruction: Application to High-Temperature Gas-Cooled Reactor Core Analysis",
        authors: &["Hori, T.", "Chiba, G."],
        year: Some(2026),
        topics: &[
            "nuclear-engineering/reactor-physics",
            "nuclear-engineering/reactor-systems/htgr/prismatic",
        ],
        source_url: Some("https://doi.org/10.5281/zenodo.20803716"),
        corpus_file: Some("kovan-open-corpus/physor-2026/physor2026-206-hori-pod-burnup-httr.pdf"),
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
            "nuclear-engineering/reactor-physics/monte-carlo",
            "nuclear-engineering/reactor-physics/reactor-kinetics",
            "nuclear-engineering/reactor-systems/research-reactors",
        ],
        source_url: Some("https://doi.org/10.5281/zenodo.20804104"),
        corpus_file: Some("kovan-open-corpus/physor-2026/physor2026-306-bures-subcritical-simulator.pdf"),
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
            "nuclear-engineering/reactor-systems/sfr",
            "nuclear-engineering/thermal-hydraulics/single-phase-flow",
            "nuclear-engineering/reactor-physics/monte-carlo",
            "nuclear-engineering/safety/accident-analysis",
        ],
        source_url: Some("https://doi.org/10.5281/zenodo.20803769"),
        corpus_file: Some("kovan-open-corpus/physor-2026/physor2026-343-acierno-hexana-sfr.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: PHYSOR_2026_BASIS,
    },
    CorpusLiterature {
        id: "krpan2026physor",
        kind: LiteratureKind::Paper,
        title: "A peek into the MSRE, six decades later: a hyper-fidelity simulation of the classical molten salt reactor",
        authors: &["Krpan, R.", "Fiorina, C.", "Clarno, K.", "Genoni, C.", "Gentry, C.A.", "Park, S.M.", "Ragusa, J."],
        year: Some(2026),
        topics: &[
            "nuclear-engineering/reactor-systems/msr",
            "nuclear-engineering/reactor-physics/monte-carlo",
            "nuclear-engineering/thermal-hydraulics",
            "nuclear-engineering/scientific-computing/validation",
        ],
        source_url: Some("https://doi.org/10.5281/zenodo.20803785"),
        corpus_file: Some("kovan-open-corpus/physor-2026/physor2026-449-krpan-msre-hyper-fidelity.pdf"),
        status: SourceStatus::VerifiedOpenLicence,
        status_basis: PHYSOR_2026_BASIS,
    },
];

/// Curated corpus-level relationships. Empty: no corpus entry cites another
/// (see [`LITERATURE`]). A connection is added only where a document states
/// the relationship.
pub const CONNECTIONS: &[CorpusConnection] = &[];

/// The topic at `path`, if any.
pub fn topic_at(path: &str) -> Option<&'static CorpusTopic> {
    TOPICS.iter().find(|t| t.path == path)
}

/// The direct children of the topic at `path`, in table order.
pub fn children_of(path: &str) -> impl Iterator<Item = &'static CorpusTopic> + '_ {
    TOPICS.iter().filter(move |t| t.parent_path() == Some(path))
}

/// The literature filed under the topic at `path`.
pub fn literature_in(path: &str) -> impl Iterator<Item = &'static CorpusLiterature> + '_ {
    LITERATURE.iter().filter(move |l| l.topics.contains(&path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The acceptance test's top level: Nuclear Engineering and its nine
    /// branches, in the brief's order.
    #[test]
    fn the_root_has_the_nine_branches() {
        assert_eq!(topic_at(ROOT_TOPIC).unwrap().title, "Nuclear Engineering");
        let branches: Vec<&str> = children_of(ROOT_TOPIC).map(|t| t.title).collect();
        assert_eq!(
            branches,
            [
                "Reactor Physics",
                "Nuclear Data",
                "Thermal Hydraulics",
                "Reactor Systems",
                "Fuel & Materials",
                "Safety",
                "PRA",
                "Fuel Cycle",
                "Scientific Computing",
            ]
        );
    }

    /// Every path is unique and a valid node id, every topic but the root
    /// has a parent that exists and comes earlier, and everything descends
    /// from the root.
    #[test]
    fn the_taxonomy_is_a_well_formed_tree() {
        let mut seen = HashSet::new();
        for t in TOPICS {
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
            assert!(t.path.starts_with(ROOT_TOPIC));
        }
    }

    /// Literature is filed only under topics that exist, has a unique id,
    /// and corpus connections join nodes that exist.
    #[test]
    fn literature_and_connections_point_at_real_nodes() {
        assert_eq!(LITERATURE.len(), 11, "the maintainer's 2026-09-22 set");
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
            assert!(!l.topics.is_empty(), "{} is filed under no topic", l.id);
            for t in l.topics {
                assert!(
                    topic_at(t).is_some(),
                    "{} is filed under missing topic {t}",
                    l.id
                );
            }
        }
        let exists = |s: &str| {
            let id = NodeId::parse(s).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(id.namespace, Namespace::Corpus, "{s} is not a corpus node");
            TOPICS.iter().any(|t| t.id() == id) || LITERATURE.iter().any(|l| l.id() == id)
        };
        for c in CONNECTIONS {
            assert!(exists(c.source), "missing source {}", c.source);
            assert!(exists(c.target), "missing target {}", c.target);
        }
    }

    #[test]
    fn linked_topics_take_the_ontology_aliases() {
        let htgr = topic_at("nuclear-engineering/reactor-systems/htgr").unwrap();
        assert_eq!(htgr.ontology.unwrap().id(), "htgr");
        assert!(htgr.aliases().contains(&"HTGR"));
        assert!(topic_at("nuclear-engineering/pra")
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
