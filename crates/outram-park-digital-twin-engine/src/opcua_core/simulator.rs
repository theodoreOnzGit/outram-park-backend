//! The seam: what a simulator supplies, and what the shared layer owns.
//!
//! [`opcua_core`](super) serves *any* OUTRAM PARK digital twin over OPC-UA. To
//! be served, a simulator supplies exactly two things:
//!
//! | It supplies | Trait | Meaning |
//! |---|---|---|
//! | **who it is** | [`OpcuaSimulator`] | one [`OpcuaSimulatorProfile`] of naming/identity strings (namespace URI, endpoint path, mDNS marker, ...) |
//! | **what it publishes** | [`OpcuaVariable`] | one `Copy` enum whose variants are its variables, plus the snapshot and request types they read and write |
//!
//! Everything else — the TCP transport, the tokio runtime and its thread, the
//! PKI directory, mDNS announcement, address-space construction, the read and
//! write callbacks and the subscription push — belongs to the shared layer and
//! is written once.
//!
//! ## Compile-time dispatch, no trait objects
//!
//! Both traits are used as **generic bounds**, never as `Box<dyn Trait>` or
//! `&dyn Trait` (workspace `CLAUDE.md`, Rust design rules). The simulator's
//! variable type is a plain enum, so adding a variable is a compile error at
//! every `match` arm rather than a runtime surprise, and the whole address
//! space is monomorphised with no dynamic dispatch and no heap indirection.
//!
//! ## Physical quantities and units
//!
//! Nothing in this module is a physical quantity. Node identifiers, browse
//! names and folder names are OPC-UA naming strings; the *values* a simulator
//! publishes carry their engineering unit in the variable's
//! [`description`](OpcuaVariable::description) text, which is what an OPC-UA
//! client displays. Unit correctness therefore lives in the simulator's own
//! node map, not here.
//!
//! ## Scope (`RESPONSIBLE_USE.md`)
//!
//! OPC-UA is a plant-connectivity protocol, so the boundary matters: this layer
//! exists so **offline educational simulators** can be driven by standard
//! OPC-UA tooling on a bench or in a classroom. Nothing built on it may be
//! connected to live operational systems, plant systems, safety-critical
//! infrastructure, real-time plant monitoring, or institutional production
//! systems, and its values are not authoritative for any operational, licensing
//! or safety purpose.

use opcua::nodes::AccessLevel;
use opcua::types::{DataTypeId, DataValue, StatusCode, Variant};

