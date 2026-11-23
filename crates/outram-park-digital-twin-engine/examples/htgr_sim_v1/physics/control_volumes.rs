//! **The control volumes the plant model actually has** (gh:#1718), listed
//! from the ASSEMBLED [`HtgrPlant`], never from named constants.
//!
//! # Why this exists
//!
//! The SB-LOCA fault tree (gh:#1719) names pressure-boundary components a
//! break could occur in, and each must sit on a control volume (CV) the model
//! has. The CV schematic tab draws this list and the fault tree checks its
//! basic events against it, so both read **one** list, taken from the plant
//! that steps. A list typed from the module docs would draw the CVs the docs
//! promise, which is exactly the defect class the engine's drawing rule exists
//! to catch (`crates/outram-park-digital-twin-engine/AGENTS.md`, "Reactor
//! geometry is DRAWN").
//!
//! # What counts, and the roles
//!
//! Not every box in a plant drawing is a CV of this model, and the list says
//! which kind each entry is ([`CvRole`]):
//!
//! | Role | Meaning | Entries |
//! |---|---|---|
//! | [`CvRole::Balance`] | an integrated energy (and, for helium, mass) balance with its own state | bed fuel node, bed solid and helium, hot duct, cold duct, RPV annuli, 8 + 8 + 8 steam-generator nodes, reflector, RPV |
//! | [`CvRole::Junction`] | an algebraic node with no capacity | the near-wall node of the passive path |
//! | [`CvRole::Inventory`] | a mass at a state, used for a transit time; **no balance** | the three secondary pipe runs |
//! | [`CvRole::NoVolume`] | a component the flow passes through with no CV | the circulator (its work enters the cold duct), turbine, condenser, feed pump (state-to-state flashes) |
//! | [`CvRole::Boundary`] | a fixed boundary condition | the RCCS at 50 degC |
//!
//! The secondary loop has **no** water inventory balance (its module doc,
//! "Still absent"), so its pipe runs are inventories, not CVs. The reflector
//! cooling channels have **no node**: their helium is in the RPV-annuli CV
//! (`primary_loop`'s nodalisation table), so they are not listed.
//!
//! # Flow direction
//!
//! Every [`CvLink`] states its nominal `from -> to` and carries the signed
//! mass flow (or heat rate). A drawing takes the direction from the **sign**,
//! never from the nominal order (the engine's animation rule): a reversed
//! flow draws reversed. While the secondary is isolated the steam generator
//! carries no flow and the helium bypasses it, hot duct straight to cold duct
//! ([`HeliumPrimaryLoop::advance_steam_generator`]); the map says so with a
//! bypass link that carries the loop flow only then.
//!
//! [`HeliumPrimaryLoop::advance_steam_generator`]: super::primary_loop::HeliumPrimaryLoop::advance_steam_generator

use super::pebble_bed;
use super::{decay_heat_removal, primary_loop, secondary_loop, HtgrPlant};
use uom::si::f64::{Mass, MassRate, Power, ThermodynamicTemperature, Volume};
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volume::cubic_meter;

/// Which circuit a CV belongs to (the schematic groups by it).
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Circuit {
    /// The pebble bed.
    Core,
    /// The helium circuit outside the bed and the steam generator.
    Primary,
    /// The steam generator's three arrays.
    SteamGenerator,
    /// The Rankine cycle outside the steam generator.
    Secondary,
    /// The passive decay-heat path, bed to RCCS.
    Passive,
}

/// What a CV holds.
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Medium {
    Helium,
    Graphite,
    /// TRISO particles (kernels and coatings): the kinetics fuel node.
    Fuel,
    Steel,
    WaterSteam,
    /// A component with no volume of its own, or a boundary condition.
    None,
}

/// What kind of entry this is; see the module docs' table.
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CvRole {
    Balance,
    Junction,
    Inventory,
    NoVolume,
    Boundary,
}

/// One entry of the list.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlVolume {
    /// Stable identifier (`primary.hot_duct`, `sg.he.3`): what the fault
    /// tree's basic events name.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    pub circuit: Circuit,
    pub medium: Medium,
    pub role: CvRole,
    /// The volume the model gives it; `None` where it has none.
    pub volume: Option<Volume>,
    /// Live temperature; `None` where the model has no state for it.
    pub temperature: Option<ThermodynamicTemperature>,
    /// Live mass; `None` where the model does not expose one.
    pub mass: Option<Mass>,
    /// Mass flow through it; `None` for a solid or a boundary.
    pub through_flow: Option<MassRate>,
    /// Inside the primary helium pressure boundary (helium the boundary
    /// holds, and the solids immersed in it).
    pub inside_primary_boundary: bool,
    /// One line on what it is, or what it is not.
    pub note: &'static str,
}

