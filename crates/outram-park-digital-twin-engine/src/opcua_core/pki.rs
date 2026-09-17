//! Where an OUTRAM PARK OPC-UA server keeps its PKI directory.
//!
//! `async-opcua` needs a writable directory to hold its **application instance
//! certificate**. This module decides where that directory lives, so a
//! simulator never scatters `./pki` folders into whatever working directory it
//! happened to be launched from. It is reactor-agnostic: the only per-simulator
//! input is the directory *name*, which comes from
//! [`OpcuaSimulatorProfile::pki_dir_name`](super::simulator::OpcuaSimulatorProfile::pki_dir_name).
//!
//! ## Layout
//!
//! | Platform | Root ([`outram_park_home`]) | PKI dir ([`pki_dir`]) |
//! |---|---|---|
//! | Linux / macOS / Termux | `$HOME/.outram-park` | `$HOME/.outram-park/<pki_dir_name>` |
//! | Windows | `%APPDATA%\outram-park` | `%APPDATA%\outram-park\<pki_dir_name>` |
//!
//! A single dot-directory under the user's home was chosen by the maintainer so
//! every OUTRAM PARK tool that needs persistent scratch space has one obvious
//! place to put it, with the conventional Windows equivalent (`%APPDATA%`,
//! i.e. `directories`' `BaseDirs::data_dir()`).
//!
//! Inside the PKI directory, `async-opcua` populates the usual OPC-UA
//! certificate-store subtree itself on first run:
//!
//! ```text
//! <pki_dir_name>/
//!   own/cert.der          <- this server's self-signed instance certificate
//!   private/private.pem   <- the matching private key
//!   trusted/              <- client certificates the server would trust
//!   rejected/             <- client certificates it has seen and refused
//! ```
//!
//! ## Nothing sensitive is stored here
//!
//! The servers built on [`opcua_core`](super) run with `SecurityPolicy::None`
//! and anonymous access (see [`super::server`]). No channel is ever encrypted or
//! signed with the keypair, no client certificate is ever validated against the
//! trust list, and no user credential of any kind is written. What lands on disk
//! is therefore a **throwaway self-signed keypair** that authenticates nothing —
//! deleting the whole directory costs nothing but a regeneration on next
//! start-up. Do not describe it as a credential store, and do not reuse the key
//! for anything.
//!
//! ## No credentials, ever (`RESPONSIBLE_USE.md`)
//!
//! This module creates a directory and reports its path. It must never grow
//! code that reads institutional credentials, API keys, access tokens, or any
//! certificate belonging to a real facility or production system.
//!
//! ## Units
//!
//! Everything here is a filesystem path or a name. No physical quantities, no
//! units.

use std::path::PathBuf;

/// Directory name of the OUTRAM PARK per-user root, on Linux/macOS/Termux.
///
/// Dot-prefixed, because it sits directly in `$HOME`.
#[cfg(not(target_os = "windows"))]
pub const UNIX_HOME_DIR_NAME: &str = ".outram-park";

/// Directory name of the OUTRAM PARK per-user root, on Windows.
///
/// Not dot-prefixed, because it sits under `%APPDATA%` where hidden
/// directories are not the convention.
#[cfg(target_os = "windows")]
pub const WINDOWS_APPDATA_DIR_NAME: &str = "outram-park";

/// The OUTRAM PARK per-user root directory, created if it does not exist.
///
/// Returns `$HOME/.outram-park` on Linux, macOS and Termux, and
/// `%APPDATA%\outram-park` on Windows. This is a **filesystem path**, not a
/// physical quantity — it carries no units.
///
/// ## Resolution order
///
/// 1. `directories::BaseDirs` — `data_dir()` on Windows, `home_dir()`
///    elsewhere. This is the normal path on every supported platform.
/// 2. The `HOME` environment variable, if `BaseDirs` could not be constructed
///    (it returns `None` when no home directory can be determined at all).
/// 3. The current directory (`.`), as a last resort so the server can still
///    start in a stripped-down container or sandbox.
///
/// The directory is created with `std::fs::create_dir_all` if missing. A
/// creation failure is **not** an error here: the path is still returned and a
/// warning is printed, because `async-opcua` will report the real problem when
/// it tries to write its certificate. That keeps a read-only home directory
/// from taking the whole simulator down.
pub fn outram_park_home() -> PathBuf {
    let root = outram_park_home_path();
    ensure_dir(&root);
    root
}

