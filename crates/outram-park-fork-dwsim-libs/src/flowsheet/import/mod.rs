//! **Read-only** importer for DWSIM's saved flowsheet files (`.dwxml`,
//! `.dwxmz`), so upstream's reference flowsheets can be used as verification and
//! validation fixtures for this crate's flowsheet layer.
//!
//! # What this is, and what it deliberately is not
//!
//! This is a **narrow reference-case loader for tests**. It reads a DWSIM save
//! file forward, once, and produces a [`Flowsheet`] plus an honest list of
//! everything it could not translate. That is all.
//!
//! It is **not** a port of DWSIM's serialization layer, which
//! [`crate::flowsheet`] explicitly excludes from its scope. In particular there
//! is:
//!
//! - **no writer** — nothing here produces a `.dwxml` or `.dwxmz`;
//! - **no round trip** — an imported flowsheet cannot be saved back, and no
//!   attempt is made to preserve the information a round trip would need;
//! - **no port of `LoadFromXML`** — the reader is original code written against
//!   the *file format*, documented by reading upstream (see "Attribution"), not
//!   a transcription of upstream's loader and its exception handling.
//!
//! The reason to have it at all: upstream ships 175 saved flowsheets under
//! `PlatformFiles/Common/{samples,tests}`, spanning heaters, columns, reactors,
//! recycles and multiphase separators, each with its converged stream states
//! stored in the file. Those are the single best V&V asset available to this
//! port, and hand-transcribing them would be both enormous and error-prone.
//!
//! # Degrading gracefully is the point
//!
//! A reference file will name unit operations and property packages this crate
//! does not implement. The importer never panics and never drops such a thing
//! silently: it imports what maps — objects, graph topology, stream states,
//! compositions, compounds — and records each thing it could not as a
//! structured [`ImportGap`]. The gap list is itself useful coverage data: run
//! the importer over the whole reference corpus and the aggregated gaps are a
//! census of what the port is still missing. See [`ImportedFlowsheet::gaps`]
//! and [`GapCategory`].
//!
//! Only three things are hard errors ([`ImportError`]): the bytes are not
//! readable, the XML is malformed, or the root element is not
//! `DWSIM_Simulation_Data`.
//!
//! # Usage
//!
//! ```no_run
//! use outram_park_fork_dwsim_libs::flowsheet::import::import_flowsheet_file;
//! use std::path::Path;
//!
//! let imported = import_flowsheet_file(Path::new("Cavett's Problem.dwxmz"))?;
//! println!("{} objects, {} connections", imported.flowsheet.len(),
//!          imported.flowsheet.connections().len());
//! for gap in &imported.gaps {
//!     println!("gap: {gap}");
//! }
//! # Ok::<(), outram_park_fork_dwsim_libs::flowsheet::import::ImportError>(())
//! ```
//!
//! # Format assumptions
//!
//! Verified by scanning all 175 reference documents at the pinned upstream
//! commit:
//!
//! - Root element `DWSIM_Simulation_Data`; sections `GeneralInfo`,
//!   `SimulationObjects`, `GraphicObjects`, `Compounds`, `PropertyPackages`
//!   (all optional except that an absent `SimulationObjects` yields an empty
//!   flowsheet).
//! - UTF-8, optionally with a byte-order mark (all 12 plain `.dwxml` files have
//!   one).
//! - No CDATA, comments, DOCTYPE, entity declarations, or namespace resolution
//!   requirements — see [`xml`] for the exact subset that is supported.
//! - A `.dwxmz` is a ZIP archive (all 163 in the corpus are DEFLATE-compressed)
//!   holding one GUID-named `.xml` member — the flowsheet — plus, usually, a
//!   `.db` compound database. Only the `.xml` member is read.
//!
//! # What is deliberately not parsed
//!
//! Everything below is present in the files and is skipped on purpose. Nothing
//! here is a bug, and none of it is reported as a gap:
//!
//! - **All graphics geometry** — `X`, `Y`, `Width`, `Height`, `Rotation`,
//!   `FlippedH`/`FlippedV`, colours, gradients, fonts, `Shape`, `DrawMode`,
//!   `Selected`. `<GraphicObjects>` is read *only* for each object's `Tag` and
//!   its connector wiring, which are topology, not geometry.
//! - **Drawing-only annotations** — `GO_Text`, `GO_Table`, `GO_MasterTable`,
//!   `GO_Chart`, `GO_Rectangle`, `GO_Image`, `GO_SpreadsheetTable` graphic
//!   objects have no simulation object and are counted in
//!   [`ImportSummary::drawing_objects_skipped`], not imported.
//! - **`<Settings>`, `<PanelLayout>`, `<WatchItems>`, `<ChartItems>`,
//!   `<Spreadsheet>`** — GUI and workspace state.
//! - **`<ScriptItems>`** — IronPython/Python.NET scripts, which this crate does
//!   not execute.
//! - **`<Reactions>` / `<ReactionSets>`** — the reactions layer lives in
//!   [`crate::reactions`] and has its own model; wiring it to imported files is
//!   a separate piece of work.
//! - **`<DynamicsManager>`, `<DynamicProperties>`, `<StoredSolutions>`,
//!   `<OptimizationCases>`, `<SensitivityAnalysis>`, `<PetroleumAssays>`.**
//! - **Property-package parameters** — interaction coefficients, calculation
//!   modes, flash-algorithm settings. Only the package's identity and type are
//!   recorded ([`ImportedPropertyPackage`]).
//! - **Compound constants** beyond name, molar mass, CAS number and formula.
//!   DWSIM stores ~200 fields per compound; this crate's composition algebra
//!   needs one, the molar mass. Full compound records are
//!   [`crate::thermo::component::Component`]'s concern.
//! - **`ObjectData::UnitOperation::power`** is left `None`, because DWSIM does
//!   not store it: `GetPowerGeneratedOrConsumed`
//!   (`SimulationObjectBaseClasses.vb:1521`) *computes* it from the connected
//!   streams at report time. A unit operation's stored scalars — a heater's
//!   `DeltaQ` \[kW\], a pump's `DeltaP` \[Pa\] — are kept verbatim and
//!   unconverted in [`ImportedFlowsheet::raw_properties`], where they serve as
//!   the fixture's answer key for a future solver.
//!
//! # Attribution
//!
//! **Original code**, not a port. The file format was documented by reading
//! **DWSIM** (<https://dwsim.org>) at commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0,
//! upstream copyright 2008-2024 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors — specifically:
//!
//! | Upstream file | What it documents |
//! |---|---|
//! | `DWSIM.FlowsheetBase/FlowsheetBase.vb` :4816-4892 | how connections are reconstructed from the connector elements |
//! | `DWSIM.Drawing.SkiaSharp/GraphicObjects/Base/GraphicObject.vb` :193-262 | the `<InputConnectors>`/`<OutputConnectors>`/`<EnergyConnector>` serialisation and its attributes |
//! | `DWSIM.Thermodynamics/MaterialStream/MaterialStream.vb` :138-181 | `SaveData`/`LoadData` — the `<PropertyPackage>` + `<Phases>` payload |
//! | `DWSIM.SharedClasses/BaseClass/SimulationObjectBaseClasses.vb` :1521-1570 | that duty is computed, not stored, and its kJ/kW unit convention |
//! | `DWSIM.Interfaces/Enums.vb` :669-753 and the stream enums | the identifier strings in [`mapping`] |
//!
//! The reference fixtures redistributed under
//! `tests/fixtures/dwsim_reference/` come from the same commit and carry their
//! provenance in the `References.md` beside them. This crate is GPL-3.0-only,
//! matching upstream. Independent OUTRAM PARK fork, **not** the official DWSIM
//! software (see `TRADEMARKS.md`).
//!
//! # Honest scope
//!
//! AI-assisted draft material with **no human V&V**. The tests are
//! *verification* — "does the reader recover what the file says?" — checked
//! against values read out of the XML by hand. No physics is evaluated here and
//! no benchmark has been run. Per the workspace `RESPONSIBLE_USE.md`, treat it
//! as untrusted until reviewed.

