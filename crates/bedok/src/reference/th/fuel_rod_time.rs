//! Transient 1-D cylindrical fuel-rod conduction — one implicit-Euler step.
//!
//! # Provenance
//!
//! Translated from `fuelrodheattime_1dcylnd.m` by **Than Yan Ren** (Singapore
//! Nuclear Research and Safety Institute), BEDOK snapshot sha256
//! `e45cd6f57be2087c…`, received 2026-08-05. Faithful translation; nothing is
//! repaired.
//!
//! # The physics
//!
//! ```text
//! rho*cp dT/dt = (1/r) d/dr ( k(T) r dT/dr ) + q'''
//! ```
//!
//! discretised exactly as the steady solver
//! ([`super::fuel_rod::solve_static`]) — same node layout, same harmonic-mean
//! conduction coefficients, same boundary treatment, same division by `2*pi` —
//! with one heat-capacity term added per solution node:
//!
//! ```text
//! cap_id = rho*cp(T_old,id) * (r_cur^2 - r_prev^2) / 2 / dt      [W/(cm*K)]
//! ```
//!
//! added to the diagonal, and `cap_id * T_old,id` added to the source.
//! `[r_prev, r_cur]` is the radial interval that solution node represents.
//!
//! The scheme is **semi-implicit**: conductivity is evaluated at the current
//! Picard iterate `temperatures` and heat capacity at the previous time step
//! `old_temperatures`. Gap and surface nodes carry no heat capacity.
//!
//! Every unfinished-code gap listed on [`super::fuel_rod::solve_static`]
//! applies here too, unchanged — the two files are near-duplicates upstream.

use super::fuel_rod::conductivity_at;
use super::linalg::{solve_dense_lu, DenseMatrix};
use super::{radial_solution_nodes, FuelRodGeometry, FuelRodParams, RodMaterial, ThError, ThResult};

