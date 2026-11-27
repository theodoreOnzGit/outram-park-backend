// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OpenFOAM (www.openfoam.com), ESI v2506:
//   src/finiteVolume/fvMesh/fvGeometryScheme/basic/basicFvGeometryScheme.C   (weights, nonOrthDeltaCoeffs)
//   src/finiteVolume/fvMesh/fvPatches/constraint/cyclic/cyclicFvPatch.C     (makeWeights, delta)
//   src/finiteVolume/fvMesh/fvPatches/constraint/coupled/coupledFvPatch.C
//   src/finiteVolume/fields/fvPatchFields/constraint/cyclic/cyclicFvPatchField.C (updateInterfaceMatrix)
//   src/finiteVolume/fields/fvPatchFields/constraint/coupled/coupledFvPatchField.H (coeffs)
//   src/finiteVolume/finiteVolume/convectionSchemes/gaussConvectionScheme/gaussConvectionScheme.C (fvmDiv)
//   src/finiteVolume/finiteVolume/laplacianSchemes/gaussLaplacianScheme/gaussLaplacianScheme.C (fvmLaplacianUncorrected)
//   src/finiteVolume/finiteVolume/ddtSchemes/EulerDdtScheme/EulerDdtScheme.C (fvmDdt)
//   src/finiteVolume/fvMatrices/fvMatrix/fvMatrixSolve.C (solveSegregated), fvMatrix.C (addBoundaryDiag/Source)
//   src/OpenFOAM/matrices/lduMatrix/lduMatrix/lduMatrixATmul.C (Amul, Tmul, sumA)
//   src/OpenFOAM/matrices/lduMatrix/solvers/PBiCG/PBiCG.C, lduMatrixSolver.C (normFactor)
//   Copyright (C) 2011-2016 OpenFOAM Foundation, 2016-2023 OpenCFD Ltd; GPL-3.0
//
// This offering is not approved or endorsed by OpenCFD Limited, producer and
// distributor of the OpenFOAM(R) software via www.openfoam.com.

//! # The scalar transport equation of the liquid-fuel precursors on a mesh with CYCLIC patches (gh:#1882, round 48)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! GeN-Foam's liquid-fuel point kinetics solves, per delayed-neutron group,
//! `ddt(alpha, C*) + Sp(lambda alpha, C*) - S + div(phi, C*) - laplacian(D, C*) = 0` on the neutronics
//! mesh. `1D_MSR_pointKinetics` closes its 1-D loop with a CYCLIC `top`/`bottom` patch pair.
//!
//! ## Why a module of its own
//!
//! OpenFOAM keeps a cyclic patch as an `lduInterface`, not as an LDU face: the `internalCoeffs` are
//! added to the diagonal when the matrix is solved, the off-diagonal coupling is the
//! `interfaceBouCoeffs`, applied in `Amul` (and, with the `internalCoeffs`, in `Tmul`), and the
//! `DILU` preconditioner does NOT see it. basic-lib's operators (`fvm::div`, `fvm::laplacian`) store
//! the seam as one more LDU face (`FvMesh::cyclic_couplings`), which differs in exactly those three
//! places, so a `PBiCG`/`DILU` solve of that matrix takes different iterations and returns different
//! bits. This module translates the upstream form: [`TransportMesh`] (face geometry and the
//! patch-by-patch interface data of `cyclicFvPatch`), [`assemble_precursor_matrix`] (the matrix and
//! its patch coefficients with upstream's operand order), [`InterfaceMatrix::solve`]
//! (`fvMatrix::solveSegregated` with `PBiCG`/`DILU` and the interfaces). basic-lib is not modified.
//!
//! ## Scope, refused by name
//!
//! Patch kinds: `cyclic` (interface), wall and plain `patch` (zero-gradient `precStar`), `empty`
//! (no faces). Any other kind (`wedge`, `symmetry`, `cyclicAMI`, `processor`) is refused
//! ([`TransportError::UnsupportedPatch`]). The explicit non-orthogonal correction of
//! `laplacianSchemes ... corrected` (`source -= V div(gamma magSf corrVec & interpolate(grad psi))`)
//! IS translated ([`TransportMesh::laplacian_correction_source`], `gradSchemes Gauss linear`). ~~First
//! form: not translated, on the argument that `nonOrthCorrectionVectors` are ~3e-15 and the term is
//! below the last bit of the source.~~ **CORRECTED 2026-11-27 (round 48)**: on the shipped mesh the
//! correction vectors are 1e-16..6e-15 (not 0), and in cells where the precursor field is large the
//! term is ~1e-7 against a source ulp of 3e-5, which flips the last bit of the source in a few cells
//! (5 of 640 cells at step 1 of the `rev` variant; found by the dumped matrix).
//! `div` is `Gauss linear` only (upwind and others are refused by being absent).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_foam_basic_lib::mesh::PatchKind;
use outram_foam_basic_lib::prelude::{FvMesh, LduMatrix, Vector3};

use crate::genfoam::thermo_mechanics::legacy_fvm::pbicg::{
    dilu_precondition, dilu_precondition_t, dilu_reciprocal_diag, losort, t_mul,
};

/// `SolverPerformance::small_`.
const SMALL: f64 = 1e-20;
/// `VSMALL` for doubles.
const VSMALL: f64 = 1e-300;
/// `ROOTVSMALL` for doubles.
const ROOTVSMALL: f64 = 1e-150;

