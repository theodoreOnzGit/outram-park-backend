///*---------------------------------------------------------------------------*\
//  =========                 |
//  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
//   \\    /   O peration     |
//    \\  /    A nd           | www.openfoam.com
//     \\/     M anipulation  |
//-------------------------------------------------------------------------------
//    Copyright (C) 2011-2017 OpenFOAM Foundation
//    Copyright (C) 2019 OpenCFD Ltd.
//-------------------------------------------------------------------------------
//License
//    This file is part of OpenFOAM.
//
//    OpenFOAM is free software: you can redistribute it and/or modify it
//    under the terms of the GNU General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    OpenFOAM is distributed in the hope that it will be useful, but WITHOUT
//    ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
//    FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
//    for more details.
//
//    You should have received a copy of the GNU General Public License
//    along with OpenFOAM.  If not, see <http://www.gnu.org/licenses/>.
//
//Application
//    rhoPimpleFoam
//
//Group
//    grpCompressibleSolvers
//
//Description
//    Transient solver for turbulent flow of compressible fluids for HVAC and
//    similar applications, with optional mesh motion and mesh topology changes.
//
//    Uses the flexible PIMPLE (PISO-SIMPLE) solution for time-resolved and
//    pseudo-transient simulations.
//
//Note
//   The motion frequency of this solver can be influenced by the presence
//   of "updateControl" and "updateInterval" in the dynamicMeshDict.
//
//\*---------------------------------------------------------------------------*/
//
//#include "fvCFD.H"
//#include "dynamicFvMesh.H"
//#include "fluidThermo.H"
//#include "turbulentFluidThermoModel.H"
//#include "bound.H"
//#include "pimpleControl.H"
//#include "pressureControl.H"
//#include "CorrectPhi.H"
//#include "fvOptions.H"
//#include "localEulerDdtScheme.H"
//#include "fvcSmooth.H"
//
//// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //
//
//int main(int argc, char *argv[])
//{
//    argList::addNote
//    (
//        "Transient solver for compressible turbulent flow.\n"
//        "With optional mesh motion and mesh topology changes."
//    );
//
//    #include "postProcess.H"
//
//    #include "addCheckCaseOptions.H"
//    #include "setRootCaseLists.H"
//    #include "createTime.H"
//    #include "createDynamicFvMesh.H"
//    #include "createDyMControls.H"
//    #include "initContinuityErrs.H"
//    #include "createFields.H"
//    #include "createFieldRefs.H"
//    #include "createRhoUfIfPresent.H"
//
//    turbulence->validate();
//
//    if (!LTS)
//    {
//        #include "compressibleCourantNo.H"
//        #include "setInitialDeltaT.H"
//    }
//
//    // * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //
//
//    Info<< "\nStarting time loop\n" << endl;
//
//    while (runTime.run())
//    {
//        #include "readDyMControls.H"
//
//        // Store divrhoU from the previous mesh so that it can be mapped
//        // and used in correctPhi to ensure the corrected phi has the
//        // same divergence
//        autoPtr<volScalarField> divrhoU;
//        if (correctPhi)
//        {
//            divrhoU.reset
//            (
//                new volScalarField
//                (
//                    "divrhoU",
//                    fvc::div(fvc::absolute(phi, rho, U))
//                )
//            );
//        }
//
//        if (LTS)
//        {
//            #include "setRDeltaT.H"
//        }
//        else
//        {
//            #include "compressibleCourantNo.H"
//            #include "setDeltaT.H"
//        }
//
//        ++runTime;
//
//        Info<< "Time = " << runTime.timeName() << nl << endl;
//
//        // --- Pressure-velocity PIMPLE corrector loop
//        while (pimple.loop())
//        {
//            if (pimple.firstIter() || moveMeshOuterCorrectors)
//            {
//                // Store momentum to set rhoUf for introduced faces.
//                autoPtr<volVectorField> rhoU;
//                if (rhoUf.valid())
//                {
//                    rhoU.reset(new volVectorField("rhoU", rho*U));
//                }
//
//                // Do any mesh changes
//                mesh.controlledUpdate();
//
//                if (mesh.changing())
//                {
//                    MRF.update();
//
//                    if (correctPhi)
//                    {
//                        // Calculate absolute flux
//                        // from the mapped surface velocity
//                        phi = mesh.Sf() & rhoUf();
//
//                        #include "correctPhi.H"
//
//                        // Make the fluxes relative to the mesh-motion
//                        fvc::makeRelative(phi, rho, U);
//                    }
//
//                    if (checkMeshCourantNo)
//                    {
//                        #include "meshCourantNo.H"
//                    }
//                }
//            }
//
//            if (pimple.firstIter() && !pimple.SIMPLErho())
//            {
//                #include "rhoEqn.H"
//            }
//
//            #include "UEqn.H"
//            #include "EEqn.H"
//
//            // --- Pressure corrector loop
//            while (pimple.correct())
//            {
//                if (pimple.consistent())
//                {
//                    #include "pcEqn.H"
//                }
//                else
//                {
//                    #include "pEqn.H"
//                }
//            }
//
//            if (pimple.turbCorr())
//            {
//                turbulence->correct();
//            }
//        }
//
//        rho = thermo.rho();
//
//        runTime.write();
//
//        runTime.printExecutionTime(Info);
//    }
//
//    Info<< "End\n" << endl;
//
//    return 0;
//}
//
//
//// ************************************************************************* //

