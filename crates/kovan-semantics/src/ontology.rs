//! A typed graph of **scientific/engineering concepts** and the semantic
//! relationships between them — reactor types, numerical methods,
//! thermal-hydraulic approximations, the software that implements them, and
//! the literature that supports or benchmarks them. This is a different
//! layer from [`crate::extract`]'s *code* symbols: a [`Concept`] is a
//! domain idea ("Boussinesq Approximation"), not a `fn`/`struct`.
//!
//! Grew out of a pair of design-experiment prototypes (Python + Rust
//! sketches, not shipped here) that explored whether a small typed
//! relationship vocabulary could usefully connect a reactor-design corpus
//! informally drawn from an Ong dissertation, IAEA TECDOC-1694, and a TUAS
//! paper. This module's own `tests::sample_ontology` is that same kind of
//! corpus, kept as a **test fixture proving the abstraction works** — see
//! its own doc comment for why it is deliberately not shipped as public
//! API.
//!
//! ## Curated core vs. everything else
//!
//! Two tiers, distinguished by [`Origin`]:
//!
//! - **[`Origin::Core`]** — a small, hand-reviewed set of foundational
//!   concepts, each a variant of a [`CoreConcept`]-implementing enum (e.g.
//!   [`Reactor`], [`Neutronics`], [`ThermalHydraulics`]). Added with
//!   [`ConceptGraph::add_core`]. This is compile-time vocabulary: adding a
//!   reactor *type* is a Rust change reviewed like any other.
//! - **[`Origin::User`] / [`Origin::Literature`]** — everything else
//!   (a specific plant model, a benchmark case, a software tool, a document)
//!   is a runtime concept added with [`ConceptGraph::add_user_concept`] /
//!   [`ConceptGraph::add_literature_concept`]. A [`ConceptGraph`] can relate
//!   these freely to the compiled core — e.g. `"pbmr400"` specialises
//!   nothing in the core, but `"htgr"` (core) can be the target of an edge
//!   from a runtime concept — **without ever mutating the core enums**.
//!   [`ConceptGraph::add_core`] and the `add_*_concept` methods reject a
//!   duplicate ID rather than silently overwriting, so a runtime concept can
//!   never shadow or replace a core one.
//!
//! Aliases (`"HTGR"`, `"GCR"`, `"TH"`, ...) are **concept metadata**, stored
//! on the [`Concept`] itself and read by [`ConceptGraph::resolve`] — never
//! modelled as graph edges. A `SpecializationOf`-shaped "alias" edge would
//! conflate "this is another name for the same thing" with "this is a more
//! specific thing," which are different claims.
//!
//! ## Relationships carry more than a label when they need to
//!
//! [`Relation`] names *what kind* of connection exists; most edges need
//! nothing more (`EdgeDetail::None`). Two relation families do need real
//! structure, and get it rather than a flattened `Vec<String>`:
//!
//! - The approximation family (`ApproximationOf`, `Simplifies`,
//!   `ReducedFrom`, `SurrogateOf`, `Represents`) can carry an
//!   [`Applicability`] — the stated assumptions and the regime/validity
//!   range they hold in.
//! - The verification/validation family (`BenchmarkOf`, `VerifiedAgainst`,
//!   `ValidatedAgainst`, `ComparedWith`) can carry a [`VerificationRecord`]
//!   — the benchmark definition, the measured result (with its uncertainty,
//!   where known), and the stated acceptance criterion — mirroring this
//!   workspace's own V&V documentation rule (`CLAUDE.md`, "Verification &
//!   validation documentation": methodology *and* results, not just prose).
//!
//! Every [`ConceptEdge`] also carries a [`RelationStatus`]: `Established`
//! for a settled claim, `Provisional` for one recorded but not yet vetted
//! (e.g. read off a single source, not cross-checked) — so a renderer can
//! visually distinguish confidence rather than presenting every edge with
//! equal weight.
//!
//! ## Scope of this pass
//!
//! This module is deliberately self-contained: no dependency on `kovan`
//! (the GUI/mindmap crate), and no file-format ingestion yet (loading user
//! concepts from Kovan TOML/Markdown is real, separate work — the API here
//! ([`ConceptGraph::add_user_concept`], [`ConceptGraph::relate_with`]) is
//! shaped so that ingestion can call straight into it once written). It is
//! meant to be independently usable by autocomplete, the literature layer,
//! `kovan-codegen`, and a future graph-visualisation front end alike —
//! deterministic and offline, with no AI/heuristic matching anywhere in
//! [`ConceptGraph::resolve`].

use std::collections::BTreeMap;
use std::fmt;

