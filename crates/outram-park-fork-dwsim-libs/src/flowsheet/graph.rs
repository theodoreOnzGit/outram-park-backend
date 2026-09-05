//! The flowsheet itself: the simulation-object registry, the connection
//! topology, tag generation, calculation-status bookkeeping, and the
//! flowsheet-level mass and energy balance.
//!
//! # What this represents physically
//!
//! A flowsheet is a directed graph whose **nodes** are unit operations and whose
//! **edges** are streams — except that in DWSIM (and here) a stream is itself a
//! node, so an edge always runs *unit operation -> stream* or *stream -> unit
//! operation*, never directly between two unit operations and never between two
//! streams. That alternation is enforced by [`Flowsheet::connect`] and is the
//! single most important structural invariant of the model: it is what lets the
//! solver treat every stream as a state carrier it can read, write, and tear.
//!
//! Each edge lands in a specific, index-addressed **connector slot** (see
//! [`crate::flowsheet::connectors`]), because slot order is meaningful — a
//! distillation column's `output(0)` is its distillate and `output(1)` its
//! bottoms.
//!
//! # Attribution
//!
//! Pure-Rust port of parts of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2008-2024 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, not
//! the official DWSIM software.
//!
//! Source regions ported here:
//!
//! - `DWSIM.FlowsheetBase/FlowsheetBase.vb` line 513 (`SimulationObjects`) and
//!   line 374 (`GraphicObjects`) — the object registry, merged into one map here
//!   (the drawing half carries no topology this port needs).
//! - `FlowsheetBase.vb` lines 140-159 — `AddObject` / `AddSimulationObject` /
//!   `AddGraphicObject`, ported as [`Flowsheet::add_object`].
//! - `FlowsheetBase.vb` lines 302-326 — `GetFlowsheetGraphicObject`,
//!   `GetFlowsheetSimulationObject`, `GetSelectedFlowsheetSimulationObject`,
//!   ported as [`Flowsheet::object_by_tag`] / [`Flowsheet::id_by_tag`].
//! - `FlowsheetBase.vb` line 1004 and lines 1118-1830 — the `objindex` counting
//!   rule (`count of objects of this type + 1`) feeding the per-type tag prefix,
//!   ported as [`Flowsheet::next_tag`].
//! - `FlowsheetBase.vb` lines 2214-2220 — `CheckTag`, the tag-uniquifying digit
//!   incrementer, ported as [`Flowsheet::unique_tag`].
//! - `FlowsheetBase.vb` lines 165-173 and 2193-2197 — `ConnectObjects` /
//!   `DisconnectObjects` / `ConnectObject`, which delegate to the drawing
//!   surface; the actual rules live in
//!   `DWSIM.Drawing.SkiaSharp/GraphicsSurface/DesignSurface.vb` lines 1270-1503
//!   (`ConnectObject`) and 1505-1564 (`DisconnectObject`), both ported here as
//!   [`Flowsheet::connect`] / [`Flowsheet::disconnect`].
//! - `FlowsheetBase.vb` lines 1839-2191 — `AddConnectedObjects`, the
//!   auto-wiring scheme, ported as [`Flowsheet::add_connected_streams`].
//! - `FlowsheetBase.vb` lines 175-290 — `DeleteSelectedObject`, ported as
//!   [`Flowsheet::remove_object`].
//! - `FlowsheetBase.vb` lines 467-475 — `ResetCalculationStatus`.
//! - `FlowsheetBase.vb` lines 3222-3230 — `Reset`.
//! - `FlowsheetBase.vb` lines 5412-5455 — `UpdateMassAndEnergyBalance`, ported
//!   as [`Flowsheet::update_mass_and_energy_balance`] returning
//!   [`FlowsheetResults`].
//! - `FlowsheetBase.vb` lines 5295-5410 — `GetResultIDs` / `GetResultValue` /
//!   `GetResultUnits`, ported as [`FlowsheetResults::result_ids`] /
//!   [`FlowsheetResults::result_value`] / [`FlowsheetResults::result_unit`].
//!
//! # Excluded DWSIM behavior
//!
//! - **The drawing surface.** `GraphicsSurface`, `DrawingObjects`,
//!   `FindObjectsAtBounds`, `SelectedObject`, zoom, and all coordinate
//!   arithmetic (`DesignSurface.vb` throughout). One consequence is spelled out
//!   on [`Flowsheet::add_connected_streams`]: DWSIM's `AddConnectedObjects`
//!   picks which nearby stream to attach by *screen distance*
//!   (FlowsheetBase.vb:1848-1867), which cannot survive the removal of
//!   geometry, so only its `scheme = 1` branch (create fresh streams) is ported.
//! - **XML/JSON persistence:** `LoadFromXML` / `SaveToXML` / `LoadFromMXML` /
//!   `SaveToMXML` / `GetProcessData` / `LoadProcessData` / `LoadZippedXML` /
//!   snapshots and undo-redo (`RegisterSnapshot`, `GetSnapshot`,
//!   `RestoreSnapshot`, `ProcessUndo`, `ProcessRedo`; FlowsheetBase.vb:2222-2865,
//!   4446-5288). DWSIM calls `RegisterSnapshot` at the top of nearly every
//!   mutator (e.g. :166, :177, :998); those calls are dropped.
//! - **Solver invocation:** `Solve`, `RequestCalculation*`, `CheckStatus`
//!   (which merely calls `FlowsheetSolver.CheckCalculatorStatus()`, :161-163),
//!   and `ChangeCalculationOrder` (:3929, a WinForms dialog). Queueing is in
//!   [`crate::flowsheet::queue`]; execution is the
//!   [`crate::flowsheet_solver`] workstream's.
//! - **.NET host plumbing:** every `MustOverride` GUI hook (`DisplayForm`,
//!   `ShowMessage`, `ShowDebugInfo`, `UpdateInterface`, `RunCodeOnUIThread`,
//!   `UpdateOpenEditForms`, ...), the resource-manager translation layer
//!   (`GetTranslatedString`, :336-372), IronPython/Python.NET scripting
//!   (`RunScript`, `ProcessScripts`, :3279-3898), the weather provider (:42),
//!   the spreadsheet bridge, `Initialize()`'s assembly reflection and compound
//!   databases (:3100-3221), and extender loading (:5457-5487).
//! - **`GetResultValue`'s unit conversion.** Upstream converts each result into
//!   the user's selected display unit system (`ConvertFromSI(...)`,
//!   FlowsheetBase.vb:5325). This port has no display-unit layer, so
//!   [`FlowsheetResults::result_value`] returns SI-internal values and
//!   [`FlowsheetResults::result_unit`] names them.
//! - **GHG emissions and CAPEX/OPEX results.** `GetResultIDs` lists eight
//!   results (:5297-5306); only the two the ported balance actually computes —
//!   residual mass balance and total energy balance — carry values here. The
//!   other six depend on `Results.GHGEmissionsSummary` and the economics module,
//!   neither of which is in this crate's scope; their IDs are still listed and
//!   their values reported as `f64::NAN` rather than fabricated.

use std::collections::HashMap;

use uom::si::f64::{MassRate, Power};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::kilowatt;

use crate::flowsheet::connectors::{Attachment, ConType, ConnectorSlot};
use crate::flowsheet::objects::{FlowsheetObject, ObjectData, ObjectId, ObjectType};
use crate::flowsheet::queue::CalculationQueue;
use crate::flowsheet::streams::PhaseIndex;

/// Why a requested connection is not allowed — the exhaustive set of failure
/// modes DWSIM signals by `Throw New Exception("This connection is not
/// allowed.")` (DesignSurface.vb:1290-1319, :1363, :1393) and
/// `"The requested connection between the given objects cannot be done."`
/// (:1495).
///
/// Upstream throws one of two opaque strings for every case. Splitting them into
/// variants is the point of the port: a caller can tell *why* a wiring attempt
/// failed instead of parsing a message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConnectionError {
    /// One of the two objects is not in the registry.
    #[error("no such object in the flowsheet: `{0}`")]
    UnknownObject(ObjectId),
    /// Source and destination are the same object.
    ///
    /// Upstream has no explicit guard; the case is caught indirectly by the
    /// stream/unit-operation alternation rule, which would reject it either as
    /// "stream to stream" or as "neither end is a stream". Reported explicitly
    /// here because the diagnosis is clearer.
    #[error("cannot connect object `{0}` to itself")]
    SelfConnection(ObjectId),
    /// One endpoint is a pure drawing annotation (`GO_*` / `Nenhum`).
    ///
    /// Upstream's outermost `If` (DesignSurface.vb:1272-1277) simply falls
    /// through and does nothing — silently reporting success to the caller.
    /// This port reports the failure instead; that is a deliberate divergence,
    /// because a silent no-op is indistinguishable from a completed connection.
    #[error("object `{0}` is a drawing annotation and cannot be connected")]
    NotConnectable(ObjectId),
    /// The two objects are already connected the other way round through this
    /// stream, so the request would create a degenerate two-object loop
    /// (DesignSurface.vb:1288-1307).
    #[error("`{from}` and `{to}` are already connected in the opposite direction")]
    AlreadyConnectedInReverse {
        /// Requested source.
        from: ObjectId,
        /// Requested destination.
        to: ObjectId,
    },
    /// Both endpoints are streams, or neither is (DesignSurface.vb:1308-1320).
    ///
    /// A connection must always run stream <-> unit operation: material stream
    /// to material stream, energy stream to energy stream, material to energy,
    /// energy to material, and unit operation to unit operation are all
    /// rejected.
    #[error(
        "invalid pairing: a connection must join exactly one stream to one unit \
         operation (got `{from_type:?}` -> `{to_type:?}`)"
    )]
    InvalidPairing {
        /// Type of the requested source.
        from_type: ObjectType,
        /// Type of the requested destination.
        to_type: ObjectType,
    },
    /// The destination is an energy stream but the source is not a type that
    /// may export a duty (DesignSurface.vb:1387-1394).
    #[error("`{0:?}` cannot supply an energy stream")]
    NotAnEnergySource(ObjectType),
    /// No free inlet slot of the required kind was available on the
    /// destination.
    #[error("no free `{required:?}` inlet slot on `{object}`")]
    NoFreeInlet {
        /// The destination object.
        object: ObjectId,
        /// The slot kind that was needed.
        required: ConType,
    },
    /// No free outlet slot was available on the source.
    #[error("no free outlet slot on `{object}`")]
    NoFreeOutlet {
        /// The source object.
        object: ObjectId,
    },
    /// An explicitly requested slot index does not exist on that object.
    #[error("`{object}` has no {slot:?}")]
    NoSuchSlot {
        /// The object addressed.
        object: ObjectId,
        /// The slot that was asked for.
        slot: ConnectorSlot,
    },
    /// An explicitly requested slot exists but is already attached.
    #[error("{slot:?} of `{object}` is already attached")]
    SlotAlreadyAttached {
        /// The object addressed.
        object: ObjectId,
        /// The slot that was asked for.
        slot: ConnectorSlot,
    },
    /// An explicitly requested slot exists and is free, but is the wrong kind
    /// (e.g. a material stream aimed at an energy terminal).
    #[error("{slot:?} of `{object}` is a `{actual:?}` slot, but a `{required:?}` slot is needed")]
    WrongSlotKind {
        /// The object addressed.
        object: ObjectId,
        /// The slot that was asked for.
        slot: ConnectorSlot,
        /// The kind that slot actually has.
        actual: ConType,
        /// The kind the connection requires.
        required: ConType,
    },
    /// The source's dedicated energy connector is already in use.
    #[error("the energy connector of `{0}` is already attached")]
    EnergyConnectorBusy(ObjectId),
}

