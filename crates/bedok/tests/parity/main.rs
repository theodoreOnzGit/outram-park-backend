//! Parity gates — stage 2 against stage 1, plus the self-checks that keep the
//! comparator itself honest.
//!
//! # The rule this file enforces
//!
//! From `docs/bedok-port-scoping.md` §1: *no component is accepted into the
//! substituted path until it reproduces stage 1 on the benchmark suite to a
//! stated tolerance, and no component is improved before it has passed
//! parity*. A substitution that changes results **and** claims to be better
//! cannot be told apart from one that is simply wrong.
//!
//! # Two kinds of test here
//!
//! - **Harness self-checks** (not ignored, and they run today). A comparator
//!   that silently reports zero difference would turn every gate in the crate
//!   into a rubber stamp, so the comparator is tested against planted
//!   divergences with known locations and magnitudes. These are the tests that
//!   make the rest of the suite worth trusting.
//! - **Component gates** (`#[ignore]`d). One per substitution in §5. They are
//!   waiting on both paths existing; see
//!   [`support::solve_iaea3d_reference`] for the two edits that activate them.
//!
//! # Methodology
//!
//! Each component gate solves IAEA-3D twice in the same process — once with
//! every component on [`Implementation::Reference`], once with the component
//! under test switched to its substitute — and compares `k_eff` and the two
//! power shapes with the `SUBSTITUTION_*` tolerances in
//! [`support::tolerance`]. Solving both in one process is why stage 1 survives
//! as running code rather than as an archived branch (§1).
//!
//! # Results
//!
//! **No component has been measured.** All seven are
//! [`ParityStatus::NotStarted`], and nothing has been substituted, so no
//! parity result exists to report. The harness self-checks below pass as of
//! 2026-08-05.

#[path = "../support/mod.rs"]
mod support;

use bedok::reference::fixtures::Iaea3dReduced;
use bedok::substituted::{Component, Implementation, ParityStatus};
use support::{compare, report_solver_not_wired, tolerance, FieldShape};

// ---------------------------------------------------------------------------
// Harness self-checks — these run now
// ---------------------------------------------------------------------------

#[test]
fn comparator_reports_no_difference_between_a_field_and_itself() {
    let reduced = Iaea3dReduced::load().expect("committed fixtures load");
    let shape = FieldShape::Radial {
        nx: reduced.grid.nx,
        ny: reduced.grid.ny,
    };

    let comparison = compare(&reduced.radial_power_map, &reduced.radial_power_map, shape);
    assert_eq!(comparison.max_absolute, 0.0);
    assert_eq!(comparison.max_relative, 0.0);
    assert_eq!(comparison.l2_absolute, 0.0);
    assert_eq!(comparison.l2_relative, 0.0);
    assert_eq!(comparison.len, 289);
    assert_eq!(
        comparison.significant, 177,
        "the 112 reflector zeros must fall below the significance floor"
    );
    assert!(comparison.is_within(tolerance::TRANSLATION_POWER_SHAPE));
}

#[test]
fn comparator_finds_a_planted_divergence_at_the_right_place() {
    // The test that makes every other gate meaningful: perturb one known node
    // and check the comparator names it. A comparator that reports the wrong
    // location is worse than none, because it sends debugging somewhere else.
    let reduced = Iaea3dReduced::load().expect("committed fixtures load");
    let ny = reduced.grid.ny;
    let shape = FieldShape::Radial {
        nx: reduced.grid.nx,
        ny,
    };

    // 0-based (ix=4, iy=6) — a powered interior position, MATLAB (5, 7).
    let (ix, iy) = (4usize, 6usize);
    let target = ix * ny + iy;
    let original = reduced.radial_power_map[target];
    assert!(original > 0.0, "picked an unpowered position by mistake");

    let mut perturbed = reduced.radial_power_map.clone();
    perturbed[target] = original * 1.01; // exactly 1 % high

    let comparison = compare(&perturbed, &reduced.radial_power_map, shape);
    assert_eq!(comparison.max_absolute_at.index, target);
    assert_eq!(comparison.max_relative_at.index, target);
    assert!(
        (comparison.max_relative - 0.01).abs() < 1e-12,
        "expected a 1 % relative difference, got {:.6e}",
        comparison.max_relative
    );
    assert!(
        (comparison.max_absolute - original * 0.01).abs() < 1e-12,
        "absolute difference {:.6e} does not match the planted perturbation",
        comparison.max_absolute
    );

    // The report must name the position in both conventions — the MATLAB one
    // is what anyone cross-checking against Yan Ren's code will need.
    let described = comparison.shape.describe(target);
    assert!(
        described.contains("ix=4"),
        "0-based coordinate: {described}"
    );
    assert!(described.contains("ix=5"), "MATLAB coordinate: {described}");

    // And a 1 % error must actually fail the tolerances it should fail.
    assert!(!comparison.is_within(tolerance::TRANSLATION_POWER_SHAPE));
    assert!(!comparison.is_within(tolerance::SUBSTITUTION_POWER_SHAPE));
    assert!(
        comparison.is_within(tolerance::BENCHMARK_POWER_SHAPE),
        "1 % is inside the 5 % benchmark bar, as intended"
    );
}