/// Errors of this module.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TransportError {
    /// A patch kind this module does not translate.
    #[error("patch `{0}` has kind {1}: only cyclic, wall/patch and empty patches are translated (gh:#1882)")]
    UnsupportedPatch(String, String),
    /// A cyclic patch without a resolved partner.
    #[error("cyclic patch `{0}` has no resolved neighbour patch")]
    CyclicPartner(String),
    /// An array of the wrong length.
    #[error("array `{0}` has the wrong length")]
    Length(&'static str),
}

/// How a patch enters the scalar equation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportPatchKind {
    /// `cyclicFvPatchField`: an `lduInterface`.
    Cyclic,
    /// `zeroGradient` (wall, plain patch).
    ZeroGradient,
    /// `emptyFvPatchField`: zero faces.
    Empty,
}

/// One patch of a [`TransportMesh`].
#[derive(Debug, Clone)]
pub struct TransportPatch {
    /// Patch name.
    pub name: String,
    /// Kind.
    pub kind: TransportPatchKind,
    /// `faceCells()`.
    pub face_cells: Vec<usize>,
    /// `neighbFvPatch().faceCells()` for a cyclic patch (empty otherwise).
    pub nbr_cells: Vec<usize>,
    /// `Sf` of the patch faces.
    pub sf: Vec<Vector3>,
    /// `magSf` of the patch faces.
    pub mag_sf: Vec<f64>,
    /// `weights` boundary values (`cyclicFvPatch::makeWeights`; 1 for a non-coupled patch).
    pub weights: Vec<f64>,
    /// `nonOrthDeltaCoeffs` boundary values (from `fvPatch::delta()` / `cyclicFvPatch::delta()`).
    pub non_orth_delta: Vec<f64>,
    /// `nonOrthCorrectionVectors` boundary values (zero for a non-coupled patch).
    pub non_orth_corr: Vec<Vector3>,
}

/// Face geometry of a mesh as the scalar transport equation needs it, with the cyclic patches as
/// interfaces.
#[derive(Debug, Clone)]
pub struct TransportMesh {
    /// Number of cells.
    pub n_cells: usize,
    /// Number of internal faces.
    pub n_internal_faces: usize,
    /// `owner` of the internal faces.
    pub owner: Vec<usize>,
    /// `neighbour` of the internal faces.
    pub neighbour: Vec<usize>,
    /// Cell volumes `V()`.
    pub volume: Vec<f64>,
    /// `Sf` of the internal faces.
    pub sf: Vec<Vector3>,
    /// `magSf` of the internal faces.
    pub mag_sf: Vec<f64>,
    /// `weights` of the internal faces (`basicFvGeometryScheme::weights`).
    pub weights: Vec<f64>,
    /// `nonOrthDeltaCoeffs` of the internal faces.
    pub non_orth_delta: Vec<f64>,
    /// `nonOrthCorrectionVectors` of the internal faces (`unitArea - delta*nonOrthDeltaCoeffs`).
    pub non_orth_corr: Vec<Vector3>,
    /// The patches, in the mesh's order.
    pub patches: Vec<TransportPatch>,
}

fn unit_area(sf: Vector3, mag_sf: f64) -> Vector3 {
    Vector3::new(sf.x / mag_sf, sf.y / mag_sf, sf.z / mag_sf)
}

fn max_scalar(a: f64, b: f64) -> f64 {
    if a > b {
        a
    } else {
        b
    }
}

