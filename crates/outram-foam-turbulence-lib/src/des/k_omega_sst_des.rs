// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! `kOmegaSSTDES` (Strelets 2001), ported from OpenFOAM-v2506 (ESI) `kOmegaSSTDES.C` on
//! `kOmegaSSTBase.C` (gh:#1545, under #1377 and #1376).
//!
//! The SST blending functions, `nut`, `gamma`, `beta` and the omega equation are those of
//! [`crate::k_omega_sst::KOmegaSST`] (ESI arithmetic), which this struct REUSES through
//! [`KOmegaSstDes::sst`]. DES changes three things: the k destruction is
//! `epsilonByk = sqrt(k)/dTilda` instead of `betaStar omega`, with
//! `dTilda = min(CDES delta, lRAS)`, `lRAS = sqrt(k)/(betaStar omega)` and
//! `CDES = F1 (CDESkom - CDESkeps) + CDESkeps` (0.82, 0.60); and `GbyNu` is NOT limited
//! (`GbyNu = GbyNu0`), so the omega production is `gamma GbyNu0`.
//!
//! **Status: AI draft, terms only.** [`KOmegaSstDes::source_terms`] is compared per cell
//! against the real ESI model on a frozen duct (README slice 4). No `correct()`, no
//! [`crate::traits::TurbulenceModel`] impl. Not ported: `useSigma true`.

use crate::k_omega_sst::KOmegaSST;
use crate::source_terms::{self, FrozenState, SourceForm, SourceTerms};
use outram_foam_basic_lib::prelude::FvMesh;
use std::sync::Arc;

/// `kOmegaSSTDES` on a frozen field (terms only).
pub struct KOmegaSstDes {
    /// The k-omega SST model whose ESI-form terms this model reuses; set `k`, `omega`,
    /// `nu` on it.
    pub sst: KOmegaSST,
    /// LES width `delta` [m] (`cubeRootVol`).
    pub delta: Vec<f64>,
    /// `CDESkom` (inner, k-omega branch), upstream 0.82.
    pub cdes_kom: f64,
    /// `CDESkeps` (outer, k-epsilon branch), upstream 0.60.
    pub cdes_keps: f64,
    /// `betaStar`, upstream 0.09.
    pub beta_star: f64,
}

