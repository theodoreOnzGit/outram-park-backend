//! Benchmark gates — the stage-1 reference against Yan Ren's captured
//! fixtures, and against the published IAEA-3D reference values.
//!
//! # Two different claims, kept apart
//!
//! `docs/bedok-port-scoping.md` §4 is explicit that these are not the same
//! check, and conflating them would let a wrong answer be reported as a
//! validated one:
//!
//! - **Translation gates** compare the Rust reference against the fixtures
//!   captured from Yan Ren's MATLAB. Agreement shows the translation is
//!   faithful. Disagreement localises a translation error, which is why the
//!   comparator reports *where* rather than merely *whether*.
//! - **Benchmark gates** compare against the published IAEA-3D reference. This
//!   is the one that is a V&V result. Note what §4 says about a failure here:
//!   the cause is ambiguous between a translation error and Yan Ren's
//!   unfinished code also disagreeing, and nothing available distinguishes
//!   them.
//!
//! "Reproduces Yan Ren's results" is a claim only the translation gates can
//! support, and only because the fixtures were captured under Octave — see
//! `tests/fixtures/iaea3d/PROVENANCE.md`.
//!
//! # Methodology
//!
//! IAEA-3D steady state, 17×17×19 nodes, 2 energy groups, 5,491 nodes /
//! 10,982 state entries. The reference path solves the case; the result is
//! reduced to `k_eff`, a radial power map and an axial power profile, and each
//! is compared against the corresponding fixture with the tolerances in
//! [`support::tolerance`] — `TRANSLATION_*` for the fixture gates,
//! `BENCHMARK_*` for the published-value gate. Node-level gates additionally
//! require the uncommitted full fixtures and skip without them.
//!
//! # Results
//!
//! **Not yet run.** Every gate below is `#[ignore]`d because
//! [`support::solve_iaea3d_reference`] is not wired — the solver modules are
//! still being written. No result may be quoted from this file until that
//! changes; per the workspace V&V rule this section records the absence of a
//! measurement rather than omitting it.

#[path = "../support/mod.rs"]
mod support;

use bedok::reference::fixtures::Iaea3dReduced;
use support::{
    assert_k_eff_within, compare, report_solver_not_wired, tolerance, FieldShape,
    SteadyStateSolution,
};

/// Loads the committed fixtures and the reference solve together, or reports
/// a skip.
fn gate_inputs(what: &str) -> Option<(SteadyStateSolution, Iaea3dReduced)> {
    let reduced = Iaea3dReduced::load().expect("committed fixtures load");
    match support::solve_iaea3d_reference() {
        Some(solution) => Some((solution, reduced)),
        None => {
            report_solver_not_wired(what);
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Translation gates — Rust reference vs the captured MATLAB fixtures
// ---------------------------------------------------------------------------

#[test]
#[ignore = "waiting on the stage-1 reference solver; see support::solve_iaea3d_reference"]
fn translation_iaea3d_k_eff_matches_the_capture() {
    let Some((solution, reduced)) = gate_inputs("translation k_eff") else {
        return;
    };
    assert_k_eff_within(
        solution.k_eff,
        reduced.k_eff,
        tolerance::TRANSLATION_K_EFF_ABS,
        "IAEA-3D translation",
    );
}

#[test]
#[ignore = "waiting on the stage-1 reference solver; see support::solve_iaea3d_reference"]
fn translation_iaea3d_radial_power_map_matches_the_capture() {
    let Some((solution, reduced)) = gate_inputs("translation radial map") else {
        return;
    };
    compare(
        &solution.radial_power_map,
        &reduced.radial_power_map,
        FieldShape::Radial {
            nx: reduced.grid.nx,
            ny: reduced.grid.ny,
        },
    )
    .assert_within(tolerance::TRANSLATION_POWER_SHAPE);
}

#[test]
#[ignore = "waiting on the stage-1 reference solver; see support::solve_iaea3d_reference"]
fn translation_iaea3d_axial_power_profile_matches_the_capture() {
    let Some((solution, reduced)) = gate_inputs("translation axial profile") else {
        return;
    };
    compare(
        &solution.axial_power_profile,
        &reduced.axial_power_profile,
        FieldShape::Axial {
            nz: reduced.grid.nz,
        },
    )
    .assert_within(tolerance::TRANSLATION_POWER_SHAPE);
}

#[test]
#[ignore = "waiting on the stage-1 reference solver; see support::solve_iaea3d_reference"]
fn translation_iaea3d_node_power_density_matches_the_capture() {
    // The gate that pins a disagreement to a node. Needs both the solver and
    // the uncommitted full fixtures, and skips on either being absent.
    let Some((solution, _)) = gate_inputs("translation node-level power") else {
        return;
    };
    let Some(full) = support::skip_unless_full_fixtures("translation node-level power") else {
        return;
    };
    let Some(power) = solution.power_density.as_ref() else {
        println!("SKIP translation node-level power: the solve did not expose a node-level field");
        return;
    };

    compare(power, &full.power_density, FieldShape::State(full.grid))
        .assert_within(tolerance::TRANSLATION_NODE_FIELD);
}

#[test]
#[ignore = "waiting on the stage-1 reference solver; see support::solve_iaea3d_reference"]
fn translation_iaea3d_converged_flux_matches_the_capture() {
    let Some(_full) = support::skip_unless_full_fixtures("translation scalar flux") else {
        return;
    };
    // The reference solve does not yet expose its converged flux vector
    // through SteadyStateSolution. When it does, add the field there and
    // compare it here against full.scalar_flux with
    // tolerance::TRANSLATION_NODE_FIELD; the flux is the quantity that
    // localises a nodal-coefficient error most directly, because the power
    // density has already been summed over groups.
    report_solver_not_wired("translation scalar flux");
}

// ---------------------------------------------------------------------------
// Benchmark gate — either path vs the published IAEA-3D reference
// ---------------------------------------------------------------------------

#[test]
#[ignore = "waiting on the published benchmark values in reference::cases"]
fn benchmark_iaea3d_k_eff_within_published_tolerance() {
    let Some((solution, _)) = gate_inputs("published k_eff") else {
        return;
    };

    // The published IAEA-3D reference eigenvalue is deliberately not written
    // here. Per DATA_POLICY.md a benchmark value must travel with its source,
    // edition and page, and that record belongs beside the case data in
    // reference::cases (with a References.md), not inside a test file where it
    // would be an uncited magic number.
    //
    // When the case lands, replace this with:
    //     assert_k_eff_within(solution.k_eff, cases::IAEA3D_PUBLISHED_K_EFF,
    //                         tolerance::BENCHMARK_K_EFF_ABS, "IAEA-3D benchmark");
    let _ = solution.k_eff;
    let _ = tolerance::BENCHMARK_K_EFF_ABS;
    println!("SKIP published k_eff: the cited benchmark value is not available yet");
}

#[test]
#[ignore = "waiting on the published benchmark values in reference::cases"]
fn benchmark_iaea3d_radial_power_within_published_tolerance() {
    let Some((solution, _)) = gate_inputs("published radial power") else {
        return;
    };
    // Same reasoning as above: the published assembly-power map is cited data
    // and belongs with the case. The tolerance is fixed and ready.
    let _ = solution.radial_power_map;
    let _ = tolerance::BENCHMARK_POWER_SHAPE;
    println!("SKIP published radial power: the cited benchmark map is not available yet");
}
