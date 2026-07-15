//! Enumerate the numerical-method catalogue and generate a sample kernel.
//!
//! Run with: `cargo run -p kovan-codegen --example catalogue`

use kovan_codegen::{generate, LinearSolver, Method, OdeSolver, PdeScheme, RootFinder};

fn main() {
    // A cross-section of the catalogue, including one entry per family.
    let sample = [
        Method::Root(RootFinder::Brent),
        Method::Linear(LinearSolver::Lu),
        Method::Ode(OdeSolver::Rk4),
        Method::Pde(PdeScheme::Poisson1dFiniteDifference),
        // A catalogued-but-not-yet-generated entry, to show the error path.
        Method::Linear(LinearSolver::Gmres),
    ];

    for m in sample {
        match generate(m) {
            Ok(src) => {
                let lines = src.lines().count();
                println!("# {m:?}  ({lines} lines generated)");
            }
            Err(e) => println!("# {m:?}  -> {e}"),
        }
    }

    // Emit one full kernel to stdout so the deterministic output is visible.
    println!("\n----- generated source for Method::Ode(OdeSolver::Rk4) -----\n");
    println!("{}", generate(Method::Ode(OdeSolver::Rk4)).unwrap());
}