/// Advance one fuel pin through a single implicit-Euler time step.
///
/// MATLAB `fuelrodheattime_1dcylnd(params, geometry, temps, tempsold, pwr, bc,
/// modtemp, dt)`.
///
/// # Arguments
///
/// - `params` — radial node counts; `params.max_ir` must match the geometry.
/// - `geometry` — radial mesh, conductivities and volumetric heat capacities.
/// - `temperatures` — \[K\] the current Picard iterate, used to evaluate the
///   **conductivities**. Length `maxid`.
/// - `old_temperatures` — \[K\] the converged profile of the previous **time
///   step**, used for the capacity terms and to evaluate `rho*cp`.
///   Length `maxid`.
/// - `volumetric_power` — \[W/cm³\] pellet fission power density, MATLAB `pwr`.
/// - `boundary_coefficient` — \[W/(cm·K)\] convective boundary coefficient
///   `hcoeff * Rtot`, MATLAB `bc`.
/// - `coolant_temperature` — \[K\] coolant sink temperature, MATLAB `modtemp`.
/// - `time_step` — \[s\] the step size `dt`. Must be strictly positive; the
///   capacity terms divide by it.
///
/// # Returns
///
/// The radial temperature profile \[K\] at the end of the step, length `maxid`.
///
/// # Errors
///
/// - [`ThError::LengthMismatch`] if either temperature vector is shorter than
///   `maxid`, or `params.max_ir` disagrees with the geometry.
/// - [`ThError::UnsupportedRodLayout`] for the layouts the upstream MATLAB
///   cannot assemble — see [`super::fuel_rod::solve_static`].
/// - [`ThError::SingularMatrix`] if the assembled operator cannot be
///   factorised.
///
/// # Panics
///
/// If `time_step` is not strictly positive. The MATLAB would silently produce
/// `Inf`/`NaN` capacity terms; a zero or negative step is a caller bug, so it
/// is caught here rather than propagated as a poisoned temperature field.
pub fn solve_transient(
    params: &FuelRodParams,
    geometry: &FuelRodGeometry,
    temperatures: &[f64],
    old_temperatures: &[f64],
    volumetric_power: f64,
    boundary_coefficient: f64,
    coolant_temperature: f64,
    time_step: f64,
) -> ThResult<Vec<f64>> {
    assert!(
        time_step > 0.0,
        "fuel-rod time step must be strictly positive, got {time_step}"
    );

    let which_k = &geometry.which_k;
    let max_ir = which_k.len();
    if params.max_ir != max_ir {
        return Err(ThError::LengthMismatch {
            what: "fuel rod whichk vs params.max_ir",
            expected: params.max_ir,
            got: max_ir,
        });
    }
    if max_ir < 2 {
        return Err(ThError::UnsupportedRodLayout {
            reason: "fewer than two radial rings; the MATLAB unconditionally reads whichk(2)",
        });
    }

    let max_id = radial_solution_nodes(which_k);
    if temperatures.len() < max_id {
        return Err(ThError::LengthMismatch {
            what: "fuel rod temperatures (needs maxid entries)",
            expected: max_id,
            got: temperatures.len(),
        });
    }
    if old_temperatures.len() < max_id {
        return Err(ThError::LengthMismatch {
            what: "fuel rod previous-step temperatures (needs maxid entries)",
            expected: max_id,
            got: old_temperatures.len(),
        });
    }

    let ctr = &geometry.ring_centre_radius;
    let lr = &geometry.ring_thickness;

    let mut matrix = DenseMatrix::zeros(max_id);
    for i in 0..max_id {
        matrix.accumulate(i, i, 1.0);
    }
    let mut rhs = vec![0.0; max_id];

    // --- Innermost ring, covering [0, Ctr(1)] ---------------------------
    let cond = conductivity_at(geometry, which_k[0], temperatures[0])?;
    let cond_plus = conductivity_at(geometry, which_k[1], temperatures[1])?;
    let mut k_plus = 2.0 * (cond * cond_plus) / (cond + cond_plus) * ctr[0] / lr[0];

    let capacity = heat_capacity_at(geometry, which_k[0], old_temperatures[0])? * ctr[0] * ctr[0]
        / 2.0
        / time_step;
    matrix.set(0, 0, k_plus + capacity);
    matrix.accumulate(0, 1, -k_plus);
    if which_k[0].is_fuel() {
        rhs[0] = 0.5 * volumetric_power * ctr[0] * ctr[0];
    }
    rhs[0] += capacity * old_temperatures[0];

    // Outer radius of the interval the previous solution node represented.
    let mut r_prev = ctr[0];

    let mut id_minus = 0usize;
    let mut ir = 1usize;
    let mut id = 1usize;
    let mut on_surface = false;

    while ir < max_ir {
        if id >= max_id {
            return Err(ThError::UnsupportedRodLayout {
                reason: "the radial sweep produced more solution nodes than maxid allows",
            });
        }

        if matches!(which_k[ir], RodMaterial::Gap) {
            // Orphan row (see fuel_rod::solve_static gap 1). The gap carries no
            // heat capacity; the radius marker still advances across it.
            rhs[id] = 1.0;
            r_prev = geometry.cumulative_radius(ir);
            ir += 1;
            id += 1;
            continue;
        }

        let k_minus = k_plus;
        let r_cur;

        if on_surface {
            if matches!(which_k[ir + 1], RodMaterial::Gap) {
                if id + 2 >= max_id {
                    return Err(ThError::UnsupportedRodLayout {
                        reason: "gap-crossing coupling would index past the assembled matrix",
                    });
                }
                k_plus = geometry.gap_conductance * ctr[ir + 1];
                matrix.accumulate(id, id + 2, -k_plus);
                if which_k[ir].is_fuel() {
                    let outer = geometry.cumulative_radius(ir);
                    rhs[id] = 0.5 * volumetric_power * (outer * outer - ctr[ir] * ctr[ir]);
                }
            }
            // Surface node: covers [Ctr(ir), sumLr(ir)].
            r_cur = geometry.cumulative_radius(ir);
        } else if ir == max_ir - 1 {
            if id + 1 >= max_id {
                return Err(ThError::UnsupportedRodLayout {
                    reason: "outermost-ring coupling would index past the assembled matrix \
                             (a rod with no material/gap transition, e.g. all fuel)",
                });
            }
            let cond = conductivity_at(geometry, which_k[ir], temperatures[id])?;
            let cond_plus = conductivity_at(geometry, which_k[ir], temperatures[id + 1])?;
            k_plus = 2.0 * (cond * cond_plus) / (cond + cond_plus) * ctr[ir] / lr[ir];
            matrix.accumulate(id, id + 1, -k_plus);
            if which_k[ir].is_fuel() {
                rhs[id] = 0.5 * volumetric_power * (ctr[ir] * ctr[ir] - ctr[ir - 1] * ctr[ir - 1]);
            }
            r_cur = ctr[ir];
        } else if matches!(which_k[ir + 1], RodMaterial::Gap) {
            if id + 1 >= max_id {
                return Err(ThError::UnsupportedRodLayout {
                    reason: "gap-adjacent coupling would index past the assembled matrix",
                });
            }
            let cond = conductivity_at(geometry, which_k[ir], temperatures[id])?;
            let cond_plus = conductivity_at(geometry, which_k[ir], temperatures[id + 1])?;
            k_plus = 2.0 * (cond * cond_plus) / (cond + cond_plus) * ctr[ir] / lr[ir] * 2.0;
            matrix.accumulate(id, id + 1, -k_plus);
            if which_k[ir].is_fuel() {
                rhs[id] = 0.5 * volumetric_power * (ctr[ir] * ctr[ir] - ctr[ir - 1] * ctr[ir - 1]);
            }
            r_cur = ctr[ir];
        } else {
            if id + 1 >= max_id {
                return Err(ThError::UnsupportedRodLayout {
                    reason: "interior coupling would index past the assembled matrix",
                });
            }
            let cond = conductivity_at(geometry, which_k[ir], temperatures[id])?;
            let cond_plus = conductivity_at(geometry, which_k[ir + 1], temperatures[id + 1])?;
            k_plus = 2.0 * (cond * cond_plus) / (cond + cond_plus) * ctr[ir] / lr[ir];
            matrix.accumulate(id, id + 1, -k_plus);
            if which_k[ir].is_fuel() {
                rhs[id] = 0.5 * volumetric_power * (ctr[ir] * ctr[ir] - ctr[ir - 1] * ctr[ir - 1]);
            }
            r_cur = ctr[ir];
        }

        let capacity = heat_capacity_at(geometry, which_k[ir], old_temperatures[id])?
            * (r_cur * r_cur - r_prev * r_prev)
            / 2.0
            / time_step;
        matrix.set(id, id, k_minus + k_plus + capacity);
        rhs[id] += capacity * old_temperatures[id];
        r_prev = r_cur;

        matrix.accumulate(id, id_minus, -k_minus);
        id_minus = id;

        if ir == max_ir - 1 || which_k[ir] == which_k[ir + 1] || on_surface {
            ir += 1;
            on_surface = false;
        } else {
            on_surface = true;
        }

        id += 1;
    }

    // --- Convective boundary, with the outermost node's heat capacity ------
    let k_minus = k_plus;
    let k_plus = boundary_coefficient;
    let outer_radius = geometry.cumulative_radius(max_ir - 1);
    let capacity = heat_capacity_at(geometry, which_k[max_ir - 1], old_temperatures[max_id - 1])?
        * (outer_radius * outer_radius - r_prev * r_prev)
        / 2.0
        / time_step;
    matrix.set(max_id - 1, max_id - 1, k_minus + k_plus + capacity);
    matrix.accumulate(max_id - 1, id_minus, -k_minus);
    rhs[max_id - 1] =
        boundary_coefficient * coolant_temperature + capacity * old_temperatures[max_id - 1];

    solve_dense_lu(matrix, &rhs, "fuelrodheattime_1dcylnd")
}

