// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Upstream: GeN-Foam 652b3da on OpenFOAM-ESI v2506
//   GeN-Foam/GeN-Foam.C:207-212           `runTime.write()` after `adjustDeltaT`
//   OpenFOAM-v2506/src/OpenFOAM/db/Time/  `Time::operator++` (`writeTime_`), `Time::writeTimeDict`, `Time::timeName`
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full notice.

//! # The time directory the coupled loop writes (gh:#1931, README round 68)
//!
//! [`RegionSolvers::written_files`] is the text of every file the port writes at the current time; [`RegionSolvers::write_time_directory`]
//! puts it on disk. Which steps write follows the clock ([`RegionSolvers::is_write_time`], `writeControl timeStep` or
//! `adjustableRunTime`/`runTime`). What is not written, by name, is [`WrittenState::not_written`].

use std::path::Path;

use super::{case_err, RegionSolver, RegionSolvers};
use crate::error::AppBuilderError;
use crate::genfoam::multi_region::written_state::{field_file, plain_patches, surface_field, value_patches, vector_data};
use crate::io::foam_ascii_mesh::write_poly_mesh;
use crate::io::foam_ascii::{
    format_g, write_dictionary_file, write_field_file, write_field_file_with_words, write_list_file, write_time_dict, Data, FoamWriteError,
    ListObject, WriteOptions,
};
use crate::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::cyclic_constraint_patches;

/// The text of a written time directory.
#[derive(Debug, Clone, Default)]
pub struct WrittenState {
    /// `(path relative to the case root, text)`, in directory order: `<time>/<region>/<field>`, `<time>/uniform/...`.
    pub files: Vec<(String, String)>,
    /// Files GeN-Foam writes in this case that the port does NOT write (state not held or format not ported), by name.
    pub not_written: Vec<String>,
}


impl RegionSolvers {
    /// `runTime.writeTime()`: whether the last [`Self::step`] landed on a write time (`writeControl timeStep`: `timeIndex % writeInterval == 0`;
    /// `adjustableRunTime`/`runTime`: the write index advanced).
    #[must_use]
    pub fn is_write_time(&self) -> bool {
        self.clock.write_time()
    }

    /// `runTime.timeName()` of the current time (`timeFormat general`) ~~at `timePrecision 6`~~ **CORRECTED 2026-11-28 (gh:#1938, README round
    /// 84):** at `Time::precision_`: `controlDict`'s `timePrecision` (default 6; 2D_fullCoupling ships 8), grown at a write time whose name would
    /// not read back as the new time (`Time.C:1189-1225`, [`super::time_name::TimeName`]).
    #[must_use]
    pub fn time_directory_name(&self) -> String {
        self.time_name.name().to_string()
    }

    /// `Time::precision_` of the current time name (gh:#1938).
    #[must_use]
    pub fn time_precision(&self) -> usize {
        self.time_name.precision()
    }