/// A typed semantic relationship between two [`Concept`]s in a
/// [`ConceptGraph`] — the edge label, always read `source <relation>
/// target` (e.g. `htgr SpecializationOf gas-cooled-reactor`). Each variant's
/// Rustdoc is the precise definition a caller reasons about;
/// [`Relation::label`] is the separate, shorter string a GUI shows a
/// scientist who has never seen a Rust identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Relation {
    /// `source` is a more specific category or form of `target` — a
    /// taxonomic is-a edge (e.g. HTGR specialises Gas-Cooled Reactor).
    SpecializationOf,
    /// `source` is constructed or obtained from `target` — a general
    /// derivation. Prefer `ApproximationOf` or `ReducedFrom` instead when
    /// the derivation is specifically an approximation or a reduction; use
    /// this one when neither fits more precisely.
    DerivedFrom,
    /// `source` approximates `target` under stated physical/mathematical
    /// assumptions. Attach an [`Applicability`] via
    /// [`EdgeDetail::Applicability`] recording those assumptions and the
    /// regime they hold in, rather than leaving them implicit.
    ApproximationOf,
    /// `source` deliberately removes detail from `target` for tractability,
    /// without necessarily being a formal mathematical approximation of it
    /// (e.g. a lumped-parameter model simplifying a spatially resolved
    /// one).
    Simplifies,
    /// `source` is a reduced-order representation constructed from
    /// `target` (e.g. a surrogate/ROM built by projecting or fitting
    /// `target`'s behaviour).
    ReducedFrom,
    /// `source` reproduces selected input-output behaviour of `target`
    /// without asserting the same underlying physics (e.g. a
    /// transfer-function surrogate of a coupled multiphysics model).
    SurrogateOf,
    /// `source` stands for or models `target` in a given context, without
    /// asserting full equivalence (e.g. an experimental facility
    /// representing a reactor design class).
    Represents,
    /// `source` depends on parameter or property `target`.
    ParameterizedBy,
    /// `source` exchanges physical or model state with `target`. Recorded
    /// directionally even though the coupling itself is often mutual — add
    /// the reverse edge too when both directions matter.
    CoupledWith,
    /// `source` employs `target` as a component or method. The catch-all:
    /// use a more specific relation when one applies, and this one when
    /// none does.
    Employs,
    /// `source` (a continuous governing equation or model) is discretised
    /// using numerical formulation `target` (e.g. conservation of energy
    /// discretised by the finite-volume method).
    DiscretizedBy,
    /// `source` is numerically solved using algorithm/solver `target`.
    SolvedBy,
    /// `source` produces data consumed elsewhere in the graph (e.g. a
    /// Monte Carlo transport code generating multigroup cross sections).
    GeneratesData,
    /// `source` (typically a fitted or reduced model) is identified/fitted
    /// from dataset or process `target`.
    IdentifiedFrom,
    /// `source` (a model, method, or algorithm) is implemented by
    /// software/code `target`.
    ImplementedBy,
    /// `source` is a benchmark problem or model defined against reference
    /// case `target`. Attach a [`VerificationRecord`] via
    /// [`EdgeDetail::Verification`] with the benchmark definition, rather
    /// than leaving it as prose.
    BenchmarkOf,
    /// `source` has been verified against reference/analytical solution
    /// `target` — "implemented correctly?" in this workspace's V&V sense.
    /// Attach a [`VerificationRecord`].
    VerifiedAgainst,
    /// `source` has been validated against experimental/reference data
    /// `target` — "represents physical reality well enough for its
    /// intended purpose?" in this workspace's V&V sense. Attach a
    /// [`VerificationRecord`].
    ValidatedAgainst,
    /// `source` has been compared with `target` without necessarily being
    /// a formal verification/validation exercise (e.g. two correlations
    /// compared over a shared range). Attach a [`VerificationRecord`] when
    /// the comparison has a stated result.
    ComparedWith,
    /// `source`'s claim or relationship is supported by literature/evidence
    /// source `target` — typically a [`Concept`] with
    /// [`Origin::Literature`].
    SupportedBy,
    /// `source` contradicts or is in tension with `target`. Recorded, not
    /// resolved — both may remain in the graph until a human review
    /// reconciles them; do not delete one side to make the graph
    /// "consistent."
    Contradicts,
}

impl Relation {
    /// A short, GUI-facing label — what a scientist reads in a mindmap edge
    /// tooltip, not the Rust identifier. Deliberately distinct from
    /// [`fmt::Debug`]: the enum's variant names follow Rust naming
    /// conventions (`ApproximationOf`); this reads as plain English
    /// (`"approximates"`).
    pub const fn label(self) -> &'static str {
        match self {
            Self::SpecializationOf => "is a specialised form of",
            Self::DerivedFrom => "is derived from",
            Self::ApproximationOf => "approximates",
            Self::Simplifies => "simplifies",
            Self::ReducedFrom => "is reduced from",
            Self::SurrogateOf => "is a surrogate for",
            Self::Represents => "represents",
            Self::ParameterizedBy => "is parameterised by",
            Self::CoupledWith => "is coupled with",
            Self::Employs => "employs",
            Self::DiscretizedBy => "is discretised by",
            Self::SolvedBy => "is solved by",
            Self::GeneratesData => "generates data for",
            Self::IdentifiedFrom => "is identified from",
            Self::ImplementedBy => "is implemented by",
            Self::BenchmarkOf => "is a benchmark of",
            Self::VerifiedAgainst => "is verified against",
            Self::ValidatedAgainst => "is validated against",
            Self::ComparedWith => "is compared with",
            Self::SupportedBy => "is supported by",
            Self::Contradicts => "contradicts",
        }
    }

    /// Whether this relation's meaning is enriched by an [`Applicability`]
    /// (assumptions + validity range) — the approximation/reduction family.
    /// Advisory only: [`ConceptGraph::relate_with`] accepts any
    /// [`EdgeDetail`] on any relation, this just names the family
    /// [`Relation::label`]'s own doc comments point at.
    pub const fn is_approximation_family(self) -> bool {
        matches!(
            self,
            Self::ApproximationOf
                | Self::Simplifies
                | Self::ReducedFrom
                | Self::SurrogateOf
                | Self::Represents
        )
    }

    /// Whether this relation's meaning is enriched by a
    /// [`VerificationRecord`] — the verification/validation/benchmark
    /// family. Advisory only, see [`Relation::is_approximation_family`].
    pub const fn is_verification_family(self) -> bool {
        matches!(
            self,
            Self::BenchmarkOf | Self::VerifiedAgainst | Self::ValidatedAgainst | Self::ComparedWith
        )
    }
}

/// How settled a [`ConceptEdge`] is. Independent of [`Origin`] — a
/// core-to-core edge can be provisional (a hypothesis not yet
/// cross-checked), and a literature-derived edge can be established (a
/// long-settled result simply being ingested from a new source).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationStatus {
    /// Asserted with full confidence — the normal case for curated,
    /// reviewed edges.
    Established,
    /// Recorded but not yet vetted or cross-checked (e.g. read from a
    /// single source during ingestion). A renderer should visually
    /// distinguish this from `Established` rather than presenting both
    /// with equal weight.
    Provisional,
}

/// Where a [`Concept`] or [`ConceptEdge`] came from — the tier distinction
/// the module doc describes. Never mutated after insertion: a concept's
/// origin is fixed at the point it enters a [`ConceptGraph`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// A compile-time [`CoreConcept`] variant — curated, reviewed, part of
    /// this crate's shipped vocabulary.
    Core,
    /// Added at runtime, extending a graph beyond the compiled core (e.g. a
    /// specific plant model, a software tool, a numerical method not
    /// foundational enough to be a `CoreConcept`).
    User,
    /// Derived from literature during ingestion (e.g. a document node a
    /// `SupportedBy` edge points at, or a relation extracted from a paper's
    /// stated assumptions). Concepts of this origin are natural candidates
    /// for [`RelationStatus::Provisional`] edges, though nothing enforces
    /// that pairing.
    Literature,
}

/// The stated assumptions and validity range under which an approximation,
/// simplification, reduction, surrogate, or representation holds — attached
/// to a [`ConceptEdge`] via [`EdgeDetail::Applicability`] rather than
/// flattened into a single string. Both fields are plain descriptive text
/// (this crate is a semantic/documentation layer, not a numerics engine —
/// it does not carry `uom`-typed quantities the way `tampines`/`tuas` do).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applicability {
    /// The physical/mathematical assumptions under which the relationship
    /// holds (e.g. `"density variation small except in the buoyancy
    /// term"`).
    pub assumptions: Vec<String>,
    /// The regime or range in which those assumptions are valid (e.g.
    /// `"single-phase, low Mach-number natural-circulation flow"`).
    pub validity_ranges: Vec<String>,
}

