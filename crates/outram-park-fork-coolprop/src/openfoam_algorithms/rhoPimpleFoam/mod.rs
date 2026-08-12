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
    Angle, Area, AvailableEnergy, Length, MassDensity, MassRate, Power, Pressure, Ratio, Time,
    ThermalConductance, ThermodynamicTemperature,
};
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::si::length::meter;
use uom::si::angle::radian;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use crate::openfoam_algorithms::openfoam_source::interface::one_dimensional_meshing::create_one_d_mesh;
use crate::openfoam_algorithms::openfoam_source::*;
use crate::fluid::Fluid;
use crate::flash;

mod lateral_coupling;
pub use lateral_coupling::OPCPFluidArrayError;

mod central_upwind;
use central_upwind::{
    central_face_flux, hem_sound_speed_ph, knp_face_flux, mach_blend, velocity_component,
    FaceState, C_MIN_MPS,
};

/// Selects the flux discretisation used by [`OPCPFluidArray::step`].
///
/// This is an **opt-in** switch (enum dispatch — no trait objects, per the
/// workspace design rules). The default [`SolverMode::Pimple`] runs the
/// pressure-based compressible PIMPLE algorithm exactly as before, bit-for-bit
/// (every existing test is preserved by construction).
/// [`SolverMode::HybridAllMach`] additionally injects a **Mach-weighted KNP
/// central-upwind dissipation** (see the `central_upwind` module) as a
/// deferred-correction flux, active only on near-sonic faces (`β(Ma) > 0`), to
/// damp ringing at a near-sonic front while leaving subsonic regions untouched.
///
/// This mirrors the `tampines-steam-tables` `TampinesSteamArray` all-Mach hybrid
/// (bead op-ek2). The CoolProp difference is entirely in the sound-speed closure
/// (`central_upwind::hem_sound_speed_ph`): single-phase faces use the Helmholtz
/// EOS `speed_of_sound`, two-phase faces use a homogeneous-equilibrium (HEM)
/// finite-difference of the equilibrium isentrope through the Maxwell VLE — never
/// a frozen speed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SolverMode {
    /// Pressure-based compressible **PIMPLE** — the historical, validated,
    /// default path.
    #[default]
    Pimple,
    /// PIMPLE + Mach-blended KNP shock-capturing dissipation (all-Mach hybrid).
    /// Opt-in; the default [`SolverMode::Pimple`] is the bit-identical historical
    /// path.
    HybridAllMach,
}

/// Per-`step` hybrid KNP dissipation for the continuity and momentum equations.
/// Both fields are the deferred-correction contribution `β·(knp − central)·|Sf|`
/// summed appropriately; every entry is identically zero on a subsonic
/// (`β = 0`) face, so the default `Pimple` path never sees it.
///
/// There is no separate energy term: the continuity dissipation is folded into
/// `phi` **before** the EEqn's `rho_cont`/`conv_he` recompute, so the enthalpy
/// shock-capturing is carried *implicitly* by the EEqn's `∇·(φh)` convection —
/// the conservative-ddt cancellation `(rho_cont − rho_old)/dt = −∇·φ` transports
/// the dissipative enthalpy for free. Adding a separate explicit energy source on
/// top double-counts that transport (mirrors the `TampinesSteamArray`
/// `HybridDissipation` rationale, bead op-ek2).
struct HybridDissipation {
    /// Dissipative mass flux to add to `phi` \[kg/s\], one per internal face.
    d_phi: Vec<f64>,
    /// Per-cell momentum source \[N\] (owner-loses / neighbour-gains sum).
    mom_src: Vec<Vector3>,
}

/// **Default** lower mixture-density threshold \[kg/m³\] of the rarefied-tail
/// taper on the all-Mach hybrid KNP dissipation — the [`OPCPFluidArray::new`]
/// value of the *configurable* field [`OPCPFluidArray::hybrid_rho_taper_lo`].
/// **Below** the lower threshold the KNP dissipation is scaled to **zero**
/// (rarefied emptying tail ⇒ pure PIMPLE, which is stable over the full
/// transient). See [`OPCPFluidArray::assemble_hybrid_dissipation`] and
/// [`OPCPFluidArray::set_rho_taper_window`].
///
/// **Regime / provenance note (CoolProp):** these default thresholds
/// (50/100 kg/m³) are inherited verbatim from the `TampinesSteamArray` water
/// transient, where the near-sonic ringing lives on a *dense* two-phase
/// flashing front (ρ ≳ 100 kg/m³). A bare density window is only meaningful
/// **relative to the pressure it was calibrated at** — here a steam blowdown
/// depressurising to ~1 bar (Edwards–O'Brien), where ρ ≈ 50–100 kg/m³ marks
/// the dense flashing mixture. At other pressures, or for a **low-density
/// single-phase gas** (e.g. Nitrogen at ~1 bar, ρ ≈ 1 kg/m³; Helium at HTR-10
/// conditions, ρ ≈ 1–3 kg/m³), the same numbers mean something entirely
/// different: the whole flow sits below the lower threshold, so the taper
/// zeroes the hybrid and `HybridAllMach` degenerates to `Pimple` — the honest,
/// intended outcome for a deeply subsonic gas circuit. Retune the window with
/// [`OPCPFluidArray::set_rho_taper_window`] (fluid- and pressure-aware) before
/// relying on `HybridAllMach` for such a case, and pair it with the `(p, h)`
/// admissibility guard ([`OPCPFluidArray::set_enthalpy_bounds`] +
/// [`OPCPFluidArray::set_pressure_bounds`]) so the energy equation cannot
/// overdrain enthalpy to unphysical temperatures.
const HYBRID_RHO_TAPER_LO_DEFAULT: f64 = 50.0;

/// **Default** upper mixture-density threshold \[kg/m³\] of the rarefied-tail
/// taper — the [`OPCPFluidArray::new`] value of
/// [`OPCPFluidArray::hybrid_rho_taper_hi`]. **At or above** the upper threshold
/// the KNP dissipation is applied at full weight (the dense two-phase region
/// where near-sonic ringing lives). Between the two thresholds the blend ramps
/// linearly. See the regime/provenance note on [`HYBRID_RHO_TAPER_LO_DEFAULT`].
const HYBRID_RHO_TAPER_HI_DEFAULT: f64 = 100.0;

/// **Default** lower specific-enthalpy bound \[J/kg\] of the `(p, h)`
/// admissibility guard — the [`OPCPFluidArray::new`] value of
/// [`OPCPFluidArray::h_min`]. Deliberately **wide open** (−1×10⁷ J/kg): the
/// default guard changes no physically plausible solution, it only stops a
/// diverging energy solve from driving the `(p, h)` flash to nonsense
/// temperatures. Tighten per fluid with
/// [`OPCPFluidArray::set_enthalpy_bounds`].
const H_MIN_DEFAULT: f64 = -1.0e7;