pub mod mapping;
pub mod xml;

mod builder;

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::io::Read;
use std::path::Path;

use crate::flowsheet::graph::Flowsheet;
use crate::flowsheet::objects::ObjectId;
use crate::thermo::property_package::PropertyPackageModel;

pub use builder::ROOT_ELEMENT;

/// The magic bytes at the start of every ZIP archive, used to tell a `.dwxmz`
/// from a plain `.dwxml` without trusting the file extension.
const ZIP_MAGIC: [u8; 4] = [b'P', b'K', 0x03, 0x04];

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Why a file could not be imported at all.
///
/// These are the only hard failures. Anything the importer can read but not
/// *translate* becomes an [`ImportGap`] instead, so a fixture using unsupported
/// equipment still imports.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    /// The file could not be read from disk.
    #[error("could not read `{path}`: {message}")]
    Io {
        /// Path that was attempted.
        path: String,
        /// The underlying `std::io::Error`, rendered.
        message: String,
    },
    /// The bytes are not valid UTF-8.
    #[error("`{source_name}` is not valid UTF-8: {message}")]
    Encoding {
        /// What was being decoded (a path, or a ZIP member name).
        source_name: String,
        /// The underlying decoding error, rendered.
        message: String,
    },
    /// The ZIP container could not be opened or a member could not be read.
    #[error("archive error: {0}")]
    Archive(String),
    /// The ZIP container holds no `.xml` member, so there is no flowsheet in it.
    #[error("archive contains no `.xml` member (a `.dwxmz` must hold exactly one)")]
    NoXmlEntry,
    /// The XML is malformed. Carries the reader's message, including the byte
    /// offset at which the problem was found.
    #[error("malformed XML: {0}")]
    Xml(String),
    /// The document parsed, but its root element is not [`ROOT_ELEMENT`] — so
    /// it is some other XML file, not a DWSIM flowsheet.
    #[error("not a DWSIM flowsheet: root element is `<{root}>`, expected `<{expected}>`", expected = ROOT_ELEMENT)]
    NotADwsimDocument {
        /// The root element that was found.
        root: String,
    },
}

// ---------------------------------------------------------------------------
// Gaps
// ---------------------------------------------------------------------------

