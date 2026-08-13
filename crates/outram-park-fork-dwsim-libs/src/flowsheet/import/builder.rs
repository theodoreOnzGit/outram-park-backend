//! Translation of a parsed DWSIM document into a [`Flowsheet`].
//!
//! This is the half of the importer that knows what DWSIM's elements *mean*.
//! [`super::xml`] turns bytes into a tree; this module walks that tree in one
//! forward pass and builds the flowsheet, recording everything it could not
//! translate as an [`ImportGap`] instead of failing or dropping it silently.
//!
//! # Order of work
//!
//! 1. `<Compounds>` — build the compound-name -> molar-mass \[kg/kmol\] table
//!    that every material stream needs.
//! 2. `<PropertyPackages>` — record each package and whether this crate
//!    implements it.
//! 3. `<GraphicObjects>` — index by object name to recover each object's user
//!    tag and its connector wiring. **No geometry is read.**
//! 4. `<SimulationObjects>` — create one flowsheet object per entry, then fill
//!    in stream state.
//! 5. Wire the connections recovered in step 3, skipping edges whose endpoints
//!    were not imported.
//!
//! # Format documentation
//!
//! Element meanings were documented by reading **DWSIM** at commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`, GPL-3.0),
//! specifically `DWSIM.FlowsheetBase/FlowsheetBase.vb` (`LoadFromXML`'s
//! graphic-object and connection passes, :4816-4892),
//! `DWSIM.Drawing.SkiaSharp/GraphicObjects/Base/GraphicObject.vb`
//! (`SaveData`, :193-262 — the connector serialisation),
//! `DWSIM.Thermodynamics/MaterialStream/MaterialStream.vb`
//! (`SaveData`/`LoadData`, :138-181) and `DWSIM.Interfaces/Enums.vb`. This is
//! original code documenting that format; it is deliberately **not** a port of
//! DWSIM's loader, which is excluded from this crate's scope.

use std::collections::{BTreeMap, HashMap, HashSet};

use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::catalytic_activity::katal;
use uom::si::f64::{
    AvailableEnergy, MassRate, Power, Pressure, Ratio, SpecificHeatCapacity,
    ThermodynamicTemperature, VolumeRate,
};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::kilowatt;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volume_rate::cubic_meter_per_second;

use crate::flowsheet::graph::Flowsheet;
use crate::flowsheet::objects::{ObjectData, ObjectId, ObjectType};
use crate::flowsheet::streams::{
    MaterialStreamData, MolarFlowRate, PhaseIndex, PhaseProperties, StreamCompound,
};

use super::mapping::{
    composition_basis_from_name, flow_spec_from_name, forced_phase_from_name,
    object_type_from_class_name, object_type_from_enum_name, property_package_from_class_name,
    stream_spec_from_name,
};
use super::xml::XmlNode;
use super::{
    ImportError, ImportGap, ImportedCompound, ImportedFlowsheet, ImportedPropertyPackage,
    SourceKind,
};

/// The element name DWSIM writes as the root of every saved flowsheet.
pub const ROOT_ELEMENT: &str = "DWSIM_Simulation_Data";

/// One connection recovered from the graphic objects, before it is offered to
/// [`Flowsheet::connect`].
///
/// Owned by value; endpoints are identities, never references.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RawEdge {
    from: ObjectId,
    to: ObjectId,
    /// Index of the outlet slot on `from`, or `None` when the edge leaves
    /// `from`'s dedicated energy connector (which [`Flowsheet::connect`]
    /// resolves for itself).
    from_slot: Option<usize>,
    /// Index of the inlet slot on `to`, from `AttachedToConnIndex`.
    to_slot: Option<usize>,
}