use std::sync::Arc;
use uom::si::f64::{
    Angle, Area, Length, MassRate, Power, Pressure, Time, ThermalConductance, ThermodynamicTemperature,
};
use uom::si::time::second;
use crate::openfoam_algorithms::openfoam_source::interface::one_dimensional_meshing::create_one_d_mesh;
use crate::openfoam_algorithms::openfoam_source::*;
use crate::fluid::Fluid;
use crate::flash;

mod lateral_coupling;
pub use lateral_coupling::OPCPFluidArrayError;

// ── Boundary-condition helpers ──────────────────────────────────────────────────
//
// The linear solver and field arithmetic rebuild output fields with zero-gradient
// boundaries, so the prescribed BC *types* (a fixed inlet velocity, an outlet
// pressure) are lost after every `solve()` / arithmetic op. These helpers snapshot
// a BC template and re-apply it — the equivalent of OpenFOAM's
// `field.correctBoundaryConditions()`. Local copies of the appbuilder `bc_util`
// helpers so this crate needs only `outram-foam-basic-lib`, not the solver crate.

/// Snapshot the per-patch boundary-condition template of a field, to be
/// re-applied after solves with [`correct_bcs`] / [`correct_bcs_vec`].
fn capture_bcs<T: Clone>(boundary: &[PatchField<T>]) -> Vec<BoundaryCondition<T>> {
    boundary.iter().map(|pf| pf.bc.clone()).collect()
}

/// Re-apply a scalar BC template to a field. `FixedValue` faces are reset to the
/// fixed value; other BC types keep the operator-recomputed face values.
fn correct_bcs(field: &mut VolScalarField, bcs: &[BoundaryCondition<f64>]) {
    for (pf, bc) in field.boundary.iter_mut().zip(bcs) {
        pf.bc = bc.clone();
        if let BoundaryCondition::FixedValue(v) = bc {
            for x in pf.values.iter_mut() { *x = *v; }
        }
    }
}

/// Vector counterpart of [`correct_bcs`].
fn correct_bcs_vec(field: &mut VolVectorField, bcs: &[BoundaryCondition<Vector3>]) {
    for (pf, bc) in field.boundary.iter_mut().zip(bcs) {
        pf.bc = bc.clone();
        if let BoundaryCondition::FixedValue(v) = bc {
            for x in pf.values.iter_mut() { *x = *v; }
        }
    }
}