impl TransportMesh {
    /// Build from the port's `FvMesh` (the geometry arrays of the mesh are the inputs; the weights and
    /// delta coefficients are recomputed here with upstream's formulas). `neighbours` lists
    /// `(cyclic patch, neighbourPatch)` names of the `boundary` file ([`parse_cyclic_neighbours`]); it is
    /// used for a cyclic patch whose `cyclic_partner` the mesh reader did not resolve.
    ///
    /// # Errors
    ///
    /// [`TransportError::UnsupportedPatch`], [`TransportError::CyclicPartner`].
    pub fn from_fv_mesh(mesh: &FvMesh, neighbours: &[(String, String)]) -> Result<Self, TransportError> {
        let n_int = mesh.n_internal_faces;
        let c = &mesh.cell_centres;
        let mut weights = Vec::with_capacity(n_int);
        let mut non_orth = Vec::with_capacity(n_int);
        for f in 0..n_int {
            let sf = mesh.face_area_vectors[f];
            let cf = mesh.face_centres[f];
            let o = mesh.owner[f];
            let n = mesh.neighbour[f];
            // basicFvGeometryScheme::weights: mag in the dot product
            let sfd_own = sf.dot(cf - c[o]).abs();
            let sfd_nei = sf.dot(c[n] - cf).abs();
            if (sfd_own + sfd_nei).abs() > ROOTVSMALL {
                weights.push(sfd_nei / (sfd_own + sfd_nei));
            } else {
                weights.push(0.5);
            }
            // nonOrthDeltaCoeffs
            let delta = c[n] - c[o];
            let ua = unit_area(sf, mesh.face_areas[f]);
            non_orth.push(1.0 / max_scalar(ua.dot(delta), 0.05 * delta.mag()));
        }
        // coupledFvPatch::delta() = Cf - Cn per patch face
        let coupled_delta = |p: usize, i: usize| -> Vector3 {
            let patch = &mesh.patches[p];
            let gf = patch.start + i;
            mesh.face_centres[gf] - c[mesh.owner[gf]]
        };
        let mut patches = Vec::with_capacity(mesh.patches.len());
        for (pi, p) in mesh.patches.iter().enumerate() {
            let kind = match p.kind {
                PatchKind::Cyclic => TransportPatchKind::Cyclic,
                PatchKind::Empty => TransportPatchKind::Empty,
                PatchKind::Wall | PatchKind::Patch => TransportPatchKind::ZeroGradient,
                other => {
                    return Err(TransportError::UnsupportedPatch(p.name.clone(), format!("{other:?}")));
                }
            };
            if kind == TransportPatchKind::Empty {
                patches.push(TransportPatch {
                    name: p.name.clone(),
                    kind,
                    face_cells: vec![],
                    nbr_cells: vec![],
                    sf: vec![],
                    mag_sf: vec![],
                    weights: vec![],
                    non_orth_delta: vec![],
                    non_orth_corr: vec![],
                });
                continue;
            }
            let mut face_cells = Vec::with_capacity(p.size);
            let mut sfs = Vec::with_capacity(p.size);
            let mut mags = Vec::with_capacity(p.size);
            for i in 0..p.size {
                let gf = p.start + i;
                face_cells.push(mesh.owner[gf]);
                sfs.push(mesh.face_area_vectors[gf]);
                mags.push(mesh.face_areas[gf]);
            }
            let mut w = vec![1.0; p.size];
            let mut nod = Vec::with_capacity(p.size);
            let mut corr = vec![Vector3::new(0.0, 0.0, 0.0); p.size];
            let mut nbr_cells = vec![];
            if kind == TransportPatchKind::Cyclic {
                let by_name = neighbours
                    .iter()
                    .find(|(a, _)| *a == p.name)
                    .and_then(|(_, b)| mesh.patches.iter().position(|q| q.name == *b));
                let pj = p
                    .cyclic_partner
                    .or(by_name)
                    .ok_or_else(|| TransportError::CyclicPartner(p.name.clone()))?;
                let q = &mesh.patches[pj];
                if q.size != p.size {
                    return Err(TransportError::CyclicPartner(p.name.clone()));
                }
                for i in 0..p.size {
                    let gfq = q.start + i;
                    nbr_cells.push(mesh.owner[gfq]);
                    // cyclicFvPatch::makeWeights
                    let nf = unit_area(sfs[i], mags[i]);
                    let nf_q = unit_area(mesh.face_area_vectors[gfq], mesh.face_areas[gfq]);
                    let di = nf.dot(coupled_delta(pi, i));
                    let dni = nf_q.dot(coupled_delta(pj, i));
                    w[i] = dni / (di + dni);
                    // cyclicFvPatch::delta (parallel): ddi - dni
                    let delta = coupled_delta(pi, i) - coupled_delta(pj, i);
                    let ua = unit_area(sfs[i], mags[i]);
                    nod.push(1.0 / max_scalar(ua.dot(delta), 0.05 * delta.mag()));
                    // basicFvGeometryScheme::nonOrthCorrectionVectors, coupled patch: unitArea - delta*nonOrthDelta
                    corr[i] = ua - delta * nod[i];
                }
            } else {
                for i in 0..p.size {
                    let delta = coupled_delta(pi, i);
                    let ua = unit_area(sfs[i], mags[i]);
                    nod.push(1.0 / max_scalar(ua.dot(delta), 0.05 * delta.mag()));
                }
            }
            patches.push(TransportPatch {
                name: p.name.clone(),
                kind,
                face_cells,
                nbr_cells,
                sf: sfs,
                mag_sf: mags,
                weights: w,
                non_orth_delta: nod,
                non_orth_corr: corr,
            });
        }
        let mut non_orth_corr = Vec::with_capacity(n_int);
        for f in 0..n_int {
            let ua = unit_area(mesh.face_area_vectors[f], mesh.face_areas[f]);
            let delta = c[mesh.neighbour[f]] - c[mesh.owner[f]];
            non_orth_corr.push(ua - delta * non_orth[f]);
        }
        Ok(Self {
            n_cells: mesh.n_cells,
            n_internal_faces: n_int,
            owner: mesh.owner[..n_int].to_vec(),
            neighbour: mesh.neighbour.clone(),
            volume: mesh.cell_volumes.clone(),
            sf: mesh.face_area_vectors[..n_int].to_vec(),
            mag_sf: mesh.face_areas[..n_int].to_vec(),
            weights,
            non_orth_delta: non_orth,
            non_orth_corr,
            patches,
        })
    }

