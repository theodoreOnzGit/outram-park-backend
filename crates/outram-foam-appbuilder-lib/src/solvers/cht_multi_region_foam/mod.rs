// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (openfoam.org)
//   Copyright (C) 2011-2020 OpenFOAM Foundation
// Upstream project: OpenFOAM-8 (OpenFOAM Foundation), github.com/OpenFOAM/OpenFOAM-8,
//   branch master, fetched 2026-11-03 to a scratch directory (not vendored).
// Upstream sources ported in this file:
//   src/ThermophysicalTransportModels/derivedFvPatchFields/
//     turbulentTemperatureCoupledBaffleMixed/
//     turbulentTemperatureCoupledBaffleMixedFvPatchScalarField.{H,C}
//       (the interface condition: constructor's contactRes_ from
//       thicknessLayers/kappaLayers, and updateCoeffs())
//   src/ThermophysicalTransportModels/derivedFvPatchFields/temperatureCoupledBase/
//     temperatureCoupledBase.C (kappa(): fluid -> kappaEff, solid -> kappa)
//   applications/solvers/heatTransfer/chtMultiRegionFoam/chtMultiRegionFoam.C
//     (the time loop: per PIMPLE outer pass, every fluid region then every
//     solid region), fluid/solveFluid.H (frozenFlow branch),
//     solid/solveSolid.H (the solid energy equation)
// Upstream licence: GPL-3.0-or-later.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # chtMultiRegionFoam — conjugate heat transfer between regions (gh:#1156)
//!
//! Fluid regions (each a [`PimpleFoam`] with an
//! [`EnergyEquation`](crate::solvers::energy::EnergyEquation) attached) and
//! solid regions ([`SolidRegion`]), each on its own mesh, joined at
//! conformal interface patches by the coupled-temperature condition.
//!
//! ## What upstream does (OpenFOAM-8, read 2026-11-03)
//!
//! **The interface condition**,
//! `compressible::turbulentTemperatureCoupledBaffleMixed`, is a mixed
//! (Robin) condition on the patch of each side. In `updateCoeffs()`, with
//! `kδ = kappa · patch().deltaCoeffs()` on each side:
//!
//! ```text
//! if (contactRes_ == 0)
//!     nbrIntFld = nbrField.patchInternalField();     // T in the cells beside the interface
//!     nbrKDelta = nbrField.kappa(nbrField)*nbrPatch.deltaCoeffs();
//! else
//!     nbrIntFld = nbrField;                          // the neighbour's face values
//!     nbrKDelta = contactRes_;                       // 1 / sum(thicknessLayers/kappaLayers)
//! refValue      = nbrIntFld;
//! refGrad       = 0;
//! valueFraction = nbrKDelta/(nbrKDelta + myKDelta);
//! ```
//!
//! The comment in the source states the intent: both sides agree on the face
//! temperature `(myKδ T_P + nbrKδ T_N)/(myKδ + nbrKδ)` and on the heat flux,
//! and this choice of `refValue`/`refGrad`/`valueFraction` makes the two
//! sides' equations the same. `kappa` comes from `temperatureCoupledBase`:
//! `kappaEff` of the thermophysical-transport model in a fluid (laminar: the
//! molecular k), the solid thermo's `kappa` in a solid.
//!
//! **The loop** (`chtMultiRegionFoam.C`): each time step, `while
//! (pimples.loop())` — the PIMPLE outer correctors — solves every fluid
//! region (`solveFluid.H`: `UEqn`, `EEqn`, the PISO loop; with
//! `frozenFlow`, only `EEqn`) and then every solid region (`solveSolid.H`:
//! the energy equation inside `correctNonOrthogonal()`). The interface
//! conditions are re-evaluated whenever a region's matrix is assembled, from
//! whatever the neighbour region holds at that moment: a block Gauss–Seidel
//! across regions, converged within a step by the outer correctors.
//!
//! ## How this port maps onto it, and where it differs
//!
//! | upstream | here |
//! |---|---|
//! | `turbulentTemperatureCoupledBaffleMixed::updateCoeffs` | [`CoupledTemperature::update_coeffs`], writing a per-face `BoundaryCondition::MixedField` |
//! | `thicknessLayers` / `kappaLayers` → `contactRes_` | [`CoupledTemperature::with_layers`] |
//! | `mappedPatchBase` sample patch | `outram_foam_basic_lib::mesh::RegionInterface` (face pairing by matching index or nearest face centre), reused |
//! | `patch().deltaCoeffs()` = `1/(n·(Cf − C))` on a boundary | the boundary distance of the region's own Laplacian (`DeltaCoeff`): `n·d` for the default `corrected`, `|d|` for the `orthogonal` ablation, so the condition and the matrix it feeds use one distance |
//! | `temperatureCoupledBase::kappa` | [`RegionId`] enum dispatch: fluid → its thermo's k (laminar `kappaEff`), solid → its k |
//! | fluid then solid, every outer pass | [`ChtMultiRegionFoam::step`]: each fluid's flow step, then [`ChtMultiRegionFoam::energy_outer_correctors`] passes of (every fluid's T, then every solid's T) |
//! | `frozenFlow` | `FvSolution::pimple.frozen_flow` on each fluid, as upstream |
//!
//! **One deliberate difference.** Upstream re-solves `UEqn`/`pEqn` in every
//! outer pass; here the flow step runs once per time step (with the fluid's
//! own `nOuterCorrectors`) and only the energy equations are iterated across
//! regions. With constant properties and no buoyancy the flow does not
//! depend on T, so the converged result is the same; the saving is the
//! repeated pressure solves. A Boussinesq or variable-property flow would
//! need the upstream ordering back.
//!
//! **And one ordering choice.** Upstream's fluid `EEqn` comes before the PISO
//! loop and uses the flux of the previous corrector, which suits its
//! compressible energy equation (continuity is solved alongside). This
//! incompressible T equation is solved after the PISO loop with the
//! corrected, divergence-free flux, as `MeltFoam` does, so `div(φ T)`
//! conserves energy cell by cell.
//!
//! ## Radiative flux: `qr`/`qrNbr` of `turbulentTemperatureRadCoupledMixed` (gh:#1531)
//!
//! ESI v2506's `compressible::turbulentTemperatureRadCoupledMixed` (read 2026-11-14,
//! `src/thermoTools/derivedFvPatchFields/turbulentTemperatureRadCoupledMixed/`) is the same
//! mixed condition with two more entries, `qr` (this patch's radiative flux field) and `qrNbr`
//! (the neighbour patch's), each a field name or `none`:
//!
//! ```text
//! KDeltaC = GREAT                                    // no layers
//! alpha   = kappaTp*(1 + KDeltaNbr/KDeltaC)*deltaCoeffs
//! valueFraction = KDeltaNbr/(KDeltaNbr + alpha);     refValue = TcNbr;
//! refGrad       = (qr + qrNbr)/kappaTp;              // was 0 above
//! ```
//!
//! `qr` is the radiation model's `qr` field, whose wall values are `-gamma snGrad(G)` (P1.C),
//! and ESI's tutorial `solarBeamWithTrees` sets the fluid side `qr qr; qrNbr none;` and the solid
//! side `qr none; qrNbr qr;`. Here [`InterfaceRadiation::Auto`] (the default) does exactly that
//! for every fluid that has [`EnergyEquation::with_radiation`](crate::solvers::energy::EnergyEquation::with_radiation)
//! attached: [`CoupledTemperature::update_coeffs_with_flux`] adds `(qr + qrNbr)/kappa` as `refGrad`,
//! with `qr` the radiation state of the fluid's last `correct()` (lagged, as upstream's field is when the
//! matrix is assembled; for the solid, after the fluid's `correct()` of the same pass). Ablation:
//! [`CoupledTemperature::without_radiative_flux`] / [`ChtMultiRegionFoam::without_radiative_flux_coupling`].
//! Without radiation in either region `refGrad` stays exactly 0 and nothing changes.
//!
//! **Layers.** With `thicknessLayers`/`kappaLayers`, `RadCoupledMixed` uses `KDeltaC = 1/(sum(t/k) + SMALL)` inside
//! `alpha` and the neighbour *cell* value, while `turbulentTemperatureCoupledBaffleMixed` (what
//! [`InterfaceMode::CoupledWithLayers`] follows) uses the contact conductance in place of the neighbour's
//! `kappa deltaCoeffs` and the neighbour *face* value. The two differ, so layers combined with the radiative flux
//! are not claimed to match `RadCoupledMixed` (gh:#1531 follow-up); without layers the two conditions are the
//! same to round-off (gh:#1363 compares the port with `RadCoupledMixed` itself, bit for bit).
//!
//! ## Physics on by default; the ablation
//!
//!
//! Every interface is coupled unless the caller asks otherwise:
//! [`ChtMultiRegionFoam::without_interface_coupling`] turns each interface
//! into an adiabatic (zeroGradient) wall on both sides. Pinned by
//! `tests::interface_coupling_is_on_by_default`.

