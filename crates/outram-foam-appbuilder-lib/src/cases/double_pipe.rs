//! A counterflow double-pipe heat exchanger as three regions coupled by
//! [`ChtMultiRegionFoam`] (gh:#1156, Foam Studio stage 2): hot fluid in the
//! tube (+z), the conducting tube wall, cold fluid in the annulus (−z).
//!
//! The meshes come from the caller (`outram_foam_mesh::block_primitives::DoublePipe`
//! builds them conformal, with the patch names used here). This module sets
//! the boundary conditions, marches the two flows to steady, freezes them,
//! iterates the three energy equations, and measures what an exchanger V&V
//! needs: the outlet temperatures, the heat each stream exchanged, the heat
//! through each interface, the effectiveness, and the **local UA** (below).
//! V&V record: `verification_and_validation/cht_double_pipe_gh1156.md`.
//!
//! # Boundary conditions
//!
//! | region | patch | U, p | T |
//! |---|---|---|---|
//! | tube fluid | `inlet` (z-min) | uniform U, zeroGradient p | fixed `t_hot_in` |
//! | tube fluid | `outlet` (z-max) | zeroGradient U, p = 0 | zeroGradient |
//! | tube fluid | `tube_inner_wall` | no-slip | coupled |
//! | tube wall | `tube_inner_wall`, `tube_outer_wall` | — | coupled |
//! | tube wall | `tube_ends` | — | adiabatic |
//! | annulus fluid | `inlet` (z-max) | uniform U, zeroGradient p | fixed `t_cold_in` |
//! | annulus fluid | `outlet` (z-min) | zeroGradient U, p = 0 | zeroGradient |
//! | annulus fluid | `tube_outer_wall` | no-slip | coupled |
//! | annulus fluid | `shell_wall` | no-slip | adiabatic |
//!
//! ~~The two streams carry the same volumetric flow rate and the same
//! properties, so `C_hot = C_cold` (C_r = 1)~~ **CORRECTED 2026-11-04
//! (gh:#1178):** by default (`annulus_flow_rate: None`, the demo) the two
//! streams carry the same volumetric flow rate and the same properties, so
//! `C_hot = C_cold` (C_r = 1): the stream-to-stream
//! temperature difference of an ideal counterflow exchanger is then constant
//! along it whatever the local U does, and ε = NTU/(1 + NTU) with NTU =
//! UA/C holds for any U(z). With `annulus_flow_rate: Some(..)` C_r ≠ 1; the
//! ideal exchanger's ε still depends on U(z) only through ∫ U dA, so ε-NTU
//! at the local UA remains the comparison.
//!
//! # The local UA
//!
//! `UA_local = Σ_layers q_l / (T_b,hot,l − T_b,cold,l)`: per axial layer of
//! cells, the heat through the tube's inner interface in the layer over the
//! difference of the two streams' mixing-cup temperatures there (the upwind
//! face mixing-cup at each layer's downstream face). It is the UA that
//! ε-NTU's ideal exchanger assumes, measured from the field, so ε-NTU at it
//! against the solved ε tests what the ideal model leaves out (axial
//! conduction in the wall and the fluids, and the entrance regions' radial
//! profiles), not an identity.
use petir::mathf::RealMath;
use std::sync::Arc;

use outram_foam_basic_lib::prelude::*;
use uom::si::f64::{KinematicViscosity, Length, ThermodynamicTemperature, Time, Velocity, VolumeRate};
use uom::si::kinematic_viscosity::square_meter_per_second;
use uom::si::length::meter;
use uom::si::power::watt;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;
use uom::si::volume_rate::cubic_meter_per_second;

use crate::cases::laminar_pipe::{axial_layers, PipeFlow};
use crate::error::AppBuilderError;
use crate::io::fv_schemes::DdtScheme;
use crate::solvers::cht_multi_region_foam::{ChtMultiRegionFoam, CoupledTemperature, RegionId, SolidRegion};
use crate::solvers::energy::{EnergyEquation, IncompressibleThermo};
use crate::solvers::non_orthogonal::LaplacianCorrection;
use crate::solvers::pimple_foam::PimpleFoam;

