//! Regression tests — freeze the CURRENT output of a fixed transient RICHARDS
//! case so future changes that alter results are caught.
//!
//! IMPORTANT: these baselines are **not validated**. They record what the code
//! produces today, not what is physically correct or what PFLOTRAN produces.
//! A change that trips these tests is not necessarily a bug — it means the
//! numerics changed and the baseline must be re-reviewed (and, ideally,
//! re-derived once a validated reference exists, bead op-v6s.9).

use outram_park_fork_pflotran::io::parse_deck;
use outram_park_fork_pflotran::RichardsSimulation;

/// Fixed mid-transient case: 10-cell column, XMIN wet, 1 hour of simulated time.
const REGRESSION_DECK: &str = "\
GRID
  STRUCTURED 10 1 1
  DXYZ 0.1 1.0 1.0
END
MATERIAL
  POROSITY 0.35
  PERMEABILITY 1.0e-12
END
CHARACTERISTIC_CURVES
  MODEL VAN_GENUCHTEN
  ALPHA 1.0e-4
  N 2.0
  RESIDUAL_SATURATION 0.1
END
INITIAL_PRESSURE 90000.0
BOUNDARY_CONDITION
  LOCATION XMIN
  DIRICHLET_PRESSURE 101325.0
END
TIME
  FINAL_TIME 3600.0
  INITIAL_DT 60.0
  MAX_DT 600.0
END
";

/// Baseline captured 2026-07-22 (release build) from the deck above. Frozen
/// cell pressures (Pa) at the final time. Regenerate with the `print_baseline`
/// helper if an intended numerics change moves them.
const BASELINE_PRESSURE: [f64; 10] = [
    100932.772672,
    100083.701405,
    99073.351019,
    97841.903275,
    96328.093064,
    94528.632507,
    92645.522856,
    91161.814984,
    90393.149689,
    90136.692839,
];
/// Frozen liquid saturations at the final time (same run).
const BASELINE_SATURATION: [f64; 10] = [
    0.999309, 0.993145, 0.978018, 0.949920, 0.905084, 0.844360, 0.779689, 0.731225, 0.707463,
    0.699763,
];

#[test]
fn transient_column_matches_frozen_baseline() {
    let deck = parse_deck(REGRESSION_DECK).unwrap();
    let mut sim = RichardsSimulation::from_input_deck(&deck).unwrap();
    let report = sim.run().unwrap();

    // Timestepping is deterministic for this case.
    assert_eq!(report.steps, 10, "step count drifted from baseline");
    // Newton work is solver-internal; keep a loose guard rather than an exact
    // freeze so benign Krylov changes don't trip the regression.
    assert!(
        report.total_newton_iterations > 0 && report.total_newton_iterations <= 80,
        "Newton iteration count {} outside expected band",
        report.total_newton_iterations
    );

    let p = sim.pressure();
    let s = sim.saturation();
    for i in 0..10 {
        assert!(
            (p[i] - BASELINE_PRESSURE[i]).abs() < 1.0e-2,
            "pressure regression at cell {i}: {} vs baseline {}",
            p[i],
            BASELINE_PRESSURE[i]
        );
        assert!(
            (s[i] - BASELINE_SATURATION[i]).abs() < 1.0e-5,
            "saturation regression at cell {i}: {} vs baseline {}",
            s[i],
            BASELINE_SATURATION[i]
        );
    }

    // Sanity: monotone-decreasing saturation away from the wet XMIN boundary.
    for w in s.windows(2) {
        assert!(w[1] <= w[0] + 1.0e-9, "non-monotone saturation profile");
    }
}

/// Helper (ignored by default) to regenerate the frozen baseline above.
/// Run with: `cargo test -p outram-park-fork-pflotran --test regression --release
/// -- --ignored --nocapture`.
#[test]
#[ignore = "prints a fresh baseline; not an assertion"]
fn print_baseline() {
    let deck = parse_deck(REGRESSION_DECK).unwrap();
    let mut sim = RichardsSimulation::from_input_deck(&deck).unwrap();
    let report = sim.run().unwrap();
    eprintln!("steps = {}", report.steps);
    eprintln!("total_newton = {}", report.total_newton_iterations);
    eprint!("pressures = [");
    for p in sim.pressure() {
        eprint!("{p:.6}, ");
    }
    eprintln!("]");
    eprint!("saturations = [");
    for s in sim.saturation() {
        eprint!("{s:.6}, ");
    }
    eprintln!("]");
}
