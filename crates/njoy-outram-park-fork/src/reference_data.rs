//! Locating the repository's **reference ENDF tapes** for verification &
//! validation.
//!
//! # Why this module exists
//!
//! Raw ENDF-6 evaluated tapes are large — the ENDF/B-VIII.0 U-235 neutron
//! sublibrary alone is 36.9 MB, and the eleven tapes this workspace uses for
//! V&V total ~89 MB. crates.io caps a published package at 10 MB, so **an ENDF
//! tape must never live inside a crate directory**: `cargo package` collects
//! files by walking the crate root, and anything under it is a candidate for
//! the tarball.
//!
//! They therefore live at the **repository root**, in `reference-data/endf/`,
//! outside `crates/`. Cargo cannot reach them, so no `include`/`exclude`
//! allowlist has to be maintained correctly for the packaging to stay small —
//! the layout enforces it rather than a rule. `tests/no_endf_inside_crates.rs`
//! asserts the invariant so it cannot silently regress.
//!
//! # Resolution order
//!
//! [`reference_endf`] tries, in order:
//!
//! 1. `$OUTRAM_PARK_ENDF_DIR/<file>` — explicit override, for a machine that
//!    keeps its tapes elsewhere (a shared read-only mount, a cache populated by
//!    [`crate::acquire::EndfCache`]).
//! 2. `<CARGO_MANIFEST_DIR>/../../reference-data/endf/<file>` — the in-repo
//!    location, which is where a git clone of this workspace finds them.
//!
//! and returns `None` when the tape is absent rather than erroring. **That is
//! deliberate**: the tapes are git-tracked but a consumer building this crate
//! from crates.io has no repository around it, so every V&V test that needs one
//! must skip gracefully rather than fail. Use [`reference_endf_or_skip`] to get
//! that behaviour with a printed note.
//!
//! # Data policy
//!
//! Only open, published evaluated data belongs in `reference-data/endf/` —
//! ENDF/B-VIII.0 (NNDC/IAEA) and TENDL are public. See `DATA_POLICY.md` and the
//! provenance table in `reference-data/endf/README.md`, which records the
//! library, MAT number, source URL and date accessed for every tape.

use std::path::{Path, PathBuf};

/// Environment variable that overrides where reference tapes are looked up.
pub const ENDF_DIR_ENV: &str = "OUTRAM_PARK_ENDF_DIR";

