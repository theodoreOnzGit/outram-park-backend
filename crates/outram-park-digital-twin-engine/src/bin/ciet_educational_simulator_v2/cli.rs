//! Command-line options for the CIET Educational Simulator v2.
//!
//! Hand-rolled over `std::env::args` rather than `clap`: there are five flags,
//! and adding a CLI-parsing dependency to a GUI binary for that is not worth the
//! compile time. **v2 addition** — v1 took no arguments at all.

use outram_park_digital_twin_engine::ciet_opcua::node_map::DEFAULT_OPCUA_PORT;

/// Everything the user can ask for on the command line.
///
/// Build one with [`CliOptions::parse`]; print [`HELP_TEXT`] when
/// [`CliOptions::help`] is set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOptions {
    /// Address the OPC-UA server binds to. `"0.0.0.0"` (the default) means all
    /// interfaces, so other machines on the network can connect;
    /// `"127.0.0.1"` restricts it to this machine.
    pub bind_address: String,
    /// TCP port for the OPC-UA endpoint. Default
    /// [`DEFAULT_OPCUA_PORT`] (4840), the IANA-registered OPC-UA port.
    pub port: u16,
    /// Announce the endpoint over mDNS/DNS-SD so the bundled client can find it
    /// without being told an address. Cleared by `--no-advertise`.
    pub advertise_over_mdns: bool,
    /// Run with no GUI: physics plus OPC-UA plus a periodic stdout status line.
    /// Always the behaviour on Android/Termux, regardless of this flag.
    pub headless: bool,
    /// Restore v1's per-timestep `dbg!` dump of loop temperatures. Off by
    /// default in v2 because in headless mode it buries the status line.
    pub verbose_temperatures: bool,
    /// The user asked for `--help`; print [`HELP_TEXT`] and exit 0.
    pub help: bool,
}

impl Default for CliOptions {
    /// Bind all interfaces on port 4840, advertise over mDNS, GUI on, quiet
    /// diagnostics.
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: DEFAULT_OPCUA_PORT,
            advertise_over_mdns: true,
            headless: false,
            verbose_temperatures: false,
            help: false,
        }
    }
}

/// What `--help` prints. Also the reference for what the flags mean.
pub const HELP_TEXT: &str = "\
CIET Educational Simulator v2 -- an OFFLINE educational simulator of the CIET
thermal-hydraulic facility, with an embedded OPC-UA (IEC 62541) server.

USAGE:
    ciet_educational_simulator_v2 [OPTIONS]

OPTIONS:
    --bind <ADDR>            Address the OPC-UA server binds to.
                             Default: 0.0.0.0 (all interfaces -- reachable from
                             other machines on the same network).
                             Use 127.0.0.1 to keep it on this machine only.
    --port <N>               TCP port for the OPC-UA endpoint. Default: 4840.
    --no-advertise           Do not announce the endpoint over mDNS/DNS-SD.
                             The bundled `ciet_v2_opcua_client` finds the
                             simulator via mDNS, so with this flag you must type
                             the endpoint URL into a client by hand.
    --headless               Run with no GUI: physics + OPC-UA + a periodic
                             one-line status on stdout. This is always the
                             behaviour on Android/Termux, which has no
                             windowing stack.
    --verbose-temperatures   Print the full loop temperature diagnostics every
                             timestep (v1's behaviour). Very noisy; it will bury
                             the headless status line.
    --help                   Print this help and exit.

SECURITY -- READ THIS
    The OPC-UA server runs with security policy None and anonymous access.
    There is NO authentication and NO encryption. Bound to 0.0.0.0, anyone who
    can reach this machine on the network can read every value AND write every
    control -- heater power, pump pressure, set points, valves. Do not run it on
    untrusted or public WiFi.

SCOPE
    Educational demonstration only. Never connect this to live operational
    systems, plant systems, safety-critical infrastructure, real-time plant
    monitoring, or institutional production systems. Its outputs are not
    authoritative for any operational, licensing or safety purpose.
";

/// Why a command line could not be parsed.
///
/// Displayed to the user followed by a pointer at `--help`; the binary then
/// exits with status 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// A flag that takes a value was given none, e.g. a trailing `--port`.
    MissingValue {
        /// The flag that was missing its value.
        flag: String,
    },
    /// A flag's value could not be parsed, e.g. `--port banana`.
    BadValue {
        /// The flag whose value was rejected.
        flag: String,
        /// What the user actually wrote.
        value: String,
    },
    /// An argument that is not one of the recognised flags.
    UnknownArgument {
        /// The unrecognised argument, verbatim.
        argument: String,
    },
}

