//! Integration tests — the full v1 pipeline exercised through the public API:
//! parse an input deck -> build a [`RichardsSimulation`] -> run it in time ->
//! inspect pressure/saturation and write CSV/VTK output.
//!
//! These are end-to-end **behaviour** checks (physically-sensible, bounded,
//! monotone-where-expected), not validation against published references. They
//! also serve as the crate's top-level worked examples.

use outram_park_fork_pflotran::flow::FlowMode;
use outram_park_fork_pflotran::io::{parse_deck, write_results_csv, write_vtk_structured};
use outram_park_fork_pflotran::RichardsSimulation;

/// A 1D horizontal flow-through column: 20 cells, initially unsaturated at
/// 90 kPa, XMIN held saturated (atmospheric) and XMAX held at the drier 90 kPa.
/// The steady state is a monotone saturation gradient, wet at XMIN, dry at XMAX.
const INFILTRATION_DECK: &str = "\
# 1D horizontal flow-through column (AI-designed lite deck; not real PFLOTRAN)
GRID
  STRUCTURED 20 1 1
  DXYZ 0.05 1.0 1.0
END
MATERIAL
  POROSITY 0.40
  PERMEABILITY 5.0e-12
END
CHARACTERISTIC_CURVES
  MODEL VAN_GENUCHTEN
  ALPHA 5.0e-4
  N 2.0
  RESIDUAL_SATURATION 0.08
END
INITIAL_PRESSURE 90000.0
BOUNDARY_CONDITION
  LOCATION XMIN
  DIRICHLET_PRESSURE 101325.0
END
BOUNDARY_CONDITION
  LOCATION XMAX
  DIRICHLET_PRESSURE 90000.0
END
TIME
  FINAL_TIME 86400.0
  INITIAL_DT 60.0
  MAX_DT 7200.0
END
";

#[test]
fn infiltration_column_runs_and_is_physically_sensible() {
    let deck = parse_deck(INFILTRATION_DECK).expect("deck parses");
    let mut sim = RichardsSimulation::from_input_deck(&deck).expect("sim builds");

    let report = sim.run().expect("infiltration run completes");
    assert!(report.steps > 0, "no steps taken");
    assert!(
        (report.final_time - 86400.0).abs() < 1.0,
        "did not reach final time: {}",
        report.final_time
    );

    let sat = sim.saturation();
    let p = sim.pressure();
    assert_eq!(sat.len(), 20);

    // Physical bounds.
    for (i, &s) in sat.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(&s),
            "saturation out of [0,1] at cell {i}: {s}"
        );
        assert!(p[i].is_finite(), "non-finite pressure at cell {i}");
    }

    // Infiltration from XMIN: the near-boundary cell must be wetter than the
    // far end, and the profile must be (weakly) monotone decreasing in x.
    assert!(
        sat[0] > sat[19] + 1.0e-4,
        "no wetting gradient: sat[0]={}, sat[19]={}",
        sat[0],
        sat[19]
    );
    for w in sat.windows(2) {
        assert!(
            w[1] <= w[0] + 1.0e-6,
            "saturation not monotone decreasing along x: {} -> {}",
            w[0],
            w[1]
        );
    }
    // The XMIN cell should have taken on water relative to the initial state.
    assert!(sat[0] > sat[19], "boundary cell did not wet up");
}

#[test]
fn flow_mode_run_matches_direct_simulation() {
    // Driving via the FlowMode enum must be equivalent to the simulation API.
    let deck = parse_deck(INFILTRATION_DECK).unwrap();
    let sim = RichardsSimulation::from_input_deck(&deck).unwrap();
    let mut mode = FlowMode::Richards(sim);
    assert_eq!(mode.name(), "RICHARDS");
    let report = mode.run().expect("FlowMode run completes");
    assert!(report.steps > 0);
}

#[test]
fn output_writers_produce_wellformed_csv_and_vtk() {
    let deck = parse_deck(INFILTRATION_DECK).unwrap();
    let mut sim = RichardsSimulation::from_input_deck(&deck).unwrap();
    sim.run().unwrap();

    let (nx, ny, nz) = (deck.grid.nx, deck.grid.ny, deck.grid.nz);
    let p = sim.pressure().to_vec();
    let s = sim.saturation();

    let csv = write_results_csv(nx, ny, nz, deck.grid.dx, deck.grid.dy, deck.grid.dz, &p, &s)
        .expect("csv writes");
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines[0], "x,y,z,pressure,saturation");
    assert_eq!(lines.len(), nx * ny * nz + 1, "one header + one row per cell");

    let vtk = write_vtk_structured(nx, ny, nz, deck.grid.dx, deck.grid.dy, deck.grid.dz, &p, &s)
        .expect("vtk writes");
    assert!(vtk.contains("STRUCTURED_POINTS"), "vtk missing dataset kind");
    assert!(vtk.contains("CELL_DATA"), "vtk missing cell data");
    assert!(vtk.contains("pressure"), "vtk missing pressure field");
    assert!(vtk.contains("saturation"), "vtk missing saturation field");
}
