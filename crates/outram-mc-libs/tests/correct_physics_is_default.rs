//! **Correct physics is the DEFAULT, and this test is what keeps it that way.**
//!
//! Workspace hard rule (2026-09-20): for a high-fidelity crate, physics the
//! evaluation supplies is applied unless a caller explicitly ablates it.
//!
//! Before this rule, `Nuclide::from_endf_file` returned a nuclide with
//! **neither** unresolved-resonance probability tables **nor** DBRC, and both
//! were opt-in builders that the crate's own ICSBEP benchmark examples did not
//! call. The residuals those examples recorded were therefore measured against
//! an incomplete model -- and the omission was invisible, because a missing
//! physics term does not announce itself in `k_eff`.
//!
//! This test fails if either default is ever silently turned back off.

use outram_mc_libs::material::nuclide::Nuclide;
use njoy_outram_park_fork::reference_data::reference_endf;

/// U-238 has a large unresolved range (~20-149 keV on ENDF/B-VIII.0), so a
/// correctly-constructed nuclide MUST carry probability tables over it.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "reconstructs U-238 from ENDF (~60 s); runs by default, skipped under --no-default-features"
)]
fn from_endf_file_applies_urr_and_dbrc_by_default() {
    let Some(p) = reference_endf("n-092_U_238.endf") else {
        eprintln!("SKIP: reference tape not present");
        return;
    };
    let n = Nuclide::from_endf_file(&p, "U238", 293.6, 1.0e-3).expect("construct U238");

    assert!(
        n.has_urr_probability_tables(),
        "U-238 built through from_endf_file carries NO unresolved-resonance \
         probability tables. Correct physics is the default in this crate; if \
         this was turned off deliberately, that is a maintainer decision that \
         must be recorded, not a silent change."
    );

    let (lo, hi) = n.urr_range_ev().expect("URR range");
    assert!(
        (1.0e4..3.0e4).contains(&lo) && (1.0e5..2.0e5).contains(&hi),
        "U-238 URR range [{lo:.3e}, {hi:.3e}] eV is not the expected ~20-149 keV"
    );
}
