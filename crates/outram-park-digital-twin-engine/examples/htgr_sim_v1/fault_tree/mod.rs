//! **The Fault tree tab's model** (gh:#1719, #1720): a DRAFT HTR-10
//! small-break LOCA tree in Open-PSA MEF, read and quantified by
//! `raffles::scram`, and its conversion to `bugis::fault_tree`'s plain
//! diagram.
//!
//! **DRAFT: top-event definition pending Adolphus Lye (#1719).** A research
//! and teaching model (maintainer, 2026-11-23: "U can frame it as research"),
//! **not a PSA** of the HTR-10 or of any facility. Every basic event's number
//! is labelled in the MEF file: `DUMMY` (a placeholder, not data) or
//! `GENERIC-LWR` (open literature for light-water reactors, borrowed). See
//! `sbloca_draft.xml`'s header and `References.md` beside it.
//!
//! # What lives where
//!
//! - `raffles` stays GUI-free and physics-free: it reads the XML
//!   ([`raffles::scram::mef`]) and does every piece of quantification, the
//!   BDD top-event probability ([`Bdd::probability`]), the minimal cut sets
//!   ([`zbdd::minimal_cut_sets`]), cut-set probabilities
//!   ([`cut_set_probability`]) and the importance factors
//!   ([`importance_factors_from_bdd`]). **No quantification code is written
//!   here.**
//! - `bugis` draws a plain [`Diagram`]; [`diagram`] below converts to it.
//! - This module holds what is the simulator's own: the model file, the
//!   CV/component link of each basic event, the break-size range, the
//!   reader's probability overrides.
//!
//! # The break-size range
//!
//! Each break event is gated by a house event `IN_SIZE_<x>` whose MEF
//! attributes give the band of equivalent break diameters `(d_min, d_max]`
//! that event covers. [`FaultTreeStudy::tree`] sets every such house event
//! true exactly when its band **overlaps** the selected
//! [`BreakSizeRange`]: any overlap counts the whole event, which is
//! conservative (it never under-counts a partly covered band), and needs no
//! size distribution inside a band, which nothing in the corpus supplies.
//! A **size-dependent frequency** was the alternative considered and not
//! taken for the same reason: splitting one event's rate over sizes needs a
//! distribution that would itself be a DUMMY. DRAFT; the bounds are a
//! question for Adolphus on #1719.
//!
//! # Cost, and the UI thread
//!
//! The draft tree has 13 basic events and 16 gates. Rebuilding it
//! ([`FaultTreeStudy::tree`]) and quantifying it ([`quantify`]: BDD, ZBDD
//! cut sets, five importance factors per event) is pure arithmetic on a
//! handful of nodes; `quantify_is_well_inside_a_frame` measures it. The tab
//! therefore runs it on the UI thread, **only when an input changed**, not
//! every frame; a tree large enough to take a frame would move to the
//! physics worker per the no-lag rule.

use bugis::fault_tree::{Diagram, Node, Symbol};
use raffles::scram::bdd::Bdd;
use raffles::scram::fault_tree::{Arg, Connective, FaultTreeModel};
use raffles::scram::importance::{importance_factors_from_bdd, ImportanceFactors};
use raffles::scram::mef::{parse_xml, Element, FormulaArg, MefModel};
use raffles::scram::probability::cut_set_probability;
use raffles::scram::zbdd;
use uom::si::f64::Length;
use uom::si::length::millimeter;

/// The DRAFT SB-LOCA model, compiled in so the browser build has it too.
pub const SBLOCA_DRAFT_XML: &str = include_str!("sbloca_draft.xml");

/// The reference sample: upstream SCRAM's own `Theatre` model (OpenFTA's
/// operating-theatre power supply), the file committed under
/// `reference-data/scram/upstream-input/`. Kept beside the draft because
/// SCRAM's own answers for it are on file (`reference-data/scram/
/// oracle.txt`), so the tab's whole chain (reader, quantification,
/// importance, drawing) is checked against upstream on it.
pub const THEATRE_XML: &str = include_str!("../../../../../reference-data/scram/upstream-input/Theatre/theatre.xml");

