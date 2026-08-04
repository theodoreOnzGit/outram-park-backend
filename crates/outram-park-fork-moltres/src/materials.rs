// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Physics formulation derived from Moltres (MSR multiphysics on MOOSE)
//   Upstream: https://github.com/arfc/moltres (UIUC ARFC group)
//   Upstream commit: 3dd2ce7
//   Upstream sources consulted (formulation only, no code reused):
//     src/kernels/{GroupDiffusion,SigmaR,InScatter,CoupledFissionKernel}.C
//     (the multigroup material-property vectors `diffcoef`, `remxs`, `nsf`,
//      `chi_p`, `chi_d`, `gtransfxs`, `beta_eff`, `decay_constant` and their
//      `d*_d_temp` temperature derivatives).
//   Upstream license: LGPL-2.1. The formulation is incorporated into this
//   GPL-3.0 crate under the LGPL-2.1 section 3 conversion option (LGPL code
//   may be redistributed under the ordinary GNU GPL). This file is an
//   independent finite-volume reimplementation on outram-foam-basic-lib —
//   no MOOSE, no finite elements, no upstream code copied.
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

//! Multigroup cross-section data and its materialisation onto an `FvMesh`.
//!
//! Mirrors the Moltres material-property system (`GenericMoltresMaterial`'s
//! `diffcoef` / `remxs` / `nsf` / `chi_p` / `chi_d` / `gtransfxs` vectors) as
//! plain Rust structs, with one **crucial unit change**: everything here is
//! **SI (metres)**, because the `outram-foam-basic-lib` meshes this crate
//! builds on are in metres. Reactor-physics tables are usually in
//! centimetres — convert before constructing an [`MsrMaterial`]
//! (`Sigma[1/m] = 100 * Sigma[1/cm]`, `D[m] = D[cm] / 100`).
//!
//! Temperature feedback is a **reduced linear model**: only the removal
//! (absorption + out-scatter) cross section carries a temperature derivative
//! `d Sigma_r / dT` (Moltres' `d_remxs_d_temp`), applied as
//! `Sigma_r(T) = Sigma_r(T_ref) + (dSigma_r/dT) (T - T_ref)`. Moltres itself
//! interpolates every group constant from tabulated `T` points; the linear
//! single-coefficient form is the documented first-pass simplification.

use std::sync::Arc;

use outram_foam_basic_lib::fv_operators::fvc;
use outram_foam_basic_lib::prelude::{
    Field, FvMesh, PatchField, SurfaceScalarField, VolScalarField,
};

use crate::error::MoltresError;

// ── Named field aliases (see workspace "Human interface layer" rule) ─────────

/// Scalar neutron flux field, one value per cell. Units: `1/(m^2 s)`
/// (multiply by `1e-4` for the conventional `1/(cm^2 s)`).
pub type NeutronFluxField = VolScalarField;

/// Delayed-neutron precursor concentration field, one value per cell.
/// Units: `1/m^3`.
pub type PrecursorField = VolScalarField;

/// Fuel-salt temperature field, one value per cell. Units: `K`.
pub type TemperatureField = VolScalarField;

/// Face volumetric flow flux `u . A_f`, one value per internal face.
/// Units: `m^3/s`. Positive = flow from face owner to face neighbour.
pub type FaceFluxField = SurfaceScalarField;

// ── Delayed-neutron families ─────────────────────────────────────────────────

/// One delayed-neutron precursor family.
///
/// Assumed uniform over the whole (well-mixed) fuel salt, which is why it is
/// a plain pair of numbers rather than a per-cell field (Moltres'
/// `beta_eff` / `decay_constant` material vectors, restricted to a single
/// fuel material).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DelayedFamily {
    /// Delayed fraction `beta_i` of this family (dimensionless, typically
    /// `1e-4 ..= 3e-3` per family; the 6-family total for U-235 is ~0.0065).
    pub beta: f64,
    /// Decay constant `lambda_i` in `1/s` (typically `0.01 ..= 3.0`).
    pub lambda: f64,
}

impl DelayedFamily {
    /// The classic 6-family delayed-neutron data for thermal fission of
    /// U-235 (G. R. Keepin, *Physics of Nuclear Kinetics*, Addison-Wesley,
    /// 1965 — public literature data). Total `beta = 0.006502`.
    ///
    /// `beta_i` dimensionless, `lambda_i` in `1/s`.
    #[must_use]
    pub fn keepin_u235() -> Vec<DelayedFamily> {
        vec![
            DelayedFamily {
                beta: 0.000215,
                lambda: 0.0124,
            },
            DelayedFamily {
                beta: 0.001424,
                lambda: 0.0305,
            },
            DelayedFamily {
                beta: 0.001274,
                lambda: 0.111,
            },
            DelayedFamily {
                beta: 0.002568,
                lambda: 0.301,
            },
            DelayedFamily {
                beta: 0.000748,
                lambda: 1.14,
            },
            DelayedFamily {
                beta: 0.000273,
                lambda: 3.01,
            },
        ]
    }

