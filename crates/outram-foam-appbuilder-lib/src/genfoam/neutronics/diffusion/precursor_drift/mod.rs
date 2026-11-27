// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/include/precEq.H
//                    src/classes/neutronics/include/initializeDelayedNeutroSource.H
//                    src/classes/neutronics/diffusion/include/{solveNeutronics,
//                      createNeutronicsFields}.H
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Nordine Kerkar, Konstantin Mikityuk (EPFL)
//   Upstream license: GPL-3.0
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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! **Circulating-fuel precursor drift** — GeN-Foam's `liquidFuel true` branch.
//!
//! # The physics, and why it is not a detail
//!
//! In a molten-salt reactor the fuel *is* the coolant. A delayed-neutron
//! precursor born in the core is carried away by the flow and decays wherever
//! the salt has reached by then — for the shorter-lived groups that is still
//! the core, but for the longer-lived ones it is the heat exchanger, the pump
//! leg, or the downcomer, where its neutron is worth far less. Nothing is lost
//! (the loop is closed and the precursor boundary conditions are zero-gradient,
//! so the *total* delayed source is conserved exactly); what changes is **where**
//! the delayed neutrons appear, and therefore their importance-weighted worth.
//!
//! The effect is bounded above by `beta_total` — 285 pcm for this MSFR
//! evaluation — and measured at **-177.8 pcm** on the `2D_MSFR` tutorial, by
//! running upstream twice with nothing but its `liquidFuel` switch changed.
//!
//! # The equation
//!
//! Upstream's `precEq.H`, with the time derivative dropped because
//! `eigenvalueNeutronics` multiplies it by `(1 - 1) = 0`:
//!
//! ```text
//!   lambda_k alpha C*_k  +  div(phi, C*_k)  -  laplacian(D_C, C*_k)
//!   (laplacian = OpenFOAM's NEGATIVE operator div(D grad); this workspace's
//!    `fvm::laplacian` is the POSITIVE -div(D grad), so the code ADDS it --
//!    CORRECTED 2026-10-18, gh:#952: it used to subtract, i.e. anti-diffusion)
//!     = S_n Beta_k / k_eff
//!
//!   C_k = C*_k alpha                       (precursors per TOTAL volume)
//!   S_delayed = sum_k lambda_k C_k
//! ```
//!
//! `C*` is the concentration per unit *fluid* volume and `C` per unit *total*
//! volume; upstream solves for `C*` because `alpha` is discontinuous across a
//! zone boundary and `C` would be too (its own comment says so, next to the
//! commented-out `C`-form of the same equation).
//!
//! The flux equation then takes the delayed source **as computed**, not as a
//! multiple of the fission source:
//!
//! ```text
//!   q_g = chi_p,g (1 - beta_tot) S_n / k_eff  +  chi_d,g S_delayed  +  S_scatter
//! ```
//!
//! Note the delayed term is **not** divided by `k_eff` — the division already
//! happened in the precursor equation's own source. Dividing twice is the
//! obvious slip and is worth checking against `diffusion/include/fluxEq.H` if
//! this is ever rewritten.
//!
//! ## What it reduces to when the fuel is stationary
//!
//! With `phi = 0` and `D_C = 0` the equation is algebraic,
//! `C_k = alpha C*_k = S_n Beta_k / (k_eff lambda_k)`, so
//! `S_delayed = S_n beta_tot / k_eff` and `q_g` collapses to
//! `chi_eff,g S_n / k_eff` with
//! `chi_eff = chi_p (1 - beta_tot) + chi_d beta_tot`. That is exactly the
//! solid-fuel path in [`super::eigenvalue`], and
//! [`tests::a_zero_velocity_field_reproduces_the_chi_eff_collapse`] asserts it —
//! which is what makes this an extension rather than a second, divergent model.
//!
//! # Relationship to `outram-park-fork-moltres::precursors`
//!
//! That crate has a `PrecursorDrift` solving the same advection-decay balance,
//! and it was read before this was written. It is **not** reused here, for
//! reasons that are interface and lineage rather than taste:
//!
//! | | `outram-park-fork-moltres` | here |
//! |---|---|---|
//! | lineage | Moltres (MOOSE), LGPL-2.1 | GeN-Foam `precEq.H`, GPL-3.0 |
//! | solved variable | `C` (per total volume) | `C* = C/alpha` (per fluid volume) |
//! | fluid fraction | none | per cell; **0.4 to 1.0** on the MSFR tutorial |
//! | `beta`, `lambda` | one scalar per family | per cell, from the zone's parametrised data |
//! | diffusivity | one uniform scalar | a per-cell field, `(alphat + mu/ScNo)/rhoCool` |
//! | linear solve | Gauss-Seidel | BiCGSTAB, matching upstream's `PBiCG` |
//!
//! The `alpha` split is the substantive one: upstream solves for `C*` precisely
//! because `alpha` jumps across a zone boundary and `C` would jump with it (its
//! own comment says so, beside a commented-out `C`-form of the same equation).
//! Bending this port to the other interface would mean either dropping `alpha`
//! or pushing GeN-Foam's zone model into a Moltres-lineage crate that nothing
//! else depends on and that carries no human V&V.
//!
//! **They should not be allowed to drift apart unnoticed.** Whether to converge
//! them — most plausibly by generalising the Moltres one and having both call
//! it — is a maintainer decision, tracked as a bead.
//!
//! # Scope
//!
//! Steady eigenvalue only. ~~The transient form keeps the `ddt` term and is not
//! implemented here~~ **CORRECTED 2026-11-25** (gh:#1361 round 6): the transient
//! form, with the `ddt` term, is
//! [`super::DiffusionNeutronics::upstream_transient_step`]. `DiffusionNeutronics::step`
//! still uses the `chi_eff` collapse.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::sync::Arc;

