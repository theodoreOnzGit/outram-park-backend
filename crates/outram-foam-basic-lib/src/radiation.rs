// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), ESI v2506
// Copyright (C) 2011-2017 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//! P1 thermal-radiation model: the incident-radiation `G` equation, the Marshak
//! boundary conditions, constant absorption/emission and the `Ru`/`Rp` energy
//! source (gh:#1379, epic gh:#1376).
//!
//! # What is ported
//!
//! | Upstream (ESI v2506 `src/thermophysicalModels/radiation/`) | Here |
//! |---|---|
//! | `radiationModels/P1/P1.C` `P1::calculate`, `Ru`, `Rp` | [`P1Model::solve`], [`P1Solution::ru`], [`P1Solution::rp`] |
//! | `derivedFvPatchFields/MarshakRadiation` | [`P1PatchModel::Marshak`] |
//! | `derivedFvPatchFields/MarshakRadiationFixedTemperature` | [`P1PatchModel::MarshakFixedTemperature`] |
//! | `submodels/absorptionEmissionModel/constantAbsorptionEmission` | [`ConstantAbsorptionEmission`] |
//! | `scatterModel/noScatter` | `sigma_eff = 0` in [`P1Model`] |
//! | `boundaryRadiationProperties` `lookup` (constant emissivity per patch) | the `emissivity` of the Marshak variants |
//! | `radiationModel::ST(T)`, `radiationModel::Sh(thermo, he)` | [`P1Solution::st_matrix`], [`P1Solution::sh_matrix`] |
//! | `physicoChemical::sigma` from the `etc/controlDict` constants | [`stefan_boltzmann_esi_v2506`] |
//!
//! `viewFactor`, `fvDOM`, `solarLoad`, `opaqueSolid`, wavelength bands, soot,
//! scatter models and per-face emissivity models are NOT ported; the survey of
//! what they would take is in
//! `verification_and_validation/upstream_radiation_gh1379/README.md`.
//!
//! # Model
//!
//! Gray P1 (Modest, *Radiative Heat Transfer*, 3rd ed., ch. 16): with
//! `gamma = 1/(3 a + sigma_eff + a0)` the incident radiation `G` [W/m2] obeys
//!
//! ```text
//!   div(gamma grad G) - a G = -4 e sigma T^4 - E
//! ```
//!
//! and the Marshak wall condition `Ep (4 sigma Tw^4 - G_w) = gamma dG/dn_out`,
//! `Ep = eps / (2 (2 - eps))`, discretised as a `mixed` patch with
//! `valueFraction = 1/(1 + gamma deltaCoeffs/Ep)`, `refValue = 4 sigma Tw^4`,
//! `refGrad = 0`. The energy sources are `Ru = a G - E` and `Rp = 4 e sigma`
//! (net absorbed power `Ru - Rp T^4`).
//!
//! # Sign conventions
//!
//! The matrices follow this crate's `fvm` convention (see
//! `fv_operators::fvm::laplacian`): the diffusion operator has a
//! POSITIVE diagonal. Upstream's matrices carry the opposite sign, so every
//! matrix here is upstream's, negated: `diag`/`source` are upstream's `-diag`/
//! `-source`, bit for bit (negation is exact).
//!
//! # Defaults
//!
//! Nothing in this module is wired into any solver by default; a solver opts
//! in by calling [`P1Model::solve`] and adding [`P1Solution::st_matrix`] or
//! [`P1Solution::sh_matrix`] to its energy equation.
//!
//! Units: SI throughout (`a`, `e` in 1/m, `E` in W/m3, `G`, `qr` in W/m2, T in K).

use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::field::Field;
use crate::fields::vol_field::VolScalarField;
use crate::fv_operators::fvc;
use crate::fv_operators::fvm::{laplacian_with_delta_diag_and_source, DeltaCoeff};
use crate::ldu_matrix::fv_matrix::{FvMatrix, SolverPerformance, SolverSettings};

/// OpenFOAM's `ROOTVSMALL` (1e-150), added to the P1 diffusion denominator so `gamma`
/// stays finite for a vacuum. Re-exported from `primitives::scalar` (one definition).
pub use crate::primitives::scalar::ROOT_VSMALL;