/// What a link carries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LinkKind {
    /// Advected mass, signed: positive runs `from -> to`.
    Flow(MassRate),
    /// A heat path, with its rate when the model exposes one (positive
    /// `from -> to`).
    Heat(Option<Power>),
}

/// A connection between two entries.
#[derive(Clone, Debug, PartialEq)]
pub struct CvLink {
    pub from: String,
    pub to: String,
    pub kind: LinkKind,
}

/// A component of the primary pressure boundary a break could occur in,
/// placed on the CV(s) whose helium it holds.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundaryComponent {
    /// Stable identifier the fault tree names (`rpv.fuel_loading_tube`).
    pub id: &'static str,
    pub name: &'static str,
    /// The CV ids it bounds.
    pub cvs: Vec<String>,
    /// Where its existence and placement come from, and what is assumed.
    pub source: &'static str,
}

/// The whole list: entries, links and boundary components.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlVolumeMap {
    pub volumes: Vec<ControlVolume>,
    pub links: Vec<CvLink>,
    pub boundary: Vec<BoundaryComponent>,
}

impl ControlVolumeMap {
    /// The entry with id `id`.
    pub fn get(&self, id: &str) -> Option<&ControlVolume> {
        self.volumes.iter().find(|v| v.id == id)
    }

    /// Every entry linked to `id`, in link order, each once.
    pub fn neighbours(&self, id: &str) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for l in &self.links {
            let other = if l.from == id {
                l.to.as_str()
            } else if l.to == id {
                l.from.as_str()
            } else {
                continue;
            };
            if !out.contains(&other) {
                out.push(other);
            }
        }
        out
    }

    /// Header of [`Self::to_csv`].
    pub fn csv_header() -> &'static str {
        "id,name,circuit,medium,role,volume_m3,inside_primary_boundary,neighbours,temperature_k,mass_kg,through_flow_kg_per_s"
    }

    /// One row per entry: the structure (id to neighbours, which
    /// [`Self::structure_csv`] pins) then the live state, fixed precision.
    /// An absent value is an empty field. The `--dump-cvs` output.
    pub fn to_csv(&self) -> String {
        let mut out = String::from(Self::csv_header());
        out.push('\n');
        for v in &self.volumes {
            out.push_str(&self.structure_row(v));
            out.push_str(&format!(
                ",{},{},{}\n",
                opt(v.temperature.map(|t| t.get::<kelvin>()), 3),
                opt(v.mass.map(|m| m.get::<kilogram>()), 6),
                opt(v.through_flow.map(|m| m.get::<kilogram_per_second>()), 6),
            ));
        }
        out
    }

    /// The structural columns only (`id` .. `neighbours`), which do not
    /// change as the plant steps: what the regression test pins and what the
    /// fault tree's CV check reads.
    pub fn structure_csv(&self) -> String {
        let mut out = String::from("id,name,circuit,medium,role,volume_m3,inside_primary_boundary,neighbours\n");
        for v in &self.volumes {
            out.push_str(&self.structure_row(v));
            out.push('\n');
        }
        out
    }

    fn structure_row(&self, v: &ControlVolume) -> String {
        format!(
            "{},{},{:?},{:?},{:?},{},{},{}",
            v.id,
            v.name,
            v.circuit,
            v.medium,
            v.role,
            v.volume.map_or(String::new(), |x| format!("{:.6e}", x.get::<cubic_meter>())),
            v.inside_primary_boundary,
            self.neighbours(&v.id).join(";"),
        )
    }
}

/// The map as plain numbers, for the GUI snapshot (gh:#1718): what the
/// physics thread (or the browser's worker) publishes and the CV tab draws.
/// Absent values are `NAN`; a link's `value` is kg/s for a flow and W for a
/// heat path.
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CvSnapshot {
    pub volumes: Vec<CvRow>,
    pub links: Vec<LinkRow>,
    pub boundary: Vec<BoundaryRow>,
}

/// One [`ControlVolume`] as plain numbers.
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct CvRow {
    pub id: String,
    pub name: String,
    pub circuit: Circuit,
    pub medium: Medium,
    pub role: CvRole,
    pub volume_m3: f64,
    pub temperature_k: f64,
    pub mass_kg: f64,
    pub through_flow_kg_per_s: f64,
    pub inside_primary_boundary: bool,
    pub note: String,
}