    /// The files `runTime.write()` writes at the current time, as text.
    ///
    /// # Errors
    /// [`FoamWriteError`] for a boundary condition the writer does not lay out.
    pub fn written_files(&self, opts: &WriteOptions) -> Result<WrittenState, FoamWriteError> {
        let tn = self.time_directory_name();
        let mut state = WrittenState::default();
        let mut add = |path: String, text: String| state.files.push((format!("{tn}/{path}"), text));
        let mut reactor_state: Option<Vec<(String, String)>> = None;
        let mut not_written: Vec<String> = Vec::new();
        for r in &self.regions {
            let name = r.name.as_str();
            let mut fields = Vec::new();
            match &r.solver {
                RegionSolver::OnePhase(f) => {
                    let (fl, trad) = f.solver.written_fields(&tn)?;
                    fields.extend(fl);
                    // gh:#1961 (README round 81): the `RAS` fields (with their wall functions' word entries) and `T.fixedTemperature`
                    for (mut ff, words) in f.solver.written_turbulence_fields(&tn)? {
                        cyclic_constraint_patches(&mut ff, &r.fv);
                        add(format!("{name}/{}", ff.object), write_field_file_with_words(&ff, opts, &words)?);
                    }
                    for (path, per_cell) in trad {
                        let (class, location, object) = ("scalarFieldField", format!("{tn}/{name}"), path.split_once('/').map_or(path.as_str(), |(_, o)| o).to_string());
                        add(path.clone(), write_list_file(class, &location, &object, &ListObject::FieldField(per_cell), opts));
                    }
                    for nm in ["powerDensityNeutronics", "powerDensityNeutronicsToLiquid"] {
                        if let Some(v) = r.fields.scalar(nm) {
                            fields.extend(field_file(&format!("{name}/{nm}"), &tn, Data::scalar(v.to_vec()), plain_patches(&r.fv, "zeroGradient")));
                        }
                    }
                    if !f.solver.writes_alpha_rho_mag_u() {
                        not_written.push(format!("{name}/alphaRhoMagU: refused by name (no flow solved)"));
                    }
                    if r.remove_baffles {
                        match &r.baffle_less {
                            Some(Ok(m)) => {
                                let files = write_poly_mesh(m, &format!("{tn}/{name}.baffleLess/polyMesh"), opts).map_err(|e| FoamWriteError::Mesh(e.to_string()))?;
                                for (file, text) in files {
                                    add(format!("{name}.baffleLess/polyMesh/{file}"), text);
                                }
                            }
                            Some(Err(msg)) => return Err(FoamWriteError::Mesh(format!("{name}.baffleLess: {msg} is refused by name"))),
                            None => not_written.push(format!("{name}.baffleLess/polyMesh/*: not read")),
                        }
                    }
                }
                RegionSolver::Neutronics(n) => {
                    reactor_state = Some(vec![("keff".to_string(), format_g(n.k_eff, opts.precision)), ("pTarget".to_string(), format_g(n.p_target, opts.precision))]);
                    let cells = r.fv.n_cells;
                    let st = n.solver.state();
                    let star = n.solver.flux_star();
                    let mut one_group = vec![0.0; cells];
                    for (g, fl) in st.flux().iter().enumerate() {
                        for (acc, v) in one_group.iter_mut().zip(fl.internal.as_slice()) {
                            *acc += *v;
                        }
                        fields.extend(field_file(&format!("{name}/flux{g}"), &tn, Data::scalar(fl.internal.as_slice().to_vec()), value_patches(&r.fv, "fixedValue", Data::scalar(vec![0.0]))));
                        let fs = star.and_then(|s| s.get(g)).cloned().unwrap_or_else(|| fl.internal.as_slice().to_vec());
                        fields.extend(field_file(&format!("{name}/fluxStar{g}"), &tn, Data::scalar(fs), value_patches(&r.fv, "fixedValue", Data::scalar(vec![0.0]))));
                    }
                    for (k, p) in st.precursors().iter().enumerate() {
                        fields.extend(field_file(&format!("{name}/prec{k}"), &tn, Data::scalar(p.internal.as_slice().to_vec()), plain_patches(&r.fv, "zeroGradient")));
                        fields.extend(field_file(&format!("{name}/precStar{k}"), &tn, Data::scalar(vec![0.0]), plain_patches(&r.fv, "zeroGradient")));
                    }
                    fields.extend(field_file(&format!("{name}/oneGroupFlux"), &tn, Data::scalar(one_group), plain_patches(&r.fv, "zeroGradient")));
                    for nm in ["powerDensity", "secondaryPowerDensity", "TFuel", "TClad", "TCool", "TStruct", "TStructMech", "rhoCool"] {
                        if let Some(v) = r.fields.scalar(nm) {
                            fields.extend(field_file(&format!("{name}/{nm}"), &tn, Data::scalar(v.to_vec()), plain_patches(&r.fv, "zeroGradient")));
                        }
                    }
                    if let Some(d) = r.fields.vector("disp") {
                        fields.extend(field_file(&format!("{name}/disp"), &tn, vector_data(d), plain_patches(&r.fv, "zeroGradient")));
                    }
                    fields.extend(field_file(&format!("{name}/defaultFlux"), &tn, Data::scalar(vec![1.0]), value_patches(&r.fv, "fixedValue", Data::scalar(vec![0.0]))));
                    for nm in ["defaultPrec", "defaultExternalSourceFlux"] {
                        fields.extend(field_file(&format!("{name}/{nm}"), &tn, Data::scalar(vec![0.0]), plain_patches(&r.fv, "zeroGradient")));
                    }
                    // `polyMesh::movePoints` sets the points' instance to the current time: `points` is written only after a move
                    if n.mesh.is_moving() {
                        let points: Vec<[f64; 3]> = n.mesh.points().iter().map(|p| [p.x, p.y, p.z]).collect();
                        add(
                            format!("{name}/polyMesh/points"),
                            write_list_file("vectorField", &format!("{tn}/{name}/polyMesh"), "points", &ListObject::Plain(Data::vector(&points)), opts),
                        );
                    }
                    // `meshPhi`: the last move's swept-volume flux, else the one READ from the start directory (`fvMesh` keeps it until the mesh moves)
                    let phi = n.mesh.mesh_phi().map(<[f64]>::to_vec).or_else(|| r.fields.scalar("meshPhi").map(<[f64]>::to_vec));
                    match phi {
                        Some(phi) => {
                            let file = surface_field(&format!("{name}/meshPhi"), &tn, &r.fv, &phi);
                            if let Some(f) = file {
                                add(format!("{name}/meshPhi"), write_field_file(&f, opts)?);
                            }
                        }
                        None => not_written.push(format!("{name}/meshPhi: the mesh never moved and none was read (upstream has no such object)")),
                    }
                }
                RegionSolver::PointKinetics(p) => {
                    // the objects `pointKineticNeutronics` registers with AUTO_WRITE (README round 69: the real `110` of the PK stage)
                    let pk = &p.pk;
                    let patches_flux = value_patches(&r.fv, "fixedValue", Data::scalar(vec![0.0]));
                    for (g, fl) in pk.fluxes.iter().enumerate() {
                        fields.extend(field_file(&format!("{name}/flux{g}"), &tn, Data::scalar(fl.clone()), patches_flux.clone()));
                    }
                    for (k, pr) in pk.prec.iter().enumerate() {
                        fields.extend(field_file(&format!("{name}/prec{k}"), &tn, Data::scalar(pr.clone()), plain_patches(&r.fv, "zeroGradient")));
                    }
                    fields.extend(field_file(&format!("{name}/oneGroupFlux"), &tn, Data::scalar(pk.one_group_flux.clone()), plain_patches(&r.fv, "zeroGradient")));
                    for nm in ["powerDensity", "secondaryPowerDensity", "TFuel", "TClad", "TCool", "TStruct", "TStructMech", "rhoCool"] {
                        if let Some(v) = r.fields.scalar(nm) {
                            fields.extend(field_file(&format!("{name}/{nm}"), &tn, Data::scalar(v.to_vec()), plain_patches(&r.fv, "zeroGradient")));
                        }
                    }
                    fields.extend(field_file(&format!("{name}/defaultPrec"), &tn, Data::scalar(vec![0.0]), plain_patches(&r.fv, "zeroGradient")));
                    // `fvMesh` keeps the `meshPhi` it READ (the PK mesh is a fresh one at the written points and never moves)
                    match r.fields.scalar("meshPhi") {
                        Some(phi) => {
                            if let Some(f) = surface_field(&format!("{name}/meshPhi"), &tn, &r.fv, phi) {
                                add(format!("{name}/meshPhi"), write_field_file(&f, opts)?);
                            }
                        }
                        None => not_written.push(format!("{name}/meshPhi: none was read (upstream has no such object)")),
                    }
                    let mut entries = vec![("keff".to_string(), format_g(p.k_eff, opts.precision))];
                    for (k, vals) in pk.reactor_state_write_entries() {
                        let text = if k == "precursorPowers" {
                            let mut t = format!("{} (", vals.len());
                            for v in &vals {
                                t.push(' ');
                                t.push_str(&format_g(*v, opts.precision));
                            }
                            t.push_str(" )");
                            t
                        } else {
                            format_g(vals[0], opts.precision)
                        };
                        entries.push((k, text));
                    }
                    reactor_state = Some(entries);
                }
                RegionSolver::ThermoMechanics(t) => {
                    fields.extend(t.solver.written_fields(&tn, &r.fv)?);
                }
            }
            for mut f in fields {
                // gh:#1961 (README round 81): a field on a cyclic mesh patch is written with the constraint type
                cyclic_constraint_patches(&mut f, &r.fv);
                let path = format!("{name}/{}", f.object);
                add(path, write_field_file(&f, opts)?);
            }
        }
        if let Some(entries) = reactor_state {
            add("uniform/reactorState".to_string(), write_dictionary_file(&format!("{tn}/uniform"), "reactorState", &entries));
        }
        add("uniform/functionObjects/functionObjectProperties".to_string(), write_dictionary_file(&format!("{tn}/uniform/functionObjects"), "functionObjectProperties", &[]));
        add("uniform/time".to_string(), write_time_dict(self.time, &tn, self.start_time_index + self.clock.time_index(), self.clock.delta_t(), self.clock.delta_t0(), opts));
        state.files.sort();
        state.not_written = not_written;
        Ok(state)
    }