/// The physical set-up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DoublePipeExchanger {
    /// Length of the exchanger.
    pub length: Length,
    /// Both fluids' properties.
    pub fluid: IncompressibleThermo,
    /// Both fluids' kinematic viscosity.
    pub nu: KinematicViscosity,
    /// The tube wall's properties.
    pub wall: IncompressibleThermo,
    /// Volumetric flow rate of the tube (hot) stream, and of the annulus
    /// stream too unless [`Self::annulus_flow_rate`] is set.
    pub flow_rate: VolumeRate,
    /// Volumetric flow rate of the annulus (cold) stream. `None` (the demo)
    /// gives it the tube's flow rate, so `C_hot = C_cold`; `Some` sets the
    /// two Reynolds numbers independently (gh:#1178's design grid, where
    /// `C_r ≠ 1`).
    pub annulus_flow_rate: Option<VolumeRate>,
    /// Tube (hot) inlet temperature.
    pub t_hot_in: ThermodynamicTemperature,
    /// Annulus (cold) inlet temperature.
    pub t_cold_in: ThermodynamicTemperature,
}

impl DoublePipeExchanger {
    /// The demo's exchanger: L = 200 mm; water-like fluids (ρ = 1000 kg/m³,
    /// c_p = 4000 J/(kg K), k = 0.6 W/(m K), ν = 1e-6 m²/s, Pr = 6.7); a
    /// stainless-steel-like tube (ρ = 8000, c = 500, k = 16); 2.36e-7 m³/s
    /// per stream (Re = 30 in a 10 mm tube); 350 K hot, 300 K cold. Indicative
    /// property values for a demonstration, not a material data set.
    #[must_use]
    pub fn demo() -> Self {
        Self {
            length: Length::new::<meter>(0.2),
            fluid: IncompressibleThermo::si(1000.0, 4000.0, 0.6),
            nu: KinematicViscosity::new::<square_meter_per_second>(1.0e-6),
            wall: IncompressibleThermo::si(8000.0, 500.0, 16.0),
            flow_rate: VolumeRate::new::<cubic_meter_per_second>(30.0 * 1.0e-6 / 0.01 * std::f64::consts::PI * 0.005 * 0.005),
            t_hot_in: ThermodynamicTemperature::new::<kelvin>(350.0),
            t_cold_in: ThermodynamicTemperature::new::<kelvin>(300.0),
            annulus_flow_rate: None,
        }
    }

    /// Heat-capacity rate of the tube stream, `ρ c_p Q` \[W/K\] (of each
    /// stream when [`Self::annulus_flow_rate`] is `None`).
    #[must_use]
    pub fn capacity_rate(&self) -> f64 {
        self.fluid.rho_cp() * self.flow_rate.get::<cubic_meter_per_second>()
    }

    /// The annulus stream's volumetric flow rate: [`Self::annulus_flow_rate`],
    /// or the tube's when that is `None`.
    #[must_use]
    pub fn annulus_flow(&self) -> VolumeRate {
        self.annulus_flow_rate.unwrap_or(self.flow_rate)
    }

    /// The three regions coupled: fluids from `inner` and `outer` (the tube
    /// and annulus meshes), the solid from `tube`, with this module's
    /// boundary conditions; each fluid at rest and at the inlet temperature,
    /// marched at its own pseudo time step `dt_inner` / `dt_outer`.
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] if a mesh lacks a patch named in the module
    /// docs.
    pub fn build(&self, inner: Arc<FvMesh>, tube: Arc<FvMesh>, outer: Arc<FvMesh>, dt_inner: Time, dt_outer: Time) -> Result<ChtMultiRegionFoam, AppBuilderError> {
        let th = self.t_hot_in.get::<kelvin>();
        let tc = self.t_cold_in.get::<kelvin>();
        let fluid = |mesh: Arc<FvMesh>, t_in: f64, dt: Time, q: VolumeRate| -> Result<PimpleFoam, AppBuilderError> {
            let area = patch_area(&mesh, "inlet")?;
            let flow = PipeFlow {
                radius: Length::new::<meter>((area / std::f64::consts::PI).sqrt()),
                length: self.length,
                bulk_velocity: Velocity::new::<meter_per_second>(q.get::<cubic_meter_per_second>() / area),
                nu: self.nu,
            };
            let mut s = flow.build_solver(mesh.clone(), dt);
            s.solution.pimple.n_non_orthogonal_correctors = 1;
            let mut e = EnergyEquation::uniform(mesh, self.fluid, t_in);
            e.set_patch("inlet", BoundaryCondition::FixedValue(t_in));
            s.energy = Some(e);
            Ok(s)
        };
        let f_in = fluid(inner.clone(), th, dt_inner, self.flow_rate)?;
        let f_out = fluid(outer.clone(), tc, dt_outer, self.annulus_flow())?;
        let wall = SolidRegion::new("tube", tube.clone(), self.wall, 0.5 * (th + tc));
        let pi = |m: &FvMesh, name: &str| patch_index(m, name);
        let ifaces = vec![
            CoupledTemperature::new(RegionId::Fluid(0), inner.clone(), pi(&inner, "tube_inner_wall")?, RegionId::Solid(0), tube.clone(), pi(&tube, "tube_inner_wall")?),
            CoupledTemperature::new(RegionId::Solid(0), tube.clone(), pi(&tube, "tube_outer_wall")?, RegionId::Fluid(1), outer.clone(), pi(&outer, "tube_outer_wall")?),
        ];
        ChtMultiRegionFoam::new(vec![f_in, f_out], vec![wall], ifaces)
    }
}