    /// Sum of `beta_i` over a family list (the total delayed fraction
    /// `beta`, dimensionless).
    #[must_use]
    pub fn total_beta(families: &[DelayedFamily]) -> f64 {
        families.iter().map(|f| f.beta).sum()
    }
}

// ── Per-zone material data ───────────────────────────────────────────────────

/// Multigroup neutron-diffusion constants for one material zone.
///
/// All vectors are indexed by energy group `g = 0 .. G-1`, ordered from the
/// **highest** energy group to the lowest (the Moltres/Serpent convention).
/// **Units are SI (metres)** throughout — see the module docs for cm → m
/// conversion.
#[derive(Debug, Clone, PartialEq)]
pub struct MsrMaterial {
    /// Human-readable zone name (diagnostics only).
    pub name: String,
    /// Diffusion coefficient `D_g` in `m` (typically `0.003 ..= 0.03` m,
    /// i.e. 0.3–3 cm). Moltres `diffcoef`.
    pub diffusion: Vec<f64>,
    /// Removal cross section `Sigma_{r,g} = Sigma_{a,g} + sum_{g' != g}
    /// Sigma_{g->g'}` in `1/m` (absorption **plus out-scatter**; the
    /// in-scatter *into* `g` is handled separately via `scattering`).
    /// Moltres `remxs`.
    pub sigma_removal: Vec<f64>,
    /// Fission-production cross section `nu Sigma_{f,g}` in `1/m`
    /// (zero in non-fuel zones). Moltres `nsf`.
    pub nu_sigma_f: Vec<f64>,
    /// Prompt fission spectrum `chi_{p,g}` (dimensionless; sums to 1 over
    /// groups in fissile zones, all-zero allowed in non-fuel zones).
    /// Moltres `chi_p`.
    pub chi_prompt: Vec<f64>,
    /// Delayed fission spectrum `chi_{d,g}` (dimensionless; sums to 1 in
    /// fissile zones, all-zero allowed elsewhere). Moltres `chi_d`.
    pub chi_delayed: Vec<f64>,
    /// Scattering matrix `scattering[g_from][g_to] = Sigma_{g_from ->
    /// g_to}` in `1/m`, with **zero diagonal** (within-group scattering is
    /// already excluded from `sigma_removal`'s out-scatter sum). Moltres
    /// `gtransfxs` off-diagonals.
    pub scattering: Vec<Vec<f64>>,
    /// Fission power conversion `kappa Sigma_{f,g}` in `J/m`, so the local
    /// power density is `q''' = sum_g kappaSigma_{f,g} phi_g` in `W/m^3`.
    /// (`kappa ~ 3.2e-11 J` per fission.) Zero in non-fuel zones.
    pub sigma_power: Vec<f64>,
    /// Reduced linear temperature-feedback coefficient
    /// `d Sigma_{r,g} / dT` in `1/(m K)` (Moltres `d_remxs_d_temp`;
    /// positive = heating adds absorption = negative reactivity feedback).
    pub d_sigma_removal_d_temp: Vec<f64>,
}

impl MsrMaterial {
    /// A non-multiplying material with every cross section zero except the
    /// given per-group `diffusion` (m) and `sigma_removal` (1/m). Useful for
    /// external-loop / reflector zones.
    #[must_use]
    pub fn non_fuel(name: impl Into<String>, diffusion: Vec<f64>, sigma_removal: Vec<f64>) -> Self {
        let g = diffusion.len();
        Self {
            name: name.into(),
            diffusion,
            sigma_removal,
            nu_sigma_f: vec![0.0; g],
            chi_prompt: vec![0.0; g],
            chi_delayed: vec![0.0; g],
            scattering: vec![vec![0.0; g]; g],
            sigma_power: vec![0.0; g],
            d_sigma_removal_d_temp: vec![0.0; g],
        }
    }