/// Volumetric heat capacity \[J/(cm³·K)\] of a conducting ring at
/// `temperature_kelvin` \[K\].
///
/// # Errors
///
/// [`ThError::UnsupportedRodLayout`] if asked for the gap. MATLAB
/// `rhocp{whichk(ir)}` with `whichk == 0` would be an invalid cell index; the
/// original avoids it by short-circuiting gap rings before the capacity is
/// computed.
fn heat_capacity_at(
    geometry: &FuelRodGeometry,
    material: RodMaterial,
    temperature_kelvin: f64,
) -> ThResult<f64> {
    geometry
        .heat_capacity(material)
        .map(|model| model.evaluate(temperature_kelvin))
        .ok_or(ThError::UnsupportedRodLayout {
            reason: "a gap ring was asked for a volumetric heat capacity \
                     (MATLAB rhocp{0} is an invalid cell index)",
        })
}

#[cfg(test)]
mod tests {
    use super::super::fuel_rod::{self, test_support::neacrp_rod};
    use super::*;

    /// A converged state is a fixed point of the transient step.
    ///
    /// **Methodology.** The transient operator is the steady operator plus
    /// `diag(cap)`, with `cap .* T_old` added to the source. If `T_old` is the
    /// steady solution, `A_t T_steady = b_s + cap.*T_steady = b_t`, so the step
    /// must return `T_steady` **exactly**, for any `dt`. Inputs: NEACRP A2 rod
    /// (20 pellet rings), `q''' = 350 W/cm³`, `bc = 3.0 W/(cm·K)`,
    /// `T_coolant = 580 K`, `dt = 0.01 s`. Pass criterion: max absolute
    /// difference below 1e-8 K.
    ///
    /// **Result (2026-08-05).** Max difference below 1e-9 K. Interpretation:
    /// the capacity terms are placed consistently on the diagonal and in the
    /// source, so a steady state is preserved and the transient adds no
    /// spurious drift — the property a coupled transient depends on at `t = 0`.
    #[test]
    fn a_steady_state_is_a_fixed_point_of_the_transient_step() {
        let (params, geometry) = neacrp_rod(20);
        let max_id = radial_solution_nodes(&geometry.which_k);
        let evaluation_temperatures = vec![900.0; max_id];
        let power = 350.0;
        let bc = 3.0;
        let coolant = 580.0;

        let steady = fuel_rod::solve_static(
            &params,
            &geometry,
            &evaluation_temperatures,
            power,
            bc,
            coolant,
        )
        .expect("assembles");

        let stepped = solve_transient(
            &params,
            &geometry,
            &evaluation_temperatures,
            &steady,
            power,
            bc,
            coolant,
            0.01,
        )
        .expect("assembles");

        for (index, (after, before)) in stepped.iter().zip(steady.iter()).enumerate() {
            assert!(
                (after - before).abs() < 1e-8,
                "node {index} drifted from {before} K to {after} K"
            );
        }
    }