/// The label every view of the draft carries.
pub const DRAFT_LABEL: &str = "DRAFT: top-event definition pending Adolphus Lye (#1719)";
/// The label of the reference sample.
pub const SAMPLE_LABEL: &str = "Sample tree (SCRAM 'Theatre'), not the HTR-10 SB-LOCA tree";

/// Which model the tab shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ModelChoice {
    /// The DRAFT HTR-10 small-break LOCA tree.
    #[default]
    SblocaDraft,
    /// SCRAM's `Theatre` sample, the reference the chain is checked on.
    TheatreSample,
}

impl ModelChoice {
    /// Both, in menu order.
    pub const ALL: [ModelChoice; 2] = [ModelChoice::SblocaDraft, ModelChoice::TheatreSample];

    /// Menu text.
    pub fn label(self) -> &'static str {
        match self {
            ModelChoice::SblocaDraft => "HTR-10 SB-LOCA (DRAFT)",
            ModelChoice::TheatreSample => "Reference: SCRAM Theatre sample",
        }
    }

    /// The banner the tab draws over the diagram.
    pub fn banner(self) -> &'static str {
        match self {
            ModelChoice::SblocaDraft => DRAFT_LABEL,
            ModelChoice::TheatreSample => SAMPLE_LABEL,
        }
    }

    fn xml(self) -> &'static str {
        match self {
            ModelChoice::SblocaDraft => SBLOCA_DRAFT_XML,
            ModelChoice::TheatreSample => THEATRE_XML,
        }
    }

    fn top(self) -> &'static str {
        match self {
            ModelChoice::SblocaDraft => "SB_LOCA",
            ModelChoice::TheatreSample => "Theatre",
        }
    }
}

/// Where a basic event's number comes from (its MEF `data` attribute).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataKind {
    /// A placeholder, not data.
    Dummy,
    /// Open literature for light-water reactors, borrowed.
    GenericLwr,
    /// No `data` attribute (the upstream sample's own numbers).
    Unlabelled,
}

impl DataKind {
    /// The tag drawn beside the number, empty when unlabelled.
    pub fn tag(self) -> &'static str {
        match self {
            DataKind::Dummy => "DUMMY",
            DataKind::GenericLwr => "GENERIC-LWR",
            DataKind::Unlabelled => "",
        }
    }
}

/// A basic event's documentation, read from its MEF `<attributes>`.
#[derive(Clone, Debug, PartialEq)]
pub struct EventInfo {
    pub name: String,
    /// The pressure-boundary component id (`physics::control_volumes`).
    pub component: Option<String>,
    /// The control-volume id it sits on.
    pub cv: Option<String>,
    pub data: DataKind,
    pub source: String,
}

/// One break event's size band, read from its `IN_SIZE_<x>` house event.
#[derive(Clone, Debug, PartialEq)]
pub struct SizeBand {
    /// The house event's name.
    pub house: String,
    /// Smallest equivalent break diameter the event covers (exclusive).
    pub d_min: Length,
    /// Largest equivalent break diameter the event covers (inclusive).
    pub d_max: Length,
    pub source: String,
}

/// The selected SB-LOCA break-size range, equivalent diameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BreakSizeRange {
    pub min: Length,
    pub max: Length,
}

impl Default for BreakSizeRange {
    /// DRAFT default: 1 mm to 65 mm, the largest stated HTR-10 RPV
    /// penetration and the DN65 depressurisation of Gao and Shi (2002)
    /// s.5.3.1. The bounds are a question for Adolphus (#1719).
    fn default() -> Self {
        Self { min: Length::new::<millimeter>(1.0), max: Length::new::<millimeter>(65.0) }
    }
}

impl SizeBand {
    /// Whether this band, `(d_min, d_max]`, overlaps `range`, `[min, max]`.
    pub fn overlaps(&self, range: &BreakSizeRange) -> bool {
        self.d_min < range.max && self.d_max >= range.min
    }
}

