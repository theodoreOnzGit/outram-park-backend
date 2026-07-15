//! Enumerate the numerical-method catalogue KOVAN codegen targets.
//!
//! Run with: `cargo run -p kovan-codegen --example catalogue`

use kovan_codegen::{LinearSolver, Method, OdeSolver, RootFinder};

fn main() {
    let sample = [
        Method::Root(RootFinder::Brent),
        Method::Linear(LinearSolver::ConjugateGradient),
        Method::Ode(OdeSolver::DormandPrince),
    ];
    for m in sample {
        println!("{m:?}");
    }
}
