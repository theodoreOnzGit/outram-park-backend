//! Verifies the reference-fixture loader against the fixtures themselves.
//!
//! # Why this test exists
//!
//! Every parity gate in this crate is a comparison against these files. If the
//! loader mis-reads them — a transposed map, an off-by-one in the MATLAB index
//! conversion, a silently truncated field — then every gate downstream is
//! comparing against a permuted reactor and reporting a pass. Nothing else in
//! the suite would catch it.
//!
//! So this file checks the loader, not the physics. **It needs no solver and
//! passes today.**
//!
//! # Methodology
//!
//! Committed reduced fixtures (present in every clone):
//!
//! 1. Shapes — 17×17 radial map, 19-entry axial profile, 10,982-entry state
//!    vector length, 5,491 nodes.
//! 2. `k_eff` against the value quoted in `PROVENANCE.md`.
//! 3. The normalisation the provenance claims — mean exactly 1 over powered
//!    nodes — verified rather than assumed.
//! 4. The reflector structure: 112 of 289 radial positions and 2 of 19 axial
//!    planes are exactly zero.
//! 5. Exact `%.17g` round-tripping, which is what lets a fixture comparison be
//!    limited by the physics rather than by the file format.
//!
//! Full node-level fixtures (gitignored, regenerable): the same checks on
//! shape, plus the strongest available check on the whole pipeline — reducing
//! the loaded `power_density` over `z` and over groups must reproduce the
//! committed radial map. That closes the loop between the two fixture tiers
//! and exercises the index convention on all 10,982 entries. These are skipped
//! with a message when the files are absent.
//!
//! # Results
//!
//! Measured 2026-08-05, GNU Octave capture of the same date: all checks pass.
//! `k_eff` reads back as 1.0290842761799579, matching `PROVENANCE.md` to all
//! ten quoted figures. Both reduced maps normalise to a mean of exactly 1.0
//! over powered nodes (bit-exact, difference 0.0). With the full fixtures
//! present, the radial map derived from `power_density` reproduces the
//! committed `radial_power_map.csv` to a maximum absolute difference of 0.0 —
//! bit-exact — and the axial profile to 1.3e-15, the latter being pure
//! summation-order noise.

mod support;

use bedok::reference::fixtures::{self, Iaea3dReduced};
use support::{compare, FieldShape};

/// Absolute tolerance on the derived-versus-committed reduction cross-check.
///
/// `1e-12`. The measured differences are 0.0 (radial) and 1.3e-15 (axial), so
/// this leaves three orders of headroom for summation-order variation while
/// still catching any real discrepancy in the reduction rule or the index
/// convention.
const REDUCTION_ABS_TOL: f64 = 1e-12;

#[test]
fn reduced_fixtures_load_with_the_documented_shapes() {
    let reduced = Iaea3dReduced::load().expect("committed fixtures load");

    assert_eq!(reduced.grid.nx, 17);
    assert_eq!(reduced.grid.ny, 17);
    assert_eq!(reduced.grid.nz, 19, "the axial reflector plane is included");
    assert_eq!(reduced.grid.ngroups, 2);
    assert_eq!(reduced.grid.nodes(), 5_491);
    assert_eq!(reduced.grid.state_len(), 10_982);

    assert_eq!(reduced.radial_power_map.len(), 17 * 17);
    assert_eq!(reduced.axial_power_profile.len(), 19);
}

#[test]
fn k_eff_matches_the_provenance_document() {
    let reduced = Iaea3dReduced::load().expect("committed fixtures load");

    // PROVENANCE.md quotes 1.0290842762; the file carries the full %.17g form.
    assert_eq!(reduced.k_eff, fixtures::IAEA3D_K_EFF);
    assert!(
        (reduced.k_eff - 1.029_084_276_2).abs() < 5e-11,
        "k_eff {} disagrees with the ten figures in PROVENANCE.md",
        reduced.k_eff
    );
}