    /// Check internal consistency against an expected group count `g`:
    /// vector lengths, non-negativity, zero scattering diagonal, and (for
    /// fissile zones) fission spectra summing to 1 within `1e-6`.
    ///
    /// # Errors
    /// [`MoltresError::InvalidMaterial`] describing the first problem found.
    pub fn validate(&self, g: usize) -> Result<(), MoltresError> {
        let check_len = |what: &str, len: usize| {
            if len != g {
                Err(MoltresError::InvalidMaterial(format!(
                    "material '{}': {what} has {len} entries, expected {g} groups",
                    self.name
                )))
            } else {
                Ok(())
            }
        };
        check_len("diffusion", self.diffusion.len())?;
        check_len("sigma_removal", self.sigma_removal.len())?;
        check_len("nu_sigma_f", self.nu_sigma_f.len())?;
        check_len("chi_prompt", self.chi_prompt.len())?;
        check_len("chi_delayed", self.chi_delayed.len())?;
        check_len("sigma_power", self.sigma_power.len())?;
        check_len("d_sigma_removal_d_temp", self.d_sigma_removal_d_temp.len())?;
        check_len("scattering (rows)", self.scattering.len())?;
        for (i, row) in self.scattering.iter().enumerate() {
            check_len("scattering (columns)", row.len())?;
            if row[i] != 0.0 {
                return Err(MoltresError::InvalidMaterial(format!(
                    "material '{}': scattering[{i}][{i}] must be zero (within-group \
                     scattering is not a transfer term)",
                    self.name
                )));
            }
        }
        let non_negative = |what: &str, v: &[f64]| {
            if v.iter().any(|x| *x < 0.0 || !x.is_finite()) {
                Err(MoltresError::InvalidMaterial(format!(
                    "material '{}': {what} has a negative or non-finite entry",
                    self.name
                )))
            } else {
                Ok(())
            }
        };
        non_negative("diffusion", &self.diffusion)?;
        non_negative("sigma_removal", &self.sigma_removal)?;
        non_negative("nu_sigma_f", &self.nu_sigma_f)?;
        non_negative("chi_prompt", &self.chi_prompt)?;
        non_negative("chi_delayed", &self.chi_delayed)?;
        non_negative("sigma_power", &self.sigma_power)?;
        for row in &self.scattering {
            non_negative("scattering", row)?;
        }
        if self.diffusion.iter().any(|d| *d <= 0.0) {
            return Err(MoltresError::InvalidMaterial(format!(
                "material '{}': every diffusion coefficient must be > 0",
                self.name
            )));
        }
        let fissile = self.nu_sigma_f.iter().any(|x| *x > 0.0);
        if fissile {
            for (what, chi) in [
                ("chi_prompt", &self.chi_prompt),
                ("chi_delayed", &self.chi_delayed),
            ] {
                let s: f64 = chi.iter().sum();
                if (s - 1.0).abs() > 1e-6 {
                    return Err(MoltresError::InvalidMaterial(format!(
                        "material '{}' is fissile but {what} sums to {s}, expected 1",
                        self.name
                    )));
                }
            }
        }
        Ok(())
    }
}

// ── Mesh-materialised cross sections ─────────────────────────────────────────

/// Cross sections materialised as per-cell fields on one `FvMesh`, ready for
/// finite-volume assembly. Built once by [`XsFields::materialize`]; the
/// per-zone [`MsrMaterial`] data is broadcast to cells through a
/// `zone_of_cell` map.
///
/// The diffusion coefficient is additionally interpolated to mesh faces
/// (**linear** face interpolation via `fvc::interpolate`; harmonic averaging
/// at strong material discontinuities is a documented future refinement)
/// because `fvm::laplacian` consumes a face field.
#[derive(Debug, Clone)]
pub struct XsFields {
    /// Number of energy groups `G` (>= 1).
    pub energy_groups: usize,
    /// The mesh the fields live on.
    pub mesh: Arc<FvMesh>,
    /// Face-interpolated diffusion coefficient `D_g` per group, `m`.
    pub diffusion_face: Vec<SurfaceScalarField>,
    /// Removal cross section `Sigma_{r,g}` per group at the reference
    /// temperature, `1/m`.
    pub sigma_removal: Vec<VolScalarField>,
    /// Fission production `nu Sigma_{f,g}` per group, `1/m`.
    pub nu_sigma_f: Vec<VolScalarField>,
    /// Prompt spectrum `chi_{p,g}` per group, dimensionless.
    pub chi_prompt: Vec<VolScalarField>,
    /// Delayed spectrum `chi_{d,g}` per group, dimensionless.
    pub chi_delayed: Vec<VolScalarField>,
    /// Scattering transfer `Sigma_{g_from->g_to}` as `scattering[from][to]`,
    /// `1/m` (zero diagonal).
    pub scattering: Vec<Vec<VolScalarField>>,
    /// Power conversion `kappa Sigma_{f,g}` per group, `J/m`.
    pub sigma_power: Vec<VolScalarField>,
    /// Linear feedback coefficient `d Sigma_{r,g}/dT` per group, `1/(m K)`.
    pub d_sigma_removal_d_temp: Vec<VolScalarField>,
}

