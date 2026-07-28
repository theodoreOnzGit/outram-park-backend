//! Cooperative mDNS / DNS-SD announcement and browsing for CIET v2.
//!
//! The problem this solves: a student runs the simulator on a laptop, a
//! demonstrator runs the bundled OPC-UA client on a phone, and neither wants to
//! type an IP address. So the **simulator volunteers its presence** on the local
//! link with a standard DNS-SD announcement, and the **client listens** for it.
//!
//! | Direction | Entry point |
//! |---|---|
//! | Server announces itself | [`advertise_simulator`] → [`MdnsAdvertisement`] |
//! | Client listens for servers | [`SimulatorBrowser::start`] → [`SimulatorBrowser::discovered`] |
//!
//! ## This is announcement only — never scanning
//!
//! **The only network traffic this module originates is a multicast DNS-SD
//! announcement of *this* machine's own service, and multicast queries for the
//! `_opcua-tcp._tcp` service type.** Nothing here probes, sweeps, enumerates or
//! fingerprints another host. A server that has not chosen to announce itself is
//! simply not discovered, and that is the correct behaviour.
//!
//! **Do not add a port scanner, a subnet sweeper, an ARP/ICMP host prober, or
//! any "just try every address in the /24" loop to this module or anywhere else
//! in this crate.** Unsolicited scanning of a network you do not administer
//! breaches institutional acceptable-use policy, and it is out of scope per the
//! workspace `RESPONSIBLE_USE.md` — this project is for education, research and
//! V&V, not for network reconnaissance. If discovery does not work on a given
//! network, the supported answer is "type the URL in by hand", not "find it by
//! probing".
//!
//! ## Practical caveat: many networks break this
//!
//! mDNS is a link-local multicast protocol, and a great many networks
//! deliberately stop it working:
//!
//! - **Campus and enterprise WiFi** commonly enable **client isolation** (also
//!   sold as "AP isolation" / "peer-to-peer blocking"), so two devices on the
//!   same SSID cannot reach each other at all — no multicast, and no direct TCP
//!   either. Discovery fails, *and* so does the subsequent OPC-UA connection,
//!   even with a hand-typed URL.
//! - Many managed networks **filter multicast / UDP 5353** outright, or place
//!   wired and wireless clients in different VLANs.
//! - Guest networks, VPNs and container bridge networks routinely do both.
//!
//! A home router or a phone hotspot generally works fine, and so does a single
//! machine talking to itself over loopback. When demonstrating on institutional
//! WiFi, expect to need a hotspot. This is a property of the network, not a bug
//! in this module, and no amount of code here can fix it.
//!
//! ## Scope (`RESPONSIBLE_USE.md`)
//!
//! What gets announced is an **offline educational simulator**. This module must
//! never be used to discover, enumerate or connect to live operational systems,
//! plant systems, safety-critical infrastructure or institutional production
//! systems.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::{Arc, RwLock};

use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent, ServiceInfo};

use super::node_map::ENDPOINT_PATH;

/// DNS-SD service type CIET v2 announces itself under.
///
/// `_opcua-tcp._tcp` is the service type registered with IANA for OPC-UA over
/// TCP, and is what OPC-UA tooling (UaExpert's "local discovery", for instance)
/// already looks for. The trailing `.local.` is the mDNS domain.
///
/// Because this is the *generic* OPC-UA type, any other OPC-UA server on the
/// link will also appear under it — which is why every CIET announcement also
/// carries the [`CIET_PRODUCT_TXT_VALUE`] TXT marker and
/// [`SimulatorBrowser::discovered`] filters on it.
pub const CIET_MDNS_SERVICE_TYPE: &str = "_opcua-tcp._tcp.local.";

/// Prefix of the DNS-SD instance name the simulator announces.
///
/// The full instance name is this prefix, optionally followed by `-` and a
/// caller-supplied suffix (a machine name, a bench number, a student's name) so
/// several simulators on one link stay distinguishable.
pub const CIET_MDNS_INSTANCE_PREFIX: &str = "CIET-Educational-Simulator-v2";

/// TXT record key carrying the OPC-UA endpoint path, e.g. `/ciet`.
///
/// A client needs this to build the full endpoint URL, because DNS-SD gives it
/// only a host and a port.
pub const PATH_TXT_KEY: &str = "path";