    /// `mesh.V() * fvc::div(gammaMagSf * correctedSnGrad<scalar>::correction(psi))`: the explicit
    /// non-orthogonal correction of `laplacianSchemes ... corrected` (`gaussLaplacianScheme`, scalar
    /// `gamma`): `correction = nonOrthCorrectionVectors & linear(grad(psi))` with `gradSchemes Gauss
    /// linear` (`gaussGrad::gradf`, `igGrad /= V`), `fvc::div` = `surfaceIntegrate` then `/V`. On a mesh
    /// whose cells are aligned to round-off this is ~1e-16 of the other terms, and is what decides the last
    /// bit of the source in cells where the precursor field is large (README Round 48).
    ///
    /// `gamma_mag_sf` is `gamma_face*magSf` (internal faces and every patch).
    ///
    /// # Errors
    ///
    /// [`TransportError::Length`].
    pub fn laplacian_correction_source(&self, psi: &[f64], gamma_mag_sf: &FaceScalar) -> Result<Vec<f64>, TransportError> {
        let n = self.n_cells;
        let ssf = self.interpolate_linear(psi)?;
        let mut grad = vec![Vector3::new(0.0, 0.0, 0.0); n];
        for f in 0..self.n_internal_faces {
            let sfssf = self.sf[f] * ssf.internal[f];
            grad[self.owner[f]] += sfssf;
            grad[self.neighbour[f]] -= sfssf;
        }
        for (pi, p) in self.patches.iter().enumerate() {
            for i in 0..p.face_cells.len() {
                grad[p.face_cells[i]] += p.sf[i] * ssf.patches[pi][i];
            }
        }
        for c in 0..n {
            grad[c] = grad[c] / self.volume[c];
        }
        let mut ivf = vec![0.0; n];
        for f in 0..self.n_internal_faces {
            let gp = grad[self.owner[f]];
            let gn = grad[self.neighbour[f]];
            let g_face = (gp - gn) * self.weights[f] + gn;
            let corr = self.non_orth_corr[f].dot(g_face);
            let flux_corr = gamma_mag_sf.internal[f] * corr;
            ivf[self.owner[f]] += flux_corr;
            ivf[self.neighbour[f]] -= flux_corr;
        }
        for (pi, p) in self.patches.iter().enumerate() {
            if p.kind != TransportPatchKind::Cyclic {
                // nonOrthCorrectionVectors are Zero on a non-coupled patch: the face flux is 0
                continue;
            }
            for i in 0..p.face_cells.len() {
                let w = p.weights[i];
                let g_face = grad[p.face_cells[i]] * w + grad[p.nbr_cells[i]] * (1.0 - w);
                let corr = p.non_orth_corr[i].dot(g_face);
                ivf[p.face_cells[i]] += gamma_mag_sf.patches[pi][i] * corr;
            }
        }
        Ok((0..n).map(|c| self.volume[c] * (ivf[c] / self.volume[c])).collect())
    }

    /// `fvc::flux(U*alpha)` with `interpolationSchemes default linear`
    /// (`surfaceInterpolationScheme::dotInterpolate`): internal faces
    /// `Sf & (w (vP - vN) + vN)`; a cyclic patch `Sf & (w vP + (1 - w) vNbr)`; a zero-gradient patch
    /// `Sf & v` of the face cell.
    ///
    /// # Errors
    ///
    /// [`TransportError::Length`].
    pub fn flux_of_u_alpha(&self, u: &[Vector3], alpha: &[f64]) -> Result<FaceFlux, TransportError> {
        if u.len() != self.n_cells || alpha.len() != self.n_cells {
            return Err(TransportError::Length("u or alpha"));
        }
        let v: Vec<Vector3> = (0..self.n_cells).map(|c| u[c] * alpha[c]).collect();
        let internal = (0..self.n_internal_faces)
            .map(|f| {
                let vp = v[self.owner[f]];
                let vn = v[self.neighbour[f]];
                let w = self.weights[f];
                self.sf[f].dot((vp - vn) * w + vn)
            })
            .collect();
        let patches = self
            .patches
            .iter()
            .map(|p| match p.kind {
                TransportPatchKind::Empty => vec![],
                TransportPatchKind::ZeroGradient => (0..p.face_cells.len()).map(|i| p.sf[i].dot(v[p.face_cells[i]])).collect(),
                TransportPatchKind::Cyclic => (0..p.face_cells.len())
                    .map(|i| {
                        let w = p.weights[i];
                        let pif = v[p.face_cells[i]];
                        let pnf = v[p.nbr_cells[i]];
                        p.sf[i].dot(pif * w + pnf * (1.0 - w))
                    })
                    .collect(),
            })
            .collect();
        Ok(FaceFlux { internal, patches })
    }

    /// `linear` interpolation of a cell field to the faces (internal and patches): the face values the
    /// laplacian uses for `gamma` (`lambda (vP - vN) + vN`; cyclic `w vP + (1 - w) vNbr`; zero-gradient
    /// patches the cell value).
    ///
    /// # Errors
    ///
    /// [`TransportError::Length`].
    pub fn interpolate_linear(&self, f: &[f64]) -> Result<FaceScalar, TransportError> {
        if f.len() != self.n_cells {
            return Err(TransportError::Length("field"));
        }
        let internal = (0..self.n_internal_faces)
            .map(|k| self.weights[k] * (f[self.owner[k]] - f[self.neighbour[k]]) + f[self.neighbour[k]])
            .collect();
        let patches = self
            .patches
            .iter()
            .map(|p| match p.kind {
                TransportPatchKind::Empty => vec![],
                TransportPatchKind::ZeroGradient => p.face_cells.iter().map(|&c| f[c]).collect(),
                TransportPatchKind::Cyclic => (0..p.face_cells.len())
                    .map(|i| {
                        let w = p.weights[i];
                        w * f[p.face_cells[i]] + (1.0 - w) * f[p.nbr_cells[i]]
                    })
                    .collect(),
            })
            .collect();
        Ok(FaceScalar { internal, patches })
    }
}