use outram_foam_basic_lib::prelude::{fvc, fvm, Field, FvMesh, SurfaceScalarField, VolScalarField};

/// The fields a circulating-fuel precursor solve needs, on the neutronics mesh.
///
/// Upstream builds all three in `diffusion/include/solveNeutronics.H` from the
/// thermal-hydraulic solution mapped onto this mesh:
///
/// ```text
///   phi            = fvc::flux(U * alpha)
///   diffusivity    = (alphat + mu/ScNo) / rhoCool
/// ```
///
/// This port takes them as given, because it has no coupled multi-region driver
/// to produce them; hand it either upstream's own written fields (what the
/// verification test does) or whatever a future driver computes.
#[derive(Debug, Clone)]
pub struct PrecursorTransport {
    /// Fluid volume fraction `alpha` `[-]`, per cell.
    ///
    /// `1` where the cell is entirely fuel salt, less where the zone is partly
    /// structure — 0.4 in the MSFR tutorial's heat-exchanger zone. The solved
    /// variable is `C* = C/alpha`, so this scales both the removal term and the
    /// conversion back to per-total-volume concentrations.
    pub alpha: VolScalarField,
    /// Volumetric face flux `phi = fvc::flux(U alpha)` `[m^3/s]`, per face.
    ///
    /// Positive from owner to neighbour, as everywhere else in the FV layer.
    pub phi: SurfaceScalarField,
    /// Precursor diffusivity `[m^2/s]`, per cell.
    ///
    /// Upstream's `(alphat + mu/ScNo)/rhoCool` — turbulent plus laminar mass
    /// diffusion of the precursor species. Interpolated to faces here, matching
    /// the `volScalarField` upstream hands to `fvm::laplacian`.
    pub diffusivity: VolScalarField,
}

impl PrecursorTransport {
    /// Build from per-cell and per-face data.
    ///
    /// # Panics
    ///
    /// Panics if any array is not sized to the mesh.
    #[must_use]
    pub fn new(
        mesh: &Arc<FvMesh>,
        alpha: Vec<f64>,
        phi_internal: Vec<f64>,
        phi_boundary: Vec<Vec<f64>>,
        diffusivity: Vec<f64>,
    ) -> Self {
        assert_eq!(alpha.len(), mesh.n_cells, "alpha is not sized to the mesh");
        assert_eq!(
            diffusivity.len(),
            mesh.n_cells,
            "diffusivity is not sized to the mesh"
        );
        assert_eq!(
            phi_internal.len(),
            mesh.n_internal_faces,
            "phi has the wrong number of internal faces"
        );
        assert_eq!(
            phi_boundary.len(),
            mesh.patches.len(),
            "phi does not carry one value list per patch"
        );
        for (p, vals) in mesh.patches.iter().zip(phi_boundary.iter()) {
            assert_eq!(
                vals.len(),
                p.size,
                "phi patch `{}` is the wrong size",
                p.name
            );
        }

        let mut phi = SurfaceScalarField::uniform("phi", mesh.clone(), 0.0);
        phi.internal = Field::new(phi_internal);
        for (patch, vals) in phi.boundary.iter_mut().zip(phi_boundary) {
            patch.values = Field::new(vals);
        }

        Self {
            alpha: cell_field("alpha", mesh, alpha),
            phi,
            diffusivity: cell_field("diffCoeffPrec", mesh, diffusivity),
        }
    }

    /// The precursor diffusivity interpolated to faces, ready for
    /// `fvm::laplacian`.
    #[must_use]
    pub fn diffusivity_face(&self) -> SurfaceScalarField {
        fvc::interpolate(&self.diffusivity)
    }
}