/// Errors from flowsheet registry operations that are not connection failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FlowsheetError {
    /// No object with that identity is registered.
    #[error("no such object in the flowsheet: `{0}`")]
    UnknownObject(ObjectId),
    /// An object with that identity is already registered. DWSIM's
    /// `Dictionary.Add` throws the same way (FlowsheetBase.vb:158).
    #[error("an object with id `{0}` is already in the flowsheet")]
    DuplicateObjectId(ObjectId),
    /// A connection operation failed.
    #[error(transparent)]
    Connection(#[from] ConnectionError),
}

/// One resolved edge of the flowsheet graph.
///
/// Derived on demand from the connector slots (which are the single source of
/// truth), so a `Connection` can never disagree with the objects it describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    /// Upstream object.
    pub from: ObjectId,
    /// Which slot on `from` the edge leaves.
    pub from_slot: ConnectorSlot,
    /// Downstream object.
    pub to: ObjectId,
    /// Which slot on `to` the edge enters.
    pub to_slot: ConnectorSlot,
}

/// Flowsheet-level results — DWSIM's `IFlowsheetResults`
/// (FlowsheetBase.vb:4036), restricted to the two quantities
/// `UpdateMassAndEnergyBalance` actually computes (:5412-5455).
///
/// Both are stored in DWSIM's internal units; the `uom` accessors convert.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FlowsheetResults {
    /// Residual overall mass balance \[kg/s\]: total feed mass flow into the
    /// flowsheet minus total product mass flow out of it
    /// (FlowsheetBase.vb:5440-5453).
    ///
    /// A **converged, mass-conserving** flowsheet gives `0`. A non-zero value is
    /// the imbalance, in kg/s: positive means more mass enters than leaves.
    pub residual_mass_balance: f64,
    /// Total energy balance \[kW\]: the sum of every unit operation's net power
    /// generated (`> 0`) or consumed (`< 0`) (FlowsheetBase.vb:5423-5436).
    pub total_energy_balance: f64,
}

impl FlowsheetResults {
    /// Residual mass balance as a `uom` [`MassRate`] \[kg/s\].
    #[must_use]
    pub fn residual_mass_balance_rate(&self) -> MassRate {
        MassRate::new::<kilogram_per_second>(self.residual_mass_balance)
    }

    /// Total energy balance as a `uom` [`Power`] \[W\] (converted from the
    /// stored kW).
    #[must_use]
    pub fn total_energy_balance_power(&self) -> Power {
        Power::new::<kilowatt>(self.total_energy_balance)
    }

    /// The result identifiers a flowsheet exposes — DWSIM's `GetResultIDs`
    /// (FlowsheetBase.vb:5295-5316), in upstream order.
    ///
    /// Six of the eight (GHG emissions and CAPEX/OPEX) are **not computed by
    /// this port** — see the module's "Excluded DWSIM behavior". They are listed
    /// so the identifier set round-trips, and
    /// [`FlowsheetResults::result_value`] returns `f64::NAN` for them rather
    /// than a fabricated number.
    #[must_use]
    pub fn result_ids() -> Vec<&'static str> {
        vec![
            "Total GHG Mass Emissions",
            "Total GHG Molar Emissions",
            "Total CO2eq GHG Mass Emissions",
            "Total CO2eq GHG Molar Emissions",
            "Residual Mass Balance",
            "Total Energy Balance",
            "Total CAPEX",
            "Total OPEX",
        ]
    }

    /// Value of a named result in SI-internal units — DWSIM's `GetResultValue`
    /// (FlowsheetBase.vb:5318-5366), minus the display-unit conversion.
    ///
    /// Returns `f64::NAN` for an unknown identifier (as upstream does) **and**
    /// for the six results this port does not compute.
    #[must_use]
    pub fn result_value(&self, id: &str) -> f64 {
        match id {
            "Residual Mass Balance" => self.residual_mass_balance,
            "Total Energy Balance" => self.total_energy_balance,
            _ => f64::NAN,
        }
    }

    /// Unit label of a named result — DWSIM's `GetResultUnits`
    /// (FlowsheetBase.vb:5368-5410), fixed to this port's SI-internal units
    /// rather than the user's display system.
    #[must_use]
    pub fn result_unit(id: &str) -> &'static str {
        match id {
            "Total GHG Mass Emissions" | "Total CO2eq GHG Mass Emissions" => "kg/s",
            "Total GHG Molar Emissions" | "Total CO2eq GHG Molar Emissions" => "mol/s",
            "Residual Mass Balance" => "kg/s",
            "Total Energy Balance" => "kW",
            "Total CAPEX" => "$",
            "Total OPEX" => "$/year",
            _ => "",
        }
    }
}

/// The object types that may supply an energy stream — DWSIM's allow-list at
/// `DesignSurface.vb:1388-1390`.
const ENERGY_SOURCE_TYPES: [ObjectType; 18] = [
    ObjectType::Cooler,
    ObjectType::Heater,
    ObjectType::Pipe,
    ObjectType::Expander,
    ObjectType::ShortcutColumn,
    ObjectType::DistillationColumn,
    ObjectType::AbsorptionColumn,
    ObjectType::ReboiledAbsorber,
    ObjectType::RefluxedAbsorber,
    ObjectType::OtEnergyRecycle,
    ObjectType::ComponentSeparator,
    ObjectType::SolidSeparator,
    ObjectType::Filter,
    ObjectType::CustomUo,
    ObjectType::CapeOpenUo,
    ObjectType::FlowsheetUo,
    ObjectType::External,
    ObjectType::RctConversion,
];

/// Energy-supplying types that route the duty through a normal **output slot**
/// of kind [`ConType::Energy`] rather than through the dedicated energy
/// connector — DWSIM's exclusion list at `DesignSurface.vb:1395-1398`.
const ENERGY_VIA_OUTPUT_SLOT: [ObjectType; 9] = [
    ObjectType::CapeOpenUo,
    ObjectType::CustomUo,
    ObjectType::DistillationColumn,
    ObjectType::AbsorptionColumn,
    ObjectType::OtEnergyRecycle,
    ObjectType::External,
    ObjectType::RefluxedAbsorber,
    ObjectType::ReboiledAbsorber,
    ObjectType::RctConversion,
];

/// A process flowsheet: the object registry plus its connection topology.
///
/// Objects are keyed by [`ObjectId`] (DWSIM's immutable `Name`) and additionally
/// tracked in insertion order, so iteration is deterministic — a property
/// DWSIM gets for free from .NET's insertion-ordered `Dictionary` and which
/// reports and solver ordering both depend on.
///
/// Owned by value throughout; no lifetimes, no `Box`, no trait objects. Share a
/// flowsheet between threads as `Arc<RwLock<Flowsheet>>` per the workspace
/// shared-state rule.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Flowsheet {
    objects: HashMap<ObjectId, FlowsheetObject>,
    order: Vec<ObjectId>,
    next_id: usize,
    /// Pending calculation requests (DWSIM's `CalculationQueue`,
    /// FlowsheetBase.vb:519). Public so the solver workstream can drain it.
    pub calculation_queue: CalculationQueue,
    /// Last computed flowsheet-level results.
    pub results: FlowsheetResults,
    /// Whether the flowsheet is being solved in dynamic (transient) mode —
    /// DWSIM's `DynamicMode` (FlowsheetBase.vb:44). Consumed by the
    /// [`crate::dynamics`] workstream; this module only stores it.
    pub dynamic_mode: bool,
    /// Whether the last solve completed successfully — DWSIM's `Solved`
    /// (FlowsheetBase.vb:3259).
    pub solved: bool,
    /// Last flowsheet-level error message — DWSIM's `ErrorMessage`
    /// (FlowsheetBase.vb:3233).
    pub error_message: Option<String>,
    /// Accumulated log lines — DWSIM's `MessagesLog` (FlowsheetBase.vb:55).
    /// Plain strings; the message-box plumbing around them is excluded.
    pub messages_log: Vec<String>,
}

