// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), ESI v2506
//   Copyright (C) 2011-2017 OpenFOAM Foundation
//   Copyright (C) 2016-2025 OpenCFD Ltd.
// Upstream sources read for this file (ESI v2506):
//   src/thermophysicalModels/radiation/radiationModels/viewFactor/viewFactor.C
//       (`calculate()` writes `qr` on the `viewFactorWall` patches; `Ru`, `Rp` are zero so
//       `rad.Sh` adds nothing to the energy equation)
//   src/thermophysicalModels/radiation/radiationModels/radiationModel/radiationModel.C
//       (`correct()`: `firstIter_ || timeIndex % solverFreq_ == 0`)
//   applications/solvers/heatTransfer/chtMultiRegionFoam/fluid/EEqn.H
//       (`EEqn.solve; thermo.correct(); rad.correct();`)
// Upstream licence: GPL-3.0-or-later.

//! # Opt-in `viewFactor` surface-to-surface radiation beside P1 (gh:#1541, epic gh:#1379)
//!
//! [`ViewFactorCoupling`] is attached to an [`EnergyEquation`](crate::solvers::energy::EnergyEquation)
//! with [`EnergyEquation::with_view_factor_radiation`](crate::solvers::energy::EnergyEquation::with_view_factor_radiation).
//! **Off by default**: an energy equation without it is bit for bit what it was.
//!
//! ## What upstream does, and what is here
//!
//! ESI's `viewFactor` model has no volumetric source (`Ru = Rp = 0`), so the energy equation is
//! unchanged. After each energy solve `rad.correct()` runs `viewFactor::calculate()` (on the first
//! call and every `solverFreq`-th time index), which reads the stored wall temperature of every
//! `viewFactorWall` patch and writes the radiative wall flux `qr` of those patches. The coupled
//! interface condition then reads `qr` (lagged by one `correct()`), exactly as with P1.
//!
//! | upstream | here |
//! |---|---|
//! | `viewFactor::calculate`, direct branch | [`outram_foam_basic_lib::view_factor_radiation::ViewFactorModel::solve`] (gh:#1529) |
//! | `F`, `globalFaceFaces` from `viewFactorsGen` | [`ViewFactorCoupling::generate`] runs the Rust port `view_factor_generation::generate_view_factors` (gh:#1540) on the region's mesh |
//! | `greyDiffusiveRadiationViewFactor` `qro` | the `qro` argument, per patch |
//! | `boundaryRadiationProperties` emissivity | the `emissivity` argument, per patch |
//! | `qr` field read by the coupled BC | [`ViewFactorCoupling::qr`], one field per MESH patch (zero on patches that do not radiate) |
//!
//! The wall temperature `viewFactor` reads is the stored `T` boundary value; on a mixed (coupled)
//! patch that is the value the condition gave at assembly, as for P1 (gh:#1531).
//!
//! ## Not here
//!
//! Agglomeration (`finalAgglom`), 2-D meshes (ESI's `viewFactorsGen` has a separate 2-D branch,
//! not ported), `smoothing`, `useSolarLoad`, the iterative solver, several bands and parallel runs.
//! Every `empty` patch must be excluded from the radiating patches: the enclosure is the set of
//! patches given, so a region with empty patches gives an open (and upstream-2-D) enclosure.

use crate::error::AppBuilderError;
use outram_foam_basic_lib::io::poly_mesh::PolyMesh;
use outram_foam_basic_lib::prelude::*;
use outram_foam_basic_lib::view_factor_generation::{
    generate_view_factors, ViewFactorGenInput, ViewFactorGenSettings,
};
use outram_foam_basic_lib::view_factor_radiation::{
    GreyDiffusiveViewFactorPatch, ViewFactorModel, ViewFactorPatch,
};

fn case_error(msg: String) -> AppBuilderError {
    AppBuilderError::Case {
        case: std::path::PathBuf::from("energy view-factor radiation"),
        msg,
    }
}