/// Work out the OUTRAM PARK per-user root **without creating it**.
///
/// Same resolution order as [`outram_park_home`], no filesystem side effects.
/// Use it to display or test a path on a machine whose home directory may be
/// read-only.
pub fn outram_park_home_path() -> PathBuf {
    if let Some(base_dirs) = directories::BaseDirs::new() {
        #[cfg(target_os = "windows")]
        {
            return base_dirs.data_dir().join(WINDOWS_APPDATA_DIR_NAME);
        }
        #[cfg(not(target_os = "windows"))]
        {
            return base_dirs.home_dir().join(UNIX_HOME_DIR_NAME);
        }
    }

    // `BaseDirs::new()` returned `None`: no home directory could be determined.
    // Fall back to `$HOME` (which Termux always sets), then to the current
    // directory.
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        if !home.as_os_str().is_empty() {
            #[cfg(target_os = "windows")]
            {
                return home.join(WINDOWS_APPDATA_DIR_NAME);
            }
            #[cfg(not(target_os = "windows"))]
            {
                return home.join(UNIX_HOME_DIR_NAME);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        PathBuf::from(".").join(WINDOWS_APPDATA_DIR_NAME)
    }
    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from(".").join(UNIX_HOME_DIR_NAME)
    }
}

/// The PKI directory for one simulator, created if it does not exist.
///
/// This is `<`[`outram_park_home`]`>/<dir_name>`, where `dir_name` is the
/// simulator's
/// [`pki_dir_name`](super::simulator::OpcuaSimulatorProfile::pki_dir_name).
/// Pass the result straight to `ServerBuilder::pki_dir`; `async-opcua` creates
/// the `own/`, `private/`, `trusted/` and `rejected/` subdirectories itself and
/// writes a self-signed application instance certificate into `own/` on first
/// start-up.
///
/// As with [`outram_park_home`], a creation failure is warned about rather than
/// returned as an error.
pub fn pki_dir(dir_name: &str) -> PathBuf {
    let dir = outram_park_home().join(dir_name);
    ensure_dir(&dir);
    dir
}

/// A per-instance PKI directory underneath [`pki_dir`], created if missing.
///
/// Returns `<`[`pki_dir`]`>/<sanitised instance_tag>`.
///
/// ## Why this exists: parallel instances clobber a shared certificate store
///
/// `async-opcua` writes its self-signed keypair into the PKI directory on
/// start-up. Two servers starting **concurrently** against the same directory
/// race on `own/cert.der` and `private/private.pem`, and can read a
/// half-written file — a real hazard for headless simulator tests, which may run
/// several servers at once, and for a developer running a simulator while a test
/// suite runs.
///
/// The tag makes each instance's store disjoint. [`server`](super::server)
/// derives it from the TCP port, which is the one thing two servers that can
/// coexist on a machine must differ in, so isolation is automatic and needs no
/// configuration. Tests that want a stronger guarantee — a fresh directory per
/// run rather than per port — can pass [`unique_instance_tag`].
///
/// `instance_tag` is sanitised to ASCII alphanumerics, `-` and `_`; anything
/// else becomes `-`, and an empty result becomes `"default"`. That keeps a
/// caller from escaping the directory with `../` or breaking on a path
/// separator.
pub fn instance_pki_dir(dir_name: &str, instance_tag: &str) -> PathBuf {
    let dir = pki_dir(dir_name).join(sanitise_tag(instance_tag));
    ensure_dir(&dir);
    dir
}

/// A short tag that is unique to this process **and** this thread, for isolating
/// a PKI directory (or any other per-instance scratch path) in parallel tests.
///
/// The tag is `<process id>-<thread id digits>-<nanoseconds>`, e.g.
/// `"18342-7-913204771"`. It is deliberately not cryptographic — it only has to
/// stop two concurrent instances picking the same directory.
///
/// Cargo runs `#[test]` functions as threads inside one process, so the process
/// id alone does not separate them; the thread id and a nanosecond timestamp do.
/// Note that each call returns a **new** value, so capture it once per instance
/// rather than calling it repeatedly.
pub fn unique_instance_tag() -> String {
    let process_id = std::process::id();

    // `ThreadId` has no stable numeric accessor on stable Rust, so use its Debug
    // form (`ThreadId(7)`) and keep the digits.
    let thread_id = format!("{:?}", std::thread::current().id());
    let thread_digits: String = thread_id.chars().filter(|c| c.is_ascii_digit()).collect();

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.subsec_nanos())
        .unwrap_or(0);

    format!("{process_id}-{thread_digits}-{nanos}")
}

