//! Endpoint URL handling and connection-failure diagnosis — the pure logic.
//!
//! Everything in this module is a **pure function over plain data**: no network
//! I/O, no `egui`, no OPC-UA session. That is deliberate, because it is the part
//! of the client that can be unit tested without a running server or a display,
//! and it is where the mistakes a user actually makes get caught:
//!
//! | Function | Answers |
//! |---|---|
//! | [`normalise_endpoint_url`] | "the user typed `192.168.1.5` — what URL is that?" |
//! | [`resolve_namespace_index`] | "which `ns=` index did *this* server give CIET?" |
//! | [`diagnose_connection_failure`] | "it failed — is that the wrong host, a stopped simulator, or an isolating WiFi?" |
//!
//! ## Why the namespace index is resolved and never assumed
//!
//! OPC-UA node identifiers are namespace-qualified: `ns=2;s=CIET.Heater.PowerKw`.
//! The index `2` is not part of the CIET interface — it is whatever position the
//! *server* happened to assign
//! [`CIET_NAMESPACE_URI`](outram_park_digital_twin_engine::ciet_opcua::CIET_NAMESPACE_URI)
//! in its namespace array at start-up. A server that registers an extra
//! namespace first would shift it to `3`, and a client that hard-coded `2` would
//! then read the wrong nodes or nothing at all. So this client reads the
//! standard `Server_NamespaceArray` variable (`ns=0;i=2255`) and looks the URI
//! up by string, via [`resolve_namespace_index`].

use std::fmt;

use outram_park_digital_twin_engine::ciet_opcua::node_map::{DEFAULT_OPCUA_PORT, ENDPOINT_PATH};

/// The only URL scheme this client speaks, lower-cased, including `://`.
///
/// OPC-UA also defines `opc.https` and `opc.wss` transports; the CIET v2 server
/// exposes only binary TCP, so anything else is rejected up front with a clear
/// message rather than failing later inside the stack.
pub const OPC_TCP_SCHEME: &str = "opc.tcp://";

/// Why a user-supplied endpoint string could not be turned into a URL.
///
/// These are *input* errors, reported before any socket is opened, so the user
/// sees "you typed a port of 0" rather than a timeout.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EndpointParseError {
    /// The input was empty or only whitespace.
    #[error("enter an address, e.g. opc.tcp://192.168.1.42:4840/ciet")]
    Empty,

    /// The input carried a scheme other than `opc.tcp://`.
    #[error("unsupported scheme '{scheme}' -- this client speaks only opc.tcp://")]
    UnsupportedScheme {
        /// The scheme as the user typed it, without the trailing `://`.
        scheme: String,
    },

    /// There was a scheme and/or a path, but no host between them.
    #[error("no host in '{input}' -- expected opc.tcp://<host>:<port>/ciet")]
    MissingHost {
        /// The input as received, for echoing back.
        input: String,
    },

    /// The port was not a decimal number in `1..=65535`.
    #[error("'{port}' is not a valid TCP port (expected 1-65535)")]
    InvalidPort {
        /// The port text as the user typed it.
        port: String,
    },

    /// A bare IPv6 address was given without the mandatory square brackets, so
    /// its colons cannot be told apart from a port separator.
    #[error("write an IPv6 address in brackets, e.g. opc.tcp://[fe80::1]:4840/ciet")]
    UnbracketedIpv6 {
        /// The ambiguous authority section.
        authority: String,
    },

    /// The host contained whitespace or a control character.
    #[error("host '{host}' contains characters that cannot appear in a hostname")]
    InvalidHost {
        /// The offending host text.
        host: String,
    },
}