/// The `(patch, neighbourPatch)` pairs of the cyclic patches in the text of a `polyMesh/boundary` file.
#[must_use]
pub fn parse_cyclic_neighbours(boundary_text: &str) -> Vec<(String, String)> {
    let cleaned: String = boundary_text.replace('{', " { ").replace('}', " } ").replace(';', " ; ");
    let words: Vec<&str> = cleaned.split_whitespace().collect();
    let mut out = vec![];
    let mut depth = 0usize;
    let mut name = String::new();
    let mut i = 0;
    while i < words.len() {
        match words[i] {
            "{" => {
                depth += 1;
                if depth == 1 && i > 0 {
                    name = words[i - 1].to_string();
                }
            }
            "}" => depth = depth.saturating_sub(1),
            "neighbourPatch" if i + 1 < words.len() => out.push((name.clone(), words[i + 1].to_string())),
            _ => {}
        }
        i += 1;
    }
    out
}

/// A face flux: internal faces and one list per patch.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceFlux {
    /// Internal faces.
    pub internal: Vec<f64>,
    /// Per patch (empty patches: empty list).
    pub patches: Vec<Vec<f64>>,
}

/// A face scalar field: internal faces and one list per patch.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceScalar {
    /// Internal faces.
    pub internal: Vec<f64>,
    /// Per patch.
    pub patches: Vec<Vec<f64>>,
}

/// The inputs of one group's precursor equation (all per cell, owned: no lifetimes).
#[derive(Debug, Clone)]
pub struct PrecursorEquationInputs {
    /// `alpha`.
    pub alpha: Vec<f64>,
    /// `lambda_i` (1/s).
    pub lambda: f64,
    /// The cell field subtracted as `- Su`: `initN power beta / pgt`.
    pub su: Vec<f64>,
    /// `diffCoeffPrec` (cell field).
    pub gamma: Vec<f64>,
    /// The field being solved for (`precPKStar`, current values): the explicit non-orthogonal correction of
    /// the laplacian uses its gradient.
    pub psi: Vec<f64>,
    /// Upstream's form is `true` (the explicit non-orthogonal correction of `laplacianSchemes ... corrected`).
    /// `false` is the named ablation [`PrecursorEquationInputs::without_non_orth_correction`].
    pub non_orth_correction: bool,
    /// `phi = fvc::flux(U alpha)`.
    pub phi: FaceFlux,
    /// `Some((deltaT, alpha.oldTime(), psi.oldTime()))` for `fvm::ddt(alpha, psi)`; `None` for the
    /// equilibrium matrix (no `ddt`).
    pub ddt: Option<(f64, Vec<f64>, Vec<f64>)>,
}

impl PrecursorEquationInputs {
    /// The ablation: leave out the explicit non-orthogonal correction of the laplacian (the first form of
    /// this module; it misses the last bit of the source in cells with a large precursor field).
    #[must_use]
    pub fn without_non_orth_correction(mut self) -> Self {
        self.non_orth_correction = false;
        self
    }
}

/// One cyclic interface of an assembled matrix.
#[derive(Debug, Clone)]
pub struct MatrixInterface {
    /// `faceCells`.
    pub face_cells: Vec<usize>,
    /// The neighbour patch's face cells.
    pub nbr_cells: Vec<usize>,
    /// `boundaryCoeffs` (the `interfaceBouCoeffs`).
    pub bou: Vec<f64>,
    /// `internalCoeffs` (the `interfaceIntCoeffs` of `Tmul`).
    pub int: Vec<f64>,
}

/// The `fvMatrix` of the precursor equation before `solve()`: LDU part without the boundary diagonal,
/// source, and per-patch `internalCoeffs` / `boundaryCoeffs`.
#[derive(Debug, Clone)]
pub struct InterfaceMatrix {
    /// `diag()` (without `addBoundaryDiag`).
    pub diag: Vec<f64>,
    /// `upper()`.
    pub upper: Vec<f64>,
    /// `lower()`.
    pub lower: Vec<f64>,
    /// `source()`.
    pub source: Vec<f64>,
    /// `internalCoeffs()` per patch.
    pub internal_coeffs: Vec<Vec<f64>>,
    /// `boundaryCoeffs()` per patch.
    pub boundary_coeffs: Vec<Vec<f64>>,
}

