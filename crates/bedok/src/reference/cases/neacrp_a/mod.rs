//! NEACRP-L-335 PWR rod-ejection benchmark — cases A2 (steady and transient)
//! and A1 (transient, hot zero power).
//!
//! # Provenance
//!
//! | | |
//! |---|---|
//! | Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
//! | Source files | `neacrpa2.m`, `neacrpa2t.m`, `neacrpa1t.m` |
//! | Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
//! | Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
//! | Benchmark | NEACRP 3-D LWR Core Transient Benchmark, NEA/NSC/DOC(93)25 (NEACRP-L-335 Rev. 1), 1991 |
//!
//! # The case
//!
//! A PWR core octant with rotational symmetry, 17 × 17 radial nodes of
//! 10.803 cm and 18 axial nodes of specified thickness, two energy groups,
//! eleven materials, seven control-rod banks, and full cross-section feedback
//! on boron, fuel temperature, coolant temperature and coolant density.
//!
//! - **A2** ejects the central control assembly from 100 steps to fully
//!   withdrawn in 0.1 s, at **full power**.
//! - **A1** does the same from **fully inserted**, at **hot zero power**
//!   (2775 W core, 286 °C inlet), where the ejected worth is around one dollar
//!   and the response is a prompt-critical power spike.
//!
//! # Why the three constructors share one body
//!
//! `neacrpa2t.m` is a *copy* of `neacrpa2.m` with a handful of assignments
//! changed and a transient block appended; `neacrpa1t.m` is in turn a copy of
//! `neacrpa2t.m`. Every difference is a plain overwrite of a leaf value —
//! nothing downstream in the constructor reads `params.boron`,
//! `params.fueltempavg`, `th.powratio` or `geometry.crod` — so applying the
//! deltas *after* building A2 gives bit-identical results to running the
//! copied file top to bottom, while keeping the three-way relationship visible
//! instead of triplicating 500 lines. Each delta is named in the doc comment
//! of the constructor that applies it.

pub mod tables;

use crate::error::{BedokError, Result};
use crate::reference::grid::{Geometry, Grid};

use super::csv_maps::CompositionMap;
use super::fuel::FuelGeometry;
use super::geometry::{
    geometry_ends_3d, matlab_int64_scale, Boundaries, Boundary, CaseGeometry, ControlRodConfig,
    GridScale,
};
use super::params::{
    colon, CaseParams, FuelDiscretisation, KineticsData, RodEjection, TransientSchedule,
};
use super::sigmas::{fissile_node_mask, CaseConstants, FeedbackTable, SigmaValues};
use super::th::{CoolantInlet, CoolantInletTemperature, FlowDirection, ThermalHydraulics};
use super::BuiltCase;

/// Radial nodes in the native mesh.
const NATIVE_NX: usize = 17;
/// Axial nodes in the native mesh.
const NATIVE_NZ: usize = 18;
/// Assembly pitch \[cm\]. MATLAB `geometry.Xtot = 10.803*17`.
const ASSEMBLY_PITCH_CM: f64 = 10.803;

/// Axial node thicknesses of the native 18-plane mesh \[cm\].
///
/// MATLAB `Zlengths`. Planes 1 and 18 are the axial reflectors; planes 2–4 and
/// 15–17 are the finer meshing at the ends of the fuelled height.
const AXIAL_LENGTHS_CM: [f64; NATIVE_NZ] = [
    30.0, 7.7, 11.0, 15.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 30.0, 12.8, 12.8,
    8.0, 30.0,
];