impl XsFields {
    /// Broadcast per-zone materials onto the mesh.
    ///
    /// - `zone_of_cell[c]` gives the index into `materials` for cell `c`
    ///   (length must equal `mesh.n_cells`; every index must be in range).
    /// - Every material must validate against the group count of
    ///   `materials[0]`.
    ///
    /// # Errors
    /// [`MoltresError::SizeMismatch`] for a bad `zone_of_cell`,
    /// [`MoltresError::InvalidMaterial`] for inconsistent material data.
    pub fn materialize(
        mesh: Arc<FvMesh>,
        zone_of_cell: &[usize],
        materials: &[MsrMaterial],
    ) -> Result<Self, MoltresError> {
        if materials.is_empty() {
            return Err(MoltresError::InvalidMaterial(
                "at least one material is required".into(),
            ));
        }
        let g = materials[0].diffusion.len();
        if g == 0 {
            return Err(MoltresError::InvalidMaterial(
                "materials must have at least one energy group".into(),
            ));
        }
        for m in materials {
            m.validate(g)?;
        }
        if zone_of_cell.len() != mesh.n_cells {
            return Err(MoltresError::SizeMismatch {
                what: "zone_of_cell",
                expected: mesh.n_cells,
                got: zone_of_cell.len(),
            });
        }
        if let Some(bad) = zone_of_cell.iter().find(|z| **z >= materials.len()) {
            return Err(MoltresError::SizeMismatch {
                what: "zone_of_cell entry (zone index out of range)",
                expected: materials.len(),
                got: *bad,
            });
        }

        // Broadcast one per-zone scalar into a per-cell field.
        let broadcast = |name: String, pick: &dyn Fn(&MsrMaterial) -> f64| -> VolScalarField {
            let vals: Vec<f64> = zone_of_cell.iter().map(|z| pick(&materials[*z])).collect();
            scalar_field(&mesh, name, vals)
        };

        let mut diffusion_face = Vec::with_capacity(g);
        let mut sigma_removal = Vec::with_capacity(g);
        let mut nu_sigma_f = Vec::with_capacity(g);
        let mut chi_prompt = Vec::with_capacity(g);
        let mut chi_delayed = Vec::with_capacity(g);
        let mut sigma_power = Vec::with_capacity(g);
        let mut d_srem_dt = Vec::with_capacity(g);
        let mut scattering = Vec::with_capacity(g);

        for gg in 0..g {
            let d_vol = broadcast(format!("D{gg}"), &|m| m.diffusion[gg]);
            diffusion_face.push(fvc::interpolate(&d_vol));
            sigma_removal.push(broadcast(format!("sigmaR{gg}"), &|m| m.sigma_removal[gg]));
            nu_sigma_f.push(broadcast(format!("nuSigmaF{gg}"), &|m| m.nu_sigma_f[gg]));
            chi_prompt.push(broadcast(format!("chiP{gg}"), &|m| m.chi_prompt[gg]));
            chi_delayed.push(broadcast(format!("chiD{gg}"), &|m| m.chi_delayed[gg]));
            sigma_power.push(broadcast(format!("sigmaPow{gg}"), &|m| m.sigma_power[gg]));
            d_srem_dt.push(broadcast(format!("dSigmaRdT{gg}"), &|m| {
                m.d_sigma_removal_d_temp[gg]
            }));
            let mut row = Vec::with_capacity(g);
            for gt in 0..g {
                row.push(broadcast(format!("sigmaS{gg}to{gt}"), &|m| {
                    m.scattering[gg][gt]
                }));
            }
            scattering.push(row);
        }

        Ok(Self {
            energy_groups: g,
            mesh,
            diffusion_face,
            sigma_removal,
            nu_sigma_f,
            chi_prompt,
            chi_delayed,
            scattering,
            sigma_power,
            d_sigma_removal_d_temp: d_srem_dt,
        })
    }