impl Applicability {
    /// An empty applicability record — build it up with
    /// [`Applicability::with_assumption`] / [`Applicability::with_validity_range`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one stated assumption.
    pub fn with_assumption(mut self, assumption: impl Into<String>) -> Self {
        self.assumptions.push(assumption.into());
        self
    }

    /// Append one stated validity range.
    pub fn with_validity_range(mut self, range: impl Into<String>) -> Self {
        self.validity_ranges.push(range.into());
        self
    }
}

/// A verification/validation/benchmark record — mirrors this workspace's
/// own V&V documentation rule (`CLAUDE.md`, "Verification & validation
/// documentation"): methodology (`benchmark`) *and* results (`result`,
/// with uncertainty stated inline where known), not just prose. Attached to
/// a [`ConceptEdge`] via [`EdgeDetail::Verification`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationRecord {
    /// What is being verified/validated/benchmarked/compared against what,
    /// and the reference case (e.g. `"HTR-10 initial criticality
    /// benchmark"`, `"IAEA TECDOC-1694 benchmark problem 3"`).
    pub benchmark: String,
    /// The measured/reported result, stated with its uncertainty where
    /// known (e.g. `"k_eff = 1.00234 +/- 0.00015"`). `None` when the record
    /// only establishes that the comparison/benchmark exists, not yet its
    /// outcome.
    pub result: Option<String>,
    /// The stated pass/acceptance criterion (e.g. `"within 500 pcm of the
    /// reference k_eff"`). `None` for a plain comparison with no
    /// pass/fail criterion (typical of `Relation::ComparedWith`).
    pub acceptance_criterion: Option<String>,
}

impl VerificationRecord {
    /// A record naming only the benchmark/reference case — add
    /// [`VerificationRecord::with_result`] / [`VerificationRecord::with_acceptance_criterion`]
    /// once those are known.
    pub fn new(benchmark: impl Into<String>) -> Self {
        Self {
            benchmark: benchmark.into(),
            result: None,
            acceptance_criterion: None,
        }
    }

    /// Attach the measured/reported result (with its uncertainty, where
    /// known).
    pub fn with_result(mut self, result: impl Into<String>) -> Self {
        self.result = Some(result.into());
        self
    }

    /// Attach the stated pass/acceptance criterion.
    pub fn with_acceptance_criterion(mut self, criterion: impl Into<String>) -> Self {
        self.acceptance_criterion = Some(criterion.into());
        self
    }
}

/// The structured payload a [`ConceptEdge`] carries beyond its
/// [`Relation`] — enum-dispatched (per this workspace's Rust design rules:
/// no trait objects for a closed set) rather than a generic `Vec<String>`
/// that would flatten an [`Applicability`] or a [`VerificationRecord`] into
/// unstructured prose.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum EdgeDetail {
    /// No structured detail beyond the relation type itself — the common
    /// case (e.g. most `SpecializationOf`/`Employs`/`CoupledWith` edges).
    #[default]
    None,
    /// See [`Relation::is_approximation_family`].
    Applicability(Applicability),
    /// See [`Relation::is_verification_family`].
    Verification(VerificationRecord),
}

/// A compile-time-known, curated scientific concept — the "fundamental"
/// tier the module doc distinguishes from user- and literature-derived
/// concepts (see [`Origin`]). Implemented by small, hand-reviewed `Copy`
/// enums (e.g. [`Reactor`], [`Neutronics`], [`ThermalHydraulics`]), each
/// grouping one domain's foundational vocabulary — not an exhaustive
/// catalogue of every concept in that domain; specific plant models,
/// benchmark cases, and software tools are runtime [`Origin::User`] /
/// [`Origin::Literature`] concepts instead (see the module doc).
///
/// `id` is an **explicit string, matched per variant** — never derived from
/// the variant's Rust name (e.g. via `{:?}`/[`fmt::Debug`]). That keeps a
/// concept's identity, as seen by a stored graph, a serialised edge, or a
/// GUI referencing it by string, independent of Rust identifier choices:
/// renaming a variant is then a pure refactor that cannot silently change
/// what a persisted reference resolves to.
pub trait CoreConcept: Copy {
    /// The stable semantic ID (kebab-case by convention, e.g. `"htgr"`).
    /// See the trait doc for why this is not derived from the variant name.
    fn id(self) -> &'static str;
    /// The full human-readable name (e.g. `"High-Temperature Gas-Cooled
    /// Reactor"`).
    fn name(self) -> &'static str;
    /// Case/punctuation-insensitive alternate names this concept resolves
    /// under via [`ConceptGraph::resolve`] (e.g. `["HTGR", "HTGRs"]`) —
    /// concept *metadata*, never modelled as graph edges (see the module
    /// doc).
    fn aliases(self) -> &'static [&'static str];
}

/// Reactor-type taxonomy — a small, hand-reviewed is-a hierarchy of nuclear
/// reactor classes. Foundational vocabulary, not an exhaustive reactor
/// catalogue: specific plant/benchmark models (PBMR-400, HTR-10, GT-MHR,
/// ...) are runtime concepts added to a [`ConceptGraph`], not variants
/// here — see the module doc's "Curated core vs. everything else".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reactor {
    /// The root of the taxonomy — any fission reactor.
    NuclearReactor,
    /// A reactor cooled by a gas (helium, CO2, ...) rather than a liquid.
    GasCooledReactor,
    /// High-Temperature Gas-Cooled Reactor — a [`Reactor::GasCooledReactor`]
    /// operating at high coolant outlet temperature, typically
    /// graphite-moderated and helium-cooled.
    Htgr,
    /// Very-High-Temperature Reactor — an [`Reactor::Htgr`] pushed to even
    /// higher outlet temperature for process-heat applications.
    Vhtr,
    /// Fluoride-Salt-Cooled High-Temperature Reactor — solid (typically
    /// TRISO) fuel cooled by a molten fluoride salt rather than a gas.
    Fhr,
    /// Molten Salt Reactor — fuel dissolved or suspended in a molten salt
    /// coolant (a circulating-fuel design), rather than solid fuel.
    Msr,
}

