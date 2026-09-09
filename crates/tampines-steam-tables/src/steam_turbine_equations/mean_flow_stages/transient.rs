//! Transient mean-flow turbine: the stage chain driven by the crate's 1-D
//! HEM KNP hybrid solver (bead `op-yi7m`).
//!
//! # What changes against the steady model
//!
//! [`super::MeanFlowTurbine`] solves the stage chain once per call, with the
//! stage pressures supplied. That is a design-point model: it answers what the
//! machine does at a given operating point, and nothing about how it gets
//! there.
//!
//! This model hands the flow field to [`TampinesSteamArray`] instead. The mesh
//! is the machine, **one cell per stage**, and the solver owns everything the
//! steady model was told:
//!
//! - **Pressure is solved, not supplied.** The pressure distribution along the
//!   machine is whatever the PIMPLE pressure equation produces from the
//!   boundary conditions.
//! - **Axial velocity is solved, not derived from an enthalpy drop.** The
//!   velocity triangle is built on the axial velocity the momentum equation
//!   gives, so the triangle follows the transient rather than a design
//!   assumption.
//! - **The work appears as an energy sink.** Each stage's specific work is
//!   converted to a power using the local mass flux and registered as a
//!   negative per-cell power source, so the energy equation sees the work
//!   leaving through the shaft.
//!
//! # Why the KNP hybrid
//!
//! The constructor sets [`SolverMode::HybridAllMach`], PIMPLE with Mach-blended
//! KNP central-upwind dissipation. Turbine passages run near-sonic at the
//! nozzle throats and the last stages expand into the dome, which is exactly
//! the regime the plain pressure-based path rings in. The hybrid damps that
//! ringing while keeping the flashing behaviour, and it is stable over a full
//! transient. The mode is public, so a caller who wants the historical
//! bit-identical PIMPLE path can still ask for it.
//!
//! # What is still assumed
//!
//! - **Shaft speed is fixed.** The stages report the power they extracted, but
//!   nothing feeds it into a torque balance yet. Coupling to
//!   [`super::super::generator`] is the obvious next step.
//! - **The rotor's share of the local pressure drop is an input**, given per
//!   stage as [`TransientStage::rotor_drop_fraction`], because with pressure
//!   solved rather than supplied there is no interstage station to read it off.
//! - Everything the steady model does not do is still not done: mean-line only,
//!   no radial equilibrium, no spanwise variation, no tip leakage.

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::velocity::meter_per_second;
use uom::si::volume::cubic_meter;

use crate::interfaces::object_oriented_programming::TampinesSteamTableCV;
use crate::openfoam_algorithms::rhoPimpleFoam::{SolverMode, TampinesSteamArray};
use crate::openfoam_algorithms::openfoam_source::mesh::MeshError;

use super::stage::{RotorBlading, StageGeometry, StageWorkSplit};
use super::velocity_triangle::VelocityTriangle;

/// One stage of the transient machine.
///
/// Carries no pressures: in the transient model those are solved by the array,
/// not supplied. What remains is the mean-line geometry, the blading, and how
/// much of the locally solved pressure drop the rotor itself takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransientStage {
    /// Mean-line geometry.
    pub geometry: StageGeometry,
    /// Rotor blading, carrying both the impulse and the reaction parameters.
    pub blading: RotorBlading,
    /// Fraction of this stage's locally solved isentropic drop taken across the
    /// rotor rather than the stator. Zero is a pure impulse rotor; 0.5 is the
    /// classical fifty-percent reaction stage.
    pub rotor_drop_fraction: Ratio,
}

/// What one stage did during one timestep.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransientStageOutcome {
    /// The mean-radius triangle this stage ran at, built on the solved axial
    /// velocity.
    pub triangle: VelocityTriangle,
    /// How the extracted work divided between the two mechanisms.
    pub work_split: StageWorkSplit,
    /// Mass flow through this cell, from the solved density and velocity.
    pub mass_flow: MassRate,
    /// Shaft power this stage extracted. Positive means work leaving the steam.
    pub shaft_power: Power,
}