/// One [`CvLink`] as plain numbers.
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct LinkRow {
    pub from: String,
    pub to: String,
    /// A mass flow (kg/s) rather than a heat path (W).
    pub is_flow: bool,
    /// Signed, positive `from -> to`; `NAN` when the model does not expose
    /// it.
    pub value: f64,
}

/// One [`BoundaryComponent`] as plain data.
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct BoundaryRow {
    pub id: String,
    pub name: String,
    pub cvs: Vec<String>,
    pub source: String,
}

impl CvSnapshot {
    /// Flatten `map`.
    pub fn from_map(map: &ControlVolumeMap) -> Self {
        let nan = f64::NAN;
        Self {
            volumes: map
                .volumes
                .iter()
                .map(|v| CvRow {
                    id: v.id.clone(),
                    name: v.name.clone(),
                    circuit: v.circuit,
                    medium: v.medium,
                    role: v.role,
                    volume_m3: v.volume.map_or(nan, |x| x.get::<cubic_meter>()),
                    temperature_k: v.temperature.map_or(nan, |x| x.get::<kelvin>()),
                    mass_kg: v.mass.map_or(nan, |x| x.get::<kilogram>()),
                    through_flow_kg_per_s: v.through_flow.map_or(nan, |x| x.get::<kilogram_per_second>()),
                    inside_primary_boundary: v.inside_primary_boundary,
                    note: v.note.to_string(),
                })
                .collect(),
            links: map
                .links
                .iter()
                .map(|l| match l.kind {
                    LinkKind::Flow(m) => LinkRow {
                        from: l.from.clone(),
                        to: l.to.clone(),
                        is_flow: true,
                        value: m.get::<kilogram_per_second>(),
                    },
                    LinkKind::Heat(q) => LinkRow {
                        from: l.from.clone(),
                        to: l.to.clone(),
                        is_flow: false,
                        value: q.map_or(nan, |q| q.get::<uom::si::power::watt>()),
                    },
                })
                .collect(),
            boundary: map
                .boundary
                .iter()
                .map(|b| BoundaryRow {
                    id: b.id.to_string(),
                    name: b.name.to_string(),
                    cvs: b.cvs.clone(),
                    source: b.source.to_string(),
                })
                .collect(),
        }
    }

    /// The row with id `id`.
    pub fn get(&self, id: &str) -> Option<&CvRow> {
        self.volumes.iter().find(|v| v.id == id)
    }
}

/// `value` at `decimals` places, or empty when absent or not finite.
fn opt(value: Option<f64>, decimals: usize) -> String {
    match value {
        Some(x) if x.is_finite() => format!("{x:.decimals$}"),
        _ => String::new(),
    }
}

/// A balance entry; the constructor for the many CVs that have every field.
#[allow(clippy::too_many_arguments)]
fn cv(
    id: &str,
    name: &str,
    circuit: Circuit,
    medium: Medium,
    role: CvRole,
    volume: Option<Volume>,
    temperature: Option<ThermodynamicTemperature>,
    mass: Option<Mass>,
    through_flow: Option<MassRate>,
    inside_primary_boundary: bool,
    note: &'static str,
) -> ControlVolume {
    ControlVolume {
        id: id.to_string(),
        name: name.to_string(),
        circuit,
        medium,
        role,
        volume,
        temperature,
        mass,
        through_flow,
        inside_primary_boundary,
        note,
    }
}

fn flow(from: &str, to: &str, m: MassRate) -> CvLink {
    CvLink { from: from.to_string(), to: to.to_string(), kind: LinkKind::Flow(m) }
}

fn heat(from: &str, to: &str, q: Option<Power>) -> CvLink {
    CvLink { from: from.to_string(), to: to.to_string(), kind: LinkKind::Heat(q) }
}

/// Steam-generator node ids, station `i` counted from 1 at the hot inlet.
pub fn sg_helium_id(i: usize) -> String {
    format!("sg.he.{i}")
}
/// See [`sg_helium_id`].
pub fn sg_metal_id(i: usize) -> String {
    format!("sg.metal.{i}")
}
/// See [`sg_helium_id`].
pub fn sg_water_id(i: usize) -> String {
    format!("sg.water.{i}")
}

