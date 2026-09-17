//! ODE-solver code generation.
//!
//! Emits self-contained, documented Rust source for the initial-value-problem
//! integrators in the KOVAN catalogue (docs/kovan.md § "Numerical Methods → ODE
//! Solvers"). Each template file is both emitted verbatim by [`generate`] and
//! compiled into [`mod@reference`] so the tests exercise the exact emitted source.
//!
//! Fully generated: explicit Euler, explicit-midpoint RK2, classical RK4,
//! implicit backward Euler. Catalogued but not yet generated: Dormand–Prince
//! (adaptive RK45), Crank–Nicolson.

use crate::{CodegenError, OdeSolver};

/// The exact algorithm sources that [`generate`] emits, compiled for testing.
pub mod reference {
    /// Explicit Euler — see `templates/euler.rs`.
    pub mod euler {
        include!("templates/euler.rs");
    }
    /// Explicit-midpoint RK2 — see `templates/rk2.rs`.
    pub mod rk2 {
        include!("templates/rk2.rs");
    }
    /// Classical RK4 — see `templates/rk4.rs`.
    pub mod rk4 {
        include!("templates/rk4.rs");
    }
    /// Implicit backward Euler — see `templates/backward_euler.rs`.
    pub mod backward_euler {
        include!("templates/backward_euler.rs");
    }
}

/// Generate Rust source for the requested [`OdeSolver`].
///
/// # Errors
/// [`CodegenError::Unimplemented`] for Dormand–Prince and Crank–Nicolson, which
/// are catalogued but not yet generated.
pub fn generate(method: OdeSolver) -> Result<String, CodegenError> {
    let src = match method {
        OdeSolver::Euler => include_str!("templates/euler.rs"),
        OdeSolver::Rk2 => include_str!("templates/rk2.rs"),
        OdeSolver::Rk4 => include_str!("templates/rk4.rs"),
        OdeSolver::BackwardEuler => include_str!("templates/backward_euler.rs"),
        OdeSolver::DormandPrince => {
            return Err(CodegenError::Unimplemented("ODE solver: DormandPrince"))
        }
        OdeSolver::CrankNicolson => {
            return Err(CodegenError::Unimplemented("ODE solver: CrankNicolson"))
        }
    };
    Ok(src.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test problem dy/dt = y, y(0) = 1, exact solution y(t) = e^t.
    fn exp_rhs(_t: f64, y: f64) -> f64 {
        y
    }

    #[test]
    fn rk4_matches_exponential() {
        // 100 steps over [0, 1]; RK4 global error ~O(h^4) should be tiny.
        let traj = reference::rk4::rk4_integrate(exp_rhs, 0.0, 1.0, 0.01, 100);
        let y1 = traj.last().unwrap().1;
        assert!((y1 - std::f64::consts::E).abs() < 1e-8, "got {y1}");
    }

    #[test]
    fn rk2_beats_euler_at_same_step() {
        let e = std::f64::consts::E;
        let h = 0.01;
        let n = 100;
        let euler = reference::euler::euler_integrate(exp_rhs, 0.0, 1.0, h, n)
            .last()
            .unwrap()
            .1;
        let rk2 = reference::rk2::rk2_integrate(exp_rhs, 0.0, 1.0, h, n)
            .last()
            .unwrap()
            .1;
        assert!(
            (rk2 - e).abs() < (euler - e).abs(),
            "rk2 {rk2} euler {euler}"
        );
    }

    #[test]
    fn backward_euler_is_stable_on_stiff_decay() {
        // dy/dt = -1000 y, y(0) = 1. Explicit Euler at h = 0.01 would blow up
        // (|1 + h*lambda| = 9); backward Euler stays bounded and decays.
        let stiff = |_t: f64, y: f64| -1000.0 * y;
        let traj = reference::backward_euler::backward_euler_integrate(stiff, 0.0, 1.0, 0.01, 100)
            .unwrap();
        let y_end = traj.last().unwrap().1;
        assert!(y_end.is_finite() && y_end.abs() < 1e-2, "got {y_end}");
        // Monotone decay toward zero.
        for w in traj.windows(2) {
            assert!(w[1].1.abs() <= w[0].1.abs() + 1e-12);
        }
    }

    #[test]
    fn generate_is_deterministic() {
        for m in [
            OdeSolver::Euler,
            OdeSolver::Rk2,
            OdeSolver::Rk4,
            OdeSolver::BackwardEuler,
        ] {
            assert_eq!(generate(m).unwrap(), generate(m).unwrap());
        }
    }

    #[test]
    fn unimplemented_variants_error() {
        assert!(generate(OdeSolver::DormandPrince).is_err());
        assert!(generate(OdeSolver::CrankNicolson).is_err());
    }
}