impl Flowsheet {
    /// An empty flowsheet.
    #[must_use]
    pub fn new() -> Self {
        Flowsheet::default()
    }

    // -----------------------------------------------------------------
    // Registry
    // -----------------------------------------------------------------

    /// Number of registered objects.
    #[must_use]
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Whether the flowsheet has no objects.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Object identities in insertion order.
    #[must_use]
    pub fn object_ids(&self) -> &[ObjectId] {
        &self.order
    }

    /// Iterate the objects in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &FlowsheetObject> {
        self.order.iter().filter_map(|id| self.objects.get(id))
    }

    /// Borrow an object by identity — DWSIM's `GetObject`
    /// (FlowsheetBase.vb:3950).
    #[must_use]
    pub fn object(&self, id: &ObjectId) -> Option<&FlowsheetObject> {
        self.objects.get(id)
    }

    /// Mutably borrow an object by identity.
    pub fn object_mut(&mut self, id: &ObjectId) -> Option<&mut FlowsheetObject> {
        self.objects.get_mut(id)
    }

    /// Whether an object with this identity is registered.
    #[must_use]
    pub fn contains(&self, id: &ObjectId) -> bool {
        self.objects.contains_key(id)
    }

    /// Identity of the first object carrying this user-visible tag — DWSIM's
    /// `GetFlowsheetSimulationObject` (FlowsheetBase.vb:308-312), which does the
    /// same linear `Where(...).FirstOrDefault` scan.
    ///
    /// Tags are kept unique by [`Flowsheet::next_tag`], so "first" is "the one"
    /// for objects this API created.
    #[must_use]
    pub fn id_by_tag(&self, tag: &str) -> Option<&ObjectId> {
        self.order
            .iter()
            .find(|id| self.objects.get(*id).is_some_and(|o| o.tag == tag))
    }

    /// Borrow the first object carrying this tag.
    #[must_use]
    pub fn object_by_tag(&self, tag: &str) -> Option<&FlowsheetObject> {
        self.iter().find(|o| o.tag == tag)
    }

    /// Mutably borrow the first object carrying this tag.
    pub fn object_by_tag_mut(&mut self, tag: &str) -> Option<&mut FlowsheetObject> {
        let id = self.id_by_tag(tag)?.clone();
        self.objects.get_mut(&id)
    }

    /// Identities of every object of a given type, in insertion order.
    #[must_use]
    pub fn ids_of_type(&self, object_type: ObjectType) -> Vec<ObjectId> {
        self.iter()
            .filter(|o| o.object_type == object_type)
            .map(|o| o.id.clone())
            .collect()
    }

    /// Make a tag unique within this flowsheet — DWSIM's `CheckTag`
    /// (FlowsheetBase.vb:2214-2220).
    ///
    /// While the tag is taken, every maximal run of digits in it is incremented
    /// by one and re-formatted **zero-padded to its original width**, so
    /// `PUMP-1` -> `PUMP-2` and `HX-09` -> `HX-10`. Upstream does this with
    /// `Regex.Replace(tag, "\d+", m => (int.Parse(m) + 1).ToString(new
    /// String("0", m.Length)))`; the port reproduces the semantics without a
    /// regex engine.
    ///
    /// **Divergence.** A tag containing *no* digits sends upstream into an
    /// infinite loop (the replace is a no-op, so the `While` never exits). This
    /// port appends `-1` in that case and then increments normally, which
    /// terminates. A `guard` also caps the search at 1,000,000 attempts and
    /// falls back to appending the object count; that limit is unreachable in
    /// any real flowsheet.
    #[must_use]
    pub fn unique_tag(&self, tag: &str) -> String {
        let mut candidate = tag.to_string();
        let mut guard = 0usize;
        while self.id_by_tag(&candidate).is_some() {
            candidate = match increment_digit_runs(&candidate) {
                Some(next) => next,
                None => format!("{candidate}-1"),
            };
            guard += 1;
            if guard > 1_000_000 {
                return format!("{tag}-{}", self.objects.len() + 1);
            }
        }
        candidate
    }

    /// The next default tag for a new object of `object_type` — DWSIM's
    /// `objindex` rule (FlowsheetBase.vb:1004) composed with the per-type prefix
    /// and then uniquified by [`Flowsheet::unique_tag`].
    ///
    /// `objindex` is *one more than the number of objects of that type already
    /// present*, so the first pump is `PUMP-1`. Note that this counting rule can
    /// collide after a deletion (delete `PUMP-1` of two pumps and the next pump
    /// is `PUMP-2` again) — which is exactly why upstream follows it with
    /// `CheckTag`, and why this method does too.
    #[must_use]
    pub fn next_tag(&self, object_type: ObjectType) -> String {
        let index = self
            .objects
            .values()
            .filter(|o| o.object_type == object_type)
            .count()
            + 1;
        let base = format!("{}{}", object_type.default_tag_prefix(), index);
        self.unique_tag(&base)
    }

    /// Add a new object of `object_type`, returning its generated identity —
    /// DWSIM's `AddObject` / `AddSimulationObject` (FlowsheetBase.vb:140-159),
    /// minus the graphic-object construction.
    ///
    /// If `tag` is `None` the default tag from [`Flowsheet::next_tag`] is used;
    /// a supplied tag is still passed through [`Flowsheet::unique_tag`], so the
    /// returned object's tag may differ from what was asked for.
    ///
    /// **Divergence on identity.** DWSIM names objects with a .NET
    /// `Guid.NewGuid()` (e.g. FlowsheetBase.vb:1287). This port has no GUID
    /// dependency (pure-Rust/Android-portability rule), so identities are a
    /// per-flowsheet monotonic counter rendered as `<prefix><n>`, e.g.
    /// `PUMP-obj-3`. They are unique within a flowsheet and, unlike GUIDs,
    /// **deterministic**, which makes tests reproducible. Use
    /// [`Flowsheet::add_object_with_id`] to keep a GUID read from a DWSIM file.
    pub fn add_object(&mut self, object_type: ObjectType, tag: Option<&str>) -> ObjectId {
        self.next_id += 1;
        let id = ObjectId(format!(
            "{}obj-{}",
            object_type.default_tag_prefix(),
            self.next_id
        ));
        let tag = match tag {
            Some(t) => self.unique_tag(t),
            None => self.next_tag(object_type),
        };
        let object = FlowsheetObject::new(id.clone(), tag, object_type);
        self.order.push(id.clone());
        self.objects.insert(id.clone(), object);
        id
    }

    /// Add an object with a caller-supplied identity — the path used when
    /// loading a flowsheet whose object names (GUIDs) must be preserved
    /// (DWSIM's `AddObject(t, x, y, id, tag)`, FlowsheetBase.vb:145-148).
    ///
    /// The tag is still uniquified.
    ///
    /// # Errors
    /// [`FlowsheetError::DuplicateObjectId`] if that identity is already
    /// registered (upstream's `Dictionary.Add` throws).
    pub fn add_object_with_id(
        &mut self,
        id: ObjectId,
        object_type: ObjectType,
        tag: Option<&str>,
    ) -> Result<(), FlowsheetError> {
        if self.objects.contains_key(&id) {
            return Err(FlowsheetError::DuplicateObjectId(id));
        }
        let tag = match tag {
            Some(t) => self.unique_tag(t),
            None => self.next_tag(object_type),
        };
        let object = FlowsheetObject::new(id.clone(), tag, object_type);
        self.order.push(id.clone());
        self.objects.insert(id, object);
        Ok(())
    }

    /// Remove an object, first disconnecting every connector it holds and
    /// detaching any spec/adjust block that pointed at it — DWSIM's
    /// `DeleteSelectedObject` (FlowsheetBase.vb:175-290).
    ///
    /// Upstream walks the deleted object's input, output and energy connectors
    /// calling `DisconnectObjects` on each (:187-234), and — when the deleted
    /// object is a spec, adjust, or PID block — clears `IsSpecAttached` /
    /// `AttachedSpecId` / `IsAdjustAttached` / `AttachedAdjustId` on the
    /// objects it referenced (:236-274).
    ///
    /// **Divergence, in the safe direction.** Upstream only clears the flags
    /// when the object being deleted *is* the logical block, so deleting the
    /// *target* leaves a dangling `AttachedSpecId` behind. This port instead
    /// sweeps every remaining object and clears any
    /// [`FlowsheetObject::attached_spec`] / [`FlowsheetObject::attached_adjust`]
    /// that names the removed object, which handles both directions and leaves
    /// no dangling references.
    ///
    /// Returns the removed object.
    ///
    /// # Errors
    /// [`FlowsheetError::UnknownObject`] if `id` is not registered.
    pub fn remove_object(&mut self, id: &ObjectId) -> Result<FlowsheetObject, FlowsheetError> {
        if !self.objects.contains_key(id) {
            return Err(FlowsheetError::UnknownObject(id.clone()));
        }

        // Disconnect every peer, in both directions.
        let peers: Vec<ObjectId> = {
            let o = &self.objects[id];
            let mut p: Vec<ObjectId> = Vec::new();
            for c in o.inputs.iter().chain(o.outputs.iter()) {
                if let Some(a) = &c.attachment {
                    p.push(a.peer.clone());
                }
            }
            if let Some(a) = &o.energy_connector.attachment {
                p.push(a.peer.clone());
            }
            p
        };
        for peer in peers {
            // Direction is unknown here, so try both; `disconnect` is a no-op
            // when there is nothing to remove.
            let _ = self.disconnect(id, &peer);
            let _ = self.disconnect(&peer, id);
        }

        // Clear any logical-block attachment naming this object.
        for other in self.objects.values_mut() {
            if other.attached_spec.as_ref() == Some(id) {
                other.attached_spec = None;
            }
            if other.attached_adjust.as_ref() == Some(id) {
                other.attached_adjust = None;
            }
        }

        self.order.retain(|o| o != id);
        Ok(self.objects.remove(id).expect("presence checked above"))
    }

    /// Clear the whole flowsheet — DWSIM's `Reset` (FlowsheetBase.vb:3222-3230),
    /// which clears `SimulationObjects`, `GraphicObjects`, the drawing objects,
    /// and the selected compounds, and resets the flowsheet options.
    ///
    /// This port clears the object registry and the calculation queue, and
    /// resets the results and status flags. The compound list and flowsheet
    /// options are not part of this module's model.
    pub fn reset(&mut self) {
        self.objects.clear();
        self.order.clear();
        self.next_id = 0;
        self.calculation_queue.clear();
        self.results = FlowsheetResults::default();
        self.solved = false;
        self.error_message = None;
        self.messages_log.clear();
    }

    // -----------------------------------------------------------------
    // Topology
    // -----------------------------------------------------------------

    /// Connect `from` to `to`, optionally naming the exact outlet slot on
    /// `from` (`from_slot_index`) and inlet slot on `to` (`to_slot_index`) —
    /// port of `GraphicsSurface.ConnectObject` (DesignSurface.vb:1270-1503),
    /// which DWSIM reaches through `FlowsheetBase.ConnectObjects`
    /// (FlowsheetBase.vb:165-168) and `ConnectObject` (:2193-2197).
    ///
    /// `None` for either index means "first free slot of the required kind",
    /// matching upstream's `fidx = -1` / `tidx = -1` sentinels.
    ///
    /// # The rules, in the order they are applied
    ///
    /// 1. Both objects must exist and be distinct, and neither may be a drawing
    ///    annotation (:1272-1277).
    /// 2. The pair must not already be connected in the opposite direction
    ///    through a stream (:1288-1307).
    /// 3. **Exactly one end must be a stream** (:1308-1320): stream-to-stream
    ///    (of any mix) and unit-operation-to-unit-operation are both rejected.
    /// 4. If `to` is **not** an energy stream (:1321-1385): the inlet slot on
    ///    `to` must be free and of kind [`ConType::Energy`] when `from` is an
    ///    energy stream, or [`ConType::In`] otherwise; then the first free
    ///    outlet slot on `from` (of any kind, as upstream) is taken.
    /// 5. If `to` **is** an energy stream (:1386-1450): `from` must be one of
    ///    the 18 duty-exporting types (:1388-1390); most of them route through
    ///    `from`'s dedicated energy connector into `to`'s inlet 0, while the
    ///    nine types listed at :1395-1398 instead use a free
    ///    [`ConType::Energy`] **outlet** slot on `from`.
    ///
    /// # Divergences from upstream, all deliberate
    ///
    /// - **Validation is complete before anything is mutated.** DWSIM sets
    ///   `InConSlot.IsAttached = True` while it is still searching
    ///   (:1328, :1338, ...), so a request that fails later leaves the
    ///   destination slot falsely marked attached — a real defect that corrupts
    ///   the flowsheet. This port resolves both endpoints first and commits only
    ///   on success.
    /// - **A busy inlet 0 on the energy stream is an error**, not a silent
    ///   overwrite. Upstream attaches `gObjTo.InputConnectors(0)` unconditionally
    ///   at :1405-1409, orphaning whatever was there.
    /// - **Drawing annotations report an error** rather than silently doing
    ///   nothing (see [`ConnectionError::NotConnectable`]).
    ///
    /// # Errors
    /// One of the [`ConnectionError`] variants, each naming the specific rule
    /// that failed.
    pub fn connect(
        &mut self,
        from: &ObjectId,
        to: &ObjectId,
        from_slot_index: Option<usize>,
        to_slot_index: Option<usize>,
    ) -> Result<Connection, ConnectionError> {
        if from == to {
            return Err(ConnectionError::SelfConnection(from.clone()));
        }
        let from_obj = self
            .objects
            .get(from)
            .ok_or_else(|| ConnectionError::UnknownObject(from.clone()))?;
        let to_obj = self
            .objects
            .get(to)
            .ok_or_else(|| ConnectionError::UnknownObject(to.clone()))?;

        // (1) drawing annotations cannot be wired (DesignSurface.vb:1272-1277).
        if from_obj.object_type.is_drawing_only() {
            return Err(ConnectionError::NotConnectable(from.clone()));
        }
        if to_obj.object_type.is_drawing_only() {
            return Err(ConnectionError::NotConnectable(to.clone()));
        }

        // (2) reverse-connection guard (DesignSurface.vb:1288-1307).
        if from_obj.object_type.is_stream()
            && from_obj.inputs.first().and_then(|c| c.peer()) == Some(to)
        {
            return Err(ConnectionError::AlreadyConnectedInReverse {
                from: from.clone(),
                to: to.clone(),
            });
        }
        if to_obj.object_type.is_stream()
            && to_obj.outputs.first().and_then(|c| c.peer()) == Some(from)
        {
            return Err(ConnectionError::AlreadyConnectedInReverse {
                from: from.clone(),
                to: to.clone(),
            });
        }

        // (3) exactly one end must be a stream (DesignSurface.vb:1308-1320).
        if from_obj.object_type.is_stream() == to_obj.object_type.is_stream() {
            return Err(ConnectionError::InvalidPairing {
                from_type: from_obj.object_type,
                to_type: to_obj.object_type,
            });
        }

        let (from_slot, to_slot) = if !to_obj.object_type.is_energy_stream() {
            // (4) ordinary destination (DesignSurface.vb:1321-1385).
            let required = if from_obj.object_type.is_energy_stream() {
                ConType::Energy
            } else {
                ConType::In
            };
            let to_slot = resolve_inlet(to_obj, to_slot_index, required)?;
            let from_slot = resolve_outlet(from_obj, from_slot_index, None)?;
            (from_slot, to_slot)
        } else {
            // (5) destination is an energy stream (DesignSurface.vb:1386-1450).
            if !ENERGY_SOURCE_TYPES.contains(&from_obj.object_type) {
                return Err(ConnectionError::NotAnEnergySource(from_obj.object_type));
            }
            if ENERGY_VIA_OUTPUT_SLOT.contains(&from_obj.object_type) {
                let to_slot = resolve_inlet(to_obj, to_slot_index, ConType::In)?;
                let from_slot = resolve_outlet(from_obj, from_slot_index, Some(ConType::Energy))?;
                (from_slot, to_slot)
            } else {
                if from_obj.energy_connector.is_attached() {
                    return Err(ConnectionError::EnergyConnectorBusy(from.clone()));
                }
                // Upstream hard-codes inlet 0 here (DesignSurface.vb:1405-1409).
                let to_slot = resolve_inlet(to_obj, Some(0), ConType::In)?;
                (ConnectorSlot::Energy, to_slot)
            }
        };

        self.commit_connection(from, from_slot, to, to_slot);
        Ok(Connection {
            from: from.clone(),
            from_slot,
            to: to.clone(),
            to_slot,
        })
    }

    /// Write the resolved attachment onto both endpoints. Private: callers go
    /// through [`Flowsheet::connect`], which validates first.
    fn commit_connection(
        &mut self,
        from: &ObjectId,
        from_slot: ConnectorSlot,
        to: &ObjectId,
        to_slot: ConnectorSlot,
    ) {
        if let Some(o) = self.objects.get_mut(from) {
            *slot_mut(o, from_slot).expect("slot resolved during validation") = Some(Attachment {
                peer: to.clone(),
                peer_slot: to_slot,
            });
        }
        if let Some(o) = self.objects.get_mut(to) {
            *slot_mut(o, to_slot).expect("slot resolved during validation") = Some(Attachment {
                peer: from.clone(),
                peer_slot: from_slot,
            });
        }
    }

    /// Remove every connection between `from` and `to`, in that direction —
    /// port of `GraphicsSurface.DisconnectObject` (DesignSurface.vb:1505-1564),
    /// reached upstream through `FlowsheetBase.DisconnectObjects` (:170-173).
    ///
    /// Upstream scans `from`'s inlets, then its outlets, then its energy
    /// connector, clearing both ends of every attachment whose peer is `to`.
    /// This port does the same and additionally clears the peer slot by the
    /// **recorded peer slot** rather than by a stored index, so a stale index
    /// cannot orphan a connector.
    ///
    /// Returns how many connections were removed; `0` is not an error (upstream
    /// is likewise silent).
    ///
    /// # Errors
    /// [`FlowsheetError::UnknownObject`] if either object is not registered.
    pub fn disconnect(&mut self, from: &ObjectId, to: &ObjectId) -> Result<usize, FlowsheetError> {
        if !self.objects.contains_key(from) {
            return Err(FlowsheetError::UnknownObject(from.clone()));
        }
        if !self.objects.contains_key(to) {
            return Err(FlowsheetError::UnknownObject(to.clone()));
        }

        // Collect the (local slot, peer slot) pairs to clear.
        let to_clear: Vec<(ConnectorSlot, ConnectorSlot)> = {
            let o = &self.objects[from];
            let mut v = Vec::new();
            for (i, c) in o.inputs.iter().enumerate() {
                if let Some(a) = &c.attachment {
                    if &a.peer == to {
                        v.push((ConnectorSlot::Input(i), a.peer_slot));
                    }
                }
            }
            for (i, c) in o.outputs.iter().enumerate() {
                if let Some(a) = &c.attachment {
                    if &a.peer == to {
                        v.push((ConnectorSlot::Output(i), a.peer_slot));
                    }
                }
            }
            if let Some(a) = &o.energy_connector.attachment {
                if &a.peer == to {
                    v.push((ConnectorSlot::Energy, a.peer_slot));
                }
            }
            v
        };

        let removed = to_clear.len();
        for (local, peer_slot) in to_clear {
            if let Some(o) = self.objects.get_mut(from) {
                if let Some(s) = slot_mut(o, local) {
                    *s = None;
                }
            }
            if let Some(o) = self.objects.get_mut(to) {
                if let Some(s) = slot_mut(o, peer_slot) {
                    *s = None;
                }
            }
        }
        Ok(removed)
    }

    /// Every edge of the flowsheet graph, derived from the connector slots.
    ///
    /// Enumerated from each object's **outlet** slots and its energy connector,
    /// in insertion order, so each edge appears exactly once and the ordering is
    /// deterministic. (An inlet attachment is always the mirror image of some
    /// other object's outlet or energy attachment.)
    #[must_use]
    pub fn connections(&self) -> Vec<Connection> {
        let mut out = Vec::new();
        for o in self.iter() {
            for (i, c) in o.outputs.iter().enumerate() {
                if let Some(a) = &c.attachment {
                    out.push(Connection {
                        from: o.id.clone(),
                        from_slot: ConnectorSlot::Output(i),
                        to: a.peer.clone(),
                        to_slot: a.peer_slot,
                    });
                }
            }
            if let Some(a) = &o.energy_connector.attachment {
                out.push(Connection {
                    from: o.id.clone(),
                    from_slot: ConnectorSlot::Energy,
                    to: a.peer.clone(),
                    to_slot: a.peer_slot,
                });
            }
        }
        out
    }

    /// Create a fresh material/energy stream on each of a unit operation's slots
    /// and wire them up — the portable half of DWSIM's `AddConnectedObjects`
    /// (FlowsheetBase.vb:1839-2191).
    ///
    /// Upstream's routine has two branches per object type. `scheme = 2` looks
    /// for **existing** streams lying within 250 drawing units of the object and
    /// attaches the nearest (`FindObjectsAtBounds`, then
    /// `OrderBy(|dx| + |dy|)`, :1848-1867); that branch depends entirely on
    /// screen coordinates and therefore cannot be ported — see the module's
    /// "Excluded DWSIM behavior". The `scheme = 1` branch creates brand-new
    /// streams and connects them (e.g. :1913-1924 for a compressor), and that is
    /// what this method does, generalised: rather than 20 hand-written per-type
    /// cases, it walks the object's actual connector layout and creates one
    /// stream of the matching kind per free slot — which reproduces every
    /// upstream case whose slot kinds it is derived from, without the
    /// per-type duplication.
    ///
    /// Slots already attached are left alone. Returns the identities of the
    /// streams it created, inlets first then outlets, in slot order.
    ///
    /// # Errors
    /// [`FlowsheetError::UnknownObject`] if `id` is not registered. Individual
    /// connection failures are impossible by construction here (each stream is
    /// created fresh for one specific free slot), but are propagated if they
    /// somehow occur.
    pub fn add_connected_streams(
        &mut self,
        id: &ObjectId,
    ) -> Result<Vec<ObjectId>, FlowsheetError> {
        let obj = self
            .objects
            .get(id)
            .ok_or_else(|| FlowsheetError::UnknownObject(id.clone()))?;
        if obj.object_type.is_stream() {
            // A stream has no unit operation to wire streams onto.
            return Ok(Vec::new());
        }
        let free_inlets: Vec<(usize, ConType)> = obj
            .inputs
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.is_attached())
            .map(|(i, c)| (i, c.connector_type))
            .collect();
        let free_outlets: Vec<(usize, ConType)> = obj
            .outputs
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.is_attached())
            .map(|(i, c)| (i, c.connector_type))
            .collect();

        let mut created = Vec::new();
        for (i, kind) in free_inlets {
            let stream_type = match kind {
                ConType::Energy => ObjectType::EnergyStream,
                _ => ObjectType::MaterialStream,
            };
            let sid = self.add_object(stream_type, None);
            self.connect(&sid, id, Some(0), Some(i))?;
            created.push(sid);
        }
        for (i, kind) in free_outlets {
            let stream_type = match kind {
                ConType::Energy => ObjectType::EnergyStream,
                _ => ObjectType::MaterialStream,
            };
            let sid = self.add_object(stream_type, None);
            self.connect(id, &sid, Some(i), Some(0))?;
            created.push(sid);
        }
        Ok(created)
    }

    // -----------------------------------------------------------------
    // Status and balances
    // -----------------------------------------------------------------

    /// Mark every object as un-calculated and dirty — DWSIM's
    /// `ResetCalculationStatus` (FlowsheetBase.vb:467-475), which sets
    /// `SetDirtyStatus(True)`, `Calculated = False` and
    /// `GraphicObject.Calculated = False` on each simulation object.
    ///
    /// Call this before a cold solve so nothing is skipped as "already done".
    pub fn reset_calculation_status(&mut self) {
        for o in self.objects.values_mut() {
            o.calculated = false;
            o.dirty = true;
        }
    }

    /// Objects whose last calculation recorded an error message.
    ///
    /// DWSIM's `CheckStatus` (FlowsheetBase.vb:161-163) does something entirely
    /// different — it asks the solver whether the user has requested an abort —
    /// and is excluded (see the module header). This method is the useful
    /// flowsheet-level status query that survives.
    #[must_use]
    pub fn objects_with_errors(&self) -> Vec<&FlowsheetObject> {
        self.iter().filter(|o| o.error_message.is_some()).collect()
    }

    /// Material streams that enter the flowsheet from outside — those whose
    /// inlet slot 0 is unattached (FlowsheetBase.vb:5445-5447).
    #[must_use]
    pub fn feed_streams(&self) -> Vec<&FlowsheetObject> {
        self.iter()
            .filter(|o| {
                o.object_type.is_material_stream()
                    && o.inputs.first().is_some_and(|c| !c.is_attached())
            })
            .collect()
    }

    /// Material streams that leave the flowsheet — those whose outlet slot 0 is
    /// unattached (FlowsheetBase.vb:5448-5450).
    #[must_use]
    pub fn product_streams(&self) -> Vec<&FlowsheetObject> {
        self.iter()
            .filter(|o| {
                o.object_type.is_material_stream()
                    && o.outputs.first().is_some_and(|c| !c.is_attached())
            })
            .collect()
    }

    /// Recompute and store the flowsheet-level mass and energy balance — DWSIM's
    /// `UpdateMassAndEnergyBalance` (FlowsheetBase.vb:5412-5455).
    ///
    /// - **Total energy balance** \[kW\] = the sum of every unit operation's
    ///   `GetPowerGeneratedOrConsumed()` (:5423-5435). Indicators are excluded
    ///   (:5414). Objects whose power is `None` (never calculated) contribute
    ///   zero.
    /// - **Residual mass balance** \[kg/s\] = the sum of the mass flows of every
    ///   material stream with a free inlet (a flowsheet feed) minus the sum over
    ///   every material stream with a free outlet (a flowsheet product)
    ///   (:5440-5453). A stream that is free at *both* ends is counted in both
    ///   sums and therefore cancels — faithful to upstream, whose `If`s are
    ///   independent, not exclusive.
    ///
    /// **Excluded from this port:** upstream's per-equipment efficiency lookup
    /// (:5426-5434) reads `Eficiencia` / `ThermalEfficiency` / `Efficiency` /
    /// `AdiabaticEfficiency` by .NET reflection into a local `eff` that is then
    /// **never used**. It is dead code and is not reproduced.
    ///
    /// Returns the freshly computed [`FlowsheetResults`], which is also stored
    /// in [`Flowsheet::results`].
    pub fn update_mass_and_energy_balance(&mut self) -> FlowsheetResults {
        let total_energy: f64 = self
            .iter()
            .filter(|o| o.object_type.is_unit_operation())
            .filter_map(|o| match &o.data {
                ObjectData::UnitOperation { power, .. } => *power,
                _ => None,
            })
            .sum();

        let mut total_mass = 0.0f64;
        for o in self.iter() {
            if !o.object_type.is_material_stream() {
                continue;
            }
            let Some(ms) = o.data.as_material() else {
                continue;
            };
            let mf = ms
                .phase(PhaseIndex::Mixture)
                .properties
                .massflow
                .unwrap_or(0.0);
            if o.inputs.first().is_some_and(|c| !c.is_attached()) {
                total_mass += mf;
            }
            if o.outputs.first().is_some_and(|c| !c.is_attached()) {
                total_mass -= mf;
            }
        }

        self.results = FlowsheetResults {
            residual_mass_balance: total_mass,
            total_energy_balance: total_energy,
        };
        self.results
    }
}

