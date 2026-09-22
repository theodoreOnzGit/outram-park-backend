// ---------------------------------------------------------------------------
// Ported from SCRAM (a probabilistic risk analysis tool).
//
//   Upstream project: SCRAM — Olzhas Rakhimov
//   Upstream repo:    https://github.com/rakhimov/scram
//   Upstream file:    src/initializer.{h,cc}, src/xml.{h,cc},
//                     src/element.{h,cc}, src/model.{h,cc},
//                     src/fault_tree.{h,cc}, src/event.{h,cc}
//   Upstream commit:  b85b78940de38996eeffec54d946824bd4280a1c  (2019-07-03)
//   Accessed:         2026-09-22
//
//   Copyright (C) 2014-2018 Olzhas Rakhimov
//   Licensed under the GNU General Public License, version 3 or later.
//
// Same-licence port: SCRAM is GPL-3.0-or-later, RAFFLES is GPL-3.0-only.
//
// Translation notes: upstream validates against a RelaxNG schema
// (`share/*.rng`) with libxml2 and then walks the DOM through `Initializer`,
// registering elements into a `Model` of `Element` subclasses with
// `Role`-based public/private naming. This reads the same documents with a
// pure-Rust pull parser (no libxml2, so Android and wasm stay clean) and
// builds a plain owned tree. **There is no schema validation**: a malformed
// document is caught by the structure checks here, which are narrower than
// the RelaxNG grammar. What that costs is stated in the module doc.
//
// Name resolution follows upstream's `Element`/`Role` scheme: a name
// declared inside `<define-component role="private">` belongs to that
// component and is known by its full dotted path, while a public one keeps
// its bare name. A reference is looked up in the local path first and in the
// model's public names second — `Initializer::GetEntity`, reproduced in
// `Index`. That is the rule `ThreeMotor` needs and the reason it could not
// be read before.
//
// NOT read here: substitutions, event trees, alignments and
// `<define-extern-function>` — each is its own chunk. An unrecognised
// element is REFUSED rather than skipped, so a model using one cannot be
// silently mis-read as a smaller model. CCF groups ARE read, into
// [`super::ccf`]; note that their `<members>` element DECLARES its basic
// events rather than referencing them, which is upstream's
// `ProcessCcfMembers` and the reason `TwoTrain/common_cause.xml` declares
// `ValveOne` nowhere else.
// ---------------------------------------------------------------------------

//! Reading SCRAM's Model Exchange Format.
//!
//! Until this existed, upstream's models reached the tests through a shell
//! script that scraped a flat subset of the XML, and two things were out of
//! reach because of it: `<xi:include>`, and `<define-component>`'s private
//! namespaces. The second is why `ThreeMotor/three_motor` — the one model in
//! the fixture that no analysis route could cover — was excluded: it declares
//! `E1` twice, once at the top and once inside a private component, and a
//! flat parser silently merges them into a wrong tree.
//!
//! # What this reads
//!
//! `<define-fault-tree>` and `<define-component>`, `<define-gate>`,
//! `<define-basic-event>`, `<define-house-event>`, `<define-parameter>`,
//! `<define-CCF-group>`, `<model-data>`, all eleven MEF connectives, event
//! references (`<event>`,
//! `<gate>`, `<basic-event>`, `<house-event>`, and `<event type="…">`),
//! `<not>` and `<constant>` arguments, and the expression elements
//! [`super::expression`] evaluates.
//!
//! **Formulas do not nest**, and that is a property of the format rather than
//! a limit of this reader: upstream's grammar (`share/input.rng`) lets a
//! connective take only event references, `<not>` around one event, or a
//! `<constant>`. See [`Formula`].
//!
//! # What it does not
//!
//! Substitutions, event trees, alignments and extern functions.
//! **An element it does not recognise is an error**, never a skip — a model
//! using one would otherwise be read as a smaller, different model that
//! happens to parse, which is the failure this whole port exists to avoid.
//!
//! The remaining expression elements — the trigonometric functions,
//! `<switch>`, the test-event conditions and `<extern-function>` calls — are
//! refused for the same reason. None appears in any of upstream's own input
//! models.
//!
//! There is also **no schema validation**. Upstream validates against a
//! RelaxNG grammar before looking at anything; this checks only what it needs
//! to build the model. A document SCRAM would reject may be read here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::ccf::{proxy_arguments, CcfGroup, CcfModel};
use super::expression::{Expression, Parameters, DEFAULT_MISSION_TIME};
use super::fault_tree::{Connective, FaultTreeBuilder, FaultTreeModel};
use crate::{RafflesError, Result};

/// A parsed XML element: name, attributes, children, and any text.
///
/// A deliberately small DOM. The pull parser gives events; the MEF walk wants
/// to look at a subtree more than once (an expression's arity decides which
/// formula it is), so the events are collected first.
#[derive(Debug, Clone)]
pub struct Element {
    /// Local name, with any namespace prefix stripped.
    pub name: String,
    /// Attributes, by local name.
    pub attributes: HashMap<String, String>,
    /// Child elements, in document order.
    pub children: Vec<Element>,
}

impl Element {
    /// The value of an attribute, or `None`.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(String::as_str)
    }

    /// The value of an attribute, or an error naming what was missing.
    fn need_attr(&self, name: &str) -> Result<&str> {
        self.attr(name)
            .ok_or_else(|| invalid(format!("<{}> has no `{name}` attribute", self.name)))
    }

    /// Children, ignoring the presentational elements MEF allows anywhere.
    ///
    /// `<label>` and `<attributes>` carry documentation, not structure;
    /// upstream stores them on the `Element` base class and no analysis reads
    /// them.
    fn structural(&self) -> impl Iterator<Item = &Element> {
        self.children
            .iter()
            .filter(|c| !matches!(c.name.as_str(), "label" | "attributes" | "attribute"))
    }
}

fn invalid(reason: String) -> RafflesError {
    RafflesError::InvalidParameter {
        parameter: "model".to_string(),
        value: 0.0,
        reason,
    }
}

/// Parses an XML document into [`Element`]s.
///
/// Namespace prefixes are stripped: MEF uses one vocabulary and the only
/// prefixed element in upstream's suite is `<xi:include>`, which is resolved
/// before this sees it.
pub fn parse_xml(source: &str) -> Result<Element> {
    use xml::reader::{EventReader, XmlEvent};

    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    for event in EventReader::from_str(source) {
        match event.map_err(|e| invalid(format!("malformed XML: {e}")))? {
            XmlEvent::StartElement {
                name, attributes, ..
            } => {
                stack.push(Element {
                    name: name.local_name,
                    attributes: attributes
                        .into_iter()
                        .map(|a| (a.name.local_name, a.value))
                        .collect(),
                    children: Vec::new(),
                });
            }
            XmlEvent::EndElement { .. } => {
                let done = stack
                    .pop()
                    .ok_or_else(|| invalid("unbalanced XML".into()))?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(done),
                    None => root = Some(done),
                }
            }
            _ => {}
        }
    }
    root.ok_or_else(|| invalid("the document has no root element".into()))
}

