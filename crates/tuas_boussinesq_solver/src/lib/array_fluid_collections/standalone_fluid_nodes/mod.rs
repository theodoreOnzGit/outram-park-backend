//! Standalone single-material fluid-node arrays for high-Peclet-number flow.
//!
//! These are one-dimensional arrays of fluid control volumes advanced without
//! axial conduction (advection-dominated, high Peclet number). Each array is
//! coupled radially to solid surfaces through a thermal-conductance array
//! (W/K per node) and terminated by a [`SingleCVNode`](crate::single_control_vol::SingleCVNode)
//! at each end so it can link to neighbouring control volumes; temperatures are
//! in kelvin.
//!
//! What belongs here: the timestep advancers — `core_fluid_node` (fluid coupled
//! to a single surrounding solid, e.g. a pipe wall) and `shell_fluid_node`
//! (fluid coupled to both an inner and an outer solid) — and the shared
//! conductance-matrix / power-vector temperature solver in this file. Solid-only
//! node arrays live in the sibling `standalone_solid_nodes` module.
/// deals with fluid nodes in the core region
pub mod core_fluid_node;

/// deals with fluid nodes as if they were in a shell region
/// that means they are exposed to an inner region and an outer region
pub mod shell_fluid_node;

use uom::si::f64::*;
use uom::ConstZero;
use ndarray::*;
use uom::si::thermodynamic_temperature::kelvin;
use crate::matrix::SquareMatrix;

use crate::tuas_lib_error::TuasLibError;

/// Solves for a temperature vector given a conductance matrix and power vector.
///
/// Uses the pure-Rust `SquareMatrix` LU solver inlined into this crate
/// (`crate::matrix`), so this path has no `outram-foam-basic-lib` / system BLAS
/// (OpenBLAS/Intel-MKL) dependency.
#[inline]
pub fn solve_conductance_matrix_power_vector(
    thermal_conductance_matrix: Array2<ThermalConductance>,
    power_vector: Array1<Power>,
) -> Result<Array1<ThermodynamicTemperature>, TuasLibError> {
    let n = power_vector.len();

    // Strip uom units → plain f64 and fill a SquareMatrix
    let mut mat = SquareMatrix::new(n);
    for i in 0..n {
        for j in 0..n {
            mat.set(i, j, thermal_conductance_matrix[[i, j]].value);
        }
    }
    let rhs: Vec<f64> = power_vector.iter().map(|p| p.value).collect();

    // Compile-time unit safety check (same as before)
    let _unit_check: Power =
        power_vector[0] + thermal_conductance_matrix[[0, 0]] * ThermodynamicTemperature::ZERO;

    let sol = mat
        .solve(&rhs)
        .map_err(|e| TuasLibError::GenericStringError(format!("SquareMatrix LU solve: {e}")))?;

    let temperature_vector: Array1<ThermodynamicTemperature> = Array1::from_iter(
        sol.into_iter()
            .map(|f| ThermodynamicTemperature::new::<kelvin>(f)),
    );

    Ok(temperature_vector)
}
