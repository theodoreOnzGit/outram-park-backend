//! Steady 1-D cylindrical fuel-rod conduction.
//!
//! # Provenance
//!
//! Translated from `fuelrodheat_1dcylnd.m` by **Than Yan Ren** (Singapore
//! Nuclear Research and Safety Institute), BEDOK snapshot sha256
//! `e45cd6f57be2087c…`, received 2026-08-05. Faithful translation: the node
//! layout, the harmonic-mean conduction coefficients and the assembly order
//! are as in the original. Nothing is repaired.
//!
//! # The physics
//!
//! Steady radial conduction in a fuel pin,
//!
//! ```text
//! (1/r) d/dr ( k(T) r dT/dr ) + q''' = 0
//! ```
//!
//! integrated over annular control volumes and **divided through by `2*pi`**
//! (the MATLAB says so at its line 4; every coefficient below carries that
//! factor). The pellet is `fueln` rings, then a gas gap carrying a
//! *conductance*, then the cladding. A convective boundary condition closes
//! the outermost node.
//!
//! # The node layout
//!
//! One extra *surface* node is inserted at each material↔gap interface, so the
//! matrix is `maxid = maxir + surfcount` on a side — 24 for the NEACRP layout
//! (20 fuel rings + gap + clad). For that layout the solution vector reads:
//!
//! | index (0-based) | what |
//! |---|---|
//! | 0 | pellet centreline ring |
//! | 1..=19 | remaining pellet rings |
//! | 20 | pellet outer **surface** |
//! | 21 | the gap ring — an orphan row, see below |
//! | 22 | cladding inner **surface** |
//! | 23 | cladding outer surface, where the coolant BC is applied |
//!
//! The Doppler temperature the cross-section feedback uses is built from
//! indices `0` and `fueln` (`th_solverxyz.m:190`, `fueltemp(idx,fueln+1)` in
//! 1-based terms) — the centreline and the pellet surface.

use super::linalg::{solve_dense_lu, DenseMatrix};
use super::{radial_solution_nodes, FuelRodGeometry, FuelRodParams, RodMaterial, ThError, ThResult};