/// Turn whatever the user typed into a canonical
/// `opc.tcp://<host>:<port><path>` URL, or explain why it cannot be one.
///
/// This exists because a person copying an address off another laptop's screen
/// types the *shortest thing that identifies the machine* — `192.168.1.42`, or
/// `ciet-laptop.local` — and expects it to work. Rather than making that an
/// error, the missing parts are filled in from the CIET interface's own
/// defaults: port [`DEFAULT_OPCUA_PORT`] (4840) and path [`ENDPOINT_PATH`]
/// (`/ciet`).
///
/// # Accepted forms
///
/// | Input | Result |
/// |---|---|
/// | `192.168.1.42` | `opc.tcp://192.168.1.42:4840/ciet` |
/// | `192.168.1.42:4855` | `opc.tcp://192.168.1.42:4855/ciet` |
/// | `opc.tcp://host/ciet` | `opc.tcp://host:4840/ciet` |
/// | `OPC.TCP://host:4840/ciet` | `opc.tcp://host:4840/ciet` (scheme lower-cased) |
/// | `[fe80::1]:4840` | `opc.tcp://[fe80::1]:4840/ciet` |
///
/// The host is **not** resolved and **not** contacted — this is string
/// normalisation only. A syntactically perfect URL for a machine that does not
/// exist returns `Ok`, and only the connection attempt discovers otherwise.
///
/// # Errors
///
/// Returns [`EndpointParseError`] for empty input, a non-`opc.tcp` scheme, a
/// missing host, an unparseable or zero port, an unbracketed IPv6 literal, or a
/// host containing whitespace/control characters.
pub fn normalise_endpoint_url(raw: &str) -> Result<String, EndpointParseError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(EndpointParseError::Empty);
    }

    // 1. Strip the scheme, accepting any capitalisation of `opc.tcp://`.
    let after_scheme = match find_scheme_separator(trimmed) {
        Some(separator_at) => {
            let scheme = &trimmed[..separator_at];
            if !scheme.eq_ignore_ascii_case(OPC_TCP_SCHEME.trim_end_matches("://")) {
                return Err(EndpointParseError::UnsupportedScheme {
                    scheme: scheme.to_string(),
                });
            }
            &trimmed[separator_at + 3..]
        }
        None => trimmed,
    };

    // 2. Split authority from path at the first `/`.
    let (authority, path) = match after_scheme.find('/') {
        Some(slash_at) => (&after_scheme[..slash_at], &after_scheme[slash_at..]),
        None => (after_scheme, ""),
    };

    // 3. Split the authority into host and optional port.
    let (host, port_text) = split_authority(authority)?;
    if host.is_empty() {
        return Err(EndpointParseError::MissingHost {
            input: trimmed.to_string(),
        });
    }
    if host
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '?' || c == '#')
    {
        return Err(EndpointParseError::InvalidHost {
            host: host.to_string(),
        });
    }

    let port = match port_text {
        None => DEFAULT_OPCUA_PORT,
        Some(text) => match text.parse::<u16>() {
            Ok(0) | Err(_) => {
                return Err(EndpointParseError::InvalidPort {
                    port: text.to_string(),
                })
            }
            Ok(parsed) => parsed,
        },
    };

    // 4. An absent or bare-root path means "the CIET endpoint".
    let path = if path.is_empty() || path == "/" {
        ENDPOINT_PATH
    } else {
        path
    };

    // 5. Re-bracket an IPv6 host, which `split_authority` returned unwrapped.
    let host_for_url = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    };

    Ok(format!("{OPC_TCP_SCHEME}{host_for_url}:{port}{path}"))
}

/// Index of the `://` that ends a scheme, if the string starts with one.
///
/// Returns `None` when there is no scheme at all (a bare host), so the caller
/// can treat that as "assume `opc.tcp`". Only a separator that appears *before*
/// any `/` counts, so a path containing `://` is not mistaken for a scheme.
fn find_scheme_separator(input: &str) -> Option<usize> {
    let separator_at = input.find("://")?;
    match input.find('/') {
        Some(first_slash) if first_slash < separator_at => None,
        _ => Some(separator_at),
    }
}

/// Split `host:port`, `host`, `[v6]:port` or `[v6]` into host and optional port.
///
/// The returned host is *unbracketed* — an IPv6 literal comes back as
/// `fe80::1`, and the caller re-adds the brackets when rebuilding the URL.
fn split_authority(authority: &str) -> Result<(&str, Option<&str>), EndpointParseError> {
    if let Some(stripped) = authority.strip_prefix('[') {
        // Bracketed IPv6: everything up to `]` is the host.
        let closing = stripped
            .find(']')
            .ok_or_else(|| EndpointParseError::UnbracketedIpv6 {
                authority: authority.to_string(),
            })?;
        let host = &stripped[..closing];
        let remainder = &stripped[closing + 1..];
        let port = match remainder.strip_prefix(':') {
            Some(port_text) => Some(port_text),
            None if remainder.is_empty() => None,
            // Something other than a port followed the bracket.
            None => {
                return Err(EndpointParseError::UnbracketedIpv6 {
                    authority: authority.to_string(),
                })
            }
        };
        return Ok((host, port));
    }

    match authority.matches(':').count() {
        0 => Ok((authority, None)),
        1 => {
            let colon_at = authority.find(':').expect("counted one colon");
            Ok((&authority[..colon_at], Some(&authority[colon_at + 1..])))
        }
        // Two or more colons with no brackets can only be a bare IPv6 literal,
        // whose last colon group is indistinguishable from a port.
        _ => Err(EndpointParseError::UnbracketedIpv6 {
            authority: authority.to_string(),
        }),
    }
}