    /// What `createBaffleLessMeshes` writes into the START directory when the run is constructed (gh:#1954, README round 84):
    /// `removeBaffles` calls `findBaffles`, which writes `<start>/<region>.baffleLess/polyMesh/sets/duplicateFaces` (`mergeOrSplitBaffles.C:215-231`),
    /// and `mesh.write()` BEFORE `addZones` (`meshHandler.C:483-527`), so the eight `polyMesh` files carry empty zone files
    /// ([`crate::io::foam_ascii_mesh::PolyMeshOut::at_start`]). Paths as in [`Self::written_files`] (`<start>/<region>.baffleLess/polyMesh/...`).
    ///
    /// # Errors
    /// [`FoamWriteError::Mesh`] after the first step (the start name is gone), or for a baffle-less mesh that was refused.
    pub fn baffle_less_start_files(&self, opts: &WriteOptions) -> Result<WrittenState, FoamWriteError> {
        if self.step != 0 {
            return Err(FoamWriteError::Mesh("the start directory's baffle-less mesh is written before the first step".to_string()));
        }
        let tn = self.time_directory_name();
        let mut state = WrittenState::default();
        for r in &self.regions {
            if !r.remove_baffles {
                continue;
            }
            let name = r.name.as_str();
            match &r.baffle_less {
                Some(Ok(m)) => {
                    let location = format!("{tn}/{name}.baffleLess/polyMesh");
                    match write_poly_mesh(&m.at_start(), &location, opts) {
                        Ok(files) => {
                            for (file, text) in files {
                                state.files.push((format!("{location}/{file}"), text));
                            }
                        }
                        Err(e) => return Err(FoamWriteError::Mesh(e.to_string())),
                    }
                    match m.duplicate_faces_file(&location) {
                        Some((file, text)) => state.files.push((format!("{location}/{file}"), text)),
                        None => state.not_written.push(format!("{name}.baffleLess/polyMesh/sets/duplicateFaces: the mesh was not built by removeBaffles")),
                    }
                }
                Some(Err(msg)) => return Err(FoamWriteError::Mesh(format!("{name}.baffleLess: {msg} is refused by name"))),
                None => state.not_written.push(format!("{name}.baffleLess/polyMesh/*: not read")),
            }
        }
        state.files.sort();
        Ok(state)
    }

    /// Write the time directory under `root` (`<root>/<time>/...`): [`Self::written_files`] and the file I/O.
    ///
    /// # Errors
    /// [`AppBuilderError`] for a boundary condition the writer does not lay out, or a file that cannot be written.
    pub fn write_time_directory(&self, root: &Path, opts: &WriteOptions) -> Result<WrittenState, AppBuilderError> {
        let state = self.written_files(opts).map_err(|e| case_err(root, e.to_string()))?;
        for (rel, text) in &state.files {
            let path = root.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| AppBuilderError::Io { path: parent.to_path_buf(), source: e })?;
            }
            std::fs::write(&path, text).map_err(|e| AppBuilderError::Io { path: path.clone(), source: e })?;
        }
        Ok(state)
    }
}