/// What the machine did during one timestep.
#[derive(Debug, Clone, PartialEq)]
pub struct TransientStepOutcome {
    /// One entry per stage, in flow order.
    pub stage_outcomes: Vec<TransientStageOutcome>,
}

impl TransientStepOutcome {
    /// Total shaft power over all stages.
    pub fn total_shaft_power(&self) -> Power {
        self.stage_outcomes
            .iter()
            .fold(Power::new::<uom::si::power::watt>(0.0), |sum, outcome| {
                sum + outcome.shaft_power
            })
    }
}

/// A multistage axial steam turbine solved in time on the 1-D HEM array.
#[derive(Debug, Clone)]
pub struct TransientMeanFlowTurbine {
    /// The 1-D HEM solver. One cell per stage, and public so a caller can set
    /// boundary conditions, initial conditions and solver controls directly.
    pub array: TampinesSteamArray,
    /// Stages in flow order, one per cell of `array`.
    pub stages: Vec<TransientStage>,
    /// Shaft speed, shared by every stage and held fixed for now.
    pub shaft_speed: AngularVelocity,
}

impl TransientMeanFlowTurbine {
    /// Builds the machine, meshing one cell per stage and selecting the KNP
    /// hybrid solver mode.
    ///
    /// `length` is the axial extent of the whole blade path and `xs_area` its
    /// mean annulus area. Both are uniform, which is the mean-line assumption
    /// showing up in the mesh: a real machine opens its annulus toward the
    /// exhaust, and this does not.
    pub fn new(
        stages: Vec<TransientStage>,
        length: Length,
        xs_area: Area,
        delta_t: Time,
        shaft_speed: AngularVelocity,
    ) -> Result<Self, MeshError> {
        let mut array = TampinesSteamArray::new(length, xs_area, stages.len() as i64, delta_t)?;

        // Turbine passages run near-sonic at the nozzle throats and the last
        // stages expand into the dome. That is the regime the plain
        // pressure-based path rings in, so the hybrid is the right default here
        // even though it is not the array's own default.
        array.set_solver_mode(SolverMode::HybridAllMach);

        Ok(Self {
            array,
            stages,
            shaft_speed,
        })
    }

    /// Sets every cell to one uniform `(p, T)` state.
    ///
    /// The array's own initial condition is liquid water at 1 bar and 300 K,
    /// which is not a useful starting point for a steam turbine, so this is
    /// almost always the first call after construction.
    pub fn initialise_uniform(
        &mut self,
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
    ) {
        let specific_enthalpy =
            crate::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase(
                temperature,
                pressure,
            );

        for cell in 0..self.array.mesh.n_cells {
            self.array.p.internal[cell] = pressure.get::<pascal>();
            self.array.he.internal[cell] = specific_enthalpy.get::<joule_per_kilogram>();
        }

        // rho, T, psi and the transport properties all follow from (p, he).
        self.array.correct_thermo();
    }

    /// The steam state of one stage, as a control volume.
    ///
    /// This is the "one control volume per stage" of the steady model, except
    /// that here the state is read back out of the solver each timestep rather
    /// than being marched by the control volume itself.
    pub fn stage_control_volume(&self, stage_index: usize) -> TampinesSteamTableCV {
        let pressure = Pressure::new::<pascal>(self.array.p.internal[stage_index]);
        let specific_enthalpy =
            AvailableEnergy::new::<joule_per_kilogram>(self.array.he.internal[stage_index]);
        let volume = Volume::new::<cubic_meter>(self.array.mesh.cell_volumes[stage_index]);

        TampinesSteamTableCV::new_from_ph(pressure, specific_enthalpy, volume)
    }