/// `wedgePolyPatch::calcGeometry` (OpenFOAM-v2506 `wedgePolyPatch.C:46-112`): the face transformation tensor
/// `faceT_ = rotationTensor(centreNormal_, n_)` of wedge patch `patch`, `n_` the average of the patch's face normals
/// (`gAverage`), `centreNormal_` the coordinate-plane normal it is symmetric about. `None` for a patch that is not a
/// wedge or is empty. (gh:#1891: the wedge patch value of a vector field is `transform(faceT_, cell value)`, which is
/// what makes `fvc::flux` on a wedge face a few 1e-14 instead of exactly 0.)
///
/// The face normals are `Sf/|Sf|` of the mesh; upstream's `faceNormals()` come from the patch points, which can differ
/// in the last bit (the test against the instrumented binary says how much).
#[must_use]
pub fn wedge_face_t(mesh: &FvMesh, patch: usize) -> Option<[[f64; 3]; 3]> {
    use outram_foam_basic_lib::prelude::Vector3;
    let p = mesh.patches.get(patch)?;
    if p.kind != outram_foam_basic_lib::mesh::PatchKind::Wedge || p.size == 0 {
        return None;
    }
    let mut sum = Vector3::new(0.0, 0.0, 0.0);
    for fi in 0..p.size {
        let s = mesh.face_area_vectors[p.start + fi];
        sum += s / s.mag();
    }
    let n2 = sum / (p.size as f64);
    let sign = |x: f64| if x >= 0.0 { 1.0 } else { -1.0 };
    let comp = |x: f64| sign(x) * (x.abs().max(0.5) - 0.5);
    let mut n1 = Vector3::new(comp(n2.x), comp(n2.y), comp(n2.z));
    let m = n1.mag();
    if m > 1e-150 {
        n1 = n1 / m;
    }
    // transform.H `rotationTensor(n1, n2)`
    let s = n1.dot(n2);
    let n3 = n1.cross(n2);
    let mag_sqr_n3 = n3.mag_sqr();
    let n1a = [n1.x, n1.y, n1.z];
    let n2a = [n2.x, n2.y, n2.z];
    let n3a = [n3.x, n3.y, n3.z];
    let mut t = [[0.0; 3]; 3];
    if mag_sqr_n3 > 1e-15 {
        for i in 0..3 {
            for j in 0..3 {
                let ident = if i == j { 1.0 } else { 0.0 };
                let a = s * ident;
                let b = ((1.0 - s) * (n3a[i] * n3a[j])) / mag_sqr_n3;
                let c = n2a[i] * n1a[j] - n1a[i] * n2a[j];
                t[i][j] = (a + b) + c;
            }
        }
    } else if s < 0.0 {
        for i in 0..3 {
            for j in 0..3 {
                let ident = if i == j { 1.0 } else { 0.0 };
                t[i][j] = ident + 2.0 * (n1a[i] * n2a[j]);
            }
        }
    } else {
        for i in 0..3 {
            t[i][i] = 1.0;
        }
    }
    Some(t)
}

/// `phi = fvc::flux(U*alpha)` on the neutronics mesh, with the patch values upstream's fields carry
/// (`solveNeutronics.H:48`, gh:#1891): internal faces linear (`w (P - N) + N` then `Sf & .`), every patch the cell
/// value (`zeroGradient`) except the wedge patches, whose `U` is `transform(faceT_, U_cell)`
/// (`wedgeFvPatchField::evaluate`) so their flux is `Sf & (faceT U alpha)`, about 1e-14 and not 0. `diffusivity` is
/// zero until set. The earlier form, which gave wedge faces exactly zero flux, is
/// [`precursor_transport_zero_wedge_flux`].
#[must_use]
pub fn upstream_precursor_transport(
    mesh: &Arc<FvMesh>,
    u: &[outram_foam_basic_lib::prelude::Vector3],
    alpha: &[f64],
) -> PrecursorTransport {
    use outram_foam_basic_lib::prelude::{Field as F, Vector3};
    let n = mesh.n_cells;
    let mut field = outram_foam_basic_lib::prelude::VolVectorField::uniform("U", mesh.clone(), Vector3::new(0.0, 0.0, 0.0));
    field.internal = F::new((0..n).map(|c| u[c] * alpha[c]).collect());
    let phi = fvc::flux(&field);
    let phi_internal: Vec<f64> = (0..mesh.n_internal_faces).map(|f| phi.internal[f]).collect();
    let mut phi_boundary: Vec<Vec<f64>> = Vec::with_capacity(mesh.patches.len());
    for (pi, p) in mesh.patches.iter().enumerate() {
        let rot = wedge_face_t(mesh, pi);
        let mut vals = Vec::with_capacity(p.size);
        for fi in 0..p.size {
            let gf = p.start + fi;
            let c = mesh.owner[gf];
            let sf = mesh.face_area_vectors[gf];
            let v = match rot {
                Some(t) => {
                    let uc = u[c];
                    let tv = Vector3::new(
                        t[0][0] * uc.x + t[0][1] * uc.y + t[0][2] * uc.z,
                        t[1][0] * uc.x + t[1][1] * uc.y + t[1][2] * uc.z,
                        t[2][0] * uc.x + t[2][1] * uc.y + t[2][2] * uc.z,
                    );
                    tv * alpha[c]
                }
                None => u[c] * alpha[c],
            };
            vals.push(sf.dot(v));
        }
        phi_boundary.push(vals);
    }
    PrecursorTransport::new(mesh, alpha.to_vec(), phi_internal, phi_boundary, vec![0.0; n])
}

