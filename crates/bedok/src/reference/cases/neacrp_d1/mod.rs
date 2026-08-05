//! NEACRP-L-335 BWR case D1 — steady state and the inlet cold-water-injection
//! transient.
//!
//! # Provenance
//!
//! | | |
//! |---|---|
//! | Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
//! | Source files | `neacrpd1.m`, `neacrpd1t.m` (driven by `run_neacrpd1t.m`) |
//! | Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
//! | Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
//! | Benchmark | NEACRP 3-D LWR Core Transient Benchmark, NEA/NSC/DOC(93)25 (NEACRP-L-335 Rev. 1), 1991 |
//!
//! # The case
//!
//! A BWR core quadrant, 17 × 17 radial nodes of half an assembly pitch
//! (15.24 cm) and 14 axial nodes of 30.48 cm, two energy groups, nineteen
//! materials. Feedback on fuel temperature and coolant density only — the
//! void feedback is the physics of interest.
//!
//! The transient doubles the inlet subcooling with a 2.5 s time constant over
//! 20 s, at constant flow and with no rod motion: colder water raises the
//! coolant density, which adds reactivity, which raises power.
//!
//! # A naming discrepancy in the source
//!
//! `neacrpd1.m` is headed "BWR NEACRP BENCHMARK - Case D2" while the function,
//! the data files and `neacrpd1t.m` all say D1. The transient file's header is
//! specific and consistent (spec §6.2, Figure 6.1, Tables 5.1/5.2), so the
//! comment in `neacrpd1.m` appears to be a stale copy. Recorded, not resolved.

pub mod tables;

use crate::error::Result;
use crate::reference::grid::{Geometry, Grid};

use super::csv_maps::CompositionMap;
use super::fuel::FuelGeometry;
use super::geometry::{
    geometry_ends_3d, matlab_int64_scale, Boundaries, Boundary, CaseGeometry, GridScale,
};
use super::params::{
    colon, CaseParams, FuelDiscretisation, KineticsData, ThermalHydraulicModel, TransientSchedule,
};
use super::sigmas::{fissile_node_mask, CaseConstants, FeedbackTable, SigmaValues};
use super::th::{
    ColdWaterInjection, CoolantInlet, CoolantInletTemperature, FlowDirection, ThermalHydraulics,
};
use super::BuiltCase;

/// Radial nodes in the native mesh.
const NATIVE_NX: usize = 17;
/// Axial nodes in the native mesh — the height of `NEACRPD1_COL.csv`.
const NATIVE_NZ: usize = 14;
/// Full assembly pitch \[cm\]; the model uses half of it per node.
const ASSEMBLY_PITCH_CM: f64 = 30.48;
/// Uniform axial node height \[cm\]. MATLAB `Zlengths`, a scalar here.
const AXIAL_LENGTH_CM: f64 = 30.48;
/// Steady-state inlet subcooling below saturated liquid \[kJ/kg\].
const INLET_SUBCOOLING_KJ_PER_KG: f64 = 46.52;
/// System pressure \[MPa\].
const PRESSURE_MPA: f64 = 6.7;