/// One thing in the source file the importer could read but not translate.
///
/// Gaps are informational, never fatal. Aggregating them over a corpus of
/// reference files answers "what does this port still not cover?" — which is
/// why every variant names the specific object or identifier involved rather
/// than just a count. Group them with [`ImportGap::category`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportGap {
    /// A `<SimulationObject>` whose type has no [`crate::flowsheet::objects::ObjectType`]
    /// in this crate. The object and all its connections are skipped.
    #[error("unsupported object type `{dwsim_type}` (object `{object_name}`)")]
    UnsupportedObjectType {
        /// The `<Type>` string from the file.
        dwsim_type: String,
        /// The object's DWSIM identity (`<Name>`).
        object_name: String,
    },
    /// A `<SimulationObject>` with neither `<Name>` nor `<ComponentName>`, so it
    /// cannot be keyed or connected. Skipped.
    #[error("simulation object of type `{dwsim_type}` has no identity element")]
    UnidentifiedObject {
        /// The `<Type>` string from the file.
        dwsim_type: String,
    },
    /// The flowsheet registry refused the object — in practice a duplicate
    /// identity, which DWSIM's own `Dictionary.Add` would also reject.
    #[error("object `{object_name}` rejected by the flowsheet: {reason}")]
    ObjectRejected {
        /// The object's DWSIM identity.
        object_name: String,
        /// The registry's error, rendered.
        reason: String,
    },
    /// Two objects claimed the same tag, so the second was renamed to keep tags
    /// unique. The topology is unaffected; only the label changed.
    #[error("tag `{original}` was already taken; object relabelled `{assigned}`")]
    TagRenamed {
        /// The tag as written in the file.
        original: String,
        /// The tag the flowsheet assigned instead.
        assigned: String,
    },
    /// A `<PropertyPackage>` this crate does not implement. Stream states are
    /// still imported — nothing here evaluates thermodynamics — but the
    /// flowsheet cannot be re-solved with that package.
    #[error("unsupported property package `{dwsim_type}` (id `{package_id}`)")]
    UnsupportedPropertyPackage {
        /// The package's `<Type>` string.
        dwsim_type: String,
        /// The package's `<ID>`, as material streams reference it.
        package_id: String,
    },
    /// A connection the file records that this crate's connection rules reject
    /// outright, even with slot indices left free. The edge is missing from the
    /// imported flowsheet.
    #[error("connection `{from_tag}` -> `{to_tag}` rejected: {reason}")]
    ConnectionRejected {
        /// Tag of the upstream object.
        from_tag: String,
        /// Tag of the downstream object.
        to_tag: String,
        /// The connection error, rendered.
        reason: String,
    },
    /// A connection whose *exact* slot indices this crate's connector layout
    /// could not honour, but which was made on the first free slots instead.
    ///
    /// The edge exists and the topology is right; only the slot index may
    /// differ from the file. Usually means this crate's
    /// [`crate::flowsheet::connectors::ConnectorLayout`] for that object type
    /// has fewer slots than DWSIM's shape, or is a documented port-side
    /// approximation.
    #[error("connection `{from_tag}` -> `{to_tag}` made on free slots instead of ({requested_from_slot:?}, {requested_to_slot:?}): {reason}")]
    ConnectionSlotFallback {
        /// Tag of the upstream object.
        from_tag: String,
        /// Tag of the downstream object.
        to_tag: String,
        /// Outlet slot index the file asked for, if any.
        requested_from_slot: Option<usize>,
        /// Inlet slot index the file asked for, if any.
        requested_to_slot: Option<usize>,
        /// Why the exact-slot attempt failed, rendered.
        reason: String,
    },
    /// A connection pointing at an object that was not imported (because its
    /// type was unsupported, or it is a drawing annotation).
    #[error("connection from `{from_object}` points at `{to_object}`, which was not imported")]
    DanglingConnection {
        /// Identity of the source object.
        from_object: String,
        /// Identity of the missing destination.
        to_object: String,
    },
    /// A field was present but its value was not one this crate recognises. The
    /// field keeps its default; the raw value is recorded here.
    #[error("object `{object_tag}`: field `{field}` has unrecognised value `{value}`")]
    UnparsedField {
        /// Tag of the object the field belongs to.
        object_tag: String,
        /// The element name.
        field: String,
        /// The verbatim text.
        value: String,
    },
    /// A compound on a stream is not in the file's `<Compounds>` section and its
    /// molar mass could not be derived from its stored flows either. It is
    /// still added to the stream, with molar mass `0`, so composition indices
    /// stay aligned — but any molar-mass-weighted quantity computed from that
    /// stream will be wrong.
    #[error("stream `{object_tag}`: no molar mass available for compound `{compound}`")]
    MissingCompoundMolarMass {
        /// Tag of the stream.
        object_tag: String,
        /// The compound's name.
        compound: String,
    },
}

/// Coarse grouping of [`ImportGap`] variants, for aggregating a corpus-wide
/// census.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GapCategory {
    /// An object type this crate does not model.
    UnsupportedObjectType,
    /// A property package this crate does not implement.
    UnsupportedPropertyPackage,
    /// An object that could not be registered at all.
    ObjectProblem,
    /// A connection that is missing or was made on a different slot.
    Connection,
    /// A field value that was not understood.
    UnparsedField,
    /// Missing compound data.
    CompoundData,
}

impl fmt::Display for GapCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            GapCategory::UnsupportedObjectType => "unsupported object type",
            GapCategory::UnsupportedPropertyPackage => "unsupported property package",
            GapCategory::ObjectProblem => "object problem",
            GapCategory::Connection => "connection",
            GapCategory::UnparsedField => "unparsed field",
            GapCategory::CompoundData => "compound data",
        };
        f.write_str(s)
    }
}