/// One-dimensional compressible PIMPLE pipe array driven by the **CoolProp
/// Helmholtz EOS** (this crate's [`crate::Fluid`] / [`crate::flash`]).
///
/// This is the CoolProp-fork analogue of `outram-foam-appbuilder-lib`'s
/// `RhoPimpleFoam`, specialised to a **1-D pipe**: the mesh is built
/// automatically from a length, a cross-sectional area, and a cell count via
/// [`create_one_d_mesh`], instead of being read from an OpenFOAM `polyMesh`
/// directory. It is the field-based (1-D array) counterpart of the 0-D
/// [`crate::OPCPFluidSingleCV`], and the transient-flow backbone for coupling
/// CoolProp fluid properties into a system-code-style pipe network.
///
/// It solves the same compressible PIMPLE system as `RhoPimpleFoam`:
/// ```text
///   ∂ρ/∂t   + ∇·(ρU)    = 0            (continuity, explicit rhoEqn)
///   ∂(ρU)/∂t + ∇·(ρUU)  = −∇p + ∇·τ    (momentum, UEqn)
///   ∂(ρh)/∂t + ∇·(ρUh)  = dp/dt        (energy, h-form, EEqn)
///   ρ = ρ(p, h),  ψ = (∂ρ/∂p)_T        (EOS — see `correct_thermo`)
/// ```
///
/// ## What differs from `RhoPimpleFoam`
/// - **Mesh**: a uniform 1-D `FvMesh` (`n_cells` cells along x) rather than an
///   arbitrary polyMesh.
/// - **Control**: a few plain fields (`delta_t`, corrector counts) replace the
///   `ControlDict` / `FvSchemes` / `FvSolution` dictionaries — this crate does
///   not consume OpenFOAM case files.
/// - **Thermophysics**: [`Self::correct_thermo`] does a per-cell single-phase
///   `(p, h)` flash on the stored [`fluid`](Self::fluid) via [`crate::flash`],
///   updating `ρ`, `T` and the compressibility `ψ = (∂ρ/∂p)_T` from the CoolProp
///   Helmholtz EOS (replacing the old placeholder `ρ = ψ·p`).
///   [`Self::correct_transport`] then updates `μ`/`αh` from
///   [`crate::transport::viscosity`]/[`crate::transport::conductivity`] at the
///   refreshed `(T, ρ)` — for a fluid/state with no transport model (`None`)
///   a cell's `μ`/`αh` are left at their previous value, never a wrong number.
///
/// C++ reference: `applications/solvers/compressible/rhoPimpleFoam/`.
#[derive(Clone,Debug)]
pub struct OPCPFluidArray {
    /// The working fluid whose CoolProp Helmholtz EOS closes the thermo update.
    pub fluid: Fluid,
    /// 1-D finite-volume mesh (built by [`create_one_d_mesh`]).
    pub mesh: Arc<FvMesh>,

    // ── Time control ────────────────────────────────────────────────────────
    /// Fixed time step Δt [s].
    pub delta_t: Time,
    /// Number of PIMPLE outer correctors (≥ 1).
    pub n_outer_correctors: usize,
    /// Number of PISO pressure correctors per outer loop (≥ 1).
    pub n_inner_correctors: usize,

    // ── Fields ──────────────────────────────────────────────────────────────
    /// Velocity field [m/s].
    pub u: VolVectorField,
    /// Pressure field [Pa].
    pub p: VolScalarField,
    /// Density field [kg/m³].
    pub rho: VolScalarField,
    /// Temperature field [K].
    pub t: VolScalarField,
    /// Specific enthalpy [J/kg].
    pub he: VolScalarField,
    /// Dynamic viscosity μ [Pa·s].
    pub mu: VolScalarField,
    /// Effective thermal diffusivity αh = κ/Cp [kg/(m·s)].
    pub alpha_h: VolScalarField,
    /// Compressibility ψ = (∂ρ/∂p)_T [s²/m²], from the EOS in `correct_thermo`.
    pub psi: VolScalarField,
    /// Mass flux φ = ρ U·Sf [kg/s].
    pub phi: SurfaceScalarField,

    // ── Pipe geometry (bookkeeping only — not read by `step()`) ──────────────
    /// Constant cross-sectional area \[m²\], duplicating the value baked into
    /// the mesh at construction — kept explicit so [`Self::get_hydraulic_diameter`]
    /// doesn't need to reach into mesh internals.
    pub xs_area: Area,
    /// Wetted perimeter \[m\], used with `xs_area` for the hydraulic diameter
    /// `D_h = 4·xs_area/wetted_perimeter` (see [`Self::get_hydraulic_diameter`]).
    /// Defaults to zero at construction — set it before relying on that method.
    pub wetted_perimeter: Length,
    /// Incline angle from horizontal \[rad\] — bookkeeping for a future
    /// hydrostatic-pressure term in a pipe-network layer built on top of this
    /// array. Not read by `step()`.
    pub incline_angle: Angle,