use std::sync::Arc;

use crate::error::AppBuilderError;
use crate::solvers::energy::{face_conduction_in, EnergyEquation, EnergySolve, IncompressibleThermo};
use crate::solvers::non_orthogonal::LaplacianCorrection;
use crate::solvers::pimple_foam::PimpleFoam;
use outram_foam_basic_lib::prelude::*;
use uom::si::f64::{Length, Power, TemperatureInterval, ThermalConductivity, Time};
use uom::si::length::meter;
use uom::si::power::watt;
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::time::second;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;

/// A solid region: conduction only, `ρ c ∂T/∂t − ∇·(k ∇T) = 0`
/// (`solveSolid.H`, isotropic).
#[derive(Debug, Clone)]
pub struct SolidRegion {
    /// Region name (for reports).
    pub name: String,
    /// The temperature, its properties and settings.
    pub energy: EnergyEquation,
    /// Laplacian treatment; default `Gauss linear corrected`.
    pub laplacian: LaplacianCorrection,
    /// Non-orthogonal correctors of the solid equation (upstream
    /// `nNonOrthCorr` of the solid's PIMPLE dict); default 0.
    pub n_non_orthogonal_correctors: usize,
}

impl SolidRegion {
    /// A solid at uniform `t0` \[K\], every patch adiabatic until set.
    #[must_use]
    pub fn new(name: impl Into<String>, mesh: Arc<FvMesh>, thermo: IncompressibleThermo, t0: f64) -> Self {
        Self {
            name: name.into(),
            energy: EnergyEquation::uniform(mesh, thermo, t0),
            laplacian: LaplacianCorrection::Corrected,
            n_non_orthogonal_correctors: 0,
        }
    }
}

