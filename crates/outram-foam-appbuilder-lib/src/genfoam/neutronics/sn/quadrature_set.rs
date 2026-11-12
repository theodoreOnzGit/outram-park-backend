// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da0cc55
//   Upstream source: src/classes/neutronics/SN/include/
//                    {readQuadratureSet,calcLegendreMatrices,legendrePoly,
//                     solveNeutronicsSN,fluxEqSN,calcSourceSN,
//                     calcThisIntraSource,updateScalarFluxes,
//                     updateThisScalarFlux,normFluxesSN}.H/.C,
//                    SN/SNNeutronics.C (facePhis_, maxDeltaT), and
//                    src/classes/neutronics/include/{calcKeff,precEq}.H
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


//! The `quadratureSet` of a GeN-Foam S_N case, read as upstream reads it
//! (gh:#1474, gh:#1361).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! [`AngularQuadrature::level_symmetric`](super::AngularQuadrature::level_symmetric)
//! bakes the standard S2, S4, S6 sets. GeN-Foam instead reads the directions
//! and weights of ONE octant from `constant/<region>/quadratureSet`
//! (`discreteDirectionsOct`, `directionUxOct`, `directionUyOct`,
//! `directionUzOct`, `directionWeightsOct`) and replicates them over the eight
//! sign octants in a fixed order (`readQuadratureSet.H`); the weights are then
//! normalised to sum to 1 (not `4 pi`) and every direction to unit length. This
//! type is exactly that, so a case's own set (the Godiva tutorial ships a
//! 4-direction-per-octant set that is not level-symmetric S4) is used as it is.
//!
//! It also builds `legendreMatrices_` (`calcLegendreMatrices.H`): for each
//! moment `L`, `P_L(Omega_i . Omega_j)` between every pair of directions.

use std::path::Path;
use std::sync::Arc;

use outram_foam_basic_lib::fields::boundary::bc::{BoundaryCondition, PatchField};
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::surface_field::SurfaceScalarField;
use outram_foam_basic_lib::io::dict::{FoamDict, FoamEntry, FoamFile, FoamValue};
use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;

use crate::error::AppBuilderError;

/// The octant sign pattern of `readQuadratureSet.H`, in upstream's order.
const OCTANTS: [(f64, f64, f64); 8] = [
    (1.00, 1.00, 1.00),
    (-1.0, 1.00, 1.00),
    (-1.0, -1.0, 1.00),
    (1.00, -1.0, 1.00),
    (1.00, 1.00, -1.0),
    (-1.0, 1.00, -1.0),
    (-1.0, -1.0, -1.0),
    (1.00, -1.0, -1.0),
];

/// A discrete-ordinates direction set in GeN-Foam's convention: unit versors
/// and weights summing to 1.
#[derive(Debug, Clone, PartialEq)]
pub struct QuadratureSet {
    versors: Vec<Vector3>,
    weights: Vec<f64>,
}

fn parse_err(file: &str, msg: impl Into<String>) -> AppBuilderError {
    AppBuilderError::Parse {
        file: file.to_string(),
        line: 0,
        msg: msg.into(),
    }
}

fn octant_list(d: &FoamDict, key: &str, n: usize, file: &str) -> Result<Vec<f64>, AppBuilderError> {
    let entry = d
        .get(key)
        .ok_or_else(|| parse_err(file, format!("missing `{key}`")))?;
    let list = match entry {
        FoamEntry::List(l) => l,
        FoamEntry::Tokens(t) => match t.last() {
            Some(FoamValue::List(l)) => l,
            _ => return Err(parse_err(file, format!("`{key}` does not end in a `( … )` list"))),
        },
        _ => return Err(parse_err(file, format!("`{key}` is not a list"))),
    };
    let mut out = Vec::with_capacity(list.len());
    for v in list {
        match v {
            FoamValue::Scalar(x) => out.push(*x),
            _ => return Err(parse_err(file, format!("`{key}` holds a non-number"))),
        }
    }
    if out.len() != n {
        return Err(parse_err(
            file,
            format!("`{key}` has {} values, `discreteDirectionsOct` is {n}", out.len()),
        ));
    }
    Ok(out)
}

