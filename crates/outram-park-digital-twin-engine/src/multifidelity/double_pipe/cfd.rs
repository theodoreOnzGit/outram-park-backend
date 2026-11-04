//! The high-fidelity side: `outram_foam_appbuilder_lib::cases::double_pipe`
//! at a design point, and what is read off it.
//!
//! The case, its boundary conditions, its steady criteria, the local UA, the
//! profiles and the pressure drop are all that crate's (V&V: its
//! `verification_and_validation/cht_double_pipe_gh1156.md`). This module
//! sets the case up from a [`DoublePipeInput`], runs it, and converts the
//! measured mean conductance of each side into the Nusselt number the TUAS
//! 1-D model's circular geometry needs to carry the same conductance per
//! unit length (V&V record of gh:#1178, "Closures").
use std::sync::Arc;

use outram_foam_appbuilder_lib::cases::double_pipe::{self, DoublePipeExchanger, ExchangerProfiles};
use outram_foam_appbuilder_lib::solvers::energy::IncompressibleThermo;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_mesh::block_primitives::DoublePipe;
use uom::si::f64::{KinematicViscosity, Length, Power, Pressure, Ratio, TemperatureInterval, ThermalConductance, ThermodynamicTemperature, Velocity, VolumeRate};
use uom::si::kinematic_viscosity::square_meter_per_second;
use uom::si::length::meter;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::temperature_interval::kelvin as dkelvin;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;
use uom::si::volume_rate::cubic_meter_per_second;

use super::{fixed, DoublePipeInput, DoublePipeOutput};

/// The CFD mesh: the cross-section of `outram_foam_mesh::block_primitives::DoublePipe`
/// and the axial resolution as a multiple of [`fixed::LAYERS_PER_METRE`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CfdMesh {
    /// O-grid side (`4 n` cells around).
    pub n: usize,
    /// Radial cells: tube ring, wall, annulus.
    pub nr: [usize; 3],
    /// Axial layers = round(this × 375 L).
    pub axial_scale: f64,
}

impl CfdMesh {
    /// The 20k demo grid's cross-section and axial cell (V&V record, "Mesh").
    #[must_use]
    pub fn default_grid() -> Self {
        Self { n: 5, nr: [4, 2, 5], axial_scale: 1.0 }
    }

    /// The mesh check's finer grid: n = 7, nr = (6, 3, 7), 1.5× the layers.
    #[must_use]
    pub fn fine_check() -> Self {
        Self { n: 7, nr: [6, 3, 7], axial_scale: 1.5 }
    }

    /// The coarse regression-test grid: n = 3, nr = (2, 2, 3), 0.4× the
    /// layers. Not a dataset grid.
    #[must_use]
    pub fn coarse_test() -> Self {
        Self { n: 3, nr: [2, 2, 3], axial_scale: 0.4 }
    }

    /// Axial layers for length `l` \[m\].
    #[must_use]
    pub fn layers(&self, l: f64) -> usize {
        ((self.axial_scale * fixed::LAYERS_PER_METRE * l).round() as usize).max(3)
    }
}

/// Everything the CFD run produced at one point.
#[derive(Debug, Clone, PartialEq)]
pub struct CfdRecord {
    /// x.
    pub input: DoublePipeInput,
    /// The outputs (with `nu_hot`, `nu_cold` the 1-D-convention closures).
    pub output: DoublePipeOutput,
    /// Heat the cold stream gained (energy check against `output.q`).
    pub q_cold: Power,
    /// `Q / LMTD` (a consistency check, not evidence; stage 2).
    pub ua_lmtd: ThermalConductance,
    /// Hot heat-capacity rate from the solved inlet flux.
    pub c_hot: ThermalConductance,
    /// Cold heat-capacity rate from the solved inlet flux.
    pub c_cold: ThermalConductance,
    /// ΔT-weighted mean Nusselt numbers on the mesh's own hydraulic
    /// diameters (the CFD's native convention), hot and cold.
    pub nu_mesh: [f64; 2],
    /// Mean convective conductance per unit length `G' = h_m P` \[W/(m K)\],
    /// hot and cold.
    pub g_per_length: [f64; 2],
    /// Cells: tube fluid, wall, annulus fluid.
    pub cells: [usize; 3],
    /// Axial layers.
    pub layers: usize,
    /// Flow steps to the steady criterion.
    pub flow_steps: usize,
    /// Energy passes to the steady criterion.
    pub energy_passes: usize,
    /// Wall time \[s\] (the only non-deterministic field).
    pub wall_seconds: f64,
    /// The axial profiles.
    pub profiles: ExchangerProfiles,
}

/// Steady criterion of the flows (`step_flows`' metric), as stage 2.
pub const FLOW_STEADY: f64 = 1.0e-3;
/// Steady criterion of the energy: a pass changing no cell by more than this \[K\].
pub const ENERGY_STEADY_K: f64 = 1.0e-8;