impl CoreConcept for Reactor {
    fn id(self) -> &'static str {
        match self {
            Self::NuclearReactor => "nuclear-reactor",
            Self::GasCooledReactor => "gas-cooled-reactor",
            Self::Htgr => "htgr",
            Self::Vhtr => "vhtr",
            Self::Fhr => "fhr",
            Self::Msr => "msr",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::NuclearReactor => "Nuclear Reactor",
            Self::GasCooledReactor => "Gas-Cooled Reactor",
            Self::Htgr => "High-Temperature Gas-Cooled Reactor",
            Self::Vhtr => "Very-High-Temperature Reactor",
            Self::Fhr => "Fluoride-Salt-Cooled High-Temperature Reactor",
            Self::Msr => "Molten Salt Reactor",
        }
    }

    fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::NuclearReactor => &[],
            Self::GasCooledReactor => &["GCR"],
            Self::Htgr => &["HTGR", "HTGRs"],
            Self::Vhtr => &["VHTR"],
            Self::Fhr => &["FHR"],
            Self::Msr => &["MSR"],
        }
    }
}

/// Neutron population modelling — the transport equation and its diffusion
/// approximation. Foundational vocabulary only; specific solvers/codes are
/// runtime concepts (see [`Reactor`]'s doc for the same point).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Neutronics {
    /// The neutron transport equation — angular-flux-resolved neutron
    /// balance.
    Transport,
    /// The neutron diffusion equation — [`Neutronics::Transport`]'s
    /// scalar-flux approximation, valid in a diffusion-dominated regime
    /// (see [`Relation::ApproximationOf`]).
    Diffusion,
}

impl CoreConcept for Neutronics {
    fn id(self) -> &'static str {
        match self {
            Self::Transport => "neutron-transport",
            Self::Diffusion => "neutron-diffusion",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Transport => "Neutron Transport",
            Self::Diffusion => "Neutron Diffusion",
        }
    }

    fn aliases(self) -> &'static [&'static str] {
        &[]
    }
}

/// Thermal-hydraulic conservation laws and the natural-circulation /
/// Boussinesq vocabulary this workspace's own `tuas_boussinesq_solver`
/// crate implements. Foundational vocabulary only; see [`Reactor`]'s doc
/// for the same "not an exhaustive catalogue" point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalHydraulics {
    /// Conservation of mass (continuity).
    ConservationMass,
    /// Conservation of momentum.
    ConservationMomentum,
    /// Conservation of energy.
    ConservationEnergy,
    /// Buoyancy-driven flow with no forced circulation.
    NaturalCirculation,
    /// The Boussinesq approximation — treats density as constant except in
    /// the buoyancy term of the momentum equation (see
    /// [`Relation::ApproximationOf`]).
    Boussinesq,
}

impl CoreConcept for ThermalHydraulics {
    fn id(self) -> &'static str {
        match self {
            Self::ConservationMass => "conservation-mass",
            Self::ConservationMomentum => "conservation-momentum",
            Self::ConservationEnergy => "conservation-energy",
            Self::NaturalCirculation => "natural-circulation",
            Self::Boussinesq => "boussinesq",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::ConservationMass => "Conservation of Mass",
            Self::ConservationMomentum => "Conservation of Momentum",
            Self::ConservationEnergy => "Conservation of Energy",
            Self::NaturalCirculation => "Natural Circulation",
            Self::Boussinesq => "Boussinesq Approximation",
        }
    }

    fn aliases(self) -> &'static [&'static str] {
        &[]
    }
}

/// One node in a [`ConceptGraph`] — a scientific/engineering idea, owned
/// (not borrowed) so both compiled [`CoreConcept`]s and runtime-added
/// concepts share the same representation once inserted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Concept {
    /// The stable semantic ID other concepts/edges reference it by.
    pub id: String,
    /// The full human-readable name.
    pub name: String,
    /// Alternate names [`ConceptGraph::resolve`] matches, case/punctuation-
    /// insensitively. Metadata on the concept, never a graph edge.
    pub aliases: Vec<String>,
    /// Where this concept came from — see [`Origin`].
    pub origin: Origin,
}

/// One edge in a [`ConceptGraph`] — always directional (`source
/// <relation> target`; see [`ConceptGraph::outgoing`] /
/// [`ConceptGraph::incoming`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptEdge {
    /// The source concept's ID.
    pub source: String,
    /// The relationship type.
    pub relation: Relation,
    /// The target concept's ID.
    pub target: String,
    /// How settled this edge is — see [`RelationStatus`].
    pub status: RelationStatus,
    /// Structured detail beyond the relation type — see [`EdgeDetail`].
    pub detail: EdgeDetail,
}

/// What [`ConceptGraph::resolve`] found for a query. Distinguishes "no
/// match" from "more than one match" rather than collapsing both to
/// `None`, since a caller (e.g. an autocomplete popup) typically wants to
/// react differently — offer nothing vs. offer a disambiguation list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveOutcome<'a> {
    /// Exactly one concept's ID, name, or alias normalises to the query.
    Found(&'a Concept),
    /// More than one concept matched — e.g. two concepts sharing an alias.
    /// Carries every match, in [`ConceptGraph`]'s own (ID-sorted, so
    /// deterministic) iteration order.
    Ambiguous(Vec<&'a Concept>),
    /// No concept's ID, name, or alias normalises to the query.
    NotFound,
}

impl<'a> ResolveOutcome<'a> {
    /// The single matched concept, or `None` for [`ResolveOutcome::Ambiguous`]
    /// / [`ResolveOutcome::NotFound`] alike — the shape the earlier design
    /// prototypes' `resolve` returned, kept here as a convenience for a
    /// caller that doesn't care to distinguish "ambiguous" from "not
    /// found".
    pub fn single(&self) -> Option<&'a Concept> {
        match self {
            Self::Found(c) => Some(c),
            _ => None,
        }
    }
}

/// Errors from mutating a [`ConceptGraph`]. Never from [`ConceptGraph::resolve`]
/// (see [`ResolveOutcome`]) — resolution failure is a normal outcome, not
/// an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OntologyError {
    /// [`ConceptGraph::relate`] / [`ConceptGraph::relate_with`] referenced a
    /// concept ID not present in the graph.
    UnknownConcept(String),
    /// [`ConceptGraph::add_user_concept`] / [`ConceptGraph::add_literature_concept`]
    /// was given an ID that already exists — including a
    /// [`Origin::Core`] one. Returned rather than silently overwriting, so
    /// a runtime concept can never shadow or mutate the compiled core (see
    /// the module doc).
    DuplicateConcept(String),
}

impl fmt::Display for OntologyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownConcept(id) => write!(f, "unknown concept: {id:?}"),
            Self::DuplicateConcept(id) => write!(f, "concept already exists: {id:?}"),
        }
    }
}

impl std::error::Error for OntologyError {}

/// A graph of [`Concept`]s connected by [`ConceptEdge`]s. Concepts are
/// stored in a [`BTreeMap`] (not a hash map) so every iteration order —
/// [`ConceptGraph::concepts`], and therefore [`ResolveOutcome::Ambiguous`]'s
/// match list — is deterministic by concept ID, matching this crate's
/// offline/deterministic charter (see the module doc).
#[derive(Debug, Clone, Default)]
pub struct ConceptGraph {
    concepts: BTreeMap<String, Concept>,
    edges: Vec<ConceptEdge>,
}