/// Build an [`ImportedFlowsheet`] from a parsed document.
///
/// # Errors
/// [`ImportError::NotADwsimDocument`] if the root element is not
/// [`ROOT_ELEMENT`]. Everything else short of that degrades into an
/// [`ImportGap`] — an importable file never fails here.
pub fn build(root: &XmlNode, source: SourceKind) -> Result<ImportedFlowsheet, ImportError> {
    if root.name != ROOT_ELEMENT {
        return Err(ImportError::NotADwsimDocument {
            root: root.name.clone(),
        });
    }

    let mut gaps: Vec<ImportGap> = Vec::new();

    let general_info = read_general_info(root);
    let compounds = read_compounds(root);
    let molar_masses: HashMap<String, f64> = compounds
        .iter()
        .filter(|c| c.molar_mass.is_finite() && c.molar_mass > 0.0)
        .map(|c| (c.name.clone(), c.molar_mass))
        .collect();
    let property_packages = read_property_packages(root, &mut gaps);

    // Graphic objects, indexed by the object identity they decorate.
    let graphics: HashMap<String, &XmlNode> = root
        .child("GraphicObjects")
        .map(|g| {
            g.children_named("GraphicObject")
                .into_iter()
                .filter_map(|n| Some((graphic_name(n)?, n)))
                .collect()
        })
        .unwrap_or_default();

    let mut flowsheet = Flowsheet::new();
    let mut raw_properties: HashMap<ObjectId, BTreeMap<String, String>> = HashMap::new();
    let mut stream_packages: HashMap<ObjectId, String> = HashMap::new();
    let mut imported: HashSet<String> = HashSet::new();

    let sim_objects = root
        .child("SimulationObjects")
        .map(|s| s.children_named("SimulationObject"))
        .unwrap_or_default();

    for node in &sim_objects {
        let dwsim_type = node.child_text("Type").unwrap_or_default().to_string();
        let name = match object_identity(node) {
            Some(n) => n,
            None => {
                gaps.push(ImportGap::UnidentifiedObject {
                    dwsim_type: dwsim_type.clone(),
                });
                continue;
            }
        };
        let graphic = graphics.get(&name).copied();

        let object_type = match resolve_object_type(&dwsim_type, graphic) {
            Some(t) => t,
            None => {
                gaps.push(ImportGap::UnsupportedObjectType {
                    dwsim_type: dwsim_type.clone(),
                    object_name: name.clone(),
                });
                continue;
            }
        };

        let requested_tag = graphic
            .and_then(|g| g.child_text("Tag"))
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| name.clone());

        let id = ObjectId(name.clone());
        if let Err(e) = flowsheet.add_object_with_id(id.clone(), object_type, Some(&requested_tag))
        {
            gaps.push(ImportGap::ObjectRejected {
                object_name: name.clone(),
                reason: e.to_string(),
            });
            continue;
        }
        imported.insert(name.clone());

        raw_properties.insert(id.clone(), scalar_leaves(node));

        // Metadata common to every object kind.
        {
            let object = flowsheet
                .object_mut(&id)
                .expect("just inserted under this id");
            if object.tag != requested_tag {
                gaps.push(ImportGap::TagRenamed {
                    original: requested_tag.clone(),
                    assigned: object.tag.clone(),
                });
            }
            object.description = node
                .child_text("ComponentDescription")
                .unwrap_or_default()
                .to_string();
            object.calculated = node.child_bool("Calculated").unwrap_or(false);
            object.dirty = !object.calculated;
            object.active = graphic.and_then(|g| g.child_bool("Active")).unwrap_or(true);
            object.error_message = node
                .child_text("ErrorMessage")
                .filter(|m| !m.is_empty())
                .map(str::to_string);
            if node.child_bool("IsSpecAttached").unwrap_or(false) {
                object.attached_spec = node
                    .child_text("AttachedSpecId")
                    .filter(|s| !s.is_empty())
                    .map(|s| ObjectId(s.to_string()));
            }
            if node.child_bool("IsAdjustAttached").unwrap_or(false) {
                object.attached_adjust = node
                    .child_text("AttachedAdjustId")
                    .filter(|s| !s.is_empty())
                    .map(|s| ObjectId(s.to_string()));
            }
        }

        match object_type {
            ObjectType::MaterialStream => {
                let data = build_material_stream(node, &requested_tag, &molar_masses, &mut gaps);
                if let Some(pp) = node
                    .child_text("PropertyPackage")
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                {
                    stream_packages.insert(id.clone(), pp);
                }
                if let Some(slot) = flowsheet.object_mut(&id) {
                    slot.data = ObjectData::Material(data);
                }
            }
            ObjectType::EnergyStream => {
                if let Some(kw) = node.child_f64("EnergyFlow") {
                    if kw.is_finite() {
                        if let Some(energy) = flowsheet
                            .object_mut(&id)
                            .and_then(|o| o.data.as_energy_mut())
                        {
                            energy.set_power(Power::new::<kilowatt>(kw));
                        }
                    }
                } else if let Some(raw) = node.child_text("EnergyFlow") {
                    if !raw.is_empty() {
                        gaps.push(ImportGap::UnparsedField {
                            object_tag: requested_tag.clone(),
                            field: "EnergyFlow".to_string(),
                            value: raw.to_string(),
                        });
                    }
                }
            }
            // Every other type becomes an `ObjectData::UnitOperation`. Its
            // stored scalars are kept verbatim in `raw_properties` rather than
            // being written into the flowsheet model — see the module docs of
            // `crate::flowsheet::import` for why `power` stays `None`.
            _ => {}
        }
    }

    // --- topology -----------------------------------------------------------
    let mut edges: Vec<RawEdge> = Vec::new();
    let mut seen: HashSet<RawEdge> = HashSet::new();
    for node in &sim_objects {
        let Some(name) = object_identity(node) else {
            continue;
        };
        if !imported.contains(&name) {
            continue;
        }
        let Some(graphic) = graphics.get(&name).copied() else {
            continue;
        };
        collect_edges(graphic, &name, &imported, &mut edges, &mut seen, &mut gaps);
    }

    for edge in edges {
        let from_tag = tag_of(&flowsheet, &edge.from);
        let to_tag = tag_of(&flowsheet, &edge.to);
        match flowsheet.connect(&edge.from, &edge.to, edge.from_slot, edge.to_slot) {
            Ok(_) => {}
            Err(exact) => match flowsheet.connect(&edge.from, &edge.to, None, None) {
                Ok(_) => gaps.push(ImportGap::ConnectionSlotFallback {
                    from_tag,
                    to_tag,
                    requested_from_slot: edge.from_slot,
                    requested_to_slot: edge.to_slot,
                    reason: exact.to_string(),
                }),
                Err(loose) => gaps.push(ImportGap::ConnectionRejected {
                    from_tag,
                    to_tag,
                    reason: loose.to_string(),
                }),
            },
        }
    }

    let drawing_objects_skipped = graphics
        .values()
        .filter(|g| !imported.contains(&graphic_name(g).unwrap_or_default()))
        .count();

    Ok(ImportedFlowsheet {
        flowsheet,
        gaps,
        source,
        general_info,
        compounds,
        property_packages,
        stream_packages,
        raw_properties,
        drawing_objects_skipped,
    })
}

