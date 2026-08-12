//! Reactor-agnostic OPC-UA (IEC 62541) server layer for OUTRAM PARK digital
//! twins.
//!
//! This is the half of an OPC-UA interface that has nothing to do with any
//! particular plant: the TCP transport, the server thread and its `tokio`
//! runtime, the PKI directory, mDNS announcement and browsing, address-space
//! construction, the read/write callbacks and the subscription push. A
//! simulator supplies only **who it is** and **what it publishes**; everything
//! else lives here and is written once.
//!
//! ## Layout
//!
//! | Module | Role |
//! |---|---|
//! | [`simulator`] | the seam — [`OpcuaSimulator`] (identity) and [`OpcuaVariable`] (variables, snapshot, requests) |
//! | [`server`] | the server, run on its own thread with its own tokio runtime |
//! | [`pki`] | where the certificate store lives (`~/.outram-park/...`) |
//! | [`discovery`] | cooperative mDNS announce (server) and browse (client) |
//!
//! ## Adding a second simulator
//!
//! Read [`simulator`] first — it is the whole contract. In outline:
//!
//! 1. Declare a marker type and give it an [`OpcuaSimulatorProfile`] (namespace
//!    URI, endpoint path, PKI directory name, mDNS marker, ...).
//! 2. Declare a `Copy` enum whose variants are the variables, and implement
//!    [`OpcuaVariable`] on it, naming the snapshot type reads come from and the
//!    request type writes are parked in.
//! 3. Wrap [`server::spawn_opcua_server_thread`] in a concrete per-simulator
//!    function, so callers never write a turbofish.
//!
//! `ciet_opcua` is the worked example of all three steps.
//!
//! ## Compile-time dispatch only
//!
//! Both traits are used as generic bounds — no `Box<dyn Trait>`, no
//! `&dyn Trait`, per the workspace `CLAUDE.md` Rust design rules. There are no
//! lifetime parameters anywhere in this module; shared state is
//! `Arc<RwLock<T>>`.
//!
//! ## Portability
//!
//! No GUI, no physics. `async-opcua` is pure Rust (RustCrypto, not
//! `openssl-sys`), so this module builds on Android/Termux with no target gate
//! — a headless Termux build serves OPC-UA exactly as a desktop one does.
//!
//! ## Security: there is none, deliberately
//!
//! Servers built here run with **`SecurityPolicy::None` and anonymous access**.
//! Anyone who can reach the port can read every output and write every control.
//! That is a deliberate choice for throwaway teaching demonstrators, and it is
//! why a warning banner is printed whenever a server binds to anything other
//! than loopback. Hardening (certificates, trust lists, user tokens, audit
//! trails) is explicitly **out of scope** and left to security researchers. Do
//! not describe anything built on this as secured.
//!
//! ## Scope limit (`RESPONSIBLE_USE.md`)
//!
//! OPC-UA is a plant-connectivity protocol, so the boundary matters: this layer
//! exists so **offline educational simulators** can be driven by standard
//! OPC-UA tooling on a bench or in a classroom. It must **never** be connected
//! to live operational systems, plant systems, safety-critical infrastructure,
//! real-time plant monitoring, or institutional production systems, and its
//! outputs are not authoritative for any operational, licensing or safety
//! purpose.

pub mod discovery;
pub mod pki;
pub mod server;
pub mod simulator;

pub use discovery::{
    advertise_simulator, DiscoveredSimulator, DiscoveryError, MdnsAdvertisement, MdnsBrowser,
    OPCUA_MDNS_SERVICE_TYPE,
};
pub use server::{
    spawn_opcua_server_thread, OpcuaEndpointInfo, OpcuaServerConfig, OpcuaServerError,
    OpcuaServerHandle, SUBSCRIPTION_PUSH_INTERVAL,
};
pub use simulator::{variant_as_f64, OpcuaFolder, OpcuaSimulator, OpcuaSimulatorProfile, OpcuaVariable};
