//! CIET v2's binding to the shared mDNS / DNS-SD discovery layer.
//!
//! The announcement and browsing machinery is reactor-agnostic and lives in
//! [`opcua_core::discovery`](crate::opcua_core::discovery). This module supplies
//! the two strings that make an announcement *CIET's* — the instance-name prefix
//! and the `product` TXT marker — and binds the browser to them, so a
//! [`SimulatorBrowser`] reports CIET simulators and ignores every other OPC-UA
//! server on the link.
//!
//! | Direction | Entry point |
//! |---|---|
//! | Simulator announces itself | handled inside [`super::server::spawn_opcua_server_thread`] |
//! | Client listens for simulators | [`SimulatorBrowser::start`] → [`SimulatorBrowser::discovered`] |
//!
//! ## This is announcement only — never scanning
//!
//! The only network traffic originated is a multicast DNS-SD announcement of
//! *this* machine's own service, and multicast queries for the
//! `_opcua-tcp._tcp` service type. Nothing probes, sweeps, enumerates or
//! fingerprints another host, and nothing here may ever grow a port scanner or
//! a subnet sweeper — see [`opcua_core::discovery`](crate::opcua_core::discovery)
//! for the full statement of that rule, and `RESPONSIBLE_USE.md` for why it is
//! not negotiable.
//!
//! ## Practical caveat: many networks break this
//!
//! Campus and enterprise WiFi commonly enable client isolation, and many managed
//! networks filter multicast outright, so discovery finds nothing *and* the
//! subsequent OPC-UA connection fails even with a hand-typed URL. A phone
//! hotspot or a home router works. That is a property of the network, not a bug.
//!
//! ## Units
//!
//! Everything here is transport metadata — host names, ports, IP addresses, DNS
//! labels. No physical quantities, no units.

use crate::opcua_core::discovery::MdnsBrowser;

use super::simulator::CietOpcuaSimulator;

pub use crate::opcua_core::discovery::{
    DiscoveredSimulator, DiscoveryError, MdnsAdvertisement,
    OPCUA_MDNS_SERVICE_TYPE as CIET_MDNS_SERVICE_TYPE, PATH_TXT_KEY, PRODUCT_TXT_KEY,
};

/// Prefix of the DNS-SD instance name the simulator announces.
///
/// The full instance name is this prefix, optionally followed by `-` and a
/// caller-supplied suffix (a machine name, a bench number, a student's name) so
/// several simulators on one link stay distinguishable. It is also the fallback
/// when a supplied instance name sanitises to nothing.
pub const CIET_MDNS_INSTANCE_PREFIX: &str = "CIET-Educational-Simulator-v2";

/// TXT record value that identifies an announcement as *this* simulator.
///
/// [`SimulatorBrowser::discovered`] returns only services whose
/// [`PRODUCT_TXT_KEY`] equals this, so a CIET instance is never confused with
/// some other OPC-UA server that happens to share the link — `_opcua-tcp._tcp`
/// is the generic OPC-UA service type and everything answers to it.
pub const CIET_PRODUCT_TXT_VALUE: &str = "ciet-educational-simulator-v2";

/// Listens for CIET v2 simulators announcing themselves on the local link.
///
/// The shared [`MdnsBrowser`] bound to CIET, so it filters on
/// [`CIET_PRODUCT_TXT_VALUE`] and nothing else needs to know that.
///
/// Construct once with [`start`](MdnsBrowser::start), then poll
/// [`discovered`](MdnsBrowser::discovered) whenever convenient — from an egui
/// repaint, from a CLI loop, from anywhere. Polling never blocks, so it is safe
/// to call at frame rate.
pub type SimulatorBrowser = MdnsBrowser<CietOpcuaSimulator>;