/// Mutable access to one connector slot's attachment, or `None` if the slot
/// index is out of range.
fn slot_mut(object: &mut FlowsheetObject, slot: ConnectorSlot) -> Option<&mut Option<Attachment>> {
    match slot {
        ConnectorSlot::Input(i) => object.inputs.get_mut(i).map(|c| &mut c.attachment),
        ConnectorSlot::Output(i) => object.outputs.get_mut(i).map(|c| &mut c.attachment),
        ConnectorSlot::Energy => Some(&mut object.energy_connector.attachment),
    }
}

/// Resolve the destination inlet slot: the requested index if given (which must
/// exist, be free, and have kind `required`), else the first free slot of that
/// kind — DWSIM's two branches at DesignSurface.vb:1323-1341 (and :1343-1361 for
/// the energy case, which differs only in `required`).
fn resolve_inlet(
    object: &FlowsheetObject,
    index: Option<usize>,
    required: ConType,
) -> Result<ConnectorSlot, ConnectionError> {
    match index {
        Some(i) => {
            let c = object
                .inputs
                .get(i)
                .ok_or_else(|| ConnectionError::NoSuchSlot {
                    object: object.id.clone(),
                    slot: ConnectorSlot::Input(i),
                })?;
            if c.is_attached() {
                return Err(ConnectionError::SlotAlreadyAttached {
                    object: object.id.clone(),
                    slot: ConnectorSlot::Input(i),
                });
            }
            if c.connector_type != required {
                return Err(ConnectionError::WrongSlotKind {
                    object: object.id.clone(),
                    slot: ConnectorSlot::Input(i),
                    actual: c.connector_type,
                    required,
                });
            }
            Ok(ConnectorSlot::Input(i))
        }
        None => object
            .inputs
            .iter()
            .position(|c| !c.is_attached() && c.connector_type == required)
            .map(ConnectorSlot::Input)
            .ok_or_else(|| ConnectionError::NoFreeInlet {
                object: object.id.clone(),
                required,
            }),
    }
}

