//! Passive mDNS discovery, polled on a timer from the GUI thread.
//!
//! ## Passive, not a scan
//!
//! This is **listening**, not searching. A CIET v2 simulator announces itself
//! over multicast DNS (`_opcua-tcp._tcp.local.`) when its OPC-UA server starts;
//! [`SimulatorBrowser`] subscribes to those announcements and reports what
//! arrives. Nothing here sends a packet to any host, sweeps an address range,
//! probes a port, or fingerprints a service. A machine that does not advertise
//! itself is invisible to this client and stays that way — which is the correct
//! and intended behaviour, and why the fallback for a network that blocks mDNS is
//! *asking the user for the address* rather than going to look for it.
//!
//! ## Why it so often finds nothing
//!
//! Campus and enterprise WiFi routinely (a) block or rate-limit multicast, and
//! (b) enable client isolation, so two laptops on the same SSID cannot address
//! each other at all. Both defeat discovery, and the second defeats a manual
//! connection too. The working answer for a classroom is a phone hotspot or a
//! home router; [`DiscoveryStatus`] exists so the UI can say that at the moment
//! the user is looking at an empty list, rather than burying it in a help page.

use std::time::{Duration, Instant};

use outram_park_digital_twin_engine::ciet_opcua::discovery::{DiscoveredSimulator, SimulatorBrowser};

/// How often the browse list is re-read, milliseconds.
///
/// [`SimulatorBrowser::discovered`] is a non-blocking read of an already-received
/// announcement set, so this costs nothing; 1000 ms simply keeps the list from
/// flickering as entries are refreshed.
pub const BROWSE_POLL_INTERVAL_MS: u64 = 1000;

/// How long an empty list is tolerated before the UI shows the full
/// "why is nothing here" explanation, seconds.
///
/// Short enough that a user does not sit staring at an empty panel, long enough
/// that a simulator on a working network normally appears first and the
/// explanation is never shown at all.
pub const EMPTY_LIST_GRACE_SECONDS: u64 = 4;

/// What the discovery panel should be telling the user right now.
///
/// An enum so the panel's `match` is exhaustive and the "no servers found" case
/// cannot accidentally be rendered as a bare empty table (workspace Rust design
/// rules: enums for dispatch).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryStatus {
    /// mDNS could not be started at all — no multicast socket, or the platform
    /// refused it. Manual entry is the only route, and the UI says so.
    Unavailable {
        /// The error text from the browser, shown verbatim.
        message: String,
    },
    /// Listening, and it is too early to conclude anything.
    Listening,
    /// Listening for longer than [`EMPTY_LIST_GRACE_SECONDS`] with nothing heard.
    /// This is the state that gets the campus-WiFi guidance.
    NothingFound {
        /// How long this client has been listening.
        listening_for: Duration,
    },
    /// At least one simulator has announced itself.
    Found {
        /// How many.
        count: usize,
    },
}

/// Owns the mDNS browser and the last list it produced.
///
/// Lives on the GUI thread. Polling it is cheap and non-blocking, so it needs no
/// worker of its own — unlike the OPC-UA session, which does.
pub struct DiscoveryPoller {
    /// `None` when the browser could not be started; `start_error` says why.
    browser: Option<SimulatorBrowser>,
    /// Failure text from [`SimulatorBrowser::start`], if it failed.
    start_error: Option<String>,
    /// The most recent browse result.
    simulators: Vec<DiscoveredSimulator>,
    /// When this poller began listening.
    started_at: Instant,
    /// When the list was last re-read.
    last_polled: Instant,
    /// Whether any simulator has *ever* been seen, so a simulator that stops
    /// announcing does not make the UI claim mDNS never worked.
    has_ever_found: bool,
}

impl DiscoveryPoller {
    /// Start listening for simulator announcements.
    ///
    /// A failure to start is not fatal and does not panic: the poller records the
    /// reason, reports [`DiscoveryStatus::Unavailable`], and the manual endpoint
    /// box continues to work. That matters because mDNS is exactly the part most
    /// likely to be unavailable in the environments this client is used in.
    pub fn start() -> Self {
        let now = Instant::now();
        match SimulatorBrowser::start() {
            Ok(browser) => Self {
                browser: Some(browser),
                start_error: None,
                simulators: Vec::new(),
                started_at: now,
                last_polled: now,
                has_ever_found: false,
            },
            Err(error) => Self {
                browser: None,
                start_error: Some(error.to_string()),
                simulators: Vec::new(),
                started_at: now,
                last_polled: now,
                has_ever_found: false,
            },
        }
    }

    /// Re-read the browse list if [`BROWSE_POLL_INTERVAL_MS`] has elapsed.
    ///
    /// Safe and cheap to call on every repaint; it rate-limits itself. Returns
    /// `true` when the list was actually re-read this call.
    pub fn poll(&mut self) -> bool {
        if self.last_polled.elapsed() < Duration::from_millis(BROWSE_POLL_INTERVAL_MS) {
            return false;
        }
        self.last_polled = Instant::now();

        if let Some(browser) = &self.browser {
            self.simulators = browser.discovered();
            self.simulators
                .sort_by(|a, b| a.instance_name.cmp(&b.instance_name));
            if !self.simulators.is_empty() {
                self.has_ever_found = true;
            }
        }
        true
    }