impl ConceptGraph {
    /// An empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a compiled [`CoreConcept`] as an [`Origin::Core`] concept.
    ///
    /// A collision with an existing ID is a **programming error** (two
    /// `CoreConcept` variants sharing an ID, or a core concept added
    /// twice) rather than possible user input, so this panics instead of
    /// returning a `Result` — matching how the compiled core is meant to
    /// be assembled once, at startup, by code the author controls.
    /// Runtime-sourced insertions ([`ConceptGraph::add_user_concept`],
    /// [`ConceptGraph::add_literature_concept`]) return `Result` instead,
    /// because *their* input is not under the author's control.
    pub fn add_core<C: CoreConcept>(&mut self, concept: C) {
        let id = concept.id().to_string();
        let prior = self.concepts.insert(
            id.clone(),
            Concept {
                id,
                name: concept.name().to_string(),
                aliases: concept.aliases().iter().map(|s| (*s).to_string()).collect(),
                origin: Origin::Core,
            },
        );
        assert!(
            prior.is_none(),
            "duplicate core concept id (compile-time bug): {:?}",
            concept.id()
        );
    }

    /// Add a runtime, [`Origin::User`] concept. Fails on a duplicate ID
    /// (including a core one) — see [`OntologyError::DuplicateConcept`].
    pub fn add_user_concept(
        &mut self,
        id: impl Into<String>,
        name: impl Into<String>,
        aliases: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<(), OntologyError> {
        self.add_concept(id, name, aliases, Origin::User)
    }

    /// Add a runtime, [`Origin::Literature`] concept (typically a document
    /// a `SupportedBy`/`Contradicts` edge points at). Fails on a duplicate
    /// ID (including a core one) — see [`OntologyError::DuplicateConcept`].
    pub fn add_literature_concept(
        &mut self,
        id: impl Into<String>,
        name: impl Into<String>,
        aliases: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<(), OntologyError> {
        self.add_concept(id, name, aliases, Origin::Literature)
    }

    fn add_concept(
        &mut self,
        id: impl Into<String>,
        name: impl Into<String>,
        aliases: impl IntoIterator<Item = impl Into<String>>,
        origin: Origin,
    ) -> Result<(), OntologyError> {
        let id = id.into();
        if self.concepts.contains_key(&id) {
            return Err(OntologyError::DuplicateConcept(id));
        }
        self.concepts.insert(
            id.clone(),
            Concept {
                id,
                name: name.into(),
                aliases: aliases.into_iter().map(Into::into).collect(),
                origin,
            },
        );
        Ok(())
    }

    /// Add a plain edge: [`RelationStatus::Established`],
    /// [`EdgeDetail::None`]. Use [`ConceptGraph::relate_with`] for a
    /// provisional edge or one carrying an [`Applicability`] /
    /// [`VerificationRecord`].
    pub fn relate(
        &mut self,
        source: &str,
        relation: Relation,
        target: &str,
    ) -> Result<(), OntologyError> {
        self.relate_with(
            source,
            relation,
            target,
            RelationStatus::Established,
            EdgeDetail::None,
        )
    }

    /// Add an edge with an explicit [`RelationStatus`] and [`EdgeDetail`].
    /// Fails if `source` or `target` is not already a concept in this
    /// graph — an edge can never dangle.
    pub fn relate_with(
        &mut self,
        source: &str,
        relation: Relation,
        target: &str,
        status: RelationStatus,
        detail: EdgeDetail,
    ) -> Result<(), OntologyError> {
        if !self.concepts.contains_key(source) {
            return Err(OntologyError::UnknownConcept(source.to_string()));
        }
        if !self.concepts.contains_key(target) {
            return Err(OntologyError::UnknownConcept(target.to_string()));
        }
        self.edges.push(ConceptEdge {
            source: source.to_string(),
            relation,
            target: target.to_string(),
            status,
            detail,
        });
        Ok(())
    }

    /// Normalise a query for alias/name/ID matching: lowercase, alphanumeric
    /// characters only. Deterministic string processing — no fuzzy scoring,
    /// no AI, matching this crate's offline charter.
    fn normalize(s: &str) -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    }

    /// Resolve a query string against every concept's ID, name, and
    /// aliases, case/punctuation-insensitively (e.g. `"htgr"`, `"HTGR"`,
    /// and `"H T G R"` all normalise the same way). See [`ResolveOutcome`]
    /// for how "no match" and "more than one match" are distinguished.
    pub fn resolve(&self, query: &str) -> ResolveOutcome<'_> {
        let needle = Self::normalize(query);
        let matches: Vec<&Concept> = self
            .concepts
            .values()
            .filter(|c| {
                std::iter::once(c.id.as_str())
                    .chain(std::iter::once(c.name.as_str()))
                    .chain(c.aliases.iter().map(String::as_str))
                    .any(|candidate| Self::normalize(candidate) == needle)
            })
            .collect();
        match matches.len() {
            0 => ResolveOutcome::NotFound,
            1 => ResolveOutcome::Found(matches[0]),
            _ => ResolveOutcome::Ambiguous(matches),
        }
    }

    /// One concept by its exact ID (no normalisation — use
    /// [`ConceptGraph::resolve`] for alias/name matching).
    pub fn concept(&self, id: &str) -> Option<&Concept> {
        self.concepts.get(id)
    }

    /// Every concept, in ID-sorted (deterministic) order.
    pub fn concepts(&self) -> impl Iterator<Item = &Concept> {
        self.concepts.values()
    }

    /// Every edge, in insertion order.
    pub fn edges(&self) -> &[ConceptEdge] {
        &self.edges
    }

    /// Edges whose `source` is `id`, optionally filtered to one
    /// [`Relation`]. Directional: this never includes an edge whose
    /// `target` is `id` — see [`ConceptGraph::incoming`] for the reverse.
    pub fn outgoing(&self, id: &str, relation: Option<Relation>) -> Vec<&ConceptEdge> {
        self.edges
            .iter()
            .filter(|e| e.source == id && relation.map_or(true, |r| e.relation == r))
            .collect()
    }

    /// Edges whose `target` is `id`, optionally filtered to one
    /// [`Relation`] — the reverse of [`ConceptGraph::outgoing`].
    pub fn incoming(&self, id: &str, relation: Option<Relation>) -> Vec<&ConceptEdge> {
        self.edges
            .iter()
            .filter(|e| e.target == id && relation.map_or(true, |r| e.relation == r))
            .collect()
    }

