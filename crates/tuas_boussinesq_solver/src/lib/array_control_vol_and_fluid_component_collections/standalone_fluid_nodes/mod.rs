/// deals with fluid nodes in the core region
pub mod core_fluid_node;

/// deals with fluid nodes as if they were in a shell region 
/// that means they are exposed to an inner region and an outer region
pub mod shell_fluid_node;

use uom::si::f64::*;
use uom::ConstZero;
use ndarray::*;
use uom::si::thermodynamic_temperature::kelvin;
use openfoam_basic_lib::matrix::SquareMatrix;

use crate::tuas_lib_error::TuasLibError;

/// Solves for a temperature vector given a conductance matrix and power vector.
///
/// Uses the pure-Rust `SquareMatrix` LU solver from `openfoam-basic-lib`,
/// eliminating the system BLAS (OpenBLAS/Intel-MKL) dependency for this path.
#[inline]
pub fn solve_conductance_matrix_power_vector(
    thermal_conductance_matrix: Array2<ThermalConductance>,
    power_vector: Array1<Power>)
-> Result<Array1<ThermodynamicTemperature>, TuasLibError>{

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

    // LU solve — infallible (guards against singularity internally)
    let sol = mat.solve(&rhs);

    let temperature_vector: Array1<ThermodynamicTemperature> =
        Array1::from_iter(sol.into_iter().map(|f| ThermodynamicTemperature::new::<kelvin>(f)));

    Ok(temperature_vector)
}