/// `physicoChemical::sigma` [W/m2/K4] exactly as ESI v2506 builds it at start-up
/// from the SI constants in `etc/controlDict` (`k = 1.38065e-23`,
/// `h = 6.62607e-34`, `c = 2.99792e8`, all truncated to six digits):
/// `sqr(pi)/60 * pow4(k) / (pow3(hr) * sqr(c))`, `hr = h/(2 pi)`, in that
/// operation order. It is 5.67040855806e-8, about 1e-5 above CODATA's
/// 5.670374e-8; the radiation here follows upstream, not CODATA (the same
/// choice as `R_UNIVERSAL_ESI_V2506` for the gas constant, gh:#1362).
#[must_use]
pub fn stefan_boltzmann_esi_v2506() -> f64 {
    let pi = std::f64::consts::PI;
    let two_pi = 2.0 * std::f64::consts::PI;
    let (k, h, c) = (1.38065e-23_f64, 6.62607e-34_f64, 2.99792e8_f64);
    let hr = h / two_pi;
    let k2 = k * k;
    let k4 = k2 * k2;
    let hr3 = hr * (hr * hr);
    let c0 = (pi * pi) / 60.0;
    (c0 * k4) / (hr3 * (c * c))
}

/// `constantAbsorptionEmission`: uniform gray absorption, emission coefficient and
/// emission source (ESI keywords `absorptivity`, `emissivity`, `E`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConstantAbsorptionEmission {
    /// Absorption coefficient `a` [1/m].
    pub absorptivity: f64,
    /// Emission coefficient `e` [1/m] (the medium's, not a wall emissivity).
    pub emissivity: f64,
    /// Additional emission source `E` [W/m3].
    pub emission_source: f64,
}

/// What a boundary patch does for the `G` equation.
#[derive(Debug, Clone, PartialEq)]
pub enum P1PatchModel {
    /// `MarshakRadiation`: wall at the boundary temperature of the `T` field,
    /// with the wall emissivity `emissivity` (boundaryRadiationProperties `lookup`).
    Marshak {
        /// Wall emissivity, dimensionless, in (0, 1].
        emissivity: f64,
    },
    /// `MarshakRadiationFixedTemperature`: wall radiating at `t_rad` [K]
    /// regardless of the `T` field.
    MarshakFixedTemperature {
        /// Wall emissivity, dimensionless, in (0, 1].
        emissivity: f64,
        /// Radiation temperature `Trad` [K].
        t_rad: f64,
    },
    /// Insulating / symmetry patch for `G` (`zeroGradient`): no radiative flux.
    ZeroGradient,
}

/// The P1 model: medium properties and one [`P1PatchModel`] per mesh patch.
#[derive(Debug, Clone, PartialEq)]
pub struct P1Model {
    /// Absorption and emission of the medium.
    pub absorption_emission: ConstantAbsorptionEmission,
    /// Effective scattering coefficient `sigma_eff` [1/m]; `0` is `noScatter`.
    pub sigma_eff: f64,
    /// One entry per patch of the mesh, in patch order.
    pub patches: Vec<P1PatchModel>,
}

/// Result of [`P1Model::solve`].
#[derive(Debug, Clone)]
pub struct P1Solution {
    /// Incident radiation `G` [W/m2]; boundary conditions are `MixedField` with the
    /// Marshak coefficients, boundary values evaluated (upstream's `evaluate`).
    pub g: VolScalarField,
    /// Radiative heat flux on each patch face `qr = -gamma snGrad(G)` [W/m2], one
    /// vector per patch (`-0.0` on a `ZeroGradient` patch, as upstream).
    pub qr: Vec<Field<f64>>,
    /// Linear-solver summary.
    pub performance: SolverPerformance,
    /// Absorption coefficient `a` used (per cell, [1/m]).
    pub a: Vec<f64>,
    /// Emission coefficient `e` used (per cell, [1/m]).
    pub e: Vec<f64>,
    /// Emission source `E` used (per cell, [W/m3]).
    pub emission: Vec<f64>,
    /// The temperature field `T` the solution was computed from.
    pub t: VolScalarField,
}