impl ImportGap {
    /// Which [`GapCategory`] this gap belongs to.
    #[must_use]
    pub fn category(&self) -> GapCategory {
        match self {
            ImportGap::UnsupportedObjectType { .. } => GapCategory::UnsupportedObjectType,
            ImportGap::UnsupportedPropertyPackage { .. } => GapCategory::UnsupportedPropertyPackage,
            ImportGap::UnidentifiedObject { .. }
            | ImportGap::ObjectRejected { .. }
            | ImportGap::TagRenamed { .. } => GapCategory::ObjectProblem,
            ImportGap::ConnectionRejected { .. }
            | ImportGap::ConnectionSlotFallback { .. }
            | ImportGap::DanglingConnection { .. } => GapCategory::Connection,
            ImportGap::UnparsedField { .. } => GapCategory::UnparsedField,
            ImportGap::MissingCompoundMolarMass { .. } => GapCategory::CompoundData,
        }
    }
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Where an imported flowsheet came from.
///
/// An enum rather than a boolean so a diagnostic can name the ZIP member the
/// flowsheet was read out of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKind {
    /// A plain `.dwxml` document.
    PlainXml,
    /// The named `.xml` member of a `.dwxmz` ZIP archive.
    ZippedXml {
        /// The member's name inside the archive (a GUID plus `.xml`).
        entry: String,
    },
}

/// One entry of the file's `<Compounds>` section.
///
/// Only the four fields this crate has a use for are kept; DWSIM stores about
/// two hundred per compound (see the module's "what is deliberately not
/// parsed").
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedCompound {
    /// Compound name, as material streams reference it.
    pub name: String,
    /// Molar mass \[kg/kmol\] — DWSIM's `<Molar_Weight>`. `NaN` if the file did
    /// not give one.
    pub molar_mass: f64,
    /// CAS registry number, or empty.
    pub cas_number: String,
    /// Chemical formula as DWSIM writes it (e.g. `HOH`), or empty.
    pub formula: String,
}

/// One entry of the file's `<PropertyPackages>` section.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedPropertyPackage {
    /// The package's `<ID>` (e.g. `PP-5dc0fb93-...`), which material streams
    /// reference through their `<PropertyPackage>` element.
    pub id: String,
    /// The user-visible package tag, e.g. `Raoult's Law (1)`.
    pub tag: String,
    /// The package's .NET class name, verbatim.
    pub dwsim_type: String,
    /// This crate's equivalent model, or `None` if it implements no equivalent
    /// (in which case an [`ImportGap::UnsupportedPropertyPackage`] was also
    /// recorded).
    pub model: Option<PropertyPackageModel>,
}

/// Counts describing what one import produced. Cheap to aggregate across a
/// whole corpus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImportSummary {
    /// Flowsheet objects created.
    pub objects: usize,
    /// Of those, material streams.
    pub material_streams: usize,
    /// Of those, energy streams.
    pub energy_streams: usize,
    /// Of those, everything else (unit operations, logical blocks, indicators).
    pub unit_operations: usize,
    /// Edges wired into the flowsheet.
    pub connections: usize,
    /// Compounds in the file's `<Compounds>` section.
    pub compounds: usize,
    /// Property packages in the file.
    pub property_packages: usize,
    /// Gaps recorded.
    pub gaps: usize,
    /// Graphic objects with no imported simulation object — the drawing-only
    /// annotations (`GO_Text`, `GO_Table`, ...) plus the graphics of any object
    /// that was skipped.
    pub drawing_objects_skipped: usize,
}

/// A DWSIM reference file, read into this crate's model.
///
/// Owns everything (no lifetimes, no `Box`, no trait objects), so it can be
/// moved out of the importer and kept for the length of a test.
#[derive(Debug, Clone)]
pub struct ImportedFlowsheet {
    /// The flowsheet itself: objects, tags, connector topology and stream
    /// states.
    pub flowsheet: Flowsheet,
    /// Everything the importer could read but not translate, in the order it was
    /// found. Empty means the file mapped completely.
    pub gaps: Vec<ImportGap>,
    /// Whether the source was a plain document or a ZIP member.
    pub source: SourceKind,
    /// The file's `<GeneralInfo>` block verbatim (`BuildVersion`, `SavedOn`,
    /// ...), for provenance.
    pub general_info: BTreeMap<String, String>,
    /// The flowsheet's compound list, in file order.
    pub compounds: Vec<ImportedCompound>,
    /// The flowsheet's property packages, in file order.
    pub property_packages: Vec<ImportedPropertyPackage>,
    /// Material stream identity -> the `<PropertyPackage>` id it references.
    stream_packages: HashMap<ObjectId, String>,
    /// Object identity -> its scalar `<SimulationObject>` children, verbatim.
    raw_properties: HashMap<ObjectId, BTreeMap<String, String>>,
    /// See [`ImportSummary::drawing_objects_skipped`].
    drawing_objects_skipped: usize,
}

impl ImportedFlowsheet {
    /// Counts describing this import.
    #[must_use]
    pub fn summary(&self) -> ImportSummary {
        let mut s = ImportSummary {
            objects: self.flowsheet.len(),
            connections: self.flowsheet.connections().len(),
            compounds: self.compounds.len(),
            property_packages: self.property_packages.len(),
            gaps: self.gaps.len(),
            drawing_objects_skipped: self.drawing_objects_skipped,
            ..ImportSummary::default()
        };
        for object in self.flowsheet.iter() {
            if object.object_type.is_material_stream() {
                s.material_streams += 1;
            } else if object.object_type.is_energy_stream() {
                s.energy_streams += 1;
            } else {
                s.unit_operations += 1;
            }
        }
        s
    }

    /// How many gaps fall in each [`GapCategory`].
    #[must_use]
    pub fn gap_counts(&self) -> BTreeMap<GapCategory, usize> {
        let mut counts = BTreeMap::new();
        for gap in &self.gaps {
            *counts.entry(gap.category()).or_insert(0) += 1;
        }
        counts
    }