impl KOmegaSstDes {
    /// Upstream defaults; `delta = 1.0 cbrt(V)`.
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        use petir::mathf::RealMath;
        let delta = (0..mesh.n_cells)
            .map(|c| 1.0 * mesh.cell_volumes[c].r_cbrt())
            .collect();
        Self {
            sst: KOmegaSST::new(mesh),
            delta,
            cdes_kom: 0.82,
            cdes_keps: 0.60,
            beta_star: 0.09,
        }
    }

    /// Per-cell terms: `delta`, the SST terms `S2`, `CDkOmega`, `F1`, `F23`, `nut`,
    /// `GbyNu0`, `G`, `gamma`, `beta`, `Pk`, `omega_SuSp1`, `omega_Sp`, `omega_SuSp2`,
    /// `k_E`, `k_SuSp`, and the DES terms `CDES`, `lRAS`, `lLES`, `dTilda`, `epsByk`,
    /// `LESRegion`, `GbyNu` (= `GbyNu0`), `omega_E` (= `gamma GbyNu`), `k_Sp` (= `epsByk`).
    /// `state` needs `grad_u`, `grad_k`, `grad_omega`, `div_u`; `wall_distance` and
    /// `cell_length` override `y` and `delta`.
    pub fn source_terms(&self, state: &FrozenState) -> SourceTerms {
        let base = self.sst.source_terms(SourceForm::EsiV2506, state);
        let n = self.sst.mesh.n_cells;
        let k = self.sst.k.internal.as_slice();
        let w = self.sst.omega.internal.as_slice();
        let delta: &[f64] = match &state.cell_length {
            Some(v) => v.as_slice(),
            None => &self.delta,
        };
        let f1 = base.get("F1").unwrap();
        let gamma = base.get("gamma").unwrap();
        let gbn0 = base.get("GbyNu0").unwrap();
        let mut cdes = Vec::with_capacity(n);
        let mut l_ras = Vec::with_capacity(n);
        let mut l_les = Vec::with_capacity(n);
        let mut d_tilda = Vec::with_capacity(n);
        let mut eps_by_k = Vec::with_capacity(n);
        let mut les_region = Vec::with_capacity(n);
        let mut omega_e = Vec::with_capacity(n);
        for c in 0..n {
            let cd = f1[c] * (self.cdes_kom - self.cdes_keps) + self.cdes_keps;
            let lr = k[c].sqrt() / (self.beta_star * w[c]);
            let ll = cd * delta[c];
            let dt = source_terms::umin(ll, lr);
            cdes.push(cd);
            l_ras.push(lr);
            l_les.push(ll);
            d_tilda.push(dt);
            eps_by_k.push(k[c].sqrt() / dt);
            les_region.push(source_terms::neg(dt - lr));
            omega_e.push(gamma[c] * gbn0[c]);
        }
        let mut out = SourceTerms::default();
        out.push("delta", delta.to_vec());
        for name in [
            "S2",
            "CDkOmega",
            "F1",
            "F23",
            "nut",
            "GbyNu0",
            "G",
            "gamma",
            "beta",
        ] {
            out.push(name, base.get(name).unwrap().to_vec());
        }
        out.push("CDES", cdes);
        out.push("lRAS", l_ras);
        out.push("lLES", l_les);
        out.push("dTilda", d_tilda);
        out.push("epsByk", eps_by_k.clone());
        out.push("LESRegion", les_region);
        out.push("GbyNu", gbn0.to_vec());
        out.push("Pk", base.get("Pk").unwrap().to_vec());
        out.push("omega_E", omega_e);
        for name in ["omega_SuSp1", "omega_Sp", "omega_SuSp2", "k_E", "k_SuSp"] {
            out.push(name, base.get(name).unwrap().to_vec());
        }
        out.push("k_Sp", eps_by_k);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::prelude::{
        BoundaryPatch, FvMeshBuilder, PatchKind, Tensor, Vector3,
    };

    fn vx(x: f64) -> Vector3 {
        Vector3::new(x, 0.0, 0.0)
    }

    fn line_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(3)
                .n_internal_faces(2)
                .owner(vec![0, 1, 0, 2])
                .neighbour(vec![1, 2])
                .patches(vec![
                    BoundaryPatch::new("left", 2, 1, PatchKind::Wall),
                    BoundaryPatch::new("right", 3, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![8.0, 8.0, 8.0])
                .cell_centres(vec![vx(0.5), vx(1.5), vx(2.5)])
                .face_area_vectors(vec![vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(1.0), vx(2.0), vx(0.0), vx(3.0)])
                .build()
                .unwrap(),
        )
    }

    /// dTilda is the smaller of the LES and RAS lengths, epsByk = sqrt(k)/dTilda, and the
    /// omega production is unlimited (`omega_E = gamma GbyNu0`).
    #[test]
    fn des_length_is_min_and_production_unlimited() {
        let m = KOmegaSstDes::new(line_mesh());
        let g = Tensor::new(0.0, 2.0, 0.0, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0);
        let st = FrozenState {
            grad_u: vec![g; 3],
            div_u: vec![0.0; 3],
            grad_k: vec![vx(1e-3); 3],
            grad_omega: vec![vx(1e-3); 3],
            wall_distance: Some(vec![0.5, 1.5, 2.5]),
            ..FrozenState::default()
        };
        let t = m.source_terms(&st);
        let (d, l, r) = (
            t.get("dTilda").unwrap(),
            t.get("lLES").unwrap(),
            t.get("lRAS").unwrap(),
        );
        let (e, ga, gb, oe) = (
            t.get("epsByk").unwrap(),
            t.get("gamma").unwrap(),
            t.get("GbyNu0").unwrap(),
            t.get("omega_E").unwrap(),
        );
        for c in 0..3 {
            assert_eq!(d[c], l[c].min(r[c]));
            assert_eq!(e[c], m.sst.k.internal[c].sqrt() / d[c]);
            assert_eq!(oe[c], ga[c] * gb[c]);
            assert!(t.get("CDES").unwrap()[c] >= 0.60 && t.get("CDES").unwrap()[c] <= 0.82);
        }
        assert_eq!(t.get("k_Sp").unwrap(), t.get("epsByk").unwrap());
    }
}