/// The case at `x`: stage 2's demo with this length, wall conductivity and
/// the two flow rates.
#[must_use]
pub fn case_for(x: &DoublePipeInput) -> DoublePipeExchanger {
    let (qh, qc) = x.flow_rates();
    let mut c = DoublePipeExchanger::demo();
    c.length = x.length;
    c.fluid = IncompressibleThermo::si(fixed::RHO, fixed::CP, fixed::K_FLUID);
    c.nu = KinematicViscosity::new::<square_meter_per_second>(fixed::NU);
    c.wall = IncompressibleThermo::si(fixed::RHO_WALL, fixed::CP_WALL, x.k_wall.get::<watt_per_meter_kelvin>());
    c.flow_rate = VolumeRate::new::<cubic_meter_per_second>(qh);
    c.annulus_flow_rate = Some(VolumeRate::new::<cubic_meter_per_second>(qc));
    c.t_hot_in = ThermodynamicTemperature::new::<kelvin>(fixed::T_HOT_IN);
    c.t_cold_in = ThermodynamicTemperature::new::<kelvin>(fixed::T_COLD_IN);
    c
}

/// The error of a CFD run.
#[derive(Debug, Clone, PartialEq)]
pub struct CfdError(pub String);

impl std::fmt::Display for CfdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "double-pipe CFD: {}", self.0)
    }
}

impl std::error::Error for CfdError {}

/// Run the CFD at `x` on `mesh` to steady and read it off.
///
/// # Errors
/// Mesh construction or a solver failure; flows or energy not reaching the
/// steady criteria within 4000 flow steps / 20 000 energy passes.
pub fn run(x: &DoublePipeInput, mesh: CfdMesh) -> Result<CfdRecord, CfdError> {
    let t0 = std::time::Instant::now();
    let e = |m: String| CfdError(m);
    let case = case_for(x);
    let l = x.length.get::<meter>();
    let nz = mesh.layers(l);
    let m = Length::new::<meter>;
    let dp = DoublePipe {
        r_tube_inner: m(fixed::R_TUBE_INNER),
        r_tube_outer: m(fixed::R_TUBE_OUTER),
        r_shell: m(fixed::R_SHELL),
        length: x.length,
        origin: [m(0.0); 3],
        n: mesh.n,
        nr: mesh.nr,
        nz,
    };
    let [a, b, c] = dp.meshes().map_err(|err| e(err.to_string()))?;
    let fa = Arc::new(a.to_fv_mesh().map_err(|err| e(err.to_string()))?);
    let fb = Arc::new(b.to_fv_mesh().map_err(|err| e(err.to_string()))?);
    let fc = Arc::new(c.to_fv_mesh().map_err(|err| e(err.to_string()))?);
    let dz = m(l / nz as f64);
    let (qh, qc) = (case.flow_rate.get::<cubic_meter_per_second>(), case.annulus_flow().get::<cubic_meter_per_second>());
    let dt_in = double_pipe::pseudo_time_step(&fa, Velocity::new::<meter_per_second>(qh / mesh_inlet_area(&fa)), case.nu, dz, 4.0, 0.5);
    let dt_out = double_pipe::pseudo_time_step(&fc, Velocity::new::<meter_per_second>(qc / mesh_inlet_area(&fc)), case.nu, dz, 4.0, 0.5);
    let mut cht = case.build(fa, fb, fc, dt_in, dt_out).map_err(|err| e(err.to_string()))?;
    let mut flow_steps = 0;
    loop {
        flow_steps += 1;
        let r = double_pipe::step_flows(&mut cht, case.length).map_err(|err| e(err.to_string()))?;
        if r < FLOW_STEADY {
            break;
        }
        if flow_steps >= 4000 {
            return Err(e(format!("flows not steady after {flow_steps} steps ({r:.3e})")));
        }
    }
    double_pipe::freeze_flows_for_steady_energy(&mut cht);
    cht.energy_outer_correctors = 20_000;
    cht.energy_outer_tolerance = TemperatureInterval::new::<dkelvin>(ENERGY_STEADY_K);
    let st = cht.step().map_err(|err| e(err.to_string()))?;
    if st.last_pass_change > ENERGY_STEADY_K {
        return Err(e(format!("energy not steady: {st:?}")));
    }
    let rep = double_pipe::report(&cht, &case);
    let prof = double_pipe::profiles(&cht);
    let dps = [double_pipe::pressure_drop(&cht.fluids[0], fixed::RHO), double_pipe::pressure_drop(&cht.fluids[1], fixed::RHO)];
    let (g, nu_1d) = closures_from_profiles(&prof, l);
    let output = DoublePipeOutput {
        ua: ThermalConductance::new::<watt_per_kelvin>(rep.ua_local),
        q: Power::new::<watt>(rep.q_hot),
        effectiveness: Ratio::new::<ratio>(rep.effectiveness),
        t_hot_out: ThermodynamicTemperature::new::<kelvin>(rep.t_hot_out),
        t_cold_out: ThermodynamicTemperature::new::<kelvin>(rep.t_cold_out),
        dp_hot: Pressure::new::<pascal>(dps[0]),
        dp_cold: Pressure::new::<pascal>(dps[1]),
        nu_hot: Ratio::new::<ratio>(nu_1d[0]),
        nu_cold: Ratio::new::<ratio>(nu_1d[1]),
    };
    Ok(CfdRecord {
        input: *x,
        output,
        q_cold: Power::new::<watt>(rep.q_cold),
        ua_lmtd: ThermalConductance::new::<watt_per_kelvin>(rep.ua_lmtd),
        c_hot: ThermalConductance::new::<watt_per_kelvin>(rep.c_hot),
        c_cold: ThermalConductance::new::<watt_per_kelvin>(rep.c_cold),
        nu_mesh: [prof.mean_nu_hot(), prof.mean_nu_cold()],
        g_per_length: g,
        cells: dp.n_cells(),
        layers: nz,
        flow_steps,
        energy_passes: st.passes,
        wall_seconds: t0.elapsed().as_secs_f64(),
        profiles: prof,
    })
}