fn pow4(x: f64) -> f64 {
    let s = x * x;
    s * s
}

fn pow3(x: f64) -> f64 {
    x * (x * x)
}

impl P1Model {
    /// A model with `noScatter` (`sigma_eff = 0`).
    #[must_use]
    pub fn new(absorption_emission: ConstantAbsorptionEmission, patches: Vec<P1PatchModel>) -> Self {
        Self { absorption_emission, sigma_eff: 0.0, patches }
    }

    /// Solve `div(gamma grad G) - a G = -4 e sigma T^4 - E` with PCG/DIC from `G = 0`
    /// (`P1::calculate`) and evaluate `qr` on the patches.
    ///
    /// `settings.tolerance = 0` with `max_iter = n` runs exactly `n` iterations,
    /// which is how the differential test reproduces upstream's iteration count.
    ///
    /// # Panics
    /// If `self.patches.len()` differs from the number of mesh patches.
    #[must_use]
    pub fn solve(&self, t: &VolScalarField, settings: SolverSettings) -> P1Solution {
        self.solve_from(t, None, settings)
    }

    /// [`Self::solve`] warm-started from `g_init` (one `G` per cell), as upstream's
    /// `P1::calculate` solves from the current `G_` field (gh:#1531). `None` is the
    /// cold start from `G = 0` of [`Self::solve`], bit for bit.
    ///
    /// # Panics
    /// If `self.patches.len()` differs from the number of mesh patches, or `g_init`
    /// is not one value per cell.
    #[must_use]
    pub fn solve_from(&self, t: &VolScalarField, g_init: Option<&[f64]>, settings: SolverSettings) -> P1Solution {
        let mesh = t.mesh.clone();
        assert_eq!(self.patches.len(), mesh.patches.len(), "one P1PatchModel per mesh patch");
        let n = mesh.n_cells;
        let sigma = stefan_boltzmann_esi_v2506();
        let ae = &self.absorption_emission;

        // Per-cell a, e, E (aDisp + aCont with aDisp = 0, and so on).
        let a: Vec<f64> = vec![0.0 + ae.absorptivity; n];
        let e: Vec<f64> = vec![0.0 + ae.emissivity; n];
        let big_e: Vec<f64> = vec![0.0 + ae.emission_source; n];

        // gamma = 1/(3 a + sigmaEff + a0), cells and patch faces.
        let gamma_of = |a: f64| 1.0 / (((3.0 * a) + self.sigma_eff) + ROOT_VSMALL);
        let gamma_cells: Vec<f64> = a.iter().map(|&a| gamma_of(a)).collect();
        let gamma_boundary: Vec<PatchField<f64>> = mesh
            .patches
            .iter()
            .map(|p| {
                let vals = Field::new(vec![gamma_of(0.0 + ae.absorptivity); p.size]);
                PatchField { bc: BoundaryCondition::FixedField(vals.clone()), values: vals }
            })
            .collect();
        let gamma =
            VolScalarField::new("gammaRad", mesh.clone(), Field::new(gamma_cells), gamma_boundary);
        let gamma_f = fvc::interpolate(&gamma);

        // Marshak coefficients per patch face.
        let mut bcs: Vec<PatchField<f64>> = Vec::with_capacity(mesh.patches.len());
        for (pi, patch) in mesh.patches.iter().enumerate() {
            match &self.patches[pi] {
                P1PatchModel::ZeroGradient => bcs.push(PatchField::zero_gradient(patch.size)),
                model => {
                    let (emissivity, t_rad) = match model {
                        P1PatchModel::Marshak { emissivity } => (*emissivity, None),
                        P1PatchModel::MarshakFixedTemperature { emissivity, t_rad } => {
                            (*emissivity, Some(*t_rad))
                        }
                        P1PatchModel::ZeroGradient => unreachable!(),
                    };
                    let mut vf = Vec::with_capacity(patch.size);
                    let mut rv = Vec::with_capacity(patch.size);
                    for fi in 0..patch.size {
                        let gf = patch.start + fi;
                        let tp = t_rad.unwrap_or(t.boundary[pi].values[fi]);
                        // refValue = 4.0*sigma*pow4(Tp)
                        rv.push((4.0 * sigma) * pow4(tp));
                        // Ep = emissivity/(2.0*(2 - emissivity))
                        let ep = emissivity / (2.0 * (2.0 - emissivity));
                        let dc = DeltaCoeff::Orthogonal.boundary_delta_coeff(&mesh, gf);
                        // valueFraction = 1.0/(1.0 + gamma*deltaCoeffs/Ep)
                        vf.push(1.0 / (1.0 + (gamma.boundary[pi].values[fi] * dc) / ep));
                    }
                    let zeros = Field::new(vec![0.0; patch.size]);
                    bcs.push(PatchField {
                        bc: BoundaryCondition::MixedField {
                            value_fraction: Field::new(vf),
                            ref_value: Field::new(rv),
                            ref_grad: zeros.clone(),
                        },
                        values: zeros,
                    });
                }
            }
        }
        let g_start = match g_init {
            Some(g) => {
                assert_eq!(g.len(), n, "one initial G per cell");
                g.to_vec()
            }
            None => vec![0.0; n],
        };
        let g0 = VolScalarField::new("G", mesh.clone(), Field::new(g_start), bcs);

        // Matrix: upstream `laplacian(gamma, G) - Sp(a, G) == -4 (e sigma T^4) - E`,
        // negated into this crate's convention.
        let sp: Vec<f64> = (0..n).map(|c| mesh.cell_volumes[c] * a[c]).collect();
        let rhs: Vec<f64> = (0..n)
            .map(|c| mesh.cell_volumes[c] * ((4.0 * ((e[c] * sigma) * pow4(t.internal[c]))) + big_e[c]))
            .collect();
        let mat = laplacian_with_delta_diag_and_source(
            &gamma_f,
            &g0,
            DeltaCoeff::NonOrthogonal,
            Some(&sp),
            Some(&rhs),
        );
        let (solved, performance) = match g_init {
            Some(_) => mat.solve_cg_with_guess("G", &g0, settings),
            None => mat.solve_cg("G", settings),
        };

        // correctBoundaryConditions and qr.
        let mut boundary = Vec::with_capacity(mesh.patches.len());
        let mut qr = Vec::with_capacity(mesh.patches.len());
        for (pi, patch) in mesh.patches.iter().enumerate() {
            let mut vals = Vec::with_capacity(patch.size);
            let mut q = Vec::with_capacity(patch.size);
            for fi in 0..patch.size {
                let gf = patch.start + fi;
                let gc = solved.internal[mesh.owner[gf]];
                let gamma_b = gamma.boundary[pi].values[fi];
                match &g0.boundary[pi].bc {
                    BoundaryCondition::MixedField { value_fraction, ref_value, ref_grad } => {
                        let (w, rv, rg) = (value_fraction[fi], ref_value[fi], ref_grad[fi]);
                        let dc = DeltaCoeff::Orthogonal.boundary_delta_coeff(&mesh, gf);
                        // evaluate(): lerp(Gc + refGrad/dc, refValue, w)
                        vals.push((1.0 - w) * (gc + rg / dc) + w * rv);
                        // snGrad(): lerp(refGrad, (refValue - Gc)*dc, w)
                        let sn = (1.0 - w) * rg + w * ((rv - gc) * dc);
                        q.push(-gamma_b * sn);
                    }
                    _ => {
                        vals.push(gc);
                        q.push(-gamma_b * 0.0);
                    }
                }
            }
            boundary.push(PatchField { bc: g0.boundary[pi].bc.clone(), values: Field::new(vals) });
            qr.push(Field::new(q));
        }
        let g = VolScalarField::new("G", mesh.clone(), solved.internal.clone(), boundary);
        P1Solution { g, qr, performance, a, e, emission: big_e, t: t.clone() }
    }
}