fn patch_index(m: &FvMesh, name: &str) -> Result<usize, AppBuilderError> {
    m.patches.iter().position(|p| p.name == name).ok_or_else(|| AppBuilderError::Case {
        case: std::path::PathBuf::from("double_pipe"),
        msg: format!("no patch named {name}"),
    })
}

fn patch_area(m: &FvMesh, name: &str) -> Result<f64, AppBuilderError> {
    let p = &m.patches[patch_index(m, name)?];
    Ok((0..p.size).map(|i| m.face_areas[p.start + i]).sum())
}

/// A pseudo-transient time step for a laminar internal flow: Courant number
/// `co` on the cell length `dz` along the flow at velocity `2 U` (the
/// developed centreline of a pipe), capped so that the viscous number
/// `ν Δt / h² ≤ max_viscous` with h the mean cell size `(V/N)^(1/3)`
/// (gh:#1162: `PimpleFoam` diverges at a viscous number of ≈ 8; stage 1's
/// demo caps it at 0.5).
#[must_use]
pub fn pseudo_time_step(mesh: &FvMesh, bulk: Velocity, nu: KinematicViscosity, dz: Length, co: f64, max_viscous: f64) -> Time {
    let vol: f64 = mesh.cell_volumes.iter().sum();
    let h = (vol / mesh.n_cells.max(1) as f64).r_cbrt();
    let dt_co = co * dz.get::<meter>() / (2.0 * bulk.get::<meter_per_second>().max(1e-300));
    let dt_visc = max_viscous * h * h / nu.get::<square_meter_per_second>().max(1e-300);
    Time::new::<second>(dt_co.min(dt_visc))
}

/// One flow step of both fluids (energy untouched); returns the largest of
/// the two `laminar_pipe` steady metrics (max |ΔU| / U per flow-through
/// time).
///
/// # Errors
/// Whatever `PimpleFoam::step_flow` returns.
pub fn step_flows(cht: &mut ChtMultiRegionFoam, length: Length) -> Result<f64, AppBuilderError> {
    let mut worst = 0.0_f64;
    for f in &mut cht.fluids {
        let before = f.u.internal.as_slice().to_vec();
        f.step_flow()?;
        let du = before.iter().zip(f.u.internal.as_slice()).fold(0.0_f64, |a, (b, c)| a.max((*c - *b).mag()));
        let ii = f.mesh.patches.iter().position(|p| p.name == "inlet").unwrap_or(0);
        let q: f64 = -f.phi.boundary[ii].values.as_slice().iter().sum::<f64>();
        let area: f64 = (0..f.mesh.patches[ii].size).map(|i| f.mesh.face_areas[f.mesh.patches[ii].start + i]).sum();
        let u = (q / area.max(1e-300)).abs().max(1e-300);
        worst = worst.max(du / u * (length.get::<meter>() / u) / f.control.delta_t);
    }
    Ok(worst)
}

