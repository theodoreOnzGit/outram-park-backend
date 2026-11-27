// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Upstream: GeN-Foam 652b3da on OpenFOAM-ESI v2506: the field objects `fluid`/`structure`/`nuclearFuelPin` register and
// `runTime.write()` writes (`T`, `U`, `p`, `p_rgh`, `alphaPhi`, `alphaRhoPhi`, `htc`, `Re`, `Kd`, `T.passiveStructure`, ...).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full notice.

//! # The fluid region's own fields as `runTime.write()` writes them (gh:#1931, README round 68)
//!
//! [`OnePhaseCcSolver::written_fields`]: one [`FieldFile`] per field object the solver holds the state of, with the patch fields the
//! solver's own boundary conditions give (`fixedValue` values, `fixedFluxPressure` gradients, `calculated` face values). What the solver
//! does not hold is not written and is listed by the caller (gh:#1935, round 69: `dgdt` and `alphaRhoMagU` are now written; `alphaRhoMagU` is
//! refused by name under `momentumMode cellCenteredFaceReconstruction`, see [`OnePhaseCcSolver::writes_alpha_rho_mag_u`]).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_foam_basic_lib::mesh::PatchKind;

use super::{Bc, OnePhaseCcSolver};
use crate::genfoam::multi_region::written_state::{boundary_slice, face_slice, field_file, plain_patches, surface_field, value_patches, vector_data};
use crate::genfoam::thermal_hydraulics::solver::two_phase::fv::VectorBc;
use crate::genfoam::thermal_hydraulics::structure::radial_power_model::RadialPowerModel;
use crate::io::foam_ascii::{Data, FieldFile, FoamWriteError, PatchOut};