#[test]
fn comparator_notices_a_field_that_is_slightly_wrong_everywhere() {
    // The failure mode a single-point maximum can miss: no node is badly
    // wrong, but the whole field is biased. The L2 statistic is what catches
    // it, which is why the tolerance carries two numbers rather than one.
    let reduced = Iaea3dReduced::load().expect("committed fixtures load");
    let shape = FieldShape::Radial {
        nx: reduced.grid.nx,
        ny: reduced.grid.ny,
    };

    let biased: Vec<f64> = reduced
        .radial_power_map
        .iter()
        .map(|v| v * (1.0 + 5e-4))
        .collect();
    let comparison = compare(&biased, &reduced.radial_power_map, shape);

    assert!(
        comparison.max_relative < tolerance::SUBSTITUTION_POWER_SHAPE.max_relative,
        "the point-wise statistic alone would let this through"
    );
    assert!(
        comparison.l2_relative > tolerance::SUBSTITUTION_POWER_SHAPE.relative_l2,
        "the L2 statistic must catch a uniform bias"
    );
    assert!(!comparison.is_within(tolerance::SUBSTITUTION_POWER_SHAPE));
}

#[test]
fn comparator_keeps_reflector_zeros_out_of_the_relative_statistic() {
    // A difference at an entry the reference says is exactly zero has no
    // relative magnitude, and dividing by it would produce an infinity that
    // hides every real discrepancy. It must still show up in the absolute
    // statistic — a solve leaking power into the reflector is a genuine bug.
    let reduced = Iaea3dReduced::load().expect("committed fixtures load");
    let ny = reduced.grid.ny;
    let shape = FieldShape::Radial {
        nx: reduced.grid.nx,
        ny,
    };

    let corner = 0usize * ny + 15; // 0-based (0,15): unfuelled, exactly zero
    assert_eq!(reduced.radial_power_map[corner], 0.0);

    let mut leaked = reduced.radial_power_map.clone();
    leaked[corner] = 1e-3;

    let comparison = compare(&leaked, &reduced.radial_power_map, shape);
    assert!(
        comparison.max_relative.is_finite(),
        "a zero-valued reference entry must not produce an infinite ratio"
    );
    assert_eq!(comparison.max_relative, 0.0, "no significant entry moved");
    assert_eq!(comparison.max_absolute_at.index, corner);
    assert_eq!(comparison.max_absolute, 1e-3);
}

#[test]
fn tolerances_are_ordered_from_strictest_to_loosest() {
    // The three tiers mean different things and must not be permuted by an
    // edit. Translation is the strictest, benchmark the loosest; a change that
    // inverts them is a mistake regardless of the numbers chosen.
    assert!(tolerance::TRANSLATION_K_EFF_ABS < tolerance::SUBSTITUTION_K_EFF_ABS);
    assert!(tolerance::SUBSTITUTION_K_EFF_ABS < tolerance::BENCHMARK_K_EFF_ABS);

    assert!(
        tolerance::TRANSLATION_POWER_SHAPE.max_relative
            < tolerance::SUBSTITUTION_POWER_SHAPE.max_relative
    );
    assert!(
        tolerance::SUBSTITUTION_POWER_SHAPE.max_relative
            < tolerance::BENCHMARK_POWER_SHAPE.max_relative
    );

    // The translation tier must not demand more than the reference itself
    // converged to: the capture stopped at a fission-source residual of
    // 9.611e-07 and a k_eff residual of 9.272e-10.
    let reduced = Iaea3dReduced::load().expect("committed fixtures load");
    assert!(
        tolerance::TRANSLATION_K_EFF_ABS > reduced.k_eff_residual,
        "k_eff tolerance is tighter than the reference's own convergence"
    );
    assert!(
        tolerance::TRANSLATION_POWER_SHAPE.max_relative >= reduced.fission_source_residual,
        "power-shape tolerance is tighter than the reference's own convergence"
    );
}