    /// Advances the machine one timestep.
    ///
    /// Per stage, in order: read the solved state, build the triangle on the
    /// solved axial velocity, take both parts of the work, convert to a power
    /// with the local mass flux, and register it as a negative power source.
    /// Then step the array once, so the energy equation sees the work leave.
    pub fn step(&mut self) -> TransientStepOutcome {
        let stage_outcomes = self.evaluate_stages();

        let n_cells = self.array.mesh.n_cells;
        for (index, outcome) in stage_outcomes.iter().enumerate() {
            // One one-hot source per stage rather than one distributed source:
            // each stage extracts a different power, and a one-hot fraction
            // vector avoids dividing by a total that can legitimately be zero
            // before the flow starts.
            let mut fractions = vec![0.0; n_cells];
            fractions[index] = 1.0;

            // Negative: shaft work leaves the steam.
            self.array
                .lateral_link_new_power_vector(-outcome.shaft_power, fractions)
                .expect("fraction vector is built at mesh.n_cells length");
        }

        self.array.step();

        TransientStepOutcome { stage_outcomes }
    }

    /// Advances `n_steps` timesteps, returning the last step's outcome.
    pub fn run(&mut self, n_steps: usize) -> Option<TransientStepOutcome> {
        let mut last = None;

        for _ in 0..n_steps {
            last = Some(self.step());
        }

        last
    }

    /// Evaluates every stage against the currently solved field, without
    /// touching the solver.
    fn evaluate_stages(&self) -> Vec<TransientStageOutcome> {
        (0..self.stages.len())
            .map(|index| self.evaluate_stage(index))
            .collect()
    }

    fn evaluate_stage(&self, index: usize) -> TransientStageOutcome {
        let stage = self.stages[index];

        let blade_speed: Velocity = self.shaft_speed * stage.geometry.mean_radius;

        // The axial velocity comes from the momentum equation, not from an
        // assumed enthalpy drop. That is what makes this model transient: the
        // triangle follows the solved flow.
        let axial_velocity = Velocity::new::<meter_per_second>(self.array.u.internal[index].mag());

        // The stator turns the flow to its exit angle, so the absolute velocity
        // is the solved axial component divided by the cosine of that angle.
        let absolute_velocity_in = axial_velocity / stage.geometry.nozzle_angle.cos();

        let triangle = VelocityTriangle::new_from_nozzle_and_blade_angles(
            blade_speed,
            absolute_velocity_in,
            stage.geometry.nozzle_angle,
            stage.geometry.relative_exit_angle,
            stage.blading.blade_velocity_coefficient,
        );

        let rotor_enthalpy_drop = self.rotor_enthalpy_drop(index);
        let work_split = stage
            .blading
            .specific_work_split(&triangle, rotor_enthalpy_drop);

        let density = MassDensity::new::<kilogram_per_cubic_meter>(self.array.rho.internal[index]);
        let mass_flow: MassRate = density * axial_velocity * self.array.xs_area;

        let shaft_power: Power = work_split.total() * mass_flow;

        TransientStageOutcome {
            triangle,
            work_split,
            mass_flow,
            shaft_power,
        }
    }

    /// The isentropic enthalpy drop taken across this stage's rotor.
    ///
    /// The stage's own pressure fall is read from the solved field, as the drop
    /// to the next cell. The last cell has no downstream neighbour, so its drop
    /// is extrapolated linearly from the previous one, which is the usual
    /// zero-second-derivative outflow treatment. A single-cell machine gets no
    /// drop at all, and therefore no reaction part.
    fn rotor_enthalpy_drop(&self, index: usize) -> AvailableEnergy {
        let n_cells = self.array.mesh.n_cells;
        let pressure = self.array.p.internal[index];

        let downstream_pressure = if index + 1 < n_cells {
            self.array.p.internal[index + 1]
        } else if index >= 1 {
            pressure - (self.array.p.internal[index - 1] - pressure)
        } else {
            pressure
        };

        if downstream_pressure >= pressure || downstream_pressure <= 0.0 {
            return AvailableEnergy::new::<joule_per_kilogram>(0.0);
        }

        let inlet = self.stage_control_volume(index);
        let mut expanded = inlet;
        expanded.expand_isentropically(Pressure::new::<pascal>(downstream_pressure));

        let stage_drop = inlet.get_specific_enthalpy() - expanded.get_specific_enthalpy();
        let rotor_share = self.stages[index].rotor_drop_fraction.get::<ratio>();

        stage_drop * rotor_share
    }
}