/// The Model Exchange Format's connective set — upstream's `mef::Connective`.
///
/// The first eight are the analysis connectives, in upstream's own order, and
/// map one-to-one onto [`Connective`]. The last three are, in upstream's
/// words, "rarely used connectives specific to the MEF": they have no PDAG
/// representation and `Pdag::ConstructComplexGate` rewrites each into the
/// first eight. This port keeps them as read and applies the same rewrite in
/// [`MefModel::fault_tree`], so a model using them builds the tree upstream
/// would have built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MefConnective {
    /// `<and>`.
    And,
    /// `<or>`.
    Or,
    /// `<atleast min="k">` — the K-of-N vote.
    Atleast {
        /// How many arguments must occur.
        min: usize,
    },
    /// `<xor>`, two arguments.
    Xor,
    /// `<not>`, one argument.
    Not,
    /// `<nand>`.
    Nand,
    /// `<nor>`.
    Nor,
    /// A pass-through: a gate that names another event directly.
    Null,
    /// `<iff>`, two arguments — equality.
    Iff,
    /// `<imply>`, two arguments — `a` implies `b`.
    Imply,
    /// `<cardinality min="j" max="k">` — between `j` and `k` arguments occur.
    Cardinality {
        /// Fewest arguments that may occur.
        min: usize,
        /// Most arguments that may occur.
        max: usize,
    },
}

/// A gate's Boolean formula.
///
/// **MEF formulas do not nest.** Upstream's RelaxNG grammar (`share/input.rng`,
/// `define name="formula"`) admits a connective whose arguments are each an
/// `argument`, and an `argument` is an event reference, a `<not>` around one
/// event reference, or a `<constant>` — never another connective. An earlier
/// revision of this reader carried machinery for arbitrarily nested formulas
/// and said so in its module doc; that was **wrong about the format**, and the
/// claim is corrected here rather than left standing. What does exist is the
/// `<not>` wrapper, which upstream records as a *complement flag on the
/// argument* and this port lowers into an anonymous negating gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Formula {
    /// The connective.
    pub connective: MefConnective,
    /// Its arguments, in document order.
    pub args: Vec<FormulaArg>,
}

/// One argument of a [`Formula`] — upstream's `Formula::Arg`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormulaArg {
    /// A reference to a declared event, by resolved id, optionally negated by
    /// a `<not>` wrapper.
    Event {
        /// The `Id::id()` of the declaration this reference resolves to.
        name: String,
        /// Whether a `<not>` wrapped it.
        complement: bool,
    },
    /// `<constant value="true|false"/>`. Upstream substitutes its
    /// `HouseEvent::kTrue` / `kFalse` singletons.
    Constant(bool),
}

impl FormulaArg {
    /// The argument with its sense flipped — upstream's `Gate::NegateArgs`.
    fn negated(&self) -> Self {
        match self {
            FormulaArg::Event { name, complement } => FormulaArg::Event {
                name: name.clone(),
                complement: !complement,
            },
            FormulaArg::Constant(v) => FormulaArg::Constant(!v),
        }
    }
}

/// A parsed MEF model.
#[derive(Debug, Clone, Default)]
pub struct MefModel {
    /// The `name` attribute of `<opsa-mef>`, if any.
    pub name: Option<String>,
    /// Mission time, defaulting to [`DEFAULT_MISSION_TIME`].
    pub mission_time: f64,
    /// Parameter expressions, by resolved name.
    pub parameters: HashMap<String, Expression>,
    /// Basic-event expressions, by resolved name.
    pub basic_events: HashMap<String, Expression>,
    /// House-event constants, by resolved name.
    pub house_events: HashMap<String, bool>,
    /// Gate formulas, by resolved name.
    pub gates: HashMap<String, Formula>,
    /// Declaration order of the gates, so a model reads back deterministically.
    pub gate_order: Vec<String>,
    /// Common-cause failure groups, in declaration order.
    ///
    /// [`MefModel::fault_tree`] applies them; [`MefModel::without_ccf`] is
    /// the explicit ablation.
    pub ccf_groups: Vec<CcfGroup>,
}

/// One naming scope: the container a declaration sits in.
///
/// Upstream's `Initializer` threads two things through every declaration: a
/// `base_path` (the dotted chain of enclosing fault tree and components) and
/// the container's `RoleSpecifier`. `DefineFaultTree` starts the chain with
/// the tree's own name and is **public** — `mef::FaultTree` is documented as
/// "assumed to be public and belong to the root model". `DefineComponent`
/// appends its name and **inherits the container's role** unless the element
/// names one itself (`GetRole(s, parent_role)`, `initializer.cc:69`).
/// `<model-data>` declares into the empty path, publicly.
#[derive(Clone)]
struct Scope {
    /// The dotted chain of enclosing containers; empty at model level.
    base_path: String,
    /// Whether declarations here are private, i.e. keyed by full path.
    private: bool,
}

impl Scope {
    /// The model-level scope: no path, public.
    fn root() -> Self {
        Scope {
            base_path: String::new(),
            private: false,
        }
    }

    /// Upstream's `GetFullPath` — `base_path + "." + name`, **leading dot and
    /// all** when the base path is empty, because that is the key upstream's
    /// path tables hold. Reproducing the dot matters: it is what stops a
    /// local lookup inside a component from reaching a `<model-data>`
    /// declaration by path rather than by public name.
    fn full_path(&self, name: &str) -> String {
        format!("{}.{name}", self.base_path)
    }

    /// Upstream's `Id::id()` — a public element keeps its bare name, a
    /// private one is known by its full path.
    fn id(&self, name: &str) -> String {
        if self.private {
            self.full_path(name)
        } else {
            name.to_string()
        }
    }

    /// The scope a `<define-fault-tree>` opens.
    fn fault_tree(name: &str) -> Self {
        Scope {
            base_path: name.to_string(),
            private: false,
        }
    }

    /// The scope a `<define-component>` opens inside this one.
    fn component(&self, name: &str, role: Option<&str>) -> Result<Self> {
        Ok(Scope {
            base_path: if self.base_path.is_empty() {
                name.to_string()
            } else {
                format!("{}.{name}", self.base_path)
            },
            private: match role {
                Some("private") => true,
                Some("public") => false,
                // Upstream's RelaxNG grammar admits only those two, and
                // `GetRole` asserts it; with no schema validation here the
                // check has to be explicit.
                Some(other) => {
                    return Err(invalid(format!(
                        "<define-component name=\"{name}\"> has role=\"{other}\"; MEF \
                         allows only \"public\" or \"private\""
                    )))
                }
                None => self.private,
            },
        })
    }
}

/// One type's declarations, in the two tables upstream keeps them in.
///
/// `Initializer::GetEntity` consults a **path table** keyed by the full
/// dotted path (every declaration is in it) and the model's **id table**
/// keyed by `Id::id()` (bare name for a public element, full path for a
/// private one). Both are needed: the first is how a reference reaches a
/// sibling inside its own component, the second is how it reaches a public
/// declaration anywhere in the model.
#[derive(Debug, Default)]
struct Table {
    /// `Id::id()` of every declaration of this type.
    ids: std::collections::HashSet<String>,
    /// Full dotted path -> `Id::id()`.
    paths: HashMap<String, String>,
}

impl Table {
    /// Registers one declaration, returning its id.
    fn declare(&mut self, name: &str, scope: &Scope, kind: &str) -> Result<String> {
        let id = scope.id(name);
        let path = scope.full_path(name);
        if self.paths.contains_key(&path) {
            return Err(invalid(format!("{kind} `{path}` is declared twice")));
        }
        self.paths.insert(path, id.clone());
        Ok(id)
    }