/// The naming and identity strings that make one simulator's OPC-UA interface
/// distinguishable from another's.
///
/// Every field is a *name*, not a physical quantity, so none carries a unit.
/// A simulator declares one of these as a `const` and returns it from
/// [`OpcuaSimulator::PROFILE`]; the shared layer reads it wherever it would
/// otherwise have hard-coded a string.
///
/// ## The one rule that will bite you
///
/// **[`application_uri`](Self::application_uri) must never equal
/// [`namespace_uri`](Self::namespace_uri).** `async-opcua`'s diagnostics node
/// manager registers the application URI as *its own* namespace and claims
/// every node at that index (`owns_node` is `id.namespace ==
/// self.namespace_index`). Identical strings resolve to one index, so the
/// diagnostics manager would shadow the simulator's whole namespace and every
/// read would return `BadNodeIdUnknown` despite the nodes being present and
/// browsable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcuaSimulatorProfile {
    /// Namespace URI every one of this simulator's variables lives in, e.g.
    /// `"urn:outram-park:ciet-educational-simulator-v2"`.
    ///
    /// A client resolves this to a running namespace *index* (usually 2) from
    /// the server's namespace array; the index is not stable across versions
    /// and must not be hard-coded by a client.
    pub namespace_uri: &'static str,

    /// Base OPC-UA `ApplicationUri` / `ProductUri`. The TCP port is appended to
    /// the application URI per instance, so two simulators on one machine do
    /// not present the same application identity.
    ///
    /// Must differ from [`namespace_uri`](Self::namespace_uri) — see the type
    /// documentation.
    pub application_uri: &'static str,

    /// Endpoint path appended to the server URL, e.g. `"/ciet"`, giving
    /// `opc.tcp://<host>:<port>/ciet`. Announced in the mDNS `path` TXT record
    /// so a discovering client can rebuild the full URL.
    pub endpoint_path: &'static str,

    /// Human-facing `ApplicationName` used when the caller does not override
    /// it. Shown by clients in their server list and in the endpoint
    /// description.
    pub default_application_name: &'static str,

    /// TCP port used when the caller does not override it. 4840 is the
    /// IANA-registered `opcua-tcp` port and is where OPC-UA tooling looks
    /// first; a second simulator on the same machine needs a different one.
    pub default_port: u16,

    /// Name given to this simulator's node manager inside `async-opcua`.
    /// Diagnostic only — it appears in `async-opcua`'s own logging.
    pub node_manager_name: &'static str,

    /// Browse name of the folder holding the read-only outputs, e.g.
    /// `"Outputs"`.
    pub outputs_folder_name: &'static str,

    /// String node identifier of the outputs folder, e.g. `"CIET.Outputs"`.
    /// Must not collide with any variable's node identifier.
    pub outputs_folder_node_id: &'static str,

    /// Browse name of the folder holding the writable controls, e.g.
    /// `"Controls"`.
    pub controls_folder_name: &'static str,

    /// String node identifier of the controls folder, e.g.
    /// `"CIET.Controls"`.
    pub controls_folder_node_id: &'static str,

    /// Directory name of this simulator's PKI store, relative to the OUTRAM
    /// PARK per-user root — see [`super::pki`]. Holds a throwaway self-signed
    /// keypair and nothing sensitive.
    pub pki_dir_name: &'static str,

    /// Prefix of the DNS-SD instance name announced over mDNS, e.g.
    /// `"CIET-Educational-Simulator-v2"`. Used as the fallback when a
    /// caller-supplied instance name sanitises to nothing.
    pub mdns_instance_prefix: &'static str,

    /// Value of the mDNS `product` TXT record that identifies an announcement
    /// as *this* simulator, e.g. `"ciet-educational-simulator-v2"`.
    ///
    /// `_opcua-tcp._tcp` is the generic OPC-UA service type, so every other
    /// OPC-UA server on the link appears under it too; this marker is what a
    /// browser filters on.
    pub mdns_product_marker: &'static str,

    /// Short prefix for this simulator's console lines, e.g. `"CIET v2"`. The
    /// shared layer prints `"<prefix> OPC-UA: ..."` and `"<prefix> mDNS: ..."`.
    pub log_prefix: &'static str,
}

/// A simulator that can be served over OPC-UA: the identity half of the seam.
///
/// Implement it on a zero-sized marker type and give it one
/// [`OpcuaSimulatorProfile`]. It carries no data and no behaviour, so it costs
/// nothing at runtime; it exists so the shared layer's types
/// ([`super::server::OpcuaServerConfig`], [`super::discovery::MdnsBrowser`])
/// can be bound to one simulator at compile time.
///
/// ```ignore
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// pub struct MySimulator;
///
/// impl OpcuaSimulator for MySimulator {
///     const PROFILE: OpcuaSimulatorProfile = OpcuaSimulatorProfile { /* ... */ };
/// }
/// ```
pub trait OpcuaSimulator: Copy + Send + Sync + 'static {
    /// This simulator's naming and identity strings.
    const PROFILE: OpcuaSimulatorProfile;
}

/// Which of the two address-space folders a variable is filed under.
///
/// Enum rather than a boolean so a third folder becomes a compile error at
/// every match site rather than an inverted flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpcuaFolder {
    /// Read-only values the simulator publishes.
    Outputs,
    /// Values a client may write.
    Controls,
}