/// Assemble `[ddt(alpha, psi)] + Sp(lambda alpha, psi) - Su + div(phi, psi) - laplacian(gamma, psi)` with
/// upstream's operand order (`Foam::fvMatrix` operators, see the module header).
///
/// # Errors
///
/// [`TransportError::Length`].
pub fn assemble_precursor_matrix(
    mesh: &TransportMesh,
    inp: &PrecursorEquationInputs,
) -> Result<InterfaceMatrix, TransportError> {
    let n = mesh.n_cells;
    let nf = mesh.n_internal_faces;
    if inp.alpha.len() != n || inp.su.len() != n || inp.gamma.len() != n || inp.phi.internal.len() != nf {
        return Err(TransportError::Length("precursor equation inputs"));
    }
    // (ddt +) Sp : diag, source
    let mut diag = vec![0.0; n];
    let mut source = vec![0.0; n];
    if let Some((dt, alpha_old, psi_old)) = inp.ddt.as_ref() {
        let r_delta_t = 1.0 / dt;
        for c in 0..n {
            diag[c] = (r_delta_t * inp.alpha[c]) * mesh.volume[c];
            source[c] = ((r_delta_t * alpha_old[c]) * psi_old[c]) * mesh.volume[c];
        }
    }
    for c in 0..n {
        // fvm::Sp: diag += V*sp, sp = lambda*alpha ; (ddt + Sp): diag(ddt) += diag(Sp)
        let sp = mesh.volume[c] * (inp.lambda * inp.alpha[c]);
        diag[c] += sp;
    }
    // - Su : source += V*su
    for c in 0..n {
        source[c] += mesh.volume[c] * inp.su[c];
    }
    // div(phi, psi), Gauss linear
    let mut d_lower = vec![0.0; nf];
    let mut d_upper = vec![0.0; nf];
    let mut d_diag = vec![0.0; n];
    for f in 0..nf {
        d_lower[f] = -mesh.weights[f] * inp.phi.internal[f];
        d_upper[f] = d_lower[f] + inp.phi.internal[f];
    }
    for f in 0..nf {
        d_diag[mesh.owner[f]] -= d_lower[f];
        d_diag[mesh.neighbour[f]] -= d_upper[f];
    }
    // laplacian(gamma, psi): upper = deltaCoeffs*gammaMagSf, negSumDiag
    let gamma_face = mesh.interpolate_linear(&inp.gamma)?;
    let mut l_upper = vec![0.0; nf];
    let mut l_diag = vec![0.0; n];
    for f in 0..nf {
        let gamma_mag_sf = gamma_face.internal[f] * mesh.mag_sf[f];
        l_upper[f] = mesh.non_orth_delta[f] * gamma_mag_sf;
    }
    for f in 0..nf {
        l_diag[mesh.owner[f]] -= l_upper[f];
        l_diag[mesh.neighbour[f]] -= l_upper[f];
    }
    // the explicit non-orthogonal correction: lap.source = 0 - V*div(gammaMagSf*correction)
    let mut gamma_mag_sf = FaceScalar {
        internal: (0..nf).map(|f| gamma_face.internal[f] * mesh.mag_sf[f]).collect(),
        patches: vec![],
    };
    for (pi, p) in mesh.patches.iter().enumerate() {
        gamma_mag_sf.patches.push((0..p.face_cells.len()).map(|i| gamma_face.patches[pi][i] * p.mag_sf[i]).collect());
    }
    if inp.non_orth_correction {
        let correction = mesh.laplacian_correction_source(&inp.psi, &gamma_mag_sf)?;
        for c in 0..n {
            let lap_source = 0.0 - correction[c];
            source[c] -= lap_source;
        }
    }
    // (A + div) - laplacian
    let mut upper = vec![0.0; nf];
    let mut lower = vec![0.0; nf];
    for c in 0..n {
        diag[c] += d_diag[c];
        diag[c] -= l_diag[c];
    }
    for f in 0..nf {
        upper[f] = d_upper[f] - l_upper[f];
        lower[f] = d_lower[f] - l_upper[f];
    }
    // patch coefficients
    let mut internal_coeffs = Vec::with_capacity(mesh.patches.len());
    let mut boundary_coeffs = Vec::with_capacity(mesh.patches.len());
    for (pi, p) in mesh.patches.iter().enumerate() {
        let m = p.face_cells.len();
        let mut ic = Vec::with_capacity(m);
        let mut bc = Vec::with_capacity(m);
        for i in 0..m {
            let flux = inp.phi.patches[pi][i];
            match p.kind {
                TransportPatchKind::Cyclic => {
                    let w = p.weights[i];
                    let div_int = flux * w;
                    let div_bou = -flux * (1.0 - w);
                    let g_mag_sf = gamma_face.patches[pi][i] * p.mag_sf[i];
                    let lap_int = g_mag_sf * (-p.non_orth_delta[i]);
                    let lap_bou = (-g_mag_sf) * p.non_orth_delta[i];
                    ic.push(div_int - lap_int);
                    bc.push(div_bou - lap_bou);
                }
                TransportPatchKind::ZeroGradient | TransportPatchKind::Empty => {
                    // valueInternalCoeffs = 1, gradientInternalCoeffs = 0, both boundary coefficients 0
                    ic.push(flux * 1.0);
                    bc.push(0.0);
                }
            }
        }
        internal_coeffs.push(ic);
        boundary_coeffs.push(bc);
    }
    Ok(InterfaceMatrix { diag, upper, lower, source, internal_coeffs, boundary_coeffs })
}

/// `solverPerformance` of one solve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PbicgIfPerformance {
    /// Iterations.
    pub iterations: usize,
    /// Normalised initial residual.
    pub initial_residual: f64,
    /// Normalised final residual.
    pub final_residual: f64,
}

fn g_sum_mag(v: &[f64]) -> f64 {
    let mut s = 0.0;
    for x in v {
        s += x.abs();
    }
    s
}

fn g_sum_prod(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    for i in 0..a.len() {
        s += a[i] * b[i];
    }
    s
}

fn converged(final_residual: f64, initial_residual: f64, tolerance: f64, rel_tol: f64) -> bool {
    final_residual < tolerance || (rel_tol > SMALL && final_residual < rel_tol * initial_residual)
}

