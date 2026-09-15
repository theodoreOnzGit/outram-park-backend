//! Where the CIET v2 OPC-UA server keeps its PKI directory.
//!
//! The path resolution itself is reactor-agnostic and lives in
//! [`opcua_core::pki`](crate::opcua_core::pki); this module names CIET's
//! directory and binds the shared helpers to it, so callers never pass the
//! directory name by hand.
//!
//! `async-opcua` needs a writable directory to hold its **application instance
//! certificate**. Putting it under the user's home means the simulator never
//! scatters `./pki` folders into whatever working directory it happened to be
//! launched from.
//!
//! ## Layout
//!
//! | Platform | Root | PKI dir ([`ciet_v2_pki_dir`]) |
//! |---|---|---|
//! | Linux / macOS / Termux | `$HOME/.outram-park` | `$HOME/.outram-park/ciet-v2-opcua-pki` |
//! | Windows | `%APPDATA%\outram-park` | `%APPDATA%\outram-park\ciet-v2-opcua-pki` |
//!
//! Inside the PKI directory, `async-opcua` populates the usual OPC-UA
//! certificate-store subtree itself on first run:
//!
//! ```text
//! ciet-v2-opcua-pki/
//!   own/cert.der          <- this server's self-signed instance certificate
//!   private/private.pem   <- the matching private key
//!   trusted/              <- client certificates the server would trust
//!   rejected/             <- client certificates it has seen and refused
//! ```
//!
//! ## Nothing sensitive is stored here
//!
//! The CIET v2 server runs with `SecurityPolicy::None` and anonymous access
//! (see [`super::server`]). No channel is ever encrypted or signed with the
//! keypair, no client certificate is ever validated against the trust list, and
//! no user credential of any kind is written. What lands on disk is therefore a
//! **throwaway self-signed keypair** that authenticates nothing — deleting the
//! whole directory costs nothing but a regeneration on next start-up. Do not
//! describe it as a credential store, and do not reuse the key for anything.
//!
//! ## No credentials, ever (`RESPONSIBLE_USE.md`)
//!
//! This module names a directory and reports its path. It must never grow code
//! that reads institutional credentials, API keys, access tokens, or any
//! certificate belonging to a real facility or production system.
//!
//! ## Units
//!
//! Everything here is a filesystem path or a name. No physical quantities, no
//! units.

use std::path::PathBuf;

use crate::opcua_core::pki;

pub use crate::opcua_core::pki::{outram_park_home, outram_park_home_path, unique_instance_tag};

/// Directory name of the CIET v2 PKI store, relative to the OUTRAM PARK
/// per-user root ([`outram_park_home`]).
pub const CIET_V2_PKI_DIR_NAME: &str = "ciet-v2-opcua-pki";

/// The PKI directory for the CIET v2 OPC-UA server, created if it does not
/// exist.
///
/// This is `<`[`outram_park_home`]`>/ciet-v2-opcua-pki`. Pass it straight to
/// `ServerBuilder::pki_dir`; `async-opcua` creates the `own/`, `private/`,
/// `trusted/` and `rejected/` subdirectories itself and writes a self-signed
/// application instance certificate into `own/` on first start-up.
///
/// A creation failure is warned about rather than returned as an error, so a
/// read-only home directory cannot take the whole simulator down.
pub fn ciet_v2_pki_dir() -> PathBuf {
    pki::pki_dir(CIET_V2_PKI_DIR_NAME)
}

/// A per-instance PKI directory underneath [`ciet_v2_pki_dir`], created if
/// missing.
///
/// Returns `<`[`ciet_v2_pki_dir`]`>/<sanitised instance_tag>`.
///
/// ## Why this exists: parallel instances clobber a shared certificate store
///
/// `async-opcua` writes its self-signed keypair into the PKI directory on
/// start-up. Two servers starting **concurrently** against the same directory
/// race on `own/cert.der` and `private/private.pem`, and can read a
/// half-written file — a real hazard for the headless CIET tests, which may run
/// several simulators at once, and for a developer running the simulator while a
/// test suite runs.
///
/// The tag makes each instance's store disjoint. The shared server layer derives
/// it from the TCP port, which is the one thing two servers that can coexist on
/// a machine must differ in, so isolation is automatic and needs no
/// configuration. Tests that want a stronger guarantee — a fresh directory per
/// run rather than per port — can pass [`unique_instance_tag`].
///
/// `instance_tag` is sanitised to ASCII alphanumerics, `-` and `_`; anything
/// else becomes `-`, and an empty result becomes `"default"`. That keeps a
/// caller from escaping the directory with `../` or breaking on a path
/// separator.
pub fn ciet_v2_instance_pki_dir(instance_tag: &str) -> PathBuf {
    pki::instance_pki_dir(CIET_V2_PKI_DIR_NAME, instance_tag)
}