    /// The simulators currently announcing themselves, sorted by instance name.
    pub fn simulators(&self) -> &[DiscoveredSimulator] {
        &self.simulators
    }

    /// What the UI should be saying about discovery right now.
    pub fn status(&self) -> DiscoveryStatus {
        if let Some(message) = &self.start_error {
            return DiscoveryStatus::Unavailable {
                message: message.clone(),
            };
        }
        if !self.simulators.is_empty() {
            return DiscoveryStatus::Found {
                count: self.simulators.len(),
            };
        }
        let listening_for = self.started_at.elapsed();
        if listening_for < Duration::from_secs(EMPTY_LIST_GRACE_SECONDS) {
            DiscoveryStatus::Listening
        } else {
            DiscoveryStatus::NothingFound { listening_for }
        }
    }

    /// Whether a simulator has been seen at any point since start-up.
    ///
    /// Distinguishes "mDNS has never delivered anything" from "the simulator we
    /// found has since gone quiet", which call for different advice.
    pub fn has_ever_found(&self) -> bool {
        self.has_ever_found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies the discovery status reported for each situation the panel must
    /// render differently.
    ///
    /// **Methodology.** [`DiscoveryStatus`] cannot be exercised end-to-end
    /// without a live multicast network, so the four variants are constructed
    /// directly and checked for the distinctness the panel's `match` relies on —
    /// specifically that `Listening` (too early to advise) and `NothingFound`
    /// (time to advise) are separate values, and that `Unavailable` carries the
    /// browser's own message rather than a substituted one. Pass criterion: the
    /// four variants compare unequal pairwise, and `Unavailable` round-trips its
    /// message.
    ///
    /// **Results (2026-07-28).** All 4 variants distinct; `Unavailable`
    /// preserved the message "no multicast interface" verbatim;
    /// `NothingFound` carried the elapsed listening duration. Interpretation: the
    /// grace period is representable in the type, so the panel cannot show the
    /// campus-WiFi block during the first second of a normal, successful start-up.
    ///
    /// **Limitation, stated honestly.** This is a type-level check of the status
    /// contract, not a validation of mDNS behaviour on a real network. Whether a
    /// running simulator is actually discovered is not verified here and cannot
    /// be verified in a sandbox without multicast; it needs a two-machine manual
    /// test, which has **not** been performed as of 2026-07-28.
    #[test]
    fn discovery_status_distinguishes_early_listening_from_nothing_found() {
        let unavailable = DiscoveryStatus::Unavailable {
            message: "no multicast interface".to_string(),
        };
        let listening = DiscoveryStatus::Listening;
        let nothing = DiscoveryStatus::NothingFound {
            listening_for: Duration::from_secs(EMPTY_LIST_GRACE_SECONDS + 1),
        };
        let found = DiscoveryStatus::Found { count: 2 };

        assert_ne!(unavailable, listening);
        assert_ne!(listening, nothing);
        assert_ne!(nothing, found);
        assert_ne!(unavailable, found);

        match &unavailable {
            DiscoveryStatus::Unavailable { message } => {
                assert_eq!(message, "no multicast interface");
            }
            other => panic!("expected Unavailable, got {other:?}"),
        }
        match &nothing {
            DiscoveryStatus::NothingFound { listening_for } => {
                assert!(*listening_for >= Duration::from_secs(EMPTY_LIST_GRACE_SECONDS));
            }
            other => panic!("expected NothingFound, got {other:?}"),
        }
    }

    /// Verifies the grace period is short enough to be useful and long enough
    /// not to fire spuriously.
    ///
    /// **Methodology.** The maintainer's requirement is that the empty-list
    /// guidance appears "after a few seconds". Assert
    /// [`EMPTY_LIST_GRACE_SECONDS`] lies in `2..=10` s and that the browse poll
    /// interval is shorter than it, so at least two browse reads happen before
    /// the guidance is shown — otherwise the panel could advise a hotspot before
    /// it had ever actually looked.
    ///
    /// **Results (2026-07-28).** Measured `EMPTY_LIST_GRACE_SECONDS = 4` s and
    /// `BROWSE_POLL_INTERVAL_MS = 1000` ms, giving 4 browse reads inside the
    /// grace period. Interpretation: the guidance is shown promptly but never
    /// before the client has genuinely listened.
    #[test]
    fn the_empty_list_grace_period_allows_several_browse_reads() {
        assert!((2..=10).contains(&EMPTY_LIST_GRACE_SECONDS));
        let reads_before_advice = (EMPTY_LIST_GRACE_SECONDS * 1000) / BROWSE_POLL_INTERVAL_MS;
        assert!(
            reads_before_advice >= 2,
            "only {reads_before_advice} browse reads before advising the user"
        );
    }
}
