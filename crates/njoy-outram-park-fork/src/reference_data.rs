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

/// Environment variable that turns a data-gated **skip into a hard failure**.
///
/// # Why this exists
///
/// A data-gated test must pass when its reference data is absent — a crates.io
/// consumer has no repository around the crate. But `cargo` swallows a passing
/// test's stdout, so the skip note below is invisible in a normal run, and
/// nothing anywhere fails when a test skips. A skipped test is therefore
/// counted in the "N passed" totals that get quoted as evidence.
///
/// That is not hypothetical. On 2026-09-11 an audit of those totals found **19
/// tests across 4 binaries passing in 0.00 s having asserted nothing**, three of
/// the binaries looking for tapes this repository already had — one of them
/// because the tapes moved directory on 2026-08-17 and *nothing failed*, so six
/// tests asserted nothing for three and a half weeks.
///
/// Set this to `1` in CI, where the reference data *is* present and a skip means
/// something is wrong. Leave it unset for a developer who legitimately lacks the
/// data.
pub const REQUIRE_REFERENCE_DATA_ENV: &str = "OUTRAM_PARK_REQUIRE_REFERENCE_DATA";

/// Is the "a skip is a failure" flag set ([`REQUIRE_REFERENCE_DATA_ENV`])?
///
/// Public so that a test which gates on data this module does not own — an NJOY
/// PENDF named by its own environment variable, a git-lfs library outside the
/// repository — can honour the same flag instead of silently passing:
///
/// ```no_run
/// # use njoy_outram_park_fork::reference_data::reference_data_required;
/// # let have_it = false;
/// # let label = "my-test";
/// if !have_it {
///     assert!(!reference_data_required(), "[{label}] reference data absent");
///     println!("[{label}] SKIP");
///     return;
/// }
/// ```
pub fn reference_data_required() -> bool {
    matches!(
        std::env::var(REQUIRE_REFERENCE_DATA_ENV).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes")
    )
}

/// Decide what a missing reference file means: `None` (skip, printing a note),
/// or a panic when `required`.
///
/// Split out as a **pure** function taking `required` as an argument rather than
/// reading the environment itself, so its two branches can be unit-tested
/// directly. Testing it through the environment would mean mutating a
/// process-global from inside a test binary whose tests run concurrently on
/// threads — which is racy, and would leave this guard in the same
/// never-actually-executed state as the tests it exists to catch.
fn missing_reference(required: bool, label: &str, file: &str, dir: &Path, hint: &str) -> Option<PathBuf> {
    assert!(
        !required,
        "[{label}] reference file {file} not found in {}, and \
         {REQUIRE_REFERENCE_DATA_ENV} is set — this test would have silently \
         passed without asserting anything",
        dir.display()
    );
    println!("[{label}] SKIP: reference file {file} not found in {} ({hint})", dir.display());
    None
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
        None => missing_reference(
            reference_data_required(),
            label,
            file,
            &reference_endf_dir(),
            &format!("set {ENDF_DIR_ENV} to a directory holding it"),
        ),
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

    /// Without the flag, a missing reference file is a skip — the contract a
    /// crates.io consumer depends on, since they have no repository around the
    /// crate.
    #[test]
    fn missing_reference_skips_by_default() {
        let got = missing_reference(
            false,
            "unit-test",
            "no-such-tape.endf",
            Path::new("/nonexistent"),
            "hint",
        );
        assert!(got.is_none());
    }

    /// With the flag, the same situation is a hard failure naming the file —
    /// so CI cannot accumulate tests that pass without asserting anything.
    ///
    /// This is the branch that matters, and it is asserted here rather than
    /// through an environment variable on purpose: `cargo` runs a binary's
    /// tests concurrently on threads of one process, so setting the variable
    /// inside a test would race every other test reading it.
    #[test]
    #[should_panic(expected = "silently passed without asserting anything")]
    fn missing_reference_panics_when_required() {
        missing_reference(
            true,
            "unit-test",
            "no-such-tape.endf",
            Path::new("/nonexistent"),
            "hint",
        );
    }
}

// ── Other reference-data subdirectories (golden tapes that are not ENDF) ─────

/// Environment variable that overrides the **root** of the reference-data tree
/// (the parent of `endf/`, `gendf/`, …) for [`reference_file`]. Distinct from
/// [`ENDF_DIR_ENV`], which overrides only the `endf/` subdirectory.
pub const REFERENCE_DATA_ROOT_ENV: &str = "OUTRAM_PARK_REFERENCE_DATA_DIR";

/// The directory `reference-data/<subdir>` is read from, whether or not it
/// exists: `$OUTRAM_PARK_REFERENCE_DATA_DIR/<subdir>` when set, else the
/// in-repo `<crate>/../../reference-data/<subdir>`.
///
/// `subdir` is a bare directory name such as `"gendf"`; the raw ENDF tapes keep
/// their own accessor ([`reference_endf_dir`]) because they have a separate
/// override variable and the library-suffix tolerance.
pub fn reference_data_dir(subdir: &str) -> PathBuf {
    if let Ok(root) = std::env::var(REFERENCE_DATA_ROOT_ENV) {
        if !root.is_empty() {
            return PathBuf::from(root).join(subdir);
        }
    }
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../reference-data");
    p.push(subdir);
    p
}

/// Absolute path of `reference-data/<subdir>/<file>` (e.g. a golden GENDF tape
/// under `"gendf"`), or `None` when it is not present on this machine.
///
/// Same contract as [`reference_endf`]: a data-gated test must **skip** when
/// this returns `None` — a crates.io consumer has no repository around the crate.
pub fn reference_file(subdir: &str, file: &str) -> Option<PathBuf> {
    let p = reference_data_dir(subdir).join(file);
    p.exists().then_some(p)
}

/// [`reference_file`], but prints a skip note naming `label` and the directory
/// tried when the file is absent — the idiom for a data-gated V&V test.
pub fn reference_file_or_skip(subdir: &str, file: &str, label: &str) -> Option<PathBuf> {
    match reference_file(subdir, file) {
        Some(p) => Some(p),
        None => missing_reference(
            reference_data_required(),
            label,
            file,
            &reference_data_dir(subdir),
            &format!("set {REFERENCE_DATA_ROOT_ENV} to the reference-data root holding it"),
        ),
    }
}