/// Build the NEACRP PWR case A2 at steady state.
///
/// Rust translation of `neacrpa2.m`.
///
/// # Grid
///
/// Unlike `iaea3ds.m` and `neacrpd1.m`, this case does **not** overwrite the
/// requested node counts — the three `params.maxi*` assignments are commented
/// out in the source, labelled "Recommended". It therefore runs on whatever
/// the driver asked for, and the refinement factors
/// `xscale = round(maxix/17)`, `zscale = round(maxiz/18)` are live: the
/// composition maps are sampled with `ceil(ix/maxix*17)` and each native axial
/// plane is split into `zscale` nodes of `Zlengths(k)/zscale`.
///
/// The energy-group count *is* overwritten, to 2.
///
/// # Boron
///
/// `params.boron = 1000` ppm, labelled "initial concentration" — this is
/// **not** the critical boron concentration. The two transient variants
/// replace it with values found by a boron search (1139.01 ppm for A2,
/// 551.31 ppm for A1); the benchmark's own PANTHER values are 1160.6 and
/// 567.7 ppm respectively.
///
/// # Errors
///
/// - [`BedokError::EmptyGrid`] if the requested grid is coarser than the
///   native mesh, which would make a refinement factor zero.
/// - [`BedokError::Fixture`] if the requested axial node count is not a whole
///   multiple of `zscale` covering all 18 native planes — the MATLAB indexes
///   `Zlengths(ceil(iz/zscale))` and errors out of bounds in that case.
pub fn neacrp_a2(input: &CaseParams) -> Result<BuiltCase> {
    let ngroups = 2;
    let grid = Grid::new(input.grid.nx, input.grid.ny, input.grid.nz, ngroups)?;

    let scale = GridScale {
        x: matlab_int64_scale(grid.nx, NATIVE_NX, grid)?,
        y: matlab_int64_scale(grid.ny, NATIVE_NX, grid)?,
        z: matlab_int64_scale(grid.nz, NATIVE_NZ, grid)?,
    };
    if grid.nz > NATIVE_NZ * scale.z {
        return Err(BedokError::Fixture {
            path: "neacrpa2".to_string(),
            reason: format!(
                "maxiz = {} exceeds {NATIVE_NZ} native planes at zscale = {}; \
                 Zlengths(ceil(iz/zscale)) would be out of bounds",
                grid.nz, scale.z
            ),
        });
    }

    // ----- reactor dimensions [cm] -----
    let x_total = ASSEMBLY_PITCH_CM * NATIVE_NX as f64;
    let y_total = ASSEMBLY_PITCH_CM * NATIVE_NX as f64;
    let z_total: f64 = AXIAL_LENGTHS_CM.iter().sum();

    let step_x = x_total / grid.nx as f64;
    let step_y = y_total / grid.ny as f64;

    let nodes = grid.nodes();
    let lx = vec![step_x; nodes];
    let ly = vec![step_y; nodes];

    // Lz(iz : maxiz : end) = Zlengths(ceil(iz/zscale))/zscale — the same
    // thickness at every radial position.
    let axial_thickness: Vec<f64> = (1..=grid.nz)
        .map(|iz| {
            let plane = (iz as f64 / scale.z as f64).ceil() as usize;
            AXIAL_LENGTHS_CM[plane - 1] / scale.z as f64
        })
        .collect();
    let mut lz = vec![0.0f64; nodes];
    let mut volume = vec![0.0f64; nodes];
    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            for iz in 0..grid.nz {
                let idx = grid.index(0, ix, iy, iz);
                lz[idx] = axial_thickness[iz];
                volume[idx] = lx[idx] * ly[idx] * lz[idx];
            }
        }
    }

    // Ctr(z) = sum(Lz(1:iz)) - 0.5*Lz(iz). The MATLAB sums over *flat*
    // indices 1..iz, which coincide with the first radial column's z entries,
    // so this is the cumulative axial height of the node's top face minus half
    // its own thickness.
    let mut axial_center = vec![0.0f64; grid.nz];
    let mut cumulative = 0.0;
    for iz in 0..grid.nz {
        cumulative += axial_thickness[iz];
        axial_center[iz] = cumulative - 0.5 * axial_thickness[iz];
    }
    let mut centers = vec![[0.0f64; 3]; nodes];
    for ix in 0..grid.nx {
        for iy in 0..grid.ny {
            for iz in 0..grid.nz {
                centers[grid.index(0, ix, iy, iz)] = [
                    (ix as f64 + 0.5) * step_x,
                    (iy as f64 + 0.5) * step_y,
                    axial_center[iz],
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

    // ----- material map -----
    let mut which_sigma = vec![0usize; nodes];
    let bands: [(usize, usize, CompositionMap); 4] = [
        (1, scale.z, CompositionMap::NeacrpA2AxialReflector),
        (scale.z + 1, 2 * scale.z, CompositionMap::NeacrpA2LowerFuel),
        (
            2 * scale.z + 1,
            17 * scale.z,
            CompositionMap::NeacrpA2MainFuel,
        ),
        (
            17 * scale.z + 1,
            18 * scale.z,
            CompositionMap::NeacrpA2AxialReflector,
        ),
    ];
    for (iz_first, iz_last, map) in bands {
        let data = map.load()?;
        for iz in iz_first..=iz_last.min(grid.nz) {
            for ix in 1..=grid.nx {
                for iy in 1..=grid.ny {
                    let row = sample_index(ix, grid.nx);
                    let col = sample_index(iy, grid.ny);
                    which_sigma[grid.index(0, ix - 1, iy - 1, iz - 1)] =
                        data.index_at_matlab(row, col)?;
                }
            }
        }
    }

    let ends = geometry_ends_3d(grid, &which_sigma)?;

    // ----- cross sections -----
    let base = tables::base_sigmas()?;
    let constants = CaseConstants::fast_group_birth(tables::MATERIALS, ngroups, None);

    // The one mask, computed from the fission table, reused by all four
    // state-variable feedbacks exactly as the MATLAB does
    // (`sigmavalues.fueltemp.upd = sigmavalues.boron.upd`, etc.).
    let update_mask = fissile_node_mask(&which_sigma, &base.nu_fission)?;

    let sigmas = SigmaValues {
        nu: constants.nu.clone(),
        chi: constants.chi.clone(),
        boron: Some(FeedbackTable {
            reference: Some(1200.2),
            derivative: tables::boron_derivatives()?,
            update_mask: update_mask.clone(),
        }),
        fuel_temperature: Some(FeedbackTable {
            reference: Some(891.45),
            derivative: tables::fuel_temperature_derivatives()?,
            update_mask: update_mask.clone(),
        }),
        coolant_temperature: Some(FeedbackTable {
            reference: Some(579.75),
            derivative: tables::coolant_temperature_derivatives()?,
            update_mask: update_mask.clone(),
        }),
        coolant_density: Some(FeedbackTable {
            reference: Some(0.7125),
            derivative: tables::coolant_density_derivatives()?,
            update_mask,
        }),
        control_rod: Some(FeedbackTable {
            // `neacrpa2.m` sets no `sigmavalues.crod.ref`;
            // `sigmavalupd3d_handler.m` assigns 0 before applying it.
            reference: None,
            derivative: tables::control_rod_increments()?,
            update_mask: Vec::new(),
        }),
        base,
    };

    // ----- control rods -----
    let crod_bottom = 37.7;
    let crod_step = 1.5942237;
    let crod_max_steps = 228.0;
    let control_rods = ControlRodConfig {
        bank_count: 7,
        bottom: crod_bottom,
        step: crod_step,
        max_steps: crod_max_steps,
        top: crod_bottom + crod_step * crod_max_steps,
        banks: CompositionMap::NeacrpA2ControlRodBanks.load()?,
        // 0 is at the bottom (fully inserted).
        positions: vec![100.0, 200.0, 100.0, 200.0, 200.0, 200.0, 200.0],
    };

    // ----- thermal hydraulics -----
    let fuel_discretisation = FuelDiscretisation::neacrp_default();
    let th = ThermalHydraulics {
        max_power_watt: 693.75e6,
        power_ratio: 1.0,
        coolant_heat_fraction: 0.019,
        coolant: CoolantInlet {
            pressure_mpa: 15.5,
            temperature: CoolantInletTemperature::Fixed(559.15),
            inlet_void: 0.00000000000001,
        },
        // Two earlier flow-rate expressions are commented out in the source;
        // the live one divides the core mass flow by the flow area left after
        // 314 rods per assembly-quarter.
        mass_flux_g_per_s_cm2: 12_893_000.0
            / 157.0
            / (4.0 * ASSEMBLY_PITCH_CM * ASSEMBLY_PITCH_CM
                - 314.0 * std::f64::consts::PI * 0.47585 * 0.47585),
        flow_direction: FlowDirection::Upward,
        fuel_pins_per_node: (264.0 / 4.0) / scale.x as f64 / scale.y as f64,
        guide_tubes_per_node: 25.0,
        inlet_forcing: None,
    };

    let fuel = FuelGeometry::build(
        fuel_discretisation,
        4.11950E-01,
        6.8E-03,
        5.71E-02,
        1.2665,
        0.7,
        // tcon{3}: gap conductance from the NEACRP benchmark [W/cm^2/K]
        1.0,
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
        control_rods: Some(control_rods),
    };

    let params = CaseParams {
        grid,
        num_extra_unknowns: 0,
        boron_ppm: Some(1000.0),
        fuel_temperature_average: Some(891.19),
        coolant_temperature_average: Some(559.19),
        coolant_density_average: Some(0.7464),
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

/// Build the NEACRP PWR case A2 rod-ejection transient.
///
/// Rust translation of `neacrpa2t.m`.
///
/// Identical to [`neacrp_a2`] for the steady state; the file differs from
/// `neacrpa2.m` in exactly these places, and each is applied here:
///
/// | MATLAB | Value | Why |
/// |---|---|---|
/// | `params.tend` | 5 s | transient window |
/// | `params.tgrid` | `[0:0.0025:0.2, 0.2:0.01:1, 1:0.05:5, 5]` | fine over the spike |
/// | `params.outprefix` | `neacrpa2t` | history CSV prefix |
/// | `params.boron` | 1139.01 ppm | critical boron **for this code** (coupled `k_eff` 1.000005); the benchmark's PANTHER value is 1160.6 ppm |
/// | `params.velocities` | `[0.28e8, 0.44e6]` cm/s | Table 2.1 |
/// | `params.beta_dnp` | `0.0076 * [0.034, 0.200, 0.183, 0.404, 0.145, 0.034]` | Table 2.2 |
/// | `params.lambda_dnp` | `[0.0128, 0.0318, 0.1190, 0.3181, 1.4027, 3.9286]` 1/s | Table 2.2 |
/// | `geometry.fuel.rhocp` | UO₂ + Zircaloy | §2.7 |
/// | `geometry.crodeject` | 1 | the central CA |
/// | `geometry.crodejectto` | 228 steps | fully withdrawn |
/// | `params.ejectduration` | 0.1 s | Figure 3.2 |
///
/// # The time grid repeats its junctions
///
/// `[0:0.0025:0.2, 0.2:0.01:1, …]` contains `0.2` twice, `1` twice and `5`
/// twice (the last from the explicit `params.tend` at the end). Reproduced
/// rather than deduplicated — see
/// [`TransientSchedule::has_duplicate_times`].
///
/// # Errors
///
/// As [`neacrp_a2`].
pub fn neacrp_a2_transient(input: &CaseParams) -> Result<BuiltCase> {
    let mut case = neacrp_a2(input)?;

    let t_end = 5.0;
    let mut time_grid = colon(0.0, 0.0025, 0.2);
    time_grid.extend(colon(0.2, 0.01, 1.0));
    time_grid.extend(colon(1.0, 0.05, t_end));
    time_grid.push(t_end);

    case.params.boron_ppm = Some(1139.01);
    case.params.transient = Some(TransientSchedule {
        t_end,
        time_grid,
        output_prefix: "neacrpa2t".to_string(),
        rod_ejection: Some(RodEjection {
            bank: 1,
            target_steps: 228.0,
            duration: 0.1,
        }),
    });
    case.params.kinetics = Some(pwr_kinetics());
    if let Some(fuel) = case.geometry.fuel.as_mut() {
        fuel.with_transient_heat_capacity();
    }

    Ok(case)
}

/// Build the NEACRP PWR case A1 rod-ejection transient at hot zero power.
///
/// Rust translation of `neacrpa1t.m`, which is `neacrpa2t.m` with these
/// changes:
///
/// | MATLAB | A2 transient | A1 transient |
/// |---|---|---|
/// | `params.tgrid` | `[0:0.0025:0.2, …]` | `[0:0.001:0.6, 0.6:0.005:1, 1:0.025:5, 5]` |
/// | `params.outprefix` | `neacrpa2t` | `neacrpa1t` |
/// | `th.powratio` | 1 | 1e-6 (2775 W core = 693.75 W per quarter) |
/// | `params.boron` | 1139.01 ppm | 551.31 ppm (benchmark PANTHER value 567.7 ppm) |
/// | `params.fueltempavg` | 891.19 K | 559.15 K — at HZP the fuel is in equilibrium with the coolant |
/// | `geometry.crod` | `[100 200 100 200 200 200 200]` | `[0 0 0 228 0 0 0]` — Figure 3.1: banks 1,2,3,5,6,7 fully inserted, bank 4 fully withdrawn |
///
/// The inlet temperature is unchanged: the HZP specification's 286 °C is the
/// same 559.15 K that case A2 already uses.
///
/// # Note from the source
///
/// The 1 ms time grid over 0–0.6 s is there because the ejected worth is
/// around one dollar at HZP, so the power spike is super-prompt-critical and
/// spans several decades. The source also warns that at, say, 1000 ppm the
/// core is roughly 4200 pcm subcritical and the transient stops being a
/// prompt excursion — i.e. the boron value above is load-bearing.
///
/// # Errors
///
/// As [`neacrp_a2`].
pub fn neacrp_a1_transient(input: &CaseParams) -> Result<BuiltCase> {
    let mut case = neacrp_a2_transient(input)?;

    let t_end = 5.0;
    let mut time_grid = colon(0.0, 0.001, 0.6);
    time_grid.extend(colon(0.6, 0.005, 1.0));
    time_grid.extend(colon(1.0, 0.025, t_end));
    time_grid.push(t_end);

    case.params.boron_ppm = Some(551.31);
    case.params.fuel_temperature_average = Some(559.15);
    if let Some(transient) = case.params.transient.as_mut() {
        transient.time_grid = time_grid;
        transient.output_prefix = "neacrpa1t".to_string();
    }
    if let Some(th) = case.th.as_mut() {
        th.power_ratio = 1e-6;
    }
    if let Some(rods) = case.geometry.control_rods.as_mut() {
        rods.positions = vec![0.0, 0.0, 0.0, 228.0, 0.0, 0.0, 0.0];
    }

    Ok(case)
}

/// The delayed-neutron and prompt-velocity data both PWR transients use.
///
/// NEACRP-L-335 Tables 2.1 and 2.2: six precursor groups totalling 0.76 %.
fn pwr_kinetics() -> KineticsData {
    let total_beta = 0.0076;
    KineticsData {
        velocities: vec![0.28E8, 0.44E6],
        beta: [0.034, 0.200, 0.183, 0.404, 0.145, 0.034]
            .iter()
            .map(|f| total_beta * f)
            .collect(),
        lambda: vec![0.0128, 0.0318, 0.1190, 0.3181, 1.4027, 3.9286],
    }
}

/// The composition-map sampling index MATLAB writes as `ceil(ix/maxix*17)`,
/// with `ix` 1-based.
fn sample_index(one_based: usize, max_index: usize) -> usize {
    ((one_based as f64) / (max_index as f64) * (NATIVE_NX as f64)).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::cases::params::TransientSchedule;

    fn steady() -> BuiltCase {
        neacrp_a2(&CaseParams::main_exec_defaults()).expect("A2 builds")
    }

    /// A2 keeps the requested grid — unlike IAEA-3D and D1, it overrides
    /// nothing but the group count.
    #[test]
    fn the_requested_grid_is_kept() {
        let case = steady();
        assert_eq!(case.grid().nx, 17);
        assert_eq!(case.grid().nz, 18);
        assert_eq!(case.grid().ngroups, 2);
        assert_eq!(case.geometry.scale, GridScale { x: 1, y: 1, z: 1 });
    }

    #[test]
    fn axial_mesh_matches_the_specification() {
        let case = steady();
        let grid = case.grid();
        assert!((case.geometry.base.z_total - 427.3).abs() < 1e-12);
        // Plane 1 is a 30 cm axial reflector, plane 2 is 7.7 cm.
        assert_eq!(case.geometry.base.lz[grid.index(0, 0, 0, 0)], 30.0);
        assert_eq!(case.geometry.base.lz[grid.index(0, 0, 0, 1)], 7.7);
        assert_eq!(case.geometry.base.lz[grid.index(0, 0, 0, 17)], 30.0);
        // Radial nodes are one assembly across.
        assert!((case.geometry.base.lx[0] - 10.803).abs() < 1e-12);
        // Node centre in z: 30 + 7.7/2 for the second plane.
        assert!((case.geometry.centers[grid.index(0, 0, 0, 1)][2] - (30.0 + 3.85)).abs() < 1e-12);
        // Volume is the product of the three lengths.
        let idx = grid.index(0, 0, 0, 1);
        let expect = 10.803 * 10.803 * 7.7;
        assert!((case.geometry.base.volume[idx] - expect).abs() < 1e-9);
    }

    /// Axial banding: reflector, one lower-fuel plane, fifteen main-fuel
    /// planes, reflector.
    #[test]
    fn axial_bands_come_from_the_right_maps() {
        let case = steady();
        // NEACRPA2_1(1,1) = 1 (axial reflector), _2(1,1) = 4, _3(1,1) = 4.
        assert_eq!(case.material_at(0, 0, 0), 1, "bottom axial reflector");
        assert_eq!(case.material_at(0, 0, 1), 4, "lower fuel plane");
        assert_eq!(case.material_at(0, 0, 16), 4, "main fuel");
        assert_eq!(case.material_at(0, 0, 17), 1, "top axial reflector");
        // NEACRPA2_3(1,2) = 9 — a 2.6 w/o assembly with 20 burnable absorbers.
        assert_eq!(case.material_at(0, 1, 5), 9);
    }

    #[test]
    fn thermal_hydraulic_inputs_match_the_source() {
        let case = steady();
        let th = case.th.as_ref().expect("A2 has thermal hydraulics");
        assert_eq!(th.max_power_watt, 693.75e6);
        assert_eq!(th.power_ratio, 1.0);
        assert_eq!(th.coolant.pressure_mpa, 15.5);
        assert_eq!(
            th.coolant.temperature,
            CoolantInletTemperature::Fixed(559.15)
        );
        assert_eq!(th.fuel_pins_per_node, 66.0);
        assert_eq!(th.guide_tubes_per_node, 25.0);
        // A PWR core mass flux is a few hundred g/s/cm^2.
        assert!(
            (300.0..400.0).contains(&th.mass_flux_g_per_s_cm2),
            "mass flux {} out of the expected PWR range",
            th.mass_flux_g_per_s_cm2
        );
        assert!(th.inlet_forcing.is_none());
    }

    #[test]
    fn control_rod_banks_are_loaded_and_positioned() {
        let case = steady();
        let rods = case
            .geometry
            .control_rods
            .as_ref()
            .expect("A2 has control rods");
        assert_eq!(rods.bank_count, 7);
        assert_eq!(rods.positions.len(), 7);
        assert_eq!(rods.positions[0], 100.0, "central CA at 100 steps");
        assert_eq!(rods.banks.at_matlab(1, 1), 1.0, "bank 1 at the centre");
        assert!((rods.top - (37.7 + 1.5942237 * 228.0)).abs() < 1e-12);
        // Tip height of the central bank.
        let heights = rods.tip_heights();
        assert!((heights[0] - (37.7 + 100.0 * 1.5942237)).abs() < 1e-12);
    }

    #[test]
    fn feedback_tables_are_all_present_with_their_reference_states() {
        let case = steady();
        let s = &case.sigmas;
        assert_eq!(
            s.boron.as_ref().expect("boron table").reference,
            Some(1200.2)
        );
        assert_eq!(
            s.fuel_temperature
                .as_ref()
                .expect("Doppler table")
                .reference,
            Some(891.45)
        );
        assert_eq!(
            s.coolant_temperature
                .as_ref()
                .expect("coolant T table")
                .reference,
            Some(579.75)
        );
        assert_eq!(
            s.coolant_density
                .as_ref()
                .expect("coolant density table")
                .reference,
            Some(0.7125)
        );
        // The control-rod table has no reference in the case file.
        assert_eq!(s.control_rod.as_ref().expect("rod table").reference, None);

        // The one fissile mask, shared by all four state variables.
        let boron_mask = &s.boron.as_ref().expect("boron").update_mask;
        assert_eq!(boron_mask.len(), case.grid().nodes());
        assert_eq!(
            boron_mask,
            &s.coolant_density.as_ref().expect("density").update_mask
        );
        // An axial reflector node is not updated; a fuel node is.
        let grid = case.grid();
        assert_eq!(boron_mask[grid.index(0, 0, 0, 0)], 0.0);
        assert_eq!(boron_mask[grid.index(0, 0, 0, 5)], 1.0);
    }

    #[test]
    fn steady_case_has_no_transient_data() {
        let case = steady();
        assert!(case.params.transient.is_none());
        assert!(case.params.kinetics.is_none());
        assert_eq!(case.params.boron_ppm, Some(1000.0));
        assert!(
            case.geometry
                .fuel
                .as_ref()
                .expect("fuel")
                .heat_capacity
                .is_empty(),
            "rho*cp is set only by the transient files"
        );
    }

    #[test]
    fn a2_transient_applies_its_documented_deltas() {
        let case = neacrp_a2_transient(&CaseParams::main_exec_defaults()).expect("builds");
        assert_eq!(case.params.boron_ppm, Some(1139.01));
        let t = case.params.transient.as_ref().expect("transient data");
        assert_eq!(t.t_end, 5.0);
        assert_eq!(t.output_prefix, "neacrpa2t");
        assert_eq!(t.time_grid[0], 0.0);
        assert_eq!(*t.time_grid.last().expect("non-empty"), 5.0);
        assert!(
            t.has_duplicate_times(),
            "the MATLAB concatenation repeats 0.2, 1 and 5"
        );
        let eject = t.rod_ejection.as_ref().expect("rod ejection");
        assert_eq!(eject.bank, 1);
        assert_eq!(eject.target_steps, 228.0);
        assert_eq!(eject.duration, 0.1);

        let k = case.params.kinetics.as_ref().expect("kinetics");
        assert_eq!(k.velocities, vec![0.28E8, 0.44E6]);
        assert_eq!(k.beta.len(), 6);
        assert!((k.total_beta() - 0.0076).abs() < 1e-15);
        assert_eq!(k.lambda[5], 3.9286);

        assert_eq!(
            case.geometry
                .fuel
                .as_ref()
                .expect("fuel")
                .heat_capacity
                .len(),
            2
        );
        // Everything else still matches A2.
        assert_eq!(case.params.fuel_temperature_average, Some(891.19));
        assert_eq!(case.th.as_ref().expect("th").power_ratio, 1.0);
    }

    #[test]
    fn a1_transient_is_a2_transient_at_hot_zero_power() {
        let case = neacrp_a1_transient(&CaseParams::main_exec_defaults()).expect("builds");
        assert_eq!(case.params.boron_ppm, Some(551.31));
        assert_eq!(case.params.fuel_temperature_average, Some(559.15));
        assert_eq!(case.th.as_ref().expect("th").power_ratio, 1e-6);
        // 693.75 MW * 1e-6 = 693.75 W per quarter core = 2775 W whole core.
        let th = case.th.as_ref().expect("th");
        assert!((th.max_power_watt * th.power_ratio * 4.0 - 2775.0).abs() < 1e-9);

        let rods = case.geometry.control_rods.as_ref().expect("rods");
        assert_eq!(rods.positions, vec![0.0, 0.0, 0.0, 228.0, 0.0, 0.0, 0.0]);

        let t = case.params.transient.as_ref().expect("transient");
        assert_eq!(t.output_prefix, "neacrpa1t");
        // 1 ms steps over the spike: 0.001 s apart at the start.
        assert!((t.time_grid[1] - 0.001).abs() < 1e-15);
        assert_eq!(t.t_end, 5.0);

        // The kinetics data is inherited unchanged from A2.
        let k = case.params.kinetics.as_ref().expect("kinetics");
        assert!((k.total_beta() - 0.0076).abs() < 1e-15);
    }

    /// The transient grids run from 0 to `tend` and never go backwards.
    #[test]
    fn transient_time_grids_are_monotone_non_decreasing() {
        for case in [
            neacrp_a2_transient(&CaseParams::main_exec_defaults()).expect("builds"),
            neacrp_a1_transient(&CaseParams::main_exec_defaults()).expect("builds"),
        ] {
            let t: &TransientSchedule = case.params.transient.as_ref().expect("transient");
            assert!(t.time_grid.windows(2).all(|w| w[1] >= w[0]));
            assert_eq!(t.time_grid[0], 0.0);
            assert!(*t.time_grid.last().expect("non-empty") <= t.t_end);
        }
    }

    /// A grid coarser than the native mesh is rejected rather than silently
    /// producing an empty axial loop.
    #[test]
    fn a_coarser_grid_is_rejected() {
        let mut input = CaseParams::main_exec_defaults();
        input.grid = Grid::new(8, 8, 8, 2).expect("valid grid");
        assert!(neacrp_a2(&input).is_err());
    }
}