    /// How many concepts this graph holds.
    pub fn len(&self) -> usize {
        self.concepts.len()
    }

    /// Whether this graph holds no concepts.
    pub fn is_empty(&self) -> bool {
        self.concepts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A worked example spanning all three [`CoreConcept`] domains plus a
    /// realistic spread of runtime concepts (specific plant/benchmark
    /// models, software, numerical methods, and three literature nodes) —
    /// informally in the spirit of an Ong dissertation / IAEA TECDOC-1694 /
    /// TUAS-paper reactor-design corpus that motivated this module, but
    /// **not** an extraction from those documents: no specific numeric
    /// result (a k_eff, a benchmark tolerance, ...) is asserted anywhere
    /// below, exactly because this crate has not ingested them and
    /// [`RESPONSIBLE_USE.md`]/this workspace's "never fabricate or
    /// overclaim" rule forbid inventing one. This exists to exercise the
    /// abstraction end-to-end (multi-domain concepts, every `Origin`,
    /// `EdgeDetail` variety, a provisional edge, alias resolution) — it is
    /// a test fixture, not a shipped ontology, which is why it lives here
    /// rather than behind a `pub fn` at the crate root.
    ///
    /// [`RESPONSIBLE_USE.md`]: https://github.com/theodoreOnzGit/outram-park-backend/blob/develop/RESPONSIBLE_USE.md
    fn sample_ontology() -> ConceptGraph {
        let mut g = ConceptGraph::new();

        for c in [
            Reactor::NuclearReactor,
            Reactor::GasCooledReactor,
            Reactor::Htgr,
            Reactor::Vhtr,
            Reactor::Fhr,
            Reactor::Msr,
        ] {
            g.add_core(c);
        }
        for c in [Neutronics::Transport, Neutronics::Diffusion] {
            g.add_core(c);
        }
        for c in [
            ThermalHydraulics::ConservationMass,
            ThermalHydraulics::ConservationMomentum,
            ThermalHydraulics::ConservationEnergy,
            ThermalHydraulics::NaturalCirculation,
            ThermalHydraulics::Boussinesq,
        ] {
            g.add_core(c);
        }

        let user_concepts: &[(&str, &str, &[&str])] = &[
            ("pbmr400", "PBMR-400", &[]),
            ("htr10", "HTR-10", &[]),
            ("gt-mhr", "Gas Turbine Modular Helium Reactor", &["GT-MHR"]),
            ("thermal-hydraulics", "Thermal Hydraulics", &["TH"]),
            ("neutronics", "Neutronics", &[]),
            ("multiphysics", "Multiphysics", &[]),
            ("variable-density-flow", "Variable-Density Flow", &[]),
            ("heat-conduction", "Heat Conduction", &[]),
            ("finite-volume", "Finite-Volume Method", &["FVM"]),
            ("first-order-upwind", "First-Order Upwind", &[]),
            ("brent-dekker", "Brent-Dekker Root Finding", &[]),
            ("lapack", "LAPACK", &[]),
            ("openmc", "OpenMC", &[]),
            ("genfoam", "GeN-Foam", &[]),
            ("multigroup-xs", "Multigroup Cross Sections", &["MGXS"]),
            ("frequency-response", "Frequency-Response Dataset", &[]),
            ("transfer-function", "Transfer-Function Surrogate", &[]),
            ("pid", "PID Controller", &["PID"]),
            ("ciet", "Compact Integral Effects Test Facility", &["CIET"]),
            ("pbmr-benchmark", "PBMR-400 Benchmark Model", &[]),
            ("htr10-benchmark", "HTR-10 Benchmark", &[]),
            ("astra", "ASTRA Critical Facility", &["ASTRA"]),
        ];
        for (id, name, aliases) in user_concepts {
            g.add_user_concept(*id, *name, aliases.iter().copied())
                .unwrap();
        }

        for (id, name) in [
            ("ong2024", "Ong 2024 Dissertation"),
            ("iaea1694", "IAEA TECDOC-1694"),
            ("tuas", "TUAS Thermal-Hydraulics Paper"),
        ] {
            g.add_literature_concept(id, name, std::iter::empty::<&str>())
                .unwrap();
        }

        g.relate(
            "gas-cooled-reactor",
            Relation::SpecializationOf,
            "nuclear-reactor",
        )
        .unwrap();
        g.relate("htgr", Relation::SpecializationOf, "gas-cooled-reactor")
            .unwrap();
        g.relate("vhtr", Relation::SpecializationOf, "htgr")
            .unwrap();
        g.relate("fhr", Relation::SpecializationOf, "nuclear-reactor")
            .unwrap();
        g.relate("msr", Relation::SpecializationOf, "nuclear-reactor")
            .unwrap();

        g.relate_with(
            "boussinesq",
            Relation::ApproximationOf,
            "variable-density-flow",
            RelationStatus::Established,
            EdgeDetail::Applicability(
                Applicability::new()
                    .with_assumption("density variation small except in the buoyancy term")
                    .with_validity_range("single-phase, low Mach-number natural-circulation flow"),
            ),
        )
        .unwrap();
        g.relate_with(
            "neutron-diffusion",
            Relation::ApproximationOf,
            "neutron-transport",
            RelationStatus::Established,
            EdgeDetail::Applicability(
                Applicability::new()
                    .with_assumption("diffusion regime: weakly absorbing, optically thick medium"),
            ),
        )
        .unwrap();

        g.relate("multiphysics", Relation::CoupledWith, "thermal-hydraulics")
            .unwrap();
        g.relate("multiphysics", Relation::CoupledWith, "neutronics")
            .unwrap();

        g.relate("openmc", Relation::GeneratesData, "multigroup-xs")
            .unwrap();
        g.relate("genfoam", Relation::Employs, "multigroup-xs")
            .unwrap();
        g.relate("genfoam", Relation::GeneratesData, "frequency-response")
            .unwrap();
        g.relate(
            "transfer-function",
            Relation::IdentifiedFrom,
            "frequency-response",
        )
        .unwrap();
        g.relate_with(
            "transfer-function",
            Relation::SurrogateOf,
            "multiphysics",
            RelationStatus::Provisional,
            EdgeDetail::None,
        )
        .unwrap();
        g.relate("pid", Relation::Employs, "transfer-function")
            .unwrap();

        g.relate("thermal-hydraulics", Relation::Employs, "conservation-mass")
            .unwrap();
        g.relate(
            "thermal-hydraulics",
            Relation::Employs,
            "conservation-momentum",
        )
        .unwrap();
        g.relate(
            "thermal-hydraulics",
            Relation::Employs,
            "conservation-energy",
        )
        .unwrap();
        g.relate(
            "conservation-energy",
            Relation::DiscretizedBy,
            "finite-volume",
        )
        .unwrap();
        g.relate("finite-volume", Relation::Employs, "first-order-upwind")
            .unwrap();
        g.relate("conservation-momentum", Relation::SolvedBy, "brent-dekker")
            .unwrap();
        g.relate("finite-volume", Relation::SolvedBy, "lapack")
            .unwrap();
        g.relate("finite-volume", Relation::ImplementedBy, "genfoam")
            .unwrap();
        g.relate("ciet", Relation::Represents, "fhr").unwrap();

        g.relate("pbmr-benchmark", Relation::DerivedFrom, "pbmr400")
            .unwrap();
        g.relate("pbmr-benchmark", Relation::Simplifies, "pbmr400")
            .unwrap();
        g.relate_with(
            "htr10-benchmark",
            Relation::BenchmarkOf,
            "htr10",
            RelationStatus::Established,
            EdgeDetail::Verification(VerificationRecord::new("HTR-10 benchmark model definition")),
        )
        .unwrap();
        g.relate_with(
            "genfoam",
            Relation::ValidatedAgainst,
            "htr10-benchmark",
            RelationStatus::Established,
            EdgeDetail::Verification(VerificationRecord::new("HTR-10 benchmark")),
        )
        .unwrap();
        g.relate_with(
            "openmc",
            Relation::VerifiedAgainst,
            "astra",
            RelationStatus::Established,
            EdgeDetail::Verification(VerificationRecord::new("ASTRA critical facility")),
        )
        .unwrap();
        g.relate_with(
            "pbmr-benchmark",
            Relation::ComparedWith,
            "htr10-benchmark",
            RelationStatus::Established,
            EdgeDetail::Verification(VerificationRecord::new(
                "PBMR-400 vs. HTR-10 pebble-bed benchmark comparison",
            )),
        )
        .unwrap();

        g.relate("transfer-function", Relation::SupportedBy, "ong2024")
            .unwrap();
        g.relate("pbmr-benchmark", Relation::SupportedBy, "iaea1694")
            .unwrap();
        g.relate("htr10-benchmark", Relation::SupportedBy, "iaea1694")
            .unwrap();
        g.relate("thermal-hydraulics", Relation::SupportedBy, "tuas")
            .unwrap();

        g
    }

    #[test]
    fn sample_ontology_is_large_enough_to_exercise_the_abstraction() {
        let g = sample_ontology();
        assert!(
            g.len() >= 35,
            "expected at least 35 concepts, got {}",
            g.len()
        );
        assert!(
            g.edges().len() >= 25,
            "expected at least 25 edges, got {}",
            g.edges().len()
        );
    }

    // --- alias resolution ---

    #[test]
    fn resolve_matches_id_name_and_alias_case_and_punctuation_insensitively() {
        let g = sample_ontology();
        assert_eq!(g.resolve("HTGR").single().unwrap().id, "htgr");
        assert_eq!(g.resolve("H T G R").single().unwrap().id, "htgr");
        assert_eq!(
            g.resolve("high-temperature gas-cooled reactor")
                .single()
                .unwrap()
                .id,
            "htgr"
        );
        assert_eq!(g.resolve("FHR").single().unwrap().id, "fhr");
        assert_eq!(g.resolve("TH").single().unwrap().id, "thermal-hydraulics");
    }

    #[test]
    fn resolve_reports_not_found_distinctly_from_ambiguous() {
        let g = sample_ontology();
        assert_eq!(
            g.resolve("this concept does not exist"),
            ResolveOutcome::NotFound
        );
        assert_eq!(g.resolve("this concept does not exist").single(), None);
    }

    #[test]
    fn resolve_reports_ambiguous_when_two_concepts_share_a_normalised_form() {
        let mut g = ConceptGraph::new();
        g.add_user_concept("alpha-one", "Alpha One", ["Shared"])
            .unwrap();
        g.add_user_concept("alpha-two", "Alpha Two", ["Shared"])
            .unwrap();

        match g.resolve("shared") {
            ResolveOutcome::Ambiguous(matches) => assert_eq!(matches.len(), 2),
            other => panic!("expected Ambiguous, got {other:?}"),
        }
        assert_eq!(g.resolve("shared").single(), None);
    }

    // --- canonical identity ---

    #[test]
    fn core_concept_ids_are_stable_strings_not_derived_from_the_variant_name() {
        assert_eq!(Reactor::Htgr.id(), "htgr");
        assert_eq!(Reactor::NuclearReactor.id(), "nuclear-reactor");
        assert_eq!(ThermalHydraulics::Boussinesq.id(), "boussinesq");
    }

    #[test]
    fn a_concept_resolved_by_id_name_or_alias_has_one_canonical_id() {
        let g = sample_ontology();
        let by_id = g.resolve("htgr").single().unwrap();
        let by_name = g
            .resolve("High-Temperature Gas-Cooled Reactor")
            .single()
            .unwrap();
        let by_alias = g.resolve("HTGRs").single().unwrap();
        assert_eq!(by_id.id, "htgr");
        assert_eq!(by_id.id, by_name.id);
        assert_eq!(by_id.id, by_alias.id);
    }

    // --- graph directionality ---

    #[test]
    fn edges_are_directional_not_symmetric() {
        let g = sample_ontology();
        assert!(
            !g.outgoing("htgr", Some(Relation::SpecializationOf))
                .is_empty(),
            "htgr specialises gas-cooled-reactor"
        );
        assert!(
            g.outgoing("gas-cooled-reactor", Some(Relation::SpecializationOf))
                .iter()
                .all(|e| e.target != "htgr"),
            "the reverse edge must not exist just because the forward one does"
        );
        assert!(
            g.incoming("gas-cooled-reactor", Some(Relation::SpecializationOf))
                .iter()
                .any(|e| e.source == "htgr"),
            "incoming is the deliberate reverse-lookup, distinct from outgoing"
        );
    }

    // --- cross-domain relationships ---

    #[test]
    fn edges_connect_concepts_across_different_curated_domains() {
        let g = sample_ontology();
        // "multiphysics" (a runtime concept) couples the ThermalHydraulics
        // and Neutronics domains -- neither compiled enum references the
        // other directly.
        let targets: Vec<&str> = g
            .outgoing("multiphysics", Some(Relation::CoupledWith))
            .iter()
            .map(|e| e.target.as_str())
            .collect();
        assert!(targets.contains(&"thermal-hydraulics"));
        assert!(targets.contains(&"neutronics"));

        // "ciet" (a runtime, Origin::User facility concept) represents
        // "fhr" (a compiled Reactor variant) -- a runtime concept relating
        // straight to the core, per the module doc.
        assert!(g
            .outgoing("ciet", Some(Relation::Represents))
            .iter()
            .any(|e| e.target == "fhr"));
    }

    // --- user extensions never mutate the compiled core ---

    #[test]
    fn user_concepts_can_relate_to_core_concepts_without_touching_the_core() {
        let mut g = ConceptGraph::new();
        g.add_core(Reactor::Htgr);
        g.add_user_concept(
            "my-htgr-design",
            "My HTGR Design Variant",
            std::iter::empty::<&str>(),
        )
        .unwrap();
        g.relate("my-htgr-design", Relation::SpecializationOf, "htgr")
            .unwrap();

        assert_eq!(g.concept("htgr").unwrap().origin, Origin::Core);
        assert_eq!(g.concept("my-htgr-design").unwrap().origin, Origin::User);
        assert!(g
            .outgoing("my-htgr-design", Some(Relation::SpecializationOf))
            .iter()
            .any(|e| e.target == "htgr"));
    }

    #[test]
    fn a_user_concept_cannot_shadow_a_core_concepts_id() {
        let mut g = ConceptGraph::new();
        g.add_core(Reactor::Htgr);
        let err = g
            .add_user_concept("htgr", "A different HTGR", std::iter::empty::<&str>())
            .unwrap_err();
        assert_eq!(err, OntologyError::DuplicateConcept("htgr".to_string()));
        // The core concept itself must be untouched.
        assert_eq!(
            g.concept("htgr").unwrap().name,
            "High-Temperature Gas-Cooled Reactor"
        );
        assert_eq!(g.concept("htgr").unwrap().origin, Origin::Core);
    }

    #[test]
    fn relate_rejects_a_dangling_edge() {
        let mut g = ConceptGraph::new();
        g.add_user_concept("only-concept", "Only Concept", std::iter::empty::<&str>())
            .unwrap();
        assert_eq!(
            g.relate("only-concept", Relation::Employs, "does-not-exist")
                .unwrap_err(),
            OntologyError::UnknownConcept("does-not-exist".to_string())
        );
        assert_eq!(
            g.relate("does-not-exist", Relation::Employs, "only-concept")
                .unwrap_err(),
            OntologyError::UnknownConcept("does-not-exist".to_string())
        );
    }

    // --- EdgeDetail richness, not flattened into a generic string ---

    #[test]
    fn approximation_edges_carry_structured_applicability() {
        let g = sample_ontology();
        let edge = g
            .outgoing("boussinesq", Some(Relation::ApproximationOf))
            .into_iter()
            .next()
            .unwrap();
        match &edge.detail {
            EdgeDetail::Applicability(a) => {
                assert!(!a.assumptions.is_empty());
                assert!(!a.validity_ranges.is_empty());
            }
            other => panic!("expected Applicability, got {other:?}"),
        }
    }

    #[test]
    fn verification_edges_carry_a_structured_record() {
        let g = sample_ontology();
        let edge = g
            .outgoing("htr10-benchmark", Some(Relation::BenchmarkOf))
            .into_iter()
            .next()
            .unwrap();
        match &edge.detail {
            EdgeDetail::Verification(v) => {
                assert_eq!(v.benchmark, "HTR-10 benchmark model definition")
            }
            other => panic!("expected Verification, got {other:?}"),
        }
    }

    #[test]
    fn verification_record_builder_round_trips_result_and_criterion() {
        let record = VerificationRecord::new("some benchmark")
            .with_result("k_eff = 1.00234 +/- 0.00015")
            .with_acceptance_criterion("within 500 pcm of the reference k_eff");
        assert_eq!(record.benchmark, "some benchmark");
        assert_eq!(
            record.result.as_deref(),
            Some("k_eff = 1.00234 +/- 0.00015")
        );
        assert_eq!(
            record.acceptance_criterion.as_deref(),
            Some("within 500 pcm of the reference k_eff")
        );
    }

    // --- provisional vs. established ---

    #[test]
    fn plain_relate_defaults_to_established_while_relate_with_can_mark_provisional() {
        let g = sample_ontology();
        let established = g
            .outgoing("htgr", Some(Relation::SpecializationOf))
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(established.status, RelationStatus::Established);

        let provisional = g
            .outgoing("transfer-function", Some(Relation::SurrogateOf))
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(provisional.status, RelationStatus::Provisional);
    }

    // --- relations not naturally present in the sample corpus ---

    #[test]
    fn parameterized_by_and_contradicts_work_on_a_minimal_synthetic_graph() {
        // Deliberately not part of `sample_ontology`: forcing a
        // "Contradicts" edge into a real reactor-design corpus would assert
        // a false claim about two real benchmark models. A small synthetic
        // graph exercises the relation type honestly instead.
        let mut g = ConceptGraph::new();
        g.add_user_concept("model-a", "Model A", std::iter::empty::<&str>())
            .unwrap();
        g.add_user_concept("model-b", "Model B", std::iter::empty::<&str>())
            .unwrap();
        g.add_user_concept("parameter-x", "Parameter X", std::iter::empty::<&str>())
            .unwrap();

        g.relate("model-a", Relation::ParameterizedBy, "parameter-x")
            .unwrap();
        g.relate("model-a", Relation::Contradicts, "model-b")
            .unwrap();

        assert!(g
            .outgoing("model-a", Some(Relation::ParameterizedBy))
            .iter()
            .any(|e| e.target == "parameter-x"));
        assert!(g
            .outgoing("model-a", Some(Relation::Contradicts))
            .iter()
            .any(|e| e.target == "model-b"));
    }

    // --- GUI-facing labels ---

    #[test]
    fn every_relation_has_a_human_label_distinct_from_its_variant_name() {
        assert_eq!(Relation::ApproximationOf.label(), "approximates");
        assert_eq!(
            Relation::SpecializationOf.label(),
            "is a specialised form of"
        );
        assert_eq!(Relation::Contradicts.label(), "contradicts");
    }

    #[test]
    fn relation_families_are_named_correctly() {
        assert!(Relation::ApproximationOf.is_approximation_family());
        assert!(Relation::SurrogateOf.is_approximation_family());
        assert!(!Relation::Employs.is_approximation_family());

        assert!(Relation::BenchmarkOf.is_verification_family());
        assert!(Relation::ComparedWith.is_verification_family());
        assert!(!Relation::SpecializationOf.is_verification_family());
    }

    #[test]
    fn an_empty_graph_reports_empty() {
        let g = ConceptGraph::new();
        assert!(g.is_empty());
        assert_eq!(g.len(), 0);
        assert_eq!(g.resolve("anything"), ResolveOutcome::NotFound);
    }
}
