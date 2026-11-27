// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Upstream: GeN-Foam 652b3da on OpenFOAM-ESI v2506: the field objects `legacyThermoMechanics` registers and `runTime.write()` writes
// (`disp`, `fuelDisp`, `CRDisp`, `TStruct`, `TFuel`, `TStructFromTH`, `meshDisp`, `isPorous`).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full notice.

//! # The thermo-mechanics region's own fields as `runTime.write()` writes them (gh:#1931, README round 68)
//!
//! [`LegacyFvmThermoMechanics::written_fields`]: `disp` (with its `GFTractionDisplacement` patches: `traction`, `pressure` and the
//! `value` the patch holds), `fuelDisp`, `CRDisp`, `TStruct` with their `fixedValue`/`zeroGradient` patches, `meshDisp`, `TFuel`,
//! `TStructFromTH`, `isPorous`. The material fields (`E`, `mu`, `nu`, `rho`, `alpha`), `fuelDispVector` and `gapWidth` are not written
//! (see the README round 68 results).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_foam_basic_lib::mesh::{FvMesh, PatchKind};

use super::{DispPatch, LegacyFvmThermoMechanics, ScalarPatch};
use crate::genfoam::multi_region::written_state::{field_file, plain_patches, vector_data};
use crate::io::foam_ascii::{Data, FieldFile, FoamWriteError, PatchOut};

impl LegacyFvmThermoMechanics {
    /// The index of the solver's patch of the region mesh's patch `name` (the solver's mesh may omit the `empty` patch).
    fn patch_index(&self, name: &str) -> Option<usize> {
        self.mesh.patches.iter().position(|q| q.name == name)
    }

    fn scalar_patches(&self, region_mesh: &FvMesh, patches: &[ScalarPatch]) -> Vec<PatchOut> {
        region_mesh
            .patches
            .iter()
            .map(|p| match self.patch_index(&p.name).map(|i| &patches[i]) {
                _ if p.kind == PatchKind::Empty => PatchOut::plain(&p.name, "empty"),
                Some(ScalarPatch::FixedValue(v)) => PatchOut::with_value(&p.name, "fixedValue", Data::scalar(vec![*v; p.size])),
                _ => PatchOut::plain(&p.name, "zeroGradient"),
            })
            .collect()
    }

    /// Every field object of the region the solver holds, as files of time directory `time_name`.
    ///
    /// # Errors
    /// [`FoamWriteError::PatchType`] for a displacement patch the writer does not lay out.
    pub fn written_fields(&self, time_name: &str, region_mesh: &FvMesh) -> Result<Vec<FieldFile>, FoamWriteError> {
        let mesh = region_mesh;
        let mut out = Vec::new();
        let mut push = |f: Option<FieldFile>| {
            if let Some(f) = f {
                out.push(f);
            }
        };
        let disp_patches = mesh
            .patches
            .iter()
            .map(|p| {
                let Some(pi) = self.patch_index(&p.name).filter(|_| p.kind != PatchKind::Empty) else {
                    return Ok(PatchOut::plain(&p.name, "empty"));
                };
                match &self.disp_patches[pi] {
                    DispPatch::ZeroGradient => Ok(PatchOut::plain(&p.name, "zeroGradient")),
                    DispPatch::FixedValue(v) => Ok(PatchOut::with_value(&p.name, "fixedValue", vector_data(&vec![*v; p.size]))),
                    DispPatch::GfTractionDisplacement(g) => Ok(PatchOut {
                        name: p.name.clone(),
                        ptype: "GFTractionDisplacement".to_string(),
                        entries: vec![
                            ("traction".to_string(), vector_data(g.traction())),
                            ("pressure".to_string(), Data::scalar(g.pressure().to_vec())),
                            ("value".to_string(), vector_data(&self.disp_b[pi])),
                        ],
                    }),
                    #[allow(unreachable_patterns)]
                    _ => Err(FoamWriteError::PatchType("displacement patch type".to_string())),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        push(field_file("thermoMechanicalRegion/disp", time_name, vector_data(&self.disp), disp_patches));
        push(field_file("thermoMechanicalRegion/fuelDisp", time_name, Data::scalar(self.fuel_disp.clone()), self.scalar_patches(mesh, &self.fuel_patches)));
        push(field_file("thermoMechanicalRegion/CRDisp", time_name, Data::scalar(self.cr_disp.clone()), self.scalar_patches(mesh, &self.cr_patches)));
        push(field_file("thermoMechanicalRegion/TStruct", time_name, Data::scalar(self.t_struct.clone()), self.scalar_patches(mesh, &self.t_patches)));
        push(field_file("thermoMechanicalRegion/meshDisp", time_name, vector_data(&self.mesh_disp), plain_patches(mesh, "zeroGradient")));
        push(field_file("thermoMechanicalRegion/TFuel", time_name, Data::scalar(self.t_fuel.clone()), plain_patches(mesh, "zeroGradient")));
        push(field_file("thermoMechanicalRegion/TStructFromTH", time_name, Data::scalar(self.t_struct_from_th.clone()), plain_patches(mesh, "zeroGradient")));
        // the material properties of the structure cards (`E` is the card's Young modulus, `mu = E / rho / (2 (1 + nu))`)
        for (name, v) in [("E", &self.rho_e), ("mu", &self.mu), ("nu", &self.nu), ("rho", &self.rho)] {
            push(field_file(&format!("thermoMechanicalRegion/{name}"), time_name, Data::scalar(v.clone()), plain_patches(mesh, "zeroGradient")));
        }
        push(field_file("thermoMechanicalRegion/isPorous", time_name, Data::scalar(self.is_porous.clone()), plain_patches(mesh, "zeroGradient")));
        Ok(out)
    }
}