/// Freeze both flows and switch every energy equation to steady
/// (chtMultiRegionSimpleFoam's form), ready for [`ChtMultiRegionFoam::step`]
/// to iterate the temperatures.
pub fn freeze_flows_for_steady_energy(cht: &mut ChtMultiRegionFoam) {
    for f in &mut cht.fluids {
        f.solution.pimple.frozen_flow = true;
        if let Some(e) = f.energy.as_mut() {
            e.ddt = DdtScheme::SteadyState;
        }
    }
    for s in &mut cht.solids {
        s.energy.ddt = DdtScheme::SteadyState;
    }
}

/// What the exchanger did (all heats in W, temperatures in K).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExchangerReport {
    /// Mixing-cup outlet temperature of the hot (tube) stream.
    pub t_hot_out: f64,
    /// Mixing-cup outlet temperature of the cold (annulus) stream.
    pub t_cold_out: f64,
    /// Heat lost by the hot stream: net enthalpy in minus out, plus the axial
    /// conduction in through its inlet and outlet faces.
    pub q_hot: f64,
    /// Heat gained by the cold stream (same definition, opposite sign).
    pub q_cold: f64,
    /// Heat into the tube wall through its inner interface (from the hot
    /// fluid), as the wall's matrix saw it.
    pub q_wall_in: f64,
    /// Heat out of the tube wall through its outer interface.
    pub q_wall_out: f64,
    /// Heat-capacity rate of each stream \[W/K\], `ρ c_p Q` from the solved
    /// inlet flux.
    pub c_hot: f64,
    /// As `c_hot`, cold stream.
    pub c_cold: f64,
    /// ε = q_hot / (C_min (T_hot,in − T_cold,in)).
    pub effectiveness: f64,
    /// The local UA \[W/K\] (module docs).
    pub ua_local: f64,
    /// Q / LMTD from the four terminal temperatures \[W/K\]; reported, not a
    /// check (near-identity under ε-NTU).
    pub ua_lmtd: f64,
}

/// Measure the exchanger in its current state (see [`ExchangerReport`]).
///
/// # Panics
/// If `cht` was not built by [`DoublePipeExchanger::build`] (two fluids with
/// energy, one solid, the patch names of the module docs).
#[must_use]
pub fn report(cht: &ChtMultiRegionFoam, case: &DoublePipeExchanger) -> ExchangerReport {
    let lap = LaplacianCorrection::Corrected;
    let stream = |f: &PimpleFoam| -> (f64, f64, f64) {
        let e = f.energy.as_ref().expect("fluid with energy");
        let m = &f.mesh;
        let (ii, io) = (patch_index(m, "inlet").expect("inlet"), patch_index(m, "outlet").expect("outlet"));
        let h_net_out = e.enthalpy_out(ii, &f.phi).get::<watt>() + e.enthalpy_out(io, &f.phi).get::<watt>();
        let cond_in = e.conduction_in(ii, lap).get::<watt>() + e.conduction_in(io, lap).get::<watt>();
        let t_out = e.mixing_cup(io, &f.phi).unwrap_or(f64::NAN);
        let q_in: f64 = -f.phi.boundary[ii].values.as_slice().iter().sum::<f64>();
        // Heat lost by the stream = enthalpy in − out + conduction in.
        (-h_net_out + cond_in, t_out, e.thermo.rho_cp() * q_in)
    };
    let (lost_hot, t_hot_out, c_hot) = stream(&cht.fluids[0]);
    let (lost_cold, t_cold_out, c_cold) = stream(&cht.fluids[1]);
    let q_hot = lost_hot;
    let q_cold = -lost_cold;
    let q_wall_in = cht.interface_heat_in(cht.interfaces[0].b).get::<watt>();
    let q_wall_out = -cht.interface_heat_in(cht.interfaces[1].a).get::<watt>();
    let th = case.t_hot_in.get::<kelvin>();
    let tc = case.t_cold_in.get::<kelvin>();
    let c_min = c_hot.min(c_cold);
    let effectiveness = q_hot / (c_min * (th - tc));
    // Local UA: per-layer interface heat over the bulk difference.
    let (bh, bc) = (layer_bulk_temperatures(&cht.fluids[0]), layer_bulk_temperatures(&cht.fluids[1]));
    let fi = &cht.fluids[0];
    let e = fi.energy.as_ref().expect("fluid with energy");
    let layers = axial_layers(&fi.mesh);
    let iw = cht.interfaces[0].a.patch;
    let p = &fi.mesh.patches[iw];
    let mut ql = vec![0.0; layers.len()];
    for k in 0..p.size {
        let o = fi.mesh.owner[p.start + k];
        if let Some(l) = layers.iter().position(|z| (z - fi.mesh.cell_centres[o].z).abs() < 1e-9) {
            ql[l] -= e.face_conduction(iw, k, lap).get::<watt>();
        }
    }
    let ua_local = ql.iter().zip(bh.iter().zip(&bc)).filter(|(_, (h, c))| h.is_finite() && c.is_finite()).map(|(q, (h, c))| q / (h - c)).sum();
    let (d1, d2) = (th - t_cold_out, t_hot_out - tc);
    let lmtd = if (d1 - d2).abs() < 1e-12 * d1.abs().max(1e-300) { d1 } else { (d1 - d2) / (d1 / d2).r_ln() };
    ExchangerReport { t_hot_out, t_cold_out, q_hot, q_cold, q_wall_in, q_wall_out, c_hot, c_cold, effectiveness, ua_local, ua_lmtd: q_hot / lmtd }
}