impl OnePhaseCcSolver {
    /// Every field object of the fluid region the solver holds, as files of time directory `time_name`.
    ///
    /// # Errors
    /// [`FoamWriteError::PatchType`] for a boundary condition the writer does not lay out (`inletOutlet`).
    pub fn written_fields(&self, time_name: &str) -> Result<(Vec<FieldFile>, Vec<(String, Vec<Data>)>), FoamWriteError> {
        let mesh = &self.case.mesh;
        let ni = mesh.n_internal_faces;
        let n = mesh.n_cells;
        let mut out: Vec<FieldFile> = Vec::new();
        let mut trad_objects: Vec<(String, Vec<Data>)> = Vec::new();
        let mut push = |f: Option<FieldFile>| {
            if let Some(f) = f {
                out.push(f);
            }
        };
        let zero_gradient = |path: &str, values: Vec<f64>| field_file(path, time_name, Data::scalar(values), plain_patches(mesh, "zeroGradient"));

        // ---- T, U: the patch fields of the case's own boundary conditions
        if let Some(fc) = &self.case.flow {
            let t_patches = mesh
                .patches
                .iter()
                .zip(&fc.t_bc)
                .map(|(p, bc)| match bc {
                    _ if p.kind == PatchKind::Empty => Ok(PatchOut::plain(&p.name, "empty")),
                    Bc::ZeroGradient => Ok(PatchOut::plain(&p.name, "zeroGradient")),
                    Bc::Fixed(v) => Ok(PatchOut::with_value(&p.name, "fixedValue", Data::scalar(v.clone()))),
                    Bc::Empty => Ok(PatchOut::plain(&p.name, "empty")),
                    Bc::Calculated => Ok(PatchOut::plain(&p.name, "calculated")),
                    Bc::FixedGradient(_) => Err(FoamWriteError::PatchType("fixedGradient (T)".to_string())),
                    Bc::InletOutlet { .. } => Err(FoamWriteError::PatchType("inletOutlet".to_string())),
                })
                .collect::<Result<Vec<_>, _>>()?;
            push(field_file("fluidRegion/T", time_name, Data::scalar(self.t.clone()), t_patches));
            let u_patches = mesh
                .patches
                .iter()
                .zip(&fc.u_bc)
                .map(|(p, bc)| match bc {
                    _ if p.kind == PatchKind::Empty => Ok(PatchOut::plain(&p.name, "empty")),
                    VectorBc::ZeroGradient => Ok(PatchOut::plain(&p.name, "zeroGradient")),
                    VectorBc::Slip => Ok(PatchOut::plain(&p.name, "slip")),
                    VectorBc::Fixed(v) => Ok(PatchOut::with_value(&p.name, "fixedValue", vector_data(v))),
                    _ => Err(FoamWriteError::PatchType("velocity boundary condition other than fixedValue, zeroGradient, slip".to_string())),
                })
                .collect::<Result<Vec<_>, _>>()?;
            push(field_file("fluidRegion/U", time_name, vector_data(&self.u.c), u_patches));
        }

        // ---- p_rgh, p, Kd: the pressure state of the flow
        if let Some(fl) = &self.flow {
            let prgh_patches = mesh
                .patches
                .iter()
                .zip(&fl.p_rgh.bc)
                .map(|(p, bc)| match bc {
                    _ if p.kind == PatchKind::Empty => Ok(PatchOut::plain(&p.name, "empty")),
                    Bc::FixedGradient(g) => Ok(PatchOut {
                        name: p.name.clone(),
                        ptype: "fixedFluxPressure".to_string(),
                        entries: vec![
                            ("gradient".to_string(), Data::scalar(g.clone())),
                            ("value".to_string(), Data::scalar(boundary_slice(&fl.p_rgh.b, mesh, p))),
                        ],
                    }),
                    Bc::Fixed(v) => Ok(PatchOut::with_value(&p.name, "fixedValue", Data::scalar(v.clone()))),
                    Bc::ZeroGradient => Ok(PatchOut::plain(&p.name, "zeroGradient")),
                    _ => Err(FoamWriteError::PatchType("p_rgh boundary condition other than fixedFluxPressure, fixedValue, zeroGradient".to_string())),
                })
                .collect::<Result<Vec<_>, _>>()?;
            push(field_file("fluidRegion/p_rgh", time_name, Data::scalar(fl.p_rgh.c.clone()), prgh_patches));
            // `p = p_rgh + rho gh`, calculated: the boundary values are `p_rgh_b + rho_b gh_f`
            let p_patches = mesh
                .patches
                .iter()
                .map(|p| {
                    if p.kind == PatchKind::Empty {
                        return PatchOut::plain(&p.name, "empty");
                    }
                    let v: Vec<f64> = (p.start..p.start + p.size).map(|f| fl.p_rgh.b[f - ni] + self.rho_b[f - ni] * fl.ghf[f]).collect();
                    PatchOut::with_value(&p.name, "calculated", Data::scalar(v))
                })
                .collect();
            push(field_file("fluidRegion/p", time_name, Data::scalar(self.p.clone()), p_patches));
            let kd: Vec<f64> = fl.kd.iter().flat_map(|k| [k[0], 0.0, 0.0, 0.0, k[1], 0.0, 0.0, 0.0, k[2]]).collect();
            push(field_file("fluidRegion/Kd", time_name, Data::flat(9, kd), plain_patches(mesh, "zeroGradient")));
        }

        // ---- alphaRhoMagU (init-time value or READ; refused by name when `correctAlphaRhoMagU()` runs) and dgdt (`fluid.C:185-224`)
        if let Some(fc) = &self.case.flow {
            if !fc.reconstruct_u {
                let patches = mesh
                    .patches
                    .iter()
                    .map(|p| {
                        if p.kind == PatchKind::Empty {
                            PatchOut::plain(&p.name, "empty")
                        } else {
                            PatchOut::with_value(&p.name, "calculated", Data::scalar(boundary_slice(&self.alpha_rho_mag_u.1, mesh, p)))
                        }
                    })
                    .collect();
                push(field_file("fluidRegion/alphaRhoMagU", time_name, Data::scalar(self.alpha_rho_mag_u.0.clone()), patches));
            }
            // `dgdt` patch values are `extrapolatedCalculated`'s: the adjacent cell's value
            let patches = mesh
                .patches
                .iter()
                .map(|p| {
                    if p.kind == PatchKind::Empty {
                        PatchOut::plain(&p.name, "empty")
                    } else {
                        PatchOut::with_value(&p.name, "calculated", Data::scalar((p.start..p.start + p.size).map(|f| self.dgdt[mesh.owner[f]]).collect()))
                    }
                })
                .collect();
            push(field_file("fluidRegion/dgdt", time_name, Data::scalar(self.dgdt.clone()), patches));
        }

        // ---- the face fluxes
        push(surface_field("fluidRegion/alphaPhi", time_name, mesh, &self.alpha_phi));
        push(surface_field("fluidRegion/alphaRhoPhi", time_name, mesh, &self.alpha_rho_phi));

        // ---- cell fields on zeroGradient patches
        push(zero_gradient("fluidRegion/htc", self.htc.clone()));
        push(zero_gradient("fluidRegion/Re", self.re.clone()));
        push(zero_gradient("fluidRegion/heatFlux.structure", self.structure_heat_flux.clone()));
        if !self.t_pas.is_empty() {
            push(zero_gradient("fluidRegion/T.passiveStructure", self.t_pas.clone()));
        }
        let (fuel, clad) = self.fuel_clad_temperatures();
        push(zero_gradient("fluidRegion/T.fuelAvForNeutronics", fuel));
        push(zero_gradient("fluidRegion/T.cladAvForNeutronics", clad));
        // `alpha.structure` (class `structure`), `alpha.passiveStructure`, `alpha.powerModel`: uniform in this case
        for name in ["alpha.structure", "alpha.passiveStructure", "alpha.powerModel"] {
            push(zero_gradient(&format!("fluidRegion/{name}"), self.case.alpha_structure.clone()));
        }

        // ---- nuclearFuelPin: Tfi, Tfo, Tci, Tco, Tfav, Tcav (nodes of the radial mesh) and the radial profile Trad
        for m in &self.radial {
            if let RadialPowerModel::NuclearFuelPin(pin) = m {
                let (mut tfi, mut tfo, mut tci, mut tco) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
                let (mut tfav, mut tcav) = (vec![0.0; n], vec![0.0; n]);
                let mut trad: Vec<Data> = vec![Data::scalar(Vec::new()); n];
                for &c in pin.cells() {
                    let (a, b, cc, d) = pin.boundary_temperatures(c);
                    (tfi[c], tfo[c], tci[c], tco[c]) = (a, b, cc, d);
                    (tfav[c], tcav[c]) = pin.averages(c);
                    trad[c] = Data::scalar(pin.trad(c).to_vec());
                }
                for (suffix, v) in [("Tfi", tfi), ("Tfo", tfo), ("Tci", tci), ("Tco", tco), ("Tfav", tfav), ("Tcav", tcav)] {
                    push(zero_gradient(&format!("fluidRegion/{suffix}.nuclearFuelPin"), v));
                }
                trad_objects.push(("fluidRegion/Trad.nuclearFuelPin".to_string(), trad));
            }
        }
        let _ = (face_slice, value_patches);
        Ok((out, trad_objects))
    }
}

impl OnePhaseCcSolver {
    /// Whether [`Self::written_fields`] writes `alphaRhoMagU`: not under `momentumMode cellCenteredFaceReconstruction`, where
    /// `reconstructU_1p.H:21` calls `correctAlphaRhoMagU()` every corrector (not ported; refused by name, gh:#1935).
    #[must_use]
    pub fn writes_alpha_rho_mag_u(&self) -> bool {
        self.case.flow.as_ref().is_some_and(|f| !f.reconstruct_u)
    }
}