    /// The scalar properties DWSIM stored on one object, verbatim and
    /// **unconverted** — element name -> text.
    ///
    /// These are the file's *reference answers*: a heater's `DeltaQ` \[kW\], a
    /// valve's `DeltaP` \[Pa\], a column's `NumberOfStages`. They are kept as
    /// text in DWSIM's internal units precisely because they are the numbers a
    /// future solver must reproduce; converting them here would invite a silent
    /// unit error in the one place that must not have one. Parse the ones a
    /// given test needs, and state the unit in that test.
    ///
    /// Returns `None` if `id` names no imported object.
    #[must_use]
    pub fn raw_properties(&self, id: &ObjectId) -> Option<&BTreeMap<String, String>> {
        self.raw_properties.get(id)
    }

    /// The `<PropertyPackage>` id a material stream references, or `None` for a
    /// stream that names none (and for every non-stream object).
    #[must_use]
    pub fn stream_property_package(&self, id: &ObjectId) -> Option<&str> {
        self.stream_packages.get(id).map(String::as_str)
    }

    /// Look up a property package by its `<ID>`.
    #[must_use]
    pub fn property_package(&self, id: &str) -> Option<&ImportedPropertyPackage> {
        self.property_packages.iter().find(|p| p.id == id)
    }

    /// Molar mass \[kg/kmol\] of a compound named in the file's `<Compounds>`
    /// section, or `None`.
    #[must_use]
    pub fn compound_molar_mass(&self, name: &str) -> Option<f64> {
        self.compounds
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.molar_mass)
    }
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// Import a DWSIM flowsheet from a file on disk.
///
/// The container is detected from the **bytes**, not the extension: a file
/// starting with the ZIP magic `PK\x03\x04` is treated as a `.dwxmz`, anything
/// else as a plain `.dwxml`. That makes a renamed or extension-less fixture
/// import correctly.
///
/// # Errors
/// [`ImportError`] — see its variants; the file not existing, not being UTF-8,
/// not being well-formed XML, or not being a DWSIM document.
pub fn import_flowsheet_file(path: &Path) -> Result<ImportedFlowsheet, ImportError> {
    let bytes = std::fs::read(path).map_err(|e| ImportError::Io {
        path: path.display().to_string(),
        message: e.to_string(),
    })?;
    import_flowsheet_bytes(&bytes, &path.display().to_string())
}

/// Import from bytes already in memory, detecting the container as
/// [`import_flowsheet_file`] does.
///
/// `source_name` is used only in error messages.
///
/// # Errors
/// [`ImportError`].
pub fn import_flowsheet_bytes(
    bytes: &[u8],
    source_name: &str,
) -> Result<ImportedFlowsheet, ImportError> {
    if bytes.starts_with(&ZIP_MAGIC) {
        return import_flowsheet_dwxmz(bytes);
    }
    let text = std::str::from_utf8(bytes).map_err(|e| ImportError::Encoding {
        source_name: source_name.to_string(),
        message: e.to_string(),
    })?;
    import_flowsheet_dwxml(text)
}

/// Import from the text of a plain `.dwxml` document.
///
/// A leading byte-order mark is tolerated.
///
/// # Errors
/// [`ImportError::Xml`] if the document is malformed,
/// [`ImportError::NotADwsimDocument`] if its root element is not
/// [`ROOT_ELEMENT`].
pub fn import_flowsheet_dwxml(text: &str) -> Result<ImportedFlowsheet, ImportError> {
    let root = xml::parse_document(text).map_err(|e| ImportError::Xml(e.to_string()))?;
    builder::build(&root, SourceKind::PlainXml)
}

/// Import from the bytes of a `.dwxmz` ZIP archive.
///
/// The **first** member whose name ends in `.xml` (case-insensitively) is the
/// flowsheet; every reference archive in the corpus holds exactly one. The
/// companion `.db` compound database is ignored — this crate reads compound
/// data out of the flowsheet's own `<Compounds>` section.
///
/// # Errors
/// [`ImportError::Archive`] if the container is unreadable,
/// [`ImportError::NoXmlEntry`] if it holds no XML member, plus the errors of
/// [`import_flowsheet_dwxml`].
pub fn import_flowsheet_dwxmz(archive: &[u8]) -> Result<ImportedFlowsheet, ImportError> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive))
        .map_err(|e| ImportError::Archive(e.to_string()))?;

    let mut found: Option<(usize, String)> = None;
    for index in 0..zip.len() {
        let entry = zip
            .by_index(index)
            .map_err(|e| ImportError::Archive(e.to_string()))?;
        let name = entry.name().to_string();
        if name.to_ascii_lowercase().ends_with(".xml") {
            found = Some((index, name));
            break;
        }
    }
    let Some((index, name)) = found else {
        return Err(ImportError::NoXmlEntry);
    };

    let mut entry = zip
        .by_index(index)
        .map_err(|e| ImportError::Archive(e.to_string()))?;
    let mut raw = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut raw)
        .map_err(|e| ImportError::Archive(e.to_string()))?;
    let text = String::from_utf8(raw).map_err(|e| ImportError::Encoding {
        source_name: name.clone(),
        message: e.to_string(),
    })?;

    let root = xml::parse_document(&text).map_err(|e| ImportError::Xml(e.to_string()))?;
    builder::build(&root, SourceKind::ZippedXml { entry: name })
}

#[cfg(test)]
mod tests {
    //! # Verification — the importer against hand-written miniature documents
    //!
    //! **Methodology.** Each test states a small `DWSIM_Simulation_Data`
    //! document whose expected import is obvious by inspection, imports it, and
    //! checks the objects, the topology, the stream state and the gap list.
    //! These are unit tests of the translation rules; the *integration* tests
    //! against real upstream reference files live in
    //! `tests/flowsheet_import.rs`. Verification only — no thermodynamics is
    //! evaluated.
    //! **Results (2026-08-11, release build):** all five tests pass.