/// Mixing-cup temperature of every axial layer of a fluid region \[K\],
/// layers in [`axial_layers`] order: the flux-weighted temperature of the
/// upwind cells of the layer's downstream axial faces (internal faces, plus
/// the outflow patch for the last layer), so it does not depend on the flow
/// direction. `NaN` for a layer with no axial flux.
#[must_use]
pub fn layer_bulk_temperatures(f: &PimpleFoam) -> Vec<f64> {
    let m = &f.mesh;
    let t = &f.energy.as_ref().expect("fluid with energy").t;
    let layers = axial_layers(m);
    let (mut num, mut den) = (vec![0.0; layers.len()], vec![0.0; layers.len()]);
    for fc in 0..m.n_internal_faces {
        if m.face_area_vectors[fc].z.abs() > 0.5 * m.face_areas[fc] {
            let phi = f.phi.internal[fc];
            let up = if phi >= 0.0 { m.owner[fc] } else { m.neighbour[fc] };
            if let Some(l) = layers.iter().position(|z| (z - m.cell_centres[up].z).abs() < 1e-9) {
                num[l] += phi.abs() * t.internal[up];
                den[l] += phi.abs();
            }
        }
    }
    // The last layer's downstream face is the outlet patch: include
    // outflow boundary faces too, or that layer has no bulk temperature.
    for (pi, p) in m.patches.iter().enumerate() {
        for k in 0..p.size {
            let gf = p.start + k;
            let phi = f.phi.boundary[pi].values[k];
            if phi > 0.0 && m.face_area_vectors[gf].z.abs() > 0.5 * m.face_areas[gf] {
                let o = m.owner[gf];
                if let Some(l) = layers.iter().position(|z| (z - m.cell_centres[o].z).abs() < 1e-9) {
                    num[l] += phi * t.internal[o];
                    den[l] += phi;
                }
            }
        }
    }
    num.iter().zip(&den).map(|(a, b)| if *b > 0.0 { a / b } else { f64::NAN }).collect()
}

/// The exchanger's axial profiles (gh:#1178): per axial layer, the heat
/// through each interface, its area, the two streams' mixing-cup
/// temperatures and the two interface temperatures; from them the local
/// Nusselt number of each side. Layers in [`axial_layers`] order (z
/// increasing); the two fluids' meshes are conformal, so their layers
/// coincide.
///
/// The interface temperature of a face is the one the matrix saw,
/// `T_f = T_P + q_f / (κ A_f / d)`, from the face conduction `q_f` and the
/// same boundary distance `d` ([`LaplacianCorrection::Corrected`]) as
/// [`report`]; for the mixed coupled condition it is
/// `w T_ref + (1 − w) T_P`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExchangerProfiles {
    /// Layer centre z \[m\].
    pub z: Vec<f64>,
    /// Heat from the hot fluid into the wall through the inner interface \[W\].
    pub q_inner: Vec<f64>,
    /// Heat from the wall into the cold fluid through the outer interface \[W\].
    pub q_outer: Vec<f64>,
    /// Inner-interface area of the layer \[m²\].
    pub area_inner: Vec<f64>,
    /// Outer-interface area of the layer \[m²\].
    pub area_outer: Vec<f64>,
    /// Hot (tube) stream mixing-cup temperature \[K\].
    pub t_bulk_hot: Vec<f64>,
    /// Cold (annulus) stream mixing-cup temperature \[K\].
    pub t_bulk_cold: Vec<f64>,
    /// Area-mean inner-interface temperature \[K\].
    pub t_wall_inner: Vec<f64>,
    /// Area-mean outer-interface temperature \[K\].
    pub t_wall_outer: Vec<f64>,
    /// Hydraulic diameter of the tube, `4 A / P` from the mesh (inlet area
    /// over the inner interface's perimeter) \[m\].
    pub dh_hot: f64,
    /// Hydraulic diameter of the annulus, `4 A / P` with both walls wetted
    /// (inlet area over the outer interface's plus the shell's perimeter) \[m\].
    pub dh_cold: f64,
    /// Fluid conductivity \[W/(m K)\].
    pub k_fluid: f64,
}

