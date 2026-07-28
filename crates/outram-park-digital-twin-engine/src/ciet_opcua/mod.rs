//! OPC-UA (IEC 62541) interface layer for the CIET Educational Simulator v2.
//!
//! This module is the **shared interface** between the two CIET v2 binaries:
//!
//! - `ciet_educational_simulator_v2` — the simulator, which runs the physics and
//!   hosts the OPC-UA server;
//! - `ciet_v2_opcua_client` — the bundled demo client, which discovers a
//!   simulator on the local network and drives it.
//!
//! It contains **no physics** (per this crate's `CLAUDE.md`) and **no GUI**, so
//! it compiles everywhere the workspace targets — including headless on Termux
//! / `aarch64-linux-android`, with no target gate. `async-opcua` was chosen
//! precisely for that: its crypto is RustCrypto, not `openssl-sys`.
//!
//! ## Layout
//!
//! | Module | Role |
//! |---|---|
//! | [`state`] | [`CietState`], the flat plant snapshot shared between threads |
//! | [`node_map`] | the enums defining every OPC-UA variable — the single source of truth |
//! | [`pki_paths`] | where the PKI directory lives (`~/.outram-park/...`) |
//! | [`server`] | the OPC-UA server, run on its own thread with its own tokio runtime |
//! | [`discovery`] | cooperative mDNS announce (server) and browse (client) |
//!
//! ## Reading this module for the first time
//!
//! Start at [`node_map`]. Its three enums — [`CietSignal`] (read-only outputs),
//! [`CietControl`] (writable continuous set points) and [`CietSwitch`]
//! (writable on/off controls) — define the entire interface. The server's
//! address space, its read/write callbacks, the simulator's "how to connect"
//! table and the demo client's variable list are all generated from them, so
//! there is exactly one place to look up what a node means.
//!
//! ## Security: there is none, deliberately
//!
//! The server runs with **`SecurityPolicy::None` and anonymous access**. Anyone
//! who can reach the port can read every output and write every control. That
//! is a deliberate choice for a throwaway teaching demonstrator — it makes
//! "point UaExpert at it and poke the loop" a ten-second exercise — and it is
//! the reason the simulator prints a plain warning banner whenever it is bound
//! to anything other than loopback.
//!
//! Hardening this (certificates, a trust list, user tokens, an audit trail) is
//! explicitly **out of scope** here and left to security researchers. Do not
//! describe this interface as secured, and do not deploy it anywhere that
//! matters.
//!
//! ## Scope limit (`RESPONSIBLE_USE.md`)
//!
//! OPC-UA is a plant-connectivity protocol, so the boundary matters: this
//! interface exists so an **offline educational simulator** can be driven by
//! standard OPC-UA tooling on a bench or in a classroom. It must **never** be
//! connected to live operational systems, plant systems, safety-critical
//! infrastructure, real-time plant monitoring, or institutional production
//! systems, and its outputs are not authoritative for any operational,
//! licensing or safety purpose.

pub mod discovery;
pub mod node_map;
pub mod pki_paths;
pub mod server;
pub mod state;
pub mod user_controls;

pub use node_map::{
    CietControl, CietSignal, CietSwitch, CIET_NAMESPACE_URI, DEFAULT_OPCUA_PORT, ENDPOINT_PATH,
};
pub use state::{CietState, HeaterControlSettings, HeaterType, SharedCietState};