/// One OPC-UA variable a simulator publishes: the variable half of the seam.
///
/// Implement it on a **`Copy` enum** whose variants enumerate every variable —
/// typically one that wraps the simulator's own signal / control / switch
/// enums, as `ciet_opcua`'s `CietNode` does. The shared layer calls these
/// methods to build the address space, to serve reads, and to route writes; it
/// never needs to know what any of them mean.
///
/// ## Associated types
///
/// - [`Snapshot`](Self::Snapshot) is the simulator's flat plant state, shared
///   as `Arc<RwLock<Snapshot>>`. Reads clone it under a read lock.
/// - [`Requests`](Self::Requests) is the simulator's pending-write mailbox,
///   shared as `Arc<RwLock<Requests>>`. Writes are **recorded** there, never
///   applied to the snapshot, so the simulator's own thread stays the only
///   writer of plant state.
///
/// ## Units
///
/// Values cross this seam as bare OPC-UA `Variant`s, so the unit is *not*
/// carried in the type. State the engineering unit in
/// [`description`](Self::description) — that string is what a client displays
/// next to the number, and it is the only place a remote operator can learn
/// whether they are writing kW or W.
pub trait OpcuaVariable: Copy + Send + Sync + 'static {
    /// The simulator this variable belongs to, supplying the naming profile.
    type Simulator: OpcuaSimulator;

    /// The plant-state snapshot this variable is read out of.
    ///
    /// Cloned once per read callback and once per subscription push, so keep it
    /// a flat struct of scalars rather than something expensive to copy.
    type Snapshot: Send + Sync + Clone + 'static;

    /// The pending-request mailbox a client's write is recorded in.
    ///
    /// The simulator's physics thread drains it at the top of its next
    /// timestep, which is where clamping and range enforcement happen.
    type Requests: Send + Sync + 'static;

    /// Every variable, in the order the address space presents them.
    ///
    /// Outputs conventionally come first. Called twice at start-up and never
    /// again, so allocating a `Vec` here is not on any hot path.
    fn all() -> Vec<Self>;

    /// The string part of this variable's `NodeId`, e.g.
    /// `"CIET.Temperature.BT12HeaterOutletDegC"`.
    ///
    /// Must be unique across every variable *and* distinct from the two folder
    /// node identifiers in the profile, or the address-space insertion fails
    /// with [`super::server::OpcuaServerError::AddressSpace`]. Treat these as
    /// public API: client configurations and saved trend definitions reference
    /// them by name.
    fn node_identifier(&self) -> &'static str;

    /// Short OPC-UA browse name — a **single path segment**, so it must be
    /// non-empty and contain no `.`.
    fn browse_name(&self) -> &'static str;

    /// Human-facing label shown by clients and by the simulator's own UI.
    fn display_name(&self) -> &'static str;

    /// Description a client displays. **Name the engineering unit here**, and
    /// for a writable variable the envelope its writes are held to.
    fn description(&self) -> String;

    /// OPC-UA data type of the served value. Must agree with what
    /// [`read`](Self::read) returns.
    fn data_type(&self) -> DataTypeId;

    /// Access level. The shared layer registers a write callback for exactly
    /// those variables whose access level contains `CURRENT_WRITE`, so this is
    /// the single source of truth for writability — there is no second flag to
    /// contradict it.
    fn access_level(&self) -> AccessLevel;

    /// Which folder this variable is filed under.
    fn folder(&self) -> OpcuaFolder;

    /// Read this variable's current value out of a plant-state snapshot.
    ///
    /// The returned `Variant` must match [`data_type`](Self::data_type). This
    /// is the only place the shared read callbacks touch simulator state, so
    /// the mapping from node to field exists exactly once.
    fn read(&self, snapshot: &Self::Snapshot) -> Variant;

    /// Record a client's write as a *pending request*.
    ///
    /// **Do not apply it to the snapshot here.** Park it in `requests`; the
    /// simulator's physics thread applies and clamps it on its next timestep,
    /// which is what stops a remote write racing the simulator's own writers.
    ///
    /// Return `Good` when the request was recorded, `BadTypeMismatch` for a
    /// payload of the wrong type, `BadNothingToDo` for an empty one, and
    /// `BadNotWritable` for a read-only variable (which the shared layer never
    /// calls this on, since it registers no write callback for one).
    fn record_write(&self, requests: &mut Self::Requests, value: DataValue) -> StatusCode;
}

/// Interpret an OPC-UA `Variant` as an `f64`, or `None` if it is not a number.
///
/// A helper for implementing [`OpcuaVariable::record_write`] on a continuous
/// control. `Double` is normally such a control's declared type, but real
/// clients also send `Float` and the integer types (a spin box bound to an
/// `i32`, a script writing `5` rather than `5.0`), so those are accepted and
/// widened — exactly, at any magnitude. Strings, booleans and structures are
/// refused, because silently reinterpreting them would hide a client bug.
///
/// The value is dimensionless here: its unit is whatever the variable's
/// [`description`](OpcuaVariable::description) says it is.
pub fn variant_as_f64(variant: &Variant) -> Option<f64> {
    match variant {
        Variant::Double(value) => Some(*value),
        Variant::Float(value) => Some(*value as f64),
        Variant::SByte(value) => Some(*value as f64),
        Variant::Byte(value) => Some(*value as f64),
        Variant::Int16(value) => Some(*value as f64),
        Variant::UInt16(value) => Some(*value as f64),
        Variant::Int32(value) => Some(*value as f64),
        Variant::UInt32(value) => Some(*value as f64),
        Variant::Int64(value) => Some(*value as f64),
        Variant::UInt64(value) => Some(*value as f64),
        _ => None,
    }
}