/// The control volumes of `plant`, read from its assembled state.
pub fn control_volume_map(plant: &HtgrPlant) -> ControlVolumeMap {
    let mut volumes = Vec::new();
    let mut links = Vec::new();

    // --- Core: the bed's two-temperature CV (whichever tier is selected
    // steps this node; the placeholders delegate to it) and the fuel node.
    let bed = plant.core.porous_media_node();
    let primary = &plant.primary;
    let m_loop = primary.mass_flow();
    let pebbles = pebble_bed::pebble_count();
    let solid_volume = pebble_bed::pebble_volume() * pebbles;
    let fuel_volume = (pebble_bed::pebble_volume() - pebble_bed::pebble_graphite_volume()) * pebbles;
    let he = bed.helium_state();
    let void = bed.helium_void_volume();
    volumes.push(cv(
        "core.fuel",
        "Fuel (TRISO particles)",
        Circuit::Core,
        Medium::Fuel,
        CvRole::Balance,
        Some(fuel_volume),
        Some(plant.kinetics.fuel_temperature()),
        None,
        None,
        true,
        "the kinetics fuel node: inventory-averaged kernel; mass not exposed by the model",
    ));
    volumes.push(cv(
        "core.solid",
        "Pebble bed solid",
        Circuit::Core,
        Medium::Graphite,
        CvRole::Balance,
        Some(solid_volume),
        Some(bed.pebble_temperature()),
        Some(pebble_bed::graphite_mass()),
        None,
        true,
        "one node for the whole bed: a volume average, not a peak",
    ));
    volumes.push(cv(
        "core.helium",
        "Pebble bed helium",
        Circuit::Core,
        Medium::Helium,
        CvRole::Balance,
        Some(void),
        Some(bed.helium_temperature()),
        Some(Mass::new::<kilogram>(he.density * void.get::<cubic_meter>())),
        Some(m_loop),
        true,
        "the bed's void helium; its state is the core outlet",
    ));

    // --- Primary helium outside the bed.
    volumes.push(cv(
        "primary.hot_duct",
        "Hot plenum + hot gas duct centre tube",
        Circuit::Primary,
        Medium::Helium,
        CvRole::Balance,
        Some(primary_loop::hot_duct_volume()),
        Some(primary.hot_duct_temperature()),
        Some(primary.hot_duct_mass()),
        Some(m_loop),
        true,
        "hot gas plenum in the bottom reflector and the coaxial duct's centre tube",
    ));
    volumes.push(cv(
        "primary.circulator",
        "Helium circulator",
        Circuit::Primary,
        Medium::None,
        CvRole::NoVolume,
        None,
        None,
        None,
        Some(m_loop),
        true,
        "no CV: its shaft work is the cold-duct CV's source",
    ));
    volumes.push(cv(
        "primary.cold_duct",
        "Cold duct (coaxial annulus)",
        Circuit::Primary,
        Medium::Helium,
        CvRole::Balance,
        Some(primary_loop::cold_duct_volume()),
        Some(primary.cold_duct_temperature()),
        Some(primary.cold_duct_mass()),
        Some(m_loop),
        true,
        "circulator discharge to the reactor vessel",
    ));
    volumes.push(cv(
        "primary.rpv_annuli",
        "RPV annuli + risers + top plenum",
        Circuit::Primary,
        Medium::Helium,
        CvRole::Balance,
        Some(primary_loop::rpv_annuli_volume()),
        Some(primary.core_inlet_temperature()),
        Some(primary.rpv_annuli_mass()),
        Some(m_loop),
        true,
        "RPV/barrel annulus, the 20 riser boreholes (no node of their own), top plenum; its state is the core inlet",
    ));

    // --- Steam generator: three arrays, hot-side order.
    let sg = primary.steam_generator();
    let state = sg.state();
    let inv = sg.node_inventories();
    let m_shell = primary.steam_generator_shell_flow();
    let m_tube = primary.steam_generator_tube_flow(plant.secondary.mass_flow());
    let n = inv.len();
    for (k, node) in inv.iter().enumerate() {
        let i = k + 1;
        let t = |v: &Vec<ThermodynamicTemperature>| v.get(k).copied();
        volumes.push(ControlVolume {
            id: sg_helium_id(i),
            name: format!("SG helium {i}/{n}"),
            circuit: Circuit::SteamGenerator,
            medium: Medium::Helium,
            role: CvRole::Balance,
            volume: Some(node.helium_volume),
            temperature: t(&state.hot_node_temperatures),
            mass: Some(node.helium_mass),
            through_flow: Some(m_shell),
            inside_primary_boundary: true,
            note: "shell-side helium cell, counted from the hot inlet",
        });
        volumes.push(ControlVolume {
            id: sg_metal_id(i),
            name: format!("SG tube metal {i}/{n}"),
            circuit: Circuit::SteamGenerator,
            medium: Medium::Steel,
            role: CvRole::Balance,
            volume: Some(node.metal_volume),
            temperature: t(&state.metal_node_temperatures),
            mass: Some(node.metal_mass),
            through_flow: None,
            inside_primary_boundary: true,
            note: "tube wall slice: the primary-to-secondary boundary",
        });
        volumes.push(ControlVolume {
            id: sg_water_id(i),
            name: format!("SG water/steam {i}/{n}"),
            circuit: Circuit::SteamGenerator,
            medium: Medium::WaterSteam,
            role: CvRole::Balance,
            volume: Some(node.water_volume),
            temperature: t(&state.cold_node_temperatures),
            mass: Some(node.water_mass),
            through_flow: Some(m_tube),
            inside_primary_boundary: false,
            note: "tube-side water/steam cell at the same station",
        });
    }

    // --- Secondary: inventories and state-to-state components.
    let sec = &plant.secondary;
    let m_sec = sec.mass_flow();
    let run_v = secondary_loop::piping_run_volumes();
    let run_m = sec.piping_run_inventories();
    let t_cond = sec.condensate().get_temperature();
    let secondary_entries: [(&str, &str, CvRole, Option<Volume>, Option<ThermodynamicTemperature>, Option<Mass>, &'static str); 6] = [
        (
            "secondary.main_steam_line",
            "Main steam line",
            CvRole::Inventory,
            Some(run_v.main_steam),
            Some(sec.turbine_inlet_temperature()),
            Some(run_m.main_steam),
            "inventory at the SG outlet state, for the transit time; no balance (invented volume)",
        ),
        ("secondary.turbine", "Turbine", CvRole::NoVolume, None, None, None, "a state-to-state (p, s) flash; no CV"),
        (
            "secondary.exhaust_duct",
            "Turbine exhaust duct",
            CvRole::Inventory,
            Some(run_v.exhaust),
            Some(t_cond),
            Some(run_m.exhaust),
            "inventory at the turbine outlet state; temperature is the condenser saturation (invented volume)",
        ),
        ("secondary.condenser", "Condenser", CvRole::NoVolume, None, Some(t_cond), None, "condensate is the saturated liquid; no inventory"),
        ("secondary.feed_pump", "Feed pump", CvRole::NoVolume, None, None, None, "pump work v dp / eta on the condensate; no CV"),
        (
            "secondary.feedwater_line",
            "Condensate / feedwater line",
            CvRole::Inventory,
            Some(run_v.feedwater),
            Some(t_cond),
            Some(run_m.feedwater),
            "inventory at the hotwell condensate state; no balance (invented volume)",
        ),
    ];
    for (id, name, role, volume, temperature, mass, note) in secondary_entries {
        let medium = if role == CvRole::NoVolume { Medium::None } else { Medium::WaterSteam };
        volumes.push(cv(id, name, Circuit::Secondary, medium, role, volume, temperature, mass, Some(m_sec), false, note));
    }

    // --- Passive decay-heat path.
    let path = &plant.decay_heat_path;
    volumes.push(cv(
        "passive.near_wall",
        "Near-wall node",
        Circuit::Passive,
        Medium::None,
        CvRole::Junction,
        None,
        None,
        None,
        None,
        true,
        "algebraic junction of the bed-to-reflector network; no capacity",
    ));
    volumes.push(cv(
        "passive.reflector",
        "Side reflector + boronated brick",
        Circuit::Passive,
        Medium::Graphite,
        CvRole::Balance,
        Some(decay_heat_removal::reflector_graphite_volume() + decay_heat_removal::boronated_brick_volume()),
        Some(path.reflector_temperature()),
        Some(decay_heat_removal::reflector_graphite_mass() + decay_heat_removal::boronated_brick_mass()),
        None,
        true,
        "one solid node: graphite annulus and boronated carbon brick",
    ));
    volumes.push(cv(
        "passive.rpv",
        "Reactor pressure vessel wall",
        Circuit::Passive,
        Medium::Steel,
        CvRole::Balance,
        Some(decay_heat_removal::rpv_wall_volume()),
        Some(path.rpv_temperature()),
        Some(decay_heat_removal::rpv_mass()),
        None,
        false,
        "the lateral shell only (invented wall thickness); it IS the boundary",
    ));
    volumes.push(cv(
        "passive.rccs",
        "RCCS (cavity cooling)",
        Circuit::Passive,
        Medium::None,
        CvRole::Boundary,
        None,
        Some(decay_heat_removal::CoreToRccsPath::rccs_boundary()),
        None,
        None,
        false,
        "a fixed 50 degC boundary, not a CV",
    ));

    // --- Links. The primary loop, in the published flow order.
    links.push(flow("primary.rpv_annuli", "core.helium", m_loop));
    links.push(flow("core.helium", "primary.hot_duct", m_loop));
    links.push(flow("primary.hot_duct", &sg_helium_id(1), m_shell));
    for i in 1..n {
        links.push(flow(&sg_helium_id(i), &sg_helium_id(i + 1), m_shell));
    }
    links.push(flow(&sg_helium_id(n.max(1)), "primary.circulator", m_shell));
    // While isolated the steam generator is valved out and the helium goes
    // from the hot duct to the circulator directly (a zero flow otherwise).
    links.push(flow("primary.hot_duct", "primary.circulator", primary.steam_generator_bypass_flow()));
    links.push(flow("primary.circulator", "primary.cold_duct", m_loop));
    links.push(flow("primary.cold_duct", "primary.rpv_annuli", m_loop));
    // Bed heat paths.
    links.push(heat("core.fuel", "core.solid", None));
    links.push(heat("core.solid", "core.helium", Some(bed.heat_to_helium())));
    // Steam generator: helium -> metal -> water at each station, and the
    // tube-side flow, which runs from the hot-side end n back to 1.
    for i in 1..=n {
        links.push(heat(&sg_helium_id(i), &sg_metal_id(i), None));
        links.push(heat(&sg_metal_id(i), &sg_water_id(i), None));
    }
    links.push(flow("secondary.feedwater_line", &sg_water_id(n.max(1)), m_tube));
    for i in (1..n).rev() {
        links.push(flow(&sg_water_id(i + 1), &sg_water_id(i), m_tube));
    }
    links.push(flow(&sg_water_id(1), "secondary.main_steam_line", m_tube));
    links.push(flow("secondary.main_steam_line", "secondary.turbine", m_sec));
    links.push(flow("secondary.turbine", "secondary.exhaust_duct", m_sec));
    links.push(flow("secondary.exhaust_duct", "secondary.condenser", m_sec));
    links.push(flow("secondary.condenser", "secondary.feed_pump", m_sec));
    links.push(flow("secondary.feed_pump", "secondary.feedwater_line", m_sec));
    // Passive path.
    links.push(heat("core.solid", "passive.near_wall", None));
    links.push(heat("core.helium", "passive.near_wall", None));
    links.push(heat("passive.near_wall", "passive.reflector", Some(path.heat_from_core())));
    links.push(heat("passive.reflector", "primary.rpv_annuli", Some(path.heat_to_risers())));
    links.push(heat("passive.reflector", "passive.rpv", None));
    links.push(heat("passive.rpv", "passive.rccs", Some(path.heat_to_rccs())));

    ControlVolumeMap { volumes, links, boundary: boundary_components(n) }
}

/// The primary pressure-boundary components a break could occur in, each on
/// the CV whose helium it holds. Sources are `docs/reactor-scoping/
/// htr10-plant-data.md` (its [S2] = Gao & Shi / INET plant description,
/// [S5] = the IAEA CRP-5 description) section by section.
fn boundary_components(sg_nodes: usize) -> Vec<BoundaryComponent> {
    let one = |id: &str| vec![id.to_string()];
    vec![
        BoundaryComponent {
            id: "rpv.shell",
            name: "RPV shell",
            cvs: one("primary.rpv_annuli"),
            source: "plant-data s.4.2: RPV design pressure 3.5 MPa; the RPV/core-barrel annulus holds 250 degC helium ([S5] s.2)",
        },
        BoundaryComponent {
            id: "rpv.fuel_loading_tube",
            name: "Fuel loading tube (top of RPV)",
            cvs: one("primary.rpv_annuli"),
            source: "plant-data s.4.3: 62 mm [S2] / 65 mm [S5]; enters the top cold plenum, which is in the RPV-annuli CV; the DN65 DLOFC break of physics::depressurisation (Gao & Shi 2002 s.5.3.1)",
        },
        BoundaryComponent {
            id: "rpv.fuel_discharge_penetration",
            name: "Fuel discharge tube RPV penetration",
            cvs: one("primary.rpv_annuli"),
            source: "plant-data s.4.3: 65 mm penetration [S2] s.2; about 1 % of the flow branches into the discharge tube from the RPV annulus (s.4.4 step 5)",
        },
        BoundaryComponent {
            id: "rpv.control_rod_nozzles",
            name: "Control-rod and absorber-ball nozzles",
            cvs: one("primary.rpv_annuli"),
            source: "plant-data s.4.3: 10 control-rod and 7 KLAK channels; cold helium flows to the rod holes (s.4.4 step 5); nozzle bores NOT stated",
        },
        BoundaryComponent {
            id: "rpv.instrument_lines",
            name: "Instrument and measuring lines",
            cvs: one("primary.rpv_annuli"),
            source: "plant-data s.4.3: 3 irradiation/measuring channels; instrument-line bores and their CV are NOT stated, placed on the RPV annuli (ASSUMED)",
        },
        BoundaryComponent {
            id: "hgd.pressure_tube",
            name: "Hot gas duct outer pressure tube",
            cvs: one("primary.cold_duct"),
            source: "plant-data s.4.1: the 900 mm outer tube carries the pressure, the cold annulus is inside it [S2] s.4",
        },
        BoundaryComponent {
            id: "sg.pressure_vessel",
            name: "SG pressure vessel",
            cvs: one("primary.cold_duct"),
            source: "plant-data s.4.4 step 2 and s.3: circulator discharge runs between the SG vessel and its sleeve, the stream the cold-duct CV carries; that annulus has no CV of its own",
        },
        BoundaryComponent {
            id: "sg.tubes",
            name: "SG heat-transfer tubes",
            cvs: (1..=sg_nodes).map(sg_helium_id).collect(),
            source: "plant-data s.3: the tubes separate primary helium (shell side) from 4 MPa water; a rupture is water INGRESS into the primary, not helium loss",
        },
        BoundaryComponent {
            id: "circulator.housing",
            name: "Circulator housing and shaft seal",
            cvs: one("primary.cold_duct"),
            source: "plant-data s.3: vertical circulator on the SG vessel; it discharges into the stream the cold-duct CV holds (primary_loop: the circulator has no CV)",
        },
        BoundaryComponent {
            id: "primary.safety_valves",
            name: "Primary safety valves (2)",
            cvs: one("primary.rpv_annuli"),
            source: "plant-data s.4.2: two valves, set 3.5 and 3.75 MPa ([S5] s.1); their location is NOT stated, placed on the RPV annuli (ASSUMED)",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The list is pinned: ids, names, roles, volumes (6 significant
    /// figures) and neighbours of the plant as constructed must match the
    /// committed `reference/control_volumes_structure.csv`. A change here is
    /// a change to what the model HAS, and the fixture is regenerated on
    /// purpose (`--dump-cvs`), never to make this pass.
    #[test]
    fn the_control_volume_list_matches_the_pinned_fixture() {
        let map = control_volume_map(&HtgrPlant::new());
        let fixture = include_str!("../reference/control_volumes_structure.csv");
        assert_eq!(map.structure_csv(), fixture);
    }

    /// Checked by hand against the physics modules' own docs (2026-11-23):
    /// the five helium balances of `primary_loop`'s nodalisation table (bed
    /// void, hot duct, cold duct, RPV annuli, plus the SG's 8 shell cells),
    /// `steam_generator`'s 8 + 8 + 8 nodes, `decay_heat_removal`'s two solid
    /// nodes, junction and fixed boundary, and no CV for the circulator or
    /// the riser channels.
    #[test]
    fn the_list_has_what_the_physics_modules_document() {
        let map = control_volume_map(&HtgrPlant::new());
        let count = |c: Circuit, m: Medium| map.volumes.iter().filter(|v| v.circuit == c && v.medium == m).count();
        assert_eq!(count(Circuit::SteamGenerator, Medium::Helium), 8);
        assert_eq!(count(Circuit::SteamGenerator, Medium::Steel), 8);
        assert_eq!(count(Circuit::SteamGenerator, Medium::WaterSteam), 8);
        let helium_balances: Vec<&str> = map
            .volumes
            .iter()
            .filter(|v| v.medium == Medium::Helium && v.role == CvRole::Balance && v.circuit != Circuit::SteamGenerator)
            .map(|v| v.id.as_str())
            .collect();
        assert_eq!(helium_balances, ["core.helium", "primary.hot_duct", "primary.cold_duct", "primary.rpv_annuli"]);
        assert_eq!(map.get("primary.circulator").unwrap().role, CvRole::NoVolume);
        assert_eq!(map.get("passive.near_wall").unwrap().role, CvRole::Junction);
        assert_eq!(map.get("passive.rccs").unwrap().role, CvRole::Boundary);
        assert!(map.volumes.iter().all(|v| !v.id.contains("riser")), "the risers have no node");

        // The helium CVs' volumes add up to the loop's own gas volume.
        let gas: f64 = map
            .volumes
            .iter()
            .filter(|v| v.medium == Medium::Helium)
            .map(|v| v.volume.unwrap().get::<cubic_meter>())
            .sum();
        let loop_gas = HtgrPlant::new().primary.gas_volume().get::<cubic_meter>();
        assert!((gas - loop_gas).abs() < 1e-9 * loop_gas, "{gas} vs {loop_gas}");

        // Ids and names are CSV-safe (no comma), and ids are unique.
        for v in &map.volumes {
            assert!(!v.id.contains(',') && !v.name.contains(','), "{}", v.id);
            assert_eq!(map.volumes.iter().filter(|w| w.id == v.id).count(), 1, "{}", v.id);
        }

        // Every link joins listed entries, and every boundary component sits
        // on a listed CV inside the primary boundary.
        for l in &map.links {
            assert!(map.get(&l.from).is_some() && map.get(&l.to).is_some(), "{l:?}");
        }
        for b in &map.boundary {
            for c in &b.cvs {
                let v = map.get(c).unwrap_or_else(|| panic!("{} names {c}", b.id));
                assert!(v.inside_primary_boundary, "{} on {c}", b.id);
            }
        }
    }

    /// Live state: at construction every balance CV has a finite temperature
    /// and every helium CV a positive mass; the primary flow links carry the
    /// loop flow, positive (published direction), and the SG bypass none.
    /// Isolating the secondary moves the flow onto the bypass.
    #[test]
    fn live_state_and_flow_signs_follow_the_plant() {
        let mut plant = HtgrPlant::new();
        let map = control_volume_map(&plant);
        for v in map.volumes.iter().filter(|v| v.role == CvRole::Balance) {
            let t = v.temperature.unwrap().get::<kelvin>();
            assert!(t.is_finite() && t > 250.0, "{}: {t} K", v.id);
            if v.medium == Medium::Helium {
                assert!(v.mass.unwrap().get::<kilogram>() > 0.0, "{}", v.id);
            }
        }
        let m = |map: &ControlVolumeMap, from: &str, to: &str| -> f64 {
            match map.links.iter().find(|l| l.from == from && l.to == to).unwrap().kind {
                LinkKind::Flow(m) => m.get::<kilogram_per_second>(),
                LinkKind::Heat(_) => panic!("not a flow"),
            }
        };
        let loop_flow = plant.primary.mass_flow().get::<kilogram_per_second>();
        assert!(loop_flow > 0.0);
        assert_eq!(m(&map, "primary.hot_duct", "sg.he.1"), loop_flow);
        assert_eq!(m(&map, "primary.hot_duct", "primary.circulator"), 0.0);

        plant.primary.isolate_secondary(true);
        let iso = control_volume_map(&plant);
        assert_eq!(m(&iso, "primary.hot_duct", "sg.he.1"), 0.0);
        assert_eq!(m(&iso, "primary.hot_duct", "primary.circulator"), loop_flow);
        assert_eq!(m(&iso, "sg.water.1", "secondary.main_steam_line"), 0.0);

        // The snapshot carries the same entries and links as plain numbers.
        let snap = CvSnapshot::from_map(&map);
        assert_eq!(snap.volumes.len(), map.volumes.len());
        assert_eq!(snap.links.len(), map.links.len());
        assert_eq!(snap.boundary.len(), map.boundary.len());
        assert!(snap.get("passive.near_wall").unwrap().volume_m3.is_nan());
        let hot = snap.links.iter().find(|l| l.from == "primary.hot_duct" && l.to == "sg.he.1").unwrap();
        assert!(hot.is_flow && hot.value == loop_flow);
        assert!(snap.links.iter().any(|l| !l.is_flow && l.value.is_nan()));

        // The CSV carries a header and one row per entry.
        let csv = map.to_csv();
        assert_eq!(csv.lines().count(), map.volumes.len() + 1);
        assert!(csv.starts_with(ControlVolumeMap::csv_header()));
        assert_eq!(opt(None, 3), "");
        assert_eq!(opt(Some(f64::NAN), 3), "");
        assert_eq!(opt(Some(1.23456), 2), "1.23");
    }
}