/// Reduce an arbitrary tag to something safe to use as a single path component.
fn sanitise_tag(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "default".to_owned()
    } else {
        let mut trimmed = trimmed.to_owned();
        trimmed.truncate(64);
        trimmed
    }
}

/// A one-line, human-readable summary of where a simulator's PKI directory is,
/// for a "how to connect" panel or a start-up log line.
///
/// The wording deliberately states that nothing sensitive is stored, so a
/// reader of the GUI is not misled into thinking the interface is secured.
///
/// # Example output
///
/// ```text
/// PKI directory: /home/alice/.outram-park/ciet-v2-opcua-pki (self-signed keypair only -- SecurityPolicy::None stores no credentials)
/// ```
pub fn describe_pki_location(dir_name: &str) -> String {
    format!(
        "PKI directory: {} (self-signed keypair only -- SecurityPolicy::None stores no credentials)",
        pki_dir(dir_name).display()
    )
}

/// Create `dir` (and any missing parents), printing a warning instead of
/// failing if that is not possible.
fn ensure_dir(dir: &std::path::Path) {
    if dir.is_dir() {
        return;
    }
    if let Err(error) = std::fs::create_dir_all(dir) {
        eprintln!(
            "OUTRAM PARK OPC-UA PKI: could not create directory {} ({error}); \
             the OPC-UA server will report the underlying problem if it needs to write there",
            dir.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that a tag which could escape the PKI directory is reduced to a
    /// single safe path component, since the tag comes from a caller (a port
    /// number today, potentially a user string tomorrow).
    ///
    /// **Methodology.** Feed [`sanitise_tag`] a path-traversal attempt, a path
    /// separator, an empty string and an over-long string. Pass criterion: the
    /// result contains only ASCII alphanumerics, `-` and `_`; is never empty;
    /// is at most 64 characters; and contains no `/`, `\` or `..` sequence that
    /// could leave the directory. No filesystem access.
    ///
    /// **Results (2026-08-12, Linux x86_64).** `"../../etc"` → `"etc"`;
    /// `"port-4840"` → `"port-4840"` (unchanged, the normal case); `"a/b"` →
    /// `"a-b"`; `""` → `"default"`; 200 `x` → 64 characters. All four
    /// constraints held for every case. Interpretation: a per-instance tag
    /// cannot be used to write outside the PKI directory.
    #[test]
    fn instance_tags_cannot_escape_the_pki_directory() {
        for raw in ["../../etc", "port-4840", "a/b", "", &"x".repeat(200)] {
            let cleaned = sanitise_tag(raw);
            assert!(!cleaned.is_empty(), "{raw:?} sanitised to an empty tag");
            assert!(
                cleaned.len() <= 64,
                "{raw:?} sanitised to {} characters",
                cleaned.len()
            );
            assert!(
                cleaned
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                "{raw:?} sanitised to {cleaned:?}, which has illegal characters"
            );
        }

        assert_eq!(sanitise_tag("port-4840"), "port-4840");
        assert_eq!(sanitise_tag(""), "default");
    }

    /// Verifies that the per-user root resolves to the documented directory for
    /// this platform, since every simulator's PKI store hangs off it.
    ///
    /// **Methodology.** Call [`outram_park_home_path`] (the non-creating half of
    /// [`outram_park_home`]) and compare its last component against the platform
    /// rule: `outram-park` on Windows, `.outram-park` everywhere else. Pass
    /// criterion: exact match. **No directory is created**, so the test is safe
    /// on a read-only home and leaves nothing behind.
    ///
    /// **Results (2026-08-12, Linux x86_64, `directories` 5.0.1).** The resolved
    /// root was `/home/<user>/.outram-park`: last component `.outram-park`.
    /// Interpretation: the documented layout table matches what the code
    /// produces on the maintainer's platform. The Windows branch is
    /// `cfg`-selected and was not exercised on this run.
    #[test]
    fn the_per_user_root_is_the_documented_directory() {
        let root = outram_park_home_path();
        let last = root
            .components()
            .next_back()
            .expect("the resolved root must have at least one component")
            .as_os_str()
            .to_string_lossy()
            .into_owned();

        #[cfg(target_os = "windows")]
        assert_eq!(last, WINDOWS_APPDATA_DIR_NAME, "wrong Windows root name");
        #[cfg(not(target_os = "windows"))]
        assert_eq!(last, UNIX_HOME_DIR_NAME, "wrong unix root name");
    }
}