    use super::*;
    use crate::flowsheet::objects::ObjectType;

    /// A feed stream -> heater -> product stream with an energy stream on the
    /// heater's duty inlet, in DWSIM's element shapes but stripped of
    /// everything the importer ignores.
    fn miniature_flowsheet() -> String {
        r#"<?xml version="1.0" encoding="utf-8"?>
<DWSIM_Simulation_Data>
  <GeneralInfo><BuildVersion>8.0.0.0</BuildVersion></GeneralInfo>
  <SimulationObjects>
    <SimulationObject>
      <Type>DWSIM.Thermodynamics.Streams.MaterialStream</Type>
      <Name>MAT-feed</Name>
      <ComponentDescription>Feed</ComponentDescription>
      <SpecType>Temperature_and_Pressure</SpecType>
      <DefinedFlow>Mass</DefinedFlow>
      <Calculated>true</Calculated>
      <PropertyPackage>PP-1</PropertyPackage>
      <Phases>
        <Phase>
          <ID>0</ID>
          <Compounds>
            <Compound>
              <Name>Water</Name><MoleFraction>0.25</MoleFraction><MassFraction>0.0588</MassFraction>
              <MassFlow>0.0588</MassFlow><MolarFlow>3.264</MolarFlow>
            </Compound>
            <Compound>
              <Name>Methane</Name><MoleFraction>0.75</MoleFraction><MassFraction>0.9412</MassFraction>
              <MassFlow>0.9412</MassFlow><MolarFlow>9.792</MolarFlow>
            </Compound>
          </Compounds>
          <Properties>
            <temperature>350.5</temperature><pressure>202650</pressure>
            <massflow>1</massflow><molarflow>13.056</molarflow>
            <enthalpy>123.5</enthalpy><entropy>0.75</entropy>
            <molecularWeight>76.6</molecularWeight><surfaceTension>NaN</surfaceTension>
          </Properties>
        </Phase>
        <Phase><ID>2</ID><Properties><molarfraction>1</molarfraction></Properties></Phase>
      </Phases>
    </SimulationObject>
    <SimulationObject>
      <Type>DWSIM.UnitOperations.UnitOperations.Heater</Type>
      <Name>HT-1</Name>
      <DeltaQ>1523.39</DeltaQ>
      <CalcMode>OutletTemperature</CalcMode>
    </SimulationObject>
    <SimulationObject>
      <Type>DWSIM.UnitOperations.Streams.EnergyStream</Type>
      <Name>EN-1</Name>
      <EnergyFlow>1523.39</EnergyFlow>
    </SimulationObject>
    <SimulationObject>
      <Type>DWSIM.Thermodynamics.Streams.MaterialStream</Type>
      <Name>MAT-prod</Name>
      <SpecType>Pressure_and_Enthalpy</SpecType>
      <Phases><Phase><ID>0</ID><Properties><temperature>700</temperature><pressure>202650</pressure></Properties></Phase></Phases>
    </SimulationObject>
  </SimulationObjects>
  <GraphicObjects>
    <GraphicObject>
      <ObjectType>MaterialStream</ObjectType><Name>MAT-feed</Name><Tag>FEED</Tag>
      <InputConnectors><Connector IsAttached="false" /></InputConnectors>
      <OutputConnectors><Connector IsAttached="true" ConnType="ConOut" AttachedToObjID="HT-1" AttachedToConnIndex="0" AttachedToEnergyConn="False" /></OutputConnectors>
      <EnergyConnector><Connector IsAttached="false" /></EnergyConnector>
    </GraphicObject>
    <GraphicObject>
      <ObjectType>Heater</ObjectType><Name>HT-1</Name><Tag>HEATER-1</Tag>
      <InputConnectors><Connector IsAttached="true" ConnType="ConIn" AttachedFromObjID="MAT-feed" AttachedFromConnIndex="0" AttachedFromEnergyConn="False" /><Connector IsAttached="true" ConnType="ConEn" AttachedFromObjID="EN-1" AttachedFromConnIndex="0" AttachedFromEnergyConn="False" /></InputConnectors>
      <OutputConnectors><Connector IsAttached="true" ConnType="ConOut" AttachedToObjID="MAT-prod" AttachedToConnIndex="0" AttachedToEnergyConn="False" /></OutputConnectors>
      <EnergyConnector><Connector IsAttached="false" /></EnergyConnector>
    </GraphicObject>
    <GraphicObject>
      <ObjectType>EnergyStream</ObjectType><Name>EN-1</Name><Tag>DUTY</Tag>
      <InputConnectors><Connector IsAttached="false" /></InputConnectors>
      <OutputConnectors><Connector IsAttached="true" ConnType="ConEn" AttachedToObjID="HT-1" AttachedToConnIndex="1" AttachedToEnergyConn="False" /></OutputConnectors>
      <EnergyConnector><Connector IsAttached="false" /></EnergyConnector>
    </GraphicObject>
    <GraphicObject>
      <ObjectType>MaterialStream</ObjectType><Name>MAT-prod</Name><Tag>PROD</Tag>
      <InputConnectors><Connector IsAttached="true" ConnType="ConIn" AttachedFromObjID="HT-1" AttachedFromConnIndex="0" AttachedFromEnergyConn="False" /></InputConnectors>
      <OutputConnectors><Connector IsAttached="false" /></OutputConnectors>
      <EnergyConnector><Connector IsAttached="false" /></EnergyConnector>
    </GraphicObject>
    <GraphicObject>
      <ObjectType>GO_Text</ObjectType><Name>TEXT-1</Name><Tag>note</Tag>
    </GraphicObject>
  </GraphicObjects>
  <PropertyPackages>
    <PropertyPackage><ID>PP-1</ID><Type>DWSIM.Thermodynamics.PropertyPackages.PengRobinsonPropertyPackage</Type><Tag>Peng-Robinson (1)</Tag></PropertyPackage>
  </PropertyPackages>
  <Compounds>
    <Compound><Name>Water</Name><Molar_Weight>18.01528</Molar_Weight><CAS_Number>7732-18-5</CAS_Number><Formula>HOH</Formula></Compound>
    <Compound><Name>Methane</Name><Molar_Weight>16.04246</Molar_Weight><CAS_Number>74-82-8</CAS_Number><Formula>CH4</Formula></Compound>
  </Compounds>
</DWSIM_Simulation_Data>"#
            .to_string()
    }