/// A loaded model: the MEF document plus what the simulator reads from its
/// attributes.
#[derive(Clone, Debug)]
pub struct FaultTreeStudy {
    pub choice: ModelChoice,
    mef: MefModel,
    pub events: Vec<EventInfo>,
    pub bands: Vec<SizeBand>,
}

impl FaultTreeStudy {
    /// Read `choice`'s MEF document.
    pub fn load(choice: ModelChoice) -> Result<Self, String> {
        let root = parse_xml(choice.xml()).map_err(|e| e.to_string())?;
        let mef = MefModel::from_element(&root).map_err(|e| e.to_string())?;
        let mut events = Vec::new();
        let mut bands = Vec::new();
        collect_metadata(&root, &mut events, &mut bands);
        Ok(Self { choice, mef, events, bands })
    }

    /// The tree for `range`, with `count_sg_tube_rupture` setting the house
    /// event of that name where the model has one: the MEF model with its
    /// `IN_SIZE_*` house events set from the bands (see the module docs),
    /// lowered by [`MefModel::fault_tree`].
    pub fn tree(&self, range: &BreakSizeRange, count_sg_tube_rupture: bool) -> Result<FaultTreeModel, String> {
        let mut mef = self.mef.clone();
        for band in &self.bands {
            mef.house_events.insert(band.house.clone(), band.overlaps(range));
        }
        if mef.house_events.contains_key("COUNT_SG_TUBE_RUPTURE") {
            mef.house_events.insert("COUNT_SG_TUBE_RUPTURE".to_string(), count_sg_tube_rupture);
        }
        mef.fault_tree(self.choice.top()).map_err(|e| e.to_string())
    }

    /// The house-event values [`Self::tree`] uses for `range`, by name: for
    /// labelling the diagram.
    pub fn house_values(&self, range: &BreakSizeRange, count_sg_tube_rupture: bool) -> Vec<(String, bool)> {
        let mut out: Vec<(String, bool)> = self
            .mef
            .house_events
            .iter()
            .map(|(name, value)| {
                let v = match self.bands.iter().find(|b| &b.house == name) {
                    Some(b) => b.overlaps(range),
                    None if name == "COUNT_SG_TUBE_RUPTURE" => count_sg_tube_rupture,
                    None => *value,
                };
                (name.clone(), v)
            })
            .collect();
        out.sort();
        out
    }

    /// The documentation of basic event `name`.
    pub fn event(&self, name: &str) -> Option<&EventInfo> {
        self.events.iter().find(|e| e.name == name)
    }

    /// The names of the house events each gate references, in argument
    /// order: how the diagram names the constants the lowering left.
    fn gate_house_args(&self, gate: &str) -> Vec<String> {
        match self.mef.gates.get(gate) {
            Some(f) => f
                .args
                .iter()
                .filter_map(|a| match a {
                    FormulaArg::Event { name, .. } if self.mef.house_events.contains_key(name) => Some(name.clone()),
                    _ => None,
                })
                .collect(),
            None => Vec::new(),
        }
    }
}

/// Walk the document for `<define-basic-event>` and `<define-house-event>`
/// attributes.
fn collect_metadata(element: &Element, events: &mut Vec<EventInfo>, bands: &mut Vec<SizeBand>) {
    let attr = |e: &Element, key: &str| -> Option<String> {
        e.children
            .iter()
            .filter(|c| c.name == "attributes")
            .flat_map(|c| c.children.iter())
            .find(|a| a.name == "attribute" && a.attr("name") == Some(key))
            .and_then(|a| a.attr("value"))
            .map(str::to_string)
    };
    match element.name.as_str() {
        "define-basic-event" => {
            let data = match attr(element, "data").as_deref() {
                Some("DUMMY") => DataKind::Dummy,
                Some("GENERIC-LWR") => DataKind::GenericLwr,
                _ => DataKind::Unlabelled,
            };
            events.push(EventInfo {
                name: element.attr("name").unwrap_or_default().to_string(),
                component: attr(element, "component"),
                cv: attr(element, "cv"),
                data,
                source: attr(element, "source").unwrap_or_default(),
            });
        }
        "define-house-event" => {
            let mm = |k: &str| attr(element, k).and_then(|v| v.parse::<f64>().ok()).map(Length::new::<millimeter>);
            if let (Some(d_min), Some(d_max)) = (mm("d_min_mm"), mm("d_max_mm")) {
                bands.push(SizeBand {
                    house: element.attr("name").unwrap_or_default().to_string(),
                    d_min,
                    d_max,
                    source: attr(element, "band_source").unwrap_or_default(),
                });
            }
        }
        _ => {}
    }
    for c in &element.children {
        collect_metadata(c, events, bands);
    }
}