/// TXT record key carrying the product marker.
pub const PRODUCT_TXT_KEY: &str = "product";

/// TXT record value that identifies an announcement as *this* simulator.
///
/// [`SimulatorBrowser::discovered`] returns only services whose
/// [`PRODUCT_TXT_KEY`] equals this, so a CIET instance is never confused with
/// some other OPC-UA server that happens to share the link.
pub const CIET_PRODUCT_TXT_VALUE: &str = "ciet-educational-simulator-v2";

/// A CIET v2 simulator that announced itself on the local link.
///
/// Every field is transport metadata — hostnames, ports, IP addresses. No
/// physical quantity and no units are involved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredSimulator {
    /// DNS-SD instance name as announced, e.g.
    /// `"CIET-Educational-Simulator-v2-bench3"`. This is what a user picks from
    /// in a list.
    pub instance_name: String,
    /// mDNS host name of the machine running the simulator, e.g.
    /// `"ciet-bench3.local."`.
    pub host: String,
    /// TCP port the OPC-UA server listens on. Normally
    /// [`super::node_map::DEFAULT_OPCUA_PORT`] (4840).
    pub port: u16,
    /// Ready-to-use OPC-UA endpoint URL, e.g.
    /// `"opc.tcp://192.168.1.42:4840/ciet"`.
    ///
    /// Built from the first announced IPv4 address where there is one (an IP
    /// literal connects on more networks than an mDNS host name, which needs a
    /// working `.local.` resolver), otherwise from [`Self::host`]. The path
    /// comes from the announcement's `path` TXT record.
    pub endpoint_url: String,
    /// Every IP address announced for the service, in the order IPv4 first then
    /// IPv6. Useful when the first-choice address is unreachable.
    pub addresses: Vec<IpAddr>,
}

/// A live mDNS announcement of this simulator, which is withdrawn when dropped.
///
/// Keep it alive for as long as the server is listening: dropping it unregisters
/// the service and shuts down the mDNS daemon thread, so clients stop seeing the
/// simulator. There are no lifetime parameters — the value owns everything it
/// needs.
pub struct MdnsAdvertisement {
    /// The daemon that owns the announcement. `ServiceDaemon` is internally an
    /// `Arc` around a background thread, so this is cheap to hold.
    daemon: ServiceDaemon,
    /// Fully-qualified service name, needed to unregister.
    fullname: String,
    /// Instance name as announced, for logging and for the GUI to display.
    instance_name: String,
}

impl MdnsAdvertisement {
    /// The DNS-SD instance name actually announced.
    ///
    /// May differ from what the caller asked for: the name is sanitised to the
    /// characters DNS-SD allows.
    pub fn instance_name(&self) -> &str {
        &self.instance_name
    }

    /// The fully-qualified DNS-SD service name, e.g.
    /// `"CIET-Educational-Simulator-v2._opcua-tcp._tcp.local."`.
    pub fn fullname(&self) -> &str {
        &self.fullname
    }
}

impl std::fmt::Debug for MdnsAdvertisement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MdnsAdvertisement")
            .field("instance_name", &self.instance_name)
            .field("fullname", &self.fullname)
            .finish_non_exhaustive()
    }
}

impl Drop for MdnsAdvertisement {
    fn drop(&mut self) {
        // Withdraw the announcement, then stop the daemon. Both are best-effort:
        // a failure here means the announcement will simply age out of clients'
        // caches after its TTL, which is not worth panicking over during a
        // shutdown path.
        if let Err(error) = self.daemon.unregister(&self.fullname) {
            eprintln!(
                "CIET v2 mDNS: could not unregister {} ({error})",
                self.fullname
            );
        }
        if let Err(error) = self.daemon.shutdown() {
            eprintln!("CIET v2 mDNS: could not shut down the daemon ({error})");
        }
    }
}

