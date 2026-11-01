//! Reusable PCICE verification cases (gh:#1068), set up once so the
//! finite-element comparison (gh:#1069) and any other solver can run exactly
//! the same problems.
//!
//! Each case is a [`PciceCase`]: geometry, EOS, initial condition, boundary
//! conditions, end time and the PCICE numerics the V&V record used. It builds
//! a [`PcicePipe`] ([`PciceCase::build_pcice`]) or the density-based
//! [`crate::hem::HemPipeOf`] on the same mesh ([`PciceCase::build_hem_pipe`],
//! inviscid like PCICE).
//!
//! Methodology and results: `crates/tampines/verification_and_validation/pcice.md`.

use petir::mathf::RealMath;
use tampines_steam_tables::hem::{
    AreaProfile, HemBoundary, HemEquationOfState, HemInitialCondition, HemNumerics, HemPipe,
    HemPipeGeometry, Iapws97Hem, IdealGasHem, PipeEnd, PrimitiveState,
};
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    Area, AvailableEnergy, Length, MassDensity, Pressure, Ratio, ThermodynamicTemperature, Time,
    Velocity,
};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::{megapascal, pascal};
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

use super::{PciceBoundary, PciceNumerics, PcicePipe};
use crate::TampinesError;

/// One verification problem, solver-independent.
#[derive(Debug, Clone)]
pub struct PciceCase<E: HemEquationOfState> {
    /// Short name for messages.
    pub name: &'static str,
    /// Mesh and flow area.
    pub geometry: HemPipeGeometry,
    /// Equation of state.
    pub eos: E,
    /// Initial condition.
    pub initial: HemInitialCondition,
    /// Boundary at `x = 0`.
    pub inlet: PciceBoundary,
    /// Boundary at `x = L`.
    pub outlet: PciceBoundary,
    /// Time at which the case is compared.
    pub end_time: Time,
    /// The PCICE numerics the V&V record used for this case.
    pub numerics: PciceNumerics,
}

/// The `HemPipe` boundary that matches a PCICE one.
fn hem_boundary(b: PciceBoundary) -> HemBoundary {
    match b {
        PciceBoundary::Wall => HemBoundary::Wall,
        PciceBoundary::FixedPressure(p) => HemBoundary::FixedPressure(p),
        PciceBoundary::Reservoir {
            stagnation_pressure,
            stagnation_enthalpy,
        } => HemBoundary::Reservoir {
            stagnation_pressure,
            stagnation_enthalpy,
        },
    }
}

impl<E: HemEquationOfState> PciceCase<E> {
    /// A [`PcicePipe`] set up for this case with [`Self::numerics`].
    ///
    /// # Errors
    ///
    /// As [`PcicePipe::new`] and [`PcicePipe::set_boundary`].
    pub fn build_pcice(&self) -> Result<PcicePipe<E>, TampinesError> {
        let mut p = PcicePipe::new(
            self.geometry,
            self.eos.clone(),
            self.initial.clone(),
            self.numerics,
        )?;
        p.set_boundary(PipeEnd::Inlet, self.inlet)?;
        p.set_boundary(PipeEnd::Outlet, self.outlet)?;
        Ok(p)
    }

    /// The density-based KNP `HemPipe` on the same mesh, EOS, initial state
    /// and boundaries, with wall friction off (PCICE is inviscid).
    ///
    /// # Errors
    ///
    /// As `HemPipe::new` and `HemPipe::set_boundary`.
    pub fn build_hem_pipe(&self, numerics: HemNumerics) -> Result<HemPipe<E>, TampinesError> {
        let mut p = HemPipe::new(
            self.geometry,
            self.eos.clone(),
            self.initial.clone(),
            numerics,
        )?
        .without_wall_friction();
        p.set_boundary(PipeEnd::Inlet, hem_boundary(self.inlet))?;
        p.set_boundary(PipeEnd::Outlet, hem_boundary(self.outlet))?;
        Ok(p)
    }
}

fn gas(p: f64, rho: f64, u: f64) -> PrimitiveState {
    PrimitiveState::PressureDensity {
        pressure: Pressure::new::<pascal>(p),
        density: MassDensity::new::<kilogram_per_cubic_meter>(rho),
        velocity: Velocity::new::<meter_per_second>(u),
    }
}

