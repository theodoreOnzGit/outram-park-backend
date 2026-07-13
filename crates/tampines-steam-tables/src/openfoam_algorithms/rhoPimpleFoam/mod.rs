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
use uom::si::f64::{Angle, Area, Length, MassRate, Power, Pressure, ThermalConductance, ThermodynamicTemperature, Time};
use uom::si::time::second;
use crate::openfoam_algorithms::openfoam_source::interface::one_dimensional_meshing::create_one_d_mesh;
use crate::openfoam_algorithms::openfoam_source::*;

mod lateral_coupling;
pub use lateral_coupling::TampinesSteamArrayError;

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

/// One-dimensional compressible PIMPLE pipe array driven by the TAMPINES steam
/// tables.
///
/// This is the tampines-steam-tables analogue of `outram-foam-appbuilder-lib`'s
/// `RhoPimpleFoam`, specialised to a **1-D pipe**: the mesh is built
/// automatically from a length, a cross-sectional area, and a cell count via
/// [`create_one_d_mesh`], instead of being read from an OpenFOAM `polyMesh`
/// directory. It is intended as the transient-flow backbone for coupling the
/// IAPWS-IF97 steam properties into a system-code-style pipe network.
///
/// It solves the same compressible PIMPLE system as `RhoPimpleFoam`:
/// ```text
///   ∂ρ/∂t   + ∇·(ρU)    = 0            (continuity, explicit rhoEqn)
///   ∂(ρU)/∂t + ∇·(ρUU)  = −∇p + ∇·τ    (momentum, UEqn)
///   ∂(ρh)/∂t + ∇·(ρUh)  = dp/dt        (energy, h-form, EEqn)
///   ρ = ψ·p                             (EOS — see `correct_thermo`)
/// ```
///
/// ## What differs from `RhoPimpleFoam`
/// - **Mesh**: a uniform 1-D `FvMesh` (`n_cells` cells along x) rather than an
///   arbitrary polyMesh.
/// - **Control**: a few plain fields (`delta_t`, corrector counts) replace the
///   `ControlDict` / `FvSchemes` / `FvSolution` dictionaries — this crate does
///   not consume OpenFOAM case files.
/// - **Thermophysics**: the placeholder `ρ = ψ·p` EOS in [`Self::correct_thermo`]
///   is the intended plug-in point for the TAMPINES steam tables (see the
///   `EEqn.rs` / `rhoEqn.rs` / `pEqn.rs` reference files in this module).
///
/// C++ reference: `applications/solvers/compressible/rhoPimpleFoam/`.
#[derive(Clone)]
pub struct TampinesSteamArray {
    /// 1-D finite-volume mesh (built by [`create_one_d_mesh`]).
    pub mesh: Arc<FvMesh>,

    // ── Time control ────────────────────────────────────────────────────────
    /// Fixed time step Δt \[s\].
    pub delta_t: Time,
    /// Number of PIMPLE outer correctors (≥ 1).
    pub n_outer_correctors: usize,
    /// Number of PISO pressure correctors per outer loop (≥ 1).
    pub n_inner_correctors: usize,

    // ── Fields ──────────────────────────────────────────────────────────────
    /// Velocity field \[m/s\].
    pub u: VolVectorField,
    /// Pressure field \[Pa\].
    pub p: VolScalarField,
    /// Density field \[kg/m³\].
    pub rho: VolScalarField,
    /// Temperature field \[K\].
    pub t: VolScalarField,
    /// Specific enthalpy \[J/kg\].
    pub he: VolScalarField,
    /// Dynamic viscosity μ \[Pa·s\].
    pub mu: VolScalarField,
    /// Effective thermal diffusivity αh = κ/Cp \[kg/(m·s)\].
    pub alpha_h: VolScalarField,
    /// Compressibility ψ = ∂ρ/∂p|_T = ρ/p \[s²/m²\].
    pub psi: VolScalarField,
    /// Mass flux φ = ρ U·Sf \[kg/s\].
    pub phi: SurfaceScalarField,