    // ── Flow bookkeeping (independent of the PIMPLE `u`/`phi` solve) ─────────
    /// Bulk mass flowrate \[kg/s\] — plain storage for a caller-driven pipe-
    /// network layer. **Not** read by `step()`: this solver computes its own
    /// per-cell mass flux `phi` from the momentum/pressure equations, so this
    /// field never feeds back into the PIMPLE loop.
    pub mass_flowrate: MassRate,
    /// Pressure loss \[Pa\] — plain storage, independent of `mass_flowrate`
    /// (no Reynolds/Bejan recomputation between the two; see
    /// [`Self::set_mass_flowrate`]).
    pub pressure_loss: Pressure,
    /// Internal pressure source \[Pa\] (e.g. a simulated pump) — plain storage.
    pub internal_pressure_source: Pressure,

    // ── Lateral (radial) thermal coupling ─────────────────────────────────────
    /// One entry per laterally-connected array/solid; each inner `Vec` has one
    /// temperature per mesh cell (length `mesh.n_cells`). Populated by
    /// [`Self::lateral_link_new_temperature_vector_avg_conductance`], consumed
    /// and cleared once per [`Self::step`] (see [`Self::clear_vectors`]).
    pub lateral_adjacent_array_temperature_vector: Vec<Vec<ThermodynamicTemperature>>,
    /// Parallel to `lateral_adjacent_array_temperature_vector`: per-cell
    /// thermal conductance \[W/K\] to each laterally-connected array.
    pub lateral_adjacent_array_conductance_vector: Vec<Vec<ThermalConductance>>,

    // ── Volumetric heat source ─────────────────────────────────────────────────
    /// One entry per registered heat source; total power `q_vector[i]` is
    /// distributed across cells by `q_fraction_vector[i]` (see
    /// [`Self::lateral_link_new_power_vector`]).
    pub q_vector: Vec<Power>,
    /// Parallel to `q_vector`: per-cell distribution fraction (length
    /// `mesh.n_cells`, need not sum to 1 — mirrors TUAS's `q_fraction_vector`).
    pub q_fraction_vector: Vec<Vec<f64>>,
}

impl OPCPFluidArray {
    /// Build a 1-D pipe array of `fluid` with uniform initial conditions.
    ///
    /// The mesh spans x ∈ \[0, `length`\] with `number_of_cells` equal cells and
    /// constant cross-sectional area `xs_area`. Both end patches (`"left"`,
    /// `"right"`) are generic; set field boundary conditions afterwards to impose
    /// inlets/outlets.
    ///
    /// Fields are initialised to a **single-phase reference state at p = 1 bar,
    /// T = 300 K**, with `ρ`, the specific enthalpy `h` and the compressibility
    /// `ψ = (∂ρ/∂p)_T` taken from `fluid`'s CoolProp Helmholtz EOS (so the first
    /// `correct_thermo` `(p, h)` flash is well-posed). If that reference `(p, T)`
    /// is not single-phase for `fluid` (e.g. a liquid, which the single-phase
    /// solver cannot reach), it falls back to inert placeholders
    /// (ρ = 1 kg/m³, h = 0, ψ = 1e-5 s²/m²); overwrite the fields after
    /// construction for a specific case. `μ`/`αh` start at fixed placeholders
    /// (air-like values) and are overwritten by [`Self::correct_transport`] on
    /// the first call — call it once after construction if the initial step
    /// should already use the fluid's real transport properties.
    ///
    /// ## Parameters
    /// - `fluid`           — working fluid (its EOS closes the thermo update)
    /// - `length`          — total pipe length \[m\]
    /// - `xs_area`         — constant cross-sectional area \[m²\]
    /// - `number_of_cells` — number of cells; must be ≥ 1
    /// - `delta_t`         — fixed time step \[s\]
    ///
    /// ## Errors
    /// Returns [`MeshError::NonPositiveCellCount`] if `number_of_cells < 1`
    /// (propagated from [`create_one_d_mesh`]).
    pub fn new(
        fluid: Fluid,
        length: Length,
        xs_area: Area,
        number_of_cells: i64,
        delta_t: Time,
    ) -> Result<Self, MeshError> {
        let mesh = Arc::new(create_one_d_mesh(length, xs_area, number_of_cells)?);

        // EOS-consistent reference state at (1 bar, 300 K), with a placeholder
        // fallback if that point is not single-phase for this fluid.
        let (p0, t0) = (1.0e5_f64, 300.0_f64);
        let (rho0, he0, psi0) = match flash::state_pt(fluid, t0, p0) {
            Ok(s) => (s.density, s.enthalpy, flash::drho_dp_t(fluid, t0, s.density)),
            Err(_) => (1.0, 0.0, 1.0e-5),
        };

        let u       = VolVectorField::zero("U", mesh.clone());
        let p       = VolScalarField::uniform("p", mesh.clone(), p0);
        let rho     = VolScalarField::uniform("rho", mesh.clone(), rho0);
        let t       = VolScalarField::uniform("T", mesh.clone(), t0);
        let he      = VolScalarField::uniform("he", mesh.clone(), he0);
        let mu      = VolScalarField::uniform("mu", mesh.clone(), 1.8e-5);
        let alpha_h = VolScalarField::uniform("alphaEff", mesh.clone(), 2.5e-5);
        let psi     = VolScalarField::uniform("psi", mesh.clone(), psi0);
        let phi     = SurfaceScalarField::zeros("phi", mesh.clone());

        Ok(Self {
            fluid,
            mesh,
            delta_t,
            n_outer_correctors: 1,
            n_inner_correctors: 2,
            u,
            p,
            rho,
            t,
            he,
            mu,
            alpha_h,
            psi,
            phi,
            xs_area,
            wetted_perimeter: Length::new::<meter>(0.0),
            incline_angle: Angle::new::<radian>(0.0),
            mass_flowrate: MassRate::new::<kilogram_per_second>(0.0),
            pressure_loss: Pressure::new::<pascal>(0.0),
            internal_pressure_source: Pressure::new::<pascal>(0.0),
            lateral_adjacent_array_temperature_vector: Vec::new(),
            lateral_adjacent_array_conductance_vector: Vec::new(),
            q_vector: Vec::new(),
            q_fraction_vector: Vec::new(),
        })
    }