/// `lduMatrix::Amul` with the interfaces: the LDU product, then `result[faceCell] -= bou * psi[nbrCell]`
/// (`cyclicFvPatchField::updateInterfaceMatrix`).
fn amul_with_interfaces(a: &LduMatrix, ifs: &[MatrixInterface], psi: &[f64]) -> Vec<f64> {
    let mut y = a.multiply(psi);
    for itf in ifs {
        for i in 0..itf.face_cells.len() {
            y[itf.face_cells[i]] -= itf.bou[i] * psi[itf.nbr_cells[i]];
        }
    }
    y
}

/// `lduMatrix::Tmul` with the interfaces: the transpose product, then
/// `result[faceCell] -= internalCoeffs * psi[nbrCell]`.
fn tmul_with_interfaces(a: &LduMatrix, ifs: &[MatrixInterface], psi: &[f64]) -> Vec<f64> {
    let mut y = t_mul(a, psi);
    for itf in ifs {
        for i in 0..itf.face_cells.len() {
            y[itf.face_cells[i]] -= itf.int[i] * psi[itf.nbr_cells[i]];
        }
    }
    y
}

/// `lduMatrix::solver::normFactor` with `sumA(tmpField, interfaceBouCoeffs, interfaces)`.
fn norm_factor_with_interfaces(a: &LduMatrix, ifs: &[MatrixInterface], psi: &[f64], source: &[f64], apsi: &[f64]) -> f64 {
    let n = psi.len();
    let mut sum_a = a.diag.clone();
    for f in 0..a.n_internal_faces {
        sum_a[a.neighbour[f]] += a.lower[f];
        sum_a[a.owner[f]] += a.upper[f];
    }
    for itf in ifs {
        for i in 0..itf.face_cells.len() {
            sum_a[itf.face_cells[i]] -= itf.bou[i];
        }
    }
    let mut avg = 0.0;
    for x in psi {
        avg += x;
    }
    let avg = avg / n.max(1) as f64;
    let mut s = 0.0;
    for c in 0..n {
        let t = sum_a[c] * avg;
        s += (apsi[c] - t).abs() + (source[c] - t).abs();
    }
    s + SMALL
}

/// `PBiCG::solve` with `preconditioner DILU` on a matrix with interfaces, from OpenFOAM-ESI v2506.
/// `a` carries the diagonal INCLUDING the boundary diagonal; the preconditioner sees only `a`.
pub fn pbicg_dilu_with_interfaces(
    a: &LduMatrix,
    ifs: &[MatrixInterface],
    source: &[f64],
    psi: &mut [f64],
    tolerance: f64,
    rel_tol: f64,
    max_iter: usize,
    min_iter: usize,
) -> PbicgIfPerformance {
    let n = psi.len();
    let wa0 = amul_with_interfaces(a, ifs, psi);
    let mut r_a: Vec<f64> = (0..n).map(|c| source[c] - wa0[c]).collect();
    let norm = norm_factor_with_interfaces(a, ifs, psi, source, &wa0);
    let initial = g_sum_mag(&r_a) / norm;
    let mut perf = PbicgIfPerformance { iterations: 0, initial_residual: initial, final_residual: initial };
    if min_iter == 0 && converged(perf.final_residual, initial, tolerance, rel_tol) {
        return perf;
    }
    let mut p_t = vec![0.0; n];
    let w_t0 = tmul_with_interfaces(a, ifs, psi);
    let mut r_t: Vec<f64> = (0..n).map(|c| source[c] - w_t0[c]).collect();
    let mut w_t = w_t0;
    let mut w_a = wa0;
    let mut p_a = vec![0.0; n];
    let mut w_ar_t = 0.0_f64;
    let r_d = dilu_reciprocal_diag(a);
    let ls = losort(a);
    loop {
        let w_ar_t_old = w_ar_t;
        dilu_precondition(a, &r_d, &r_a, &mut w_a);
        dilu_precondition_t(a, &ls, &r_d, &r_t, &mut w_t);
        w_ar_t = g_sum_prod(&w_a, &r_t);
        if perf.iterations == 0 {
            p_a.copy_from_slice(&w_a);
            p_t.copy_from_slice(&w_t);
        } else {
            let beta = w_ar_t / w_ar_t_old;
            for c in 0..n {
                p_a[c] = w_a[c] + beta * p_a[c];
                p_t[c] = w_t[c] + beta * p_t[c];
            }
        }
        w_a = amul_with_interfaces(a, ifs, &p_a);
        w_t = tmul_with_interfaces(a, ifs, &p_t);
        let w_ap_t = g_sum_prod(&w_a, &p_t);
        if w_ap_t.abs() / norm < VSMALL {
            break;
        }
        let alpha = w_ar_t / w_ap_t;
        for c in 0..n {
            psi[c] += alpha * p_a[c];
            r_a[c] -= alpha * w_a[c];
            r_t[c] -= alpha * w_t[c];
        }
        perf.final_residual = g_sum_mag(&r_a) / norm;
        perf.iterations += 1;
        let go_on = (perf.iterations < max_iter && !converged(perf.final_residual, initial, tolerance, rel_tol))
            || perf.iterations < min_iter;
        if !go_on {
            break;
        }
    }
    perf
}