/// Resolve the source outlet slot: the requested index if given (which must
/// exist and be free), else the first free outlet — DWSIM's branches at
/// DesignSurface.vb:1367-1385 and :1431-1449.
///
/// `required` is `None` for the ordinary path, where upstream takes the first
/// free outlet **regardless of kind** (:1368-1369), and `Some(ConType::Energy)`
/// for the duty-export path, where it filters on `ConType.ConEn` (:1433).
/// Note that upstream applies no kind check at all when an explicit `fidx` is
/// given (:1378, :1442); that asymmetry is reproduced.
fn resolve_outlet(
    object: &FlowsheetObject,
    index: Option<usize>,
    required: Option<ConType>,
) -> Result<ConnectorSlot, ConnectionError> {
    match index {
        Some(i) => {
            let c = object
                .outputs
                .get(i)
                .ok_or_else(|| ConnectionError::NoSuchSlot {
                    object: object.id.clone(),
                    slot: ConnectorSlot::Output(i),
                })?;
            if c.is_attached() {
                return Err(ConnectionError::SlotAlreadyAttached {
                    object: object.id.clone(),
                    slot: ConnectorSlot::Output(i),
                });
            }
            Ok(ConnectorSlot::Output(i))
        }
        None => object
            .outputs
            .iter()
            .position(|c| !c.is_attached() && required.is_none_or(|r| c.connector_type == r))
            .map(ConnectorSlot::Output)
            .ok_or_else(|| ConnectionError::NoFreeOutlet {
                object: object.id.clone(),
            }),
    }
}

