use crate::openfoam_algorithms::openfoam_source::interface::one_dimensional_meshing::create_one_d_mesh;
use crate::openfoam_algorithms::openfoam_source::*;
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
    Angle, Area, Length, MassRate, Power, Pressure, Ratio, ThermalConductance,
    ThermodynamicTemperature, Time,
};
use uom::si::ratio::ratio;
use uom::si::time::second;

mod lateral_coupling;
pub use lateral_coupling::TampinesSteamArrayError;

mod central_upwind;
use central_upwind::{
    central_face_flux, hem_sound_speed_ph, knp_face_flux, mach_blend, velocity_component,
    FaceState, C_MIN_MPS,
};

/// Selects the flux discretisation used by [`TampinesSteamArray::step`].
///
/// This is an **opt-in** switch (enum dispatch — no trait objects, per the
/// workspace design rules). The default [`SolverMode::Pimple`] runs the
/// pressure-based compressible PIMPLE algorithm exactly as before, bit-for-bit
/// (the recent Edwards flashing-plateau fix and every existing test are
/// preserved by construction). [`SolverMode::HybridAllMach`] additionally
/// injects a **Mach-weighted KNP central-upwind dissipation** (see the
/// `central_upwind` module) as a deferred-correction flux, active only on
/// near-sonic faces (`β(Ma) > 0`), to damp the ringing at a near-sonic flashing
/// front while leaving subsonic regions untouched.
///
/// [`SolverMode::HybridAllMach`] damps the near-sonic ringing (~55 % less excess
/// total variation over 0–0.15 s) while retaining the Edwards flashing plateau
/// (≈ 388 psia). It is **stable over the full 600 ms transient**: the earlier
/// late-time instability (an emptying-pipe near-sonic cell driven across the
/// `(p,h)` 273.15 K validity edge past t ≈ 0.18 s, bug `op-21g.15.7`) is fixed by
/// the rarefied-tail density taper on the KNP dissipation — see
/// [`TampinesSteamArray::assemble_hybrid_dissipation`]. The default
/// [`SolverMode::Pimple`] remains bit-for-bit the historical validated path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SolverMode {
    /// Pressure-based compressible **HEM-closed PIMPLE** — the historical,
    /// validated, default path (recovers the Edwards flashing plateau; stable
    /// over the full 600 ms transient).
    #[default]
    Pimple,
    /// PIMPLE + Mach-blended KNP shock-capturing dissipation (all-Mach hybrid).
    /// Damps the near-sonic ringing at the flashing front (~55 %) while retaining
    /// the flashing plateau, and is stable over the full 600 ms Edwards transient
    /// (rarefied-tail density taper, bug `op-21g.15.7`). The default
    /// [`SolverMode::Pimple`] is the bit-identical historical path.
    HybridAllMach,
}

/// Per-`step` hybrid KNP dissipation for the continuity and momentum equations.
/// Both fields are the deferred-correction contribution `β·(knp − central)·|Sf|`
/// summed appropriately; every entry is identically zero on a subsonic
/// (`β = 0`) face, so the default `Pimple` path never sees it.
///
/// ## Why there is no separate energy term
///
/// The continuity dissipation is folded into `phi` **before** the EEqn's
/// `rho_cont`/`conv_he` recompute, so the enthalpy shock-capturing is carried
/// *implicitly* by the EEqn's `∇·(φh)` convection — the plateau-fix cancellation
/// `(rho_cont − rho_old)/dt = −∇·φ` transports the dissipative enthalpy for
/// free. Adding a *separate* explicit `ΔF_ener` source on top double-counts that
/// enthalpy transport and breaks the finely-balanced plateau cancellation: in
/// testing it either over-drained the near-break cell below the 273.15 K
/// isotherm or, when scaled down to stay stable, over-damped and suppressed the
/// physical flashing front. Energy shock-capturing therefore rides on the
/// continuity dissipation, not a standalone source (bead op-21g.15.6 —
/// documented in `collaboration/edwards_tampines_regen/hybrid_debug_log.md`).
struct HybridDissipation {
    /// Dissipative mass flux to add to `phi` \[kg/s\], one per internal face.
    d_phi: Vec<f64>,
    /// Per-cell momentum source \[N\] (owner-loses / neighbour-gains sum).
    mom_src: Vec<Vector3>,
}

/// Lower mixture-density threshold \[kg/m³\] of the rarefied-tail taper on the
/// all-Mach hybrid KNP dissipation. **Below** this the KNP dissipation is scaled
/// to **zero** (rarefied emptying tail ⇒ pure PIMPLE, which is stable over the
/// full transient). See [`TampinesSteamArray::assemble_hybrid_dissipation`] for
/// the physical rationale and the full-transient stability fix (bug op-21g.15.7).
const HYBRID_RHO_TAPER_LO: f64 = 50.0;

/// Upper mixture-density threshold \[kg/m³\] of the rarefied-tail taper. **At or
/// above** this the KNP dissipation is applied at full weight (the dense
/// two-phase flashing front where the ringing lives — its minimum dissipated
/// face density is measured at ≈ 106.5 kg/m³ over the 0–0.15 s physics window, so
/// the front sits entirely in the full-weight band and the ~55 % ringing
/// reduction / ≈ 388 psia plateau are unchanged). Between `LO` and `HI` the blend
/// ramps linearly. See [`TampinesSteamArray::assemble_hybrid_dissipation`].
const HYBRID_RHO_TAPER_HI: f64 = 100.0;

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
            for x in pf.values.iter_mut() {
                *x = *v;
            }
        }
    }
}