#[test]
fn no_component_may_be_used_before_its_gate_passes() {
    // The scoping rule as an executable check rather than a paragraph.
    for component in Component::ALL {
        let status = component.parity_status();
        if status.is_accepted() {
            assert!(
                matches!(status, ParityStatus::Passed { .. }),
                "{component:?} is accepted without a Passed status"
            );
            if let ParityStatus::Passed {
                max_relative_difference,
                measured,
            } = status
            {
                assert!(
                    max_relative_difference <= tolerance::SUBSTITUTION_POWER_SHAPE.max_relative,
                    "{component:?} claims a pass at {max_relative_difference:.3e}, \
                     outside the substitution tolerance"
                );
                assert!(
                    !measured.is_empty(),
                    "{component:?} claims a pass with no measurement date"
                );
            }
        }
    }
}

#[test]
fn the_reference_implementation_is_always_selectable() {
    // Every kernel must offer the reference path unconditionally: it defines
    // parity, so it cannot itself be gated on one.
    use bedok::substituted::{
        channel_flow::ChannelFlowKernel, chf::ChfKernel, cross_sections::CrossSectionSource,
        drift_flux::DriftFluxKernel, fuel_rod::FuelRodKernel, kinetics::KineticsKernel,
        linear_solver::LinearSolverKernel,
    };

    assert!(ChannelFlowKernel::default().is_accepted());
    assert!(DriftFluxKernel::default().is_accepted());
    assert!(ChfKernel::default().is_accepted());
    assert!(FuelRodKernel::default().is_accepted());
    assert!(KineticsKernel::default().is_accepted());
    assert!(CrossSectionSource::default().is_accepted());
    assert!(LinearSolverKernel::default().is_accepted());

    assert_eq!(
        ChannelFlowKernel::default().implementation(),
        Implementation::Reference
    );
    assert!(
        !ChannelFlowKernel::Tuas.is_accepted(),
        "an unmeasured substitute must not be selectable"
    );
    assert_eq!(
        ChannelFlowKernel::Tuas.implementation(),
        Implementation::Substituted
    );
}

// ---------------------------------------------------------------------------
// Component gates — one per substitution in the §5 map
// ---------------------------------------------------------------------------

/// Runs one component's parity gate, or reports why it could not.
///
/// Both solves are reduced to the same three quantities, so every component
/// gate is the same comparison with a different kernel selected. When the
/// substituted path exists, this grows a second solve and a comparison; the
/// tolerances and reporting are already in place.
fn run_component_gate(component: Component) {
    let what = format!("{component:?} parity");
    let Some(_reference) = support::solve_iaea3d_reference() else {
        report_solver_not_wired(&what);
        return;
    };
    // The substituted solve goes here once the component exists. Until then,
    // reaching this line would mean a substitution was written without a gate,
    // which is precisely what the scoping rule forbids.
    assert!(
        !component.parity_status().is_accepted(),
        "{component:?} claims to have passed a gate this harness has never run"
    );
    println!("SKIP {what}: no substituted implementation exists yet");
}

#[test]
#[ignore = "no substituted implementation yet; see substituted::channel_flow"]
fn channel_flow_reproduces_the_reference() {
    run_component_gate(Component::ChannelFlow);
}

#[test]
#[ignore = "no substituted implementation yet; see substituted::drift_flux"]
fn drift_flux_reproduces_the_reference() {
    run_component_gate(Component::DriftFlux);
}

#[test]
#[ignore = "no substituted implementation yet; see substituted::chf"]
fn critical_heat_flux_reproduces_the_reference() {
    run_component_gate(Component::CriticalHeatFlux);
}

#[test]
#[ignore = "no substituted implementation yet; see substituted::fuel_rod"]
fn fuel_rod_reproduces_the_reference() {
    run_component_gate(Component::FuelRod);
}

#[test]
#[ignore = "no substituted implementation yet; see substituted::kinetics"]
fn kinetics_reproduces_the_reference() {
    run_component_gate(Component::Kinetics);
}

#[test]
#[ignore = "no substituted implementation yet; see substituted::cross_sections"]
fn cross_sections_reproduce_the_reference() {
    run_component_gate(Component::CrossSections);
}

#[test]
#[ignore = "no substituted implementation yet; see substituted::linear_solver"]
fn linear_solver_reproduces_the_reference() {
    run_component_gate(Component::LinearSolver);
}

#[test]
#[ignore = "waiting on both paths; see support::solve_iaea3d_reference"]
fn a_fully_substituted_solve_reproduces_the_reference_k_eff() {
    // The end-to-end gate: every component substituted at once. It is not a
    // replacement for the per-component gates — if it fails, it says nothing
    // about which substitution caused it, which is the whole reason §1
    // requires them one at a time.
    let Some(reference) = support::solve_iaea3d_reference() else {
        report_solver_not_wired("fully substituted solve");
        return;
    };
    let _ = reference;
    let _ = tolerance::SUBSTITUTION_K_EFF_ABS;
    println!("SKIP fully substituted solve: no substituted path exists yet");
}