/// Increment every maximal run of ASCII digits in `tag` by one, zero-padded to
/// the run's original width — the regex-free equivalent of DWSIM's
/// `Regex.Replace(tag, "\d+", m => (int.Parse(m) + 1).ToString(new String("0",
/// m.Length)))` (FlowsheetBase.vb:2217).
///
/// Returns `None` when the tag contains no digits (upstream would loop forever
/// in that case; see [`Flowsheet::unique_tag`]). A run that overflows its
/// original width simply grows, matching .NET's `ToString("000")` behaviour
/// (`99` -> `100`). Runs longer than a `u128` can hold are left unchanged.
fn increment_digit_runs(tag: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut out = String::with_capacity(tag.len() + 1);
    let mut i = 0;
    let mut found = false;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let run = &tag[start..i];
            found = true;
            match run.parse::<u128>() {
                Ok(v) => out.push_str(&format!("{:0width$}", v + 1, width = run.len())),
                Err(_) => out.push_str(run),
            }
        } else {
            // Push the whole non-digit character, not the byte, so non-ASCII
            // tags survive intact.
            let ch = tag[i..].chars().next().expect("index is a char boundary");
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    found.then_some(out)
}

#[cfg(test)]
mod tests {
    //! # V&V — flowsheet registry, topology, and balances
    //!
    //! **Methodology.** Verification against the transcribed upstream logic:
    //! the connection rules of `DesignSurface.ConnectObject`
    //! (DesignSurface.vb:1270-1503), the disconnection sweep (:1505-1564),
    //! `CheckTag`'s digit incrementer (FlowsheetBase.vb:2214-2220), the
    //! `objindex` tag rule (:1004), `ResetCalculationStatus` (:467), and
    //! `UpdateMassAndEnergyBalance` (:5412). The mass and energy balances are
    //! checked against hand-computed sums, not against a physical benchmark —
    //! no thermodynamics is evaluated in this module. Numbers recorded
    //! 2026-08-11, release build.

    use super::*;
    use crate::flowsheet::streams::PhaseIndex;
    use approx::assert_relative_eq;

    /// A flowsheet with one pump and two material streams (unconnected).
    fn pump_and_streams() -> (Flowsheet, ObjectId, ObjectId, ObjectId) {
        let mut fs = Flowsheet::new();
        let pump = fs.add_object(ObjectType::Pump, None);
        let s_in = fs.add_object(ObjectType::MaterialStream, None);
        let s_out = fs.add_object(ObjectType::MaterialStream, None);
        (fs, pump, s_in, s_out)
    }

    fn set_mass_flow(fs: &mut Flowsheet, id: &ObjectId, kg_per_s: f64) {
        let o = fs.object_mut(id).unwrap();
        let ms = o.data.as_material_mut().unwrap();
        ms.phase_mut(PhaseIndex::Mixture).properties.massflow = Some(kg_per_s);
    }

    /// **Methodology.** The `objindex` rule (FlowsheetBase.vb:1004) numbers the
    /// n-th object of a type `n`, prefixed per type; `CheckTag` (:2214) then
    /// uniquifies. Material streams have an empty prefix (:1283) so their tags
    /// are bare numbers.
    /// **Result (2026-08-11):** pumps tag `PUMP-1`, `PUMP-2`; material streams
    /// tag `1`, `2`; an explicitly requested duplicate tag `PUMP-1` becomes
    /// `PUMP-2` then `PUMP-3`.
    #[test]
    fn default_tags_follow_the_objindex_rule_and_stay_unique() {
        let mut fs = Flowsheet::new();
        let p1 = fs.add_object(ObjectType::Pump, None);
        let p2 = fs.add_object(ObjectType::Pump, None);
        assert_eq!(fs.object(&p1).unwrap().tag, "PUMP-1");
        assert_eq!(fs.object(&p2).unwrap().tag, "PUMP-2");

        let m1 = fs.add_object(ObjectType::MaterialStream, None);
        let m2 = fs.add_object(ObjectType::MaterialStream, None);
        assert_eq!(fs.object(&m1).unwrap().tag, "1");
        assert_eq!(fs.object(&m2).unwrap().tag, "2");

        let p3 = fs.add_object(ObjectType::Pump, Some("PUMP-1"));
        assert_eq!(
            fs.object(&p3).unwrap().tag,
            "PUMP-3",
            "PUMP-1 taken -> PUMP-2 taken -> PUMP-3"
        );
    }

    /// **Methodology.** `CheckTag`'s digit incrementer (FlowsheetBase.vb:2217)
    /// increments every digit run zero-padded to its original width. Checked
    /// directly on the helper.
    /// **Result (2026-08-11):** `PUMP-1`->`PUMP-2`; `HX-09`->`HX-10`;
    /// `E99`->`E100`; `A1B2`->`A2B3`; `PUMP` (no digits) -> `None`.
    #[test]
    fn digit_incrementer_matches_upstream_regex_semantics() {
        assert_eq!(increment_digit_runs("PUMP-1").as_deref(), Some("PUMP-2"));
        assert_eq!(increment_digit_runs("HX-09").as_deref(), Some("HX-10"));
        assert_eq!(increment_digit_runs("E99").as_deref(), Some("E100"));
        assert_eq!(increment_digit_runs("A1B2").as_deref(), Some("A2B3"));
        assert_eq!(increment_digit_runs("PUMP"), None);
    }

    /// **Methodology.** A digit-free tag makes upstream's `CheckTag` loop
    /// forever; this port appends `-1` and then increments (documented
    /// divergence).
    /// **Result (2026-08-11):** two objects both asking for tag `FEED` get
    /// `FEED` and `FEED-1`; a third gets `FEED-2`.
    #[test]
    fn digit_free_tags_terminate_instead_of_looping() {
        let mut fs = Flowsheet::new();
        let a = fs.add_object(ObjectType::MaterialStream, Some("FEED"));
        let b = fs.add_object(ObjectType::MaterialStream, Some("FEED"));
        let c = fs.add_object(ObjectType::MaterialStream, Some("FEED"));
        assert_eq!(fs.object(&a).unwrap().tag, "FEED");
        assert_eq!(fs.object(&b).unwrap().tag, "FEED-1");
        assert_eq!(fs.object(&c).unwrap().tag, "FEED-2");
    }