/// Sod's left state `(p, ρ, u)` in SI: `(1e5 Pa, 1 kg/m³, 0)`.
pub const SOD_LEFT: (f64, f64, f64) = (1.0e5, 1.0, 0.0);
/// Sod's right state: `(1e4 Pa, 0.125 kg/m³, 0)`.
pub const SOD_RIGHT: (f64, f64, f64) = (1.0e4, 0.125, 0.0);
/// Sod's tube length (m) and diaphragm position (m), as in
/// `tampines-steam-tables`' `hem_pipe_verification.rs`.
pub const SOD_LENGTH_M: f64 = 10.0;
/// Diaphragm position, m.
pub const SOD_DIAPHRAGM_M: f64 = 5.0;

/// Sod's comparison time: dimensionless `t = 0.2` on the 10 m tube,
/// `0.2 · 10 / √(1e5)` s.
pub fn sod_end_time() -> Time {
    Time::new::<second>(0.2 * SOD_LENGTH_M / SOD_LEFT.0.sqrt())
}

/// **Sod's shock tube** in SI (air, `γ = 1.4`), closed at both ends (the
/// waves do not reach the ends by [`sod_end_time`]). PCICE numerics: material
/// Courant 0.5, acoustic Courant capped at 0.5, since the shock and the
/// rarefaction are the answer here.
///
/// # Errors
///
/// Only for `n_cells = 0`.
pub fn sod_ideal_gas(n_cells: usize) -> Result<PciceCase<IdealGasHem>, TampinesError> {
    Ok(PciceCase {
        name: "Sod (ideal gas)",
        geometry: HemPipeGeometry::new(
            Length::new::<meter>(SOD_LENGTH_M),
            Area::new::<square_meter>(1.0),
            n_cells,
        )?,
        eos: IdealGasHem::air(),
        initial: HemInitialCondition::Riemann {
            diaphragm: Length::new::<meter>(SOD_DIAPHRAGM_M),
            left: gas(SOD_LEFT.0, SOD_LEFT.1, SOD_LEFT.2),
            right: gas(SOD_RIGHT.0, SOD_RIGHT.1, SOD_RIGHT.2),
        },
        inlet: PciceBoundary::Wall,
        outlet: PciceBoundary::Wall,
        end_time: sod_end_time(),
        numerics: PciceNumerics {
            acoustic_courant_cap: Some(Ratio::new::<ratio>(0.5)),
            ..PciceNumerics::default()
        },
    })
}

/// Low-Mach advection: background pressure, Pa.
pub const LOW_MACH_PRESSURE_PA: f64 = 1.0e5;
/// Background temperature, K.
pub const LOW_MACH_T0_K: f64 = 300.0;
/// Temperature bump amplitude, K.
pub const LOW_MACH_DT_K: f64 = 30.0;
/// Bump centre at `t = 0`, m.
pub const LOW_MACH_X0_M: f64 = 0.3;
/// Bump width `σ` in `exp(−((x − x0)/σ)²)`, m.
pub const LOW_MACH_SIGMA_M: f64 = 0.05;
/// Flow velocity, m/s (Mach ≈ 0.003).
pub const LOW_MACH_U_MS: f64 = 1.0;

/// The exact low-Mach advection solution: uniform `p` and `u`, and the
/// temperature bump carried with the flow, `ρ(x, t) = p / (R T0(x − u t))`.
pub fn low_mach_advection_exact_density(x: Length, t: Time) -> MassDensity {
    let r = IdealGasHem::air().specific_gas_constant.get::<joule_per_kilogram_kelvin>();
    let xi = (x.get::<meter>() - LOW_MACH_U_MS * t.get::<second>() - LOW_MACH_X0_M)
        / LOW_MACH_SIGMA_M;
    let temp = LOW_MACH_T0_K + LOW_MACH_DT_K * (-xi * xi).r_exp();
    MassDensity::new::<kilogram_per_cubic_meter>(LOW_MACH_PRESSURE_PA / (r * temp))
}

