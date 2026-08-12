// Ported from NJOY2016 `src/gaminr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! GAMINR photon-interaction group averaging — the vector (`mfh = 23`) path.
//!
//! GAMINR's initial-energy quadrature is *identical* to GROUPR's: `gpanel`
//! (`gaminr.f90:874-1011`) is line-for-line the same panel-marching Lobatto
//! integrator as `panel`, and `gtsig`/`gtflx` (`gaminr.f90:1133-1160`,
//! `:825-872`) are the same cross-section / flux feeders reduced to
//! `nl = nz = 1`. For a **1-D photoatomic cross-section vector** (total,
//! coherent, incoherent, pair-production, photoelectric, or a heating response —
//! `mfd = 23`, `mtd = 501/502/504/516/522/525/602/621`) the group average is the
//! same flux-weighted ratio
//!
//! ```text
//! sigma_g = integral(sigma*phi) / integral(phi)
//! ```
//!
//! so this module **re-exports the shared engine** in [`crate::groupr::panel`]
//! rather than duplicating it. Build a [`PointwiseXs`] from the MF=23 cross
//! section, pick a [`GroupFlux`] (constant for `iwt = 2`, `1/E` variants for
//! `iwt = 3`, matching `gtflx`/`gnwtf`), and call [`group_average_vector`] over
//! the photon group structure. Serialize the result with the shared GENDF writer
//! ([`GendfSection`], re-exported here) using `mf = 23`.
//!
//! # Not ported — the photon scatter/production **matrix** (`gtff`, `dspla`)
//!
//! The group-to-group matrix feed function `gtff` (`gaminr.f90:1162-1514`) is
//! **not** ported — the coherent (Rayleigh) form-factor Legendre integral
//! (`mtd = 502`), the incoherent (Compton, Klein–Nishina × S(q,Z)) energy-angle
//! integral (`mtd = 504`), and the pair-production matrix (`mtd = 516`) all build
//! the `ff(il, ig)` feed function this vector path fixes to 1. The matrix branch
//! of `dspla` (`gaminr.f90:1083-1128`, `mfd = 26`) that divides those integrals
//! by the group flux is likewise not ported. See [`crate::groupr::kinematics`]
//! for the analogous neutron matrix-path gap. [`gtff_matrix`] is a documented
//! `NotPorted` stub.

use crate::NjoyError;

// Re-export the shared numeric engine with GAMINR-facing names, so a GAMINR user
// finds it without needing to know it is physically shared with GROUPR.
pub use crate::groupr::gendf::{GendfGroupRecord, GendfSection, GendfTape};
pub use crate::groupr::panel::{
    group_average_vector, group_integral, GroupFlux, GroupIntegral, PointwiseXs,
};

/// The photon scatter/production group-to-group **matrix** feed function —
/// **not ported**.
///
/// Placeholder for `gtff`'s matrix branches (`gaminr.f90:1162-1514`): the
/// coherent form-factor (`mtd = 502`), incoherent Klein–Nishina × S(q,Z)
/// (`mtd = 504`), and pair-production (`mtd = 516`) feed functions. Ported, this
/// would return the lab-frame secondary-group feed function `ff(il, ig)` at
/// incident photon energy `e_in` \[eV\] for reaction `mt`.
///
/// Returns [`NjoyError::NotPorted`] tagged `"gaminr::gtff"`.
pub fn gtff_matrix(_e_in: f64, _mt: i32) -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("gaminr::gtff"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// The shared engine, driven through the GAMINR re-exports, group-averages a
    /// photoatomic vector correctly: a constant cross section returns itself.
    ///
    /// **Methodology.** Average a constant photoatomic `sigma ≡ 0.42 barn` over
    /// three photon groups with a constant weight (`iwt = 2`, the GAMINR default)
    /// via [`group_average_vector`]. Assert every group equals `0.42`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** all groups = 0.42 (dev < 1e-13).
    #[test]
    fn photon_vector_average_via_shared_engine() {
        let sigma = PointwiseXs::Constant(0.42);
        let bounds = vec![1.0e4, 1.0e5, 1.0e6, 1.0e7];
        let g = group_average_vector(&sigma, &GroupFlux::Flat, &bounds);
        assert_eq!(g.len(), 3);
        assert!(g.iter().all(|&v| (v - 0.42).abs() < 1e-12));
    }

    /// A photoatomic vector section serializes to a GENDF `mf = 23` section and
    /// reads back losslessly through the shared writer.
    ///
    /// **Methodology.** Build a `mf=23, mt=501` (total) section over two groups,
    /// round-trip it, and assert equality. Confirms GAMINR reuses the GROUPR
    /// GENDF record layout with its own MF.
    ///
    /// **Result (2026-07-15).** parsed == original.
    #[test]
    fn photon_gendf_section_round_trips() {
        let section = GendfSection::vector(
            23,
            501,
            6000.0,
            0.0,
            2,
            0.0,
            &[(1, 1.0e6, 0.5), (2, 5.0e5, 0.3)],
        );
        let rows = section.to_rows();
        let back = GendfSection::from_rows(23, 501, &rows).unwrap();
        assert_eq!(back, section);
        // Arc feeder path is exercised elsewhere; touch Arc so the import is used.
        let _ = Arc::new(section);
    }

    /// The photon matrix feed function honestly reports itself unported.
    #[test]
    fn gtff_matrix_reports_not_ported() {
        assert!(matches!(
            gtff_matrix(1.0e6, 504),
            Err(NjoyError::NotPorted("gaminr::gtff"))
        ));
    }
}