impl core::fmt::Display for CliError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingValue { flag } => write!(f, "`{flag}` needs a value"),
            Self::BadValue { flag, value } => {
                write!(f, "`{flag}` got an unusable value: `{value}`")
            }
            Self::UnknownArgument { argument } => write!(f, "unknown argument: `{argument}`"),
        }
    }
}

impl CliOptions {
    /// Parse an argument list, which must **exclude** the program name.
    ///
    /// Typical use: `CliOptions::parse(std::env::args().skip(1))`.
    ///
    /// Recognises `--bind <addr>`, `--port <n>`, `--no-advertise`,
    /// `--headless`, `--verbose-temperatures`, `--help` / `-h`. Anything else is
    /// a [`CliError::UnknownArgument`] rather than being silently ignored, so a
    /// typo in a flag name cannot quietly leave the server on all interfaces
    /// when the user meant to restrict it.
    pub fn parse<I>(arguments: I) -> Result<Self, CliError>
    where
        I: IntoIterator<Item = String>,
    {
        let mut options = Self::default();
        let mut arguments = arguments.into_iter();

        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--bind" => {
                    options.bind_address = arguments.next().ok_or(CliError::MissingValue {
                        flag: "--bind".to_string(),
                    })?;
                }
                "--port" => {
                    let raw = arguments.next().ok_or(CliError::MissingValue {
                        flag: "--port".to_string(),
                    })?;
                    options.port = raw.parse::<u16>().map_err(|_| CliError::BadValue {
                        flag: "--port".to_string(),
                        value: raw.clone(),
                    })?;
                }
                "--no-advertise" => options.advertise_over_mdns = false,
                "--headless" => options.headless = true,
                "--verbose-temperatures" => options.verbose_temperatures = true,
                "--help" | "-h" => options.help = true,
                other => {
                    return Err(CliError::UnknownArgument {
                        argument: other.to_string(),
                    })
                }
            }
        }

        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_string).collect()
    }

    /// Verification that the defaults are the documented ones: all interfaces,
    /// port 4840, mDNS on, GUI on, quiet.
    ///
    /// **Methodology.** Parse an empty argument list and compare every field
    /// against the values stated in `HELP_TEXT`. Pass criterion: exact equality.
    ///
    /// **Results (2026-07-28).** `bind_address == "0.0.0.0"`, `port == 4840`,
    /// `advertise_over_mdns == true`, `headless == false`,
    /// `verbose_temperatures == false`. Matches the documented defaults.
    #[test]
    fn empty_command_line_gives_documented_defaults() {
        let options = CliOptions::parse(Vec::<String>::new()).unwrap();
        assert_eq!(options.bind_address, "0.0.0.0");
        assert_eq!(options.port, 4840);
        assert!(options.advertise_over_mdns);
        assert!(!options.headless);
        assert!(!options.verbose_temperatures);
        assert!(!options.help);
    }

    /// Verification that every documented flag is honoured, including the
    /// value-taking ones.
    ///
    /// **Methodology.** Parse a command line exercising all six flags and check
    /// each resulting field. Pass criterion: exact equality on all fields.
    ///
    /// **Results (2026-07-28).** All six flags took effect as documented.
    #[test]
    fn every_flag_is_honoured() {
        let options = CliOptions::parse(words(
            "--bind 127.0.0.1 --port 14840 --no-advertise --headless \
             --verbose-temperatures --help",
        ))
        .unwrap();
        assert_eq!(options.bind_address, "127.0.0.1");
        assert_eq!(options.port, 14840);
        assert!(!options.advertise_over_mdns);
        assert!(options.headless);
        assert!(options.verbose_temperatures);
        assert!(options.help);
    }

    /// A mistyped flag must be an error, not a silent no-op.
    ///
    /// **Methodology.** Parse `--bnid 127.0.0.1` (a plausible typo for
    /// `--bind`), a trailing `--port` with no value, and `--port banana`. Pass
    /// criterion: each returns the matching [`CliError`] variant.
    ///
    /// **Results (2026-07-28).** `UnknownArgument`, `MissingValue` and
    /// `BadValue` respectively. This matters because silently ignoring
    /// `--bnid 127.0.0.1` would leave the unauthenticated server listening on
    /// every interface when the user asked for loopback only.
    #[test]
    fn bad_command_lines_are_rejected() {
        assert_eq!(
            CliOptions::parse(words("--bnid 127.0.0.1")),
            Err(CliError::UnknownArgument {
                argument: "--bnid".to_string()
            })
        );
        assert_eq!(
            CliOptions::parse(words("--port")),
            Err(CliError::MissingValue {
                flag: "--port".to_string()
            })
        );
        assert_eq!(
            CliOptions::parse(words("--port banana")),
            Err(CliError::BadValue {
                flag: "--port".to_string(),
                value: "banana".to_string()
            })
        );
    }
}