    /// Upstream's `GetEntity`, for one type.
    ///
    /// Local scope first — `base_path + "." + reference` in the path table —
    /// then the id table for an unqualified reference, or the path table for
    /// a dotted one ("direct access").
    fn lookup(&self, reference: &str, scope: &Scope) -> Option<&String> {
        if !scope.base_path.is_empty() {
            if let Some(id) = self.paths.get(&format!("{}.{reference}", scope.base_path)) {
                return Some(id);
            }
        }
        if reference.contains('.') {
            self.paths.get(reference)
        } else {
            self.ids.get(reference)
        }
    }
}

/// Every declaration in the model, indexed the way upstream indexes them.
///
/// Built in a first pass over the document so that the second pass can
/// resolve a reference to a declaration that appears *later* — upstream gets
/// the same effect by deferring formulas and expressions to its `tbd_` list
/// and walking it once registration is complete.
#[derive(Debug, Default)]
struct Index {
    gates: Table,
    basic_events: Table,
    house_events: Table,
    parameters: Table,
}

impl Index {
    /// Refuses an event id already taken by any of the three event types.
    ///
    /// `mef::Model::Add` checks basic events, gates and house events against
    /// one another, so the three share one id namespace even though they are
    /// three tables.
    fn claim_event_id(&self, id: &str) -> Result<()> {
        for table in [&self.gates, &self.basic_events, &self.house_events] {
            if table.ids.contains(id) {
                return Err(invalid(format!("`{id}` is declared twice")));
            }
        }
        Ok(())
    }

    /// Upstream's `GetEvent`: a `<event>` reference, which may name a gate, a
    /// basic event or a house event, searched in that order.
    fn resolve_event(&self, reference: &str, scope: &Scope) -> Result<String> {
        // The local-scope check sweeps all three tables before any global
        // lookup happens, which is why this is not three `lookup` calls.
        if !scope.base_path.is_empty() {
            let full = format!("{}.{reference}", scope.base_path);
            for table in [&self.gates, &self.basic_events, &self.house_events] {
                if let Some(id) = table.paths.get(&full) {
                    return Ok(id.clone());
                }
            }
        }
        for table in [&self.gates, &self.basic_events, &self.house_events] {
            let hit = if reference.contains('.') {
                table.paths.get(reference)
            } else {
                table.ids.get(reference)
            };
            if let Some(id) = hit {
                return Ok(id.clone());
            }
        }
        Err(undefined("event", reference, scope))
    }

    /// One typed reference — `<gate>`, `<basic-event>`, `<house-event>`.
    fn resolve_typed(&self, kind: &str, reference: &str, scope: &Scope) -> Result<String> {
        let table = match kind {
            "gate" => &self.gates,
            "basic-event" => &self.basic_events,
            "house-event" => &self.house_events,
            "parameter" => &self.parameters,
            _ => return Err(invalid(format!("<{kind}> is not a reference element"))),
        };
        table
            .lookup(reference, scope)
            .cloned()
            .ok_or_else(|| undefined(kind, reference, scope))
    }
}

/// Upstream's `UndefinedElement`, which carries the same three facts.
fn undefined(kind: &str, reference: &str, scope: &Scope) -> RafflesError {
    invalid(format!(
        "undefined {kind} `{reference}`, referenced from base path `{}`",
        scope.base_path
    ))
}