    /// Removal cross sections adjusted for a temperature field with the
    /// reduced linear feedback model:
    /// `Sigma_r(T)[c] = Sigma_r[c] + (dSigma_r/dT)[c] (T[c] - t_ref)`,
    /// in `1/m`. `temperature` in `K`, `t_ref` in `K`.
    ///
    /// The result is clamped below at `1e-12 1/m` so an extreme temperature
    /// excursion cannot produce a non-physical negative removal (documented
    /// limitation of the linear model).
    ///
    /// # Errors
    /// [`MoltresError::SizeMismatch`] if `temperature` is not on this mesh.
    pub fn sigma_removal_at(
        &self,
        temperature: &TemperatureField,
        t_ref: f64,
    ) -> Result<Vec<VolScalarField>, MoltresError> {
        if temperature.internal.len() != self.mesh.n_cells {
            return Err(MoltresError::SizeMismatch {
                what: "temperature field",
                expected: self.mesh.n_cells,
                got: temperature.internal.len(),
            });
        }
        let t = temperature.internal.as_slice();
        let mut out = Vec::with_capacity(self.energy_groups);
        for g in 0..self.energy_groups {
            let base = self.sigma_removal[g].internal.as_slice();
            let slope = self.d_sigma_removal_d_temp[g].internal.as_slice();
            let vals: Vec<f64> = (0..self.mesh.n_cells)
                .map(|c| (base[c] + slope[c] * (t[c] - t_ref)).max(1e-12))
                .collect();
            out.push(scalar_field(&self.mesh, format!("sigmaR{g}(T)"), vals));
        }
        Ok(out)
    }
}

/// Build a `VolScalarField` from per-cell values with zero-gradient boundary
/// patches (the neutral choice; solvers override boundary handling through
/// the flux fields' own boundary conditions, not through cross sections).
pub(crate) fn scalar_field(
    mesh: &Arc<FvMesh>,
    name: impl Into<String>,
    values: Vec<f64>,
) -> VolScalarField {
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect();
    VolScalarField::new(name, mesh.clone(), Field::new(values), boundary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
    use uom::si::area::square_meter;
    use uom::si::f64::{Area, Length};
    use uom::si::length::meter;

    fn one_group_fuel() -> MsrMaterial {
        MsrMaterial {
            name: "fuel".into(),
            diffusion: vec![0.009],
            sigma_removal: vec![0.2],
            nu_sigma_f: vec![0.3],
            chi_prompt: vec![1.0],
            chi_delayed: vec![1.0],
            scattering: vec![vec![0.0]],
            sigma_power: vec![1e-11],
            d_sigma_removal_d_temp: vec![0.0],
        }
    }

    /// Keepin 6-family U-235 data must total beta = 0.006502 (the published
    /// value) — guards against typos in the constants.
    #[test]
    fn keepin_beta_total() {
        let fams = DelayedFamily::keepin_u235();
        assert_eq!(fams.len(), 6);
        let beta = DelayedFamily::total_beta(&fams);
        assert!((beta - 0.006502).abs() < 1e-9, "beta = {beta}");
    }

    #[test]
    fn validate_catches_bad_spectrum() {
        let mut m = one_group_fuel();
        m.chi_prompt = vec![0.5]; // fissile but spectrum does not sum to 1
        assert!(m.validate(1).is_err());
    }

    #[test]
    fn validate_catches_scattering_diagonal() {
        let mut m = one_group_fuel();
        m.scattering = vec![vec![0.1]];
        assert!(m.validate(1).is_err());
    }

    #[test]
    fn materialize_broadcasts_zones() {
        let mesh = Arc::new(
            create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), 4)
                .unwrap(),
        );
        let fuel = one_group_fuel();
        let ext = MsrMaterial::non_fuel("loop", vec![0.009], vec![0.1]);
        let xs = XsFields::materialize(mesh, &[0, 0, 1, 1], &[fuel, ext]).unwrap();
        assert_eq!(xs.energy_groups, 1);
        assert_eq!(xs.nu_sigma_f[0].internal.as_slice(), &[0.3, 0.3, 0.0, 0.0]);
        assert_eq!(
            xs.sigma_removal[0].internal.as_slice(),
            &[0.2, 0.2, 0.1, 0.1]
        );
    }

    #[test]
    fn temperature_feedback_shifts_removal() {
        let mesh = Arc::new(
            create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), 2)
                .unwrap(),
        );
        let mut fuel = one_group_fuel();
        fuel.d_sigma_removal_d_temp = vec![2e-4];
        let xs = XsFields::materialize(mesh.clone(), &[0, 0], &[fuel]).unwrap();
        let temp = scalar_field(&mesh, "T", vec![900.0, 950.0]);
        let s = xs.sigma_removal_at(&temp, 900.0).unwrap();
        let v = s[0].internal.as_slice();
        assert!((v[0] - 0.2).abs() < 1e-14);
        assert!((v[1] - (0.2 + 2e-4 * 50.0)).abs() < 1e-14);
    }
}