/// Announce this simulator on the local link over mDNS / DNS-SD.
///
/// `port` is the TCP port the OPC-UA server listens on (normally
/// [`super::node_map::DEFAULT_OPCUA_PORT`], 4840). `instance_name` is a short
/// human-facing label; it is sanitised to DNS-SD-safe characters and, if empty
/// after sanitisation, replaced by [`CIET_MDNS_INSTANCE_PREFIX`].
///
/// The announcement carries two TXT records so a browser can tell a CIET
/// instance apart from any other OPC-UA server on the link:
///
/// ```text
/// path=/ciet
/// product=ciet-educational-simulator-v2
/// ```
///
/// Addresses are filled in automatically by `mdns-sd` (`enable_addr_auto`), and
/// tracked as interfaces come and go, so a laptop that moves from ethernet to
/// WiFi keeps announcing a reachable address.
///
/// Keep the returned [`MdnsAdvertisement`] alive for as long as the server runs;
/// dropping it withdraws the announcement.
///
/// # Errors
///
/// Returns [`DiscoveryError::DaemonStart`] if no mDNS daemon can be started (no
/// usable multicast interface, or UDP 5353 unavailable), and
/// [`DiscoveryError::Register`] if the announcement itself is rejected. Neither
/// is fatal to the simulator: the OPC-UA server still works, users just have to
/// type the URL.
pub fn advertise_simulator(
    port: u16,
    instance_name: &str,
) -> Result<MdnsAdvertisement, DiscoveryError> {
    let instance_name = sanitise_instance_name(instance_name);
    let host_name = format!("{instance_name}.local.");

    let mut properties: BTreeMap<String, String> = BTreeMap::new();
    properties.insert(PATH_TXT_KEY.to_owned(), ENDPOINT_PATH.to_owned());
    properties.insert(
        PRODUCT_TXT_KEY.to_owned(),
        CIET_PRODUCT_TXT_VALUE.to_owned(),
    );
    let properties: Vec<(String, String)> = properties.into_iter().collect();

    let daemon = ServiceDaemon::new().map_err(|e| DiscoveryError::DaemonStart(e.to_string()))?;

    // `()` for the address list plus `enable_addr_auto()`: mdns-sd fills in this
    // machine's addresses itself and keeps them current as interfaces change.
    let service_info = ServiceInfo::new(
        CIET_MDNS_SERVICE_TYPE,
        &instance_name,
        &host_name,
        (),
        port,
        properties.as_slice(),
    )
    .map_err(|e| DiscoveryError::Register(e.to_string()))?
    .enable_addr_auto();

    let fullname = service_info.get_fullname().to_owned();

    daemon
        .register(service_info)
        .map_err(|e| DiscoveryError::Register(e.to_string()))?;

    Ok(MdnsAdvertisement {
        daemon,
        fullname,
        instance_name,
    })
}

/// Listens for CIET v2 simulators announcing themselves on the local link.
///
/// Construct once with [`start`](Self::start), then poll
/// [`discovered`](Self::discovered) whenever convenient — from an egui repaint,
/// from a CLI loop, from anywhere. Polling never blocks, so it is safe to call
/// at frame rate.
///
/// There are no lifetime parameters: the browser owns its daemon, its event
/// receiver, and an `Arc<RwLock<..>>` of everything found so far.
pub struct SimulatorBrowser {
    /// Kept alive so the browse operation keeps running; shut down on `Drop`.
    daemon: ServiceDaemon,
    /// Events from the browse operation. Drained on each
    /// [`discovered`](Self::discovered) call.
    events: mdns_sd::Receiver<ServiceEvent>,
    /// Everything found so far, keyed by DNS-SD full name so a re-announcement
    /// updates rather than duplicates an entry.
    ///
    /// `Arc<RwLock<..>>` per the workspace design rules, so a GUI thread and a
    /// worker can both look at the list.
    found: Arc<RwLock<BTreeMap<String, DiscoveredSimulator>>>,
}

impl SimulatorBrowser {
    /// Start listening for CIET v2 announcements.
    ///
    /// This issues multicast DNS-SD **queries for the `_opcua-tcp._tcp` service
    /// type** and nothing else. It does not probe hosts, sweep addresses, or
    /// contact any server that has not announced itself.
    ///
    /// # Errors
    ///
    /// [`DiscoveryError::DaemonStart`] if no mDNS daemon can be started, and
    /// [`DiscoveryError::Browse`] if the browse operation is rejected. On a
    /// network with client isolation or multicast filtering the call typically
    /// *succeeds* and simply never finds anything — see the module docs.
    pub fn start() -> Result<Self, DiscoveryError> {
        let daemon =
            ServiceDaemon::new().map_err(|e| DiscoveryError::DaemonStart(e.to_string()))?;
        let events = daemon
            .browse(CIET_MDNS_SERVICE_TYPE)
            .map_err(|e| DiscoveryError::Browse(e.to_string()))?;

        Ok(Self {
            daemon,
            events,
            found: Arc::new(RwLock::new(BTreeMap::new())),
        })
    }