/// A one-line, human-readable summary of where the PKI directory is, for the
/// simulator's "how to connect" panel and its start-up log line.
///
/// The wording deliberately states that nothing sensitive is stored, so a
/// reader of the GUI is not misled into thinking the interface is secured.
///
/// # Example output
///
/// ```text
/// PKI directory: /home/alice/.outram-park/ciet-v2-opcua-pki (self-signed keypair only -- SecurityPolicy::None stores no credentials)
/// ```
pub fn describe_pki_location() -> String {
    pki::describe_pki_location(CIET_V2_PKI_DIR_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that the PKI path ends in the two components this platform's
    /// layout rule specifies, so a user can find it from the documentation
    /// alone.
    ///
    /// **Methodology.** Call [`outram_park_home_path`] (the non-creating half of
    /// [`outram_park_home`]) and append the PKI directory name, then compare
    /// the final two path components against the platform rule: on Windows
    /// `outram-park/ciet-v2-opcua-pki`, everywhere else
    /// `.outram-park/ciet-v2-opcua-pki`. Pass criterion: both components match
    /// exactly. No directory is created, so the test is safe on a read-only
    /// home.
    ///
    /// **Results (2026-07-28, unchanged 2026-08-12 after the shared-layer
    /// extraction; Linux x86_64, `directories` 5.0.1).** The resolved path was
    /// `/home/<user>/.outram-park/ciet-v2-opcua-pki`: last component
    /// `ciet-v2-opcua-pki`, parent component `.outram-park`. Interpretation: the
    /// documented layout table matches what the code produces on the
    /// maintainer's platform. The Windows branch is `cfg`-selected and was not
    /// exercised on this run.
    #[test]
    fn pki_path_ends_in_the_platform_components() {
        let path = outram_park_home_path().join(CIET_V2_PKI_DIR_NAME);

        let components: Vec<String> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        assert!(
            components.len() >= 2,
            "path {path:?} has too few components"
        );

        let last = &components[components.len() - 1];
        let parent = &components[components.len() - 2];

        assert_eq!(last, CIET_V2_PKI_DIR_NAME, "wrong PKI directory name");

        #[cfg(target_os = "windows")]
        assert_eq!(
            parent,
            crate::opcua_core::pki::WINDOWS_APPDATA_DIR_NAME,
            "wrong Windows root name"
        );
        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            parent,
            crate::opcua_core::pki::UNIX_HOME_DIR_NAME,
            "wrong unix root name"
        );
    }

    /// Verifies that the human-facing summary names the actual directory and
    /// states plainly that nothing sensitive is stored.
    ///
    /// **Methodology.** Call [`describe_pki_location`] and check that it is a
    /// single line, that it contains the PKI directory name, and that it
    /// mentions `SecurityPolicy::None`. Pass criterion: all three hold.
    ///
    /// **Results (2026-07-28, unchanged 2026-08-12; Linux x86_64).** Output was
    /// `PKI directory: /home/<user>/.outram-park/ciet-v2-opcua-pki
    /// (self-signed keypair only -- SecurityPolicy::None stores no
    /// credentials)` — 1 line, contains `ciet-v2-opcua-pki`, contains
    /// `SecurityPolicy::None`. Interpretation: a user reading the GUI panel is
    /// told both where the directory is and that it holds no credential.
    #[test]
    fn description_names_the_directory_and_the_missing_security() {
        let description = describe_pki_location();
        assert_eq!(description.lines().count(), 1, "must be a single line");
        assert!(
            description.contains(CIET_V2_PKI_DIR_NAME),
            "description does not name the PKI directory: {description}"
        );
        assert!(
            description.contains("SecurityPolicy::None"),
            "description does not state that there is no security: {description}"
        );
    }
}
