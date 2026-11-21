//! **EGSnrc port** (gh:#976): one module per upstream Mortran file. ~~Only `egsnrc_mortran` and `egsnrc_macros` hold code~~ **CORRECTED 2026-11-21:** `egsnrc_mortran`, `egsnrc_macros`, `egs_utilities_mortran`, `nrcaux_mortran`, `get_inputs_mortran`, `get_media_inputs_mortran`, `pegs4_routines_mortran` and `pegs4_mortran` hold code (first ports, not differentially tested, gh:#1680; `eii_mortran` and `lnblnk1_mortran` point to where their routines live); ~~the rest (`rad_compton1`, the `EEMF`/`emf` macros, `egs_parallel`, `transportp_macros`, `pegs4_macros`) are placeholders or deliberately not ported~~ **CORRECTED 2026-11-21:** `emf_macros` is ported inside `egsnrc_mortran` (`ElectrConfig::em`) and `EEMF_macros` has its own module, `eemf_macros_mortran` (magnetic, electric and combined fields, PRESTA-II and PRESTA-I, exact and inexact BCA; both compared with a compiled EGSnrc); the rest (`rad_compton1`, ~~`EEMF_macros_beamnrc`~~ **CORRECTED 2026-11-21 (later):** `EEMF_macros_beamnrc` is ported as `EemfVariant::BeamNrc` inside `eemf_macros_mortran`, `transportp_macros`, `pegs4_macros`) are placeholders or deliberately not ported; ~~`egs_parallel`~~ **CORRECTED 2026-11-21:** `egs_parallel_mortran` holds the job-control state transitions of `egs_pjob_control`/`egs_pjob_finish` (compared with real processes).
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
pub mod estar_cpp;

/// The EGSnrc commit the file list was read at.
pub const UPSTREAM_COMMIT: &str = "bd49a47aae3938f49f4100b8e018091861408b19";

/// Upstream paths (relative to the EGSnrc repository root), one per module
/// above, in the same order.
pub const UPSTREAM_FILES: [&str; 22] = [
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
    "HEN_HOUSE/egs++/estar/estar.cpp",
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

    /// Each file names its upstream path and commit and states its status.
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
            include_str!("estar_cpp.rs"),
        ];
        for (path, doc) in UPSTREAM_FILES.iter().zip(docs) {
            assert!(doc.contains(path), "{path}");
            // A file is a placeholder, a first port, or deliberately not
            // re-ported (it lives in PETIR); it must say which.
            let says = doc.contains("placeholder, nothing ported (gh:#976)")
                || doc.contains("first port")
                || doc.contains("not re-ported, deliberately (gh:#1680)");
            assert!(says, "{path}");
            assert!(doc.contains(UPSTREAM_COMMIT), "{path}");
        }
    }
}

/// A decimal constant from the Mortran source (gh:#1687).
///
/// Mortran3 emits real constants without a `d0` suffix, so a stock gfortran
/// build of EGSnrc reads them as **single precision**, then widens them
/// (`1.6605655` becomes `1.66056549549102783...`). The port's standard is the
/// exact `f64` constant. With the cargo feature `egsnrc-f32-constants`, every
/// constant wrapped in `lit` is first rounded through `f32`, which reproduces
/// the stock build bit for bit and is how the port is validated against it
/// (`docs/egsnrc_phase2_differential.md`). Off by default.
#[inline]
pub const fn lit(x: f64) -> f64 {
    if cfg!(feature = "egsnrc-f32-constants") { x as f32 as f64 } else { x }
}

/// A Mortran decimal constant times a `real*4` variable. In a stock build
/// (single-precision constants) both operands are single, so the product is
/// rounded to `f32`; with exact `f64` constants (the default, and the
/// `-fdefault-real-8` build) the constant is double and the product is `f64`
/// (gh:#1694: found by dumping `eta_array` from both builds).
#[inline]
pub fn lit_mul(constant: f64, v: f32) -> f64 {
    if cfg!(feature = "egsnrc-f32-constants") { f64::from(constant as f32 * v) } else { constant * f64::from(v) }
}

/// `Log(c)` of a Mortran decimal constant: with single-precision constants the
/// argument is `real*4`, so a stock build calls `logf` and gets a single result
/// (`elarray(ndata) = Log(1e5)` in `init_spin`); otherwise `f64` (gh:#1694).
#[inline]
pub fn lit_ln(constant: f64) -> f64 {
    if cfg!(feature = "egsnrc-f32-constants") {
        f64::from(petir::mathf::RealMath::r_ln(constant) as f32)
    } else {
        petir::mathf::RealMath::r_ln(constant)
    }
}

/// A Mortran decimal constant divided by an integer (`atav = 1./nsh`): with
/// single-precision constants the quotient is `real*4` (gh:#1694).
#[inline]
pub fn lit_div(constant: f64, n: f64) -> f64 {
    if cfg!(feature = "egsnrc-f32-constants") { f64::from(constant as f32 / n as f32) } else { constant / n }
}
