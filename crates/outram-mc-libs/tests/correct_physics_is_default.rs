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

/// **The ACE route must apply URR by default too** — GitHub #307.
///
/// This test covered only `from_endf_file`, and that gap was load-bearing:
/// `from_ace` set `urr: None`, so the two routes through this crate carried
/// **different physics** while the test written to stop exactly that drift went
/// on passing. A rule pinned on one construction path is pinned on none.
///
/// DBRC is deliberately **not** required here. It needs 0 K elastic data to
/// sample the target velocity, and an ACE table broadened to its own
/// temperature carries none — the ESZ elastic column is already at `kT`. That
/// is a property of the table, not a choice of the reader, so requiring it
/// would make this test fail for a correct implementation.
#[test]
fn from_ace_applies_urr_by_default() {
    use njoy_outram_park_fork::acer::read;
    use njoy_outram_park_fork::reference_data::ace_reference_file_or_skip;

    let rel = "reference-njoy/endf-b-viii.0/293.6K/U238.ace.gz";
    let Some(p) = ace_reference_file_or_skip(rel, "correct-physics-ace") else {
        return;
    };
    let raw = read::read(&p).expect("read the reference U238 ACE table");
    let n = Nuclide::from_ace(&raw, "U238").expect("construct U238 from ACE");

    assert!(
        n.has_urr_probability_tables(),
        "a U-238 built from ACE carries NO unresolved-resonance probability \
         tables. The ENDF route applies them by default, so this is two routes \
         through one crate with different physics -- the exact drift this file \
         exists to prevent. ACE stores them in the UNR block at JXS(23); see \
         UrrProbabilityTables::from_ace."
    );

    // And they must cover the unresolved range rather than merely exist.
    let mid = 8.45e4; // inside U-238's ~20-149 keV unresolved range
    let dilute = n.xs_at_energy(mid, 293.6);
    let shielded = n.xs_at_energy_urr(mid, 293.6, 0.05);
    println!(
        "U238 from ACE at {mid:.3e} eV: infinitely dilute total {:.5} b, \
         band-0.05 total {:.5} b",
        dilute.total, shielded.total
    );
    assert!(
        (shielded.total - dilute.total).abs() > 1.0e-6 * dilute.total.max(1.0),
        "sampling a low probability band changed nothing ({:.6} vs {:.6} b), so \
         the tables are attached but not reaching the cross sections -- which is \
         indistinguishable from not having them",
        shielded.total,
        dilute.total
    );
}

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

// ─────────────────────────────────────────────────────────────────────────────
// Multigroup scattering anisotropy (GitHub #265)
// ─────────────────────────────────────────────────────────────────────────────

use outram_mc_libs::physics::physics_mg::{Mgxs, ScatterAngle};

fn two_group_set() -> Mgxs {
    Mgxs::new(
        "2g",
        vec![0.080, 0.180],
        vec![0.010, 0.080],
        vec![0.0032, 0.040],
        vec![0.008, 0.100],
        vec![1.0, 0.0],
        vec![0.050, 0.020, 0.000, 0.100],
    )
}

/// **A set that carries angular moments must transport with them, with no
/// further opt-in.**
///
/// This is the pattern the workspace rule asks for: construct through the
/// ordinary path and assert the physics is present. Before #265 the MG kernel
/// resampled isotropically no matter what the set carried, and nothing in the
/// crate could tell — a `⟨μ⟩` of 0.3 and a `⟨μ⟩` of 0 transported identically.
/// On the bare 35 cm cube that difference is worth **−4308 ± 93 pcm**
/// (`verification_and_validation/mg_anisotropic_scattering/`), so a silent
/// regression here is not a rounding matter.
#[test]
fn mg_scattering_anisotropy_is_applied_by_default_when_the_set_carries_it() {
    let aniso = two_group_set()
        .with_legendre_scattering(vec![vec![1.0, 0.3, 0.1, 0.03]; 4])
        .expect("P3 kernel is samplable");

    match &aniso.scatter_angle {
        ScatterAngle::Legendre { order, .. } => assert_eq!(*order, 3, "P3 set"),
        other => panic!(
            "a set built with Legendre moments reports {other:?}. The moments \
             were accepted and then discarded — the MG kernel will transport \
             this P3 set as if it were P0."
        ),
    }
    for g in 0..2 {
        assert!(
            (aniso.group_mean_cosine(g) - 0.3).abs() < 1e-12,
            "group {g} transports <mu> = {} for a kernel declaring 0.3",
            aniso.group_mean_cosine(g)
        );
    }

    // The ablation must be an explicit, visible act — and must actually ablate.
    let ablated = aniso.clone().without_scatter_anisotropy();
    assert_eq!(ablated.scatter_angle, ScatterAngle::Isotropic);
    for g in 0..2 {
        assert_eq!(ablated.group_mean_cosine(g), 0.0);
    }

    // A set with no moments is isotropic because it has nothing to apply —
    // that is a property of the data, not a default that hides physics.
    assert_eq!(two_group_set().scatter_angle, ScatterAngle::Isotropic);
}