/// **Low-Mach advection** of a temperature (density) bump in air at
/// `u = 1 m/s`, `c ≈ 347 m/s`: a 1 m duct, a stagnation reservoir at the inlet
/// holding the background state and a fixed pressure at the outlet. The step
/// is set by the material Courant number 0.5 alone, so the acoustic Courant
/// number is about 175. Compared at `t = 0.3 s`.
///
/// # Errors
///
/// Only for `n_cells = 0`.
pub fn low_mach_advection(n_cells: usize) -> Result<PciceCase<IdealGasHem>, TampinesError> {
    let eos = IdealGasHem::air();
    let geometry = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(1.0),
        n_cells,
    )?;
    let p = LOW_MACH_PRESSURE_PA;
    let u = LOW_MACH_U_MS;
    let per_cell = (0..n_cells)
        .map(|i| {
            let rho = low_mach_advection_exact_density(geometry.cell_centre(i), Time::new::<second>(0.0));
            gas(p, rho.get::<kilogram_per_cubic_meter>(), u)
        })
        .collect();
    let r = eos.specific_gas_constant.get::<joule_per_kilogram_kelvin>();
    let rho0 = p / (r * LOW_MACH_T0_K);
    let h0 = eos.gamma / (eos.gamma - 1.0) * r * LOW_MACH_T0_K + 0.5 * u * u;
    Ok(PciceCase {
        name: "low-Mach advection (ideal gas)",
        geometry,
        eos,
        initial: HemInitialCondition::PerCell(per_cell),
        inlet: PciceBoundary::Reservoir {
            stagnation_pressure: Pressure::new::<pascal>(p + 0.5 * rho0 * u * u),
            stagnation_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(h0),
        },
        outlet: PciceBoundary::FixedPressure(Pressure::new::<pascal>(p)),
        end_time: Time::new::<second>(0.3),
        numerics: PciceNumerics {
            max_dt: Time::new::<second>(1.0),
            ..PciceNumerics::default()
        },
    })
}

/// **Water shock tube on IAPWS-IF97**: compressed liquid at 440 K, 10 MPa on
/// the left and 1 MPa on the right (`T_sat(1 MPa) ≈ 453 K`, so both sides stay
/// liquid), a 10 m closed tube, compared at 2 ms. Acoustic Courant capped at
/// 0.5, as for Sod.
///
/// # Errors
///
/// Only for `n_cells = 0`.
pub fn water_shock_tube(n_cells: usize) -> Result<PciceCase<Iapws97Hem>, TampinesError> {
    let state = |p_mpa: f64| PrimitiveState::PressureTemperature {
        pressure: Pressure::new::<megapascal>(p_mpa),
        temperature: ThermodynamicTemperature::new::<kelvin>(440.0),
        velocity: Velocity::new::<meter_per_second>(0.0),
    };
    Ok(PciceCase {
        name: "water shock tube (IAPWS-IF97)",
        geometry: HemPipeGeometry::new(
            Length::new::<meter>(10.0),
            Area::new::<square_meter>(1.0e-2),
            n_cells,
        )?,
        eos: Iapws97Hem,
        initial: HemInitialCondition::Riemann {
            diaphragm: Length::new::<meter>(5.0),
            left: state(10.0),
            right: state(1.0),
        },
        inlet: PciceBoundary::Wall,
        outlet: PciceBoundary::Wall,
        end_time: Time::new::<second>(2.0e-3),
        numerics: PciceNumerics {
            acoustic_courant_cap: Some(Ratio::new::<ratio>(0.5)),
            ..PciceNumerics::default()
        },
    })
}

/// **A converging–diverging duct at rest** (areas 2 : 1 : 2 m², throat at mid
/// length, 1 m, air at 1e5 Pa and 300 K, walls). The quasi-1-D wall pressure
/// force must balance the face pressure forces exactly, so nothing moves.
///
/// # Errors
///
/// Only for `n_cells = 0`.
pub fn nozzle_at_rest(n_cells: usize) -> Result<PciceCase<IdealGasHem>, TampinesError> {
    Ok(PciceCase {
        name: "converging-diverging duct at rest",
        geometry: HemPipeGeometry::with_area_profile(
            Length::new::<meter>(1.0),
            AreaProfile::ConvergingDiverging {
                inlet_m2: 2.0,
                throat_m2: 1.0,
                outlet_m2: 2.0,
                throat_fraction: 0.5,
            },
            n_cells,
        )?,
        eos: IdealGasHem::air(),
        initial: HemInitialCondition::Uniform(PrimitiveState::PressureTemperature {
            pressure: Pressure::new::<pascal>(1.0e5),
            temperature: ThermodynamicTemperature::new::<kelvin>(300.0),
            velocity: Velocity::new::<meter_per_second>(0.0),
        }),
        inlet: PciceBoundary::Wall,
        outlet: PciceBoundary::Wall,
        end_time: Time::new::<second>(0.1),
        numerics: PciceNumerics::default(),
    })
}