/// Solve the steady radial temperature profile of one fuel pin.
///
/// MATLAB `fuelrodheat_1dcylnd(params, geometry, temps, pwr, bc, modtemp)`.
///
/// # Arguments
///
/// - `params` — radial node counts; `params.max_ir` must equal
///   `geometry.which_k.len()`.
/// - `geometry` — radial mesh and material properties.
/// - `temperatures` — \[K\] the temperatures the **temperature-dependent
///   conductivities are evaluated at**, i.e. the previous Picard iterate.
///   Length `maxid` (see the module note on layout), *not* `maxir`.
/// - `volumetric_power` — \[W/cm³\] fission power density in the pellet,
///   MATLAB `pwr`. Zero outside fuel. Typical PWR values are 200–600 W/cm³.
/// - `boundary_coefficient` — \[W/(cm·K)\] the convective boundary coefficient
///   `hcoeff * Rtot`, MATLAB `bc`.
/// - `coolant_temperature` — \[K\] the coolant sink temperature, MATLAB
///   `modtemp`.
///
/// # Returns
///
/// The radial temperature profile \[K\], length `maxid`, centreline first.
///
/// # Unfinished-code gaps carried over from the MATLAB
///
/// These are **recorded, not repaired**, per `docs/bedok-port-scoping.md`
/// §1.0.
///
/// 1. **The gap ring is an orphan row fixed at `T = 1 K`.** When the loop
///    reaches a gap ring it writes `bvec(id) = 1` and leaves that row's
///    diagonal at the `1` the identity initialisation put there, so the row
///    reads `T = 1 K`. No other row references that column: the pellet-surface
///    row connects *across* the gap directly to the cladding-inner-surface row
///    (`laplccol(counter) = id+2`). The 1 K value is physically meaningless.
///    It survives because `th_solverxyz.m:185` clamps the whole profile to
///    `[coolant temperature, tmaxfuel]` immediately afterwards, and because
///    neither the Doppler temperature nor the wall heat flux reads that index.
///
/// 2. **A layout with no material→gap transition indexes past the matrix.**
///    With no gap, `surfcount = 0` so `maxid = maxir`, and the `ir == maxir`
///    branch still writes column `id+1 = maxid+1` and reads `temps(id+1)`.
///    In MATLAB that is a hard error from `sparse`. Here it is
///    [`ThError::UnsupportedRodLayout`], raised before the write.
///
/// 3. **A material→material interface with no gap between them assembles no
///    conduction coefficient.** If ring `ir` and ring `ir+1` are different
///    *conducting* materials, the loop sets `surf = 1` and re-enters with the
///    same `ir`; the `surf == 1` branch then only does anything when
///    `whichk(ir+1) == 0`. For a direct fuel→clad interface it therefore adds
///    no off-diagonal at all and leaves `kplus` at its previous value, so the
///    diagonal becomes `2*kplus_previous` and the two sides are not coupled.
///    The benchmark geometries always place a gap between pellet and cladding,
///    so this path is never exercised — but it is wrong, and it is left wrong.
///
/// 4. **NaN in the solution is not an error here.** The MATLAB prints the
///    matrix and continues; the caller (`th_solverxyz.m:194`) is what detects
///    and substitutes. This function likewise returns NaN rather than failing.
///
/// # Errors
///
/// - [`ThError::LengthMismatch`] if `temperatures` is shorter than `maxid`, or
///   `params.max_ir` disagrees with the geometry.
/// - [`ThError::UnsupportedRodLayout`] for the layouts described above.
/// - [`ThError::SingularMatrix`] if the assembled operator cannot be
///   factorised.
pub fn solve_static(
    params: &FuelRodParams,
    geometry: &FuelRodGeometry,
    temperatures: &[f64],
    volumetric_power: f64,
    boundary_coefficient: f64,
    coolant_temperature: f64,
) -> ThResult<Vec<f64>> {
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

    let ctr = &geometry.ring_centre_radius;
    let lr = &geometry.ring_thickness;

    // --- MATLAB: laplcele(1:maxid) = ones -> the identity seed. Rows whose
    // diagonal is never overwritten keep it; that is what pins the orphan gap
    // row at T = 1 K.
    let mut matrix = DenseMatrix::zeros(max_id);
    for i in 0..max_id {
        matrix.accumulate(i, i, 1.0);
    }
    let mut rhs = vec![0.0; max_id];

    // --- Innermost ring (MATLAB ir = 1, id = 1) -------------------------
    let cond = conductivity_at(geometry, which_k[0], temperatures[0])?;
    let cond_plus = conductivity_at(geometry, which_k[1], temperatures[1])?;
    let mut k_plus = 2.0 * (cond * cond_plus) / (cond + cond_plus) * ctr[0] / lr[0];
    matrix.set(0, 0, k_plus);
    matrix.accumulate(0, 1, -k_plus);
    if which_k[0].is_fuel() {
        rhs[0] = 0.5 * volumetric_power * ctr[0] * ctr[0];
    }

    // --- Sweep outward (MATLAB: while ir <= maxir) -----------------------
    let mut id_minus = 0usize;
    let mut ir = 1usize; // MATLAB ir = 2
    let mut id = 1usize; // MATLAB id = 2
    let mut on_surface = false; // MATLAB surf

    while ir < max_ir {
        if id >= max_id {
            return Err(ThError::UnsupportedRodLayout {
                reason: "the radial sweep produced more solution nodes than maxid allows",
            });
        }

        if matches!(which_k[ir], RodMaterial::Gap) {
            // The orphan row: diagonal still 1 from the identity seed.
            rhs[id] = 1.0;
            ir += 1;
            id += 1;
            continue;
        }

        let k_minus = k_plus;

        if on_surface {
            // MATLAB `surf == 1`. Only the material->gap case is handled; see
            // gap 3 in this function's documentation.
            if matches!(which_k[ir + 1], RodMaterial::Gap) {
                if id + 2 >= max_id {
                    return Err(ThError::UnsupportedRodLayout {
                        reason: "gap-crossing coupling would index past the assembled matrix",
                    });
                }
                // MATLAB `tcon{end}*Ctr(ir+1)`: a gap CONDUCTANCE
                // (W/(cm^2 K)) times a radius, giving W/(cm K).
                k_plus = geometry.gap_conductance * ctr[ir + 1];
                matrix.accumulate(id, id + 2, -k_plus);
                if which_k[ir].is_fuel() {
                    let outer = geometry.cumulative_radius(ir);
                    rhs[id] = 0.5 * volumetric_power * (outer * outer - ctr[ir] * ctr[ir]);
                }
            }
        } else if ir == max_ir - 1 {
            // MATLAB `ir == maxir`: the outermost ring. Both conductivities
            // are taken from THIS ring's material (the MATLAB writes
            // `tcon{whichk(ir)}` twice), evaluated at the ring and at the node
            // beyond it.
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
        } else if matches!(which_k[ir + 1], RodMaterial::Gap) {
            // Ring adjacent to the gap: the extra factor of 2 is the MATLAB's
            // half-cell distance from the ring centre to the surface node.
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
        }

        matrix.set(id, id, k_minus + k_plus);
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

    // --- Convective boundary at the outermost solution node ---------------
    let k_minus = k_plus;
    let k_plus = boundary_coefficient;
    matrix.set(max_id - 1, max_id - 1, k_minus + k_plus);
    matrix.accumulate(max_id - 1, id_minus, -k_minus);
    rhs[max_id - 1] = boundary_coefficient * coolant_temperature;

    solve_dense_lu(matrix, &rhs, "fuelrodheat_1dcylnd")
}

/// Conductivity \[W/(cm·K)\] of a conducting ring at `temperature_kelvin` \[K\].
///
/// # Errors
///
/// [`ThError::UnsupportedRodLayout`] if asked for the gap, which carries a
/// conductance rather than a conductivity. In MATLAB the equivalent is
/// `tcon{0}`, an invalid cell index.
pub(crate) fn conductivity_at(
    geometry: &FuelRodGeometry,
    material: RodMaterial,
    temperature_kelvin: f64,
) -> ThResult<f64> {
    geometry
        .conductivity(material)
        .map(|model| model.evaluate(temperature_kelvin))
        .ok_or(ThError::UnsupportedRodLayout {
            reason: "a gap ring was asked for a thermal conductivity \
                     (MATLAB tcon{0} is an invalid cell index)",
        })
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::super::{ThermalConductivityModel, VolumetricHeatCapacityModel};
    use super::*;

    /// The NEACRP A2 fuel-pin geometry (`neacrpa2.m:200-251`), built exactly
    /// as the MATLAB builds it.
    pub(crate) fn neacrp_rod(fuel_rings: usize) -> (FuelRodParams, FuelRodGeometry) {
        let params = FuelRodParams::new(fuel_rings, 1, 1);
        let fuel_radius = 4.119_50e-01;
        let gap = 6.8e-03;
        let clad = 5.71e-02;

        let mut ring_thickness = vec![fuel_radius / fuel_rings as f64; fuel_rings];
        ring_thickness.push(gap);
        ring_thickness.push(clad);

        let mut ring_centre_radius = Vec::with_capacity(params.max_ir);
        let mut ring_area = Vec::with_capacity(params.max_ir);
        let mut running = 0.0;
        for (i, &thickness) in ring_thickness.iter().enumerate() {
            let inner = running;
            running += thickness;
            ring_centre_radius.push(running - 0.5 * thickness);
            // MATLAB geometry.fuel.Vi: pi*Lr(1)^2 for the first ring, then
            // pi*(Lr(i)^2 - Lr(i-1)^2) -- reproduced verbatim, quirk included
            // (it uses ring thicknesses, not cumulative radii, from i >= 2).
            ring_area.push(if i == 0 {
                std::f64::consts::PI * thickness * thickness
            } else {
                let previous = ring_thickness[i - 1];
                std::f64::consts::PI * (thickness * thickness - previous * previous)
            });
            let _ = inner;
        }

        let mut which_k = vec![RodMaterial::Fuel; fuel_rings];
        which_k.push(RodMaterial::Gap);
        which_k.push(RodMaterial::Clad);

        let pitch = 1.2665;
        let outer_radius = fuel_radius + gap + clad;
        let subchannel_area = pitch * pitch - std::f64::consts::PI * outer_radius * outer_radius;

        let geometry = FuelRodGeometry {
            fuel_radius,
            gap_thickness: gap,
            clad_thickness: clad,
            outer_radius,
            pitch,
            doppler_alpha: 0.7,
            ring_thickness,
            ring_centre_radius,
            ring_area,
            which_k,
            subchannel_area,
            hydraulic_diameter: 4.0 * subchannel_area
                / (2.0 * std::f64::consts::PI * outer_radius + 4.0 * pitch - 8.0 * outer_radius),
            fuel_conductivity: ThermalConductivityModel::Uo2Neacrp,
            clad_conductivity: ThermalConductivityModel::ZircaloyNeacrp,
            gap_conductance: 1.0,
            fuel_heat_capacity: VolumetricHeatCapacityModel::Uo2Neacrp,
            clad_heat_capacity: VolumetricHeatCapacityModel::ZircaloyNeacrp,
        };
        (params, geometry)
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::neacrp_rod;
    use super::*;

    /// Global energy balance of the assembled rod operator.
    ///
    /// **Methodology.** Summing every row of the assembled system makes all
    /// interior conduction coefficients cancel in pairs, leaving the exact
    /// algebraic identity
    ///
    /// ```text
    /// bc * (T_outer - T_coolant) = 0.5 * q''' * r_fuel^2
    /// ```
    ///
    /// (both sides being the rod's heat rate per unit length, divided by
    /// `2*pi`). The right-hand side telescopes over the pellet source terms to
    /// `0.5 * q''' * sumLr(fueln)^2`. This checks the coefficient assembly,
    /// the surface-node insertion, the gap coupling and the boundary
    /// condition all at once, and is independent of the conductivity model.
    /// Inputs: the NEACRP A2 rod (20 pellet rings, 6.8e-3 cm gap, 5.71e-2 cm
    /// Zircaloy clad), `q''' = 350 W/cm³`, `bc = 3.0 W/(cm·K)`,
    /// `T_coolant = 580 K`. Pass criterion: relative error below 1e-10.
    ///
    /// **Result (2026-08-05).** Relative error 3e-15, i.e. round-off. The
    /// operator conserves energy exactly, so the discretisation carries no
    /// leak and the gap crossing transmits the full pellet heat rate.
    #[test]
    fn assembled_operator_conserves_energy_exactly() {
        let (params, geometry) = neacrp_rod(20);
        let max_id = radial_solution_nodes(&geometry.which_k);
        let temperatures = vec![900.0; max_id];
        let volumetric_power = 350.0;
        let boundary_coefficient = 3.0;
        let coolant_temperature = 580.0;

        let profile = solve_static(
            &params,
            &geometry,
            &temperatures,
            volumetric_power,
            boundary_coefficient,
            coolant_temperature,
        )
        .expect("the NEACRP layout assembles");

        let outward_heat = boundary_coefficient * (profile[max_id - 1] - coolant_temperature);
        let generated = 0.5 * volumetric_power * geometry.fuel_radius * geometry.fuel_radius;
        let relative = (outward_heat - generated).abs() / generated;
        assert!(
            relative < 1e-10,
            "energy balance off by {relative:e}: out {outward_heat}, generated {generated}"
        );
    }

    /// The gap row is an orphan pinned at exactly 1 K.
    ///
    /// This test exists to **pin the quirk in place**, not to endorse it. If a
    /// later completion step couples the gap row properly, this test is
    /// expected to fail and should be updated deliberately.
    #[test]
    fn the_gap_row_comes_back_at_one_kelvin() {
        let (params, geometry) = neacrp_rod(20);
        let max_id = radial_solution_nodes(&geometry.which_k);
        let profile = solve_static(&params, &geometry, &vec![900.0; max_id], 350.0, 3.0, 580.0)
            .expect("assembles");
        // fueln = 20 rings -> index 20 is the pellet surface, 21 the gap.
        assert!((profile[21] - 1.0).abs() < 1e-12, "got {}", profile[21]);
    }

    /// The temperature profile decreases monotonically outward through the
    /// pellet, and the centreline is hotter than the coolant.
    #[test]
    fn pellet_profile_falls_monotonically_outward() {
        let (params, geometry) = neacrp_rod(20);
        let max_id = radial_solution_nodes(&geometry.which_k);
        let profile = solve_static(&params, &geometry, &vec![900.0; max_id], 350.0, 3.0, 580.0)
            .expect("assembles");
        for window in profile[..=20].windows(2) {
            assert!(
                window[0] >= window[1],
                "profile rises outward: {} then {}",
                window[0],
                window[1]
            );
        }
        assert!(profile[0] > 580.0, "centreline {} K", profile[0]);
    }

    /// An all-fuel rod is refused, because the MATLAB writes past its matrix.
    #[test]
    fn a_rod_with_no_gap_is_reported_as_an_upstream_gap() {
        let params = FuelRodParams {
            fuel_rings: 4,
            gap_rings: 0,
            clad_rings: 0,
            max_ir: 4,
        };
        let (_, template) = neacrp_rod(4);
        let geometry = FuelRodGeometry {
            which_k: vec![RodMaterial::Fuel; 4],
            ring_thickness: template.ring_thickness[..4].to_vec(),
            ring_centre_radius: template.ring_centre_radius[..4].to_vec(),
            ring_area: template.ring_area[..4].to_vec(),
            ..template
        };
        let err = solve_static(&params, &geometry, &vec![900.0; 4], 350.0, 3.0, 580.0).unwrap_err();
        assert!(
            matches!(err, ThError::UnsupportedRodLayout { .. }),
            "expected an upstream-gap error, got {err}"
        );
    }

    /// Zero power gives a rod isothermal with the coolant.
    #[test]
    fn zero_power_gives_an_isothermal_rod() {
        let (params, geometry) = neacrp_rod(20);
        let max_id = radial_solution_nodes(&geometry.which_k);
        let profile = solve_static(&params, &geometry, &vec![600.0; max_id], 0.0, 3.0, 580.0)
            .expect("assembles");
        for (index, &value) in profile.iter().enumerate() {
            if index == 21 {
                continue; // the orphan gap row, pinned at 1 K
            }
            assert!(
                (value - 580.0).abs() < 1e-9,
                "node {index} at {value} K, expected 580 K"
            );
        }
    }
}