    /// **Methodology.** Import the miniature flowsheet and check the object
    /// count, the tags, the types, and that the one drawing annotation was
    /// skipped rather than imported.
    /// **Result (2026-08-11):** 4 objects (2 material streams, 1 energy stream,
    /// 1 heater); tags FEED/HEATER-1/DUTY/PROD; 1 drawing object skipped; no
    /// gaps.
    #[test]
    fn imports_objects_tags_and_types() {
        let imported = import_flowsheet_dwxml(&miniature_flowsheet()).unwrap();
        assert_eq!(imported.gaps, vec![], "miniature flowsheet must map fully");
        let s = imported.summary();
        assert_eq!(s.objects, 4);
        assert_eq!(s.material_streams, 2);
        assert_eq!(s.energy_streams, 1);
        assert_eq!(s.unit_operations, 1);
        assert_eq!(s.drawing_objects_skipped, 1);
        assert_eq!(s.compounds, 2);

        for (tag, ty) in [
            ("FEED", ObjectType::MaterialStream),
            ("HEATER-1", ObjectType::Heater),
            ("DUTY", ObjectType::EnergyStream),
            ("PROD", ObjectType::MaterialStream),
        ] {
            let object = imported
                .flowsheet
                .object_by_tag(tag)
                .unwrap_or_else(|| panic!("missing object tagged {tag}"));
            assert_eq!(object.object_type, ty);
        }
        // DWSIM identities are preserved verbatim.
        assert!(imported
            .flowsheet
            .contains(&ObjectId("MAT-feed".to_string())));
    }

    /// **Methodology.** Check the three edges the file records, including that
    /// the energy stream lands on the heater's `ConEn` slot (`Input(1)`), and
    /// that every endpoint resolves to a real object.
    /// **Result (2026-08-11):** 3 connections; `DUTY -> HEATER-1` on
    /// `Input(1)`; all endpoints resolve.
    #[test]
    fn imports_topology_including_the_energy_slot() {
        let imported = import_flowsheet_dwxml(&miniature_flowsheet()).unwrap();
        let connections = imported.flowsheet.connections();
        assert_eq!(connections.len(), 3);
        for c in &connections {
            assert!(imported.flowsheet.object(&c.from).is_some());
            assert!(imported.flowsheet.object(&c.to).is_some());
        }
        let duty = imported.flowsheet.id_by_tag("DUTY").unwrap().clone();
        let heater = imported.flowsheet.id_by_tag("HEATER-1").unwrap().clone();
        let edge = connections
            .iter()
            .find(|c| c.from == duty && c.to == heater)
            .expect("the energy stream must be wired to the heater");
        assert_eq!(
            edge.to_slot,
            crate::flowsheet::connectors::ConnectorSlot::Input(1),
            "an energy stream takes the heater's ConEn slot"
        );
    }

    /// **Methodology.** Read the feed stream's state back through the
    /// `uom`-typed accessors and compare with the values written in the XML:
    /// `T = 350.5 K`, `P = 202650 Pa`, `w = 1 kg/s`, `h = 123.5 kJ/kg`
    /// (= 123 500 J/kg), `s = 0.75 kJ/(kg·K)` (= 750 J/(kg·K)), composition
    /// `x = (0.25, 0.75)`, molar masses from `<Compounds>`. Also check that
    /// `surfaceTension = NaN` became `None`, and that the energy stream's
    /// 1523.39 kW reads back as 1 523 390 W.
    /// **Result (2026-08-11):** every value matches to 1e-9 relative; `NaN`
    /// stored as `None`; energy stream = 1 523 390 W.
    #[test]
    fn imports_stream_state_in_si_through_uom() {
        use uom::si::available_energy::joule_per_kilogram;
        use uom::si::mass_rate::kilogram_per_second;
        use uom::si::power::watt;
        use uom::si::pressure::pascal;
        use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
        use uom::si::thermodynamic_temperature::kelvin;

        let imported = import_flowsheet_dwxml(&miniature_flowsheet()).unwrap();
        let feed = imported
            .flowsheet
            .object_by_tag("FEED")
            .unwrap()
            .data
            .as_material()
            .unwrap();

        assert!((feed.temperature().unwrap().get::<kelvin>() - 350.5).abs() < 1e-9);
        assert!((feed.pressure().unwrap().get::<pascal>() - 202_650.0).abs() < 1e-6);
        assert!((feed.mass_flow().unwrap().get::<kilogram_per_second>() - 1.0).abs() < 1e-12);
        assert!(
            (feed.mass_enthalpy().unwrap().get::<joule_per_kilogram>() - 123_500.0).abs() < 1e-6,
            "DWSIM stores kJ/kg; the uom accessor must report J/kg"
        );
        assert!(
            (feed
                .mass_entropy()
                .unwrap()
                .get::<joule_per_kilogram_kelvin>()
                - 750.0)
                .abs()
                < 1e-9
        );
        assert_eq!(
            feed.spec,
            crate::flowsheet::streams::StreamSpec::TemperatureAndPressure
        );
        assert_eq!(feed.compound_names(), vec!["Water", "Methane"]);
        assert_eq!(feed.overall_composition(), vec![0.25, 0.75]);
        assert!(
            (feed
                .phase(crate::flowsheet::streams::PhaseIndex::Mixture)
                .compounds[0]
                .molar_mass
                - 18.01528)
                .abs()
                < 1e-9
        );
        assert_eq!(
            feed.phase(crate::flowsheet::streams::PhaseIndex::Mixture)
                .properties
                .surface_tension,
            None,
            "DWSIM's NaN means 'not calculated' and must become None"
        );
        assert_eq!(feed.vapor_fraction().map(|b| b.value), Some(1.0));

        let duty = imported
            .flowsheet
            .object_by_tag("DUTY")
            .unwrap()
            .data
            .as_energy()
            .unwrap();
        assert!(
            (duty.power().unwrap().get::<watt>() - 1_523_390.0).abs() < 1e-3,
            "DWSIM stores kW; the uom accessor must report W"
        );
    }