/// The `viewFactor` radiation state and settings of one fluid energy equation.
#[derive(Debug, Clone)]
pub struct ViewFactorCoupling {
    model: ViewFactorModel,
    /// Mesh patch index of each radiating patch, in the model's patch order (ESI: the
    /// `viewFactorWall` group in patch-index order).
    pub patches: Vec<usize>,
    emissivity: Vec<Vec<f64>>,
    bcs: Vec<GreyDiffusiveViewFactorPatch>,
    /// Solve every `solver_freq`-th time step (upstream `solverFreq`, default 1); the first call
    /// always solves.
    pub solver_freq: usize,
    /// The radiative wall flux `qr` \[W/m2\] of the last `correct()`, one field per mesh patch
    /// (zero where the patch does not radiate; upstream's initial `0/qr` value is zero here too).
    pub qr: Vec<Field<f64>>,
    time_index: usize,
    first_iter: bool,
}

impl ViewFactorCoupling {
    /// A coupling from a prebuilt model (`F` already assembled), for the radiating mesh patches
    /// `patches`, with a constant emissivity and `qro` per radiating patch.
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] if a patch index is out of range, the counts disagree, or the
    /// model's patch sizes differ from the mesh's.
    pub fn new(
        model: ViewFactorModel,
        t: &VolScalarField,
        patches: Vec<usize>,
        emissivity: &[f64],
        qro: &[f64],
    ) -> Result<Self, AppBuilderError> {
        let np = t.mesh.patches.len();
        if patches.iter().any(|&p| p >= np) || emissivity.len() != patches.len() || qro.len() != patches.len() {
            return Err(case_error(format!(
                "{} radiating patches, {} emissivities, {} qro values, mesh has {np} patches",
                patches.len(),
                emissivity.len(),
                qro.len()
            )));
        }
        let size = |i: usize| t.mesh.patches[patches[i]].size;
        let emis = (0..patches.len()).map(|i| vec![emissivity[i]; size(i)]).collect();
        let bcs = (0..patches.len()).map(|i| GreyDiffusiveViewFactorPatch::new(vec![qro[i]; size(i)])).collect();
        let qr = t.mesh.patches.iter().map(|p| Field::new(vec![0.0; p.size])).collect();
        Ok(Self {
            model,
            patches,
            emissivity: emis,
            bcs,
            solver_freq: 1,
            qr,
            time_index: 0,
            first_iter: true,
        })
    }

    /// Generate `F` with the Rust `viewFactorsGen` port from the region's `polyMesh` and build the
    /// coupling for the named wall patches (in the order given, which must be the mesh's patch order
    /// as ESI's `viewFactorWall` group is), un-agglomerated, direct solver, variable emissivity.
    ///
    /// `t` is the region's temperature field (its mesh must be the one `poly` was converted to).
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] if a named patch is missing, the generator refuses the mesh, or
    /// the sizes disagree.
    pub fn generate(
        poly: &PolyMesh,
        t: &VolScalarField,
        patch_names: &[&str],
        emissivity: &[f64],
        qro: &[f64],
    ) -> Result<Self, AppBuilderError> {
        let mut indices = Vec::new();
        let mut patch_faces = Vec::new();
        for name in patch_names {
            let mi = t
                .mesh
                .patches
                .iter()
                .position(|p| p.name == *name)
                .ok_or_else(|| case_error(format!("no patch {name}")))?;
            let pp = poly
                .patches
                .iter()
                .find(|p| p.name == *name)
                .ok_or_else(|| case_error(format!("no polyMesh patch {name}")))?;
            indices.push(mi);
            patch_faces.push((pp.start..pp.start + pp.size).map(|f| poly.faces[f].verts.clone()).collect());
        }
        let gen = generate_view_factors(
            &ViewFactorGenInput { points: poly.points.clone(), patch_faces },
            ViewFactorGenSettings::default(),
        )
        .map_err(case_error)?;
        let vp: Vec<ViewFactorPatch> = indices
            .iter()
            .zip(patch_names)
            .map(|(&mi, name)| {
                let p = &t.mesh.patches[mi];
                let areas: Vec<f64> = (p.start..p.start + p.size).map(|f| t.mesh.face_areas[f]).collect();
                ViewFactorPatch { name: (*name).to_string(), agglom: (0..areas.len()).collect(), fine_areas: areas }
            })
            .collect();
        let model = ViewFactorModel::new(vp, &gen.global_face_faces, &gen.f, false, false)
            .map_err(|e| case_error(e.to_string()))?;
        Self::new(model, t, indices, emissivity, qro)
    }

    /// Count a new time step (upstream's `runTime.timeIndex()`), which `solver_freq` reads.
    pub fn begin_time_step(&mut self) {
        self.time_index += 1;
    }

    /// The time steps counted so far.
    #[must_use]
    pub fn time_index(&self) -> usize {
        self.time_index
    }

    /// `rad.correct()`: `viewFactor::calculate()` from the boundary temperatures of `t` if this is
    /// the first call or the time index is a multiple of `solver_freq`; returns whether it solved.
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] if the model refuses the inputs (a shape error).
    pub fn correct(&mut self, t: &VolScalarField) -> Result<bool, AppBuilderError> {
        let due = self.first_iter || self.solver_freq == 0 || self.time_index % self.solver_freq == 0;
        if due {
            let tb: Vec<Vec<f64>> = self.patches.iter().map(|&pi| t.boundary[pi].values.as_slice().to_vec()).collect();
            self.model
                .solve(&tb, &self.emissivity, &mut self.bcs)
                .map_err(|e| case_error(e.to_string()))?;
            for (k, &pi) in self.patches.iter().enumerate() {
                self.qr[pi] = Field::new(self.bcs[k].value.clone());
            }
            self.first_iter = false;
        }
        Ok(due)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solvers::energy::{EnergyEquation, IncompressibleThermo};
    use outram_foam_basic_lib::primitives::Vector3;
    use outram_foam_basic_lib::io::poly_mesh::{MeshFace, PolyMesh};
    use outram_foam_basic_lib::mesh::{BoundaryPatch, PatchKind};

    /// A single hex cell of side 1 m: six one-face wall patches (a closed convex enclosure).
    fn unit_cell() -> PolyMesh {
        let p = |x: f64, y: f64, z: f64| Vector3::new(x, y, z);
        let points = vec![
            p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.),
            p(0., 0., 1.), p(1., 0., 1.), p(1., 1., 1.), p(0., 1., 1.),
        ];
        let loops: [[usize; 4]; 6] = [[0, 4, 7, 3], [1, 2, 6, 5], [0, 1, 5, 4], [3, 7, 6, 2], [0, 3, 2, 1], [4, 5, 6, 7]];
        let faces = loops.iter().map(|l| MeshFace { verts: l.to_vec(), owner: 0, neighbour: None }).collect();
        let names = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];
        let patches = names.iter().enumerate().map(|(i, n)| BoundaryPatch::new((*n).to_string(), i, 1, PatchKind::Wall)).collect();
        PolyMesh { points, faces, n_internal_faces: 0, n_cells: 1, patches }
    }

    fn setup() -> (PolyMesh, EnergyEquation) {
        let pm = unit_cell();
        let mesh = std::sync::Arc::new(pm.to_fv_mesh().expect("mesh"));
        (pm, EnergyEquation::uniform(mesh, IncompressibleThermo::si(1000.0, 4000.0, 0.6), 300.0))
    }

    const NAMES: [&str; 6] = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];

    /// Off by default; a gen/shape error is reported; `solver_freq` follows the time index; and with all
    /// walls black at one temperature the net flux vanishes (isothermal enclosure: qr ~ 0 to the F row-sum
    /// error, which is ~1 % on a 1-face-per-wall cube).
    #[test]
    fn view_factor_is_off_by_default_and_isothermal_enclosure_exchanges_nothing() {
        let (pm, e) = setup();
        assert!(e.view_factor.is_none());
        assert!(ViewFactorCoupling::generate(&pm, &e.t, &["nope"], &[1.0], &[0.0]).is_err());
        assert!(ViewFactorCoupling::generate(&pm, &e.t, &NAMES, &[1.0; 5], &[0.0; 6]).is_err());
        let mut v = ViewFactorCoupling::generate(&pm, &e.t, &NAMES, &[1.0; 6], &[0.0; 6]).expect("coupling");
        v.solver_freq = 3;
        let mut due = Vec::new();
        for _ in 0..4 {
            v.begin_time_step();
            due.push(v.correct(&e.t).expect("correct"));
        }
        assert_eq!(due, vec![true, false, true, false]);
        // sigma T^4 at 300 K is ~459 W/m2; the F row-sum error of a coarse 1-face cube is below ~2 %.
        for q in &v.qr {
            assert!(q.as_slice()[0].abs() < 0.03 * 459.0, "{}", q.as_slice()[0]);
        }
        assert_eq!(v.time_index(), 4);
    }
}