impl MefModel {
    /// Reads a model from a file, resolving `<xi:include>` relative to it.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if the file cannot be read, the XML
    /// is malformed, or the model uses a construct this reader refuses.
    pub fn from_file(path: &Path) -> Result<Self> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| invalid(format!("reading {}: {e}", path.display())))?;
        let base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let mut root = parse_xml(&source)?;
        resolve_includes(&mut root, &base)?;
        Self::from_element(&root)
    }

    /// Reads a model from an already-parsed `<opsa-mef>` element.
    ///
    /// # Errors
    ///
    /// As [`MefModel::from_file`].
    pub fn from_element(root: &Element) -> Result<Self> {
        if root.name != "opsa-mef" {
            return Err(invalid(format!(
                "the root element is <{}>, not <opsa-mef>",
                root.name
            )));
        }
        let mut model = MefModel {
            name: root.attr("name").map(str::to_string),
            mission_time: DEFAULT_MISSION_TIME,
            ..Default::default()
        };
        // Two passes, for the reason upstream defers to its `tbd_` list: a
        // formula may reference a gate declared later in the document, so
        // nothing can be resolved until every declaration is registered.
        let mut index = Index::default();
        model.register_container(root, &Scope::root(), &mut index)?;
        model.define_container(root, &Scope::root(), &index)?;
        Ok(model)
    }

    /// First pass: registers every declaration, and refuses every construct
    /// this reader does not read.
    ///
    /// This is upstream's `Initializer::Register<T>` / `RegisterFaultTreeData`
    /// / `ProcessModelData`, which build the id and path tables before any
    /// formula or expression is looked at. House-event states are set here
    /// too, because upstream sets them inside `Register<HouseEvent>` rather
    /// than deferring them.
    fn register_container(
        &mut self,
        element: &Element,
        scope: &Scope,
        index: &mut Index,
    ) -> Result<()> {
        for child in element.structural() {
            match child.name.as_str() {
                "define-fault-tree" => {
                    // `DefineFaultTree` passes the tree's own name as the
                    // base path — a fault tree IS a path component, even
                    // though its public members keep their bare names.
                    let name = child.need_attr("name")?;
                    self.register_container(child, &Scope::fault_tree(name), index)?;
                }
                "model-data" => {
                    // `ProcessModelData` registers with an empty base path
                    // and the public role, whatever encloses it.
                    self.register_container(child, &Scope::root(), index)?;
                }
                "define-component" => {
                    let name = child.need_attr("name")?;
                    let inner = scope.component(name, child.attr("role"))?;
                    self.register_container(child, &inner, index)?;
                }
                "define-gate" => {
                    let id = index
                        .gates
                        .declare(child.need_attr("name")?, scope, "gate")?;
                    index.claim_event_id(&id)?;
                    index.gates.ids.insert(id);
                }
                "define-basic-event" => {
                    let id = index.basic_events.declare(
                        child.need_attr("name")?,
                        scope,
                        "basic event",
                    )?;
                    index.claim_event_id(&id)?;
                    index.basic_events.ids.insert(id);
                }
                "define-house-event" => {
                    let name = child.need_attr("name")?;
                    let id = index.house_events.declare(name, scope, "house event")?;
                    index.claim_event_id(&id)?;
                    index.house_events.ids.insert(id.clone());
                    let value = match child.structural().next() {
                        Some(c) if c.name == "constant" => c.need_attr("value")? == "true",
                        // Upstream's `HouseEvent` defaults to false, and
                        // `Register<HouseEvent>` sets a state only if the
                        // `<constant>` child is present.
                        None => false,
                        Some(c) => {
                            return Err(invalid(format!(
                                "house event `{id}` holds <{}>, expected <constant>",
                                c.name
                            )))
                        }
                    };
                    self.house_events.insert(id, value);
                }
                "define-parameter" => {
                    let id =
                        index
                            .parameters
                            .declare(child.need_attr("name")?, scope, "parameter")?;
                    if !index.parameters.ids.insert(id.clone()) {
                        return Err(invalid(format!("parameter `{id}` is declared twice")));
                    }
                }
                // A group's `<members>` DECLARE their basic events rather
                // than referencing them — upstream's `ProcessCcfMembers`
                // constructs a `BasicEvent` per member with the group's own
                // base path and role. `TwoTrain/common_cause.xml` declares
                // `ValveOne` nowhere else, and reading the members as
                // references instead is how that model fails to load.
                "define-CCF-group" => {
                    let group = child.need_attr("name")?;
                    let members = child
                        .structural()
                        .find(|c| c.name == "members")
                        .ok_or_else(|| {
                            invalid(format!("CCF group `{group}` declares no <members>"))
                        })?;
                    for member in members.structural() {
                        if member.name != "basic-event" {
                            return Err(invalid(format!(
                                "CCF group `{group}` holds <{}> among its members, \
                                 expected <basic-event>",
                                member.name
                            )));
                        }
                        let id = index.basic_events.declare(
                            member.need_attr("name")?,
                            scope,
                            "basic event",
                        )?;
                        index.claim_event_id(&id)?;
                        index.basic_events.ids.insert(id);
                    }
                }
                "define-substitution" => {
                    return Err(invalid(
                        "substitutions are not read yet (their own chunk of the port); \
                         refusing rather than reading a model without them"
                            .into(),
                    ))
                }
                "define-event-tree"
                | "define-initiating-event"
                | "define-sequence"
                | "define-rule"
                | "define-functional-event" => {
                    return Err(invalid(format!(
                        "<{}> belongs to event-tree analysis, which is not read yet (its \
                         own chunk of the port); refusing rather than reading a partial \
                         model",
                        child.name
                    )))
                }
                "define-alignment" => {
                    return Err(invalid(
                        "alignments are not read yet (their own chunk of the port)".into(),
                    ))
                }
                "define-extern-library" | "define-extern-function" => {
                    return Err(invalid(
                        "extern functions load shared libraries and are deliberately not \
                         ported — see the workspace RESPONSIBLE_USE.md rule on autonomous \
                         access"
                            .into(),
                    ))
                }
                other => {
                    return Err(invalid(format!(
                        "unrecognised element <{other}>; refusing rather than skipping it, \
                         since a skipped declaration reads as a smaller model that happens \
                         to parse"
                    )))
                }
            }
        }
        Ok(())
    }

    /// Second pass: reads the formulas and expressions, resolving every
    /// reference against the index the first pass built.
    ///
    /// This is upstream's walk of its `tbd_` list, which runs once every
    /// declaration is registered and is what lets a formula name a gate the
    /// document declares further down.
    fn define_container(&mut self, element: &Element, scope: &Scope, index: &Index) -> Result<()> {
        for child in element.structural() {
            match child.name.as_str() {
                "define-fault-tree" => {
                    let name = child.need_attr("name")?;
                    self.define_container(child, &Scope::fault_tree(name), index)?;
                }
                "model-data" => self.define_container(child, &Scope::root(), index)?,
                "define-component" => {
                    let name = child.need_attr("name")?;
                    let inner = scope.component(name, child.attr("role"))?;
                    self.define_container(child, &inner, index)?;
                }
                "define-gate" => {
                    let id = scope.id(child.need_attr("name")?);
                    let formula = read_formula_of(child, scope, index)?;
                    self.gates.insert(id.clone(), formula);
                    self.gate_order.push(id);
                }
                "define-basic-event" => {
                    let id = scope.id(child.need_attr("name")?);
                    let expression = match child.structural().next() {
                        Some(e) => read_expression(e, scope, index)?,
                        // A basic event with no expression is legal in MEF —
                        // its probability comes from elsewhere. Nothing in
                        // the fixture does it, so it is refused rather than
                        // guessed.
                        None => {
                            return Err(invalid(format!(
                                "basic event `{id}` declares no expression; reading one \
                                 without a probability is not supported"
                            )))
                        }
                    };
                    self.basic_events.insert(id, expression);
                }
                "define-parameter" => {
                    let id = scope.id(child.need_attr("name")?);
                    let e = child.structural().next().ok_or_else(|| {
                        invalid(format!("parameter `{id}` declares no expression"))
                    })?;
                    self.parameters
                        .insert(id, read_expression(e, scope, index)?);
                }
                "define-CCF-group" => {
                    let group = read_ccf_group(child, scope, index)?;
                    // Upstream `CcfGroup::AddDistribution`: every member's own
                    // probability is the group's distribution. That is what
                    // an analysis WITHOUT the CCF model uses, so it has to be
                    // recorded whether or not the model is applied.
                    for member in &group.members {
                        self.basic_events
                            .insert(member.clone(), group.distribution.clone());
                    }
                    self.ccf_groups.push(group);
                }
                // Read during registration, as upstream does.
                "define-house-event" => {}
                other => {
                    // Unreachable: the first pass refuses everything else.
                    // Kept as an error rather than a `_ => {}` so that a
                    // future arm added to one pass and not the other cannot
                    // become a silent skip.
                    return Err(invalid(format!("unrecognised element <{other}>")));
                }
            }
        }
        Ok(())
    }

    /// Every parameter's value, resolved by repeated passes.
    ///
    /// This terminates because upstream forbids parameter cycles
    /// (`src/cycle.h`); a cycle here simply fails to resolve and is reported
    /// rather than detected structurally.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] naming the parameters that could not
    /// be resolved.
    pub fn resolved_parameters(&self) -> Result<Parameters> {
        let mut resolved = Parameters::new();
        for _ in 0..self.parameters.len().max(1) {
            let mut progressed = false;
            for (name, e) in &self.parameters {
                if resolved.contains_key(name) {
                    continue;
                }
                if let Ok(v) = e.evaluate(&resolved, self.mission_time) {
                    resolved.insert(name.clone(), v);
                    progressed = true;
                }
            }
            if !progressed {
                break;
            }
        }
        if resolved.len() < self.parameters.len() {
            let missing: Vec<&String> = self
                .parameters
                .keys()
                .filter(|k| !resolved.contains_key(*k))
                .collect();
            return Err(invalid(format!(
                "these parameters could not be resolved, which upstream would report as a \
                 cycle: {missing:?}"
            )));
        }
        Ok(resolved)
    }

    /// This model with its common-cause groups **dropped** — the explicit
    /// ablation.
    ///
    /// Equivalent to running SCRAM *without* `--ccf`: the members keep the
    /// group's distribution as their own independent probability, which is
    /// what upstream's `AddDistribution` assigns them, and no CCF event is
    /// created. It is a real analysis, and a strictly optimistic one; it is
    /// here so that dropping the coupling is something a caller has to ask
    /// for and say why.
    pub fn without_ccf(&self) -> MefModel {
        let mut out = self.clone();
        out.ccf_groups.clear();
        out
    }

    /// This model with every common-cause group applied — upstream's
    /// `CcfGroup::ApplyModel`, over the whole model.
    ///
    /// Each member basic event becomes a proxy gate of the same id, an `or`
    /// over every CCF event that couples it, and each `k`-subset of a group's
    /// members gains a basic event `[A B …]` carrying that level's
    /// probability.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if a group fails upstream's
    /// validation, names a member that is not a declared basic event, or its
    /// factors do not suit its model.
    pub fn apply_ccf(&self) -> Result<MefModel> {
        if self.ccf_groups.is_empty() {
            return Ok(self.clone());
        }
        let parameters = self.resolved_parameters()?;
        let mut out = self.clone();
        out.ccf_groups.clear();
        for group in &self.ccf_groups {
            group.validate(&parameters, self.mission_time)?;
            let events = group.events()?;
            for member in &group.members {
                if out.basic_events.remove(member).is_none() {
                    return Err(invalid(format!(
                        "CCF group `{}` names `{member}`, which is not a declared basic \
                         event",
                        group.name
                    )));
                }
            }
            for event in &events {
                // Upstream refuses a malformed factor set at expression
                // CONSTRUCTION -- an alpha-factor group with one factor makes
                // the weighted sum an `Add` of a single argument, and
                // `EnsureMultivariateArgs` throws. Here the expression is a
                // value, so the same refusal comes from validating it.
                event.probability.validate(&parameters, self.mission_time)?;
                out.basic_events
                    .insert(event.id.clone(), event.probability.clone());
            }
            let arguments = proxy_arguments(&events);
            // Iterated in the group's own member order, not the map's: a
            // HashMap walk would make the gate order differ between runs, and
            // this workspace has already lost a day to exactly that.
            for member in &group.members {
                let args = arguments.get(member).cloned().unwrap_or_default();
                out.gates.insert(
                    member.clone(),
                    Formula {
                        connective: MefConnective::Or,
                        args: args
                            .into_iter()
                            .map(|name| FormulaArg::Event {
                                name,
                                complement: false,
                            })
                            .collect(),
                    },
                );
                out.gate_order.push(member.clone());
            }
        }
        Ok(out)
    }

    /// Flattens the model into a [`FaultTreeModel`] rooted at `top`.
    ///
    /// Only gates reachable from `top` are included, so a document declaring
    /// several independent fault trees yields the one asked for.
    ///
    /// Three things upstream expresses on the *edge* have no edge to live on
    /// here, and become anonymous gates named `<owner>.aux<n>`:
    ///
    /// * a `<not>`-wrapped argument, which upstream carries as
    ///   `Formula::Arg::complement` and turns into a complement edge in the
    ///   PDAG;
    /// * `<constant value="…"/>`, which upstream replaces with its
    ///   `HouseEvent::kTrue`/`kFalse` singletons — here a reserved house event
    ///   named `<constant:true>` or `<constant:false>`, whose angle brackets
    ///   no MEF name may contain;
    /// * `<iff>`, `<imply>` and `<cardinality>`, which have no PDAG form at
    ///   all and are rewritten exactly as `Pdag::ConstructComplexGate` does.
    ///
    /// Basic-event probabilities are **evaluated** through
    /// [`super::expression`], with the model's parameters and mission time.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `top` is not a declared gate, a
    /// reference resolves to nothing, or an expression fails to evaluate.
    pub fn fault_tree(&self, top: &str) -> Result<FaultTreeModel> {
        // A model that declares a CCF group has said its components are
        // coupled; applying that is the default, and `without_ccf` is the
        // visible ablation.
        if !self.ccf_groups.is_empty() {
            return self.apply_ccf()?.fault_tree(top);
        }
        if !self.gates.contains_key(top) {
            return Err(invalid(format!("`{top}` is not a declared gate")));
        }
        let resolved = self.resolved_parameters()?;

        // Walk from the top, collecting the gates and events actually used.
        let mut lowering = Lowering::new(self);
        lowering.gate(top)?;

        let mut builder = FaultTreeBuilder::new();
        lowering.events.sort_unstable();
        lowering.events.dedup();
        lowering.houses.sort_unstable();
        lowering.houses.dedup();

        for name in &lowering.events {
            let e = self
                .basic_events
                .get(name)
                .ok_or_else(|| invalid(format!("`{name}` is referenced but never declared")))?;
            let p = e.evaluate(&resolved, self.mission_time)?;
            builder.basic_event(name, p)?;
        }
        for name in &lowering.houses {
            builder.house_event(name, self.house_events[name])?;
        }
        for (name, value) in [(CONSTANT_TRUE, true), (CONSTANT_FALSE, false)] {
            if lowering.constants[usize::from(value)] {
                builder.house_event(name, value)?;
            }
        }
        for (name, connective, args) in &lowering.gates {
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            builder.gate(name, *connective, &refs)?;
        }
        builder.build(top)
    }

    /// Every gate no other gate references — the candidate top events.
    pub fn top_gates(&self) -> Vec<String> {
        let mut referenced: Vec<&String> = Vec::new();
        for f in self.gates.values() {
            for a in &f.args {
                if let FormulaArg::Event { name, .. } = a {
                    referenced.push(name);
                }
            }
        }
        self.gate_order
            .iter()
            .filter(|g| !referenced.iter().any(|r| *r == *g))
            .cloned()
            .collect()
    }
}