    /// **Methodology.** The heater's stored scalars must survive verbatim in
    /// [`ImportedFlowsheet::raw_properties`] — that is the fixture's answer key
    /// — while `ObjectData::UnitOperation::power` stays `None` because DWSIM
    /// computes duty rather than storing it. Also check the property-package
    /// record and the stream's reference to it.
    /// **Result (2026-08-11):** `DeltaQ = "1523.39"`, `CalcMode =
    /// "OutletTemperature"`; `power` is `None`; `PP-1` maps to
    /// [`PropertyPackageModel::PengRobinson`].
    #[test]
    fn keeps_unit_operation_scalars_verbatim_and_leaves_power_unset() {
        let imported = import_flowsheet_dwxml(&miniature_flowsheet()).unwrap();
        let heater_id = imported.flowsheet.id_by_tag("HEATER-1").unwrap().clone();
        let props = imported.raw_properties(&heater_id).unwrap();
        assert_eq!(props.get("DeltaQ").map(String::as_str), Some("1523.39"));
        assert_eq!(
            props.get("CalcMode").map(String::as_str),
            Some("OutletTemperature")
        );
        match &imported.flowsheet.object(&heater_id).unwrap().data {
            crate::flowsheet::objects::ObjectData::UnitOperation { power, .. } => {
                assert_eq!(*power, None, "DWSIM computes duty; it is not in the file");
            }
            other => panic!("heater should be a unit operation, got {other:?}"),
        }

        let feed_id = imported.flowsheet.id_by_tag("FEED").unwrap().clone();
        assert_eq!(imported.stream_property_package(&feed_id), Some("PP-1"));
        assert_eq!(
            imported.property_package("PP-1").unwrap().model,
            Some(PropertyPackageModel::PengRobinson)
        );
    }

    /// **Methodology.** Four documents that must degrade rather than blow up:
    /// an unknown object type (gap, not error), an unsupported property package
    /// (gap), a truncated document (error), and a well-formed XML file that is
    /// not a DWSIM flowsheet (error).
    /// **Result (2026-08-11):** one `UnsupportedObjectType` gap naming the
    /// type; one `UnsupportedPropertyPackage` gap; `ImportError::Xml`;
    /// `ImportError::NotADwsimDocument`.
    #[test]
    fn degrades_gracefully_and_reports_hard_failures() {
        let unknown = r#"<DWSIM_Simulation_Data><SimulationObjects>
            <SimulationObject><Type>DWSIM.UnitOperations.UnitOperations.Teleporter</Type><Name>X-1</Name></SimulationObject>
            </SimulationObjects></DWSIM_Simulation_Data>"#;
        let imported = import_flowsheet_dwxml(unknown).unwrap();
        assert_eq!(imported.flowsheet.len(), 0);
        assert_eq!(
            imported.gaps,
            vec![ImportGap::UnsupportedObjectType {
                dwsim_type: "DWSIM.UnitOperations.UnitOperations.Teleporter".to_string(),
                object_name: "X-1".to_string(),
            }]
        );
        assert_eq!(
            imported
                .gap_counts()
                .get(&GapCategory::UnsupportedObjectType),
            Some(&1)
        );

        let pkg = r#"<DWSIM_Simulation_Data><PropertyPackages>
            <PropertyPackage><ID>PP-9</ID><Type>DWSIM.Thermodynamics.PropertyPackages.NRTLPropertyPackage</Type></PropertyPackage>
            </PropertyPackages></DWSIM_Simulation_Data>"#;
        let imported = import_flowsheet_dwxml(pkg).unwrap();
        assert_eq!(imported.property_packages[0].model, None);
        assert_eq!(
            imported.gaps,
            vec![ImportGap::UnsupportedPropertyPackage {
                dwsim_type: "DWSIM.Thermodynamics.PropertyPackages.NRTLPropertyPackage".to_string(),
                package_id: "PP-9".to_string(),
            }]
        );

        assert!(matches!(
            import_flowsheet_dwxml("<DWSIM_Simulation_Data><SimulationObjects>"),
            Err(ImportError::Xml(_))
        ));
        assert!(matches!(
            import_flowsheet_dwxml("<SomeOtherRoot />"),
            Err(ImportError::NotADwsimDocument { .. })
        ));
    }
}