impl QuadratureSet {
    /// Read `constant/<region>/quadratureSet`.
    ///
    /// # Errors
    ///
    /// [`AppBuilderError::Parse`] if the file cannot be parsed or a list is
    /// missing or has a length other than `discreteDirectionsOct`.
    pub fn read(path: &Path) -> Result<Self, AppBuilderError> {
        let file = FoamFile::read(path).map_err(|e| parse_err(&path.display().to_string(), e.to_string()))?;
        let name = path.display().to_string();
        let d = &file.dict;
        // `quadratureSet_.lookupOrDefault<int>("discreteDirectionsOct", 1)`
        let n = match d.get("discreteDirectionsOct") {
            None => 1,
            Some(FoamEntry::Scalar(x)) if *x >= 1.0 && x.fract() == 0.0 => *x as usize,
            Some(_) => return Err(parse_err(&name, "`discreteDirectionsOct` must be a whole number >= 1")),
        };
        let ux = octant_list(d, "directionUxOct", n, &name)?;
        let uy = octant_list(d, "directionUyOct", n, &name)?;
        let uz = octant_list(d, "directionUzOct", n, &name)?;
        let w = octant_list(d, "directionWeightsOct", n, &name)?;
        Ok(Self::from_octant(&ux, &uy, &uz, &w))
    }

    /// Build the full set from one octant's direction cosines and weights:
    /// replicate over the eight octants (upstream's order), then normalise the
    /// weights to sum to 1 (`directionWeights_ *= 1/total`) and each versor to
    /// unit length (`*= 1/mag`), as `readQuadratureSet.H` does.
    ///
    /// # Panics
    ///
    /// If the four slices differ in length.
    #[must_use]
    pub fn from_octant(ux: &[f64], uy: &[f64], uz: &[f64], w: &[f64]) -> Self {
        let n = ux.len();
        assert!(uy.len() == n && uz.len() == n && w.len() == n, "octant lists differ in length");
        let mut versors = vec![Vector3::ZERO; 8 * n];
        let mut weights = vec![0.0; 8 * n];
        for (oct, (sx, sy, sz)) in OCTANTS.iter().enumerate() {
            for i in 0..n {
                versors[i + oct * n] = Vector3 { x: ux[i] * sx, y: uy[i] * sy, z: uz[i] * sz };
                weights[i + oct * n] = w[i];
            }
        }
        let mut total = 0.0;
        for wi in &weights {
            total += wi;
        }
        let inv_total = 1.0 / total;
        for wi in weights.iter_mut() {
            *wi *= inv_total;
        }
        for v in versors.iter_mut() {
            let mag = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
            let inv = 1.0 / mag;
            v.x *= inv;
            v.y *= inv;
            v.z *= inv;
        }
        Self { versors, weights }
    }

    /// Number of discrete directions (`8 discreteDirectionsOct`).
    #[must_use]
    pub fn direction_count(&self) -> usize {
        self.versors.len()
    }

    /// The unit direction vectors.
    #[must_use]
    pub fn versors(&self) -> &[Vector3] {
        &self.versors
    }

    /// The weights, summing to 1.
    #[must_use]
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    /// `legendreMatrices_[L][i][j] = P_L(Omega_i . Omega_j)` for `L` in
    /// `0..moments` (`calcLegendreMatrices.H`).
    #[must_use]
    pub fn legendre_matrices(&self, moments: usize) -> Vec<Vec<Vec<f64>>> {
        let n = self.versors.len();
        let mut mu = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                let (a, b) = (self.versors[i], self.versors[j]);
                mu[i][j] = a.x * b.x + a.y * b.y + a.z * b.z;
            }
        }
        (0..moments)
            .map(|l| {
                (0..n)
                    .map(|i| (0..n).map(|j| legendre_poly(l, mu[i][j])).collect())
                    .collect()
            })
            .collect()
    }

    /// `facePhis_[direction] = directionVersors_[direction] & mesh.Sf()`, the
    /// face flux of the streaming term (internal and boundary faces).
    #[must_use]
    pub fn face_phi(&self, mesh: &Arc<FvMesh>, direction: usize) -> SurfaceScalarField {
        let omega = self.versors[direction];
        let dot = |s: Vector3| omega.x * s.x + omega.y * s.y + omega.z * s.z;
        let internal: Vec<f64> = (0..mesh.n_internal_faces)
            .map(|f| dot(mesh.face_area_vectors[f]))
            .collect();
        let boundary: Vec<PatchField<f64>> = mesh
            .patches
            .iter()
            .map(|p| PatchField {
                bc: BoundaryCondition::Symmetry,
                values: Field::new(
                    (0..p.size)
                        .map(|fi| dot(mesh.face_area_vectors[p.start + fi]))
                        .collect(),
                ),
            })
            .collect();
        SurfaceScalarField::new(format!("facePhi{direction}"), mesh.clone(), Field::new(internal), boundary)
    }
}

