// Transient secondary-loop mass balance (Backward Euler) demonstration.
//
// Solves the implicit linear system  A · M^(n+1) = M^n  each timestep for the
// four control-volume masses (boiler, turbine, condenser, pump).
//
// The 4x4 dense solve uses outram-foam-basic-lib's pure-Rust `SquareMatrix`
// (an OpenFOAM `scalarSquareMatrix` LU port) instead of a BLAS/LAPACK linear
// solver -- the workspace policy is that the OpenFOAM ports remove the need for
// an external linear-algebra backend. `SquareMatrix::solve` performs Crout LU
// with scaled partial pivoting and is a direct drop-in for the former
// `Solve::solve` (ndarray) on a small system like this.

use outram_foam_basic_lib::prelude::SquareMatrix;

/// Format a length-4 mass vector for display, matching the old ndarray
/// pretty-print closely enough for the demonstration output.
fn fmt_vec(v: &[f64]) -> String {
    let items: Vec<String> = v.iter().map(|x| format!("{x}")).collect();
    format!("[{}]", items.join(", "))
}

#[test]
fn main_steam_turbtest() {
    // --- Simulation Parameters ---
    let delta_t: f64 = 0.1; // Time step (seconds)

    // Simplified flow constants (these would be derived from physical properties
    // and component characteristics in a more detailed model)
    let k1: f64 = 0.05; // Proportionality constant for flow from Pump to Boiler
    let k2: f64 = 0.04; // Proportionality constant for flow from Boiler to Turbine
    let k3: f64 = 0.06; // Proportionality constant for flow from Turbine to Condenser
    let k4: f64 = 0.055; // Proportionality constant for flow from Condenser to Pump

    // Initial masses in each control volume (kg)
    // These are arbitrary initial conditions for demonstration
    let m_b_n_initial: f64 = 100.0; // Mass in Boiler
    let m_t_n_initial: f64 = 10.0; // Mass in Turbine
    let m_c_n_initial: f64 = 50.0; // Mass in Condenser
    let m_p_n_initial: f64 = 5.0; // Mass in Pump

    // Define the vector of current masses (M^n)
    let mut m_n: Vec<f64> = vec![m_b_n_initial, m_t_n_initial, m_c_n_initial, m_p_n_initial];

    println!("Initial Masses (M^n):\n{}", fmt_vec(&m_n));
    println!("\n--- Simulating Transient Mass Balance ---");

    // --- Construct the Coefficient Matrix A for Implicit Solution ---
    // A * M^(n+1) = M^n
    //
    // Row-major fill of the same matrix the old `ndarray::array!` literal built:
    //   [1 + dt*k2,      0,          0,          -dt*k1   ]
    //   [-dt*k2,     1 + dt*k3,       0,           0       ]
    //   [ 0,        -dt*k3,      1 + dt*k4,        0       ]
    //   [ 0,          0,         -dt*k4,      1 + dt*k1    ]
    let mut a = SquareMatrix::new(4);
    a.set(0, 0, 1.0 + delta_t * k2);
    a.set(0, 3, -delta_t * k1);
    a.set(1, 0, -delta_t * k2);
    a.set(1, 1, 1.0 + delta_t * k3);
    a.set(2, 1, -delta_t * k3);
    a.set(2, 2, 1.0 + delta_t * k4);
    a.set(3, 2, -delta_t * k4);
    a.set(3, 3, 1.0 + delta_t * k1);

    println!("\nCoefficient Matrix A for implicit solution (A * M^(n+1) = M^n) is a 4x4 backward-Euler operator.");

    // --- Time Integration Loop (Example for a few steps) ---
    let num_steps = 10;
    for step in 0..num_steps {
        // Solve the linear system A * M^(n+1) = M^n for M^(n+1)
        match a.solve(&m_n) {
            Ok(m_n_plus_1) => {
                println!(
                    "\n--- Time Step {} (t = {:.1}s) ---",
                    step + 1,
                    (step + 1) as f64 * delta_t
                );
                println!(
                    "Masses at next time step (M^(n+1)):\n{}",
                    fmt_vec(&m_n_plus_1)
                );

                // Update current masses for the next iteration
                m_n = m_n_plus_1;
            }
            Err(e) => {
                eprintln!(
                    "\nError solving linear system at step {}: {:?}",
                    step + 1,
                    e
                );
                break;
            }
        }
    }

    println!("\n--- Important Considerations ---");
    println!("1. **Simplifying Assumptions**: The flow rate model (flow proportional to mass) is a significant simplification.");
    println!("   In a real Rankine cycle, flow rates are complex functions of pressures, temperatures, specific volumes,");
    println!("   valve positions, pump characteristics, and turbine characteristics.");
    println!("2. **Thermodynamic Properties**: `tampines-steam-tables` would be essential in a more realistic model.");
    println!("   You would use it to calculate properties like density and pressure based on mass and volume,");
    println!("   which then drive the actual mass flow rates.");
    println!("3. **Energy Balance**: A complete transient model would also include energy balance equations for each CV.");
    println!("   These would determine the temperatures and pressures, which are crucial for calculating realistic mass flow rates.");
    println!("4. **Control Volume Volumes**: For transient mass balance, the physical volume of each control volume (e.g., boiler drum volume)");
    println!("   is critical, as mass is stored as density * volume (M = ρV).");
    println!("5. **Stability and Accuracy**: The choice of time step (delta_t) and numerical method (Backward Euler) affects stability and accuracy.");
}