/// The reserved house event standing in for `<constant value="true"/>`.
///
/// Angle brackets cannot occur in a MEF name, so this can never collide with a
/// declaration.
pub const CONSTANT_TRUE: &str = "<constant:true>";

/// The reserved house event standing in for `<constant value="false"/>`.
pub const CONSTANT_FALSE: &str = "<constant:false>";

/// Lowers a [`MefModel`]'s formulas into the flat gate list a
/// [`FaultTreeBuilder`] takes.
///
/// This is the job `Pdag`'s constructor does upstream: walk from the top gate,
/// turn each `mef::Formula` into one or more analysis gates, and record which
/// basic and house events the walk actually reached.
struct Lowering<'a> {
    model: &'a MefModel,
    /// `(name, connective, argument names)`, in the order they were emitted.
    gates: Vec<(String, Connective, Vec<String>)>,
    /// Gate ids already emitted, so a shared sub-tree is visited once.
    seen: Vec<String>,
    /// Basic events the walk reached.
    events: Vec<String>,
    /// House events the walk reached.
    houses: Vec<String>,
    /// Whether the `false` and `true` constants were used, indexed by the
    /// constant's own value.
    constants: [bool; 2],
    /// Counter for anonymous gate names.
    aux: usize,
}

impl<'a> Lowering<'a> {
    fn new(model: &'a MefModel) -> Self {
        Lowering {
            model,
            gates: Vec::new(),
            seen: Vec::new(),
            events: Vec::new(),
            houses: Vec::new(),
            constants: [false; 2],
            aux: 0,
        }
    }

    /// A fresh anonymous gate name under `owner`.
    fn fresh(&mut self, owner: &str) -> String {
        self.aux += 1;
        format!("{owner}.aux{}", self.aux)
    }

    /// Emits `gate` and everything it reaches.
    fn gate(&mut self, gate: &str) -> Result<()> {
        if self.seen.iter().any(|s| s == gate) {
            return Ok(());
        }
        self.seen.push(gate.to_string());
        let formula = self
            .model
            .gates
            .get(gate)
            .ok_or_else(|| invalid(format!("`{gate}` is referenced but never declared")))?;
        let (connective, args) = self.lower(gate, formula)?;
        self.gates.push((gate.to_string(), connective, args));
        Ok(())
    }