    /// Every CIET v2 simulator found so far, sorted by instance name.
    ///
    /// **Non-blocking.** Each call drains whatever mDNS events have arrived
    /// since the last call into the browser's internal list, then returns a
    /// snapshot of that list. Call it as often as you like.
    ///
    /// Only announcements carrying `product=`[`CIET_PRODUCT_TXT_VALUE`] are
    /// kept. Other OPC-UA servers on the same link are seen and deliberately
    /// discarded, because `_opcua-tcp._tcp` is the generic OPC-UA service type.
    ///
    /// A `ServiceRemoved` event drops the entry, so a simulator that is shut
    /// down disappears from the list.
    pub fn discovered(&self) -> Vec<DiscoveredSimulator> {
        // Drain the receiver. `try_recv` returns `Err` the moment the queue is
        // empty (or the daemon has gone away), which is the loop's exit.
        while let Ok(event) = self.events.try_recv() {
            match event {
                ServiceEvent::ServiceResolved(resolved) => {
                    if let Some(simulator) = simulator_from_resolved(&resolved) {
                        if let Ok(mut found) = self.found.write() {
                            found.insert(resolved.fullname.clone(), simulator);
                        }
                    }
                }
                ServiceEvent::ServiceRemoved(_service_type, fullname) => {
                    if let Ok(mut found) = self.found.write() {
                        found.remove(&fullname);
                    }
                }
                // SearchStarted / ServiceFound / SearchStopped carry no
                // endpoint information; the resolved event is the useful one.
                _ => {}
            }
        }

        let Ok(found) = self.found.read() else {
            return Vec::new();
        };
        let mut simulators: Vec<DiscoveredSimulator> = found.values().cloned().collect();
        simulators.sort_by(|a, b| a.instance_name.cmp(&b.instance_name));
        simulators
    }
}

impl std::fmt::Debug for SimulatorBrowser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.found.read().map(|f| f.len()).unwrap_or(0);
        f.debug_struct("SimulatorBrowser")
            .field("service_type", &CIET_MDNS_SERVICE_TYPE)
            .field("known_simulators", &count)
            .finish_non_exhaustive()
    }
}

impl Drop for SimulatorBrowser {
    fn drop(&mut self) {
        if let Err(error) = self.daemon.stop_browse(CIET_MDNS_SERVICE_TYPE) {
            eprintln!("CIET v2 mDNS: could not stop browsing ({error})");
        }
        if let Err(error) = self.daemon.shutdown() {
            eprintln!("CIET v2 mDNS: could not shut down the browse daemon ({error})");
        }
    }
}

/// Turn a resolved DNS-SD service into a [`DiscoveredSimulator`], or `None` if
/// it is not a CIET v2 simulator.
///
/// The filter is the `product` TXT record: an OPC-UA server that does not
/// announce `product=`[`CIET_PRODUCT_TXT_VALUE`] is not ours and is ignored.
fn simulator_from_resolved(resolved: &ResolvedService) -> Option<DiscoveredSimulator> {
    let product = resolved
        .txt_properties
        .get(PRODUCT_TXT_KEY)
        .map(|p| p.val_str().to_owned())?;
    if product != CIET_PRODUCT_TXT_VALUE {
        return None;
    }

    let path = resolved
        .txt_properties
        .get(PATH_TXT_KEY)
        .map(|p| p.val_str().to_owned())
        .unwrap_or_else(|| ENDPOINT_PATH.to_owned());

    // IPv4 first: a v4 literal reaches further on the sort of flat home /
    // hotspot networks this is used on, and needs no scope id.
    let mut addresses: Vec<IpAddr> = resolved
        .addresses
        .iter()
        .map(|scoped| scoped.to_ip_addr())
        .collect();
    addresses.sort_by_key(|ip| (ip.is_ipv6(), ip.to_string()));

    // Prefer an IP literal over the `.local.` host name: mDNS host-name
    // resolution needs a working resolver on the client, which Android and some
    // Linux setups lack, while the literal always works if the host is
    // reachable at all.
    let authority = match addresses.first() {
        Some(IpAddr::V4(v4)) => v4.to_string(),
        Some(IpAddr::V6(v6)) => format!("[{v6}]"),
        None => resolved.host.trim_end_matches('.').to_owned(),
    };

    let instance_name = instance_name_from_fullname(&resolved.fullname);
    let endpoint_url = format!("opc.tcp://{authority}:{}{path}", resolved.port);

    Some(DiscoveredSimulator {
        instance_name,
        host: resolved.host.clone(),
        port: resolved.port,
        endpoint_url,
        addresses,
    })
}