/// `legendrePoly(n, mu)` of `include/legendrePoly.C`, orders 0 to 7 with
/// upstream's expressions; `0.0` for any other order, as upstream returns.
#[must_use]
pub fn legendre_poly(n: usize, mu: f64) -> f64 {
    match n {
        0 => 1.0,
        1 => mu,
        2 => 0.5 * (3.0 * mu * mu - 1.0),
        3 => 0.5 * (5.0 * mu * mu * mu - 3.0 * mu),
        4 => (1.0 / 8.0) * (35.0 * mu * mu * mu * mu - 30.0 * mu * mu + 3.0),
        5 => (1.0 / 8.0) * (63.0 * mu * mu * mu * mu * mu - 70.0 * mu * mu * mu + 15.0 * mu),
        6 => {
            (1.0 / 16.0)
                * (231.0 * mu * mu * mu * mu * mu * mu - 315.0 * mu * mu * mu * mu + 105.0 * mu * mu - 5.0)
        }
        7 => {
            (1.0 / 16.0)
                * (429.0 * mu * mu * mu * mu * mu * mu * mu - 693.0 * mu * mu * mu * mu * mu
                    + 315.0 * mu * mu * mu
                    - 35.0 * mu)
        }
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn godiva() -> QuadratureSet {
        QuadratureSet::from_octant(
            &[2.09769106510711E-01, 9.54983687770317E-01, 2.09769106510711E-01, 5.77350269189625E-01],
            &[2.09769106510711E-01, 2.09769106510711E-01, 9.54983687770317E-01, 5.77350269189625E-01],
            &[9.54983687770317E-01, 2.09769106510711E-01, 2.09769106510711E-01, 5.77350269189625E-01],
            &[3.39836909454130E-01, 3.39836909454103E-01, 3.39836909454103E-01, 5.51285598432481E-01],
        )
    }

    #[test]
    fn replicates_eight_octants_and_normalises() {
        let q = godiva();
        assert_eq!(q.direction_count(), 32);
        let total: f64 = q.weights().iter().sum();
        assert!((total - 1.0).abs() < 1e-14);
        for v in q.versors() {
            assert!(((v.x * v.x + v.y * v.y + v.z * v.z).sqrt() - 1.0).abs() < 1e-14);
        }
        // octant 1 flips x only, octant 6 flips all three
        assert_eq!(q.versors()[4].x, -q.versors()[0].x);
        assert_eq!(q.versors()[4].y, q.versors()[0].y);
        assert_eq!(q.versors()[6 * 4].z, -q.versors()[0].z);
    }

    #[test]
    fn legendre_matrices_are_symmetric_with_unit_diagonal_in_p0_and_p1() {
        let q = godiva();
        let m = q.legendre_matrices(4);
        assert_eq!(m.len(), 4);
        for i in 0..32 {
            assert_eq!(m[0][i][i], 1.0);
            assert!((m[1][i][i] - 1.0).abs() < 1e-14);
            for j in 0..32 {
                assert_eq!(m[2][i][j], m[2][j][i]);
            }
        }
        // P_L(1) = 1 for the orders written out
        for l in 0..8 {
            assert!((legendre_poly(l, 1.0) - 1.0).abs() < 1e-14, "P{l}(1)");
        }
        assert_eq!(legendre_poly(8, 0.3), 0.0);
    }

    #[test]
    fn reads_the_godiva_quadrature_set_dictionary() {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("sn_quadrature_set_test");
        std::fs::create_dir_all(&dir).expect("dir");
        let p = dir.join("quadratureSet");
        std::fs::write(
            &p,
            "FoamFile{version 2.0;format ascii;class dictionary;object quadratureSet;}\n\
             discreteDirectionsOct 1;\n\
             directionUxOct nonuniform List<scalar> 1 ( 5.77350269189625E-01 );\n\
             directionUyOct nonuniform List<scalar> 1 ( 5.77350269189625E-01 );\n\
             directionUzOct nonuniform List<scalar> 1 ( 5.77350269189625E-01 );\n\
             directionWeightsOct nonuniform List<scalar> 1 ( 1.0 );\n",
        )
        .expect("write");
        let q = QuadratureSet::read(&p).expect("read");
        assert_eq!(q.direction_count(), 8);
        assert!((q.weights()[0] - 0.125).abs() < 1e-15);
        assert!(QuadratureSet::read(&dir.join("absent")).is_err());
    }
}