impl InterfaceMatrix {
    /// The CONTROL (named deviation, not upstream's form): the same matrix with every cyclic interface
    /// stored as one more LDU face, as basic-lib's `FvMesh` does, solved by the same `PBiCG`/`DILU`. The
    /// `DILU` preconditioner then sees the seam and `Amul` has no interface part, so the iterations differ
    /// from the binary's.
    pub fn solve_seam_as_ldu_face(
        &self,
        mesh: &TransportMesh,
        psi: &mut [f64],
        tolerance: f64,
        rel_tol: f64,
        max_iter: usize,
        min_iter: usize,
    ) -> PbicgIfPerformance {
        let mut owner = mesh.owner.clone();
        let mut neighbour = mesh.neighbour.clone();
        let mut lower = self.lower.clone();
        let mut upper = self.upper.clone();
        let mut diag = self.diag.clone();
        for (pi, p) in mesh.patches.iter().enumerate() {
            for (i, &cell) in p.face_cells.iter().enumerate() {
                diag[cell] += self.internal_coeffs[pi][i];
            }
        }
        for (pi, p) in mesh.patches.iter().enumerate() {
            if p.kind != TransportPatchKind::Cyclic {
                continue;
            }
            for i in 0..p.face_cells.len() {
                let (a, b) = (p.face_cells[i], p.nbr_cells[i]);
                // one LDU face per seam pair (the lower patch index of the pair owns it)
                if a < b {
                    owner.push(a);
                    neighbour.push(b);
                    // row a: -bou(this patch)*psi[b]; row b: -bou(partner)*psi[a]
                    let partner_bou = mesh
                        .patches
                        .iter()
                        .enumerate()
                        .find(|(_, q)| q.kind == TransportPatchKind::Cyclic && q.face_cells.get(i) == Some(&b))
                        .map_or(0.0, |(qi, _)| self.boundary_coeffs[qi][i]);
                    upper.push(-self.boundary_coeffs[pi][i]);
                    lower.push(-partner_bou);
                }
            }
        }
        let mut ldu = LduMatrix::new(mesh.n_cells, owner, neighbour);
        ldu.diag = diag;
        ldu.upper = upper;
        ldu.lower = lower;
        let mut source = self.source.clone();
        for (pi, p) in mesh.patches.iter().enumerate() {
            if p.kind != TransportPatchKind::Cyclic {
                for (i, &cell) in p.face_cells.iter().enumerate() {
                    source[cell] += self.boundary_coeffs[pi][i];
                }
            }
        }
        pbicg_dilu_with_interfaces(&ldu, &[], &source, psi, tolerance, rel_tol, max_iter, min_iter)
    }

    /// `fvMatrix::solveSegregated` for a scalar: `addBoundaryDiag` (patch by patch), the boundary source
    /// of the coupled patches `source += bou*psiNbr` corrected by `updateMatrixInterfaces`
    /// (`source -= bou*psiNbr`, NOT an exact cancellation in floating point), then `PBiCG`/`DILU` with
    /// the interfaces. `psi` is the initial guess and receives the solution.
    pub fn solve(
        &self,
        mesh: &TransportMesh,
        psi: &mut [f64],
        tolerance: f64,
        rel_tol: f64,
        max_iter: usize,
        min_iter: usize,
    ) -> PbicgIfPerformance {
        let mut ldu = LduMatrix::new(mesh.n_cells, mesh.owner.clone(), mesh.neighbour.clone());
        ldu.lower.copy_from_slice(&self.lower);
        ldu.upper.copy_from_slice(&self.upper);
        // addBoundaryDiag
        let mut diag = self.diag.clone();
        for (pi, p) in mesh.patches.iter().enumerate() {
            for (i, &cell) in p.face_cells.iter().enumerate() {
                diag[cell] += self.internal_coeffs[pi][i];
            }
        }
        ldu.diag = diag;
        // addBoundarySource (couples = true) adds `bou*psiNbr` to the source of the coupled cells and
        // `updateMatrixInterfaces` subtracts it again. ~~First form: both done in floating point,
        // `(s + x) - x`.~~ **CORRECTED 2026-11-27 (round 48)**: that is NOT what the binary does. Solving
        // the dumped matrices of 16 (step, group) cases once from the same state, the source reaches the
        // solver as exactly `s`: 16 of 16 solves are bit-identical without the add and subtract, 15 of 16
        // with it (the failing one by 1 ulp in cell 0, where `(s + x) - x != s`). The cause in upstream is
        // not identified (the two calls net to zero exactly); only the walls' `boundaryCoeffs`, which are
        // +-0, are added.
        let mut source = self.source.clone();
        for (pi, p) in mesh.patches.iter().enumerate() {
            if p.kind != TransportPatchKind::Cyclic {
                for (i, &cell) in p.face_cells.iter().enumerate() {
                    source[cell] += self.boundary_coeffs[pi][i];
                }
            }
        }
        let mut ifs = Vec::new();
        for (pi, p) in mesh.patches.iter().enumerate() {
            if p.kind == TransportPatchKind::Cyclic {
                ifs.push(MatrixInterface {
                    face_cells: p.face_cells.clone(),
                    nbr_cells: p.nbr_cells.clone(),
                    bou: self.boundary_coeffs[pi].clone(),
                    int: self.internal_coeffs[pi].clone(),
                });
            }
        }
        pbicg_dilu_with_interfaces(&ldu, &ifs, &source, psi, tolerance, rel_tol, max_iter, min_iter)
    }
}
