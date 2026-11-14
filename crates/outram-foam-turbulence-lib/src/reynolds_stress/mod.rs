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

//! Reynolds-stress transport models, ported from OpenFOAM-v2506 (ESI)
//! `ReynoldsStress.C`, `LRR.C` and `SSG.C` (gh:#1544, under #1336).
//!
//! This module holds what [`lrr::Lrr`] and [`ssg::Ssg`] share: the production tensor
//! `P = -twoSymm(R & gradU)`, `G = 0.5 |tr P|`, the wall-cell rescaling of `P`, the
//! diffusivities `(C k/epsilon) R + I nu`, the wall-face Reynolds stress written by
//! `correctWallShearStress`, and the output helpers.
//!
//! **Status: AI draft, terms only.** The `source_terms` functions are compared per cell with
//! the real ESI models on a frozen duct (`upstream_turb_gh1362/README.md`, slice 10). There is
//! no `correct()` (the `epsilon` and `R` transport solves, `boundNormalStress`, `bound`) and no
//! [`crate::traits::TurbulenceModel`] impl. The upstream gradient `grad(U)`, the meshWave wall
//! distance `y` and wall normal `n` are INPUTS ([`ReynoldsStressFrozen`]): the port's own wall
//! distance is not meshWave's (slice 2) and the Gauss gradient orderings are gh:#1489.
//! Not ported: `fvOptions`, `couplingFactor` (default 0), `divDevRhoReff`.

pub mod lrr;
pub mod ssg;

use crate::source_terms::{umin, SourceTerms};
use outram_foam_basic_lib::mesh::fv_mesh::{FvMesh, PatchKind};
use outram_foam_basic_lib::primitives::{SphericalTensor, SymmTensor, Tensor, Vector3};

/// Upstream `SMALL` for double precision.
pub const SMALL: f64 = 1e-15;

/// Frozen inputs a Reynolds-stress model needs besides its own fields.
#[derive(Debug, Clone, Default)]
pub struct ReynoldsStressFrozen {
    /// Cell-centred `grad(U)` as the real code forms it (`T_ij = d_i U_j`).
    pub grad_u: Vec<Tensor>,
    /// Wall distance `y` per cell [m] (upstream: `wallDist::New(mesh).y()`, meshWave).
    pub wall_distance: Vec<f64>,
    /// Unit vector to the nearest wall per cell (upstream: `wallDist::New(mesh).n()`).
    pub wall_normal: Vec<Vector3>,
}

/// Upstream `dev(SymmTensor) = st - sph(st)` with `sph = (1.0/3.0)*tr(st)`.
///
/// `SymmTensor::dev` in basic-lib uses `tr/3.0`, which rounds differently; this is the
/// upstream form (`SymmTensorI.H`).
pub fn dev_symm_tensor(st: SymmTensor) -> SymmTensor {
    st - SphericalTensor::new((1.0 / 3.0) * st.tr())
}

/// Production `P = -twoSymm(R & gradU)` for one cell (`LRR.C`, `SSG.C`).
pub fn production(r: SymmTensor, grad_u: &Tensor) -> SymmTensor {
    -((r * *grad_u).two_symm())
}

/// `G = 0.5*mag(tr(P))` for one cell.
pub fn gen_g(p: SymmTensor) -> f64 {
    0.5 * p.tr().abs()
}

/// The owner cell of every wall-patch face, in patch order then face order (the order of
/// upstream's `forAll(patches, patchi) ... forAll(curPatch, facei)` loop). A cell with several
/// wall faces appears several times.
pub fn wall_face_cells(mesh: &FvMesh) -> Vec<usize> {
    let mut out = Vec::new();
    for p in &mesh.patches {
        if matches!(p.kind, PatchKind::Wall) {
            for f in p.start..p.start + p.size {
                out.push(mesh.owner[f]);
            }
        }
    }
    out
}

/// Upstream's rescaling of `P` in wall cells so that its trace is consistent with the
/// wall-function generation: `P[celli] *= min(G[celli]/(0.5*mag(tr(P[celli])) + SMALL), 1)`,
/// applied face by face (so cells with two or three wall faces are rescaled two or three times).
pub fn wall_correct_production(p: &mut [SymmTensor], g: &[f64], wall_cells: &[usize]) {
    for &c in wall_cells {
        let f = umin(g[c] / (0.5 * p[c].tr().abs() + SMALL), 1.0);
        p[c] = p[c] * f;
    }
}

/// Diffusivity of `epsilon` and `R`: `(C*(k/epsilon))*R + I*nu`
/// (`DepsilonEff` with `C = Ceps`, `DREff` with `C = Cs`).
pub fn effective_diffusivity(c: f64, k: f64, eps: f64, r: SymmTensor, nu: f64) -> SymmTensor {
    ((c * (k / eps)) * r) + SphericalTensor::new(nu)
}

/// The wall-face Reynolds stress `correctWallShearStress` writes after the solve:
/// `Rw = -nutw*2*devSymm(n (x) snGradU)` with `n = Sf/magSf` (`ReynoldsStress.C`).
pub fn wall_face_stress(nutw: f64, face_area: Vector3, sn_grad_u: Vector3) -> SymmTensor {
    let mag_sf = face_area.mag();
    let n = Vector3::new(face_area.x / mag_sf, face_area.y / mag_sf, face_area.z / mag_sf);
    let grad_uw = n * sn_grad_u;
    ((-nutw) * 2.0) * grad_uw.dev_symm()
}

/// Append the six components of a per-cell symmetric tensor as `<name>_xx .. <name>_zz`.
pub fn push_symm(out: &mut SourceTerms, name: &str, v: &[SymmTensor]) {
    let comps: [(&str, fn(&SymmTensor) -> f64); 6] = [
        ("xx", |t| t.xx),
        ("xy", |t| t.xy),
        ("xz", |t| t.xz),
        ("yy", |t| t.yy),
        ("yz", |t| t.yz),
        ("zz", |t| t.zz),
    ];
    for (c, get) in comps {
        out.push(&format!("{name}_{c}"), v.iter().map(get).collect());
    }
}

/// Append the nine components of a per-cell tensor as `<name>_xx .. <name>_zz`.
pub fn push_tensor(out: &mut SourceTerms, name: &str, v: &[Tensor]) {
    let comps: [(&str, fn(&Tensor) -> f64); 9] = [
        ("xx", |t| t.xx),
        ("xy", |t| t.xy),
        ("xz", |t| t.xz),
        ("yx", |t| t.yx),
        ("yy", |t| t.yy),
        ("yz", |t| t.yz),
        ("zx", |t| t.zx),
        ("zy", |t| t.zy),
        ("zz", |t| t.zz),
    ];
    for (c, get) in comps {
        out.push(&format!("{name}_{c}"), v.iter().map(get).collect());
    }
}