/// Vector counterpart of [`correct_bcs`].
fn correct_bcs_vec(field: &mut VolVectorField, bcs: &[BoundaryCondition<Vector3>]) {
    for (pf, bc) in field.boundary.iter_mut().zip(bcs) {
        pf.bc = bc.clone();
        if let BoundaryCondition::FixedValue(v) = bc {
            for x in pf.values.iter_mut() {
                *x = *v;
            }
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
///   ρ, T, ψ, μ, αh from a real IAPWS-IF97 (p,h) flash (see `correct_thermo`)
/// ```
///
/// ## What differs from `RhoPimpleFoam`
/// - **Mesh**: a uniform 1-D `FvMesh` (`n_cells` cells along x) rather than an
///   arbitrary polyMesh.
/// - **Control**: a few plain fields (`delta_t`, corrector counts) replace the
///   `ControlDict` / `FvSchemes` / `FvSolution` dictionaries — this crate does
///   not consume OpenFOAM case files.
/// - **Thermophysics**: [`Self::correct_thermo`] closes the EOS with a real
///   IAPWS-IF97 `(p, h)` flash (not a placeholder linearisation) — see that
///   method's doc comment for the exact per-cell property list.
///
/// C++ reference: `applications/solvers/compressible/rhoPimpleFoam/`.
#[derive(Clone)]
pub struct TampinesSteamArray {
    /// 1-D finite-volume mesh (built by [`create_one_d_mesh`]).
    pub mesh: Arc<FvMesh>,

    // ── Time control ────────────────────────────────────────────────────────
    /// Fixed time step Δt \[s\].
    pub delta_t: Time,
    /// Number of PIMPLE outer correctors (≥ 1). See
    /// [`Self::set_piso_algorithm`] / [`Self::set_simple_algorithm`] /
    /// [`Self::set_pimple_algorithm`] for the PISO/SIMPLE/PIMPLE presets.
    pub n_outer_correctors: usize,
    /// Number of PISO pressure correctors per outer loop (≥ 1).
    pub n_inner_correctors: usize,
    /// Explicit pressure under-relaxation factor α_p ∈ (0, 1] applied once
    /// per inner correction: `p ← p_prev + α_p·(p_solved − p_prev)`.
    /// `1.0` (the [`Self::new`] default, matching classic transient PISO)
    /// takes each correction in full; smaller values trade convergence
    /// speed for stability in iterative (SIMPLE-style) solves.
    pub p_under_relaxation: Ratio,
    /// Explicit velocity under-relaxation factor α_u ∈ (0, 1] -- see
    /// [`Self::p_under_relaxation`].
    pub u_under_relaxation: Ratio,
    /// Lower pressure bound \[Pa\] applied after every pressure solve (see
    /// [`Self::step`]). Defaults to the IAPWS-IF97 lower validity limit
    /// (triple-point pressure ≈ 611.657 Pa); raise it with
    /// [`Self::set_pressure_bounds`] to clamp a violent transient (e.g. a
    /// water-hammer rarefaction that would otherwise undershoot to negative
    /// absolute pressure) instead of letting the `(p, h)` flash panic
    /// out-of-range. This mirrors OpenFOAM's `pressureControl::limit`
    /// `pMin`/`pMax` bounding — see [`Self::step`] for the reference.
    pub p_min: Pressure,
    /// Upper pressure bound \[Pa\] applied after every pressure solve.
    /// Defaults to the IAPWS-IF97 upper validity limit (100 MPa). See
    /// [`Self::p_min`].
    pub p_max: Pressure,

    // ── All-Mach hybrid (opt-in) ─────────────────────────────────────────────
    /// Flux-discretisation mode (default [`SolverMode::Pimple`], bit-identical
    /// to the historical path). See [`Self::set_solver_mode`].
    pub mode: SolverMode,
    /// Lower Mach threshold `lo` of the hybrid blend window
    /// `β(Ma) = clamp((Ma−lo)/(hi−lo), 0, 1)` (default `0.3`, dimensionless).
    /// Below `lo` the KNP dissipation is identically zero. Only read when
    /// `mode == HybridAllMach`. See [`Self::set_mach_blend_window`].
    pub ma_blend_lo: Ratio,
    /// Upper Mach threshold `hi` of the hybrid blend window (default `1.0`,
    /// dimensionless). At/above `hi` the KNP dissipation is applied at full
    /// weight. See [`Self::set_mach_blend_window`].
    pub ma_blend_hi: Ratio,

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
    /// Compressibility ψ = ∂ρ/∂p|_h \[s²/m²\] — the density's response to
    /// pressure at **fixed enthalpy**, the correct linearisation for this
    /// segregated pressure equation (he is frozen during the pressure solve).
    /// Computed by a central finite difference of the real IAPWS-IF97 `(p, h)`
    /// flash — see [`Self::correct_thermo`]. In single phase this equals the
    /// isothermal ρ·κ_T; in the two-phase dome it is much larger because it
    /// carries the flashing term `(v_g − v_f)·dx/dp`.
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
        let t0 = uom::si::f64::ThermodynamicTemperature::new::<
            uom::si::thermodynamic_temperature::kelvin,
        >(300.0);
        let he0 =
            crate::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase(t0, p0);
        let v0 =
            crate::interfaces::functional_programming::pt_flash_eqm::v_tp_eqm_single_phase(t0, p0);
        let rho0 = 1.0 / v0.get::<uom::si::specific_volume::cubic_meter_per_kilogram>();
        let kappa_t0 =
            crate::interfaces::functional_programming::pt_flash_eqm::kappa_t_tp_eqm(t0, p0).value;
        let psi0 = rho0 * kappa_t0;

        let u = VolVectorField::zero("U", mesh.clone());
        let p = VolScalarField::uniform("p", mesh.clone(), p0.get::<uom::si::pressure::pascal>());
        let rho = VolScalarField::uniform("rho", mesh.clone(), rho0);
        let t = VolScalarField::uniform(
            "T",
            mesh.clone(),
            t0.get::<uom::si::thermodynamic_temperature::kelvin>(),
        );
        let he = VolScalarField::uniform(
            "he",
            mesh.clone(),
            he0.get::<uom::si::available_energy::joule_per_kilogram>(),
        );
        let mu = VolScalarField::uniform("mu", mesh.clone(), 1.8e-5);
        let alpha_h = VolScalarField::uniform("alphaEff", mesh.clone(), 2.5e-5);
        let psi = VolScalarField::uniform("psi", mesh.clone(), psi0);
        let phi = SurfaceScalarField::zeros("phi", mesh.clone());

        Ok(Self {
            mesh,
            delta_t,
            n_outer_correctors: 1,
            n_inner_correctors: 2,
            p_under_relaxation: Ratio::new::<ratio>(1.0),
            u_under_relaxation: Ratio::new::<ratio>(1.0),
            // Default pressure bounds = the IAPWS-IF97 validity range
            // (triple-point pressure ≈ 611.657 Pa up to 100 MPa). See
            // `step` for the OpenFOAM `pressureControl` reference. The lower
            // bound is nudged 0.1% *above* the exact 273.15 K saturation
            // pressure: the `(p, h)` validity guard classifies its 273.15 K
            // isotherm via a `(T, p)` single-phase flash, and a cell clamped
            // to *exactly* `p_sat(273.15 K)` would land on the saturation
            // line and hit that flash's two-phase `todo!()`. Staying just
            // inside Region 1 avoids it.
            p_min: crate::region_4_vap_liq_equilibrium::sat_pressure_4(
                ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(273.15),
            ) * 1.001,
            p_max: Pressure::new::<uom::si::pressure::megapascal>(100.0),
            // Default: pure PIMPLE ⇒ the hybrid dissipation is never assembled,
            // so every existing constructor/test runs the unchanged code path.
            mode: SolverMode::Pimple,
            ma_blend_lo: Ratio::new::<ratio>(0.3),
            ma_blend_hi: Ratio::new::<ratio>(1.0),
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
    /// compressibility `ψ = ∂ρ/∂p|_h` (central finite difference of the `(p,h)`
    /// flash — the fixed-enthalpy compressibility the segregated pressure
    /// equation needs; captures two-phase flashing compliance),
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
        use crate::dynamic_viscosity::mu_ph_eqm;
        use crate::interfaces::functional_programming::ph_flash_eqm::{
            cp_ph_eqm, kappa_t_ph_eqm, lambda_ph_eqm, t_ph_eqm, v_ph_eqm,
        };
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
            let mu = mu_ph_eqm(p_c, h_c);
            let lambda = lambda_ph_eqm(p_c, h_c);
            let cp = cp_ph_eqm(p_c, h_c);

            // Compressibility ψ for the pressure equation = ∂ρ/∂p **at fixed
            // enthalpy**, computed by a central finite difference of the real
            // (p, h) flash. This is the physically correct linearisation for
            // this *segregated* algorithm: within a pressure-correction inner
            // iteration `he` is frozen (it is only updated by the energy
            // equation after the inner loop), so the density's response to the
            // pressure change is `∂ρ/∂p|_h`, not the isothermal `∂ρ/∂p|_T`.
            //
            // In single phase the two agree (for an ideal gas ∂ρ/∂p|_h = ρ/p =
            // ρ·κ_T exactly; for liquid ∂ρ/∂h|_p is tiny so |_h ≈ |_T), so this
            // leaves the subcooled/superheated behaviour unchanged. **Inside
            // the two-phase dome they differ by ~2 orders of magnitude**: the
            // frozen quality-weighted isothermal value `κ_T = x·κ_vap +
            // (1−x)·κ_liq` (`kappa_t_ph_eqm`, Region 4) omits the flashing term
            // `(v_g − v_f)·dx/dp`, whereas `∂ρ/∂p|_h` includes it because the
            // (p, h) flash re-solves the equilibrium quality at each pressure.
            // That flashing compliance is exactly what pins a two-phase cell on
            // the saturation line (`p = p_sat(T)`) as it depressurises — the
            // Edwards flashing plateau. With the frozen isothermal ψ the
            // pressure-eqn diagonal `ψ·V/dt` is ~100× too small in two phase
            // and the pressure overshoots straight through the plateau.
            let p_pa = self.p.internal[c];
            let p_min_pa = self.p_min.get::<pascal>();
            let p_max_pa = self.p_max.get::<pascal>();
            let dp = (p_pa * 1.0e-3).max(50.0);
            let p_hi = (p_pa + dp).min(p_max_pa);
            let p_lo = (p_pa - dp).max(p_min_pa);
            let psi_fd = if p_hi > p_lo {
                let rho_hi = 1.0
                    / v_ph_eqm(Pressure::new::<pascal>(p_hi), h_c)
                        .get::<cubic_meter_per_kilogram>();
                let rho_lo = 1.0
                    / v_ph_eqm(Pressure::new::<pascal>(p_lo), h_c)
                        .get::<cubic_meter_per_kilogram>();
                (rho_hi - rho_lo) / (p_hi - p_lo)
            } else {
                // Degenerate (both bounds clamped together): fall back to the
                // isothermal value so ψ stays defined at the EOS-range edges.
                rho * kappa_t_ph_eqm(p_c, h_c).value
            };

            self.rho.internal[c] = rho.max(1e-4);
            self.t.internal[c] = t.get::<kelvin>();
            self.psi.internal[c] = psi_fd.max(1e-12);
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
        let n = mesh.n_cells;
        let dt = self.delta_t.get::<second>();
        let settings = SolverSettings::default(); // U, energy (GS)
        let p_settings = SolverSettings {
            tolerance: 1e-8,
            max_iter: 2_000,
        }; // pEqn (PCG)
        let n_outer = self.n_outer_correctors.max(1);
        let n_inner = self.n_inner_correctors.max(1);

        let u_old = self.u.clone();
        let p_old = self.p.clone();
        let he_old = self.he.clone();
        let rho_old = self.rho.clone();

        let u_bcs = capture_bcs(&self.u.boundary);
        let p_bcs = capture_bcs(&self.p.boundary);

        // Hybrid-mode only: deferred KNP momentum dissipation, carried from one
        // outer corrector into the next outer corrector's UEqn source (a
        // one-corrector deferred-correction lag). Stays all-zero in `Pimple`
        // mode, so the momentum predictor is untouched by construction.
        let mut hybrid_mom_src = vec![Vector3::ZERO; n];

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

            // Hybrid: fold the deferred KNP momentum dissipation into the UEqn
            // source so the momentum predictor and every H(U) re-evaluation in
            // the pressure loop see it. Zero (no-op) in `Pimple` mode.
            if self.mode == SolverMode::HybridAllMach {
                for c in 0..n {
                    u_eqn.source[c] = u_eqn.source[c] + hybrid_mom_src[c];
                }
            }

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

            // ── PISO/SIMPLE pressure-correction loop (H(U) re-evaluated each pass) ──
            let alpha_p = self.p_under_relaxation.get::<ratio>();
            let alpha_u = self.u_under_relaxation.get::<ratio>();
            for _ in 0..n_inner {
                // Values at the start of this inner correction -- under-
                // relaxation (see `Self::p_under_relaxation`/`u_under_relaxation`)
                // blends each correction's *change* into these rather than
                // taking it in full. alpha = 1.0 (the default, classic
                // transient PISO) makes this a no-op.
                let p_prev_iter = self.p.clone();
                let u_prev_iter = self.u.clone();

                // HbyA = H(U)/A [m/s] from the latest U.
                let h = u_eqn.h_field(&self.u);
                let hbya = {
                    let h_sl = h.internal.as_slice();
                    let a_sl = a.internal.as_slice();
                    let vals: Vec<Vector3> = (0..n)
                        .map(|c| h_sl[c] * (1.0 / a_sl[c].max(1e-30)))
                        .collect();
                    VolVectorField::new(
                        "HbyA",
                        mesh.clone(),
                        Field::new(vals),
                        mesh.patches
                            .iter()
                            .map(|p| PatchField::zero_gradient_vec(p.size))
                            .collect(),
                    )
                };

                let rho_f = fvc::interpolate(&self.rho); // ρ_f [kg/m³]
                let rho_rauf = rho_f.clone() * rauf.clone(); // [s]
                                                             // φ_HbyA = ρ_f · flux(HbyA): mass flux [kg/s]
                let mut phi_hbya = rho_f.clone() * fvc::flux(&hbya);

                // Pressure source = ψ·V/dt·p_old − (net φ_HbyA outflow) [kg/s].
                let psi_sl = self.psi.internal.as_slice();
                let p_old_sl = p_old.internal.as_slice();
                let source_p = {
                    let mut s = vec![0.0_f64; n];
                    {
                        let phi_int = phi_hbya.internal.as_slice();
                        for f in 0..mesh.n_internal_faces {
                            s[mesh.owner[f]] -= phi_int[f];
                            s[mesh.neighbour[f]] += phi_int[f];
                        }
                    }
                    for (pi, patch) in mesh.patches.iter().enumerate() {
                        if matches!(self.u.boundary[pi].bc, BoundaryCondition::Empty) {
                            continue;
                        }
                        for fi in 0..patch.size {
                            let gf = patch.start + fi;
                            let flux = match self.u.boundary[pi].bc {
                                BoundaryCondition::FixedValue(ubc) => {
                                    let corrected_flux = rho_f.boundary[pi].values[fi]
                                        * ubc.dot(mesh.face_area_vectors[gf]);
                                    // `hbya`'s own boundary field is always
                                    // zero_gradient_vec (see its construction
                                    // above), so `phi_hbya`'s boundary value
                                    // at this patch does NOT reflect the
                                    // actual prescribed velocity BC. Without
                                    // this write-back, `self.phi = phi_hbya`
                                    // below would silently keep the wrong
                                    // boundary flux, corrupting the *next*
                                    // step's rhoEqn continuity at this patch
                                    // -- this was the root cause of a large
                                    // spurious pressure oscillation under a
                                    // nonzero inlet velocity BC (see
                                    // `lateral_coupling.rs`'s
                                    // `inlet_outlet_bcs_drive_flow_and_outlet_pressure_settles_near_imposed_value`
                                    // regression test).
                                    phi_hbya.boundary[pi].values[fi] = corrected_flux;
                                    corrected_flux
                                }
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
                // ADD the mass-flux + ψ·V/dt source to the laplacian's own
                // source rather than OVERWRITING it: `fvm::laplacian` already
                // put each FixedValue pressure boundary's Dirichlet source
                // contribution (`coeff·p_bc`) into `p_eqn.source`, and its
                // matching `coeff` into the diagonal. Overwriting the source
                // dropped the `coeff·p_bc` term while keeping the diagonal
                // one, which silently imposed `p_boundary = 0` instead of the
                // prescribed value -- so any fixed-pressure outlet drove its
                // owner cell toward zero and blew up (a spurious disturbance
                // even from a uniform equilibrium field). Shared with
                // `outram_park_fork_coolprop::OPCPFluidArray::step`.
                for (s, &sp) in p_eqn.source.iter_mut().zip(source_p.iter()) {
                    *s += sp;
                }
                let (mut p_new, _) = p_eqn.solve_cg("p", p_settings);
                correct_bcs(&mut p_new, &p_bcs);
                // Explicit pressure under-relaxation (no-op at alpha_p = 1.0):
                // internal cells only -- the Dirichlet boundary values just
                // applied by correct_bcs are the prescribed BC, not a solved
                // quantity, so they are not relaxed.
                for c in 0..n {
                    p_new.internal[c] = p_prev_iter.internal[c]
                        + alpha_p * (p_new.internal[c] - p_prev_iter.internal[c]);
                }

                // ── Pressure bounding (OpenFOAM `pressureControl::limit`) ──
                // Clamp the solved pressure into [p_min, p_max] so a violent
                // transient (e.g. a water-hammer rarefaction that undershoots
                // to negative absolute pressure) cannot drive the next
                // `correct_thermo` (p, h) flash outside the IAPWS-IF97 valid
                // range. With the default bounds (= the EOS validity range)
                // this only reshapes states the flash could not evaluate
                // anyway; raise `p_min` via `set_pressure_bounds` for a
                // tighter (e.g. cavitation-floor) clamp.
                //
                // Directly mirrors OpenFOAM's compressible pressure control,
                // which likewise limits *pressure* (not density) for robust
                // start-up with complex equations of state. From
                // `pressureControl::limit`
                // (src/finiteVolume/cfdTools/general/pressureControl/pressureControl.C,
                // OpenFOAM Foundation, GPL-3.0):
                //
                // ```cpp
                // bool Foam::pressureControl::limit(volScalarField& p) const
                // {
                //     if (limitMaxP_ || limitMinP_)
                //     {
                //         if (limitMaxP_)
                //         {
                //             const scalar pMax = max(p).value();
                //             if (pMax > pMax_.value())
                //             {
                //                 Info<< "pressureControl: p max " << pMax << endl;
                //                 p = min(p, pMax_);
                //             }
                //         }
                //         if (limitMinP_)
                //         {
                //             const scalar pMin = min(p).value();
                //             if (pMin < pMin_.value())
                //             {
                //                 Info<< "pressureControl: p min " << pMin << endl;
                //                 p = max(p, pMin_);
                //             }
                //         }
                //         return true;
                //     }
                //     else
                //     {
                //         return false;
                //     }
                // }
                // ```
                //
                // Note: `f64::clamp` leaves a NaN unchanged, so a genuinely
                // diverged (NaN) field is *not* masked here — it flows on to
                // the flash rather than being silently pinned to a bound.
                let p_min_pa = self.p_min.get::<uom::si::pressure::pascal>();
                let p_max_pa = self.p_max.get::<uom::si::pressure::pascal>();
                for pv in p_new.internal.iter_mut() {
                    *pv = pv.clamp(p_min_pa, p_max_pa);
                }
                self.p = p_new;

                // Correct the mass flux: φ = φ_HbyA − ρ_f·rAU_f·snGrad(p)·|Sf|.
                let sng = fvc::sn_grad(&self.p);
                {
                    let sng_sl = sng.internal.as_slice();
                    let rho_rauf_sl = rho_rauf.internal.as_slice();
                    for f in 0..mesh.n_internal_faces {
                        phi_hbya.internal[f] -= rho_rauf_sl[f] * sng_sl[f] * mesh.face_areas[f];
                    }
                    self.phi = phi_hbya;
                }

                // U = HbyA − rAU·∇p, re-impose BCs.
                let mut u_new = hbya - rau.clone() * fvc::grad(&self.p);
                correct_bcs_vec(&mut u_new, &u_bcs);
                // Explicit velocity under-relaxation (no-op at alpha_u = 1.0);
                // internal cells only, same rationale as pressure above.
                for c in 0..n {
                    u_new.internal[c] = u_prev_iter.internal[c]
                        + (u_new.internal[c] - u_prev_iter.internal[c]) * alpha_u;
                }
                self.u = u_new;

                // EOS update: ρ (and, once wired, T/μ/αh/ψ) from the new pressure.
                self.correct_thermo();
            }

            // ── Hybrid all-Mach KNP dissipation (deferred correction) ────────
            // Assembled from the just-converged primitives. The continuity
            // dissipation is folded into `self.phi` HERE, *before* the EEqn's
            // `rho_cont`/`conv_he` recompute below both read `self.phi`, so the
            // discrete continuity invariant `(rho_cont − rho_old)/dt = −∇·φ`
            // still holds exactly, the plateau-fix `h·∇·φ` cancellation survives,
            // AND the enthalpy shock-capturing is carried implicitly by that
            // convection (so no separate, destabilising energy source is added —
            // see `HybridDissipation`). Momentum dissipation is deferred to the
            // next outer corrector's UEqn. `Pimple` mode skips all of this.
            if self.mode == SolverMode::HybridAllMach {
                let diss = self.assemble_hybrid_dissipation();
                for f in 0..mesh.n_internal_faces {
                    self.phi.internal[f] += diss.d_phi[f];
                }
                hybrid_mom_src = diss.mom_src;
            }

            // ── Energy equation ─────────────────────────────────────────────
            //   ∂(ρh)/∂t + ∇·(φh) + (−∇·(αh∇h)) = dp/dt   [+ laplacian sign]
            let conv_he = fvc::div(&self.phi, &self.he); // explicit ∇·(φh)/V
            let alpha_h_f = fvc::interpolate(&self.alpha_h);
            let dp_dt = (self.p.clone() - p_old.clone()) * (1.0 / dt);

            // Conservative energy time derivative: ∂(ρh)/∂t discretised as
            // (ρ_cont·h − ρ_old·h_old)/dt (bead op-21g.14). Two coupled fixes:
            //
            //  1. The OLD-time term uses the OLD-time density `rho_old`
            //     (previous time level), not the current density — restoring
            //     the missing `h_old·(ρ − ρ_old)/dt` term. `ddt_coeff` reused
            //     the *current* ρ for both terms, so it was really solving
            //     ρ·∂h/∂t + ∇·(φh) = dp/dt, whose un-cancelled `h·∇·φ` outflow
            //     over-drains enthalpy during the violent flash (∇·φ ≫ 0 at the
            //     break), driving the bulk liquid subcooled and collapsing the
            //     pressure straight past `p_sat`.
            //
            //  2. The NEW-time coefficient is the **continuity density**
            //     `ρ_cont = ρ_old − dt·∇·φ` recomputed here from the final
            //     mass flux `self.phi`, NOT the EOS density that
            //     `correct_thermo` wrote into `self.rho` each inner corrector.
            //     Only with `ρ_cont` does discrete continuity
            //     `(ρ_cont − ρ_old)/dt = −∇·φ` hold *exactly*, so the
            //     `h_old·(ρ_cont − ρ_old)/dt = −h_old·∇·φ` term cancels the
            //     `h·∇·φ` part of `∇·(φh)` term-for-term. Using the EOS density
            //     (which, mid-flash, drops faster than the ψ·dp/dt the pressure
            //     equation feeds back into φ) leaves a residual that instead
            //     spuriously *over-heats* cells (a (p,h) flash into Region 5).
            //     With `ρ_cont` the energy equation reduces to the material
            //     derivative ρ Dh/Dt = dp/dt ⇒ the reversible `dh ≈ dp/ρ`
            //     (small enthalpy change) that keeps the state on the
            //     saturation dome as p falls — the flashing plateau.
            //     See `fvm::ddt_coeff_old`.
            let rho_cont = {
                let div_phi_final = fvc::div_flux(&self.phi);
                let mut rc = rho_old.clone() + (-dt) * div_phi_final;
                for c in 0..n {
                    if rc.internal[c] < 1e-4 {
                        rc.internal[c] = 1e-4;
                    }
                }
                rc
            };
            let mut e_eqn = fvm::ddt_coeff_old(&rho_cont, &rho_old, &self.he, &he_old, dt)
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
                    for (link, temps) in self
                        .lateral_adjacent_array_conductance_vector
                        .iter()
                        .zip(self.lateral_adjacent_array_temperature_vector.iter())
                    {
                        let h = link[c].get::<uom::si::thermal_conductance::watt_per_kelvin>();
                        let t_n = temps[c].get::<uom::si::thermodynamic_temperature::kelvin>();
                        e_eqn.source[c] += h * (t_n - t_c);
                    }
                    e_eqn.source[c] += self.cell_heat_source_power(c).get::<uom::si::power::watt>();
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

    /// Number of PIMPLE outer correctors per [`Self::step`] call.
    pub fn get_n_outer_correctors(&self) -> usize {
        self.n_outer_correctors
    }

    /// Sets the number of PIMPLE outer correctors (clamped to ≥ 1).
    pub fn set_n_outer_correctors(&mut self, n: usize) {
        self.n_outer_correctors = n.max(1);
    }

    /// Number of PISO pressure correctors per outer loop.
    pub fn get_n_inner_correctors(&self) -> usize {
        self.n_inner_correctors
    }

    /// Sets the number of PISO inner pressure correctors (clamped to ≥ 1).
    pub fn set_n_inner_correctors(&mut self, n: usize) {
        self.n_inner_correctors = n.max(1);
    }

    /// Pressure under-relaxation factor α_p -- see
    /// [`Self::p_under_relaxation`].
    pub fn get_pressure_under_relaxation(&self) -> Ratio {
        self.p_under_relaxation
    }

    /// Sets the pressure under-relaxation factor, clamped to (0, 1].
    pub fn set_pressure_under_relaxation(&mut self, alpha: Ratio) {
        self.p_under_relaxation = Ratio::new::<ratio>(alpha.get::<ratio>().clamp(1.0e-3, 1.0));
    }

    /// Velocity under-relaxation factor α_u -- see
    /// [`Self::u_under_relaxation`].
    pub fn get_velocity_under_relaxation(&self) -> Ratio {
        self.u_under_relaxation
    }

    /// Sets the velocity under-relaxation factor, clamped to (0, 1].
    pub fn set_velocity_under_relaxation(&mut self, alpha: Ratio) {
        self.u_under_relaxation = Ratio::new::<ratio>(alpha.get::<ratio>().clamp(1.0e-3, 1.0));
    }

    /// Configures this array for a transient PISO solve: one outer
    /// corrector, `n_correctors` inner pressure correctors, and no
    /// under-relaxation (α_p = α_u = 1.0). Appropriate when `delta_t` is a
    /// genuinely small (CFL-limited) physical timestep and the flow is
    /// actually evolving in time step-to-step -- this is [`Self::new`]'s
    /// default configuration (`n_correctors = 2`).
    pub fn set_piso_algorithm(&mut self, n_correctors: usize) {
        self.n_outer_correctors = 1;
        self.n_inner_correctors = n_correctors.max(1);
        self.p_under_relaxation = Ratio::new::<ratio>(1.0);
        self.u_under_relaxation = Ratio::new::<ratio>(1.0);
    }

    /// Configures this array for a SIMPLE steady-state solve:
    /// `n_outer_iterations` outer loops, a single pressure correction per
    /// outer loop, and classic textbook SIMPLE under-relaxation
    /// (α_p = 0.3, α_u = 0.7). Appropriate for driving this array toward a
    /// steady operating point under prescribed inlet/outlet boundary
    /// conditions (e.g. [`Self::set_inlet_velocity`] +
    /// [`Self::set_outlet_pressure`] on a "quasi-steady" component) --
    /// here `delta_t` is a pseudo-timestep controlling iteration size, not
    /// a physically meaningful timescale, so a single [`Self::step`] call
    /// with a large `n_outer_iterations` iterates to (approximate)
    /// convergence rather than advancing real time.
    pub fn set_simple_algorithm(&mut self, n_outer_iterations: usize) {
        self.n_outer_correctors = n_outer_iterations.max(1);
        self.n_inner_correctors = 1;
        self.p_under_relaxation = Ratio::new::<ratio>(0.3);
        self.u_under_relaxation = Ratio::new::<ratio>(0.7);
    }

    /// Configures this array for a PIMPLE solve -- multiple outer
    /// correctors, each with `n_inner_correctors` inner pressure
    /// correctors, at caller-chosen under-relaxation factors. The general
    /// "anything in between PISO and SIMPLE" case: e.g. more outer
    /// correctors than pure PISO (`n_outer_correctors > 1`) lets `delta_t`
    /// be larger than the PISO/CFL limit while still resolving some
    /// transient behaviour, unlike pure SIMPLE (`n_inner_correctors = 1`).
    pub fn set_pimple_algorithm(
        &mut self,
        n_outer_correctors: usize,
        n_inner_correctors: usize,
        pressure_under_relaxation: Ratio,
        velocity_under_relaxation: Ratio,
    ) {
        self.n_outer_correctors = n_outer_correctors.max(1);
        self.n_inner_correctors = n_inner_correctors.max(1);
        self.set_pressure_under_relaxation(pressure_under_relaxation);
        self.set_velocity_under_relaxation(velocity_under_relaxation);
    }

    /// Current pressure bounds `(p_min, p_max)` applied after every pressure
    /// solve in [`Self::step`] (see [`Self::p_min`]).
    pub fn get_pressure_bounds(&self) -> (Pressure, Pressure) {
        (self.p_min, self.p_max)
    }

    /// Sets the pressure bounds `[p_min, p_max]` clamped after every pressure
    /// solve (OpenFOAM `pressureControl::limit` `pMin`/`pMax` semantics —
    /// see [`Self::step`]). Raise `p_min` above the default triple-point
    /// pressure to impose e.g. a cavitation floor and keep a violent
    /// transient inside the EOS range; lower `p_max` similarly. Panics if
    /// `p_min >= p_max`.
    pub fn set_pressure_bounds(&mut self, p_min: Pressure, p_max: Pressure) {
        assert!(
            p_min.get::<uom::si::pressure::pascal>() < p_max.get::<uom::si::pressure::pascal>(),
            "pressure bounds require p_min < p_max, got p_min = {} Pa, p_max = {} Pa",
            p_min.get::<uom::si::pressure::pascal>(),
            p_max.get::<uom::si::pressure::pascal>()
        );
        self.p_min = p_min;
        self.p_max = p_max;
    }

    /// The current flux-discretisation mode (see [`SolverMode`]).
    pub fn get_solver_mode(&self) -> SolverMode {
        self.mode
    }

    /// Selects the flux-discretisation mode. [`SolverMode::Pimple`] (the
    /// default) runs the historical pressure-based path bit-identically;
    /// [`SolverMode::HybridAllMach`] additionally applies the Mach-blended KNP
    /// central-upwind dissipation as a deferred correction on near-sonic faces.
    pub fn set_solver_mode(&mut self, mode: SolverMode) {
        self.mode = mode;
    }

    /// The current hybrid Mach-blend window `(lo, hi)` (dimensionless Mach
    /// thresholds). See [`Self::set_mach_blend_window`].
    pub fn get_mach_blend_window(&self) -> (Ratio, Ratio) {
        (self.ma_blend_lo, self.ma_blend_hi)
    }

    /// Sets the hybrid Mach-blend window `β(Ma) = clamp((Ma−lo)/(hi−lo), 0, 1)`.
    ///
    /// `lo` and `hi` are dimensionless Mach thresholds: below `lo` **no** KNP
    /// dissipation is added (subsonic ⇒ the PIMPLE result is preserved), at/above
    /// `hi` it is applied at full weight, with a linear ramp between. Only
    /// affects [`SolverMode::HybridAllMach`]. Panics if `hi <= lo`.
    pub fn set_mach_blend_window(&mut self, lo: Ratio, hi: Ratio) {
        assert!(
            hi.get::<ratio>() > lo.get::<ratio>(),
            "mach blend window requires hi > lo, got lo = {}, hi = {}",
            lo.get::<ratio>(),
            hi.get::<ratio>()
        );
        self.ma_blend_lo = lo;
        self.ma_blend_hi = hi;
    }

    /// Assemble the Mach-weighted KNP central-upwind dissipation from the
    /// **current** primitive state (`ρ, U, he, p` after the inner PISO loop's
    /// `correct_thermo`).
    ///
    /// Per internal face: the per-cell HEM equilibrium sound speed
    /// ([`hem_sound_speed_ph`]) gives the cell Mach numbers; the face blend
    /// weight is `β(Ma_face)` with `Ma_face = min(Ma_owner, Ma_neighbour)` — the
    /// low-Mach scaling that keeps a liquid/two-phase interface stable (the
    /// liquid side is subsonic so `β = 0` there; dissipation activates only where
    /// *both* sides are near-sonic — see the `min` rationale at the call site).
    /// van-Leer MUSCL owner/neighbour reconstructions of `ρ, U, he, p, c` build
    /// the left/right [`FaceState`]s, and the deferred-correction dissipation is
    /// `β·(knp − central)·|Sf|` — the pure KNP jump term, identically zero on a
    /// subsonic face. Continuity dissipation is returned as a face mass flux to
    /// fold into `phi`; momentum dissipation as an owner-loses / neighbour-gains
    /// per-cell source. There is no separate energy source — the enthalpy
    /// shock-capturing rides on the continuity flux through the EEqn's `∇·(φh)`
    /// (see [`HybridDissipation`]).
    fn assemble_hybrid_dissipation(&self) -> HybridDissipation {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;
        let nif = mesh.n_internal_faces;
        let lo = self.ma_blend_lo.get::<ratio>();
        let hi = self.ma_blend_hi.get::<ratio>();

        // Per-cell HEM equilibrium sound speed, Mach number, and a validity-edge
        // safety flag.
        //
        // The KNP shock-capturing is only meaningful where the HEM `(p,h)` closure
        // is well-defined. `safe[c]` marks cells whose temperature sits
        // comfortably inside the IAPWS-IF97 `(p,h)` validity window (bounded by
        // the 273.15 K and 1073.15 K isotherms the flash panics at, plus the
        // triple-point pressure floor). Faces touching an unsafe cell get no
        // dissipation, so the hybrid can never nudge a marginal empty-pipe-tail
        // cell across those edges. The margins sit far from the ringing phase
        // (whose flashing-front cells are ~490–500 K at 2–7 MPa), so the damping
        // demonstrated on 0–0.15 s is unaffected; this only hardens the long
        // tail against last-bit trajectory sensitivity near the edges.
        const T_SAFE_LO_K: f64 = 300.0; // margin above the 273.15 K panic
        const T_SAFE_HI_K: f64 = 1050.0; // margin below the 1073.15 K panic
        let p_floor = self.p_min.get::<uom::si::pressure::pascal>();
        let mut c_cell = vec![0.0_f64; n];
        let mut ma_cell = vec![0.0_f64; n];
        let mut safe = vec![false; n];
        for i in 0..n {
            let c = hem_sound_speed_ph(self.p.internal[i], self.he.internal[i], C_MIN_MPS);
            c_cell[i] = c;
            ma_cell[i] = self.u.internal[i].mag() / c;
            let t_i = self.t.internal[i];
            safe[i] = t_i.is_finite()
                && (T_SAFE_LO_K..=T_SAFE_HI_K).contains(&t_i)
                && self.p.internal[i] > p_floor;
        }

        // Reconstruct the sound speed as a field so the face wave speeds use the
        // MUSCL owner/neighbour states, consistent with the primitives.
        let c_field = VolScalarField::new(
            "cHEM",
            mesh.clone(),
            Field::new(c_cell),
            mesh.patches
                .iter()
                .map(|p| PatchField::zero_gradient(p.size))
                .collect(),
        );

        let lim = fvc::Limiter::VanLeer;
        let (rho_pos, rho_neg) = fvc::reconstruct_pos_neg(&self.rho, lim);
        let (he_pos, he_neg) = fvc::reconstruct_pos_neg(&self.he, lim);
        let (p_pos, p_neg) = fvc::reconstruct_pos_neg(&self.p, lim);
        let (c_pos, c_neg) = fvc::reconstruct_pos_neg(&c_field, lim);
        let ux = velocity_component(&self.u, 0);
        let uy = velocity_component(&self.u, 1);
        let uz = velocity_component(&self.u, 2);
        let (ux_pos, ux_neg) = fvc::reconstruct_pos_neg(&ux, lim);
        let (uy_pos, uy_neg) = fvc::reconstruct_pos_neg(&uy, lim);
        let (uz_pos, uz_neg) = fvc::reconstruct_pos_neg(&uz, lim);

        let mut d_phi = vec![0.0_f64; nif];
        let mut mom_src = vec![Vector3::ZERO; n];

        for f in 0..nif {
            let o = mesh.owner[f];
            let nb = mesh.neighbour[f];
            let area = mesh.face_areas[f];
            if area < 1e-300 {
                continue;
            }

            // Validity-edge guard: no shock-capturing where the HEM closure is
            // near its (p,h) validity boundary on either side (see `safe`).
            if !safe[o] || !safe[nb] {
                continue;
            }

            // Blend weight gated on the *lower* Mach of the two adjacent cells
            // (subsonic ⇒ β = 0 ⇒ skip: exactly pure PIMPLE on this face).
            //
            // Using `min` (not `max`) is the low-Mach scaling that keeps the
            // scheme stable at a liquid/two-phase interface. There the liquid
            // side's sound speed (~1400 m/s) dominates the KNP wave speeds
            // `a = u ± c`, so the numerical viscosity `a_L·a_R/da ~ c_liq/2` is
            // huge; but that liquid acoustic wave is genuinely *low Mach*
            // (|u|/c_liq ≪ 1), so it must NOT be dissipated. `min(Ma)` sees the
            // subsonic liquid side and returns β = 0 there, activating the KNP
            // dissipation only where *both* sides are near-sonic — the
            // fully-developed two-phase flashing front, where `c` is uniformly
            // small and the dissipation magnitude is physical. Gating on
            // `max(Ma)` instead let the low-Mach liquid acoustic viscosity
            // through and over-drained the near-break enthalpy below the
            // 273.15 K isotherm (bead op-21g.15.6 debugging trail).
            let ma_f = ma_cell[o].min(ma_cell[nb]);
            let mut beta = mach_blend(ma_f, lo, hi);

            // Rarefied-tail (low-density) taper — the full-transient stability fix
            // (bug op-21g.15.7). The all-Mach KNP shock-capturing is designed for
            // the *dense* two-phase flashing front (mixture density
            // ρ ≳ 100 kg/m³, the near-sonic region where the ringing lives). As
            // the pipe empties, cells rarefy toward vacuum; there the HEM
            // equilibrium closure degrades, there is no flashing shock to capture,
            // and an explicit deferred-correction dissipation evaluated on a
            // nearly-empty cell over-drives it: the continuity density `ρ_cont`
            // collapses to its floor, the segregated EEqn diagonal `ρ_cont·V/dt`
            // vanishes, and a single solve tips the cell across the IAPWS-IF97
            // 273.15 K `(p,h)` validity edge (the panic this fix removes). Both the
            // continuity and momentum dissipation independently trigger this in the
            // emptying tail.
            //
            // The taper `g(ρ_face) = clamp((ρ_face − ρ_lo)/(ρ_hi − ρ_lo), 0, 1)`,
            // with `ρ_face = min(ρ_owner, ρ_neighbour)` (the lighter, at-risk side),
            // scales the blend to **zero below `ρ_lo`** (rarefied ⇒ pure PIMPLE,
            // which is stable over the full transient) and **full above `ρ_hi`**
            // (dense front ⇒ untouched). It is measured to be inert over the
            // physics-of-interest window: the minimum dissipated-face density in
            // 0–0.15 s is ≈ 106.5 kg/m³ (every ringing/plateau face sits at
            // `ρ ≥ ρ_hi`), so the ~55 % ringing reduction and the ≈ 388 psia
            // flashing plateau are unchanged, while the late-time emptying tail can
            // no longer be driven out of the `(p,h)` range. See the V&V log
            // `collaboration/edwards_tampines_regen/hybrid_stability_debug_log.md`.
            {
                let rho_face_min = self.rho.internal[o].min(self.rho.internal[nb]);
                let g = ((rho_face_min - HYBRID_RHO_TAPER_LO)
                    / (HYBRID_RHO_TAPER_HI - HYBRID_RHO_TAPER_LO))
                    .clamp(0.0, 1.0);
                beta *= g;
            }

            if beta <= 0.0 {
                continue;
            }

            let sf = mesh.face_area_vectors[f];
            let n_f = Vector3::new(sf.x / area, sf.y / area, sf.z / area);

            let l = FaceState {
                rho: rho_pos.internal[f].max(1e-10),
                u: Vector3::new(ux_pos.internal[f], uy_pos.internal[f], uz_pos.internal[f]),
                he: he_pos.internal[f],
                p: p_pos.internal[f],
                c: c_pos.internal[f].max(C_MIN_MPS),
            };
            let r = FaceState {
                rho: rho_neg.internal[f].max(1e-10),
                u: Vector3::new(ux_neg.internal[f], uy_neg.internal[f], uz_neg.internal[f]),
                he: he_neg.internal[f],
                p: p_neg.internal[f],
                c: c_neg.internal[f].max(C_MIN_MPS),
            };

            let knp = knp_face_flux(&l, &r, n_f);
            let cen = central_face_flux(&l, &r, n_f);

            // Continuity / momentum: deferred-correction dissipation
            // β·(KNP − central)·|Sf| (the pure KNP jump term). Energy is carried
            // implicitly by the continuity term (see `HybridDissipation`).
            let d_cont = beta * (knp.cont - cen.cont) * area;
            let d_mom = (knp.mom - cen.mom) * (beta * area);

            // Continuity: add the dissipative mass flux to phi (owner→neighbour
            // positive, matching phi's sign convention).
            d_phi[f] += d_cont;
            // Momentum: owner loses the outgoing flux, neighbour gains it (same
            // convention as the rhoCentralFoam conserved-variable tendencies).
            mom_src[o] = mom_src[o] - d_mom;
            mom_src[nb] = mom_src[nb] + d_mom;
        }

        HybridDissipation { d_phi, mom_src }
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
            && array
                .u
                .internal
                .as_slice()
                .iter()
                .all(|v| v.mag().is_finite());
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
        assert!(matches!(
            err,
            Err(MeshError::NonPositiveCellCount { got: 0 })
        ));
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
        let h = uom::si::f64::AvailableEnergy::new::<uom::si::available_energy::joule_per_kilogram>(
            5.0e5,
        );
        let expected_t = t_ph_eqm(p, h).get::<uom::si::thermodynamic_temperature::kelvin>();
        let expected_lambda = lambda_ph_eqm(p, h).value;
        let expected_alpha_h = expected_lambda
            / crate::interfaces::functional_programming::ph_flash_eqm::cp_ph_eqm(p, h).value;

        assert!((array.t.internal[0] - expected_t).abs() < 1e-6);
        assert!((array.alpha_h.internal[0] - expected_alpha_h).abs() < 1e-9);
        assert!(array.rho.internal[0] > 0.0);
        assert!(array.mu.internal[0] > 0.0);
    }

    /// `correct_thermo` survives a **two-phase (boiling)** cell without
    /// panicking, and produces finite, physical properties there. This is
    /// the scenario a real steam-generator tube spends most of its length
    /// in, and it used to `todo!()` out: `lambda_ph_eqm`'s critical-
    /// enhancement term delegated cp/cv/kappa_t to single-phase `(T, p)`
    /// routines that have no region-4 answer (fixed 2026-07-14 by quality-
    /// weighting the saturated region-1/region-2 values — see
    /// `thermal_conductivity::lambda_2_crit_enhancement_term_tp_two_phase_estimate`).
    #[test]
    fn correct_thermo_survives_two_phase_boiling_cell() {
        use crate::interfaces::functional_programming::ph_flash_eqm::{
            ph_flash_region, x_ph_flash,
        };
        use crate::interfaces::functional_programming::pt_flash_eqm::FwdEqnRegion;

        let mut array = TampinesSteamArray::new(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(0.01),
            3,
            Time::new::<second>(1e-4),
        )
        .unwrap();

        // 1 bar, h ≈ 1.5 MJ/kg is squarely inside the two-phase dome
        // (h_f ≈ 0.42 MJ/kg, h_g ≈ 2.68 MJ/kg ⇒ x ≈ 0.48).
        let p = Pressure::new::<uom::si::pressure::pascal>(1.0e5);
        let h = uom::si::f64::AvailableEnergy::new::<uom::si::available_energy::joule_per_kilogram>(
            1.5e6,
        );
        assert_eq!(
            ph_flash_region(p, h),
            FwdEqnRegion::Region4,
            "sample must be two-phase"
        );
        let x = x_ph_flash(p, h);
        assert!(
            x > 0.0 && x < 1.0,
            "quality {x} should be strictly two-phase"
        );

        for c in 0..3 {
            array.p.internal[c] = p.get::<uom::si::pressure::pascal>();
            array.he.internal[c] = h.get::<uom::si::available_energy::joule_per_kilogram>();
        }
        // Must not panic in the two-phase region.
        array.correct_thermo();

        for c in 0..3 {
            assert!(array.rho.internal[c].is_finite() && array.rho.internal[c] > 0.0);
            assert!(array.t.internal[c].is_finite());
            assert!(array.mu.internal[c].is_finite() && array.mu.internal[c] > 0.0);
            assert!(array.alpha_h.internal[c].is_finite() && array.alpha_h.internal[c] > 0.0);
            assert!(array.psi.internal[c].is_finite() && array.psi.internal[c] > 0.0);
            // T should be the saturation temperature at 1 bar (~372.76 K).
            assert!(
                (array.t.internal[c] - 372.76).abs() < 1.0,
                "two-phase T should be T_sat(1 bar) ≈ 372.76 K, got {}",
                array.t.internal[c]
            );
        }
    }
}