/// **Default** upper specific-enthalpy bound \[J/kg\] of the `(p, h)`
/// admissibility guard — the [`OPCPFluidArray::new`] value of
/// [`OPCPFluidArray::h_max`]. Wide open (1×10⁸ J/kg); see [`H_MIN_DEFAULT`].
const H_MAX_DEFAULT: f64 = 1.0e8;

/// Mesh-patch index of the **inlet** (`"left"`, x = 0, outward normal −x) on
/// the 1-D mesh built by [`create_one_d_mesh`]. See that function's `## Layout`
/// section for the face/patch ordering.
pub(super) const INLET_PATCH: usize = 1;

/// Mesh-patch index of the **outlet** (`"right"`, x = length, outward normal
/// +x) on the 1-D mesh built by [`create_one_d_mesh`].
pub(super) const OUTLET_PATCH: usize = 0;

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
///   ρ = ρ(p, h),  ψ = ∂ρ/∂p|_h        (EOS — see `correct_thermo`)
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
///   updating `ρ`, `T` and the compressibility `ψ = ∂ρ/∂p|_h` from the CoolProp
///   Helmholtz EOS (replacing the old placeholder `ρ = ψ·p`).
///   [`Self::correct_transport`] then updates `μ`/`αh` from
///   [`crate::transport::viscosity`]/[`crate::transport::conductivity`] at the
///   refreshed `(T, ρ)` — for a fluid/state with no transport model (`None`)
///   a cell's `μ`/`αh` are left at their previous value, never a wrong number.
///
/// C++ reference: `applications/solvers/compressible/rhoPimpleFoam/`.
#[derive(Clone, Debug)]
pub struct OPCPFluidArray {
    /// The working fluid whose CoolProp Helmholtz EOS closes the thermo update.
    pub fluid: Fluid,
    /// 1-D finite-volume mesh (built by [`create_one_d_mesh`]).
    pub mesh: Arc<FvMesh>,