impl P1Model {
    /// The state of a `P1` radiation field before its first solve: uniform incident radiation
    /// `g` [W/m2] (the `internalField` of upstream's `0/G`), no wall flux, the model's `a`,
    /// `e`, `E`, for the energy source of the first time step, which upstream evaluates
    /// from the read `G` before `radiation->correct()` has run (gh:#1531).
    #[must_use]
    pub fn initial_solution(&self, t: &VolScalarField, g: f64) -> P1Solution {
        let mesh = t.mesh.clone();
        let n = mesh.n_cells;
        let ae = &self.absorption_emission;
        let boundary = mesh.patches.iter().map(|p| PatchField::zero_gradient(p.size)).collect();
        P1Solution {
            g: VolScalarField::new("G", mesh.clone(), Field::new(vec![g; n]), boundary),
            qr: mesh.patches.iter().map(|p| Field::new(vec![0.0; p.size])).collect(),
            performance: SolverPerformance { n_iterations: 0, final_residual: 0.0, converged: true },
            a: vec![0.0 + ae.absorptivity; n],
            e: vec![0.0 + ae.emissivity; n],
            emission: vec![0.0 + ae.emission_source; n],
            t: t.clone(),
        }
    }
}

impl P1Solution {
    /// `Rp = 4 e sigma` per cell [W/m3/K4] (`P1::Rp`).
    #[must_use]
    pub fn rp(&self) -> Vec<f64> {
        let sigma = stefan_boltzmann_esi_v2506();
        self.e.iter().map(|&e| (4.0 * e) * sigma).collect()
    }