    /// Update the thermodynamic state from the current pressure and enthalpy,
    /// using the fluid's **CoolProp Helmholtz EOS**.
    ///
    /// For each cell this does a single-phase `(p, h)` flash ([`crate::flash`]):
    /// given the cell pressure `p` and specific enthalpy `he`, it looks up the
    /// density `ρ`, temperature `T` and compressibility `ψ = (∂ρ/∂p)_T`, writing
    /// them back into the `rho`, `t` and `psi` fields (`ψ` closes the pressure
    /// equation — see `pEqn`). This replaces the old placeholder `ρ = ψ·p`.
    ///
    /// If a cell's `(p, h)` does not converge to a single-phase state (e.g. it
    /// falls in the two-phase dome, which CoolProp does not yet model), that
    /// cell's `ρ`/`T`/`ψ` are **left at their previous values** rather than set
    /// to a wrong number — so the solve stays finite. Transport (`μ`, `αh`) is
    /// not updated here (CoolProp transport properties are a follow-up).
    pub fn correct_thermo(&mut self) {
        for c in 0..self.mesh.n_cells {
            let p_c = self.p.internal[c];
            let h_c = self.he.internal[c];
            if let Ok(state) = flash::state_ph(self.fluid, p_c, h_c) {
                self.rho.internal[c] = state.density.max(1e-4);
                self.t.internal[c] = state.temperature;
                self.psi.internal[c] = flash::drho_dp_t(self.fluid, state.temperature, state.density).max(1e-12);
            }
            // else: keep the previous ρ/T/ψ for this cell (finite, safe).
        }
    }

    /// Update the per-cell transport fields (dynamic viscosity `μ` and thermal
    /// diffusivity `αh = λ/c_p`) from the CoolProp transport correlations at the
    /// current `(T, ρ)` — the transport half of `correct_thermo`.
    ///
    /// For each cell this looks up `μ(fluid, T, ρ)` and `λ(fluid, T, ρ)`
    /// ([`crate::transport::viscosity`] / [`crate::transport::conductivity`])
    /// and divides the latter by `c_p` (from [`crate::props::state_trho`]) to
    /// get `αh` in OpenFOAM's mass-diffusivity convention
    /// (`kg/(m·s)` = `W/(m·K)` / `J/(kg·K)`) — matching `alpha_h`'s existing use
    /// in [`Self::step`]'s momentum (`μ`) and energy (`αh`) diffusion terms.
    ///
    /// If `fluid` has no transport model at a cell's `(T, ρ)` (`None` — never a
    /// wrong number, see [`crate::transport`]), that cell's `μ`/`αh` are left at
    /// their previous value, mirroring `correct_thermo`'s non-convergence
    /// handling.
    pub fn correct_transport(&mut self) {
        for c in 0..self.mesh.n_cells {
            let t_c = self.t.internal[c];
            let rho_c = self.rho.internal[c];
            if let Some(mu) = crate::transport::viscosity(self.fluid, t_c, rho_c) {
                self.mu.internal[c] = mu;
            }
            if let Some(lambda) = crate::transport::conductivity(self.fluid, t_c, rho_c) {
                let cp = crate::props::state_trho(self.fluid, t_c, rho_c).cp;
                if cp.is_finite() && cp > 0.0 {
                    self.alpha_h.internal[c] = lambda / cp;
                }
            }
        }
    }