/// Which region a side of an interface belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionId {
    /// Index into [`ChtMultiRegionFoam::fluids`].
    Fluid(usize),
    /// Index into [`ChtMultiRegionFoam::solids`].
    Solid(usize),
}

/// One side of an interface: the region and the patch index in its mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterfaceSide {
    /// The region.
    pub region: RegionId,
    /// Patch index in that region's mesh.
    pub patch: usize,
}

/// How an interface couples its two sides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InterfaceMode {
    /// `turbulentTemperatureCoupledBaffleMixed` without layers (the default).
    Coupled,
    /// With `thicknessLayers`/`kappaLayers`: the contact conductance
    /// `contactRes_ = 1/Σ(t_i/k_i)` \[W/(m² K)\].
    CoupledWithLayers {
        /// `1/Σ(thickness/kappa)` \[W/(m² K)\].
        contact_conductance: f64,
    },
    /// The ablation: both sides adiabatic (zeroGradient).
    Adiabatic,
}

/// Which radiative-flux fields enter one side's `refGrad` (gh:#1531): ESI
/// `turbulentTemperatureRadCoupledMixed`'s `qr` (this side's own patch field)
/// and `qrNbr` (the neighbour side's), each either a field name or `none`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RadiativeFluxTerms {
    /// `qr qr;` (true) or `qr none;`: this side's own radiative flux.
    pub own: bool,
    /// `qrNbr qr;` (true) or `qrNbr none;`: the neighbour side's radiative flux.
    pub neighbour: bool,
}

/// How an interface takes the radiative flux `qr` of a P1 fluid (gh:#1531). Upstream's
/// `refGrad = (qr + qrNbr)/kappa` (module docs, "Radiative flux").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InterfaceRadiation {
    /// **The default, physics on.** Each side takes `qr` from its own region when that region is a
    /// fluid with radiation attached (`qr qr; qrNbr none;`, the fluid side of ESI's
    /// `solarBeamWithTrees`) and `qrNbr` from the neighbour region under the same condition (the
    /// solid side: `qr none; qrNbr qr;`). With no radiation in either region this is bit for bit
    /// the interface without the term.
    #[default]
    Auto,
    /// The case states per side which fields enter (upstream's `qr`/`qrNbr` entries). Naming a
    /// field the region does not have is an error at [`ChtMultiRegionFoam::new`] (upstream:
    /// `lookupPatchField` fails).
    Explicit {
        /// Terms on side A.
        a: RadiativeFluxTerms,
        /// Terms on side B.
        b: RadiativeFluxTerms,
    },
    /// The ablation ([`CoupledTemperature::without_radiative_flux`]): `qr none; qrNbr none;`
    /// on both sides, the interface of gh:#1531's first slice.
    Ablated,
}