    // ── Time control ────────────────────────────────────────────────────────
    /// Fixed time step Δt [s].
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
    /// [`Self::step`]). Defaults to a wide, near-vacuum floor (1 Pa) that
    /// just prevents a violent transient from driving a cell to negative
    /// absolute pressure; raise it with [`Self::set_pressure_bounds`] for a
    /// tighter (e.g. fluid-EOS or cavitation) floor. Mirrors OpenFOAM's
    /// `pressureControl::limit` `pMin`/`pMax` bounding — see [`Self::step`].
    pub p_min: Pressure,
    /// Upper pressure bound \[Pa\] applied after every pressure solve.
    /// Defaults to a wide 1 GPa ceiling. See [`Self::p_min`].
    pub p_max: Pressure,
    /// Lower specific-enthalpy bound \[J/kg\] of the **`(p, h)` admissibility
    /// guard**, applied to every cell right after each energy-equation solve —
    /// i.e. before [`Self::correct_thermo`] next consumes `he` — so the energy
    /// equation cannot overdrain enthalpy past physically meaningful bounds and
    /// hand the `(p, h)` flash a nonsense temperature. Together with
    /// [`Self::p_min`]/[`Self::p_max`] (the pressure half of the window) this
    /// keeps every flashed state inside a caller-chosen `(p, h)` envelope.
    /// Defaults to a deliberately wide-open −1×10⁷ J/kg (no plausible solution
    /// is touched); tighten per fluid with [`Self::set_enthalpy_bounds`] —
    /// e.g. to the enthalpy range spanned by the fluid EOS's stated validity
    /// (`t_triple`..`t_max`) at the working pressure. Every clamp is **counted**
    /// in [`Self::h_clamp_events`], never silent.
    pub h_min: AvailableEnergy,
    /// Upper specific-enthalpy bound \[J/kg\] of the `(p, h)` admissibility
    /// guard. Defaults to a wide-open 1×10⁸ J/kg. See [`Self::h_min`].
    pub h_max: AvailableEnergy,
    /// **Cumulative** count of cell-enthalpy clamp events (dimensionless): each
    /// cell clamped to [`Self::h_min`]/[`Self::h_max`] after an energy-equation
    /// solve adds 1, across all steps since construction (or since the caller
    /// last reset it to 0 — it is plain `pub` state). `0` means the guard never
    /// engaged. **Asymmetry note:** the pressure clamp ([`Self::p_min`]/
    /// [`Self::p_max`], OpenFOAM `pressureControl::limit` semantics) is silent
    /// (uncounted, matching upstream); the enthalpy clamp is deliberately
    /// counted so an engaged guard is visible, not silent. A NaN enthalpy is
    /// neither clamped nor counted — genuine divergence is not masked.
    pub h_clamp_events: usize,

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
    /// Lower mixture-density threshold \[kg/m³\] of the rarefied-tail taper on
    /// the hybrid KNP dissipation: below it the dissipation is scaled to zero
    /// (pure PIMPLE), with a linear ramp up to [`Self::hybrid_rho_taper_hi`].
    /// Default 50 kg/m³ — a **steam-calibrated placeholder** inherited from the
    /// `TampinesSteamArray` Edwards–O'Brien blowdown (dense two-phase flashing
    /// front depressurising to ~1 bar); a bare density window is only
    /// meaningful relative to that calibration pressure, so retune it per
    /// fluid/pressure with [`Self::set_rho_taper_window`]. Only read when
    /// `mode == HybridAllMach`. See `HYBRID_RHO_TAPER_LO_DEFAULT` (this
    /// module) for the full regime/provenance note.
    pub hybrid_rho_taper_lo: MassDensity,
    /// Upper mixture-density threshold \[kg/m³\] of the rarefied-tail taper:
    /// at/above it the KNP dissipation is applied at full weight (default
    /// 100 kg/m³, same steam-calibrated provenance as
    /// [`Self::hybrid_rho_taper_lo`]). See [`Self::set_rho_taper_window`].
    pub hybrid_rho_taper_hi: MassDensity,

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
    /// Compressibility ψ = ∂ρ/∂p|_h [s²/m²], from the EOS in `correct_thermo`.
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
    ///
    /// **This is bookkeeping, not a boundary condition.** To actually *impose*
    /// a mass flow at the inlet, use [`Self::set_inlet_mass_flowrate`], which
    /// installs the self-maintaining flow-rate inlet described on
    /// [`Self::inlet_mass_flowrate`]. To read back the flow the solver actually
    /// produced, use [`Self::get_inlet_mass_flowrate_actual`] /
    /// [`Self::get_outlet_mass_flowrate_actual`].
    pub mass_flowrate: MassRate,
    /// **Prescribed** inlet mass flowrate \[kg/s\] — an actual boundary
    /// condition on the `"left"` patch, unlike the bookkeeping-only
    /// [`Self::mass_flowrate`]. `None` (the [`Self::new`] default) means no
    /// mass-flow inlet is imposed and whatever velocity BC the caller set with
    /// [`Self::set_inlet_velocity`] stands.
    ///
    /// When `Some(ṁ)`, [`Self::step`] re-derives the fixed inlet velocity
    /// `u_in = ṁ / (ρ_inlet · A_inlet)` from the **current** inlet-face density
    /// once per pressure corrector — OpenFOAM's `flowRateInletVelocity`
    /// (`src/finiteVolume/fields/fvPatchFields/derived/flowRateInletVelocity/`)
    /// semantics. Because it is re-derived from the live density rather than a
    /// caller's one-off assumed density, the imposed inlet mass flux stays
    /// equal to `ṁ` as the solution's density evolves, instead of drifting.
    /// Positive is **into** the domain (+x); a negative value prescribes
    /// outflow through the inlet patch, in which case
    /// [`Self::set_inlet_enthalpy`]'s fixed inlet enthalpy is no longer a
    /// physically meaningful BC (a fixed value on an outflow face) and the
    /// caller should not rely on it.
    ///
    /// Set it with [`Self::set_inlet_mass_flowrate`] and clear it with
    /// [`Self::clear_inlet_mass_flowrate`] (or by calling
    /// [`Self::set_inlet_velocity`], which prescribes a velocity instead and so
    /// clears this).
    pub inlet_mass_flowrate: Option<MassRate>,
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
            Ok(s) => (
                s.density,
                s.enthalpy,
                flash::drho_dp_t(fluid, t0, s.density),
            ),
            Err(_) => (1.0, 0.0, 1.0e-5),
        };

        let u = VolVectorField::zero("U", mesh.clone());
        let p = VolScalarField::uniform("p", mesh.clone(), p0);
        let rho = VolScalarField::uniform("rho", mesh.clone(), rho0);
        let t = VolScalarField::uniform("T", mesh.clone(), t0);
        let he = VolScalarField::uniform("he", mesh.clone(), he0);
        let mu = VolScalarField::uniform("mu", mesh.clone(), 1.8e-5);
        let alpha_h = VolScalarField::uniform("alphaEff", mesh.clone(), 2.5e-5);
        let psi = VolScalarField::uniform("psi", mesh.clone(), psi0);
        let phi = SurfaceScalarField::zeros("phi", mesh.clone());

        Ok(Self {
            fluid,
            mesh,
            delta_t,
            n_outer_correctors: 1,
            n_inner_correctors: 2,
            p_under_relaxation: Ratio::new::<ratio>(1.0),
            u_under_relaxation: Ratio::new::<ratio>(1.0),
            // Wide default pressure bounds (1 Pa .. 1 GPa): just enough to
            // keep a violent transient off negative absolute pressure. See
            // `step` for the OpenFOAM `pressureControl` reference.
            p_min: Pressure::new::<pascal>(1.0),
            p_max: Pressure::new::<pascal>(1.0e9),
            // Wide-open default (p, h) admissibility guard: no physically
            // plausible solution is clamped; see `h_min`/`h_max`.
            h_min: AvailableEnergy::new::<joule_per_kilogram>(H_MIN_DEFAULT),
            h_max: AvailableEnergy::new::<joule_per_kilogram>(H_MAX_DEFAULT),
            h_clamp_events: 0,
            // Default: pure PIMPLE ⇒ the hybrid dissipation is never assembled,
            // so every existing constructor/test runs the unchanged code path.
            mode: SolverMode::Pimple,
            ma_blend_lo: Ratio::new::<ratio>(0.3),
            ma_blend_hi: Ratio::new::<ratio>(1.0),
            // Steam-calibrated placeholder taper window (Edwards–O'Brien
            // provenance) — see `hybrid_rho_taper_lo` / `set_rho_taper_window`.
            hybrid_rho_taper_lo: MassDensity::new::<kilogram_per_cubic_meter>(
                HYBRID_RHO_TAPER_LO_DEFAULT,
            ),
            hybrid_rho_taper_hi: MassDensity::new::<kilogram_per_cubic_meter>(
                HYBRID_RHO_TAPER_HI_DEFAULT,
            ),
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
            inlet_mass_flowrate: None,
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
    /// density `ρ`, temperature `T` and the compressibility `ψ = ∂ρ/∂p|_h`,
    /// writing them back into the `rho`, `t` and `psi` fields (`ψ` closes the
    /// pressure equation — see `pEqn`). This replaces the old placeholder
    /// `ρ = ψ·p`.
    ///
    /// **`ψ` is `∂ρ/∂p` at *fixed enthalpy*, by a central finite difference of
    /// the `(p, h)` flash** — not the isothermal `∂ρ/∂p|_T`. This is the correct
    /// linearisation for a *segregated* pressure solve: within a pressure-
    /// correction inner iteration `he` is frozen (only the energy equation, after
    /// the inner loop, updates it), so the density's response to the pressure
    /// change is `∂ρ/∂p|_h`. In single phase the two nearly coincide (for an ideal
    /// gas `∂ρ/∂p|_h = ρ/p = ρκ_T` exactly; for a liquid `∂ρ/∂h|_p` is tiny), so
    /// this leaves subcooled/superheated behaviour unchanged; the fixed-enthalpy
    /// form additionally carries the two-phase flashing compliance where the flash
    /// crosses the dome. Mirrors the `TampinesSteamArray` `ψ` fix (bead op-ek2).
    /// If the two perturbed `(p, h)` flashes cannot both be evaluated, it falls
    /// back to the isothermal `∂ρ/∂p|_T` so `ψ` stays defined at the EOS edges.
    ///
    /// If a cell's `(p, h)` does not converge to a single-phase state (e.g. it
    /// falls in the two-phase dome, which the single-phase flash does not model),
    /// that cell's `ρ`/`T`/`ψ` are **left at their previous values** rather than
    /// set to a wrong number — so the solve stays finite. Transport (`μ`, `αh`) is
    /// not updated here (see [`Self::correct_transport`]).
    pub fn correct_thermo(&mut self) {
        let p_min_pa = self.p_min.get::<pascal>();
        let p_max_pa = self.p_max.get::<pascal>();
        for c in 0..self.mesh.n_cells {
            let p_c = self.p.internal[c];
            let h_c = self.he.internal[c];
            if let Ok(state) = flash::state_ph(self.fluid, p_c, h_c) {
                self.rho.internal[c] = state.density.max(1e-4);
                self.t.internal[c] = state.temperature;

                // Compressibility ψ = ∂ρ/∂p|_h by a central finite difference of
                // the (p, h) flash at fixed enthalpy (see the method doc).
                let dp = (p_c * 1.0e-3).max(50.0);
                let p_hi = (p_c + dp).min(p_max_pa);
                let p_lo = (p_c - dp).max(p_min_pa);
                let psi_fd = if p_hi > p_lo {
                    match (
                        flash::state_ph(self.fluid, p_hi, h_c),
                        flash::state_ph(self.fluid, p_lo, h_c),
                    ) {
                        (Ok(s_hi), Ok(s_lo)) => (s_hi.density - s_lo.density) / (p_hi - p_lo),
                        // A perturbed point fell out of the single-phase range:
                        // fall back to the isothermal ∂ρ/∂p|_T so ψ stays defined.
                        _ => flash::drho_dp_t(self.fluid, state.temperature, state.density),
                    }
                } else {
                    flash::drho_dp_t(self.fluid, state.temperature, state.density)
                };
                self.psi.internal[c] = psi_fd.max(1e-12);
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

    /// Advance the solution by one timestep of length `timestep`.
    ///
    /// This is the interface to prefer, and it matches TUAS's
    /// `FluidArray::advance_timestep`: the caller owns the clock and states
    /// the step each time, so a driver stepping several different components
    /// keeps them on one timeline.
    ///
    /// Contrast [`Self::step`], which advances by whatever `delta_t` the array
    /// was built with. That is fine for a fixed-step study, but if the caller's
    /// clock ever differs the two silently diverge — the array advances by its
    /// own stored value while the caller believes it advanced by theirs.
    ///
    /// Sets [`Self::delta_t`] to `timestep` before solving, so the stored value
    /// always reflects the step actually taken.
    ///
    /// # Errors
    ///
    /// Returns [`OPCPFluidArrayError::InvalidTimestep`] if `timestep` is not
    /// positive and finite. It is not clamped or substituted: a bad timestep
    /// means the caller's clock is wrong, and quietly advancing by something
    /// else yields a plausible-looking result for a step that never ran.
    ///
    /// # Stability
    ///
    /// Choosing a stable step is the caller's responsibility. This is an
    /// explicit-in-time PIMPLE solve, so too large a step relative to the cell
    /// size and flow speed will diverge; nothing here checks a CFL condition.
    pub fn advance_timestep(&mut self, timestep: Time) -> Result<(), OPCPFluidArrayError> {
        let seconds = timestep.get::<second>();
        if !seconds.is_finite() || seconds <= 0.0 {
            return Err(OPCPFluidArrayError::InvalidTimestep { seconds });
        }
        self.delta_t = timestep;
        self.step();
        Ok(())
    }

    /// Advance the solution by the array's stored [`Self::delta_t`].
    ///
    /// Prefer [`Self::advance_timestep`] unless the step is genuinely
    /// fixed for the life of the array: this form cannot tell the caller
    /// what step it took, so a driver with its own clock can drift from it
    /// without either side noticing.
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

        let mut u_bcs = capture_bcs(&self.u.boundary);
        let p_bcs = capture_bcs(&self.p.boundary);
        // `he`'s BC template must be captured and re-stamped exactly like `u`'s
        // and `p`'s. `FvMatrix::solve` builds its output field with *zero-
        // gradient* boundaries, so without this the field assigned back to
        // `self.he` after the energy solve silently loses every prescribed
        // enthalpy boundary — `set_inlet_enthalpy` would take effect on the
        // first outer corrector only and never again, making it a one-shot the
        // caller has to re-issue every step (measured 2026-08-12: without a
        // per-step re-issue no cell moved more than 0.4 kJ/kg from its
        // 311.20 kJ/kg seed after 10 000 steps against a 520.45 kJ/kg inlet BC,
        // i.e. the BC was inert). See the module tests
        // `inlet_enthalpy_bc_*_without_reapplying_each_step`.
        let he_bcs = capture_bcs(&self.he.boundary);

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

                // ── Flow-rate inlet (OpenFOAM `flowRateInletVelocity`) ───────
                // If a mass flowrate is prescribed on the inlet patch, re-derive
                // the fixed inlet velocity `u_in = ṁ / (ρ_inlet·A_inlet)` from
                // the density that is about to multiply it (`rho_f`'s inlet
                // patch values, used a few lines below to build the boundary
                // mass flux). Doing it here, from the live density and inside
                // the corrector loop, is what makes the *imposed mass flux*
                // exactly ṁ: the boundary flux assembled below is
                // `ρ_f·(u_in·Sf) = −ṁ` (negative = inflow, the `"left"` patch's
                // outward normal being −x) whatever the density does.
                //
                // The alternative a caller is forced into without this — pick
                // an assumed density once, convert to a velocity, and call
                // `set_inlet_velocity` — imposes `ṁ_actual = ρ_solved/ρ_assumed
                // · ṁ_target`, which drifts as the solved density departs from
                // the assumption. `self.u.boundary` *and* the captured `u_bcs`
                // template are both updated, so the momentum predictor and the
                // post-solve `correct_bcs_vec` re-stamps agree with it.
                self.apply_flow_rate_inlet(&rho_f, &mut u_bcs);

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
                                    // -- the same bug found and fixed in
                                    // `tampines-steam-tables`'s
                                    // `TampinesSteamArray::step` (see that
                                    // crate's `lateral_coupling.rs` regression
                                    // test and the workspace beads tracker).
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
                // `tampines-steam-tables`'s `TampinesSteamArray::step`.
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
                // transient cannot drive a cell to negative absolute pressure
                // (or an absurd overshoot). OpenFOAM's compressible pressure
                // control likewise limits *pressure* (not density) for robust
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
                // diverged (NaN) field is not masked here.
                let p_min_pa = self.p_min.get::<pascal>();
                let p_max_pa = self.p_max.get::<pascal>();
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

                // EOS update: ρ, T, ψ from the new pressure, then μ, αh from
                // the refreshed (T, ρ).
                self.correct_thermo();
                self.correct_transport();
            }

            // ── Hybrid all-Mach KNP dissipation (deferred correction) ────────
            // Assembled from the just-converged primitives. The continuity
            // dissipation is folded into `self.phi` HERE, *before* the EEqn's
            // `rho_cont`/`conv_he` recompute below both read `self.phi`, so the
            // discrete continuity invariant `(rho_cont − rho_old)/dt = −∇·φ`
            // still holds exactly, the conservative-ddt `h·∇·φ` cancellation
            // survives, AND the enthalpy shock-capturing is carried implicitly by
            // that convection (so no separate, destabilising energy source is
            // added — see `HybridDissipation`). Momentum dissipation is deferred
            // to the next outer corrector's UEqn. `Pimple` mode skips all of this.
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
            // (ρ_cont·h − ρ_old·h_old)/dt (bead op-ek2, mirroring the
            // `TampinesSteamArray` fix op-21g.14). Two coupled points:
            //
            //  1. The OLD-time term uses the OLD-time density `rho_old` (previous
            //     time level), restoring the `h_old·(ρ − ρ_old)/dt` term the old
            //     `ddt_coeff(&self.rho, …)` dropped (it reused the *current* ρ for
            //     both terms, so it really solved ρ·∂h/∂t + ∇·(φh) = dp/dt, whose
            //     un-cancelled `h·∇·φ` outflow over-drains enthalpy during a
            //     violent expansion).
            //
            //  2. The NEW-time coefficient is the **continuity density**
            //     `ρ_cont = ρ_old − dt·∇·φ` recomputed here from the final mass
            //     flux `self.phi`, NOT the EOS density `correct_thermo` wrote into
            //     `self.rho`. Only with `ρ_cont` does discrete continuity
            //     `(ρ_cont − ρ_old)/dt = −∇·φ` hold *exactly*, so the
            //     `h_old·(ρ_cont − ρ_old)/dt = −h_old·∇·φ` term cancels the
            //     `h·∇·φ` part of `∇·(φh)` term-for-term and the energy equation
            //     reduces to the material derivative `ρ Dh/Dt = dp/dt`. See
            //     `fvm::ddt_coeff_old`.
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
            let (mut he_new, _) = e_eqn.solve("he", settings);
            // Re-stamp the enthalpy BC template the solve just discarded — the
            // `he` counterpart of the `correct_bcs`/`correct_bcs_vec` calls in
            // the pressure/velocity loop above. See the `he_bcs` capture.
            correct_bcs(&mut he_new, &he_bcs);
            self.he = he_new;

            // ── (p, h) admissibility guard — enthalpy half ──────────────────
            // Clamp each cell's just-solved enthalpy into [h_min, h_max] right
            // here, before `correct_thermo` (next outer corrector, or the next
            // step's inner loop) hands it to the (p, h) flash — so an
            // overdraining energy solve cannot drive the flash to unphysical
            // temperatures. The pressure half is the [p_min, p_max] clamp in
            // the inner loop above. Unlike that (silent, OpenFOAM
            // `pressureControl::limit`-style) pressure clamp, every enthalpy
            // clamp is COUNTED in `h_clamp_events` so an engaged guard is
            // visible. The comparisons are false for NaN, so a genuinely
            // diverged (NaN) enthalpy is neither clamped nor counted — not
            // masked, mirroring the pressure-clamp NaN note.
            let h_min = self.h_min.get::<joule_per_kilogram>();
            let h_max = self.h_max.get::<joule_per_kilogram>();
            for hv in self.he.internal.iter_mut() {
                if *hv < h_min {
                    *hv = h_min;
                    self.h_clamp_events += 1;
                } else if *hv > h_max {
                    *hv = h_max;
                    self.h_clamp_events += 1;
                }
            }
        }
        self.clear_vectors();
    }

    /// Re-derive the fixed inlet velocity from the prescribed inlet mass
    /// flowrate ([`Self::inlet_mass_flowrate`]) and the **current** inlet-face
    /// density, i.e. OpenFOAM's `flowRateInletVelocity` boundary condition:
    ///
    /// ```text
    ///   u_in = ṁ / (ρ_inlet · A_inlet)        [m/s, +x = into the domain]
    /// ```
    ///
    /// `rho_f` must be the interpolated face density about to be used to build
    /// the boundary mass flux, so that the flux the pressure equation sees,
    /// `ρ_f·(u_in·Sf)`, is exactly `−ṁ` (negative = inflow through the `"left"`
    /// patch, whose outward normal is −x). `u_bcs` is `step`'s captured
    /// velocity-BC template, updated in lockstep so the post-solve
    /// `correct_bcs_vec` re-stamps this velocity and not a stale one.
    ///
    /// A no-op when no mass flowrate is prescribed (`inlet_mass_flowrate ==
    /// None`), when the inlet patch has no faces, or when the inlet area or
    /// density is not usable (non-finite / non-positive) — never a wrong
    /// number, mirroring [`Self::correct_thermo`]'s non-convergence handling.
    fn apply_flow_rate_inlet(
        &mut self,
        rho_f: &SurfaceScalarField,
        u_bcs: &mut [BoundaryCondition<Vector3>],
    ) {
        let Some(mdot) = self.inlet_mass_flowrate else {
            return;
        };
        let mdot = mdot.get::<kilogram_per_second>();
        let patch = &self.mesh.patches[INLET_PATCH];
        if patch.size == 0 {
            return;
        }

        // Area-weighted mean inlet-face density, and the total inlet area.
        let mut area = 0.0_f64;
        let mut rho_area = 0.0_f64;
        for fi in 0..patch.size {
            let a = self.mesh.face_areas[patch.start + fi];
            area += a;
            rho_area += a * rho_f.boundary[INLET_PATCH].values[fi];
        }
        // NaN fails `is_finite`, so a diverged density/area returns here rather
        // than prescribing a NaN velocity.
        if !area.is_finite() || area <= 0.0 {
            return;
        }
        let rho_in = rho_area / area;
        if !rho_in.is_finite() || rho_in <= 0.0 {
            return;
        }

        let v = Vector3::new(mdot / (rho_in * area), 0.0, 0.0);
        self.u.boundary[INLET_PATCH] = PatchField::fixed_value_vec(patch.size, v);
        u_bcs[INLET_PATCH] = BoundaryCondition::FixedValue(v);
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
    /// see [`Self::step`]). Raise `p_min` above the wide default floor to
    /// impose e.g. a cavitation floor and keep a violent transient off
    /// negative absolute pressure; lower `p_max` similarly. Panics if
    /// `p_min >= p_max`.
    pub fn set_pressure_bounds(&mut self, p_min: Pressure, p_max: Pressure) {
        assert!(
            p_min.get::<pascal>() < p_max.get::<pascal>(),
            "pressure bounds require p_min < p_max, got p_min = {} Pa, p_max = {} Pa",
            p_min.get::<pascal>(),
            p_max.get::<pascal>()
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
    /// `hi` it is applied at full weight, with a linear ramp between. Only affects
    /// [`SolverMode::HybridAllMach`]. Panics if `hi <= lo`.
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

    /// The current rarefied-tail density-taper window `(lo, hi)` \[kg/m³\] of
    /// the hybrid KNP dissipation. See [`Self::set_rho_taper_window`].
    pub fn get_rho_taper_window(&self) -> (MassDensity, MassDensity) {
        (self.hybrid_rho_taper_lo, self.hybrid_rho_taper_hi)
    }

    /// Sets the rarefied-tail density-taper window of the hybrid KNP
    /// dissipation: below `lo` the dissipation is scaled to zero (pure PIMPLE),
    /// at/above `hi` it is applied at full weight, with a linear ramp between
    /// (both in kg/m³, gated on the lighter side of each face). Only affects
    /// [`SolverMode::HybridAllMach`]. Panics if `hi <= lo`.
    ///
    /// The defaults (50/100 kg/m³) are a **steam-calibrated placeholder** from
    /// the `TampinesSteamArray` Edwards–O'Brien blowdown (dense two-phase
    /// flashing front depressurising to ~1 bar). A bare density window is only
    /// meaningful relative to the pressure it was calibrated at, so set a
    /// fluid- and pressure-appropriate window here before relying on
    /// `HybridAllMach` away from that regime — e.g. for a low-density gas
    /// (Helium at HTR-10 conditions, ρ ≈ 1–3 kg/m³) the default window zeroes
    /// the hybrid everywhere, degenerating it to plain PIMPLE. See
    /// `HYBRID_RHO_TAPER_LO_DEFAULT` (this module) for the full provenance note.
    pub fn set_rho_taper_window(&mut self, lo: MassDensity, hi: MassDensity) {
        assert!(
            hi.get::<kilogram_per_cubic_meter>() > lo.get::<kilogram_per_cubic_meter>(),
            "rho taper window requires hi > lo, got lo = {} kg/m^3, hi = {} kg/m^3",
            lo.get::<kilogram_per_cubic_meter>(),
            hi.get::<kilogram_per_cubic_meter>()
        );
        self.hybrid_rho_taper_lo = lo;
        self.hybrid_rho_taper_hi = hi;
    }

    /// The current enthalpy bounds `(h_min, h_max)` \[J/kg\] of the `(p, h)`
    /// admissibility guard, applied to every cell after each energy-equation
    /// solve. See [`Self::set_enthalpy_bounds`] and [`Self::h_min`].
    pub fn get_enthalpy_bounds(&self) -> (AvailableEnergy, AvailableEnergy) {
        (self.h_min, self.h_max)
    }

    /// Sets the enthalpy bounds `[h_min, h_max]` \[J/kg\] of the `(p, h)`
    /// admissibility guard: after every energy-equation solve each cell's
    /// specific enthalpy is clamped into this range — with every clamp counted
    /// in [`Self::h_clamp_events`], never silent — before
    /// [`Self::correct_thermo`] hands `(p, h)` to the EOS flash. Together with
    /// [`Self::set_pressure_bounds`] this bounds the whole `(p, h)` window the
    /// flash can be asked to evaluate, preventing an overdraining energy solve
    /// from producing unphysical temperatures. The defaults
    /// (−1×10⁷ / 1×10⁸ J/kg) are deliberately wide open; tighten per fluid —
    /// e.g. to the enthalpy range spanned by the fluid EOS's stated validity
    /// (`t_triple`..`t_max`, helium's `t_max` is 2000 K) at the working
    /// pressure. Panics if `h_min >= h_max`.
    pub fn set_enthalpy_bounds(&mut self, h_min: AvailableEnergy, h_max: AvailableEnergy) {
        assert!(
            h_min.get::<joule_per_kilogram>() < h_max.get::<joule_per_kilogram>(),
            "enthalpy bounds require h_min < h_max, got h_min = {} J/kg, h_max = {} J/kg",
            h_min.get::<joule_per_kilogram>(),
            h_max.get::<joule_per_kilogram>()
        );
        self.h_min = h_min;
        self.h_max = h_max;
    }

    /// Assemble the Mach-weighted KNP central-upwind dissipation from the
    /// **current** primitive state (`ρ, U, he, p` after the inner PISO loop's
    /// `correct_thermo`).
    ///
    /// Per internal face: the per-cell sound speed
    /// ([`hem_sound_speed_ph`] — CoolProp EOS single-phase / HEM-equilibrium
    /// two-phase) gives the cell Mach numbers; the face blend weight is
    /// `β(Ma_face)` with `Ma_face = min(Ma_owner, Ma_neighbour)` — the low-Mach
    /// scaling that keeps a subsonic acoustic interface stable (dissipation
    /// activates only where *both* sides are near-sonic). van-Leer MUSCL
    /// owner/neighbour reconstructions of `ρ, U, he, p, c` build the left/right
    /// [`FaceState`]s, and the deferred-correction dissipation is
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
        let taper_lo = self.hybrid_rho_taper_lo.get::<kilogram_per_cubic_meter>();
        let taper_hi = self.hybrid_rho_taper_hi.get::<kilogram_per_cubic_meter>();

        // Per-cell sound speed, Mach number, and a validity-edge safety flag.
        //
        // The KNP shock-capturing is only meaningful where the CoolProp closure
        // is well-defined. `safe[c]` marks cells whose `(p, h)` state yielded a
        // finite temperature, a pressure above the floor, and a physical sound
        // speed (`> C_MIN_MPS` — so a cell where neither the single-phase flash
        // nor the two-phase equilibrium closure produced a usable speed is
        // excluded). Faces touching an unsafe cell get no dissipation.
        let p_floor = self.p_min.get::<pascal>();
        let mut c_cell = vec![0.0_f64; n];
        let mut ma_cell = vec![0.0_f64; n];
        let mut safe = vec![false; n];
        for i in 0..n {
            let c = hem_sound_speed_ph(
                self.fluid,
                self.p.internal[i],
                self.he.internal[i],
                C_MIN_MPS,
            );
            c_cell[i] = c;
            ma_cell[i] = self.u.internal[i].mag() / c;
            let t_i = self.t.internal[i];
            safe[i] = t_i.is_finite() && self.p.internal[i] > p_floor && c > C_MIN_MPS;
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

            // Validity-edge guard: no shock-capturing where the closure is near
            // its (p, h) validity boundary on either side (see `safe`).
            if !safe[o] || !safe[nb] {
                continue;
            }

            // Blend weight gated on the *lower* Mach of the two adjacent cells
            // (subsonic ⇒ β = 0 ⇒ skip: exactly pure PIMPLE on this face).
            // `min(Ma)` is the low-Mach scaling that keeps the scheme stable at a
            // subsonic/near-sonic interface: a high-sound-speed (liquid) side is
            // genuinely low-Mach and must NOT be dissipated, so gating on the
            // subsonic side returns β = 0 there.
            let ma_f = ma_cell[o].min(ma_cell[nb]);
            let mut beta = mach_blend(ma_f, lo, hi);

            // Rarefied-tail (low-density) taper. The all-Mach KNP shock-capturing
            // targets a *dense* near-sonic region; as a cell rarefies toward
            // vacuum an explicit deferred-correction dissipation over-drives it.
            // The taper scales β to zero below `hybrid_rho_taper_lo` and to full
            // above `hybrid_rho_taper_hi` (configurable via
            // `set_rho_taper_window`; defaults are the steam-calibrated 50/100
            // kg/m³ placeholders), using the lighter (at-risk) face side. See
            // the regime note on `HYBRID_RHO_TAPER_LO_DEFAULT` for the CoolProp
            // low-density caveat.
            {
                let rho_face_min = self.rho.internal[o].min(self.rho.internal[nb]);
                let g = ((rho_face_min - taper_lo) / (taper_hi - taper_lo)).clamp(0.0, 1.0);
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
            // Momentum: owner loses the outgoing flux, neighbour gains it.
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
        assert!(
            (array.rho.internal[0] - 1.0).abs() > 0.05,
            "ρ must be EOS-derived, not the fallback 1.0"
        );

        array.run(10);

        let all_finite = array.p.internal.as_slice().iter().all(|x| x.is_finite())
            && array.rho.internal.as_slice().iter().all(|x| x.is_finite())
            && array.t.internal.as_slice().iter().all(|x| x.is_finite())
            && array
                .u
                .internal
                .as_slice()
                .iter()
                .all(|v| v.mag().is_finite());
        assert!(all_finite, "fields must stay finite over 10 steps");
        // Density and temperature stay physically bounded (correct_thermo ran).
        assert!(array
            .rho
            .internal
            .as_slice()
            .iter()
            .all(|&r| r > 0.0 && r < 1e3));
        assert!(array
            .t
            .internal
            .as_slice()
            .iter()
            .all(|&tt| tt > 0.0 && tt < 5e3));
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
        let expected_mu = crate::transport::viscosity(
            Fluid::Nitrogen,
            array.t.internal[0],
            array.rho.internal[0],
        )
        .unwrap();
        let expected_lambda = crate::transport::conductivity(
            Fluid::Nitrogen,
            array.t.internal[0],
            array.rho.internal[0],
        )
        .unwrap();
        let expected_cp =
            crate::props::state_trho(Fluid::Nitrogen, array.t.internal[0], array.rho.internal[0])
                .cp;

        // Nitrogen's real viscosity at (300K, ~1atm) happens to sit close to
        // the constructor's air-like placeholder (both ~1.8e-5 Pa.s), so this
        // checks the exact EOS-derived value rather than "changed from
        // placeholder" (which would be a coincidental, fragile check here).
        assert!(
            (mu1 - expected_mu).abs() / expected_mu < 1e-9,
            "mu should now be the EOS transport value"
        );
        assert!(
            (alpha_h1 - expected_lambda / expected_cp).abs() / (expected_lambda / expected_cp)
                < 1e-9,
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
        assert!(matches!(
            err,
            Err(MeshError::NonPositiveCellCount { got: 0 })
        ));
    }

    /// The opt-in [`SolverMode::HybridAllMach`] is a **no-op on a subsonic, low-
    /// density gas** (Nitrogen at ~1 bar): every internal face is deeply subsonic
    /// (`β(Ma) = 0`) *and* below the rarefied-tail density taper, so the KNP
    /// dissipation is identically zero and the run is bit-for-bit identical to the
    /// default [`SolverMode::Pimple`] path. This exercises the hybrid assembly
    /// path (`assemble_hybrid_dissipation`: the per-cell sound-speed/`safe`
    /// scan, the MUSCL reconstructions, the face loop and its guards) end-to-end
    /// without panicking, and pins the "hybrid is opt-in and does not perturb the
    /// subsonic default" contract. (To actually engage the KNP dissipation needs a
    /// near-sonic face with `ρ ≳ hybrid_rho_taper_hi` (default 100 kg/m³) — a high-pressure /
    /// choked-flow case, not this smoke test.)
    #[test]
    fn hybrid_mode_is_noop_on_subsonic_gas_and_matches_pimple() {
        let make = || {
            OPCPFluidArray::new(
                Fluid::Nitrogen,
                Length::new::<meter>(1.0),
                Area::new::<square_meter>(0.01),
                20,
                Time::new::<second>(1e-4),
            )
            .expect("valid 1-D geometry")
        };

        let mut pimple = make();
        pimple.run(10);

        let mut hybrid = make();
        hybrid.set_solver_mode(SolverMode::HybridAllMach);
        assert_eq!(hybrid.get_solver_mode(), SolverMode::HybridAllMach);
        hybrid.run(10);

        // Bit-for-bit identical to the default path (β = 0 on every face).
        for c in 0..pimple.mesh.n_cells {
            assert_eq!(
                hybrid.p.internal[c], pimple.p.internal[c],
                "hybrid pressure diverged from Pimple at cell {c} (should be a no-op)"
            );
            assert_eq!(hybrid.rho.internal[c], pimple.rho.internal[c]);
            assert_eq!(hybrid.he.internal[c], pimple.he.internal[c]);
            assert_eq!(hybrid.u.internal[c].x, pimple.u.internal[c].x);
            assert!(hybrid.p.internal[c].is_finite() && hybrid.rho.internal[c] > 0.0);
        }
    }

    /// The rho-taper window defaults to the steam-calibrated 50/100 kg/m³
    /// placeholder and round-trips through
    /// [`OPCPFluidArray::set_rho_taper_window`] /
    /// [`OPCPFluidArray::get_rho_taper_window`].
    #[test]
    fn rho_taper_window_defaults_and_roundtrips() {
        let mut array = OPCPFluidArray::new(
            Fluid::Nitrogen,
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(0.01),
            5,
            Time::new::<second>(1e-4),
        )
        .expect("valid 1-D geometry");

        let (lo, hi) = array.get_rho_taper_window();
        assert_eq!(lo.get::<kilogram_per_cubic_meter>(), 50.0);
        assert_eq!(hi.get::<kilogram_per_cubic_meter>(), 100.0);

        array.set_rho_taper_window(
            MassDensity::new::<kilogram_per_cubic_meter>(0.5),
            MassDensity::new::<kilogram_per_cubic_meter>(2.0),
        );
        let (lo, hi) = array.get_rho_taper_window();
        assert_eq!(lo.get::<kilogram_per_cubic_meter>(), 0.5);
        assert_eq!(hi.get::<kilogram_per_cubic_meter>(), 2.0);
    }

    /// The `(p, h)` admissibility guard defaults wide open, and when tightened
    /// it clamps the post-EEqn cell enthalpy **and counts every clamp** in
    /// [`OPCPFluidArray::h_clamp_events`] (visible, never silent — unlike the
    /// upstream-matching silent pressure clamp).
    #[test]
    fn enthalpy_guard_clamps_and_counts() {
        let mut array = OPCPFluidArray::new(
            Fluid::Nitrogen,
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(0.01),
            20,
            Time::new::<second>(1e-4),
        )
        .expect("valid 1-D geometry");

        // Wide-open defaults; no clamping on an untouched quiescent run.
        let (h_min, h_max) = array.get_enthalpy_bounds();
        assert_eq!(h_min.get::<joule_per_kilogram>(), -1.0e7);
        assert_eq!(h_max.get::<joule_per_kilogram>(), 1.0e8);
        assert_eq!(array.h_clamp_events, 0);
        array.run(2);
        assert_eq!(
            array.h_clamp_events, 0,
            "wide-open default bounds must not clamp a quiescent subsonic run"
        );

        // Tighten h_max below the current uniform enthalpy: the next EEqn
        // solve leaves h essentially unchanged (quiescent equilibrium), so
        // every cell must clamp to h_max and be counted.
        let h0 = array.he.internal[0];
        let h_cap = h0 - 1.0e4; // 10 kJ/kg below the current state
        array.set_enthalpy_bounds(
            AvailableEnergy::new::<joule_per_kilogram>(h0 - 1.0e5),
            AvailableEnergy::new::<joule_per_kilogram>(h_cap),
        );
        array.run(1);
        assert_eq!(
            array.h_clamp_events, array.mesh.n_cells,
            "each of the {} cells should clamp exactly once in one step",
            array.mesh.n_cells
        );
        for c in 0..array.mesh.n_cells {
            assert!(
                array.he.internal[c] <= h_cap,
                "cell {c} enthalpy {} exceeds the h_max bound {}",
                array.he.internal[c],
                h_cap
            );
        }
    }

    /// V&V: the **default steam-calibrated density taper zeroes the hybrid KNP
    /// dissipation for helium across the whole HTR-10 envelope** — i.e.
    /// [`SolverMode::HybridAllMach`] with the default taper degenerates to
    /// [`SolverMode::Pimple`] for helium, the honest, intended outcome for a
    /// deeply subsonic gas circuit (see the regime note on
    /// `HYBRID_RHO_TAPER_LO_DEFAULT`).
    ///
    /// **Methodology.** Helium mass density is computed from this crate's own
    /// Ortiz-Vega-et-al. Helmholtz EOS via `flash::state_pt(Fluid::Helium, T, p)`
    /// at four states spanning the HTR-10 operating envelope — core inlet
    /// (523.15 K, 3.0 MPa), design core outlet (973.15 K, 3.0 MPa),
    /// high-temperature test operation (1173.15 K, 3.0 MPa), and a cold
    /// depressurised state (300 K, 1 bar). Pass criterion: each density is
    /// strictly below the default lower taper threshold
    /// `hybrid_rho_taper_lo` = 50 kg/m³ (read from a freshly constructed
    /// helium array, so the assertion tracks the actual default), at which the
    /// rarefied-tail taper scales the KNP dissipation weight to exactly zero.
    ///
    /// **Results (2026-08-11, this crate's EOS, outram-park-fork-coolprop
    /// v0.1.1).** Computed densities:
    /// - 523.15 K, 3.0 MPa: **2.739989 kg/m³**
    /// - 973.15 K, 3.0 MPa: **1.478781 kg/m³**
    /// - 1173.15 K, 3.0 MPa: **1.227576 kg/m³**
    /// - 300 K, 1 bar: **0.160391 kg/m³**
    ///
    /// All are 1–2 orders of magnitude below the 50 kg/m³ default threshold,
    /// so the taper weight `g = clamp((ρ − 50)/(100 − 50), 0, 1)` is 0 on every
    /// face at every HTR-10 state: the hybrid is a guaranteed no-op for helium
    /// unless the window is retuned with
    /// [`OPCPFluidArray::set_rho_taper_window`].
    #[test]
    fn helium_htr10_default_taper_zeroes_hybrid() {
        let array = OPCPFluidArray::new(
            Fluid::Helium,
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(0.01),
            5,
            Time::new::<second>(1e-4),
        )
        .expect("valid 1-D geometry");
        let taper_lo = array.hybrid_rho_taper_lo.get::<kilogram_per_cubic_meter>();

        for (t_k, p_pa, label) in [
            (523.15, 3.0e6, "HTR-10 core inlet (523.15 K, 3.0 MPa)"),
            (
                973.15,
                3.0e6,
                "HTR-10 core outlet, design (973.15 K, 3.0 MPa)",
            ),
            (
                1173.15,
                3.0e6,
                "HTR-10 high-temperature test (1173.15 K, 3.0 MPa)",
            ),
            (300.0, 1.0e5, "cold depressurised (300 K, 1 bar)"),
        ] {
            let rho = flash::state_pt(Fluid::Helium, t_k, p_pa)
                .expect("single-phase helium state")
                .density;
            println!("MEASURE rho {label}: {rho:.6} kg/m^3");
            assert!(
                rho < taper_lo,
                "{label}: rho = {rho} kg/m^3 must sit below the default taper lo = {taper_lo}"
            );
        }
    }

    /// V&V: **helium at the HTR-10 full-power operating point is deeply
    /// subsonic** — so the Mach-blend gate (`β(Ma) = 0` below
    /// `ma_blend_lo` = 0.3) *also* keeps the hybrid KNP dissipation off,
    /// independently of the density taper.
    ///
    /// **Methodology.** Speed of sound and density are computed from this
    /// crate's own Helmholtz EOS via `flash::state_pt(Fluid::Helium,
    /// 748.15 K, 3.0 MPa)` (the core-average of the 523.15 K inlet / 973.15 K
    /// outlet at the 3.0 MPa system pressure). The bulk superficial velocity
    /// through the pebble-bed free-flow area is `u = ṁ/(ρ·A_free)` with the
    /// primary mass flow ṁ = 4.3 kg/s, core diameter 1.8 m (frontal area
    /// π/4·1.8² = 2.5447 m²), and bed porosity 0.39, giving
    /// A_free = 0.39·π/4·1.8² computed in the test. The geometry and operating
    /// figures (core diameter, porosity, mass flow, pressures, temperatures)
    /// are from the IAEA HTR-10 benchmark description — openly published
    /// (Open-tier) literature, per the maintainer directive of 2026-08-11.
    /// Pass criterion: Ma = u/c < 0.01.
    ///
    /// **Results (2026-08-11, this crate's EOS, outram-park-fork-coolprop
    /// v0.1.1).** ρ = **1.920936 kg/m³**, c = **1616.3754 m/s**,
    /// A_free = **0.992429 m²**, u = **2.255569 m/s**, so
    /// **Ma = 1.395×10⁻³** — two orders of magnitude below the 0.3 Mach-blend
    /// threshold and well under the 0.01 pass criterion. Interpretation: an
    /// HTR-10 helium circuit is a low-Mach flow for which `HybridAllMach`
    /// (correctly) contributes nothing; plain PIMPLE is the appropriate
    /// regime.
    #[test]
    fn helium_htr10_full_power_is_deeply_subsonic() {
        let state =
            flash::state_pt(Fluid::Helium, 748.15, 3.0e6).expect("single-phase helium state");
        let rho = state.density;
        let c = state.speed_of_sound;
        let mdot = 4.3_f64; // kg/s
        let d_core = 1.8_f64; // m
        let porosity = 0.39_f64;
        let a_free = core::f64::consts::PI / 4.0 * d_core * d_core * porosity; // m^2
        let u = mdot / (rho * a_free); // m/s
        let ma = u / c;
        println!("MEASURE rho = {rho:.6} kg/m^3, c = {c:.4} m/s, A_free = {a_free:.6} m^2, u = {u:.6} m/s, Ma = {ma:.3e}");
        assert!(
            ma < 0.01,
            "HTR-10 helium must be deeply subsonic, got Ma = {ma}"
        );
    }
}