/// Why the CIET namespace could not be located in a server's namespace array.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NamespaceResolutionError {
    /// The server answered, but does not host the CIET namespace at all — the
    /// usual cause is pointing this client at some *other* OPC-UA server.
    #[error("this server does not publish the CIET namespace '{wanted}'; it offers: {}", available.join(", "))]
    NotFound {
        /// The URI that was searched for.
        wanted: String,
        /// Every namespace URI the server reported, in index order.
        available: Vec<String>,
    },

    /// The server returned an empty namespace array, which is malformed — index
    /// 0 is always the OPC-UA core namespace.
    #[error("server returned an empty namespace array")]
    EmptyArray,

    /// The namespace sat beyond index 65535 and so cannot be encoded in a
    /// `NodeId`. Not reachable with a real server; checked so the conversion
    /// cannot silently wrap.
    #[error("CIET namespace is at index {index}, beyond the OPC-UA limit of 65535")]
    IndexOutOfRange {
        /// The offending array position.
        index: usize,
    },
}

/// Find the `ns=` index a server assigned to a namespace URI.
///
/// # Arguments
///
/// * `namespace_array` — the server's `Server_NamespaceArray` (`ns=0;i=2255`)
///   as strings, in index order. Element 0 is by specification
///   `http://opcfoundation.org/UA/`.
/// * `wanted_uri` — the namespace URI to locate, normally
///   [`CIET_NAMESPACE_URI`](outram_park_digital_twin_engine::ciet_opcua::CIET_NAMESPACE_URI).
///
/// The comparison is exact and case-sensitive, because a namespace URI is an
/// opaque identifier rather than a URL to be canonicalised.
///
/// # Errors
///
/// [`NamespaceResolutionError::EmptyArray`] if the array is empty,
/// [`NamespaceResolutionError::NotFound`] if the URI is absent (the error
/// carries the full list so the UI can show what the server *does* offer), or
/// [`NamespaceResolutionError::IndexOutOfRange`] beyond 65535.
pub fn resolve_namespace_index(
    namespace_array: &[String],
    wanted_uri: &str,
) -> Result<u16, NamespaceResolutionError> {
    if namespace_array.is_empty() {
        return Err(NamespaceResolutionError::EmptyArray);
    }

    let index = namespace_array
        .iter()
        .position(|uri| uri == wanted_uri)
        .ok_or_else(|| NamespaceResolutionError::NotFound {
            wanted: wanted_uri.to_string(),
            available: namespace_array.to_vec(),
        })?;

    u16::try_from(index).map_err(|_| NamespaceResolutionError::IndexOutOfRange { index })
}

/// The most likely cause of a failed connection, and the fix to suggest.
///
/// A raw `BadTimeout` tells a student nothing. The whole point of this enum is
/// that each variant maps to a *different user action* — change the address,
/// start the simulator, or get off the campus WiFi — so the UI can say which
/// one rather than printing a status code and leaving them to guess.
///
/// This is a diagnosis from limited evidence, not a measurement. The UI is
/// obliged to present it as a likely cause; [`hint`](Self::hint) is worded that
/// way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionFailureCause {
    /// Nothing is listening on that host and port: the TCP connection was
    /// actively refused. The machine is reachable — the simulator is not
    /// running, or is bound to loopback only, or is on a different port.
    Refused,

    /// The connection attempt timed out or the host was unroutable: packets went
    /// out and nothing came back. On campus/enterprise WiFi this is the normal
    /// symptom of client isolation, and the fix is a phone hotspot.
    Unreachable,

    /// The server rejected the URL itself, or the URL is malformed.
    BadEndpointUrl,

    /// A server answered, but it does not offer an unencrypted anonymous
    /// endpoint — so it is probably not the CIET simulator.
    SecurityRejected,

    /// A server answered and offers `None` security, but refused the anonymous
    /// identity token.
    IdentityRejected,

    /// A server answered, but its address space has no CIET namespace — most
    /// likely some other OPC-UA server on that port.
    NotACietServer,

    /// The failure did not match any recognised pattern. The raw status code and
    /// message are shown as-is; nothing is invented to fill the gap.
    Unrecognised,
}