/// Build the NEACRP BWR case D1 at steady state.
///
/// Rust translation of `neacrpd1.m`.
///
/// # The grid is overwritten
///
/// Like `iaea3ds.m` and unlike the PWR cases, the first three statements force
///
/// ```text
/// params.maxix = 17;  params.maxiy = 17;  params.maxiz = 14;
/// ```
///
/// `run_neacrpd1t.m` requests `maxiz = 18` and gets 14. The axial count is
/// fixed by the data: `NEACRPD1_COL.csv` has exactly 14 rows, one per axial
/// level. Read the grid back from the returned [`CaseParams`].
///
/// # How the material map is built
///
/// Two files rather than four bands. `NEACRPD1_1.csv` gives a *column type*
/// (1–10, `0` = outside the core) at each radial position; `NEACRPD1_COL.csv`
/// gives the material index for each (axial level, column type) pair. A
/// position with column type `0` is left at material `0` for its whole height.
///
/// # The inlet temperature comes from a steam-table flash
///
/// The MATLAB computes it through three IAPWS-IF97 calls — saturation
/// temperature at 6.7 MPa, the saturated-liquid enthalpy there, then a
/// `(p,h)` flash 46.52 kJ/kg below it. Per `docs/bedok-port-scoping.md` §3
/// `IAPWS_IF97.m` is **not** ported and `tampines-steam-tables` is substituted;
/// that is the one substitution allowed inside the reference path, and it
/// happens here, in
/// [`CoolantInletTemperature::evaluate_kelvin`](super::th::CoolantInletTemperature::evaluate_kelvin).
/// The stored inlet keeps the *specification* — pressure and subcooling — so
/// the substitution stays visible.
///
/// `params.cooltempavg`, which the MATLAB sets to that same inlet temperature,
/// is filled from the evaluated flash.
///
/// # Unfinished in the reference
///
/// - **`th.flowrate` is assigned twice.** The first assignment
///   (`13000000/157/400.78`) is dead; the live value is
///   `70000/(30.48² - 221*pi*0.715²)`. Both are ported as one field with the
///   live value, and the dead line is noted here rather than in code.
/// - **`params.boron = 1000` with no boron feedback table.** The steady
///   coupled driver reaches `sigmavalupd3d_handler.m`, which tests
///   `isfield(sigmavaluesref,'boron')` and skips the update, so the number has
///   no effect. It is carried through anyway.
/// - **No coolant-temperature feedback and no control-rod cross sections.**
///   See the [`tables`] module docs.
///
/// # Errors
///
/// - [`crate::error::BedokError::EmptyGrid`] if a refinement factor would be
///   zero.
/// - [`crate::error::BedokError::Fixture`] if a composition map entry is out of
///   range.
pub fn neacrp_d1(input: &CaseParams) -> Result<BuiltCase> {
    let ngroups = 2;
    let grid = Grid::new(NATIVE_NX, NATIVE_NX, NATIVE_NZ, ngroups)?;

    let scale = GridScale {
        x: matlab_int64_scale(grid.nx, NATIVE_NX, grid)?,
        y: matlab_int64_scale(grid.ny, NATIVE_NX, grid)?,
        z: matlab_int64_scale(grid.nz, NATIVE_NZ, grid)?,
    };

    // ----- reactor dimensions [cm] -----
    let x_total = ASSEMBLY_PITCH_CM / 2.0 * NATIVE_NX as f64;
    let y_total = ASSEMBLY_PITCH_CM / 2.0 * NATIVE_NX as f64;
    let z_total = grid.nz as f64 * AXIAL_LENGTH_CM;

    let step_x = x_total / grid.nx as f64;
    let step_y = y_total / grid.ny as f64;
    let step_z = AXIAL_LENGTH_CM / scale.z as f64;

    let nodes = grid.nodes();
    let lx = vec![step_x; nodes];
    let ly = vec![step_y; nodes];
    let lz = vec![step_z; nodes];
    let volume = vec![step_x * step_y * step_z; nodes];

    // Ctr(z) = sum(Lz(1:iz)) - 0.5*Lz(iz), uniform here.
    let mut centers = vec![[0.0f64; 3]; nodes];
    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            for iz in 0..grid.nz {
                centers[grid.index(0, ix, iy, iz)] = [
                    (ix as f64 + 0.5) * step_x,
                    (iy as f64 + 0.5) * step_y,
                    (iz as f64 + 0.5) * step_z,
                ];
            }
        }
    }

    let boundaries = Boundaries {
        x_min: Boundary::Reflective,
        x_max: Boundary::ZeroFlux,
        y_min: Boundary::Reflective,
        y_max: Boundary::ZeroFlux,
        z_min: Boundary::ZeroFlux,
        z_max: Boundary::ZeroFlux,
    };

    // ----- material map: radial column type x axial column table -----
    let radial = CompositionMap::NeacrpD1RadialColumns.load()?;
    let columns = CompositionMap::NeacrpD1ColumnTable.load()?;
    let mut which_sigma = vec![0usize; nodes];
    for iz in 1..=grid.nz {
        for ix in 1..=grid.nx {
            for iy in 1..=grid.ny {
                let column = radial.index_at_matlab(
                    sample_index(ix, grid.nx, NATIVE_NX),
                    sample_index(iy, grid.ny, NATIVE_NX),
                )?;
                if column == 0 {
                    continue;
                }
                let level = sample_index(iz, grid.nz, NATIVE_NZ);
                which_sigma[grid.index(0, ix - 1, iy - 1, iz - 1)] =
                    columns.index_at_matlab(level, column)?;
            }
        }
    }

    let ends = geometry_ends_3d(grid, &which_sigma)?;

    // ----- cross sections -----
    let base = tables::base_sigmas()?;
    let constants = CaseConstants::fast_group_birth(tables::MATERIALS, ngroups, None);
    let update_mask = fissile_node_mask(&which_sigma, &base.nu_fission)?;

    let sigmas = SigmaValues {
        nu: constants.nu.clone(),
        chi: constants.chi.clone(),
        // No boron, coolant-temperature or control-rod tables — see the module
        // docs of `tables`.
        boron: None,
        coolant_temperature: None,
        control_rod: None,
        fuel_temperature: Some(FeedbackTable {
            reference: Some(573.15),
            derivative: tables::fuel_temperature_derivatives()?,
            update_mask: update_mask.clone(),
        }),
        coolant_density: Some(FeedbackTable {
            reference: Some(0.55),
            derivative: tables::coolant_density_derivatives()?,
            update_mask,
        }),
        base,
    };

    // ----- thermal hydraulics -----
    let fuel_discretisation = FuelDiscretisation::neacrp_default();
    let inlet_temperature = CoolantInletTemperature::SubcooledBelowSaturation {
        pressure_mpa: PRESSURE_MPA,
        enthalpy_deficit_kj_per_kg: INLET_SUBCOOLING_KJ_PER_KG,
    };
    let th = ThermalHydraulics {
        max_power_watt: 1800.0 / 4.0 * 1e6,
        power_ratio: 1.0,
        coolant_heat_fraction: 0.019,
        coolant: CoolantInlet {
            pressure_mpa: PRESSURE_MPA,
            temperature: inlet_temperature,
            inlet_void: 0.00000000000001,
        },
        // The live assignment; an earlier `13000000/157/400.78` is overwritten.
        mass_flux_g_per_s_cm2: 70_000.0
            / (ASSEMBLY_PITCH_CM * ASSEMBLY_PITCH_CM
                - 221.0 * std::f64::consts::PI * 0.715 * 0.715),
        flow_direction: FlowDirection::Upward,
        fuel_pins_per_node: (196.0 / 4.0) / scale.x as f64 / scale.y as f64,
        guide_tubes_per_node: 25.0,
        inlet_forcing: None,
    };

    let fuel = FuelGeometry::build(
        fuel_discretisation,
        1.237 / 2.0,
        0.03 / 2.0,
        (1.43 - 1.267) / 2.0,
        1.875,
        0.7,
        // tcon{3}: gap conductance from the NEACRP benchmark [W/cm^2/K]
        0.35,
    );

    let geometry = CaseGeometry {
        base: Geometry {
            grid,
            x_total,
            y_total,
            z_total,
            lx,
            ly,
            lz,
            volume,
            which_sigma,
        },
        scale,
        centers,
        boundaries,
        ends: Some(ends),
        fuel: Some(fuel),
        control_rods: None,
    };

    let params = CaseParams {
        grid,
        num_extra_unknowns: 0,
        boron_ppm: Some(1000.0),
        fuel_temperature_average: Some(650.0),
        // MATLAB: params.cooltempavg = th.coolant.inlettemp.
        coolant_temperature_average: Some(inlet_temperature.evaluate_kelvin()),
        coolant_density_average: Some(0.453),
        fuel: Some(fuel_discretisation),
        ..input.clone()
    };

    Ok(BuiltCase {
        params,
        geometry,
        constants,
        sigmas,
        th: Some(th),
    })
}