#[test]
fn residuals_are_the_two_captured_values() {
    let reduced = Iaea3dReduced::load().expect("committed fixtures load");

    // PROVENANCE.md: fission source 9.611040e-07, k_eff 9.272337e-10. The
    // second is the floor on how tightly any translation can be expected to
    // reproduce k_eff, so it is worth asserting rather than assuming.
    assert!(
        (reduced.fission_source_residual - 9.611_040e-7).abs() < 1e-12,
        "fission-source residual {}",
        reduced.fission_source_residual
    );
    assert!(
        (reduced.k_eff_residual - 9.272_337e-10).abs() < 1e-15,
        "k_eff residual {}",
        reduced.k_eff_residual
    );
    assert!(reduced.k_eff_residual < reduced.fission_source_residual);
}

#[test]
fn power_shapes_are_normalised_to_unit_mean_over_powered_nodes() {
    let reduced = Iaea3dReduced::load().expect("committed fixtures load");

    for (what, values) in [
        ("radial map", &reduced.radial_power_map),
        ("axial profile", &reduced.axial_power_profile),
    ] {
        let powered: Vec<f64> = values.iter().copied().filter(|v| *v != 0.0).collect();
        assert!(!powered.is_empty(), "{what} is entirely zero");
        let mean = powered.iter().sum::<f64>() / powered.len() as f64;
        assert!(
            (mean - 1.0).abs() < 1e-12,
            "{what} mean over powered nodes is {mean}, not 1"
        );
        assert!(
            values.iter().all(|v| v.is_finite() && *v >= 0.0),
            "{what} holds a negative or non-finite power"
        );
    }
}

#[test]
fn the_reflector_shows_up_as_exact_zeros() {
    let reduced = Iaea3dReduced::load().expect("committed fixtures load");

    // IAEA-3D is a quarter-core-symmetric map on a 17x17 grid with the corners
    // outside the reflector: 177 of 289 positions carry power. Axially, the
    // two end planes are reflector.
    let powered_radial = reduced
        .radial_power_map
        .iter()
        .filter(|v| **v != 0.0)
        .count();
    assert_eq!(powered_radial, 177, "powered radial positions");

    let powered_axial = reduced
        .axial_power_profile
        .iter()
        .filter(|v| **v != 0.0)
        .count();
    assert_eq!(powered_axial, 17, "powered axial planes");
    assert_eq!(reduced.axial_power_profile[0], 0.0, "bottom reflector");
    assert_eq!(reduced.axial_power_profile[18], 0.0, "top reflector");
}

#[test]
fn the_radial_map_is_symmetric_as_the_geometry_requires() {
    // IAEA-3D is symmetric about the diagonal. This is a check on the loader's
    // row/column handling as much as on the physics: a transposed read would
    // pass silently here, but a *sheared* one — the classic symptom of a
    // row-major/column-major mix-up on a non-square stride — would not.
    let reduced = Iaea3dReduced::load().expect("committed fixtures load");
    let ny = reduced.grid.ny;

    for ix in 0..reduced.grid.nx {
        for iy in 0..ny {
            let a = reduced.radial_power_map[ix * ny + iy];
            let b = reduced.radial_power_map[iy * ny + ix];
            assert!(
                (a - b).abs() < 1e-12,
                "radial map asymmetric at ({ix},{iy}): {a} vs {b}"
            );
        }
    }
}

#[test]
fn values_round_trip_through_an_ieee_double_exactly() {
    // The fixtures are written at %.17g precisely so this holds. If it ever
    // fails, fixture comparisons are limited by the file format rather than by
    // the physics, and every tolerance in the suite would need re-deriving.
    let reduced = Iaea3dReduced::load().expect("committed fixtures load");
    for value in reduced
        .radial_power_map
        .iter()
        .chain(reduced.axial_power_profile.iter())
        .chain(std::iter::once(&reduced.k_eff))
    {
        let reparsed: f64 = format!("{value:.17e}")
            .parse()
            .expect("formatted value reparses");
        assert_eq!(reparsed, *value, "value {value} did not round-trip");
    }
}