    /// Advance one time step with the compressible PIMPLE algorithm.
    ///
    /// Ported line-for-line from `RhoPimpleFoam::step` (see that solver's module
    /// doc for the sign/convention rationale). The steps: explicit continuity
    /// (rhoEqn) → momentum predictor (UEqn) → PISO pressure-correction loop with
    /// the ψ·V/dt compressibility diagonal (pEqn) → energy equation (EEqn).
    /// Boundary conditions are re-applied after every field update.
    pub fn step(&mut self) {
        let mesh = self.mesh.clone();
        let n    = mesh.n_cells;
        let dt   = self.delta_t.get::<second>();
        let settings   = SolverSettings::default();                            // U, energy (GS)
        let p_settings = SolverSettings { tolerance: 1e-8, max_iter: 2_000 };  // pEqn (PCG)
        let n_outer = self.n_outer_correctors.max(1);
        let n_inner = self.n_inner_correctors.max(1);

        let u_old   = self.u.clone();
        let p_old   = self.p.clone();
        let he_old  = self.he.clone();
        let rho_old = self.rho.clone();

        let u_bcs = capture_bcs(&self.u.boundary);
        let p_bcs = capture_bcs(&self.p.boundary);

        for _ in 0..n_outer {
            // ── rhoEqn: explicit continuity ρ = ρ_old − dt·∇·φ ──────────────
            let div_phi = fvc::div_flux(&self.phi);
            self.rho = rho_old.clone() + (-dt) * div_phi;
            for c in 0..n {
                if self.rho.internal[c] < 1e-4 {
                    self.rho.internal[c] = 1e-4;
                }
            }

            // ── UEqn: ∂(ρU)/∂t + ∇·(ρUU) + (−∇·(μ∇U)) ─────────────────────
            let mut u_eqn = fvm::ddt_coeff_vec(&self.rho, &self.u, &u_old, dt, mesh.clone())
                + fvm::div_vec(&self.phi, &self.u, mesh.clone())
                + fvm::laplacian_vec(&self.mu, &self.u, mesh.clone());

            // A [kg/s]; rAU = V/A [m³·s/kg]
            let a = u_eqn.a_field();
            let rau = {
                let a_sl = a.internal.as_slice();
                let vals: Vec<f64> = (0..n)
                    .map(|c| mesh.cell_volumes[c] / a_sl[c].max(1e-30))
                    .collect();
                VolScalarField::new(
                    "rAU",
                    mesh.clone(),
                    Field::new(vals),
                    mesh.patches
                        .iter()
                        .map(|p| PatchField::zero_gradient(p.size))
                        .collect(),
                )
            };

            // Momentum predictor with explicit −V·∇p.
            let gp = fvc::grad(&self.p);
            for c in 0..n {
                u_eqn.source[c] = u_eqn.source[c] - gp.internal[c] * mesh.cell_volumes[c];
            }
            let (mut u_pred, _) = u_eqn.solve("U", settings);
            correct_bcs_vec(&mut u_pred, &u_bcs);
            for c in 0..n {
                u_eqn.source[c] = u_eqn.source[c] + gp.internal[c] * mesh.cell_volumes[c];
            }
            self.u = u_pred;

            let rauf = fvc::interpolate(&rau);

            // ── PISO pressure-correction loop (H(U) re-evaluated each pass) ──
            for _ in 0..n_inner {
                // HbyA = H(U)/A [m/s] from the latest U.
                let h = u_eqn.h_field(&self.u);
                let hbya = {
                    let h_sl = h.internal.as_slice();
                    let a_sl = a.internal.as_slice();
                    let vals: Vec<Vector3> = (0..n)
                        .map(|c| h_sl[c] * (1.0 / a_sl[c].max(1e-30)))
                        .collect();
                    VolVectorField::new(
                        "HbyA", mesh.clone(), Field::new(vals),
                        mesh.patches.iter().map(|p| PatchField::zero_gradient_vec(p.size)).collect(),
                    )
                };

                let rho_f    = fvc::interpolate(&self.rho);    // ρ_f [kg/m³]
                let rho_rauf = rho_f.clone() * rauf.clone();    // [s]
                // φ_HbyA = ρ_f · flux(HbyA): mass flux [kg/s]
                let mut phi_hbya = rho_f.clone() * fvc::flux(&hbya);

                // Pressure source = ψ·V/dt·p_old − (net φ_HbyA outflow) [kg/s].
                let psi_sl   = self.psi.internal.as_slice();
                let p_old_sl = p_old.internal.as_slice();
                let source_p = {
                    let mut s = vec![0.0_f64; n];
                    let phi_int = phi_hbya.internal.as_slice();
                    for f in 0..mesh.n_internal_faces {
                        s[mesh.owner[f]]     -= phi_int[f];
                        s[mesh.neighbour[f]] += phi_int[f];
                    }
                    for (pi, patch) in mesh.patches.iter().enumerate() {
                        if matches!(self.u.boundary[pi].bc, BoundaryCondition::Empty) {
                            continue;
                        }
                        for fi in 0..patch.size {
                            let gf = patch.start + fi;
                            let flux = match self.u.boundary[pi].bc {
                                BoundaryCondition::FixedValue(ubc) =>
                                    rho_f.boundary[pi].values[fi]
                                        * ubc.dot(mesh.face_area_vectors[gf]),
                                // outlet / zero-gradient: keep the extrapolated flux
                                _ => phi_hbya.boundary[pi].values[fi],
                            };
                            s[mesh.owner[gf]] -= flux;
                        }
                    }
                    for c in 0..n {
                        s[c] += psi_sl[c] * mesh.cell_volumes[c] / dt * p_old_sl[c];
                    }
                    s
                };

                // pEqn: [L(ρ_f·rAU_f) + ψ·V/dt]·p = source. The ψ·V/dt diagonal
                // makes the system non-singular (no reference cell needed); it is
                // symmetric SPD → PCG.
                let mut p_eqn = fvm::laplacian(&rho_rauf, &self.p);
                for c in 0..n {
                    p_eqn.ldu.diag[c] += psi_sl[c] * mesh.cell_volumes[c] / dt;
                }
                p_eqn.source = Field::new(source_p);
                let (mut p_new, _) = p_eqn.solve_cg("p", p_settings);
                correct_bcs(&mut p_new, &p_bcs);
                self.p = p_new;

                // Correct the mass flux: φ = φ_HbyA − ρ_f·rAU_f·snGrad(p)·|Sf|.
                let sng = fvc::sn_grad(&self.p);
                {
                    let sng_sl      = sng.internal.as_slice();
                    let rho_rauf_sl = rho_rauf.internal.as_slice();
                    for f in 0..mesh.n_internal_faces {
                        phi_hbya.internal[f] -= rho_rauf_sl[f] * sng_sl[f] * mesh.face_areas[f];
                    }
                    self.phi = phi_hbya;
                }

                // U = HbyA − rAU·∇p, re-impose BCs.
                self.u = hbya - rau.clone() * fvc::grad(&self.p);
                correct_bcs_vec(&mut self.u, &u_bcs);

                // EOS update: ρ, T, ψ from the new pressure, then μ, αh from
                // the refreshed (T, ρ).
                self.correct_thermo();
                self.correct_transport();
            }

            // ── Energy equation ─────────────────────────────────────────────
            //   ∂(ρh)/∂t + ∇·(φh) + (−∇·(αh∇h)) = dp/dt   [+ laplacian sign]
            let conv_he   = fvc::div(&self.phi, &self.he);   // explicit ∇·(φh)/V
            let alpha_h_f = fvc::interpolate(&self.alpha_h);
            let dp_dt     = (self.p.clone() - p_old.clone()) * (1.0 / dt);

            let mut e_eqn = fvm::ddt_coeff(&self.rho, &self.he, &he_old, dt)
                + fvm::laplacian(&alpha_h_f, &self.he);
            {
                let conv_sl = conv_he.internal.as_slice();
                let dpdt_sl = dp_dt.internal.as_slice();
                for c in 0..n {
                    let v = mesh.cell_volumes[c];
                    e_eqn.source[c] -= v * conv_sl[c]; // explicit convection
                    e_eqn.source[c] += v * dpdt_sl[c]; // dp/dt source
                }
            }
            let (he_new, _) = e_eqn.solve("he", settings);
            self.he = he_new;
        }
    }