/// A coupled-temperature interface between two regions: a port of
/// `compressible::turbulentTemperatureCoupledBaffleMixed`, with the pairing
/// of faces from [`RegionInterface`]. Without layers it is also
/// `turbulentTemperatureRadCoupledMixed` (module docs, "Radiative flux").
#[derive(Debug, Clone)]
pub struct CoupledTemperature {
    /// Side A (the pairing's mesh A).
    pub a: InterfaceSide,
    /// Side B (the pairing's mesh B).
    pub b: InterfaceSide,
    /// `pairing.face_map[i]` is the B face beside A face `i`.
    pub pairing: RegionInterface,
    /// Coupled (default), coupled through layers, or the adiabatic ablation.
    pub mode: InterfaceMode,
    /// The radiative flux `qr`/`qrNbr` in `refGrad` (gh:#1531); default [`InterfaceRadiation::Auto`].
    pub radiation: InterfaceRadiation,
}

impl CoupledTemperature {
    /// The interface between patch `patch_a` of region `a` on `mesh_a` and
    /// patch `patch_b` of region `b` on `mesh_b`, faces paired by nearest
    /// face centre (exact for a conformal interface; check with
    /// [`Self::max_pairing_gap`]).
    #[must_use]
    pub fn new(a: RegionId, mesh_a: Arc<FvMesh>, patch_a: usize, b: RegionId, mesh_b: Arc<FvMesh>, patch_b: usize) -> Self {
        let pairing = RegionInterface::from_face_centres(mesh_a, patch_a, mesh_b, patch_b);
        Self {
            a: InterfaceSide { region: a, patch: patch_a },
            b: InterfaceSide { region: b, patch: patch_b },
            pairing,
            mode: InterfaceMode::Coupled,
            radiation: InterfaceRadiation::Auto,
        }
    }

    /// State per side which radiative-flux fields enter `refGrad` (upstream's `qr`/`qrNbr`
    /// entries), instead of the default [`InterfaceRadiation::Auto`].
    #[must_use]
    pub fn with_radiative_flux(mut self, a: RadiativeFluxTerms, b: RadiativeFluxTerms) -> Self {
        self.radiation = InterfaceRadiation::Explicit { a, b };
        self
    }

    /// The ablation: `qr none; qrNbr none;` on both sides, whatever the regions radiate.
    #[must_use]
    pub fn without_radiative_flux(mut self) -> Self {
        self.radiation = InterfaceRadiation::Ablated;
        self
    }

    /// Upstream `thicknessLayers` / `kappaLayers`: a contact layer stack
    /// between the two sides, `contactRes_ = 1/Σ(t_i/k_i)`.
    #[must_use]
    pub fn with_layers(mut self, layers: &[(Length, ThermalConductivity)]) -> Self {
        let r: f64 = layers.iter().map(|(t, k)| t.get::<meter>() / k.get::<watt_per_meter_kelvin>()).sum();
        if r > 0.0 {
            self.mode = InterfaceMode::CoupledWithLayers { contact_conductance: 1.0 / r };
        }
        self
    }

    /// Largest distance between paired face centres \[m\]: zero (round-off)
    /// on a conformal interface. Also fails loudly (returns infinity) if the
    /// pairing is not one-to-one.
    #[must_use]
    pub fn max_pairing_gap(&self) -> f64 {
        let (ma, mb) = (&self.pairing.mesh_a, &self.pairing.mesh_b);
        let (pa, pb) = (&ma.patches[self.pairing.patch_a], &mb.patches[self.pairing.patch_b]);
        if pa.size != pb.size {
            return f64::INFINITY;
        }
        let mut seen = vec![false; pb.size];
        let mut gap = 0.0_f64;
        for (i, &j) in self.pairing.face_map.iter().enumerate() {
            if seen[j] {
                return f64::INFINITY;
            }
            seen[j] = true;
            gap = gap.max((ma.face_centres[pa.start + i] - mb.face_centres[pb.start + j]).mag());
        }
        gap
    }

    /// `updateCoeffs()` for side `my` (`true` = A, `false` = B): the mixed
    /// condition from the neighbour's current field, as upstream (module
    /// docs). `my_t`, `nbr_t` are the two temperature fields, `my_k`,
    /// `nbr_k` their conductivities, `my_mode`, `nbr_mode` the boundary
    /// distances of their Laplacians.
    #[must_use]
    pub fn update_coeffs(
        &self,
        my_is_a: bool,
        my_t: &VolScalarField,
        my_k: ThermalConductivity,
        my_mode: fvm::DeltaCoeff,
        nbr_t: &VolScalarField,
        nbr_k: ThermalConductivity,
        nbr_mode: fvm::DeltaCoeff,
    ) -> BoundaryCondition<f64> {
        self.update_coeffs_with_flux(my_is_a, my_t, my_k, my_mode, nbr_t, nbr_k, nbr_mode, None, None)
    }