    /// **Methodology.** A valid stream -> unit-operation -> stream wiring must
    /// take the first free slot of the right kind on each end, mark both ends
    /// attached, and appear once in [`Flowsheet::connections`].
    /// **Result (2026-08-11):** inlet stream lands on the pump's `Input(0)` (its
    /// `ConIn` slot, not the `ConEn` slot at index 1); the outlet stream is fed
    /// from `Output(0)`; two connections enumerated.
    #[test]
    fn valid_stream_to_unit_operation_wiring() {
        let (mut fs, pump, s_in, s_out) = pump_and_streams();
        let c1 = fs.connect(&s_in, &pump, None, None).unwrap();
        assert_eq!(c1.from_slot, ConnectorSlot::Output(0));
        assert_eq!(c1.to_slot, ConnectorSlot::Input(0));
        let c2 = fs.connect(&pump, &s_out, None, None).unwrap();
        assert_eq!(c2.from_slot, ConnectorSlot::Output(0));
        assert_eq!(c2.to_slot, ConnectorSlot::Input(0));

        assert!(fs.object(&pump).unwrap().inputs[0].is_attached());
        assert!(!fs.object(&pump).unwrap().inputs[1].is_attached());
        assert_eq!(
            fs.object(&s_in).unwrap().outputs[0].peer(),
            Some(&pump),
            "peer recorded on the stream side too"
        );
        assert_eq!(fs.connections().len(), 2);
    }

    /// **Methodology.** The pairing rule (DesignSurface.vb:1308-1320) rejects
    /// every same-class pairing: material-to-material, energy-to-energy,
    /// material-to-energy, energy-to-material, and unit-op-to-unit-op.
    /// **Result (2026-08-11):** all five return `InvalidPairing`; a
    /// self-connection returns `SelfConnection`; a drawing annotation returns
    /// `NotConnectable`.
    #[test]
    fn pairing_rule_rejects_every_same_class_connection() {
        let mut fs = Flowsheet::new();
        let m1 = fs.add_object(ObjectType::MaterialStream, None);
        let m2 = fs.add_object(ObjectType::MaterialStream, None);
        let e1 = fs.add_object(ObjectType::EnergyStream, None);
        let e2 = fs.add_object(ObjectType::EnergyStream, None);
        let p1 = fs.add_object(ObjectType::Pump, None);
        let p2 = fs.add_object(ObjectType::Valve, None);
        let table = fs.add_object(ObjectType::GoTable, None);

        for (a, b) in [(&m1, &m2), (&e1, &e2), (&m1, &e1), (&e1, &m1), (&p1, &p2)] {
            let err = fs.connect(a, b, None, None).unwrap_err();
            assert!(
                matches!(err, ConnectionError::InvalidPairing { .. }),
                "expected InvalidPairing for {a} -> {b}, got {err:?}"
            );
        }
        assert!(matches!(
            fs.connect(&m1, &m1, None, None).unwrap_err(),
            ConnectionError::SelfConnection(_)
        ));
        assert!(matches!(
            fs.connect(&m1, &table, None, None).unwrap_err(),
            ConnectionError::NotConnectable(_)
        ));
    }

    /// **Methodology.** The reverse-connection guard
    /// (DesignSurface.vb:1288-1307): once `pump -> stream` exists, the request
    /// `stream -> pump` must be refused, because it would loop the two objects
    /// through one stream.
    /// **Result (2026-08-11):** `AlreadyConnectedInReverse`.
    #[test]
    fn reverse_connection_through_a_stream_is_refused() {
        let (mut fs, pump, _s_in, s_out) = pump_and_streams();
        fs.connect(&pump, &s_out, None, None).unwrap();
        let err = fs.connect(&s_out, &pump, None, None).unwrap_err();
        assert!(matches!(
            err,
            ConnectionError::AlreadyConnectedInReverse { .. }
        ));
    }

    /// **Methodology.** Explicit slot indices must be validated on existence,
    /// freeness, and kind (DesignSurface.vb:1334-1340). A material stream aimed
    /// at the pump's energy terminal (`Input(1)`, `ConEn`) is the wrong kind; a
    /// second stream aimed at the already-used `Input(0)` is busy; index 9 does
    /// not exist.
    /// **Result (2026-08-11):** `WrongSlotKind`, `SlotAlreadyAttached`, and
    /// `NoSuchSlot` respectively.
    #[test]
    fn explicit_slot_indices_are_validated() {
        let (mut fs, pump, s_in, s_out) = pump_and_streams();
        let err = fs.connect(&s_in, &pump, Some(0), Some(1)).unwrap_err();
        assert!(
            matches!(
                err,
                ConnectionError::WrongSlotKind {
                    required: ConType::In,
                    actual: ConType::Energy,
                    ..
                }
            ),
            "got {err:?}"
        );

        fs.connect(&s_in, &pump, Some(0), Some(0)).unwrap();
        let err = fs.connect(&s_out, &pump, Some(0), Some(0)).unwrap_err();
        assert!(matches!(
            err,
            ConnectionError::SlotAlreadyAttached {
                slot: ConnectorSlot::Input(0),
                ..
            }
        ));

        let err = fs.connect(&s_out, &pump, Some(0), Some(9)).unwrap_err();
        assert!(matches!(
            err,
            ConnectionError::NoSuchSlot {
                slot: ConnectorSlot::Input(9),
                ..
            }
        ));
    }

    /// **Methodology.** An energy stream may only enter a `ConEn` slot
    /// (DesignSurface.vb:1343-1365). Wiring one into a pump must land on the
    /// pump's `Input(1)`, not `Input(0)`; a mixer, which has no energy slot at
    /// all, must refuse it.
    /// **Result (2026-08-11):** pump gets `Input(1)`; the mixer returns
    /// `NoFreeInlet { required: Energy }`.
    #[test]
    fn energy_stream_lands_only_in_an_energy_slot() {
        let mut fs = Flowsheet::new();
        let pump = fs.add_object(ObjectType::Pump, None);
        let e = fs.add_object(ObjectType::EnergyStream, None);
        let c = fs.connect(&e, &pump, None, None).unwrap();
        assert_eq!(c.to_slot, ConnectorSlot::Input(1));

        let mixer = fs.add_object(ObjectType::NodeIn, None);
        let e2 = fs.add_object(ObjectType::EnergyStream, None);
        let err = fs.connect(&e2, &mixer, None, None).unwrap_err();
        assert!(matches!(
            err,
            ConnectionError::NoFreeInlet {
                required: ConType::Energy,
                ..
            }
        ));
    }

    /// **Methodology.** Duty export to an energy stream
    /// (DesignSurface.vb:1386-1450). A cooler is in the allow-list (:1388) and
    /// is *not* in the output-slot exclusion list (:1395), so it must route
    /// through its dedicated energy connector into the energy stream's inlet 0.
    /// A distillation column *is* in the exclusion list, so it must use a free
    /// `ConEn` **outlet** slot (its `Output(10)`, "Condenser Duty"). A pump is
    /// not in the allow-list at all.
    /// **Result (2026-08-11):** cooler -> `ConnectorSlot::Energy`; column ->
    /// `Output(10)`; pump -> `NotAnEnergySource(Pump)`; a second energy stream
    /// on the same cooler -> `EnergyConnectorBusy`.
    #[test]
    fn duty_export_paths_match_the_two_upstream_branches() {
        let mut fs = Flowsheet::new();
        let cooler = fs.add_object(ObjectType::Cooler, None);
        let e1 = fs.add_object(ObjectType::EnergyStream, None);
        let c = fs.connect(&cooler, &e1, None, None).unwrap();
        assert_eq!(c.from_slot, ConnectorSlot::Energy);
        assert_eq!(c.to_slot, ConnectorSlot::Input(0));

        let e2 = fs.add_object(ObjectType::EnergyStream, None);
        assert!(matches!(
            fs.connect(&cooler, &e2, None, None).unwrap_err(),
            ConnectionError::EnergyConnectorBusy(_)
        ));

        let col = fs.add_object(ObjectType::DistillationColumn, None);
        let e3 = fs.add_object(ObjectType::EnergyStream, None);
        let c = fs.connect(&col, &e3, None, None).unwrap();
        assert_eq!(
            c.from_slot,
            ConnectorSlot::Output(10),
            "Condenser Duty is the only ConEn outlet"
        );

        let pump = fs.add_object(ObjectType::Pump, None);
        let e4 = fs.add_object(ObjectType::EnergyStream, None);
        assert!(matches!(
            fs.connect(&pump, &e4, None, None).unwrap_err(),
            ConnectionError::NotAnEnergySource(ObjectType::Pump)
        ));
    }

    /// **Methodology.** A failed connection must leave the flowsheet exactly as
    /// it was — the defect at DesignSurface.vb:1328 (upstream marks the
    /// destination slot attached before it knows the source can be resolved).
    /// A mixer with all six inlets used, asked to take a seventh stream, must
    /// fail without touching anything.
    /// **Result (2026-08-11):** `NoFreeInlet`; the mixer still has exactly 6
    /// attached inlets and the rejected stream is still fully free.
    #[test]
    fn a_failed_connection_mutates_nothing() {
        let mut fs = Flowsheet::new();
        let mixer = fs.add_object(ObjectType::NodeIn, None);
        let mut feeds = Vec::new();
        for _ in 0..6 {
            let s = fs.add_object(ObjectType::MaterialStream, None);
            fs.connect(&s, &mixer, None, None).unwrap();
            feeds.push(s);
        }
        let extra = fs.add_object(ObjectType::MaterialStream, None);
        let err = fs.connect(&extra, &mixer, None, None).unwrap_err();
        assert!(matches!(
            err,
            ConnectionError::NoFreeInlet {
                required: ConType::In,
                ..
            }
        ));
        assert_eq!(
            fs.object(&mixer)
                .unwrap()
                .inputs
                .iter()
                .filter(|c| c.is_attached())
                .count(),
            6
        );
        assert!(fs.object(&extra).unwrap().is_isolated());
    }