    /// Advance `n_steps` time steps of size `delta_t`.
    pub fn run(&mut self, n_steps: usize) {
        for _ in 0..n_steps {
            self.step();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::area::square_meter;
    use uom::si::length::meter;
    use uom::si::time::second;

    /// The 1-D pipe array constructs and stays finite over a handful of steps
    /// with the CoolProp-EOS `correct_thermo`. Nitrogen at the (1 bar, 300 K)
    /// reference state is a clean single-phase gas, so the per-cell `(p, h)`
    /// flash converges and the real EOS thermo path is exercised (not just the
    /// fallback). This is a smoke test — full ρ/p/U/h coupling + mesh, not
    /// physical accuracy.
    #[test]
    fn one_d_array_constructs_and_steps() {
        let mut array = OPCPFluidArray::new(
            Fluid::Nitrogen,
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(0.01),
            20,
            Time::new::<second>(1e-4),
        )
        .expect("valid 1-D geometry");

        assert_eq!(array.mesh.n_cells, 20);
        assert_eq!(array.mesh.n_internal_faces, 19);

        // The EOS path is live (not the placeholder fallback): initial ρ and h
        // match Nitrogen's CoolProp state at (1 bar, 300 K), not ρ=1 / h=0.
        let eos = flash::state_pt(Fluid::Nitrogen, 300.0, 1.0e5).unwrap();
        assert!((array.rho.internal[0] - eos.density).abs() / eos.density < 1e-9);
        assert!((array.he.internal[0] - eos.enthalpy).abs() / eos.enthalpy.abs() < 1e-9);
        assert!((array.rho.internal[0] - 1.0).abs() > 0.05, "ρ must be EOS-derived, not the fallback 1.0");

        array.run(10);

        let all_finite = array.p.internal.as_slice().iter().all(|x| x.is_finite())
            && array.rho.internal.as_slice().iter().all(|x| x.is_finite())
            && array.t.internal.as_slice().iter().all(|x| x.is_finite())
            && array.u.internal.as_slice().iter().all(|v| v.mag().is_finite());
        assert!(all_finite, "fields must stay finite over 10 steps");
        // Density and temperature stay physically bounded (correct_thermo ran).
        assert!(array.rho.internal.as_slice().iter().all(|&r| r > 0.0 && r < 1e3));
        assert!(array.t.internal.as_slice().iter().all(|&tt| tt > 0.0 && tt < 5e3));
    }

    #[test]
    fn correct_transport_updates_mu_and_alpha_h_from_eos() {
        let mut array = OPCPFluidArray::new(
            Fluid::Nitrogen,
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(0.01),
            5,
            Time::new::<second>(1e-4),
        )
        .expect("valid 1-D geometry");

        array.correct_transport();

        let mu1 = array.mu.internal[0];
        let alpha_h1 = array.alpha_h.internal[0];
        let expected_mu = crate::transport::viscosity(Fluid::Nitrogen, array.t.internal[0], array.rho.internal[0]).unwrap();
        let expected_lambda = crate::transport::conductivity(Fluid::Nitrogen, array.t.internal[0], array.rho.internal[0]).unwrap();
        let expected_cp = crate::props::state_trho(Fluid::Nitrogen, array.t.internal[0], array.rho.internal[0]).cp;

        // Nitrogen's real viscosity at (300K, ~1atm) happens to sit close to
        // the constructor's air-like placeholder (both ~1.8e-5 Pa.s), so this
        // checks the exact EOS-derived value rather than "changed from
        // placeholder" (which would be a coincidental, fragile check here).
        assert!((mu1 - expected_mu).abs() / expected_mu < 1e-9, "mu should now be the EOS transport value");
        assert!(
            (alpha_h1 - expected_lambda / expected_cp).abs() / (expected_lambda / expected_cp) < 1e-9,
            "alpha_h should now be lambda/cp from the EOS"
        );
    }

    #[test]
    fn zero_cells_is_rejected() {
        let err = OPCPFluidArray::new(
            Fluid::Nitrogen,
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(0.01),
            0,
            Time::new::<second>(1e-4),
        );
        assert!(matches!(err, Err(MeshError::NonPositiveCellCount { got: 0 })));
    }
}