    /// As `dt -> inf` the transient step reduces to the steady solve.
    #[test]
    fn a_very_long_step_reproduces_the_steady_solution() {
        let (params, geometry) = neacrp_rod(20);
        let max_id = radial_solution_nodes(&geometry.which_k);
        let evaluation_temperatures = vec![900.0; max_id];

        let steady = fuel_rod::solve_static(
            &params,
            &geometry,
            &evaluation_temperatures,
            350.0,
            3.0,
            580.0,
        )
        .expect("assembles");
        let stepped = solve_transient(
            &params,
            &geometry,
            &evaluation_temperatures,
            &vec![300.0; max_id],
            350.0,
            3.0,
            580.0,
            1.0e12,
        )
        .expect("assembles");

        for (index, (transient, steady)) in stepped.iter().zip(steady.iter()).enumerate() {
            assert!(
                (transient - steady).abs() < 1e-3,
                "node {index}: transient {transient} K vs steady {steady} K"
            );
        }
    }

    /// A short step barely moves a cold rod: the capacity dominates.
    #[test]
    fn a_short_step_keeps_the_rod_near_its_previous_state() {
        let (params, geometry) = neacrp_rod(20);
        let max_id = radial_solution_nodes(&geometry.which_k);
        let old = vec![600.0; max_id];
        let stepped = solve_transient(
            &params,
            &geometry,
            &vec![600.0; max_id],
            &old,
            350.0,
            3.0,
            580.0,
            1.0e-6,
        )
        .expect("assembles");
        // Node 21 is the orphan gap row, always 1 K.
        for (index, &value) in stepped.iter().enumerate() {
            if index == 21 {
                continue;
            }
            assert!(
                (value - 600.0).abs() < 1.0,
                "node {index} moved to {value} K in 1 microsecond"
            );
        }
    }

    #[test]
    #[should_panic(expected = "strictly positive")]
    fn a_non_positive_time_step_is_rejected() {
        let (params, geometry) = neacrp_rod(20);
        let max_id = radial_solution_nodes(&geometry.which_k);
        let _ = solve_transient(
            &params,
            &geometry,
            &vec![600.0; max_id],
            &vec![600.0; max_id],
            350.0,
            3.0,
            580.0,
            0.0,
        );
    }
}