    /// `Ru = a G - E` per cell [W/m3] (`P1::Ru`).
    #[must_use]
    pub fn ru(&self) -> Vec<f64> {
        (0..self.a.len()).map(|c| self.a[c] * self.g.internal[c] - self.emission[c]).collect()
    }

    /// Upstream `radiationModel::ST(T)` = `Ru - fvm::Sp(Rp T^3, T)`, the radiative source of
    /// a temperature equation, in this crate's sign convention (upstream's matrix
    /// negated): `diag = V (Rp T^3)`, `source = V Ru`.
    #[must_use]
    pub fn st_matrix(&self) -> FvMatrix {
        let mesh = self.t.mesh.clone();
        let (ru, rp) = (self.ru(), self.rp());
        let mut m = FvMatrix::new(mesh.clone());
        for c in 0..mesh.n_cells {
            let t = self.t.internal[c];
            m.ldu.diag[c] += mesh.cell_volumes[c] * (rp[c] * pow3(t));
            m.source[c] = mesh.cell_volumes[c] * ru[c];
        }
        m
    }

    /// Upstream `radiationModel::Sh(thermo, he)` =
    /// `Ru - fvm::Sp(4 Rp T^3/Cpv, he) - Rp T^3 (T - 4 he/Cpv)`, the radiative source of an
    /// enthalpy / internal-energy equation, given `cpv` (`thermo.Cpv()`, J/kg/K) and `he`
    /// (J/kg) per cell, in this crate's sign convention (upstream's matrix negated).
    ///
    /// # Panics
    /// If `cpv` or `he` is not one value per cell.
    #[must_use]
    pub fn sh_matrix(&self, cpv: &[f64], he: &[f64]) -> FvMatrix {
        let mesh = self.t.mesh.clone();
        assert_eq!(cpv.len(), mesh.n_cells);
        assert_eq!(he.len(), mesh.n_cells);
        let (ru, rp) = (self.ru(), self.rp());
        let mut m = FvMatrix::new(mesh.clone());
        for c in 0..mesh.n_cells {
            let t = self.t.internal[c];
            let t3 = pow3(t);
            let v = mesh.cell_volumes[c];
            m.ldu.diag[c] += v * (((4.0 * rp[c]) * t3) / cpv[c]);
            let x = (rp[c] * t3) * (t - (4.0 * he[c]) / cpv[c]);
            m.source[c] = v * ru[c] - v * x;
        }
        m
    }