/// One minimal cut set and its probability.
#[derive(Clone, Debug, PartialEq)]
pub struct RankedCutSet {
    /// Basic-event indices into the model's `basic_event_names`.
    pub members: Vec<usize>,
    pub probability: f64,
}

/// Everything the tab shows about one tree at one set of probabilities.
#[derive(Clone, Debug)]
pub struct Quantified {
    /// Exact top-event probability from the BDD; `0` when no event is in.
    pub top_probability: f64,
    /// Minimal cut sets, most probable first.
    pub cut_sets: Vec<RankedCutSet>,
    /// Importance factors per basic event (BDD route); `None` when the top
    /// event is impossible, where every factor divides by zero.
    pub importance: Vec<Option<ImportanceFactors>>,
    /// Whether each basic event appears in some minimal cut set, i.e. counts
    /// toward the top event at the current break-size range.
    pub included: Vec<bool>,
}

/// Quantify `model` at `probabilities` (one per basic event, in
/// `model.basic_event_names()` order), with raffles' own functions only.
pub fn quantify(model: &FaultTreeModel, probabilities: &[f64]) -> Result<Quantified, String> {
    let s = |e: raffles::RafflesError| e.to_string();
    let bdd = Bdd::build(model.tree()).map_err(s)?;
    let top_probability = bdd.probability(probabilities).map_err(s)?;
    let mut cut_sets: Vec<RankedCutSet> = zbdd::minimal_cut_sets(model.tree(), None)
        .map_err(s)?
        .iter()
        .map(|c| Ok(RankedCutSet { members: c.members().to_vec(), probability: cut_set_probability(c, probabilities)? }))
        .collect::<Result<_, raffles::RafflesError>>()
        .map_err(s)?;
    cut_sets.sort_by(|a, b| b.probability.total_cmp(&a.probability).then_with(|| a.members.cmp(&b.members)));
    let n = model.basic_event_names().len();
    let importance = (0..n)
        .map(|i| if top_probability > 0.0 { importance_factors_from_bdd(i, &bdd, probabilities).ok() } else { None })
        .collect();
    let included = (0..n).map(|i| cut_sets.iter().any(|c| c.members.contains(&i))).collect();
    Ok(Quantified { top_probability, cut_sets, importance, included })
}

/// Format a probability for the diagram and tables: three significant
/// figures in scientific notation, or `0`.
pub fn format_probability(p: f64) -> String {
    if p == 0.0 {
        "0".to_string()
    } else {
        format!("{p:.2e}")
    }
}