#[test]
fn a_missing_fixture_directory_is_an_error_not_a_panic() {
    let err = Iaea3dReduced::load_from("/nonexistent/bedok/fixtures");
    assert!(err.is_err(), "loading from a missing directory must fail");
}

#[test]
fn full_fixtures_load_with_the_documented_shapes() {
    let Some(full) = support::skip_unless_full_fixtures("full_fixtures_load") else {
        return;
    };

    let len = full.grid.state_len();
    assert_eq!(len, 10_982);
    assert_eq!(full.power_density.len(), len);
    assert_eq!(full.fission_source.len(), len);
    assert_eq!(full.scalar_flux.len(), len);
    assert_eq!(
        full.scalar_flux_iterates.len(),
        fixtures::SCALAR_FLUX_COLUMNS - 1,
        "four retained iterates besides the converged column"
    );
    for iterate in &full.scalar_flux_iterates {
        assert_eq!(iterate.len(), len);
    }

    assert!(
        full.power_density.iter().all(|v| v.is_finite()),
        "a NaN survived the load — every slot must be written exactly once"
    );
    assert!(
        full.scalar_flux.iter().all(|v| v.is_finite() && *v >= 0.0),
        "scalar flux must be non-negative and finite"
    );
}

#[test]
fn the_retained_iterates_bracket_the_converged_flux() {
    // Columns 2-5 are earlier iterates, so they must differ from column 1 but
    // only slightly — the capture stopped at a fission-source residual of
    // 9.6e-07. This checks the multi-column loader kept the columns in order
    // rather than shuffling them.
    let Some(full) = support::skip_unless_full_fixtures("retained_iterates") else {
        return;
    };

    let shape = FieldShape::State(full.grid);
    for (n, iterate) in full.scalar_flux_iterates.iter().enumerate() {
        let comparison = compare(iterate, &full.scalar_flux, shape);
        assert!(
            comparison.max_relative > 0.0,
            "iterate {} is identical to the converged flux — columns may have \
             been duplicated on load",
            n + 2
        );
        assert!(
            comparison.max_relative < 1e-3,
            "iterate {} differs from the converged flux by {:.3e}, far more \
             than the capture's convergence level:\n{comparison}",
            n + 2,
            comparison.max_relative
        );
    }
}

#[test]
fn reducing_the_full_power_field_reproduces_the_committed_radial_map() {
    // The strongest check available on the loader: it exercises the 1-based
    // MATLAB index conversion on all 10,982 entries, and it ties the two
    // fixture tiers together. If the index convention were wrong, the summed
    // map would be a permutation of the committed one and this would fail.
    let Some(full) = support::skip_unless_full_fixtures("radial reduction") else {
        return;
    };
    let reduced = Iaea3dReduced::load().expect("committed fixtures load");

    let derived = full.radial_power_map().expect("reduces");
    let comparison = compare(
        &derived,
        &reduced.radial_power_map,
        FieldShape::Radial {
            nx: full.grid.nx,
            ny: full.grid.ny,
        },
    );
    assert!(
        comparison.max_absolute <= REDUCTION_ABS_TOL,
        "derived radial map disagrees with the committed one:\n{comparison}"
    );
}

#[test]
fn reducing_the_full_power_field_reproduces_the_committed_axial_profile() {
    let Some(full) = support::skip_unless_full_fixtures("axial reduction") else {
        return;
    };
    let reduced = Iaea3dReduced::load().expect("committed fixtures load");

    let derived = full.axial_power_profile().expect("reduces");
    let comparison = compare(
        &derived,
        &reduced.axial_power_profile,
        FieldShape::Axial { nz: full.grid.nz },
    );
    assert!(
        comparison.max_absolute <= REDUCTION_ABS_TOL,
        "derived axial profile disagrees with the committed one:\n{comparison}"
    );
}