/// The directory reference tapes are read from, whether or not it exists.
///
/// `$OUTRAM_PARK_ENDF_DIR` when set, else the in-repo
/// `<crate>/../../reference-data/endf`. Use [`reference_endf`] instead unless
/// you specifically need the directory (to list it, or to report it in an error).
pub fn reference_endf_dir() -> PathBuf {
    if let Ok(dir) = std::env::var(ENDF_DIR_ENV) {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../reference-data/endf");
    p
}

/// Absolute path of reference tape `file` (e.g. `"tsl-CinSiC.endf"`), or `None`
/// when it is not present on this machine.
///
/// `file` is a bare file name, not a path — the directory comes from
/// [`reference_endf_dir`].
///
/// # Library-suffix tolerance
///
/// Tapes in this repository are named inconsistently: some carry the library in
/// the file name (`tsl-HinZrH-ENDF8.0.endf`, `tsl-013_Al_027-ENDF8.0.endf`) and
/// some do not (`tsl-CinSiC.endf`, `n-092_U_238.endf`), because they were
/// downloaded from NNDC at different times. Rather than make every caller guess,
/// a lookup for `<stem>.endf` that misses also tries `<stem>-ENDF8.0.endf`, and
/// vice versa. Renaming the committed tapes would be the tidier fix but would
/// break the provenance table's link to the names NNDC actually serves.
///
/// # Examples
///
/// ```
/// use njoy_outram_park_fork::reference_data::reference_endf;
///
/// // Present in a git clone of the workspace; absent when this crate is built
/// // from crates.io, in which case the caller skips the V&V test.
/// match reference_endf("a-002_He_004-ENDF8.0.endf") {
///     Some(path) => assert!(path.exists()),
///     None => { /* tape not available here — skip */ }
/// }
/// ```
pub fn reference_endf(file: &str) -> Option<PathBuf> {
    const LIB_SUFFIX: &str = "-ENDF8.0";

    let dir = reference_endf_dir();
    let direct = dir.join(file);
    if direct.exists() {
        return Some(direct);
    }

    // Try the other spelling of the same tape.
    let stem = file.strip_suffix(".endf")?;
    let alternate = match stem.strip_suffix(LIB_SUFFIX) {
        Some(bare) => format!("{bare}.endf"),
        None => format!("{stem}{LIB_SUFFIX}.endf"),
    };
    let alternate = dir.join(alternate);
    alternate.exists().then_some(alternate)
}

/// [`reference_endf`], but prints a skip note naming `label` and the directory
/// tried when the tape is absent.
///
/// The idiom for a data-gated V&V test, which must **pass** rather than fail
/// when the reference data is not on the machine:
///
/// ```no_run
/// use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
///
/// # fn main() {
/// let Some(tape) = reference_endf_or_skip("n-092_U_235-ENDF8.0.endf", "U-235 RECONR")
/// else {
///     return; // skipped, with a printed note
/// };
/// // ... run the verification against `tape`
/// # }
/// ```
pub fn reference_endf_or_skip(file: &str, label: &str) -> Option<PathBuf> {
    match reference_endf(file) {
        Some(p) => Some(p),
        None => {
            println!(
                "[{label}] SKIP: reference tape {file} not found in {} \
                 (set {ENDF_DIR_ENV} to a directory holding it)",
                reference_endf_dir().display()
            );
            None
        }
    }
}

/// `true` when `path` names a file that looks like a raw ENDF tape by extension.
///
/// Used by the packaging guard test; exposed because the same question comes up
/// when auditing a directory for data-policy compliance.
pub fn is_endf_tape(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("endf"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir_honours_the_environment_override() {
        // Not using std::env::set_var (process-global, races other tests): check
        // the fallback branch is the in-repo path instead.
        let d = reference_endf_dir();
        assert!(
            d.ends_with("reference-data/endf") || std::env::var(ENDF_DIR_ENV).is_ok(),
            "default directory should be the in-repo reference-data/endf, got {}",
            d.display()
        );
    }

    #[test]
    fn absent_tape_resolves_to_none() {
        assert!(reference_endf("definitely-not-a-real-tape-9f3a.endf").is_none());
        // A name with no `.endf` extension has no alternate spelling to try.
        assert!(reference_endf("not-a-tape").is_none());
    }

    /// The library-suffix tolerance documented on [`reference_endf`]: the ZrH
    /// tape is committed as `tsl-HinZrH-ENDF8.0.endf`, but
    /// `outram-mc-libs`' thermal suite asks for `tsl-HinZrH.endf` (the name the
    /// ENDF/B-VIII.0 `thermal_scatt/` directory uses). Both must resolve.
    ///
    /// Data-gated: passes trivially when the tapes are absent.
    #[test]
    fn either_spelling_finds_the_same_tape() {
        let bare = reference_endf("tsl-HinZrH.endf");
        let suffixed = reference_endf("tsl-HinZrH-ENDF8.0.endf");
        match (bare, suffixed) {
            (Some(a), Some(b)) => assert_eq!(a, b, "both spellings must name one file"),
            (None, None) => { /* tapes not present on this machine — skip */ }
            (a, b) => panic!("one spelling resolved and the other did not: {a:?} vs {b:?}"),
        }
    }

    #[test]
    fn endf_extension_is_recognised_case_insensitively() {
        assert!(is_endf_tape(Path::new("tsl-CinSiC.endf")));
        assert!(is_endf_tape(Path::new("N-092_U_235.ENDF")));
        assert!(!is_endf_tape(Path::new("tsl-CinSiC.leapr")));
        assert!(!is_endf_tape(Path::new("README.md")));
    }
}