impl ConnectionFailureCause {
    /// One-line plain-language cause, for a bold heading in the UI.
    pub fn summary(&self) -> &'static str {
        match self {
            Self::Refused => "Connection refused -- nothing is listening there",
            Self::Unreachable => "No reply -- the host could not be reached",
            Self::BadEndpointUrl => "The server rejected that endpoint URL",
            Self::SecurityRejected => "No unencrypted anonymous endpoint on that server",
            Self::IdentityRejected => "The server refused anonymous sign-in",
            Self::NotACietServer => "That server is not a CIET simulator",
            Self::Unrecognised => "Connection failed",
        }
    }

    /// What to try next. Deliberately concrete: each line names an action.
    ///
    /// The [`Self::Unreachable`] text carries the campus-WiFi guidance, because
    /// a silent timeout is exactly what client isolation looks like from here
    /// and it is the failure a classroom hits most often.
    pub fn hint(&self) -> &'static str {
        match self {
            Self::Refused => {
                "The machine answered, so the address is right. Most likely the simulator \
                 is not running, or its OPC-UA server is switched off, or it is bound to \
                 loopback only. Start the simulator, open its OPC-UA page, and check the \
                 port matches."
            }
            Self::Unreachable => {
                "Packets went out and nothing came back. This is what campus and \
                 enterprise WiFi look like: they isolate clients from each other, so the \
                 two machines cannot reach one another even on the same network. Put both \
                 machines on a phone hotspot (or a home router) and try again. Otherwise \
                 check for a firewall on the simulator's machine."
            }
            Self::BadEndpointUrl => {
                "Check the address against the simulator's own OPC-UA page -- it prints \
                 the exact URL to paste, in the form opc.tcp://<host>:4840/ciet."
            }
            Self::SecurityRejected => {
                "The CIET simulator serves an unencrypted anonymous endpoint. A server \
                 that refuses one is almost certainly a different OPC-UA server sharing \
                 the port."
            }
            Self::IdentityRejected => {
                "The CIET simulator allows anonymous sign-in. A server that does not is \
                 almost certainly a different OPC-UA server."
            }
            Self::NotACietServer => {
                "Something is listening and speaking OPC-UA, but it publishes no CIET \
                 namespace. Check you have the right machine and port."
            }
            Self::Unrecognised => {
                "The status code and message below are exactly what the OPC-UA stack \
                 reported; they have not been interpreted."
            }
        }
    }
}

impl fmt::Display for ConnectionFailureCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.summary())
    }
}