/// The diagram `bugis` draws: gates first (node `g` is gate `g`), then the
/// basic events, then one node per constant argument (the house events the
/// lowering replaced by `true`/`false`, named from the MEF formula where the
/// argument order allows). Basic events carry their probability and data
/// tag as the detail line; the top gate carries the top-event probability.
pub fn diagram(study: &FaultTreeStudy, model: &FaultTreeModel, probabilities: &[f64], top_probability: f64) -> Diagram {
    let tree = model.tree();
    let gates = tree.gates();
    let n_gates = gates.len();
    let mut nodes: Vec<Node> = Vec::new();
    for (g, gate) in gates.iter().enumerate() {
        let symbol = match gate.connective() {
            Connective::And => Symbol::And,
            Connective::Or => Symbol::Or,
            Connective::Atleast { min } => Symbol::AtLeast(min),
            Connective::Xor => Symbol::Xor,
            Connective::Not => Symbol::Not,
            Connective::Nand => Symbol::Nand,
            Connective::Nor => Symbol::Nor,
            Connective::Null => Symbol::Null,
        };
        let detail = if g == tree.top() { format!("P = {}", format_probability(top_probability)) } else { String::new() };
        nodes.push(Node { label: model.gate_names()[g].clone(), detail, symbol, children: Vec::new() });
    }
    for (b, name) in model.basic_event_names().iter().enumerate() {
        let tag = study.event(name).map_or("", |e| e.data.tag());
        let p = probabilities.get(b).copied().unwrap_or(f64::NAN);
        let detail = if tag.is_empty() { format_probability(p) } else { format!("{} {tag}", format_probability(p)) };
        nodes.push(Node { label: name.clone(), detail, symbol: Symbol::BasicEvent, children: Vec::new() });
    }
    for (g, gate) in gates.iter().enumerate() {
        let houses = study.gate_house_args(&model.gate_names()[g]);
        let mut k = 0;
        let mut children = Vec::new();
        for arg in gate.args() {
            match *arg {
                Arg::Gate(i) => children.push(i),
                Arg::BasicEvent(i) => children.push(n_gates + i),
                Arg::Constant(v) => {
                    let label = houses.get(k).cloned().unwrap_or_else(|| if v { "TRUE".into() } else { "FALSE".into() });
                    k += 1;
                    children.push(nodes.len());
                    nodes.push(Node {
                        label,
                        detail: if v { "true".into() } else { "false".into() },
                        symbol: Symbol::HouseEvent(v),
                        children: Vec::new(),
                    });
                }
            }
        }
        nodes[g].children = children;
    }
    Diagram { nodes, root: tree.top() }
}

/// The basic-event index a diagram node draws, if it is one.
pub fn node_event(model: &FaultTreeModel, node: usize) -> Option<usize> {
    let n_gates = model.tree().gates().len();
    node.checked_sub(n_gates).filter(|&b| b < model.basic_event_names().len())
}