    /// Turns one formula into the connective and argument names of the gate
    /// that owns it, emitting whatever auxiliary gates that needs.
    fn lower(&mut self, owner: &str, formula: &Formula) -> Result<(Connective, Vec<String>)> {
        let core = |c: Connective| -> Connective { c };
        Ok(match formula.connective {
            MefConnective::And => (core(Connective::And), self.arg_names(owner, &formula.args)?),
            MefConnective::Or => (core(Connective::Or), self.arg_names(owner, &formula.args)?),
            MefConnective::Atleast { min } => (
                Connective::Atleast { min },
                self.arg_names(owner, &formula.args)?,
            ),
            MefConnective::Xor => (Connective::Xor, self.arg_names(owner, &formula.args)?),
            MefConnective::Not => (Connective::Not, self.arg_names(owner, &formula.args)?),
            MefConnective::Nand => (Connective::Nand, self.arg_names(owner, &formula.args)?),
            MefConnective::Nor => (Connective::Nor, self.arg_names(owner, &formula.args)?),
            MefConnective::Null => (Connective::Null, self.arg_names(owner, &formula.args)?),
            // `ConstructComplexGate`, case `kIff`: a NULL gate holding the
            // COMPLEMENT of an XOR over the two arguments. A complement edge
            // under a null gate is a `Not` gate here.
            MefConnective::Iff => {
                self.expect_arity(owner, "iff", &formula.args, 2, 2)?;
                let args = self.arg_names(owner, &formula.args)?;
                let inner = self.fresh(owner);
                self.gates.push((inner.clone(), Connective::Xor, args));
                (Connective::Not, vec![inner])
            }
            // `ConstructComplexGate`, case `kImply`: an OR of the first
            // argument's complement with the second argument as written.
            MefConnective::Imply => {
                self.expect_arity(owner, "imply", &formula.args, 2, 2)?;
                let antecedent = formula.args[0].negated();
                let args = self.arg_names(owner, &[antecedent, formula.args[1].clone()])?;
                (Connective::Or, args)
            }
            // `ConstructComplexGate`, case `kCardinality`: an AND of
            // "at least `min` occur" with "at least `n - max` do NOT occur",
            // each put through upstream's `well_form` lambda.
            MefConnective::Cardinality { min, max } => {
                let n = formula.args.len();
                if max > n || min > max {
                    return Err(invalid(format!(
                        "gate `{owner}`: <cardinality min=\"{min}\" max=\"{max}\"> over {n} \
                         arguments needs min <= max <= the argument count"
                    )));
                }
                let positive: Vec<FormulaArg> = formula.args.clone();
                let negative: Vec<FormulaArg> =
                    formula.args.iter().map(FormulaArg::negated).collect();
                let lower = self.well_formed_atleast(owner, min, &positive)?;
                let upper = self.well_formed_atleast(owner, n - max, &negative)?;
                (Connective::And, vec![lower, upper])
            }
        })
    }

    /// Upstream's `well_form` lambda: an at-least gate whose threshold has
    /// collapsed is emitted as the connective it has actually become.
    fn well_formed_atleast(
        &mut self,
        owner: &str,
        min: usize,
        args: &[FormulaArg],
    ) -> Result<String> {
        let name = self.fresh(owner);
        if min == 0 {
            // `MakeConstant(true)`: the threshold is met whatever happens.
            let arg = self.arg_name(owner, &FormulaArg::Constant(true))?;
            self.gates.push((name.clone(), Connective::Null, vec![arg]));
            return Ok(name);
        }
        let names = self.arg_names(owner, args)?;
        let connective = if min == 1 {
            Connective::Or
        } else if min == names.len() {
            Connective::And
        } else {
            Connective::Atleast { min }
        };
        self.gates.push((name.clone(), connective, names));
        Ok(name)
    }

    /// Refuses an argument count the connective cannot take.
    fn expect_arity(
        &self,
        owner: &str,
        connective: &str,
        args: &[FormulaArg],
        min: usize,
        max: usize,
    ) -> Result<()> {
        if args.len() < min || args.len() > max {
            return Err(invalid(format!(
                "gate `{owner}`: <{connective}> takes {min} arguments, found {}",
                args.len()
            )));
        }
        Ok(())
    }

    /// Argument names for a whole formula.
    fn arg_names(&mut self, owner: &str, args: &[FormulaArg]) -> Result<Vec<String>> {
        args.iter().map(|a| self.arg_name(owner, a)).collect()
    }

    /// The name a single argument contributes, emitting an auxiliary gate for
    /// a complement and reaching a referenced gate's own formula.
    fn arg_name(&mut self, owner: &str, arg: &FormulaArg) -> Result<String> {
        match arg {
            FormulaArg::Constant(value) => {
                self.constants[usize::from(*value)] = true;
                Ok(if *value {
                    CONSTANT_TRUE
                } else {
                    CONSTANT_FALSE
                }
                .to_string())
            }
            FormulaArg::Event { name, complement } => {
                if self.model.gates.contains_key(name) {
                    self.gate(name)?;
                } else if self.model.house_events.contains_key(name) {
                    self.houses.push(name.clone());
                } else if self.model.basic_events.contains_key(name) {
                    self.events.push(name.clone());
                } else {
                    return Err(invalid(format!(
                        "`{name}`, referenced by `{owner}`, is not a declared gate, basic \
                         event or house event"
                    )));
                }
                if !complement {
                    return Ok(name.clone());
                }
                let negated = self.fresh(owner);
                self.gates
                    .push((negated.clone(), Connective::Not, vec![name.clone()]));
                Ok(negated)
            }
        }
    }
}

/// The formula of a `<define-gate>`: exactly one child, which is either a
/// connective or a bare event reference (an implicit `null` gate).
///
/// Upstream reaches the same place through `gate->formula(GetFormula(
/// *formulas.begin(), gate->base_path()))`, which likewise takes the first
/// and only formula child.
fn read_formula_of(gate: &Element, scope: &Scope, index: &Index) -> Result<Formula> {
    let mut kids = gate.structural();
    let first = kids.next().ok_or_else(|| {
        invalid(format!(
            "gate `{}` has no formula",
            gate.attr("name").unwrap_or("?")
        ))
    })?;
    if kids.next().is_some() {
        return Err(invalid(format!(
            "gate `{}` holds more than one formula",
            gate.attr("name").unwrap_or("?")
        )));
    }
    read_formula(first, scope, index)
}

/// Reads a formula element — upstream's `Initializer::GetFormula`.
///
/// The connective is `Null` when the node carries a `name` attribute (a gate
/// standing directly for another event, MEF's transfer symbol) or is a
/// `<constant>`; otherwise it is the element's own name, looked up in
/// upstream's `kConnectiveToString`. In the `Null` case the node **is** its
/// own single argument, which is the branch upstream writes as
/// `add_arg(formula_node)`.
fn read_formula(element: &Element, scope: &Scope, index: &Index) -> Result<Formula> {
    if element.attr("name").is_some() || element.name == "constant" {
        return Ok(Formula {
            connective: MefConnective::Null,
            args: vec![read_argument(element, scope, index)?],
        });
    }
    let num = |name: &str| -> Result<usize> {
        element
            .need_attr(name)?
            .parse()
            .map_err(|_| invalid(format!("<{}> has a non-numeric `{name}`", element.name)))
    };
    let connective = match element.name.as_str() {
        "and" => MefConnective::And,
        "or" => MefConnective::Or,
        "atleast" => MefConnective::Atleast { min: num("min")? },
        "xor" => MefConnective::Xor,
        "not" => MefConnective::Not,
        "nand" => MefConnective::Nand,
        "nor" => MefConnective::Nor,
        "null" => MefConnective::Null,
        "iff" => MefConnective::Iff,
        "imply" => MefConnective::Imply,
        "cardinality" => MefConnective::Cardinality {
            min: num("min")?,
            max: num("max")?,
        },
        other => {
            return Err(invalid(format!(
                "<{other}> is not a connective this reader knows"
            )))
        }
    };

    let mut args = Vec::new();
    for child in element.structural() {
        args.push(read_argument(child, scope, index)?);
    }
    if args.is_empty() {
        return Err(invalid(format!("<{}> has no arguments", element.name)));
    }
    Ok(Formula { connective, args })
}