    /// **Methodology.** `DisconnectObject` (DesignSurface.vb:1505-1564) clears
    /// both ends. Disconnecting a pair that is not connected removes nothing and
    /// is not an error; disconnecting an unknown object is.
    /// **Result (2026-08-11):** 1 connection removed, both ends free; a repeat
    /// removes 0; an unknown id returns `UnknownObject`.
    #[test]
    fn disconnect_clears_both_ends() {
        let (mut fs, pump, s_in, _s_out) = pump_and_streams();
        fs.connect(&s_in, &pump, None, None).unwrap();
        assert_eq!(fs.disconnect(&s_in, &pump).unwrap(), 1);
        assert!(fs.object(&pump).unwrap().is_isolated());
        assert!(fs.object(&s_in).unwrap().is_isolated());
        assert_eq!(fs.disconnect(&s_in, &pump).unwrap(), 0);
        assert!(fs.connections().is_empty());

        assert!(matches!(
            fs.disconnect(&s_in, &ObjectId::from("nope")),
            Err(FlowsheetError::UnknownObject(_))
        ));
    }

    /// **Methodology.** `DeleteSelectedObject` (FlowsheetBase.vb:175-290)
    /// disconnects everything the deleted object was attached to and clears any
    /// spec/adjust attachment naming it.
    /// **Result (2026-08-11):** after deleting the pump, both streams are
    /// isolated, the registry holds 2 objects, no connections remain, and the
    /// spec back-pointer on the surviving stream is cleared.
    #[test]
    fn removing_an_object_disconnects_and_detaches_everything() {
        let (mut fs, pump, s_in, s_out) = pump_and_streams();
        fs.connect(&s_in, &pump, None, None).unwrap();
        fs.connect(&pump, &s_out, None, None).unwrap();
        fs.object_mut(&s_out).unwrap().attached_spec = Some(pump.clone());

        let removed = fs.remove_object(&pump).unwrap();
        assert_eq!(removed.object_type, ObjectType::Pump);
        assert_eq!(fs.len(), 2);
        assert!(fs.connections().is_empty());
        assert!(fs.object(&s_in).unwrap().is_isolated());
        assert!(fs.object(&s_out).unwrap().is_isolated());
        assert_eq!(fs.object(&s_out).unwrap().attached_spec, None);
        assert!(matches!(
            fs.remove_object(&pump),
            Err(FlowsheetError::UnknownObject(_))
        ));
    }

    /// **Methodology.** [`Flowsheet::add_connected_streams`] (the portable half
    /// of `AddConnectedObjects`, FlowsheetBase.vb:1839) must create one stream
    /// per free slot, of the kind that slot requires. A pump has 2 inlets
    /// (1 material + 1 energy) and 1 outlet.
    /// **Result (2026-08-11):** 3 streams created — 1 energy, 2 material; the
    /// pump ends fully wired with 3 connections.
    #[test]
    fn add_connected_streams_fills_every_free_slot() {
        let mut fs = Flowsheet::new();
        let pump = fs.add_object(ObjectType::Pump, None);
        let created = fs.add_connected_streams(&pump).unwrap();
        assert_eq!(created.len(), 3);
        let kinds: Vec<ObjectType> = created
            .iter()
            .map(|id| fs.object(id).unwrap().object_type)
            .collect();
        assert_eq!(
            kinds,
            vec![
                ObjectType::MaterialStream,
                ObjectType::EnergyStream,
                ObjectType::MaterialStream
            ]
        );
        let p = fs.object(&pump).unwrap();
        assert!(p.all_inputs_attached());
        assert!(p.outputs.iter().all(|c| c.is_attached()));
        assert_eq!(fs.connections().len(), 3);

        // A second call is a no-op: nothing is free.
        assert!(fs.add_connected_streams(&pump).unwrap().is_empty());
    }

    /// **Methodology.** `UpdateMassAndEnergyBalance` (FlowsheetBase.vb:5412).
    /// Build feed (3 kg/s) -> pump -> product (3 kg/s), give the pump
    /// `power = -5 kW` (consumed). Residual mass = feed - product =
    /// 3 - 3 = 0 kg/s; total energy = -5 kW. Then break the balance by setting
    /// the product to 2 kg/s -> residual = +1 kg/s.
    /// **Result (2026-08-11):** residual 0.000000 kg/s and energy -5.000000 kW;
    /// after the change, residual +1.000000 kg/s. `uom` accessors report
    /// 1.000000 kg/s and -5000.000000 W.
    #[test]
    fn mass_and_energy_balance_sums_feeds_products_and_duties() {
        let (mut fs, pump, s_in, s_out) = pump_and_streams();
        fs.connect(&s_in, &pump, None, None).unwrap();
        fs.connect(&pump, &s_out, None, None).unwrap();
        set_mass_flow(&mut fs, &s_in, 3.0);
        set_mass_flow(&mut fs, &s_out, 3.0);
        if let ObjectData::UnitOperation { power, .. } = &mut fs.object_mut(&pump).unwrap().data {
            *power = Some(-5.0);
        }

        let r = fs.update_mass_and_energy_balance();
        assert_relative_eq!(r.residual_mass_balance, 0.0, epsilon = 1e-12);
        assert_relative_eq!(r.total_energy_balance, -5.0, epsilon = 1e-12);
        assert_eq!(fs.feed_streams().len(), 1);
        assert_eq!(fs.product_streams().len(), 1);

        set_mass_flow(&mut fs, &s_out, 2.0);
        let r = fs.update_mass_and_energy_balance();
        assert_relative_eq!(r.residual_mass_balance, 1.0, epsilon = 1e-12);
        assert_relative_eq!(
            r.residual_mass_balance_rate().get::<kilogram_per_second>(),
            1.0,
            epsilon = 1e-12
        );
        assert_relative_eq!(
            r.total_energy_balance_power().get::<uom::si::power::watt>(),
            -5000.0,
            epsilon = 1e-9
        );
    }

    /// **Methodology.** `GetResultIDs` / `GetResultValue` / `GetResultUnits`
    /// (FlowsheetBase.vb:5295-5410). The eight identifiers must be listed in
    /// upstream order; the two computed results must return their values; the
    /// six uncomputed ones must return `NaN` rather than a fabricated number.
    /// **Result (2026-08-11):** 8 ids; `Residual Mass Balance` = 1.5 kg/s,
    /// `Total Energy Balance` = -2 kW; `Total CAPEX` and an unknown id are NaN.
    #[test]
    fn result_ids_values_and_units() {
        let r = FlowsheetResults {
            residual_mass_balance: 1.5,
            total_energy_balance: -2.0,
        };
        let ids = FlowsheetResults::result_ids();
        assert_eq!(ids.len(), 8);
        assert_eq!(ids[4], "Residual Mass Balance");
        assert_relative_eq!(
            r.result_value("Residual Mass Balance"),
            1.5,
            epsilon = 1e-12
        );
        assert_relative_eq!(
            r.result_value("Total Energy Balance"),
            -2.0,
            epsilon = 1e-12
        );
        assert!(r.result_value("Total CAPEX").is_nan());
        assert!(r.result_value("Not A Result").is_nan());
        assert_eq!(
            FlowsheetResults::result_unit("Residual Mass Balance"),
            "kg/s"
        );
        assert_eq!(FlowsheetResults::result_unit("Total Energy Balance"), "kW");
        assert_eq!(FlowsheetResults::result_unit("Total CAPEX"), "$");
        assert_eq!(FlowsheetResults::result_unit("Not A Result"), "");
    }

    /// **Methodology.** `ResetCalculationStatus` (FlowsheetBase.vb:467) and
    /// `Reset` (:3222). Lookups by tag must find objects; `Reset` must empty
    /// the registry and queue.
    /// **Result (2026-08-11):** after marking calculated/clean, the reset
    /// restores `calculated = false, dirty = true` on every object; `Reset`
    /// leaves the flowsheet empty with an empty queue.
    #[test]
    fn status_reset_and_full_reset() {
        let (mut fs, pump, _s_in, _s_out) = pump_and_streams();
        {
            let o = fs.object_mut(&pump).unwrap();
            o.calculated = true;
            o.dirty = false;
        }
        fs.reset_calculation_status();
        assert!(!fs.object(&pump).unwrap().calculated);
        assert!(fs.object(&pump).unwrap().dirty);

        assert_eq!(fs.object_by_tag("PUMP-1").map(|o| o.id.clone()), Some(pump));
        assert!(fs.object_by_tag("NOPE").is_none());
        assert_eq!(fs.ids_of_type(ObjectType::MaterialStream).len(), 2);
        assert!(fs.objects_with_errors().is_empty());

        fs.reset();
        assert!(fs.is_empty());
        assert_eq!(fs.len(), 0);
        assert!(fs.calculation_queue.is_empty());
        assert!(fs.connections().is_empty());
    }

    /// **Methodology.** Iteration must be deterministic (insertion order),
    /// which reports and solver ordering rely on. `add_object_with_id` must
    /// reject a duplicate identity, as `Dictionary.Add` does
    /// (FlowsheetBase.vb:158).
    /// **Result (2026-08-11):** iteration yields the three insertion-ordered
    /// tags; a duplicate id returns `DuplicateObjectId`.
    #[test]
    fn iteration_is_insertion_ordered_and_ids_are_unique() {
        let (mut fs, _pump, _s_in, _s_out) = pump_and_streams();
        let tags: Vec<&str> = fs.iter().map(|o| o.tag.as_str()).collect();
        assert_eq!(tags, vec!["PUMP-1", "1", "2"]);
        assert_eq!(fs.object_ids().len(), 3);

        let id = ObjectId::from("explicit-guid");
        fs.add_object_with_id(id.clone(), ObjectType::Valve, Some("V-100"))
            .unwrap();
        assert_eq!(fs.object(&id).unwrap().tag, "V-100");
        assert!(matches!(
            fs.add_object_with_id(id, ObjectType::Valve, None),
            Err(FlowsheetError::DuplicateObjectId(_))
        ));
    }
}