    /// [`Self::update_coeffs`] with the radiative flux of `turbulentTemperatureRadCoupledMixed`
    /// (gh:#1531): `refGrad = (qr + qrNbr)/kappaTp` per face, the other two coefficients
    /// unchanged. `qr` is this side's patch field `qr` in this side's face order, `qr_nbr` the
    /// neighbour patch's `qr` in the **neighbour's** face order (it is paired through the face map,
    /// upstream's `distribute`); `None` is upstream's `none` (zero). `(None, None)` leaves
    /// `refGrad` exactly 0, as before.
    #[must_use]
    pub fn update_coeffs_with_flux(
        &self,
        my_is_a: bool,
        my_t: &VolScalarField,
        my_k: ThermalConductivity,
        my_mode: fvm::DeltaCoeff,
        nbr_t: &VolScalarField,
        nbr_k: ThermalConductivity,
        nbr_mode: fvm::DeltaCoeff,
        qr: Option<&[f64]>,
        qr_nbr: Option<&[f64]>,
    ) -> BoundaryCondition<f64> {
        let (my_k, nbr_k) = (my_k.get::<watt_per_meter_kelvin>(), nbr_k.get::<watt_per_meter_kelvin>());
        let (my_patch, nbr_patch) = if my_is_a { (self.pairing.patch_a, self.pairing.patch_b) } else { (self.pairing.patch_b, self.pairing.patch_a) };
        let (my_mesh, nbr_mesh) = (&my_t.mesh, &nbr_t.mesh);
        let n = my_mesh.patches[my_patch].size;
        if matches!(self.mode, InterfaceMode::Adiabatic) {
            return BoundaryCondition::ZeroGradient;
        }
        // The neighbour face beside each of my faces.
        let nbr_face: Vec<usize> = if my_is_a {
            self.pairing.face_map.clone()
        } else {
            let mut inv = vec![0; n];
            for (i, &j) in self.pairing.face_map.iter().enumerate() {
                inv[j] = i;
            }
            inv
        };
        let my_start = my_mesh.patches[my_patch].start;
        let nbr_start = nbr_mesh.patches[nbr_patch].start;
        let mut w = Vec::with_capacity(n);
        let mut r = Vec::with_capacity(n);
        let mut g = Vec::with_capacity(n);
        for (fi, &fj) in nbr_face.iter().enumerate() {
            let my_kd = my_k / my_mode.boundary_delta(my_mesh, my_start + fi);
            let (nbr_val, nbr_kd) = match self.mode {
                InterfaceMode::CoupledWithLayers { contact_conductance } => (nbr_t.boundary[nbr_patch].values[fj], contact_conductance),
                _ => (nbr_t.internal[nbr_mesh.owner[nbr_start + fj]], nbr_k / nbr_mode.boundary_delta(nbr_mesh, nbr_start + fj)),
            };
            r.push(nbr_val);
            w.push(nbr_kd / (nbr_kd + my_kd));
            g.push(if qr.is_none() && qr_nbr.is_none() { 0.0 } else { (qr.map_or(0.0, |q| q[fi]) + qr_nbr.map_or(0.0, |q| q[fj])) / my_k });
        }
        BoundaryCondition::MixedField { value_fraction: Field::new(w), ref_value: Field::new(r), ref_grad: Field::new(g) }
    }
}

/// What one [`ChtMultiRegionFoam::step`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct ChtStep {
    /// Energy outer passes taken.
    pub passes: usize,
    /// Largest cell |ΔT| \[K\] of the last pass, over every region.
    pub last_pass_change: f64,
    /// Largest cell |T − T_old| \[K\] over the step, over every region.
    pub step_change: f64,
}

