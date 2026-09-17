//! OPC-UA (IEC 62541) interface definition for the HTGR demonstration
//! simulator (`examples/htgr_sim_v1`).
//!
//! This module answers two questions and nothing else:
//!
//! - **What does the simulator publish?** — [`node_map`], the enums and plain
//!   [`node_map::NodeDescriptor`] data describing every variable, its SI unit,
//!   its description, its write envelope, and how much of it rests on real
//!   physics.
//! - **What does it read those values out of?** — [`state`], the flat SI
//!   [`state::HtgrPlantSnapshot`] the physics thread publishes and the server
//!   thread reads.
//!
//! It contains **no physics** (per this crate's `CLAUDE.md`), **no GUI**, and
//! **no transport**: no `async-opcua` types, no server config, no runtime, no
//! sockets. Everything here is plain data plus pure accessor functions, so it
//! can be unit-tested without standing up a server and re-pointed at whatever
//! shared OPC-UA scaffolding the crate settles on. Like [`crate::animation`]
//! and [`crate::htr10`] it is pure `std`, so it builds on Android/Termux with
//! no target gate — do not add an `egui`/`eframe` import here.
//!
//! # Unit convention, in one line
//!
//! **Everything on the wire is a bare `f64` in SI units** — W, K, Pa, kg/s, s,
//! J/kg, J/(kg K), dimensionless — with the unit named in the browse name, the
//! display name and the description of every node. The full rationale, and the
//! reason display units (MW, kPa, MPa, pcm, degC) are banned from this
//! interface, is at the top of [`node_map`]. Read it before adding a node.
//!
//! # Wiring this up (deliberately not done here)
//!
//! Nothing declares this module yet, and it depends on no server type on
//! purpose: the generic OPC-UA scaffolding is being extracted out of
//! [`crate::ciet_opcua`] separately (bead `op-szmi.1`). Three seams have to be
//! reconciled when the two land:
//!
//! | Seam | Defined here | Must agree with |
//! |---|---|---|
//! | Namespace URI | [`node_map::HTGR_NAMESPACE_URI`] | whatever the shared server takes as its namespace parameter |
//! | Endpoint / port | [`node_map::ENDPOINT_PATH`], [`node_map::DEFAULT_OPCUA_PORT`] (4841, **not** CIET's 4840) | the shared server config type that replaces `ciet_opcua`'s hard-coded pair |
//! | Shared state handle | [`state::SharedHtgrSnapshot`] (`Arc<RwLock<HtgrPlantSnapshot>>`) | however the shared server takes its state — `ciet_opcua` uses the same `Arc<RwLock<_>>` shape today |
//!
//! The address space itself needs no further glue: iterate
//! [`node_map::all_nodes`], create one variable per descriptor in the folder
//! named by its [`node_map::Subsystem`], and serve reads and writes through
//! [`node_map::NodeSource`].
//!
//! # Naming: this is not HTR-10 yet
//!
//! The bead this module was written for calls it the HTR-10 node map, but the
//! simulator behind it is a **generic** helium-cooled, graphite-moderated HTGR
//! at an illustrative ~200 MWth on ~85 kg/s of helium. The real HTR-10 is
//! 10 MWth on 4.3 kg/s ([`crate::htr10::design`] carries the cited figures).
//! Publishing this model under an HTR-10 identity would tell every client it
//! was looking at a specific licensed design, so the namespace URI, the node
//! prefix and the descriptions all say HTGR. When the HTR-10 rewrite (bead
//! `op-jyyp`) replaces the plant model, this map must be revisited as a whole —
//! renaming the namespace alone would be worse than leaving it.
//!
//! # Scope (`RESPONSIBLE_USE.md`)
//!
//! OPC-UA is a plant-connectivity protocol, so the boundary has to be stated
//! rather than assumed: this interface exists so an **offline demonstration
//! simulator** can be driven by standard OPC-UA tooling on a bench or in a
//! classroom. It must never be connected to live operational systems, plant
//! systems, safety-critical infrastructure, real-time plant monitoring, or
//! institutional production systems. Nothing it publishes is a measurement,
//! nothing it publishes is validated, and its outputs are not authoritative for
//! any operational, licensing or safety purpose.

pub mod node_map;
pub mod state;

pub use node_map::{
    all_nodes, total_node_count, HtgrControl, HtgrSignal, ModelFidelity, NodeAccess,
    NodeDescriptor, NodeSource, Subsystem, DEFAULT_OPCUA_PORT, ENDPOINT_PATH, HTGR_NAMESPACE_URI,
    UNIT_CONVENTION_NOTICE,
};
pub use state::{HtgrPlantSnapshot, SharedHtgrSnapshot};