/// Reads one formula argument — upstream's `add_arg` lambda.
///
/// An argument is an event reference, a `<not>` around exactly one event
/// reference, or a `<constant>`. The grammar admits nothing else: a
/// connective is **not** a legal argument, so formulas do not nest.
fn read_argument(element: &Element, scope: &Scope, index: &Index) -> Result<FormulaArg> {
    match element.name.as_str() {
        "constant" => Ok(FormulaArg::Constant(element.need_attr("value")? == "true")),
        "not" => {
            let mut kids = element.structural();
            let inner = kids
                .next()
                .ok_or_else(|| invalid("<not> has no argument".into()))?;
            if kids.next().is_some() {
                return Err(invalid(
                    "<not> takes exactly one argument; MEF has no negation of a formula, \
                     only of an event"
                        .into(),
                ));
            }
            Ok(FormulaArg::Event {
                name: resolve_reference(inner, scope, index)?,
                complement: true,
            })
        }
        "event" | "gate" | "basic-event" | "house-event" => Ok(FormulaArg::Event {
            name: resolve_reference(element, scope, index)?,
            complement: false,
        }),
        other => Err(invalid(format!(
            "<{other}> is not a formula argument. MEF arguments are an event reference, a \
             <not> around one, or a <constant>; a connective cannot be an argument, so \
             formulas do not nest"
        ))),
    }
}

/// Resolves an event reference to the id of the declaration it names.
///
/// `<event>` may name a gate, a basic event or a house event, so it goes
/// through upstream's `GetEvent`; the three typed reference elements go
/// through `GetEntity` on their own table, which is what makes
/// `<gate name="X"/>` an error when `X` is a basic event. A `type` attribute
/// overrides the element name, which is how `<event name="x" type="gate"/>`
/// is written.
fn resolve_reference(element: &Element, scope: &Scope, index: &Index) -> Result<String> {
    let name = element.need_attr("name")?;
    let kind = match element.attr("type") {
        Some(t) => t,
        None => element.name.as_str(),
    };
    if kind == "event" {
        index.resolve_event(name, scope)
    } else {
        index.resolve_typed(kind, name, scope)
    }
}

/// Reads a `<define-CCF-group>` — upstream's `Initializer::DefineCcfGroup`
/// plus `AddMember` / `AddDistribution` / `AddFactor`.
///
/// The factor level is optional in the grammar. Upstream deduces a missing one
/// as "one past the previous", starting at the model's minimum level:
///
/// ```cpp
/// if (!level) level = prev_level_ ? (prev_level_ + 1) : min_level;
/// ```
fn read_ccf_group(element: &Element, scope: &Scope, index: &Index) -> Result<CcfGroup> {
    let name = element.need_attr("name")?;
    let model = CcfModel::parse(element.need_attr("model")?)?;
    let mut members: Vec<String> = Vec::new();
    let mut distribution: Option<Expression> = None;
    let mut factor_elements: Vec<&Element> = Vec::new();

    for child in element.structural() {
        match child.name.as_str() {
            "members" => {
                for m in child.structural() {
                    // A DECLARATION, not a reference: see the first pass.
                    let id = scope.id(m.need_attr("name")?);
                    if members.contains(&id) {
                        return Err(invalid(format!("CCF group `{name}` names `{id}` twice")));
                    }
                    members.push(id);
                }
            }
            "distribution" => {
                let e = child.structural().next().ok_or_else(|| {
                    invalid(format!("CCF group `{name}` has an empty <distribution>"))
                })?;
                distribution = Some(read_expression(e, scope, index)?);
            }
            // The grammar allows either a <factors> wrapper or bare <factor>
            // children, and upstream accepts both.
            "factors" => factor_elements.extend(child.structural()),
            "factor" => factor_elements.push(child),
            other => {
                return Err(invalid(format!(
                    "CCF group `{name}` holds <{other}>, which is not part of the group"
                )))
            }
        }
    }

    let distribution = distribution
        .ok_or_else(|| invalid(format!("CCF group `{name}` declares no <distribution>")))?;
    if members.len() < 2 {
        return Err(invalid(format!(
            "CCF group `{name}` has {} member(s); at least 2 are needed",
            members.len()
        )));
    }

    let min_level = model.min_level(members.len());
    let mut factors: Vec<(usize, Expression)> = Vec::new();
    let mut previous = 0usize;
    for f in factor_elements {
        if f.name != "factor" {
            return Err(invalid(format!(
                "CCF group `{name}` holds <{}> among its factors",
                f.name
            )));
        }
        let level = match f.attr("level") {
            Some(text) => text.parse().map_err(|_| {
                invalid(format!("CCF group `{name}` has a non-numeric factor level"))
            })?,
            None if previous != 0 => previous + 1,
            None => min_level,
        };
        if level < min_level {
            return Err(invalid(format!(
                "CCF group `{name}`: the factor level ({level}) is less than the minimum \
                 level ({min_level}) for the {} model",
                model.as_str()
            )));
        }
        if level > members.len() {
            return Err(invalid(format!(
                "CCF group `{name}`: the factor level {level} is more than the number of \
                 members ({})",
                members.len()
            )));
        }
        if factors.iter().any(|(l, _)| *l == level) {
            return Err(invalid(format!(
                "CCF group `{name}`: redefinition of the CCF factor for level {level}"
            )));
        }
        let e = f
            .structural()
            .next()
            .ok_or_else(|| invalid(format!("CCF group `{name}` has an empty <factor>")))?;
        factors.push((level, read_expression(e, scope, index)?));
        previous = level;
    }
    factors.sort_by_key(|(level, _)| *level);

    Ok(CcfGroup {
        name: scope.id(name),
        model,
        members,
        distribution,
        factors,
        base_path: scope.base_path.clone(),
        private: scope.private,
    })
}