/// The multi-region solver.
#[derive(Debug)]
pub struct ChtMultiRegionFoam {
    /// Fluid regions; each must carry `energy: Some(..)`.
    pub fluids: Vec<PimpleFoam>,
    /// Solid regions.
    pub solids: Vec<SolidRegion>,
    /// The coupled interfaces.
    pub interfaces: Vec<CoupledTemperature>,
    /// Energy passes per step across all regions (upstream
    /// `nOuterCorrectors`, energy part); default 1.
    pub energy_outer_correctors: usize,
    /// Stop the passes early once a pass changes no cell by more than this
    /// (upstream `outerCorrectorResidualControl`, here on |ΔT|); default 0
    /// (never early).
    pub energy_outer_tolerance: TemperatureInterval,
    /// Time step of the solid regions; each fluid's T uses its own
    /// `control.delta_t`. Ignored by a `SteadyState` energy equation.
    /// Default 1 s.
    pub solid_delta_t: Time,
}

impl ChtMultiRegionFoam {
    /// A solver over the given regions and interfaces, every interface
    /// coupled (the default).
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] if a fluid has no energy equation or an
    /// interface names a missing region.
    pub fn new(fluids: Vec<PimpleFoam>, solids: Vec<SolidRegion>, interfaces: Vec<CoupledTemperature>) -> Result<Self, AppBuilderError> {
        let s = Self {
            fluids,
            solids,
            interfaces,
            energy_outer_correctors: 1,
            energy_outer_tolerance: TemperatureInterval::new::<kelvin_interval>(0.0),
            solid_delta_t: Time::new::<second>(1.0),
        };
        for (i, f) in s.fluids.iter().enumerate() {
            if f.energy.is_none() {
                return Err(config_error(format!("fluid region {i} has no energy equation")));
            }
        }
        for c in &s.interfaces {
            for side in [c.a, c.b] {
                let ok = match side.region {
                    RegionId::Fluid(i) => i < s.fluids.len(),
                    RegionId::Solid(i) => i < s.solids.len(),
                };
                if !ok {
                    return Err(config_error(format!("interface names a missing region {:?}", side.region)));
                }
            }
        }
        s.check_radiative_fluxes()?;
        Ok(s)
    }

    /// The ablation: every interface adiabatic on both sides (no conjugate
    /// heat transfer). The default couples every interface.
    #[must_use]
    pub fn without_interface_coupling(mut self) -> Self {
        for c in &mut self.interfaces {
            c.mode = InterfaceMode::Adiabatic;
        }
        self
    }

    fn temperature(&self, r: RegionId) -> &VolScalarField {
        match r {
            RegionId::Fluid(i) => &self.fluids[i].energy.as_ref().expect("checked in new").t,
            RegionId::Solid(i) => &self.solids[i].energy.t,
        }
    }

    /// `temperatureCoupledBase::kappa` and the region's Laplacian distance.
    fn kappa_and_mode(&self, r: RegionId) -> (f64, fvm::DeltaCoeff) {
        match r {
            RegionId::Fluid(i) => {
                let f = &self.fluids[i];
                let lap = LaplacianCorrection::from_scheme(&f.schemes.default_laplacian).unwrap_or(LaplacianCorrection::Corrected);
                (f.energy.as_ref().expect("checked in new").thermo.kappa(), lap.delta_coeff())
            }
            RegionId::Solid(i) => (self.solids[i].energy.thermo.kappa(), self.solids[i].laplacian.delta_coeff()),
        }
    }

    /// Whether region `r` carries a radiative flux `qr` (a fluid with P1 attached).
    fn has_radiative_flux(&self, r: RegionId) -> bool {
        match r {
            RegionId::Fluid(i) => self.fluids[i].energy.as_ref().is_some_and(|e| e.radiation.is_some()),
            RegionId::Solid(_) => false,
        }
    }

    /// The radiative wall flux `qr` of region `side.region` on `side.patch`, from the radiation
    /// state of its last `correct()`; `None` if the region has none.
    fn radiative_flux_on(&self, side: InterfaceSide) -> Option<Vec<f64>> {
        match side.region {
            RegionId::Fluid(i) => self.fluids[i].energy.as_ref()?.radiation.as_ref().map(|r| r.state.qr[side.patch].as_slice().to_vec()),
            RegionId::Solid(_) => None,
        }
    }

    /// Which of `qr`/`qrNbr` enter side `my` of interface `c` (the [`InterfaceRadiation`] rule).
    fn radiative_terms(&self, c: &CoupledTemperature, my_is_a: bool) -> RadiativeFluxTerms {
        let (my, nbr) = if my_is_a { (c.a, c.b) } else { (c.b, c.a) };
        match c.radiation {
            InterfaceRadiation::Ablated => RadiativeFluxTerms::default(),
            InterfaceRadiation::Explicit { a, b } => {
                if my_is_a {
                    a
                } else {
                    b
                }
            }
            InterfaceRadiation::Auto => RadiativeFluxTerms { own: self.has_radiative_flux(my.region), neighbour: self.has_radiative_flux(nbr.region) },
        }
    }

    /// Refuse an [`InterfaceRadiation::Explicit`] interface that names a flux its region does not
    /// have (upstream: `lookupPatchField` of a missing `qr` is fatal).
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] naming the interface side.
    pub fn check_radiative_fluxes(&self) -> Result<(), AppBuilderError> {
        for (k, c) in self.interfaces.iter().enumerate() {
            if let InterfaceRadiation::Explicit { a, b } = c.radiation {
                for (name, terms, my, nbr) in [("A", a, c.a, c.b), ("B", b, c.b, c.a)] {
                    if (terms.own && !self.has_radiative_flux(my.region)) || (terms.neighbour && !self.has_radiative_flux(nbr.region)) {
                        return Err(config_error(format!("interface {k} side {name} names a radiative flux qr that its region does not have")));
                    }
                }
            }
        }
        Ok(())
    }

    /// The ablation of gh:#1531: every interface `qr none; qrNbr none;`. The default takes the
    /// radiative flux of every P1 fluid ([`InterfaceRadiation::Auto`]).
    #[must_use]
    pub fn without_radiative_flux_coupling(mut self) -> Self {
        for c in &mut self.interfaces {
            c.radiation = InterfaceRadiation::Ablated;
        }
        self
    }

    fn temperature_mut(&mut self, r: RegionId) -> &mut VolScalarField {
        match r {
            RegionId::Fluid(i) => &mut self.fluids[i].energy.as_mut().expect("checked in new").t,
            RegionId::Solid(i) => &mut self.solids[i].energy.t,
        }
    }

    /// Re-evaluate every interface patch of region `r` from its neighbours'
    /// current fields (upstream: when the region's matrix is assembled).
    pub fn update_interfaces_of(&mut self, r: RegionId) {
        let mut updates = Vec::new();
        for c in &self.interfaces {
            for (my_is_a, my, nbr) in [(true, c.a, c.b), (false, c.b, c.a)] {
                if my.region != r {
                    continue;
                }
                let (mk, mm) = self.kappa_and_mode(my.region);
                let (nk, nm) = self.kappa_and_mode(nbr.region);
                let w = ThermalConductivity::new::<watt_per_meter_kelvin>;
                let terms = self.radiative_terms(c, my_is_a);
                let qr = if terms.own { self.radiative_flux_on(my) } else { None };
                let qr_nbr = if terms.neighbour { self.radiative_flux_on(nbr) } else { None };
                let bc = c.update_coeffs_with_flux(my_is_a, self.temperature(my.region), w(mk), mm, self.temperature(nbr.region), w(nk), nm, qr.as_deref(), qr_nbr.as_deref());
                updates.push((my.patch, bc));
            }
        }
        let t = self.temperature_mut(r);
        for (pi, bc) in updates {
            t.boundary[pi].bc = bc;
        }
        crate::solvers::energy::evaluate_boundary_values(t);
    }

    /// One time step (module docs): each non-frozen fluid's flow step, then
    /// up to `energy_outer_correctors` passes of every fluid's then every
    /// solid's energy equation, each region's interfaces re-evaluated just
    /// before its solve.
    ///
    /// # Errors
    /// Whatever a region's flow or energy solve returns.
    pub fn step(&mut self) -> Result<ChtStep, AppBuilderError> {
        let t_old_f: Vec<VolScalarField> = self.fluids.iter().map(|f| f.energy.as_ref().expect("checked in new").t.clone()).collect();
        let t_old_s: Vec<VolScalarField> = self.solids.iter().map(|s| s.energy.t.clone()).collect();
        for f in &mut self.fluids {
            if let Some(e) = f.energy.as_mut() {
                e.begin_time_step();
            }
            if !f.solution.pimple.frozen_flow {
                f.step_flow()?;
            }
        }
        let mut passes = 0;
        let mut last = 0.0;
        for _ in 0..self.energy_outer_correctors.max(1) {
            last = 0.0_f64;
            for i in 0..self.fluids.len() {
                self.update_interfaces_of(RegionId::Fluid(i));
                let r: EnergySolve = self.fluids[i].solve_energy(&t_old_f[i])?;
                last = last.max(r.max_change);
            }
            for i in 0..self.solids.len() {
                self.update_interfaces_of(RegionId::Solid(i));
                let s = &mut self.solids[i];
                let r = s.energy.solve(&t_old_s[i], None, self.solid_delta_t, s.laplacian, s.n_non_orthogonal_correctors)?;
                last = last.max(r.max_change);
            }
            passes += 1;
            if last <= self.energy_outer_tolerance.get::<kelvin_interval>() {
                break;
            }
        }
        let change = |a: &VolScalarField, b: &VolScalarField| a.internal.as_slice().iter().zip(b.internal.as_slice()).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs()));
        let mut step_change = 0.0_f64;
        for (f, o) in self.fluids.iter().zip(&t_old_f) {
            step_change = step_change.max(change(&f.energy.as_ref().expect("checked in new").t, o));
        }
        for (s, o) in self.solids.iter().zip(&t_old_s) {
            step_change = step_change.max(change(&s.energy.t, o));
        }
        Ok(ChtStep { passes, last_pass_change: last, step_change })
    }

    /// Heat into region `side.region` through the interface patch on that
    /// side \[W\], as that region's matrix saw it.
    #[must_use]
    pub fn interface_heat_in(&self, side: InterfaceSide) -> Power {
        let (k, mode) = self.kappa_and_mode(side.region);
        let t = self.temperature(side.region);
        let q = (0..t.mesh.patches[side.patch].size).map(|fi| face_conduction_in(t, side.patch, fi, k, mode)).sum();
        Power::new::<watt>(q)
    }
}