/// Strip the service type off a DNS-SD full name, leaving the instance name.
///
/// `"CIET-Educational-Simulator-v2-bench3._opcua-tcp._tcp.local."` becomes
/// `"CIET-Educational-Simulator-v2-bench3"`.
fn instance_name_from_fullname(fullname: &str) -> String {
    match fullname.find("._") {
        Some(index) => fullname[..index].to_owned(),
        None => fullname.to_owned(),
    }
}

/// Reduce an arbitrary label to something safe to use as a DNS-SD instance
/// name and as the leading label of an mDNS host name.
///
/// Keeps ASCII alphanumerics and `-`, maps everything else (spaces, dots,
/// underscores, non-ASCII) to `-`, collapses runs of `-`, and trims leading and
/// trailing `-`. An empty result falls back to [`CIET_MDNS_INSTANCE_PREFIX`],
/// and the result is truncated to 63 characters — the DNS label limit.
fn sanitise_instance_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    let cleaned = if trimmed.is_empty() {
        CIET_MDNS_INSTANCE_PREFIX
    } else {
        trimmed
    };

    // 63 octets is the maximum length of a single DNS label (RFC 1035 2.3.4).
    // Truncate on a char boundary; the filter above leaves only ASCII, so byte
    // and char indices coincide.
    let mut cleaned = cleaned.to_owned();
    cleaned.truncate(63);
    cleaned.trim_end_matches('-').to_owned()
}