/// The identity DWSIM keys an object by: `<Name>`, falling back to
/// `<ComponentName>` (older files write only the latter).
fn object_identity(node: &XmlNode) -> Option<String> {
    for field in ["Name", "ComponentName"] {
        if let Some(v) = node.child_text(field) {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// The identity a graphic object decorates (`<Name>`).
fn graphic_name(node: &XmlNode) -> Option<String> {
    node.child_text("Name")
        .filter(|n| !n.is_empty())
        .map(str::to_string)
}

/// Resolve an object's [`ObjectType`], preferring the simulation object's .NET
/// class name over the graphic object's enum member.
///
/// The class name wins because it is strictly more specific: DWSIM serialises
/// every clean-power source's graphic object as the generic `External`, so the
/// graphic alone cannot tell a wind turbine from a solar panel. `<TipoObjeto>`
/// is the pre-5.x spelling of `<ObjectType>` and is tried last.
fn resolve_object_type(dwsim_type: &str, graphic: Option<&XmlNode>) -> Option<ObjectType> {
    if let Some(t) = object_type_from_class_name(dwsim_type) {
        return Some(t);
    }
    let graphic = graphic?;
    for field in ["ObjectType", "TipoObjeto"] {
        if let Some(name) = graphic.child_text(field) {
            if let Some(t) = object_type_from_enum_name(name) {
                return Some(t);
            }
        }
    }
    None
}

/// Every direct child of `node` that is a non-empty scalar leaf, as
/// `name -> text`.
///
/// This is the imported file's *answer key*: a heater's `DeltaQ` \[kW\], a
/// pump's `DeltaP` \[Pa\], a column's stage count. The values are kept verbatim,
/// in DWSIM's internal units, precisely because they are the numbers a future
/// solver must be checked against.
fn scalar_leaves(node: &XmlNode) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for child in &node.children {
        if !child.children.is_empty() {
            continue;
        }
        let text = child.text.trim();
        if text.is_empty() {
            continue;
        }
        out.insert(child.name.clone(), text.to_string());
    }
    out
}

fn tag_of(flowsheet: &Flowsheet, id: &ObjectId) -> String {
    flowsheet
        .object(id)
        .map(|o| o.tag.clone())
        .unwrap_or_else(|| id.0.clone())
}

/// Read the `<GeneralInfo>` block as plain `name -> text` pairs.
fn read_general_info(root: &XmlNode) -> BTreeMap<String, String> {
    root.child("GeneralInfo")
        .map(scalar_leaves)
        .unwrap_or_default()
}

/// Read `<Compounds>`: the flowsheet's compound list with the one constant the
/// composition algebra needs, the molar mass \[kg/kmol\].
fn read_compounds(root: &XmlNode) -> Vec<ImportedCompound> {
    let Some(section) = root.child("Compounds") else {
        return Vec::new();
    };
    section
        .children_named("Compound")
        .into_iter()
        .filter_map(|c| {
            let name = c
                .child_text("Name")
                .filter(|n| !n.is_empty())
                .map(str::to_string)?;
            Some(ImportedCompound {
                name,
                molar_mass: c.child_f64("Molar_Weight").unwrap_or(f64::NAN),
                cas_number: c.child_text("CAS_Number").unwrap_or_default().to_string(),
                formula: c.child_text("Formula").unwrap_or_default().to_string(),
            })
        })
        .collect()
}

/// Read `<PropertyPackages>`, recording an
/// [`ImportGap::UnsupportedPropertyPackage`] for each package this crate does
/// not implement.
fn read_property_packages(
    root: &XmlNode,
    gaps: &mut Vec<ImportGap>,
) -> Vec<ImportedPropertyPackage> {
    let Some(section) = root.child("PropertyPackages") else {
        return Vec::new();
    };
    section
        .children_named("PropertyPackage")
        .into_iter()
        .map(|p| {
            let dwsim_type = p.child_text("Type").unwrap_or_default().to_string();
            let id = p.child_text("ID").unwrap_or_default().to_string();
            let model = property_package_from_class_name(&dwsim_type);
            if model.is_none() {
                gaps.push(ImportGap::UnsupportedPropertyPackage {
                    dwsim_type: dwsim_type.clone(),
                    package_id: id.clone(),
                });
            }
            ImportedPropertyPackage {
                id,
                tag: p.child_text("Tag").unwrap_or_default().to_string(),
                dwsim_type,
                model,
            }
        })
        .collect()
}

/// Collect the outgoing edges recorded on one graphic object.
///
/// DWSIM stores each edge twice — once on the source's `OutputConnectors` and
/// once on the destination's `InputConnectors` — so reading only the outgoing
/// side (as DWSIM's own loader does, FlowsheetBase.vb:4840-4892) recovers the
/// whole graph exactly once. `<EnergyConnector>` is the separate, dedicated
/// duty outlet and is read the same way.
///
/// A connector block is written more than once for objects whose base and
/// derived classes both serialise it; the last block is used (matching .NET's
/// last-wins deserialisation) and identical duplicates are dropped by `seen`.
fn collect_edges(
    graphic: &XmlNode,
    from_name: &str,
    imported: &HashSet<String>,
    edges: &mut Vec<RawEdge>,
    seen: &mut HashSet<RawEdge>,
    gaps: &mut Vec<ImportGap>,
) {
    let from = ObjectId(from_name.to_string());

    /// Append `edge` unless an identical one was already recorded.
    fn push(edge: RawEdge, edges: &mut Vec<RawEdge>, seen: &mut HashSet<RawEdge>) {
        if seen.insert(edge.clone()) {
            edges.push(edge);
        }
    }

    if let Some(outputs) = graphic.last_child("OutputConnectors") {
        for (index, connector) in outputs.children_named("Connector").into_iter().enumerate() {
            if !connector.attribute_bool("IsAttached").unwrap_or(false) {
                continue;
            }
            let Some(to_name) = connector.attribute("AttachedToObjID") else {
                continue;
            };
            if !imported.contains(to_name) {
                gaps.push(ImportGap::DanglingConnection {
                    from_object: from_name.to_string(),
                    to_object: to_name.to_string(),
                });
                continue;
            }
            push(
                RawEdge {
                    from: from.clone(),
                    to: ObjectId(to_name.to_string()),
                    from_slot: Some(index),
                    to_slot: connector.attribute_usize("AttachedToConnIndex"),
                },
                edges,
                seen,
            );
        }
    }

    if let Some(energy) = graphic.last_child("EnergyConnector") {
        for connector in energy.children_named("Connector") {
            if !connector.attribute_bool("IsAttached").unwrap_or(false) {
                continue;
            }
            let Some(to_name) = connector.attribute("AttachedToObjID") else {
                continue;
            };
            if !imported.contains(to_name) {
                gaps.push(ImportGap::DanglingConnection {
                    from_object: from_name.to_string(),
                    to_object: to_name.to_string(),
                });
                continue;
            }
            push(
                RawEdge {
                    from: from.clone(),
                    to: ObjectId(to_name.to_string()),
                    from_slot: None,
                    to_slot: connector.attribute_usize("AttachedToConnIndex"),
                },
                edges,
                seen,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Material streams
// ---------------------------------------------------------------------------

/// Build a [`MaterialStreamData`] from a `<SimulationObject>` of type
/// `DWSIM.Thermodynamics.Streams.MaterialStream`.
///
/// # Unit conventions, verified against upstream
///
/// DWSIM stores its saved state in the internal system this crate's
/// [`crate::flowsheet::streams`] module already documents — SI, except that
/// energy is expressed in kilojoules:
///
/// | Element | Unit | Set through |
/// |---|---|---|
/// | `temperature` | K | [`MaterialStreamData::set_temperature`] |
/// | `pressure` | Pa | [`MaterialStreamData::set_pressure`] |
/// | `massflow` | kg/s | [`MaterialStreamData::set_mass_flow`] |
/// | `molarflow` | mol/s | [`MaterialStreamData::set_molar_flow`] |
/// | `volumetric_flow` | m³/s | [`MaterialStreamData::set_volumetric_flow`] |
/// | `enthalpy` | kJ/kg | [`MaterialStreamData::set_mass_enthalpy`] |
/// | `entropy` | kJ/(kg·K) | [`MaterialStreamData::set_mass_entropy`] |
/// | vapour-slot `molarfraction` | - | [`MaterialStreamData::set_vapor_fraction`] |
///
/// The kJ convention is confirmed by upstream arithmetic:
/// `GetPowerGeneratedOrConsumed` computes `mi * hi` and annotates it
/// `'kg/s * kJ/kg = kJ/s = kW` (SimulationObjectBaseClasses.vb:1548), and by the
/// reference files' own internal consistency — in `heating and cooling.dwxmz`
/// the mixture slot carries `enthalpy = 981.868193359421`,
/// `molecularWeight = 58.7798133333333` and
/// `molar_enthalpy = 57714.0291236039`, and
/// `981.868 * 58.780 = 57714` holds only if the pair is (kJ/kg, kJ/kmol).
///
/// Every property is set through the `uom`-typed accessors, so the conversion
/// is type-checked rather than asserted in a comment. Properties of the other
/// seven phase slots have no `uom` setters in the data model and are written to
/// [`PhaseProperties`] directly, in the same units.
///
/// # Degradation
///
/// - A compound with no molar mass in `<Compounds>` has one derived from its
///   own `MassFlow / MolarFlow`; if that is unavailable too, the compound is
///   still added with molar mass `0` and an
///   [`ImportGap::MissingCompoundMolarMass`] is recorded.
/// - `NaN` and infinities — DWSIM's "not calculated" marker — become `None`,
///   not stored non-finite numbers.
/// - An unrecognised `<SpecType>` / `<DefinedFlow>` / `<ForcePhase>` /
///   `<CompositionBasis>` leaves the field at its default and records an
///   [`ImportGap::UnparsedField`].
fn build_material_stream(
    node: &XmlNode,
    tag: &str,
    molar_masses: &HashMap<String, f64>,
    gaps: &mut Vec<ImportGap>,
) -> MaterialStreamData {
    let mut data = MaterialStreamData::new();

    read_enum_field(node, tag, "SpecType", gaps, |v| {
        stream_spec_from_name(v).map(|s| data.spec = s)
    });
    read_enum_field(node, tag, "DefinedFlow", gaps, |v| {
        flow_spec_from_name(v).map(|f| data.defined_flow = f)
    });
    read_enum_field(node, tag, "ForcePhase", gaps, |v| {
        forced_phase_from_name(v).map(|p| data.forced_phase = p)
    });
    // The data model stores no composition basis; parse it only to report an
    // unrecognised value rather than passing over it in silence.
    read_enum_field(node, tag, "CompositionBasis", gaps, |v| {
        composition_basis_from_name(v).map(|_| ())
    });
    data.at_equilibrium = node.child_bool("AtEquilibrium").unwrap_or(false);

    let phases = node
        .child("Phases")
        .map(|p| p.children_named("Phase"))
        .unwrap_or_default();

    // The mixture slot defines the compound set and its order.
    let mixture = phases
        .iter()
        .copied()
        .find(|p| p.child_text("ID") == Some("0"));
    if let Some(mixture) = mixture {
        for compound in compound_nodes(mixture) {
            let Some(name) = compound_name(compound) else {
                continue;
            };
            let molar_mass = match molar_masses.get(&name) {
                Some(m) => *m,
                None => match derive_molar_mass(compound) {
                    Some(m) => m,
                    None => {
                        gaps.push(ImportGap::MissingCompoundMolarMass {
                            object_tag: tag.to_string(),
                            compound: name.clone(),
                        });
                        0.0
                    }
                },
            };
            data.add_compound(name, molar_mass);
        }
    }

    let compound_count = data.compound_count();

    for phase_node in phases {
        let Some(index) = phase_node
            .child_text("ID")
            .and_then(|s| s.parse::<usize>().ok())
            .and_then(PhaseIndex::from_index)
        else {
            continue;
        };
        if let Some(properties) = phase_node.last_child("Properties") {
            let mut bag = PhaseProperties::default();
            apply_phase_properties(properties, &mut bag);
            data.phase_mut(index).properties = bag;
        }
        for (slot, compound) in compound_nodes(phase_node).into_iter().enumerate() {
            if slot >= compound_count {
                break;
            }
            apply_compound(compound, &mut data.phase_mut(index).compounds[slot]);
        }
    }

    // Re-set the mixture slot's headline properties through the `uom` accessors
    // so the unit conversion is type-checked, and so a reader of this function
    // can see exactly which quantities the importer claims to understand.
    let mixture_properties = data.phase(PhaseIndex::Mixture).properties.clone();
    if let Some(t) = mixture_properties.temperature {
        data.set_temperature(ThermodynamicTemperature::new::<kelvin>(t));
    }
    if let Some(p) = mixture_properties.pressure {
        data.set_pressure(Pressure::new::<pascal>(p));
    }
    if let Some(w) = mixture_properties.massflow {
        data.set_mass_flow(MassRate::new::<kilogram_per_second>(w));
    }
    if let Some(n) = mixture_properties.molarflow {
        // `MolarFlowRate` is `uom`'s `CatalyticActivity`; its base unit,
        // `katal`, *is* mol/s, which is what DWSIM stores.
        data.set_molar_flow(MolarFlowRate::new::<katal>(n));
    }
    if let Some(q) = mixture_properties.volumetric_flow {
        data.set_volumetric_flow(VolumeRate::new::<cubic_meter_per_second>(q));
    }
    if let Some(h) = mixture_properties.enthalpy {
        data.set_mass_enthalpy(AvailableEnergy::new::<kilojoule_per_kilogram>(h));
    }
    if let Some(s) = mixture_properties.entropy {
        data.set_mass_entropy(SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(
            s,
        ));
    }
    if let Some(beta) = data.phase(PhaseIndex::Vapor).properties.molarfraction {
        data.set_vapor_fraction(Ratio::new::<ratio>(beta));
    }

    // Overall mole fractions go through the documented setter so the length
    // invariant is enforced by the data model rather than by this loop.
    let x: Vec<f64> = data
        .phase(PhaseIndex::Mixture)
        .compounds
        .iter()
        .map(|c| c.mole_fraction.unwrap_or(0.0))
        .collect();
    if !x.is_empty() {
        let _ = data.set_overall_molar_composition(&x);
    }

    data
}

/// Read `field` from `node`; call `apply`, and record an
/// [`ImportGap::UnparsedField`] when the value is present but unrecognised.
fn read_enum_field<F>(
    node: &XmlNode,
    tag: &str,
    field: &str,
    gaps: &mut Vec<ImportGap>,
    mut apply: F,
) where
    F: FnMut(&str) -> Option<()>,
{
    let Some(value) = node.child_text(field) else {
        return;
    };
    if value.is_empty() {
        return;
    }
    if apply(value).is_none() {
        gaps.push(ImportGap::UnparsedField {
            object_tag: tag.to_string(),
            field: field.to_string(),
            value: value.to_string(),
        });
    }
}

/// The `<Compound>` children of a `<Phase>`.
///
/// A DWSIM `<Phase>` writes `<Compounds>` twice: once as a bare type-name
/// string (`System.Collections.Generic.Dictionary...`) and once as the real
/// container. Only the block that actually has `<Compound>` children is used.
fn compound_nodes(phase: &XmlNode) -> Vec<&XmlNode> {
    phase
        .children_named("Compounds")
        .into_iter()
        .map(|c| c.children_named("Compound"))
        .find(|v| !v.is_empty())
        .unwrap_or_default()
}

fn compound_name(compound: &XmlNode) -> Option<String> {
    for field in ["Name", "ComponentName"] {
        if let Some(v) = compound.child_text(field) {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Molar mass \[kg/kmol\] from a compound's own stored flows:
/// `M = (MassFlow [kg/s] / MolarFlow [mol/s]) * 1000`.
///
/// Used only when the flowsheet's `<Compounds>` section does not name the
/// compound. Returns `None` unless both flows are finite and the molar flow is
/// strictly positive.
fn derive_molar_mass(compound: &XmlNode) -> Option<f64> {
    let mass = finite(compound.child_f64("MassFlow"))?;
    let mole = finite(compound.child_f64("MolarFlow"))?;
    if mole <= 0.0 {
        return None;
    }
    let m = mass / mole * 1000.0;
    (m.is_finite() && m > 0.0).then_some(m)
}

/// `Some(v)` only for a finite `v`; DWSIM writes `NaN` for "not calculated" and
/// the data model spells that `None`.
fn finite(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite())
}

/// Copy one phase's `<Properties>` block into a [`PhaseProperties`].
///
/// The left column is DWSIM's element name, the right this crate's field; the
/// units are the ones each field already documents. All 54 distinct property
/// element names appearing anywhere in the reference corpus are covered.
/// `EnthalpyF_Dmol` / `EntropyF_Dmol` on *compounds* have no counterpart in this
/// crate's [`StreamCompound`] and are deliberately not read.
fn apply_phase_properties(node: &XmlNode, p: &mut PhaseProperties) {
    for child in &node.children {
        if !child.children.is_empty() {
            continue;
        }
        let Some(value) = finite(child.text.trim().parse::<f64>().ok()) else {
            continue;
        };
        let slot: &mut Option<f64> = match child.name.as_str() {
            "temperature" => &mut p.temperature,
            "pressure" => &mut p.pressure,
            "enthalpy" => &mut p.enthalpy,
            "enthalpyF" => &mut p.enthalpy_f,
            "entropy" => &mut p.entropy,
            "entropyF" => &mut p.entropy_f,
            "molar_enthalpy" => &mut p.molar_enthalpy,
            "molar_enthalpyF" => &mut p.molar_enthalpy_f,
            "molar_entropy" => &mut p.molar_entropy,
            "molar_entropyF" => &mut p.molar_entropy_f,
            "massflow" => &mut p.massflow,
            "massfraction" => &mut p.massfraction,
            "molarflow" => &mut p.molarflow,
            "molarfraction" => &mut p.molarfraction,
            "volumetric_flow" => &mut p.volumetric_flow,
            "volumetricFraction" => &mut p.volumetric_fraction,
            "density" => &mut p.density,
            "molecularWeight" => &mut p.molecular_weight,
            "compressibility" => &mut p.compressibility,
            "compressibilityFactor" => &mut p.compressibility_factor,
            "isothermal_compressibility" => &mut p.isothermal_compressibility,
            "bulk_modulus" => &mut p.bulk_modulus,
            "heatCapacityCp" => &mut p.heat_capacity_cp,
            "heatCapacityCv" => &mut p.heat_capacity_cv,
            "idealGasHeatCapacityCp" => &mut p.ideal_gas_heat_capacity_cp,
            "idealGasHeatCapacityRatio" => &mut p.ideal_gas_heat_capacity_ratio,
            "viscosity" => &mut p.viscosity,
            "kinematic_viscosity" => &mut p.kinematic_viscosity,
            "thermalConductivity" => &mut p.thermal_conductivity,
            "surfaceTension" => &mut p.surface_tension,
            "speedOfSound" => &mut p.speed_of_sound,
            "jouleThomsonCoefficient" => &mut p.joule_thomson_coefficient,
            "bubblePressure" => &mut p.bubble_pressure,
            "bubbleTemperature" => &mut p.bubble_temperature,
            "dewPressure" => &mut p.dew_pressure,
            "dewTemperature" => &mut p.dew_temperature,
            "freezingPoint" => &mut p.freezing_point,
            "freezingPointDepression" => &mut p.freezing_point_depression,
            "activity" => &mut p.activity,
            "activityCoefficient" => &mut p.activity_coefficient,
            "fugacity" => &mut p.fugacity,
            "fugacityCoefficient" => &mut p.fugacity_coefficient,
            "logFugacityCoefficient" => &mut p.log_fugacity_coefficient,
            "kvalue" => &mut p.kvalue,
            "logKvalue" => &mut p.log_kvalue,
            "excessEnthalpy" => &mut p.excess_enthalpy,
            "excessEntropy" => &mut p.excess_entropy,
            "gibbs_free_energy" => &mut p.gibbs_free_energy,
            "helmholtz_energy" => &mut p.helmholtz_energy,
            "internal_energy" => &mut p.internal_energy,
            "molar_gibbs_free_energy" => &mut p.molar_gibbs_free_energy,
            "molar_helmholtz_energy" => &mut p.molar_helmholtz_energy,
            "molar_internal_energy" => &mut p.molar_internal_energy,
            "osmoticCoefficient" => &mut p.osmotic_coefficient,
            "pH" => &mut p.ph,
            _ => continue,
        };
        *slot = Some(value);
    }
}

/// Copy one `<Compound>` block into a [`StreamCompound`], leaving its name and
/// molar mass (already set by `add_compound`) untouched.
fn apply_compound(node: &XmlNode, c: &mut StreamCompound) {
    c.mole_fraction = finite(node.child_f64("MoleFraction"));
    c.mass_fraction = finite(node.child_f64("MassFraction"));
    c.molar_flow = finite(node.child_f64("MolarFlow"));
    c.mass_flow = finite(node.child_f64("MassFlow"));
    c.volumetric_flow = finite(node.child_f64("VolumetricFlow"));
    c.volumetric_fraction = finite(node.child_f64("VolumetricFraction"));
    c.activity_coefficient = finite(node.child_f64("ActivityCoeff"));
    c.fugacity_coefficient = finite(node.child_f64("FugacityCoeff"));
    c.kvalue = finite(node.child_f64("Kvalue"));
    c.ln_kvalue = finite(node.child_f64("lnKvalue"));
    c.molarity = finite(node.child_f64("Molarity"));
    c.molality = finite(node.child_f64("Molality"));
    c.partial_pressure = finite(node.child_f64("PartialPressure"));
    c.partial_volume = finite(node.child_f64("PartialVolume"));
    c.diffusion_coefficient = finite(node.child_f64("DiffusionCoefficient"));
    c.petroleum_fraction = node.child_bool("PetroleumFraction").unwrap_or(false);
}