fn config_error(msg: String) -> AppBuilderError {
    AppBuilderError::Case { case: std::path::PathBuf::from("chtMultiRegionFoam"), msg }
}

/// A 1-D slab mesh for conduction checks: `n` cells along x from `x0` to
/// `x0 + len`, unit cross-section (1 m × 1 m), patches `names[0]` (x-min),
/// `names[1]` (x-max) and `sides` (the four lateral faces of every cell;
/// leave them zeroGradient for 1-D conduction).
///
/// # Panics
/// If `n == 0` (no mesh), or if the mesh builder rejects the topology, which
/// it cannot for `n ≥ 1`.
#[must_use]
pub fn line_mesh(n: usize, len: Length, x0: Length, names: [&str; 2]) -> FvMesh {
    assert!(n > 0, "line_mesh needs at least one cell");
    let (len, x0) = (len.get::<meter>(), x0.get::<meter>());
    let h = len / n as f64;
    let xc = |i: usize| x0 + (i as f64 + 0.5) * h;
    let mut owner = Vec::new();
    let mut neighbour = Vec::new();
    let mut sf = Vec::new();
    let mut cf = Vec::new();
    for i in 0..n - 1 {
        owner.push(i);
        neighbour.push(i + 1);
        sf.push(Vector3::new(1.0, 0.0, 0.0));
        cf.push(Vector3::new(x0 + (i + 1) as f64 * h, 0.5, 0.5));
    }
    owner.push(0);
    sf.push(Vector3::new(-1.0, 0.0, 0.0));
    cf.push(Vector3::new(x0, 0.5, 0.5));
    owner.push(n - 1);
    sf.push(Vector3::new(1.0, 0.0, 0.0));
    cf.push(Vector3::new(x0 + len, 0.5, 0.5));
    for i in 0..n {
        for (s, c) in [
            (Vector3::new(0.0, -h, 0.0), Vector3::new(xc(i), 0.0, 0.5)),
            (Vector3::new(0.0, h, 0.0), Vector3::new(xc(i), 1.0, 0.5)),
            (Vector3::new(0.0, 0.0, -h), Vector3::new(xc(i), 0.5, 0.0)),
            (Vector3::new(0.0, 0.0, h), Vector3::new(xc(i), 0.5, 1.0)),
        ] {
            owner.push(i);
            sf.push(s);
            cf.push(c);
        }
    }
    let nb = n - 1;
    FvMeshBuilder::new()
        .n_cells(n)
        .n_internal_faces(nb)
        .owner(owner)
        .neighbour(neighbour)
        .patches(vec![
            BoundaryPatch::new(names[0], nb, 1, PatchKind::Patch),
            BoundaryPatch::new(names[1], nb + 1, 1, PatchKind::Patch),
            BoundaryPatch::new("sides", nb + 2, 4 * n, PatchKind::Wall),
        ])
        .cell_volumes(vec![h; n])
        .cell_centres((0..n).map(|i| Vector3::new(xc(i), 0.5, 0.5)).collect())
        .face_areas(sf.iter().map(|v| v.mag()).collect())
        .face_area_vectors(sf)
        .face_centres(cf)
        .build()
        .expect("a line of hexes is a valid mesh")
}

#[cfg(test)]
mod tests;