/// Things that can go wrong while announcing or browsing over mDNS.
///
/// None of these is fatal to the simulator or the client: OPC-UA still works
/// with a hand-typed URL. Report them, do not abort on them.
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    /// No mDNS daemon could be started — typically no usable multicast
    /// interface, or UDP 5353 already taken by a system responder that will not
    /// share it.
    #[error(
        "could not start the mDNS daemon: {0} (discovery is unavailable; connect by URL instead)"
    )]
    DaemonStart(String),

    /// The service announcement was rejected, e.g. an instance name that is
    /// still invalid after sanitisation, or a TXT record over the 255-byte
    /// per-record limit.
    #[error("could not register the mDNS service announcement: {0}")]
    Register(String),

    /// The browse operation could not be started.
    #[error("could not start browsing for {CIET_MDNS_SERVICE_TYPE} over mDNS: {0}")]
    Browse(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that [`CIET_MDNS_SERVICE_TYPE`] is a well-formed DNS-SD service
    /// type, since a malformed one makes both announcement and browsing fail at
    /// runtime on a machine that may not be the developer's.
    ///
    /// **Methodology.** Check the string against the DNS-SD structural rules of
    /// RFC 6763 §7 and RFC 6335: the type is `_<service>._<proto>.<domain>.`,
    /// so it must (a) end with `.local.`, (b) contain exactly two
    /// underscore-prefixed labels, (c) name a transport protocol of `_tcp` or
    /// `_udp`, (d) have a service label of 1–15 characters after its underscore,
    /// and (e) use only letters, digits and hyphens in that label. Pass
    /// criterion: all five hold. No socket is opened, so the test needs no
    /// network.
    ///
    /// **Results (2026-07-28).** `_opcua-tcp._tcp.local.` — ends with
    /// `.local.` = true; labels = `["_opcua-tcp", "_tcp", "local"]`, of which 2
    /// are underscore-prefixed; protocol label = `_tcp`; service label
    /// `opcua-tcp` is 9 characters (≤ 15) and uses only letters and a hyphen.
    /// Interpretation: the constant is the IANA-registered OPC-UA-over-TCP
    /// service type, correctly formed, so tooling that already browses for OPC-UA
    /// servers will see a CIET announcement.
    #[test]
    fn mdns_service_type_is_a_valid_dns_sd_type() {
        let service_type = CIET_MDNS_SERVICE_TYPE;
        assert!(
            service_type.ends_with(".local."),
            "{service_type} is not in the mDNS .local. domain"
        );

        let labels: Vec<&str> = service_type.trim_end_matches('.').split('.').collect();
        assert_eq!(
            labels.len(),
            3,
            "expected <service>.<proto>.local: {labels:?}"
        );

        let underscore_labels = labels.iter().filter(|l| l.starts_with('_')).count();
        assert_eq!(
            underscore_labels, 2,
            "expected exactly two underscore-prefixed labels: {labels:?}"
        );

        assert!(
            labels[1] == "_tcp" || labels[1] == "_udp",
            "protocol label must be _tcp or _udp, got {}",
            labels[1]
        );

        let service_label = labels[0].trim_start_matches('_');
        assert!(
            (1..=15).contains(&service_label.len()),
            "service label {service_label} must be 1-15 characters (RFC 6335)"
        );
        assert!(
            service_label
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "service label {service_label} has characters DNS-SD does not allow"
        );
    }

    /// Verifies that an arbitrary human label is reduced to a DNS-safe instance
    /// name, because the label goes into both the DNS-SD instance name and the
    /// leading label of the announced mDNS host name.
    ///
    /// **Methodology.** Feed [`sanitise_instance_name`] a set of awkward inputs
    /// and check the result contains only ASCII alphanumerics and `-`, has no
    /// leading or trailing `-`, is at most 63 characters (the RFC 1035 label
    /// limit), and is never empty. No network is used.
    ///
    /// **Results (2026-07-28).** `"bench 3"` → `"bench-3"`;
    /// `"CIET (teaching)"` → `"CIET-teaching"`; `"...."` → the
    /// `CIET-Educational-Simulator-v2` fallback; `""` → the same fallback; a
    /// 200-character input → 63 characters. All four constraints held for every
    /// case. Interpretation: a user may type anything into the instance-name
    /// field without producing an announcement mDNS would reject.
    #[test]
    fn instance_names_are_sanitised_to_dns_safe_labels() {
        let cases = [
            "bench 3",
            "CIET (teaching)",
            "....",
            "",
            "  leading and trailing  ",
            &"x".repeat(200),
        ];

        for raw in cases {
            let cleaned = sanitise_instance_name(raw);
            assert!(!cleaned.is_empty(), "{raw:?} sanitised to an empty label");
            assert!(
                cleaned.len() <= 63,
                "{raw:?} sanitised to {} characters, over the DNS label limit",
                cleaned.len()
            );
            assert!(
                cleaned
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-'),
                "{raw:?} sanitised to {cleaned:?}, which has illegal characters"
            );
            assert!(
                !cleaned.starts_with('-') && !cleaned.ends_with('-'),
                "{raw:?} sanitised to {cleaned:?}, which has a stray hyphen at an end"
            );
        }

        assert_eq!(sanitise_instance_name("bench 3"), "bench-3");
        assert_eq!(sanitise_instance_name(""), CIET_MDNS_INSTANCE_PREFIX);
    }

    /// Verifies that the DNS-SD full name is reduced to the instance name a user
    /// sees in the client's simulator list.
    ///
    /// **Methodology.** Feed [`instance_name_from_fullname`] a full name of the
    /// documented shape and a degenerate one with no service type. Pass
    /// criterion: the service type is stripped in the first case, and the input
    /// is returned unchanged in the second.
    ///
    /// **Results (2026-07-28).**
    /// `"CIET-Educational-Simulator-v2-bench3._opcua-tcp._tcp.local."` →
    /// `"CIET-Educational-Simulator-v2-bench3"`; `"no-service-type"` →
    /// `"no-service-type"`. Both as expected.
    #[test]
    fn instance_name_is_stripped_from_the_dns_sd_fullname() {
        assert_eq!(
            instance_name_from_fullname(
                "CIET-Educational-Simulator-v2-bench3._opcua-tcp._tcp.local."
            ),
            "CIET-Educational-Simulator-v2-bench3"
        );
        assert_eq!(
            instance_name_from_fullname("no-service-type"),
            "no-service-type"
        );
    }
}
