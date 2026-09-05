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
//! ## What is CIET's, and what is shared
//!
//! The reactor-agnostic half — TCP transport, the server thread and its tokio
//! runtime, PKI paths, mDNS, address-space construction, the read/write
//! callbacks — lives in [`opcua_core`](crate::opcua_core) and serves any OUTRAM
//! PARK simulator. **This module is only CIET's half of that contract**: the
//! plant state, the node map, the identity strings, and the mapping between
//! them. If you are adding a variable, everything you need is here; if you are
//! adding a *second simulator*, read [`opcua_core::simulator`](crate::opcua_core::simulator)
//! instead.
//!
//! ## Layout
//!
//! | Module | Role |
//! |---|---|
//! | [`state`] | [`CietState`], the flat plant snapshot shared between threads |
//! | [`node_map`] | the enums defining every OPC-UA variable — the single source of truth |
//! | [`user_controls`] | the pending-write mailbox remote writes are parked in |
//! | [`simulator`] | CIET's identity profile and [`CietNode`](simulator::CietNode), the shared layer's view of the node map |
//! | [`server`] | starting the shared OPC-UA server, bound to CIET |
//! | [`pki_paths`] | where CIET's PKI directory lives (`~/.outram-park/...`) |
//! | [`discovery`] | CIET's mDNS marker, and a browser bound to it |
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
pub mod simulator;
pub mod state;
pub mod user_controls;

pub use node_map::{
    CietControl, CietSignal, CietSwitch, CIET_NAMESPACE_URI, DEFAULT_OPCUA_PORT, ENDPOINT_PATH,
};
pub use simulator::{CietNode, CietOpcuaSimulator, CIET_OPCUA_PROFILE};
pub use state::{CietState, HeaterControlSettings, HeaterType, SharedCietState};