/// Reads an expression element.
fn read_expression(element: &Element, scope: &Scope, index: &Index) -> Result<Expression> {
    let kids: Vec<&Element> = element.structural().collect();
    let sub = |i: usize| -> Result<Expression> {
        kids.get(i)
            .ok_or_else(|| {
                invalid(format!(
                    "<{}> needs at least {} arguments, found {}",
                    element.name,
                    i + 1,
                    kids.len()
                ))
            })
            .and_then(|e| read_expression(e, scope, index))
    };
    let all = || -> Result<Vec<Expression>> {
        kids.iter()
            .map(|e| read_expression(e, scope, index))
            .collect()
    };
    let num = |name: &str| -> Result<f64> {
        element
            .need_attr(name)?
            .parse()
            .map_err(|_| invalid(format!("<{}> has a non-numeric `{name}`", element.name)))
    };

    Ok(match element.name.as_str() {
        "float" | "int" => Expression::Float(num("value")?),
        "bool" => Expression::Bool(element.need_attr("value")? == "true"),
        "system-mission-time" => Expression::MissionTime,
        "parameter" => Expression::Parameter(index.resolve_typed(
            "parameter",
            element.need_attr("name")?,
            scope,
        )?),
        "exponential" => Expression::exponential(sub(0)?, sub(1)?),
        "GLM" => Expression::glm(sub(0)?, sub(1)?, sub(2)?, sub(3)?),
        "Weibull" => Expression::weibull(sub(0)?, sub(1)?, sub(2)?, sub(3)?),
        // Upstream picks PeriodicTest's flavour by arity: 4 arguments is
        // InstantRepair, 5 is InstantTest, 11 is Complete.
        "periodic-test" => match kids.len() {
            4 => Expression::periodic_test_instant_repair(sub(0)?, sub(1)?, sub(2)?, sub(3)?),
            5 => {
                Expression::periodic_test_instant_test(sub(0)?, sub(1)?, sub(2)?, sub(3)?, sub(4)?)
            }
            n => {
                return Err(invalid(format!(
                    "<periodic-test> with {n} arguments: only the 4-argument \
                     (instant repair) and 5-argument (instant test) forms are ported; \
                     upstream's 11-argument `Complete` flavour is not"
                )))
            }
        },
        // The random deviates. Upstream picks the log-normal flavour by
        // arity, exactly as it picks the periodic test's:
        // `Initializer::Extract<LognormalDeviate>`.
        "uniform-deviate" => Expression::uniform_deviate(sub(0)?, sub(1)?),
        "normal-deviate" => Expression::normal_deviate(sub(0)?, sub(1)?),
        "lognormal-deviate" => match kids.len() {
            3 => Expression::lognormal_deviate(sub(0)?, sub(1)?, sub(2)?),
            2 => Expression::lognormal_deviate_normal(sub(0)?, sub(1)?),
            n => {
                return Err(invalid(format!(
                    "<lognormal-deviate> with {n} arguments: MEF has a three-argument \
                     (mean, error factor, level) and a two-argument (mu, sigma) form"
                )))
            }
        },
        "gamma-deviate" => Expression::gamma_deviate(sub(0)?, sub(1)?),
        "beta-deviate" => Expression::beta_deviate(sub(0)?, sub(1)?),
        // `<histogram>` is one expression -- the first boundary -- followed by
        // one `<bin>` per interval, each holding its upper boundary and its
        // weight. Upstream's `Initializer::Extract<Histogram>` splits them the
        // same way.
        "histogram" => {
            let first = kids
                .first()
                .ok_or_else(|| invalid("<histogram> has no lower boundary".into()))?;
            let mut boundaries = vec![read_expression(first, scope, index)?];
            let mut weights = Vec::new();
            for bin in &kids[1..] {
                if bin.name != "bin" {
                    return Err(invalid(format!(
                        "<histogram> holds <{}>, expected <bin>",
                        bin.name
                    )));
                }
                let parts: Vec<&Element> = bin.structural().collect();
                if parts.len() != 2 {
                    return Err(invalid(format!(
                        "<bin> takes an upper boundary and a weight, found {} children",
                        parts.len()
                    )));
                }
                boundaries.push(read_expression(parts[0], scope, index)?);
                weights.push(read_expression(parts[1], scope, index)?);
            }
            if weights.is_empty() {
                return Err(invalid("<histogram> has no <bin>".into()));
            }
            Expression::Histogram {
                boundaries,
                weights,
            }
        }
        "add" => Expression::Add(all()?),
        "sub" => Expression::Sub(all()?),
        "mul" => Expression::Mul(all()?),
        "div" => Expression::Div(all()?),
        "neg" => Expression::Neg(Arc::new(sub(0)?)),
        "abs" => Expression::Abs(Arc::new(sub(0)?)),
        "min" => Expression::Min(all()?),
        "max" => Expression::Max(all()?),
        "mean" => Expression::Mean(all()?),
        "exp" => Expression::Exp(Arc::new(sub(0)?)),
        "log" => Expression::Log(Arc::new(sub(0)?)),
        "log10" => Expression::Log10(Arc::new(sub(0)?)),
        "pow" => Expression::Pow(Arc::new(sub(0)?), Arc::new(sub(1)?)),
        "sqrt" => Expression::Sqrt(Arc::new(sub(0)?)),
        "mod" => Expression::Mod(Arc::new(sub(0)?), Arc::new(sub(1)?)),
        "trunc" | "integer" => Expression::Trunc(Arc::new(sub(0)?)),
        "round" => Expression::Round(Arc::new(sub(0)?)),
        "floor" => Expression::Floor(Arc::new(sub(0)?)),
        "ceil" => Expression::Ceil(Arc::new(sub(0)?)),
        "ite" => Expression::Ite {
            condition: Arc::new(sub(0)?),
            consequent: Arc::new(sub(1)?),
            alternate: Arc::new(sub(2)?),
        },
        "not" => Expression::Not(Arc::new(sub(0)?)),
        "and" => Expression::And(all()?),
        "or" => Expression::Or(all()?),
        "eq" => Expression::Eq(Arc::new(sub(0)?), Arc::new(sub(1)?)),
        "lt" => Expression::Lt(Arc::new(sub(0)?), Arc::new(sub(1)?)),
        "gt" => Expression::Gt(Arc::new(sub(0)?), Arc::new(sub(1)?)),
        other => {
            return Err(invalid(format!(
                "<{other}> is not an expression this reader knows. Still unread: the \
                 trigonometric functions, <switch>, the test-event conditions, and \
                 <extern-function> calls"
            )))
        }
    })
}

/// Splices `<xi:include href="...">` documents in place.
///
/// Upstream lets libxml2 do this. Only the `href` form appears in its own
/// suite; an `xpointer` is honoured only in the whole-document form it uses
/// there, and anything else is refused rather than half-applied.
fn resolve_includes(element: &mut Element, base: &Path) -> Result<()> {
    let mut replaced: Vec<Element> = Vec::new();
    for child in std::mem::take(&mut element.children) {
        if child.name == "include" {
            let href = child.need_attr("href")?;
            if let Some(pointer) = child.attr("xpointer") {
                if !pointer.contains("/opsa-mef/*") {
                    return Err(invalid(format!(
                        "<xi:include xpointer=\"{pointer}\"> selects a subset this reader \
                         does not implement; only the whole-document form is handled"
                    )));
                }
            }
            let path: PathBuf = base.join(href);
            let source = std::fs::read_to_string(&path)
                .map_err(|e| invalid(format!("including {}: {e}", path.display())))?;
            let mut included = parse_xml(&source)?;
            let inner_base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            resolve_includes(&mut included, &inner_base)?;
            // The included document's root is <opsa-mef>; its children join
            // the including element, which is what `xpointer(/opsa-mef/*)`
            // means.
            replaced.extend(included.children);
        } else {
            let mut child = child;
            resolve_includes(&mut child, base)?;
            replaced.push(child);
        }
    }
    element.children = replaced;
    Ok(())
}