    /// Net radiative power absorbed by the medium, `sum V (Ru - Rp T^4)` [W].
    #[must_use]
    pub fn net_absorbed_power(&self) -> f64 {
        let mesh = &self.t.mesh;
        let (ru, rp) = (self.ru(), self.rp());
        (0..mesh.n_cells)
            .map(|c| mesh.cell_volumes[c] * (ru[c] - rp[c] * pow4(self.t.internal[c])))
            .sum()
    }

    /// Radiative power leaving through the boundary, `sum qr |Sf|` [W]. For a converged
    /// solution `net_absorbed_power() + boundary_power()` vanishes (the integral of the
    /// `G` equation).
    #[must_use]
    pub fn boundary_power(&self) -> f64 {
        let mesh = &self.t.mesh;
        let mut s = 0.0;
        for (pi, patch) in mesh.patches.iter().enumerate() {
            for fi in 0..patch.size {
                s += self.qr[pi][fi] * mesh.face_areas[patch.start + fi];
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::fv_mesh::FvMesh;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::primitives::Vector3;
    use std::sync::Arc;

    /// Uniform 1-D slab of `n` cells, length 1 m, cross-section `s` m2: patch 0 is the
    /// x-min wall, patch 1 the x-max wall.
    fn slab(n: usize, s: f64) -> Arc<FvMesh> {
        let h = 1.0 / n as f64;
        let mut owner = Vec::new();
        let mut neighbour = Vec::new();
        let mut sf = Vec::new();
        let mut cf = Vec::new();
        for f in 0..n - 1 {
            owner.push(f);
            neighbour.push(f + 1);
            sf.push(Vector3::new(s, 0.0, 0.0));
            cf.push(Vector3::new((f + 1) as f64 * h, 0.0, 0.0));
        }
        owner.push(0);
        sf.push(Vector3::new(-s, 0.0, 0.0));
        cf.push(Vector3::new(0.0, 0.0, 0.0));
        owner.push(n - 1);
        sf.push(Vector3::new(s, 0.0, 0.0));
        cf.push(Vector3::new(1.0, 0.0, 0.0));
        let centres = (0..n).map(|c| Vector3::new((c as f64 + 0.5) * h, 0.0, 0.0)).collect();
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(n)
                .n_internal_faces(n - 1)
                .owner(owner)
                .neighbour(neighbour)
                .patches(vec![
                    BoundaryPatch::new("xmin", n - 1, 1, PatchKind::Patch),
                    BoundaryPatch::new("xmax", n, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![s * h; n])
                .cell_centres(centres)
                .face_area_vectors(sf)
                .face_centres(cf)
                .build()
                .unwrap(),
        )
    }

    fn t_field(mesh: &Arc<FvMesh>, t: f64, t_walls: [f64; 2]) -> VolScalarField {
        let boundary = (0..2).map(|p| PatchField::fixed_value(1, t_walls[p])).collect();
        VolScalarField::new("T", mesh.clone(), Field::new(vec![t; mesh.n_cells]), boundary)
    }

    #[test]
    fn stefan_boltzmann_is_the_esi_truncated_constant_value() {
        let s = stefan_boltzmann_esi_v2506();
        assert!((s / 5.670_374_419e-8 - 1.0).abs() < 2e-5);
        assert!((s - 5.670_408_558_060_823_6e-8).abs() < 1e-22);
    }

    /// An isothermal black enclosure in radiative equilibrium has `G = 4 sigma T^4`,
    /// no wall flux and no net absorption.
    #[test]
    fn isothermal_equilibrium_is_exact() {
        let mesh = slab(10, 0.1);
        let t = t_field(&mesh, 800.0, [800.0, 800.0]);
        let model = P1Model::new(
            ConstantAbsorptionEmission { absorptivity: 2.0, emissivity: 2.0, emission_source: 0.0 },
            vec![
                P1PatchModel::Marshak { emissivity: 0.7 },
                P1PatchModel::MarshakFixedTemperature { emissivity: 0.4, t_rad: 800.0 },
            ],
        );
        let sol = model.solve(&t, SolverSettings { tolerance: 1e-14, max_iter: 200 });
        let g_eq = 4.0 * stefan_boltzmann_esi_v2506() * pow4(800.0);
        for c in 0..10 {
            assert!((sol.g.internal[c] / g_eq - 1.0).abs() < 1e-10, "cell {c}");
        }
        assert!(sol.net_absorbed_power().abs() < 1e-8 * g_eq);
        assert!(sol.qr[0][0].abs() < 1e-8 * g_eq);
        // Ru - Rp T^4 = 0 per cell, and the ST and Sh matrices are balanced then.
        let st = sol.st_matrix();
        let tt = pow4(800.0);
        let rp = sol.rp()[0];
        assert!((st.source[0] / (mesh.cell_volumes[0] * rp * tt) - 1.0).abs() < 1e-9);
        let sh = sol.sh_matrix(&vec![1000.0; 10], &vec![1.0e5; 10]);
        assert!(sh.ldu.diag[0] > 0.0);
    }

    /// The integral of the G equation: absorbed power plus boundary loss vanishes.
    #[test]
    fn global_balance_holds() {
        let mesh = slab(20, 0.1);
        let t = t_field(&mesh, 1000.0, [1500.0, 500.0]);
        let model = P1Model::new(
            ConstantAbsorptionEmission { absorptivity: 1.5, emissivity: 1.2, emission_source: 300.0 },
            vec![
                P1PatchModel::Marshak { emissivity: 0.8 },
                P1PatchModel::Marshak { emissivity: 0.5 },
            ],
        );
        let sol = model.solve(&t, SolverSettings { tolerance: 1e-13, max_iter: 500 });
        assert!(sol.performance.converged);
        let scale: f64 = (0..20).map(|c| mesh.cell_volumes[c] * sol.rp()[c] * pow4(1000.0)).sum();
        assert!((sol.net_absorbed_power() + sol.boundary_power()).abs() < 1e-8 * scale);
    }

    /// A `ZeroGradient` patch carries no flux.
    #[test]
    fn zero_gradient_patch_has_no_flux() {
        let mesh = slab(8, 0.1);
        let t = t_field(&mesh, 900.0, [900.0, 900.0]);
        let model = P1Model::new(
            ConstantAbsorptionEmission { absorptivity: 1.0, emissivity: 1.0, emission_source: 0.0 },
            vec![P1PatchModel::ZeroGradient, P1PatchModel::Marshak { emissivity: 1.0 }],
        );
        let sol = model.solve(&t, SolverSettings { tolerance: 1e-13, max_iter: 200 });
        assert_eq!(sol.qr[0][0], 0.0);
        assert_eq!(sol.g.boundary[0].values[0], sol.g.internal[0]);
    }

    /// `solve_from(None)` is `solve`, and a warm start from the converged field stops at once
    /// on the same answer (gh:#1531).
    #[test]
    fn warm_start_and_initial_solution() {
        let mesh = slab(12, 0.1);
        let t = t_field(&mesh, 900.0, [1200.0, 600.0]);
        let model = P1Model::new(
            ConstantAbsorptionEmission { absorptivity: 1.5, emissivity: 1.2, emission_source: 10.0 },
            vec![P1PatchModel::Marshak { emissivity: 0.8 }, P1PatchModel::Marshak { emissivity: 0.5 }],
        );
        let settings = SolverSettings { tolerance: 1e-14, max_iter: 500 };
        let cold = model.solve(&t, settings);
        let none = model.solve_from(&t, None, settings);
        for c in 0..12 {
            assert_eq!(cold.g.internal[c].to_bits(), none.g.internal[c].to_bits());
        }
        let warm = model.solve_from(&t, Some(cold.g.internal.as_slice()), settings);
        assert!(warm.performance.n_iterations <= 1, "{}", warm.performance.n_iterations);
        for c in 0..12 {
            assert!((warm.g.internal[c] / cold.g.internal[c] - 1.0).abs() < 1e-12);
        }
        let init = model.initial_solution(&t, 123.0);
        assert!(init.g.internal.as_slice().iter().all(|&g| g == 123.0));
        assert_eq!(init.ru()[0], 1.5 * 123.0 - 10.0);
        assert_eq!(init.qr[0][0], 0.0);
    }
}