impl ExchangerProfiles {
    /// Local tube-side Nusselt number per layer,
    /// `q'' D_h / (k (T_b,hot − T_w,inner))`.
    #[must_use]
    pub fn nu_hot(&self) -> Vec<f64> {
        (0..self.z.len()).map(|l| self.q_inner[l] / self.area_inner[l] * self.dh_hot / (self.k_fluid * (self.t_bulk_hot[l] - self.t_wall_inner[l]))).collect()
    }

    /// Local annulus-side Nusselt number per layer (heated inner wall),
    /// `q'' D_h / (k (T_w,outer − T_b,cold))`.
    #[must_use]
    pub fn nu_cold(&self) -> Vec<f64> {
        (0..self.z.len()).map(|l| self.q_outer[l] / self.area_outer[l] * self.dh_cold / (self.k_fluid * (self.t_wall_outer[l] - self.t_bulk_cold[l]))).collect()
    }

    /// Mean tube-side Nusselt number weighted by the local temperature
    /// difference: `Σ q / Σ A ΔT` times `D_h / k`, i.e. the uniform h that
    /// carries the same total heat across the same local differences. This
    /// is the mean a 1-D model with one h per side needs (gh:#1178 V&V
    /// record, "Closures").
    #[must_use]
    pub fn mean_nu_hot(&self) -> f64 {
        let q: f64 = self.q_inner.iter().sum();
        let a_dt: f64 = (0..self.z.len()).map(|l| self.area_inner[l] * (self.t_bulk_hot[l] - self.t_wall_inner[l])).sum();
        q / a_dt * self.dh_hot / self.k_fluid
    }

    /// As [`Self::mean_nu_hot`], annulus side.
    #[must_use]
    pub fn mean_nu_cold(&self) -> f64 {
        let q: f64 = self.q_outer.iter().sum();
        let a_dt: f64 = (0..self.z.len()).map(|l| self.area_outer[l] * (self.t_wall_outer[l] - self.t_bulk_cold[l])).sum();
        q / a_dt * self.dh_cold / self.k_fluid
    }
}

/// Per-layer heat, area and area-mean face temperature of patch `pi` of a
/// fluid region, layers as `layers`.
fn patch_layers(f: &PimpleFoam, pi: usize, layers: &[f64]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let lap = LaplacianCorrection::Corrected;
    let e = f.energy.as_ref().expect("fluid with energy");
    let m = &f.mesh;
    let p = &m.patches[pi];
    let k = e.thermo.kappa();
    let (mut q, mut a, mut ta) = (vec![0.0; layers.len()], vec![0.0; layers.len()], vec![0.0; layers.len()]);
    for fi in 0..p.size {
        let gf = p.start + fi;
        let o = m.owner[gf];
        if let Some(l) = layers.iter().position(|z| (z - m.cell_centres[o].z).abs() < 1e-9) {
            let qf = e.face_conduction(pi, fi, lap).get::<watt>();
            let d = lap.delta_coeff().boundary_delta(m, gf);
            let tf = e.t.internal[o] + qf * d / (k * m.face_areas[gf]);
            q[l] += qf;
            a[l] += m.face_areas[gf];
            ta[l] += m.face_areas[gf] * tf;
        }
    }
    let t = ta.iter().zip(&a).map(|(x, y)| x / y).collect();
    (q, a, t)
}