/// Build the NEACRP BWR case D1 inlet cold-water-injection transient.
///
/// Rust translation of `neacrpd1t.m`, which calls `neacrpd1.m` and then adds:
///
/// | MATLAB | Value | Source |
/// |---|---|---|
/// | `sigmavalues.fp` | `sigmavalues.f * 3.20e-11` | Table 5.1 prompt energy release |
/// | `sigmavalues.fueltemp.fp`, `.coolden.fp` | the same scaling of their `f` | |
/// | `params.tend` | 20 s | §6.2 |
/// | `params.tgrid` | `[0:0.025:2, 2:0.05:6, 6:0.1:12, 12:0.2:20]` | |
/// | `params.th_model` | `'hem'` | homogeneous equilibrium, steady *and* transient |
/// | `params.velocities` | `[1/3.57e-8, 1/2.27e-6]` cm/s | Table 5.1 |
/// | `params.beta_dnp` | `[0.00026, 0.00152, 0.00139, 0.00307, 0.00110, 0.00026]` | Table 5.2 |
/// | `params.lambda_dnp` | `[0.013, 0.032, 0.119, 0.318, 1.403, 3.929]` 1/s | Table 5.2 |
/// | `geometry.fuel.rhocp` | UO₂ + Zircaloy | §5.7 |
/// | `geometry.crodeject` | 0 — **no rod motion** | §6.2 |
/// | `th.inlettemp_t` | `46.52*(2 - exp(-0.4 t))` kJ/kg subcooling | Figure 6.1 |
///
/// # The kappa-fission workaround, in the reference's own words
///
/// `neacrpd1.m` leaves `sigmavalues.fp` identically zero because the steady
/// solver derives power from the fission source and never reads it. The
/// transient normalisation `P/P0` *does* read it, and would compute `0/0`. The
/// source therefore rebuilds it as `fp = E0 * nu*Sigma_f / nu` with
/// `E0 = 3.20e-11 J/fission` and `nu = 1` as encoded, noting that under a
/// composition-uniform `nu` the **ratio** `P/P0` is exact because the `E0`
/// scale cancels. That is a repair made by the reference itself, so it is
/// ported as written — it is not this translation adding one.
///
/// # Why the model is forced to HEM
///
/// From the source header: the transient chain
/// (`th_solvertimexyz` → `singleflow1devaptime`) is a homogeneous-equilibrium
/// enthalpy march, so the *initial steady state* must run the same model. The
/// two-fluid steady solver would hand the HEM transient a slip-void density
/// mismatch, i.e. a spurious reactivity step at `t = 0`.
///
/// # Errors
///
/// As [`neacrp_d1`].
pub fn neacrp_d1_transient(input: &CaseParams) -> Result<BuiltCase> {
    let mut case = neacrp_d1(input)?;

    // ----- kappa-Sigma_f rebuilt from nu-Sigma_f -----
    const PROMPT_ENERGY_RELEASE_J: f64 = 3.20e-11;
    scale_into_kappa_fission(&mut case.sigmas.base, PROMPT_ENERGY_RELEASE_J);
    if let Some(t) = case.sigmas.fuel_temperature.as_mut() {
        scale_into_kappa_fission(&mut t.derivative, PROMPT_ENERGY_RELEASE_J);
    }
    if let Some(t) = case.sigmas.coolant_density.as_mut() {
        scale_into_kappa_fission(&mut t.derivative, PROMPT_ENERGY_RELEASE_J);
    }

    // ----- transient window -----
    let t_end = 20.0;
    let mut time_grid = colon(0.0, 0.025, 2.0);
    time_grid.extend(colon(2.0, 0.05, 6.0));
    time_grid.extend(colon(6.0, 0.1, 12.0));
    time_grid.extend(colon(12.0, 0.2, t_end));

    case.params.transient = Some(TransientSchedule {
        t_end,
        time_grid,
        output_prefix: "neacrpd1t".to_string(),
        // geometry.crodeject = 0: no control-rod motion in D1.
        rod_ejection: None,
    });
    case.params.thermal_hydraulic_model = Some(ThermalHydraulicModel::HomogeneousEquilibrium);
    case.params.kinetics = Some(KineticsData {
        // Table 5.1 gives inverse velocities; the source inverts them here.
        velocities: vec![1.0 / 3.57e-8, 1.0 / 2.27e-6],
        beta: vec![0.00026, 0.00152, 0.00139, 0.00307, 0.00110, 0.00026],
        lambda: vec![0.013, 0.032, 0.119, 0.318, 1.403, 3.929],
    });

    if let Some(fuel) = case.geometry.fuel.as_mut() {
        fuel.with_transient_heat_capacity();
    }
    if let Some(th) = case.th.as_mut() {
        th.inlet_forcing = Some(ColdWaterInjection::neacrp_d1());
    }

    Ok(case)
}