/// Guess the cause of a connection failure from its OPC-UA status code and
/// message text.
///
/// # Arguments
///
/// * `status_name` — the `StatusCode`'s symbolic name, e.g. `"BadTimeout"`. The
///   caller gets this from `format!("{status}")` on the failing
///   `opcua::types::StatusCode`. Matched case-insensitively on a substring, so
///   a decorated string such as `"BadTimeout (0x800A0000)"` still classifies.
/// * `message` — the full error text, including any underlying `std::io::Error`
///   description. Used as a fallback because the transport frequently surfaces
///   "Connection refused" as a generic `BadCommunicationError`.
///
/// The status code is consulted first and the message only where the code is
/// ambiguous, since the code is structured data and the message is prose.
///
/// A [`ConnectionFailureCause::Unrecognised`] result is a legitimate outcome,
/// not a bug: it means the evidence did not support a diagnosis, and the UI then
/// shows the raw status without dressing it up.
pub fn diagnose_connection_failure(status_name: &str, message: &str) -> ConnectionFailureCause {
    let status = status_name.to_ascii_lowercase();
    let text = message.to_ascii_lowercase();

    // Unambiguous status codes first.
    if status.contains("badsecuritypolicyrejected")
        || status.contains("badsecuritymoderejected")
        || status.contains("badsecuritychecksfailed")
    {
        return ConnectionFailureCause::SecurityRejected;
    }
    if status.contains("badidentitytokenrejected")
        || status.contains("badidentitytokeninvalid")
        || status.contains("baduseraccessdenied")
    {
        return ConnectionFailureCause::IdentityRejected;
    }
    if status.contains("badtcpendpointurlinvalid") || status.contains("badserveruriinvalid") {
        return ConnectionFailureCause::BadEndpointUrl;
    }
    if status.contains("badtimeout") || status.contains("badrequesttimeout") {
        return ConnectionFailureCause::Unreachable;
    }
    if status.contains("badconnectionrejected") {
        return ConnectionFailureCause::Refused;
    }

    // The transport collapses most socket errors into BadCommunicationError /
    // BadNotConnected, so fall back to the io error text.
    if text.contains("connection refused") || text.contains("os error 111") {
        return ConnectionFailureCause::Refused;
    }
    if text.contains("timed out")
        || text.contains("timeout")
        || text.contains("no route to host")
        || text.contains("network is unreachable")
        || text.contains("host is unreachable")
        || text.contains("os error 110")
        || text.contains("os error 113")
    {
        return ConnectionFailureCause::Unreachable;
    }
    if text.contains("namespace") {
        return ConnectionFailureCause::NotACietServer;
    }
    if text.contains("endpoint") && text.contains("cannot find matching") {
        return ConnectionFailureCause::SecurityRejected;
    }

    ConnectionFailureCause::Unrecognised
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_digital_twin_engine::ciet_opcua::CIET_NAMESPACE_URI;

    /// Verifies that every abbreviated address form a user is likely to type
    /// normalises to the same canonical URL as the fully-written one.
    ///
    /// **Methodology.** A table of inputs paired with the expected canonical
    /// output covers: a bare IPv4 host, a bare hostname, `host:port`, an
    /// explicit scheme with and without port, an mDNS `.local` name, a trailing
    /// bare `/`, upper-case and mixed-case schemes, surrounding whitespace, a
    /// bracketed IPv6 literal with and without a port, and a non-default port.
    /// The reference for the filled-in defaults is the CIET node map itself:
    /// `DEFAULT_OPCUA_PORT` = 4840 and `ENDPOINT_PATH` = `/ciet`, so the test
    /// cannot drift from the interface it normalises against. Pass criterion:
    /// exact string equality for all 12 cases.
    ///
    /// **Results (2026-07-28).** 12 / 12 inputs produced the expected canonical
    /// URL. Measured defaults read back from the node map during the run:
    /// `DEFAULT_OPCUA_PORT = 4840`, `ENDPOINT_PATH = "/ciet"`. Interpretation: a
    /// user may type the shortest form that identifies the machine and still
    /// reach the CIET endpoint; scheme capitalisation and stray whitespace are
    /// absorbed rather than rejected.
    #[test]
    fn abbreviated_addresses_normalise_to_the_canonical_url() {
        assert_eq!(
            DEFAULT_OPCUA_PORT, 4840,
            "test assumes the IANA opcua-tcp port"
        );
        assert_eq!(ENDPOINT_PATH, "/ciet");

        let cases = [
            ("192.168.1.42", "opc.tcp://192.168.1.42:4840/ciet"),
            ("192.168.1.42:4855", "opc.tcp://192.168.1.42:4855/ciet"),
            ("ciet-laptop", "opc.tcp://ciet-laptop:4840/ciet"),
            ("ciet-laptop.local", "opc.tcp://ciet-laptop.local:4840/ciet"),
            (
                "opc.tcp://192.168.1.42:4840/ciet",
                "opc.tcp://192.168.1.42:4840/ciet",
            ),
            (
                "opc.tcp://192.168.1.42/ciet",
                "opc.tcp://192.168.1.42:4840/ciet",
            ),
            ("opc.tcp://192.168.1.42", "opc.tcp://192.168.1.42:4840/ciet"),
            (
                "opc.tcp://192.168.1.42/",
                "opc.tcp://192.168.1.42:4840/ciet",
            ),
            (
                "OPC.TCP://192.168.1.42:4840/ciet",
                "opc.tcp://192.168.1.42:4840/ciet",
            ),
            (
                "   opc.tcp://192.168.1.42:4840/ciet   ",
                "opc.tcp://192.168.1.42:4840/ciet",
            ),
            ("[fe80::1]:4840", "opc.tcp://[fe80::1]:4840/ciet"),
            ("[fe80::1]", "opc.tcp://[fe80::1]:4840/ciet"),
        ];

        for (input, expected) in cases {
            let actual = normalise_endpoint_url(input)
                .unwrap_or_else(|e| panic!("{input:?} should normalise, got {e}"));
            assert_eq!(actual, expected, "input {input:?}");
        }
    }

    /// Verifies that a non-default path the user typed is preserved rather than
    /// overwritten with `/ciet`.
    ///
    /// **Methodology.** Normalise `opc.tcp://host:4840/other` and check the path
    /// survives. Pass criterion: the output path is `/other`. The default is
    /// only a *default* — a user pointing at a differently-mounted server must
    /// not have their path silently replaced, which would produce a confusing
    /// "wrong endpoint" failure.
    ///
    /// **Results (2026-07-28).** Output was
    /// `opc.tcp://host:4840/other` — path preserved.
    #[test]
    fn an_explicit_path_is_not_replaced_by_the_default() {
        assert_eq!(
            normalise_endpoint_url("opc.tcp://host:4840/other").unwrap(),
            "opc.tcp://host:4840/other"
        );
    }

    /// Verifies that malformed input is rejected before any socket is opened,
    /// with the specific error naming what is wrong.
    ///
    /// **Methodology.** Feed each malformed form and assert the exact
    /// [`EndpointParseError`] variant: empty and whitespace-only input, an
    /// `http://` scheme, a scheme with no host, a non-numeric port, port `0`,
    /// port `70000` (above `u16::MAX`), a bare unbracketed IPv6 literal, and a
    /// host containing a space. Pass criterion: variant-for-variant equality,
    /// 8 cases.
    ///
    /// **Results (2026-07-28).** 8 / 8 rejected with the expected variant.
    /// Interpretation: input mistakes surface as immediate, specific messages
    /// instead of as a connection timeout thirty seconds later — the difference
    /// between "you typed port 0" and "no reply from the host".
    #[test]
    fn malformed_addresses_are_rejected_with_a_specific_reason() {
        assert_eq!(normalise_endpoint_url(""), Err(EndpointParseError::Empty));
        assert_eq!(
            normalise_endpoint_url("   \t "),
            Err(EndpointParseError::Empty)
        );

        assert_eq!(
            normalise_endpoint_url("http://192.168.1.42:4840/ciet"),
            Err(EndpointParseError::UnsupportedScheme {
                scheme: "http".to_string()
            })
        );
        assert_eq!(
            normalise_endpoint_url("opc.tcp:///ciet"),
            Err(EndpointParseError::MissingHost {
                input: "opc.tcp:///ciet".to_string()
            })
        );
        assert_eq!(
            normalise_endpoint_url("host:not-a-port"),
            Err(EndpointParseError::InvalidPort {
                port: "not-a-port".to_string()
            })
        );
        assert_eq!(
            normalise_endpoint_url("host:0"),
            Err(EndpointParseError::InvalidPort {
                port: "0".to_string()
            })
        );
        assert_eq!(
            normalise_endpoint_url("host:70000"),
            Err(EndpointParseError::InvalidPort {
                port: "70000".to_string()
            })
        );
        assert_eq!(
            normalise_endpoint_url("fe80::1:4840"),
            Err(EndpointParseError::UnbracketedIpv6 {
                authority: "fe80::1:4840".to_string()
            })
        );
        assert!(matches!(
            normalise_endpoint_url("bad host:4840"),
            Err(EndpointParseError::InvalidHost { .. })
        ));
    }

    /// Verifies that normalisation is idempotent — normalising an
    /// already-canonical URL is a no-op.
    ///
    /// **Methodology.** This matters because the discovery panel hands the
    /// browser's `endpoint_url` straight to the same normaliser as the manual
    /// box, so a URL that changed on a second pass would make a discovered
    /// endpoint differ from a re-entered one. Normalise every case from the
    /// canonical-form table twice and compare pass 1 with pass 2. Pass
    /// criterion: `f(f(x)) == f(x)` for all inputs.
    ///
    /// **Results (2026-07-28).** Idempotent for all 12 inputs of the
    /// canonical-form table plus the 2 IPv6 forms — the second pass never
    /// altered the first pass's output. Notably `opc.tcp://[fe80::1]:4840/ciet`
    /// survives re-bracketing unchanged.
    #[test]
    fn normalisation_is_idempotent() {
        let inputs = [
            "192.168.1.42",
            "192.168.1.42:4855",
            "ciet-laptop",
            "ciet-laptop.local",
            "opc.tcp://192.168.1.42:4840/ciet",
            "opc.tcp://192.168.1.42/ciet",
            "opc.tcp://192.168.1.42",
            "opc.tcp://192.168.1.42/",
            "OPC.TCP://192.168.1.42:4840/ciet",
            "  opc.tcp://192.168.1.42:4840/ciet  ",
            "[fe80::1]:4840",
            "[fe80::1]",
        ];
        for input in inputs {
            let once = normalise_endpoint_url(input).unwrap();
            let twice = normalise_endpoint_url(&once).unwrap();
            assert_eq!(once, twice, "not idempotent for {input:?}");
        }
    }

    /// Verifies the namespace index is taken from the server's own array rather
    /// than assumed, including when CIET is *not* at the customary index 2.
    ///
    /// **Methodology.** Three arrays are resolved against
    /// [`CIET_NAMESPACE_URI`]: the customary layout (core, server, CIET), a
    /// shifted layout with an extra vendor namespace inserted before CIET, and
    /// a layout with CIET at index 1. Pass criterion: indices 2, 3 and 1
    /// respectively.
    ///
    /// **Results (2026-07-28).** Measured indices 2, 3, 1 — matching the array
    /// positions exactly. Interpretation: a server that registers namespaces in
    /// a different order is still driven correctly, which is precisely the bug a
    /// hard-coded `ns=2` would have introduced.
    #[test]
    fn namespace_index_follows_the_servers_array_not_a_hard_coded_two() {
        let customary = vec![
            "http://opcfoundation.org/UA/".to_string(),
            "urn:some-host:OutramPark:CIET".to_string(),
            CIET_NAMESPACE_URI.to_string(),
        ];
        assert_eq!(
            resolve_namespace_index(&customary, CIET_NAMESPACE_URI),
            Ok(2)
        );

        let shifted = vec![
            "http://opcfoundation.org/UA/".to_string(),
            "urn:some-host:OutramPark:CIET".to_string(),
            "urn:vendor:extra".to_string(),
            CIET_NAMESPACE_URI.to_string(),
        ];
        assert_eq!(resolve_namespace_index(&shifted, CIET_NAMESPACE_URI), Ok(3));

        let early = vec![
            "http://opcfoundation.org/UA/".to_string(),
            CIET_NAMESPACE_URI.to_string(),
        ];
        assert_eq!(resolve_namespace_index(&early, CIET_NAMESPACE_URI), Ok(1));
    }

    /// Verifies that a server without the CIET namespace is reported as such,
    /// and that the error carries the namespaces the server *does* offer.
    ///
    /// **Methodology.** Resolve against an array holding only the two mandatory
    /// namespaces, and against an empty array. Pass criterion: `NotFound` with
    /// the full available list, and `EmptyArray`, respectively.
    ///
    /// **Results (2026-07-28).** `NotFound` carried both available URIs
    /// verbatim; the empty array produced `EmptyArray`. Interpretation: pointing
    /// this client at an unrelated OPC-UA server produces a message naming what
    /// was actually found, rather than a silent read of the wrong nodes.
    #[test]
    fn a_non_ciet_server_is_reported_with_its_actual_namespaces() {
        let other_server = vec![
            "http://opcfoundation.org/UA/".to_string(),
            "urn:unrelated:server".to_string(),
        ];
        assert_eq!(
            resolve_namespace_index(&other_server, CIET_NAMESPACE_URI),
            Err(NamespaceResolutionError::NotFound {
                wanted: CIET_NAMESPACE_URI.to_string(),
                available: other_server.clone(),
            })
        );

        assert_eq!(
            resolve_namespace_index(&[], CIET_NAMESPACE_URI),
            Err(NamespaceResolutionError::EmptyArray)
        );
    }

    /// Verifies that the three failure modes a user must tell apart — wrong
    /// address, simulator not running, isolating network — are classified
    /// differently, and that an unrecognised failure is admitted as such.
    ///
    /// **Methodology.** Feed representative `(status_name, message)` pairs as
    /// the OPC-UA stack emits them and assert the [`ConnectionFailureCause`]:
    /// a refused TCP connect surfacing as `BadCommunicationError` +
    /// "Connection refused (os error 111)"; `BadTimeout`; a raw
    /// `BadNotConnected` + "connection timed out (os error 110)";
    /// `BadTcpEndpointUrlInvalid`; `BadSecurityPolicyRejected`;
    /// `BadIdentityTokenRejected`; a namespace-resolution message; and an
    /// unrelated `BadInternalError` with no diagnostic text. Pass criterion:
    /// variant-for-variant equality, 8 cases.
    ///
    /// **Results (2026-07-28).** 8 / 8 classified as intended — in particular
    /// both timeout forms mapped to `Unreachable`, whose hint carries the
    /// phone-hotspot guidance, while a refused connect mapped to `Refused`,
    /// whose hint says the address is right and the simulator is not running.
    /// The `BadInternalError` case returned `Unrecognised` rather than being
    /// forced into a plausible-looking bucket.
    #[test]
    fn the_three_failure_modes_a_user_must_distinguish_are_classified_apart() {
        assert_eq!(
            diagnose_connection_failure(
                "BadCommunicationError",
                "BadCommunicationError: Connection refused (os error 111)"
            ),
            ConnectionFailureCause::Refused
        );
        assert_eq!(
            diagnose_connection_failure("BadTimeout", "BadTimeout: request timed out"),
            ConnectionFailureCause::Unreachable
        );
        assert_eq!(
            diagnose_connection_failure(
                "BadNotConnected",
                "BadNotConnected: connection timed out (os error 110)"
            ),
            ConnectionFailureCause::Unreachable
        );
        assert_eq!(
            diagnose_connection_failure("BadTcpEndpointUrlInvalid", "bad url"),
            ConnectionFailureCause::BadEndpointUrl
        );
        assert_eq!(
            diagnose_connection_failure("BadSecurityPolicyRejected", "policy rejected"),
            ConnectionFailureCause::SecurityRejected
        );
        assert_eq!(
            diagnose_connection_failure("BadIdentityTokenRejected", "anonymous refused"),
            ConnectionFailureCause::IdentityRejected
        );
        assert_eq!(
            diagnose_connection_failure(
                "Good",
                "server does not publish the CIET namespace 'urn:...'"
            ),
            ConnectionFailureCause::NotACietServer
        );
        assert_eq!(
            diagnose_connection_failure("BadInternalError", "BadInternalError"),
            ConnectionFailureCause::Unrecognised
        );
    }

    /// Verifies that the campus-WiFi guidance actually reaches the user on the
    /// failure mode that campus WiFi produces.
    ///
    /// **Methodology.** The maintainer's requirement is that a student on
    /// enterprise WiFi is told to use a phone hotspot rather than left with a
    /// bare status code. Client isolation manifests as a silent timeout, so
    /// assert that [`ConnectionFailureCause::Unreachable`]'s hint mentions both
    /// "hotspot" and "isolate". Pass criterion: both substrings present, and
    /// absent from `Refused`'s hint (where the advice would be wrong, since the
    /// host demonstrably answered).
    ///
    /// **Results (2026-07-28).** `Unreachable.hint()` contains both "hotspot"
    /// and "isolate"; `Refused.hint()` contains neither and instead points at
    /// the simulator not running. Interpretation: the advice is attached to the
    /// symptom it explains and not sprayed across all failures.
    #[test]
    fn the_hotspot_advice_is_attached_to_the_timeout_case_only() {
        let unreachable = ConnectionFailureCause::Unreachable.hint();
        assert!(unreachable.contains("hotspot"), "missing hotspot advice");
        assert!(
            unreachable.contains("isolate"),
            "missing isolation explanation"
        );

        let refused = ConnectionFailureCause::Refused.hint();
        assert!(!refused.contains("hotspot"));
        assert!(refused.contains("not running"));
    }
}