    // ── Geometry / flow bookkeeping ─────────────────────────────────────────
    // Mirrors a subset of `outram_park_fork_coolprop::OPCPFluidArray`'s
    // interface (see `lateral_coupling.rs`), which in turn mirrors
    // `tuas_boussinesq_solver::FluidArray` -- so all three backends are
    // driveable through a comparable API.
    /// Constant cross-sectional area \[m²\] (same value passed to [`Self::new`]).
    pub xs_area: Area,
    /// Wetted perimeter \[m\] (bookkeeping -- see [`Self::get_hydraulic_diameter`]).
    pub wetted_perimeter: Length,
    /// Incline angle from horizontal \[rad\] (bookkeeping only).
    pub incline_angle: Angle,
    /// Bulk mass flowrate \[kg/s\] (plain storage -- `step()` does not read
    /// this; it is bookkeeping for a caller, same as `OPCPFluidArray`'s field).
    pub mass_flowrate: MassRate,
    /// Pressure loss \[Pa\] (plain storage, independent of `mass_flowrate`).
    pub pressure_loss: Pressure,
    /// Internal pressure source \[Pa\] (e.g. a simulated pump; plain storage).
    pub internal_pressure_source: Pressure,

    // ── Lateral coupling / heat source (see `lateral_coupling.rs`) ──────────
    /// Per-registered-link neighbour temperature, one inner `Vec` per cell.
    /// Registered via
    /// [`Self::lateral_link_new_temperature_vector_avg_conductance`] and
    /// cleared once per [`Self::step`] (see [`Self::clear_vectors`]).
    pub lateral_adjacent_array_temperature_vector: Vec<Vec<ThermodynamicTemperature>>,
    /// Parallel to `lateral_adjacent_array_temperature_vector`: per-cell
    /// thermal conductance for the same link.
    pub lateral_adjacent_array_conductance_vector: Vec<Vec<ThermalConductance>>,
    /// Per-registered-source total power; distributed across cells by the
    /// matching entry in `q_fraction_vector`.
    pub q_vector: Vec<Power>,
    /// Parallel to `q_vector`: per-cell distribution fraction for the same
    /// source (need not sum to 1).
    pub q_fraction_vector: Vec<Vec<f64>>,
}

impl TampinesSteamArray {
    /// Build a 1-D pipe array with uniform initial conditions.
    ///
    /// The mesh spans x ∈ \[0, `length`\] with `number_of_cells` equal cells and
    /// constant cross-sectional area `xs_area`. Both end patches (`"left"`,
    /// `"right"`) are generic; set field boundary conditions afterwards to impose
    /// inlets/outlets.
    ///
    /// Fields are initialised to an IAPWS-IF97-consistent liquid-water
    /// reference state (p = 1 bar, T = 300 K; ρ, `he`, ψ read from a real
    /// `(T, p)` flash, see [`Self::correct_thermo`]) -- overwrite them after
    /// construction (e.g. via [`Self::set_temperature_vector`]) for a
    /// specific case.
    ///
    /// ## Parameters
    /// - `length`          — total pipe length \[m\]
    /// - `xs_area`         — constant cross-sectional area \[m²\]
    /// - `number_of_cells` — number of cells; must be ≥ 1
    /// - `delta_t`         — fixed time step \[s\]
    ///
    /// ## Errors
    /// Returns [`MeshError::NonPositiveCellCount`] if `number_of_cells < 1`
    /// (propagated from [`create_one_d_mesh`]).
    pub fn new(
        length: Length,
        xs_area: Area,
        number_of_cells: i64,
        delta_t: Time,
    ) -> Result<Self, MeshError> {
        let mesh = Arc::new(create_one_d_mesh(length, xs_area, number_of_cells)?);

        // EOS-consistent reference state at (1 bar, 300 K) -- liquid water,
        // safely within IAPWS-IF97 Region 1's valid range (matches the
        // pattern `outram_park_fork_coolprop::OPCPFluidArray::new` uses for
        // its own initial condition).
        let p0 = uom::si::f64::Pressure::new::<uom::si::pressure::pascal>(1.0e5);
        let t0 = uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(300.0);
        let he0 = crate::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase(t0, p0);
        let v0 = crate::interfaces::functional_programming::pt_flash_eqm::v_tp_eqm_single_phase(t0, p0);
        let rho0 = 1.0 / v0.get::<uom::si::specific_volume::cubic_meter_per_kilogram>();
        let kappa_t0 = crate::interfaces::functional_programming::pt_flash_eqm::kappa_t_tp_eqm(t0, p0).value;
        let psi0 = rho0 * kappa_t0;

        let u       = VolVectorField::zero("U", mesh.clone());
        let p       = VolScalarField::uniform("p", mesh.clone(), p0.get::<uom::si::pressure::pascal>());
        let rho     = VolScalarField::uniform("rho", mesh.clone(), rho0);
        let t       = VolScalarField::uniform("T", mesh.clone(), t0.get::<uom::si::thermodynamic_temperature::kelvin>());
        let he      = VolScalarField::uniform("he", mesh.clone(), he0.get::<uom::si::available_energy::joule_per_kilogram>());
        let mu      = VolScalarField::uniform("mu", mesh.clone(), 1.8e-5);
        let alpha_h = VolScalarField::uniform("alphaEff", mesh.clone(), 2.5e-5);
        let psi     = VolScalarField::uniform("psi", mesh.clone(), psi0);
        let phi     = SurfaceScalarField::zeros("phi", mesh.clone());

        Ok(Self {
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
            wetted_perimeter: Length::new::<uom::si::length::meter>(0.0),
            incline_angle: Angle::new::<uom::si::angle::radian>(0.0),
            mass_flowrate: MassRate::new::<uom::si::mass_rate::kilogram_per_second>(0.0),
            pressure_loss: Pressure::new::<uom::si::pressure::pascal>(0.0),
            internal_pressure_source: Pressure::new::<uom::si::pressure::pascal>(0.0),
            lateral_adjacent_array_temperature_vector: Vec::new(),
            lateral_adjacent_array_conductance_vector: Vec::new(),
            q_vector: Vec::new(),
            q_fraction_vector: Vec::new(),
        })
    }

