//! **EGSnrc port scaffold** (gh:#976): one placeholder module per upstream
//! Mortran file. **Nothing is implemented.**
//!
//! Upstream: EGSnrc, <https://github.com/nrc-cnrc/EGSnrc>, commit
//! `bd49a47aae3938f49f4100b8e018091861408b19` (the file list below was read
//! from a shallow clone on 2026-11-20; EGSnrc is not vendored in this
//! workspace and no EGSnrc text is committed). Licence: AGPL-3.0, which
//! matches this workspace (see the crate's `NOTICE`).
//!
//! # What is scaffolded
//!
//! The core Mortran of `HEN_HOUSE/src/` (every `.mortran` and `.macros`
//! file; the `*.correlations` files are data, not code) and
//! `HEN_HOUSE/pegs4/pegs4.mortran`: 21 files, one module each, named by the
//! file name with `.` read as `_` and lower-cased (`egsnrc.mortran` is
//! [`egsnrc_mortran`]). Each module's `//!` gives the upstream path, the
//! commit and what the routine family does.
//!
//! # What is not, for now
//!
//! The `egs++` C++ geometry library (`HEN_HOUSE/egs++`) and the user codes
//! (`HEN_HOUSE/user_codes`). Geometry here would be this workspace's own
//! (`outram-blender`); user codes are applications, not library.
//!
//! # Rules for the port
//!
//! Electrons and positrons are PUNGGOL's (maintainer, 2026-10-20), ions stay
//! in `boon-lay`. A generator (RANMAR, RANLUX) or other general numerics go
//! to `petir`. The port keeps each file's attribution header block, cites
//! `file:line` upstream for every routine, and is differential-tested against
//! a built EGSnrc before it is called verified. Mortran macros are
//! **expanded by hand into functions and enums** (the workspace writes no
//! macros).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

pub mod egsnrc_mortran;
pub mod egsnrc_macros;
pub mod egs_utilities_mortran;
pub mod nrcaux_mortran;
pub mod ranmar_macros;
pub mod ranmar_mortran;
pub mod ranlux_macros;
pub mod ranlux_mortran;
pub mod eii_mortran;
pub mod egs_parallel_mortran;
pub mod get_inputs_mortran;
pub mod get_media_inputs_mortran;
pub mod lnblnk1_mortran;
pub mod transportp_macros;
pub mod rad_compton1_mortran;
pub mod emf_macros_mortran;
pub mod eemf_macros_mortran;
pub mod eemf_macros_beamnrc_mortran;
pub mod pegs4_macros_mortran;
pub mod pegs4_routines_mortran;
pub mod pegs4_mortran;

/// The EGSnrc commit the file list was read at.
pub const UPSTREAM_COMMIT: &str = "bd49a47aae3938f49f4100b8e018091861408b19";

/// Upstream paths (relative to the EGSnrc repository root), one per module
/// above, in the same order.
pub const UPSTREAM_FILES: [&str; 21] = [
    "HEN_HOUSE/src/egsnrc.mortran",
    "HEN_HOUSE/src/egsnrc.macros",
    "HEN_HOUSE/src/egs_utilities.mortran",
    "HEN_HOUSE/src/nrcaux.mortran",
    "HEN_HOUSE/src/ranmar.macros",
    "HEN_HOUSE/src/ranmar.mortran",
    "HEN_HOUSE/src/ranlux.macros",
    "HEN_HOUSE/src/ranlux.mortran",
    "HEN_HOUSE/src/eii.mortran",
    "HEN_HOUSE/src/egs_parallel.mortran",
    "HEN_HOUSE/src/get_inputs.mortran",
    "HEN_HOUSE/src/get_media_inputs.mortran",
    "HEN_HOUSE/src/lnblnk1.mortran",
    "HEN_HOUSE/src/transportp.macros",
    "HEN_HOUSE/src/rad_compton1.mortran",
    "HEN_HOUSE/src/emf_macros.mortran",
    "HEN_HOUSE/src/EEMF_macros.mortran",
    "HEN_HOUSE/src/EEMF_macros_beamnrc.mortran",
    "HEN_HOUSE/src/pegs4_macros.mortran",
    "HEN_HOUSE/src/pegs4_routines.mortran",
    "HEN_HOUSE/pegs4/pegs4.mortran",
];

/// The module an upstream path is scaffolded as: the file name with `.` read
/// as `_`, lower-cased.
#[must_use]
pub fn module_name(upstream_path: &str) -> String {
    let base = upstream_path.rsplit('/').next().unwrap_or(upstream_path);
    base.replace('.', "_").to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every upstream file has exactly one plain `pub mod` line here, and
    /// there is no module without an upstream file (kovan reads `mod` lines
    /// from the text, so they must be plain).
    #[test]
    fn upstream_files_and_declared_modules_agree() {
        let text = include_str!("mod.rs");
        let declared: Vec<&str> = text
            .lines()
            .filter_map(|l| l.strip_prefix("pub mod ").and_then(|r| r.strip_suffix(';')))
            .collect();
        assert_eq!(declared.len(), UPSTREAM_FILES.len());
        for (path, module) in UPSTREAM_FILES.iter().zip(&declared) {
            assert_eq!(&module_name(path).as_str(), module, "{path}");
        }
        assert_eq!(UPSTREAM_COMMIT.len(), 40);
    }

    /// Each placeholder file names its upstream path and says nothing is ported.
    #[test]
    fn every_placeholder_says_so() {
        let docs = [
            include_str!("egsnrc_mortran.rs"),
            include_str!("egsnrc_macros.rs"),
            include_str!("egs_utilities_mortran.rs"),
            include_str!("nrcaux_mortran.rs"),
            include_str!("ranmar_macros.rs"),
            include_str!("ranmar_mortran.rs"),
            include_str!("ranlux_macros.rs"),
            include_str!("ranlux_mortran.rs"),
            include_str!("eii_mortran.rs"),
            include_str!("egs_parallel_mortran.rs"),
            include_str!("get_inputs_mortran.rs"),
            include_str!("get_media_inputs_mortran.rs"),
            include_str!("lnblnk1_mortran.rs"),
            include_str!("transportp_macros.rs"),
            include_str!("rad_compton1_mortran.rs"),
            include_str!("emf_macros_mortran.rs"),
            include_str!("eemf_macros_mortran.rs"),
            include_str!("eemf_macros_beamnrc_mortran.rs"),
            include_str!("pegs4_macros_mortran.rs"),
            include_str!("pegs4_routines_mortran.rs"),
            include_str!("pegs4_mortran.rs"),
        ];
        for (path, doc) in UPSTREAM_FILES.iter().zip(docs) {
            assert!(doc.contains(path), "{path}");
            assert!(doc.contains("placeholder, nothing ported (gh:#976)"), "{path}");
            assert!(doc.contains(UPSTREAM_COMMIT), "{path}");
        }
    }
}