/// `fp <- f * scale`, the rebuild `neacrpd1t.m` performs on three tables.
fn scale_into_kappa_fission(set: &mut super::sigmas::SigmaSet, scale: f64) {
    set.kappa_fission = set
        .nu_fission
        .iter()
        .map(|row| row.iter().map(|v| v * scale).collect())
        .collect();
}

/// The composition-map sampling index MATLAB writes as
/// `ceil(ix/maxix*native)`, with `ix` 1-based.
fn sample_index(one_based: usize, max_index: usize, native: usize) -> usize {
    ((one_based as f64) / (max_index as f64) * (native as f64)).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steady() -> BuiltCase {
        neacrp_d1(&CaseParams::run_neacrpd1t_defaults()).expect("D1 builds")
    }

    /// The driver asks for 18 axial nodes and gets 14 — the height of the
    /// axial column table.
    #[test]
    fn the_axial_grid_is_overwritten_to_fourteen() {
        let input = CaseParams::run_neacrpd1t_defaults();
        assert_eq!(input.grid.nz, 18, "run_neacrpd1t.m asks for 18");
        let case = neacrp_d1(&input).expect("builds");
        assert_eq!(case.grid().nx, 17);
        assert_eq!(case.grid().nz, 14);
        assert_eq!(case.grid().ngroups, 2);
        assert_eq!(case.grid().nodes(), 17 * 17 * 14);
        assert_eq!(case.geometry.scale, GridScale { x: 1, y: 1, z: 1 });
    }

    #[test]
    fn dimensions_are_half_assembly_pitch_by_a_foot() {
        let case = steady();
        assert!((case.geometry.base.x_total - 15.24 * 17.0).abs() < 1e-12);
        assert!((case.geometry.base.z_total - 14.0 * 30.48).abs() < 1e-12);
        assert!((case.geometry.base.lx[0] - 15.24).abs() < 1e-12);
        assert!((case.geometry.base.lz[0] - 30.48).abs() < 1e-12);
        let expect = 15.24 * 15.24 * 30.48;
        assert!((case.geometry.base.volume[0] - expect).abs() < 1e-9);
    }

    /// The material map composes the radial column type with the axial table.
    #[test]
    fn material_map_composes_radial_and_axial_tables() {
        let case = steady();
        // NEACRPD1_1(1,1) = 8 -> column type 8.
        // NEACRPD1_COL(1,8) = 1, COL(2,8) = 13, COL(14,8) = 4.
        assert_eq!(case.material_at(0, 0, 0), 1, "bottom plane of column 8");
        assert_eq!(case.material_at(0, 0, 1), 13, "second plane of column 8");
        assert_eq!(case.material_at(0, 0, 13), 4, "top plane of column 8");
        // NEACRPD1_1(1,16) = 10 -> the radial reflector column, material 19
        // at every level.
        assert_eq!(case.material_at(0, 15, 0), 19);
        assert_eq!(case.material_at(0, 15, 7), 19);
        // NEACRPD1_1(17,17) = 0 -> outside the core, at every level.
        for iz in 0..14 {
            assert_eq!(case.material_at(16, 16, iz), 0, "iz = {iz}");
        }
    }

    #[test]
    fn thermal_hydraulic_inputs_match_the_source() {
        let case = steady();
        let th = case.th.as_ref().expect("D1 has thermal hydraulics");
        assert_eq!(th.max_power_watt, 450e6);
        assert_eq!(th.coolant.pressure_mpa, 6.7);
        assert_eq!(th.fuel_pins_per_node, 49.0);
        // A BWR core mass flux is around 100-150 g/s/cm^2.
        assert!(
            (100.0..150.0).contains(&th.mass_flux_g_per_s_cm2),
            "mass flux {} out of the expected BWR range",
            th.mass_flux_g_per_s_cm2
        );
        // The inlet is specified as a subcooling, not a temperature.
        match th.coolant.temperature {
            CoolantInletTemperature::SubcooledBelowSaturation {
                pressure_mpa,
                enthalpy_deficit_kj_per_kg,
            } => {
                assert_eq!(pressure_mpa, 6.7);
                assert!((enthalpy_deficit_kj_per_kg - 46.52).abs() < 1e-12);
            }
            CoolantInletTemperature::Fixed(_) => panic!("D1 specifies a subcooling"),
        }

        // The flash: saturated liquid at 6.7 MPa is about 1261.6 kJ/kg at
        // Tsat = 557.0 K, so 46.52 kJ/kg of subcooling lands a little under
        // 10 K below saturation.
        let inlet = th.coolant.temperature.evaluate_kelvin();
        let t_sat = crate::reference::th::steam::saturation_temperature(6.7);
        assert!(inlet.is_finite(), "the (p,h) flash must resolve");
        assert!(
            inlet < t_sat && inlet > t_sat - 20.0,
            "inlet {inlet} K should sit just below Tsat {t_sat} K"
        );
        // params.cooltempavg is set from the same value.
        assert_eq!(case.params.coolant_temperature_average, Some(inlet));
    }

    #[test]
    fn fuel_pin_is_the_bwr_geometry() {
        let case = steady();
        let fuel = case.geometry.fuel.as_ref().expect("D1 has a fuel pin");
        assert!((fuel.fuel_radius - 0.6185).abs() < 1e-12);
        assert!((fuel.gap_thickness - 0.015).abs() < 1e-12);
        assert!((fuel.clad_thickness - 0.0815).abs() < 1e-12);
        assert_eq!(fuel.pitch, 1.875);
        // The BWR gap conductance is 0.35 W/cm^2/K, not the PWR's 1.
        assert_eq!(
            fuel.conductivity[2],
            crate::reference::cases::fuel::ThermalConductivity::GapConductance(0.35)
        );
    }

    #[test]
    fn only_two_feedback_tables_exist() {
        let case = steady();
        let s = &case.sigmas;
        assert!(s.boron.is_none(), "no boron table despite params.boron");
        assert!(s.coolant_temperature.is_none());
        assert!(s.control_rod.is_none());
        assert_eq!(
            s.fuel_temperature.as_ref().expect("Doppler").reference,
            Some(573.15)
        );
        assert_eq!(
            s.coolant_density.as_ref().expect("density").reference,
            Some(0.55)
        );
        // params.boron is carried through even though nothing reads it.
        assert_eq!(case.params.boron_ppm, Some(1000.0));
        assert_eq!(case.params.fuel_temperature_average, Some(650.0));
        assert_eq!(case.params.coolant_density_average, Some(0.453));
        assert!(case.geometry.control_rods.is_none());
    }

    #[test]
    fn transient_rebuilds_kappa_fission_from_nu_fission() {
        let steady = steady();
        let case =
            neacrp_d1_transient(&CaseParams::run_neacrpd1t_defaults()).expect("transient builds");
        // Steady: identically zero. Transient: f scaled by E0.
        assert!(steady
            .sigmas
            .base
            .kappa_fission
            .iter()
            .all(|r| r.iter().all(|v| *v == 0.0)));
        let e0 = 3.20e-11;
        for m in 0..tables::MATERIALS {
            for g in 0..2 {
                let expect = case.sigmas.base.nu_fission[m][g] * e0;
                assert!(
                    (case.sigmas.base.kappa_fission[m][g] - expect).abs() < 1e-30,
                    "material {} group {}",
                    m + 1,
                    g + 1
                );
            }
        }
        // The two feedback tables get the same treatment.
        let doppler = case.sigmas.fuel_temperature.as_ref().expect("Doppler");
        assert!(
            (doppler.derivative.kappa_fission[1][1] - doppler.derivative.nu_fission[1][1] * e0)
                .abs()
                < 1e-30
        );
    }

    #[test]
    fn transient_schedule_and_kinetics_match_the_source() {
        let case =
            neacrp_d1_transient(&CaseParams::run_neacrpd1t_defaults()).expect("transient builds");
        let t = case.params.transient.as_ref().expect("transient data");
        assert_eq!(t.t_end, 20.0);
        assert_eq!(t.output_prefix, "neacrpd1t");
        assert_eq!(t.time_grid[0], 0.0);
        assert!((t.time_grid[1] - 0.025).abs() < 1e-15);
        assert!((*t.time_grid.last().expect("non-empty") - 20.0).abs() < 1e-12);
        assert!(t.time_grid.windows(2).all(|w| w[1] >= w[0]));
        assert!(
            t.rod_ejection.is_none(),
            "geometry.crodeject = 0: no rod motion in D1"
        );

        let k = case.params.kinetics.as_ref().expect("kinetics");
        assert!(
            (k.total_beta() - 0.0076).abs() < 1e-12,
            "total beta is 0.76 %"
        );
        assert!((k.velocities[0] - 1.0 / 3.57e-8).abs() < 1.0);
        assert_eq!(k.lambda.len(), 6);

        assert_eq!(
            case.params.thermal_hydraulic_model,
            Some(ThermalHydraulicModel::HomogeneousEquilibrium)
        );
        assert_eq!(
            case.geometry
                .fuel
                .as_ref()
                .expect("fuel")
                .heat_capacity
                .len(),
            2
        );
    }

    #[test]
    fn transient_attaches_the_cold_water_forcing() {
        let case =
            neacrp_d1_transient(&CaseParams::run_neacrpd1t_defaults()).expect("transient builds");
        let forcing = case
            .th
            .as_ref()
            .expect("th")
            .inlet_forcing
            .expect("D1 transient has an inlet forcing");
        // Continuous with the steady inlet at t = 0.
        assert!(
            (forcing.enthalpy_deficit_kj_per_kg(0.0) - INLET_SUBCOOLING_KJ_PER_KG).abs() < 1e-12
        );
        // Subcooling grows monotonically towards double.
        assert!(forcing.enthalpy_deficit_kj_per_kg(20.0) > forcing.enthalpy_deficit_kj_per_kg(1.0));
        assert!(forcing.enthalpy_deficit_kj_per_kg(20.0) < 2.0 * INLET_SUBCOOLING_KJ_PER_KG);
    }
}