/// The diagram node of basic event `event`.
pub fn event_node(model: &FaultTreeModel, event: usize) -> usize {
    model.tree().gates().len() + event
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::control_volumes::control_volume_map;
    use crate::physics::HtgrPlant;

    fn mm(x: f64) -> Length {
        Length::new::<millimeter>(x)
    }

    /// THE VERIFICATION LINK (gh:#1719): every basic event of the draft names
    /// a pressure-boundary component and a CV, both on the simulator's own
    /// control-volume list from the assembled plant, and the CV is one the
    /// component sits on. Every event is labelled DUMMY or GENERIC-LWR.
    #[test]
    fn every_basic_event_sits_on_a_listed_cv_and_component() {
        let study = FaultTreeStudy::load(ModelChoice::SblocaDraft).unwrap();
        let map = control_volume_map(&HtgrPlant::new());
        let model = study.tree(&BreakSizeRange::default(), true).unwrap();
        assert_eq!(model.basic_event_names().len(), study.events.len(), "every event is reachable");
        for e in &study.events {
            let component = e.component.as_deref().unwrap_or_else(|| panic!("{} names no component", e.name));
            let cv = e.cv.as_deref().unwrap_or_else(|| panic!("{} names no CV", e.name));
            let c = map
                .boundary
                .iter()
                .find(|b| b.id == component)
                .unwrap_or_else(|| panic!("{}: component {component} is not on the CV list", e.name));
            assert!(map.get(cv).is_some(), "{}: CV {cv} is not on the CV list", e.name);
            assert!(c.cvs.iter().any(|x| x == cv), "{}: {component} does not sit on {cv}", e.name);
            assert_ne!(e.data, DataKind::Unlabelled, "{} has no data label", e.name);
            assert!(!e.source.is_empty(), "{} has no source", e.name);
        }
        assert_eq!(study.bands.len(), 10);
    }

    /// The reference sample through the whole chain against SCRAM's own
    /// answers (`reference-data/scram/oracle.txt`, model Theatre/theatre):
    /// exact total 0.00207, products {Gen_Fail, Mains_Fail} and {Relay_Fail,
    /// Mains_Fail}, MIF 0.069 / 0.0285 / 0.0294, RAW 33.3333 / 14.4928 /
    /// 14.4928 (SCRAM prints 6 significant figures, hence the tolerance).
    #[test]
    fn the_theatre_sample_matches_scram() {
        let study = FaultTreeStudy::load(ModelChoice::TheatreSample).unwrap();
        let model = study.tree(&BreakSizeRange::default(), false).unwrap();
        let q = quantify(&model, model.probabilities()).unwrap();
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-4 * b.abs();
        assert!(close(q.top_probability, 0.00207), "{}", q.top_probability);
        let names = model.basic_event_names();
        let mut sets: Vec<Vec<&str>> = q
            .cut_sets
            .iter()
            .map(|c| {
                let mut v: Vec<&str> = c.members.iter().map(|&i| names[i].as_str()).collect();
                v.sort();
                v
            })
            .collect();
        sets.sort();
        assert_eq!(sets, vec![vec!["Gen_Fail", "Mains_Fail"], vec!["Mains_Fail", "Relay_Fail"]]);
        for (name, mif, raw) in [("Mains_Fail", 0.069, 33.3333), ("Gen_Fail", 0.0285, 14.4928), ("Relay_Fail", 0.0294, 14.4928)] {
            let i = model.basic_event_index(name).unwrap();
            let f = q.importance[i].unwrap();
            assert!(close(f.mif, mif) && close(f.raw, raw), "{name}: {f:?}");
        }
        // The diagram: 2 gates and 3 events, no constants, root at the AND.
        let d = diagram(&study, &model, model.probabilities(), q.top_probability);
        assert_eq!(d.nodes.len(), 5);
        assert_eq!(d.nodes[d.root].symbol, Symbol::And);
        assert!(d.nodes[d.root].detail.starts_with("P = 2.07e-3"));
        assert_eq!(node_event(&model, event_node(&model, 2)), Some(2));
        assert_eq!(node_event(&model, 0), None);
    }

    /// The break-size range decides which events count: the default range
    /// counts every event except SG tube rupture (house event off); raising
    /// the lower bound past the 11.28 mm leak class drops the vessel and
    /// housing leaks and the instrument lines; past 65 mm nothing is left
    /// and the top event is impossible. Moving back in brings them back.
    #[test]
    fn the_break_size_range_changes_the_included_events() {
        let study = FaultTreeStudy::load(ModelChoice::SblocaDraft).unwrap();
        let included = |range: BreakSizeRange, sgtr: bool| -> (Vec<String>, f64) {
            let model = study.tree(&range, sgtr).unwrap();
            let q = quantify(&model, model.probabilities()).unwrap();
            let names = model
                .basic_event_names()
                .iter()
                .zip(&q.included)
                .filter(|(_, &inc)| inc)
                .map(|(n, _)| n.clone())
                .collect();
            (names, q.top_probability)
        };
        let (all, p_all) = included(BreakSizeRange::default(), false);
        assert_eq!(all.len(), 12, "{all:?}");
        assert!(!all.contains(&"SG_TUBE_RUPTURE".to_string()));
        let (with_sgtr, p_sgtr) = included(BreakSizeRange::default(), true);
        assert_eq!(with_sgtr.len(), 13);
        assert!(p_sgtr > p_all);

        let (large, p_large) = included(BreakSizeRange { min: mm(20.0), max: mm(65.0) }, true);
        assert_eq!(
            large,
            ["CRD_NOZZLE_BREAK", "FDP_BREAK", "FLT_BREAK", "SV1_FAILS_TO_RESEAT", "SV1_SPURIOUS_OPEN", "SV2_FAILS_TO_RESEAT", "SV2_SPURIOUS_OPEN"]
        );
        assert!(p_large < p_all);

        let (none, p_none) = included(BreakSizeRange { min: mm(70.0), max: mm(200.0) }, true);
        assert!(none.is_empty());
        assert_eq!(p_none, 0.0);
        let model = study.tree(&BreakSizeRange { min: mm(70.0), max: mm(200.0) }, true).unwrap();
        let q = quantify(&model, model.probabilities()).unwrap();
        assert!(q.importance.iter().all(Option::is_none), "no ranking when nothing can happen");

        assert_eq!(included(BreakSizeRange::default(), false), (all, p_all), "moving back restores it");

        // House values for labelling follow the same rule.
        let houses = study.house_values(&BreakSizeRange { min: mm(20.0), max: mm(65.0) }, false);
        assert!(houses.contains(&("IN_SIZE_FLT".to_string(), true)));
        assert!(houses.contains(&("IN_SIZE_RPV_SHELL".to_string(), false)));
        assert!(houses.contains(&("COUNT_SG_TUBE_RUPTURE".to_string(), false)));
    }

    /// Changing one probability re-quantifies: the top probability, the
    /// cut-set ranking and the importance move with it, from the same tree.
    /// Diagram constants carry their house-event names.
    #[test]
    fn a_probability_override_requantifies() {
        let study = FaultTreeStudy::load(ModelChoice::SblocaDraft).unwrap();
        let model = study.tree(&BreakSizeRange::default(), false).unwrap();
        let base = quantify(&model, model.probabilities()).unwrap();
        let i = model.basic_event_index("INSTR_LINE_BREAK").unwrap();
        let mut p = model.probabilities().to_vec();
        p[i] = 0.5;
        let raised = quantify(&model, &p).unwrap();
        assert!(raised.top_probability > base.top_probability);
        assert_eq!(raised.cut_sets[0].members, vec![i], "now the dominant cut set");
        assert!(raised.importance[i].unwrap().cif > base.importance[i].unwrap().cif);
        // Order-2 cut sets exist (the safety valves).
        assert!(base.cut_sets.iter().any(|c| c.members.len() == 2));

        let d = diagram(&study, &model, &p, raised.top_probability);
        assert!(d.nodes.iter().any(|n| n.label == "IN_SIZE_FLT" && n.symbol == Symbol::HouseEvent(true)));
        assert!(d.nodes.iter().any(|n| n.label == "COUNT_SG_TUBE_RUPTURE" && n.symbol == Symbol::HouseEvent(false)));
        let instr = &d.nodes[event_node(&model, i)];
        assert_eq!(instr.detail, "5.00e-1 DUMMY");
        assert_eq!(format_probability(0.0), "0");
        assert_eq!(DataKind::GenericLwr.tag(), "GENERIC-LWR");
        assert_eq!(ModelChoice::ALL.map(|c| c.banner()), [DRAFT_LABEL, SAMPLE_LABEL]);
        assert!(ModelChoice::SblocaDraft.label().contains("DRAFT"));
        // The layout bugis gives it is well formed.
        let l = bugis::fault_tree::layout(&d);
        assert_eq!(l.placed[0].node, d.root);
        assert!(l.leaf_count >= model.basic_event_names().len());
    }

    /// Rebuild plus quantify is far inside one 16 ms frame, so the tab may
    /// do it on the UI thread when an input changes (module docs). Measured
    /// 2026-11-23 in release: see the hand-off; the bound here is loose.
    #[test]
    fn quantify_is_well_inside_a_frame() {
        let study = FaultTreeStudy::load(ModelChoice::SblocaDraft).unwrap();
        let start = std::time::Instant::now();
        let runs = 20;
        for _ in 0..runs {
            let model = study.tree(&BreakSizeRange::default(), true).unwrap();
            let _ = quantify(&model, model.probabilities()).unwrap();
        }
        let per_run = start.elapsed().as_secs_f64() / runs as f64;
        eprintln!("fault tree rebuild + quantify: {:.3} ms per run", 1e3 * per_run);
        assert!(per_run < 0.016, "{per_run} s");
    }
}
