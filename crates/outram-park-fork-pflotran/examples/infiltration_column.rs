//! Worked example — a 1D variably-saturated infiltration/flow-through column.
//!
//! Run with:
//! ```text
//! cargo run -p outram-park-fork-pflotran --example infiltration_column --release
//! ```
//!
//! This is the crate's primary top-to-bottom entry point: it shows the whole v1
//! RICHARDS workflow using only the public API —
//!
//!   1. write a (lite, AI-designed) input deck as text,
//!   2. parse it into an [`InputDeck`](outram_park_fork_pflotran::io),
//!   3. build a [`RichardsSimulation`] from it,
//!   4. run it to the final time,
//!   5. print the saturation profile and write CSV + legacy-VTK output.
//!
//! > Verification-only, untrusted AI draft: the deck format is NOT real PFLOTRAN
//! > syntax and the result has not been validated against any reference. See the
//! > crate README and `docs/ai-directed-decisions.md`.

use outram_park_fork_pflotran::io::{parse_deck, write_results_csv, write_vtk_structured};
use outram_park_fork_pflotran::RichardsSimulation;

const DECK: &str = "\
# 1D horizontal flow-through column: wet left (XMIN), drier right (XMAX).
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let deck = parse_deck(DECK)?;
    let mut sim = RichardsSimulation::from_input_deck(&deck)?;

    let report = sim.run()?;
    println!(
        "RICHARDS run: {} steps, {} total Newton iterations, t_final = {:.0} s",
        report.steps, report.total_newton_iterations, report.final_time
    );

    let p = sim.pressure();
    let s = sim.saturation();
    println!("\n  cell        x [m]   pressure [Pa]   saturation [-]");
    for i in 0..deck.grid.nx {
        let x = (i as f64 + 0.5) * deck.grid.dx;
        println!("  {i:>4}   {x:>10.4}   {:>13.2}   {:>12.5}", p[i], s[i]);
    }

    // Write output files to the system temp directory (keeps the repo clean).
    let (nx, ny, nz) = (deck.grid.nx, deck.grid.ny, deck.grid.nz);
    let csv = write_results_csv(nx, ny, nz, deck.grid.dx, deck.grid.dy, deck.grid.dz, p, &s)?;
    let vtk = write_vtk_structured(nx, ny, nz, deck.grid.dx, deck.grid.dy, deck.grid.dz, p, &s)?;
    let dir = std::env::temp_dir();
    let csv_path = dir.join("pflotran_infiltration.csv");
    let vtk_path = dir.join("pflotran_infiltration.vtk");
    std::fs::write(&csv_path, csv)?;
    std::fs::write(&vtk_path, vtk)?;
    println!("\nWrote {}", csv_path.display());
    println!("Wrote {}", vtk_path.display());

    Ok(())
}