    /// Update the thermodynamic and transport state from the current
    /// `(p, he)` per cell, via a real IAPWS-IF97 `(p, h)` flash.
    ///
    /// Per cell: `T = t_ph_eqm(p,h)`, `ρ = 1/v_ph_eqm(p,h)`, the local
    /// compressibility `ψ = ∂ρ/∂p|_T = ρ·κ_T` (`κ_T` from `kappa_t_ph_eqm`),
    /// dynamic viscosity `μ = mu_ph_eqm(p,h)`, and the OpenFOAM-convention
    /// effective thermal diffusivity `αh = κ/Cp` (`lambda_ph_eqm` over
    /// `cp_ph_eqm`, **not** divided by ρ -- matches `alphaEff` as used
    /// directly in `step()`'s `∇·(αh∇h)` term, see `EEqn.rs`).
    ///
    /// This replaces the crate's former placeholder EOS (`ρ = ψ·p`, the same
    /// ideal linearisation `RhoPimpleFoam`'s own reference solver uses before
    /// a real property package is wired in). Called once per PISO inner
    /// iteration in [`Self::step`], so -- like `he` itself -- the fields this
    /// writes lag the just-solved `he` by one outer-corrector iteration
    /// (mirrors the same lag documented on
    /// `outram_park_fork_coolprop::OPCPFluidArray::correct_thermo`).
    pub fn correct_thermo(&mut self) {
        use crate::interfaces::functional_programming::ph_flash_eqm::{
            cp_ph_eqm, kappa_t_ph_eqm, lambda_ph_eqm, t_ph_eqm, v_ph_eqm,
        };
        use crate::dynamic_viscosity::mu_ph_eqm;
        use uom::si::available_energy::joule_per_kilogram;
        use uom::si::pressure::pascal;
        use uom::si::specific_volume::cubic_meter_per_kilogram;
        use uom::si::thermodynamic_temperature::kelvin;

        for c in 0..self.mesh.n_cells {
            let p_c = Pressure::new::<pascal>(self.p.internal[c]);
            let h_c = uom::si::f64::AvailableEnergy::new::<joule_per_kilogram>(self.he.internal[c]);

            let t = t_ph_eqm(p_c, h_c);
            let v = v_ph_eqm(p_c, h_c);
            let rho = 1.0 / v.get::<cubic_meter_per_kilogram>();
            let kappa_t = kappa_t_ph_eqm(p_c, h_c).value; // raw Pa^-1 (InversePressure has no named unit)
            let mu = mu_ph_eqm(p_c, h_c);
            let lambda = lambda_ph_eqm(p_c, h_c);
            let cp = cp_ph_eqm(p_c, h_c);

            self.rho.internal[c] = rho.max(1e-4);
            self.t.internal[c] = t.get::<kelvin>();
            self.psi.internal[c] = (rho * kappa_t).max(1e-12);
            self.mu.internal[c] = mu.value;
            self.alpha_h.internal[c] = lambda.value / cp.value;
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

                // EOS update: ρ (and, once wired, T/μ/αh/ψ) from the new pressure.
                self.correct_thermo();
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

                    // Lateral (radial) thermal coupling: Q = h·(T_neighbour − T_cell)
                    // per registered link, plus any registered volumetric heat source.
                    let t_c = self.t.internal[c];
                    for (link, temps) in self.lateral_adjacent_array_conductance_vector
                        .iter()
                        .zip(self.lateral_adjacent_array_temperature_vector.iter())
                    {
                        let h = link[c].get::<uom::si::thermal_conductance::watt_per_kelvin>();
                        let t_n = temps[c].get::<uom::si::thermodynamic_temperature::kelvin>();
                        e_eqn.source[c] += h * (t_n - t_c);
                    }
                    e_eqn.source[c] += self
                        .cell_heat_source_power(c)
                        .get::<uom::si::power::watt>();
                }
            }
            let (he_new, _) = e_eqn.solve("he", settings);
            self.he = he_new;
        }
        self.clear_vectors();
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

    /// The 1-D pipe array constructs from geometry alone and stays finite over a
    /// handful of steps, using the real IAPWS-IF97 `(p, h)` flash in
    /// `correct_thermo`. This is the scaffold smoke test: it exercises mesh
    /// construction + the full ρ/p/U/h coupling, not a specific accuracy claim
    /// (see `correct_thermo`'s own tests for that).
    #[test]
    fn one_d_array_constructs_and_steps() {
        let mut array = TampinesSteamArray::new(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(0.01),
            20,
            Time::new::<second>(1e-4),
        )
        .expect("valid 1-D geometry");

        assert_eq!(array.mesh.n_cells, 20);
        assert_eq!(array.mesh.n_internal_faces, 19);

        array.run(10);

        let all_finite = array.p.internal.as_slice().iter().all(|x| x.is_finite())
            && array.rho.internal.as_slice().iter().all(|x| x.is_finite())
            && array.u.internal.as_slice().iter().all(|v| v.mag().is_finite());
        assert!(all_finite, "fields must stay finite over 10 steps");
    }

    #[test]
    fn zero_cells_is_rejected() {
        let err = TampinesSteamArray::new(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(0.01),
            0,
            Time::new::<second>(1e-4),
        );
        assert!(matches!(err, Err(MeshError::NonPositiveCellCount { got: 0 })));
    }

    #[test]
    fn new_initializes_liquid_water_reference_state_consistently() {
        // (1 bar, 300 K) should be ordinary liquid water: dense, incompressible.
        let array = TampinesSteamArray::new(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(0.01),
            3,
            Time::new::<second>(1e-4),
        )
        .unwrap();
        for c in 0..3 {
            assert!((array.t.internal[c] - 300.0).abs() < 1e-6);
            assert!(
                array.rho.internal[c] > 900.0 && array.rho.internal[c] < 1100.0,
                "rho={} should be liquid-water-like",
                array.rho.internal[c]
            );
            assert!(array.psi.internal[c] > 0.0);
        }
    }

    #[test]
    fn correct_thermo_matches_independent_reference_flash() {
        use crate::interfaces::functional_programming::ph_flash_eqm::{lambda_ph_eqm, t_ph_eqm};

        let mut array = TampinesSteamArray::new(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(0.01),
            1,
            Time::new::<second>(1e-4),
        )
        .unwrap();
        // Push to a different (p, h) point, then re-derive T/rho/etc. via
        // correct_thermo and cross-check against a freshly (independently)
        // called reference flash at the same (p, h) -- not the port's own
        // internal state.
        array.p.internal[0] = 2.0e5;
        array.he.internal[0] = 5.0e5;
        array.correct_thermo();

        let p = Pressure::new::<uom::si::pressure::pascal>(2.0e5);
        let h = uom::si::f64::AvailableEnergy::new::<uom::si::available_energy::joule_per_kilogram>(5.0e5);
        let expected_t = t_ph_eqm(p, h).get::<uom::si::thermodynamic_temperature::kelvin>();
        let expected_lambda = lambda_ph_eqm(p, h).value;
        let expected_alpha_h = expected_lambda
            / crate::interfaces::functional_programming::ph_flash_eqm::cp_ph_eqm(p, h).value;

        assert!((array.t.internal[0] - expected_t).abs() < 1e-6);
        assert!((array.alpha_h.internal[0] - expected_alpha_h).abs() < 1e-9);
        assert!(array.rho.internal[0] > 0.0);
        assert!(array.mu.internal[0] > 0.0);
    }
}