/// Measure the axial profiles (see [`ExchangerProfiles`]).
///
/// # Panics
/// If `cht` was not built by [`DoublePipeExchanger::build`].
#[must_use]
pub fn profiles(cht: &ChtMultiRegionFoam) -> ExchangerProfiles {
    let (hot, cold) = (&cht.fluids[0], &cht.fluids[1]);
    let z = axial_layers(&hot.mesh);
    let (iw, ow) = (cht.interfaces[0].a.patch, cht.interfaces[1].b.patch);
    let (q_in, a_in, tw_in) = patch_layers(hot, iw, &z);
    let (q_out_into_cold, a_out, tw_out) = patch_layers(cold, ow, &z);
    // The extruded length: the cell-centre span plus one (uniform) layer.
    let length = {
        let (lo, hi) = hot.mesh.cell_centres.iter().fold((f64::MAX, f64::MIN), |(a, b), c| (a.min(c.z), b.max(c.z)));
        (hi - lo) * z.len() as f64 / (z.len() as f64 - 1.0).max(1.0)
    };
    let inlet = |f: &PimpleFoam| patch_area(&f.mesh, "inlet").expect("inlet");
    let wall_area = |f: &PimpleFoam, name: &str| patch_area(&f.mesh, name).expect("wall patch");
    let dh_hot = 4.0 * inlet(hot) / (wall_area(hot, "tube_inner_wall") / length);
    let dh_cold = 4.0 * inlet(cold) / ((wall_area(cold, "tube_outer_wall") + wall_area(cold, "shell_wall")) / length);
    ExchangerProfiles {
        q_inner: q_in.iter().map(|x| -x).collect(),
        q_outer: q_out_into_cold,
        area_inner: a_in,
        area_outer: a_out,
        t_bulk_hot: layer_bulk_temperatures(hot),
        t_bulk_cold: layer_bulk_temperatures(cold),
        t_wall_inner: tw_in,
        t_wall_outer: tw_out,
        dh_hot,
        dh_cold,
        k_fluid: hot.energy.as_ref().expect("fluid with energy").thermo.kappa(),
        z,
    }
}

/// Static pressure drop inlet → outlet of a fluid region \[Pa\]:
/// `ρ (p̄_inlet − p̄_outlet)`, `p̄` the area mean of the kinematic pressure
/// on the patch (the fixed value where the patch fixes it, else the owner
/// cell's, which is the face value of zeroGradient). The flow enters with a
/// uniform profile, so this is the **apparent** drop of a simultaneously
/// developing flow: fully developed friction plus the entrance's momentum
/// and excess-friction increment (Shah's K(∞)).
///
/// # Panics
/// If the region has no `inlet` or `outlet` patch.
#[must_use]
pub fn pressure_drop(f: &PimpleFoam, density: f64) -> f64 {
    let m = &f.mesh;
    let mean = |name: &str| -> f64 {
        let pi = patch_index(m, name).expect("patch");
        let p = &m.patches[pi];
        let (mut s, mut a) = (0.0, 0.0);
        for fi in 0..p.size {
            let gf = p.start + fi;
            let v = match &f.p.boundary[pi].bc {
                BoundaryCondition::FixedValue(v) => *v,
                _ => f.p.internal[m.owner[gf]],
            };
            s += v * m.face_areas[gf];
            a += m.face_areas[gf];
        }
        s / a
    };
    density * (mean("inlet") - mean("outlet"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_case_numbers() {
        let c = DoublePipeExchanger::demo();
        let q = c.flow_rate.get::<cubic_meter_per_second>();
        assert!((q - 2.356_194_49e-7).abs() < 1e-14, "{q}");
        assert!((c.capacity_rate() - 4.0e6 * q).abs() < 1e-12);
        let m = crate::solvers::cht_multi_region_foam::line_mesh(10, Length::new::<meter>(0.1), Length::new::<meter>(0.0), ["inlet", "outlet"]);
        let dt = pseudo_time_step(&m, Velocity::new::<meter_per_second>(0.01), KinematicViscosity::new::<square_meter_per_second>(1e-6), Length::new::<meter>(0.01), 4.0, 0.5);
        // h = (0.1/10)^(1/3) = 0.2154 m: the viscous cap is far above the Courant step.
        assert!((dt.get::<second>() - 2.0).abs() < 1e-12);
        assert!(patch_index(&m, "nope").is_err());
        assert!((patch_area(&m, "inlet").unwrap() - 1.0).abs() < 1e-15);
    }
}