/// The earlier form of [`upstream_precursor_transport`] (before gh:#1891): the same `fvc::flux(U alpha)` with the
/// wedge patch faces set to exactly 0, as the one-way tests of rounds 6 to 14 and the workflow did.
#[must_use]
pub fn precursor_transport_zero_wedge_flux(
    mesh: &Arc<FvMesh>,
    u: &[outram_foam_basic_lib::prelude::Vector3],
    alpha: &[f64],
) -> PrecursorTransport {
    use outram_foam_basic_lib::prelude::{Field as F, Vector3};
    let n = mesh.n_cells;
    let mut field = outram_foam_basic_lib::prelude::VolVectorField::uniform("U", mesh.clone(), Vector3::new(0.0, 0.0, 0.0));
    field.internal = F::new((0..n).map(|c| u[c] * alpha[c]).collect());
    let phi = fvc::flux(&field);
    let phi_internal: Vec<f64> = (0..mesh.n_internal_faces).map(|f| phi.internal[f]).collect();
    let phi_boundary: Vec<Vec<f64>> = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(pi, p)| {
            (0..p.size)
                .map(|fi| if p.kind == outram_foam_basic_lib::mesh::PatchKind::Wedge { 0.0 } else { phi.boundary[pi].values[fi] })
                .collect()
        })
        .collect();
    PrecursorTransport::new(mesh, alpha.to_vec(), phi_internal, phi_boundary, vec![0.0; n])
}

/// Build the `lambda_k * alpha` removal coefficient field `[1/s]` for one
/// precursor group.
#[must_use]
pub(super) fn removal_coefficient(
    mesh: &Arc<FvMesh>,
    lambda: &VolScalarField,
    alpha: &VolScalarField,
) -> VolScalarField {
    let v: Vec<f64> = lambda
        .internal
        .as_slice()
        .iter()
        .zip(alpha.internal.as_slice())
        .map(|(l, a)| l * a)
        .collect();
    cell_field("lambdaAlpha", mesh, v)
}

/// A zero-gradient `VolScalarField` carrying `values`, sized to `mesh`.
///
/// Zero gradient is upstream's own boundary condition for `precStar` on every
/// real patch of the MSFR case, and the natural one for a field that is data
/// rather than an unknown.
fn cell_field(name: &str, mesh: &Arc<FvMesh>, values: Vec<f64>) -> VolScalarField {
    let mut f = VolScalarField::uniform(name, mesh.clone(), 0.0);
    f.internal = Field::new(values);
    f
}

/// Assemble and solve one precursor group's steady transport equation, returning
/// `C*` (per unit fluid volume).
///
/// `source[c]` is `S_n[c] Beta_k[c] / k_eff`, already divided by `k_eff` exactly
/// as upstream's `neutroSource_/keff_*Beta[precI]` is.
///
/// The matrix is **not** symmetric — `fvm::div` is first-order upwind, matching
/// upstream's `Gauss upwind` for `div(phi_,precStar_)` — so this uses BiCGSTAB,
/// the counterpart of upstream's `PBiCG`.
pub(super) fn solve_group(
    c_star: &VolScalarField,
    lambda_alpha: &VolScalarField,
    transport: &PrecursorTransport,
    diffusivity_face: &SurfaceScalarField,
    source: &VolScalarField,
    settings: crate::genfoam::common::linear_solvers::LinearSolve,
    delta_coeff: fvm::DeltaCoeff,
) -> Vec<f64> {
    // Sign (gh:#952): `fvm::laplacian*` here is the POSITIVE -div(D grad), so it
    // is ADDED; upstream's `- fvm::laplacian` uses OpenFOAM's negative operator.
    let eqn = fvm::sp(lambda_alpha, c_star)
        + fvm::div(&transport.phi, c_star)
        + fvm::laplacian_with_delta(diffusivity_face, c_star, delta_coeff)
        - fvm::su(source, c_star);
    let sol = settings.solve(crate::genfoam::common::linear_solvers::NeutronEquation::Precursor, &eqn, "precStar", c_star);
    sol.internal.as_slice().to_vec()
}

#[cfg(test)]
mod tests;