fn mesh_inlet_area(mesh: &FvMesh) -> f64 {
    mesh.patches.iter().find(|p| p.name == "inlet").map_or(f64::NAN, |p| (0..p.size).map(|i| mesh.face_areas[p.start + i]).sum())
}

/// From the profiles: the mean convective conductance per unit length of
/// each side, `G' = (Σ q / Σ A ΔT) (Σ A / L)` \[W/(m K)\], and the Nusselt
/// numbers that give TUAS's circular geometry the same `G'`:
/// `Nu_hot = G'_hot / (π k)` (h on D = 2 r_tube,i over the perimeter π D)
/// and `Nu_cold = G'_cold D_h / (π D_tube,o k)` (h on D_h over the tube's
/// outer perimeter, as TUAS's shell side).
#[must_use]
pub fn closures_from_profiles(p: &ExchangerProfiles, length_m: f64) -> ([f64; 2], [f64; 2]) {
    let k = p.k_fluid;
    let dt_hot: Vec<f64> = p.t_bulk_hot.iter().zip(&p.t_wall_inner).map(|(b, w)| b - w).collect();
    let dt_cold: Vec<f64> = p.t_wall_outer.iter().zip(&p.t_bulk_cold).map(|(w, b)| w - b).collect();
    let g_hot = mean_conductance_per_length(&p.q_inner, &p.area_inner, &dt_hot, length_m);
    let g_cold = mean_conductance_per_length(&p.q_outer, &p.area_outer, &dt_cold, length_m);
    let pi = std::f64::consts::PI;
    ([g_hot, g_cold], [g_hot / (pi * k), g_cold * fixed::d_cold() / (pi * 2.0 * fixed::R_TUBE_OUTER * k)])
}

/// `(Σ q / Σ A ΔT) (Σ A / L)`: the uniform h carrying the same total heat
/// across the same local differences, times the perimeter.
fn mean_conductance_per_length(q: &[f64], a: &[f64], dt: &[f64], length_m: f64) -> f64 {
    let qs: f64 = q.iter().sum();
    let adt: f64 = a.iter().zip(dt).map(|(x, y)| x * y).sum();
    let at: f64 = a.iter().sum();
    qs / adt * at / length_m
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The closure conversion on a synthetic two-layer profile with known
    /// answers: uniform h = 500 W/(m² K) on a 1 cm² interface per layer over
    /// 0.02 m, i.e. G' = h P with P = A / Δz.
    #[test]
    fn closure_conversion_and_meshes() {
        let p = ExchangerProfiles {
            z: vec![0.005, 0.015],
            q_inner: vec![0.5, 0.25],
            q_outer: vec![0.5, 0.25],
            area_inner: vec![1e-4, 1e-4],
            area_outer: vec![1e-4, 1e-4],
            t_bulk_hot: vec![340.0, 330.0],
            t_bulk_cold: vec![300.0, 300.0],
            t_wall_inner: vec![330.0, 325.0],
            t_wall_outer: vec![310.0, 305.0],
            dh_hot: 0.01,
            dh_cold: 0.008,
            k_fluid: fixed::K_FLUID,
        };
        let (g, nu) = closures_from_profiles(&p, 0.02);
        // q / (A dT) = 0.5 / (1e-4 * 10) = 500 on both layers; P = 1e-4 / 0.01.
        assert!((g[0] - 500.0 * 0.01).abs() < 1e-12, "{g:?}");
        assert!((g[1] - 500.0 * 0.01).abs() < 1e-12);
        assert!((nu[0] - 5.0 / (std::f64::consts::PI * 0.6)).abs() < 1e-12);
        assert!((nu[1] - 5.0 * 0.008 / (std::f64::consts::PI * 0.012 * 0.6)).abs() < 1e-12);
        assert_eq!(CfdMesh::default_grid().layers(0.2), 75);
        assert_eq!(CfdMesh::fine_check().layers(0.1), 56);
        assert_eq!(CfdMesh::coarse_test().layers(0.1), 15);
        let c = case_for(&DoublePipeInput::si(30.0, 15.0, 10.0, 0.2));
        assert!((c.capacity_rate() - 4.0e6 * 2.356_194_49e-7).abs() < 1e-9);
        assert!(c.annulus_flow_rate.is_some());
        assert!(!CfdError("x".into()).to_string().is_empty());
    }
}
