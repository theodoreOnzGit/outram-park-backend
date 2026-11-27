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
use crate::genfoam::multi_region::written_state::{field_file, plain_patches, value_patches, vector_data};
use crate::io::foam_ascii::{
    format_g, time_name, write_dictionary_file, write_field_file, write_list_file, write_time_dict, Data, FoamWriteError, ListObject, WriteOptions,
};

/// The text of a written time directory.
#[derive(Debug, Clone, Default)]
pub struct WrittenState {
    /// `(path relative to the case root, text)`, in directory order: `<time>/<region>/<field>`, `<time>/uniform/...`.
    pub files: Vec<(String, String)>,
    /// Files GeN-Foam writes in this case that the port does NOT write (state not held or format not ported), by name.
    pub not_written: Vec<String>,
}

/// `timePrecision` of `controlDict` (default 6): the digits of a time-directory name.
const TIME_PRECISION: usize = 6;

impl RegionSolvers {
    /// `runTime.writeTime()`: whether the last [`Self::step`] landed on a write time (`writeControl timeStep`: `timeIndex % writeInterval == 0`;
    /// `adjustableRunTime`/`runTime`: the write index advanced).
    #[must_use]
    pub fn is_write_time(&self) -> bool {
        self.clock.write_time()
    }

    /// `runTime.timeName()` of the current time (`timeFormat general`, `timePrecision 6`).
    #[must_use]
    pub fn time_directory_name(&self) -> String {
        time_name(self.time, TIME_PRECISION)
    }

    /// The files `runTime.write()` writes at the current time, as text.
    ///
    /// # Errors
    /// [`FoamWriteError`] for a boundary condition the writer does not lay out.
    pub fn written_files(&self, opts: &WriteOptions) -> Result<WrittenState, FoamWriteError> {
        let tn = self.time_directory_name();
        let mut state = WrittenState::default();
        let mut add = |path: String, text: String| state.files.push((format!("{tn}/{path}"), text));
        let mut k_eff_p_target: Option<(f64, f64)> = None;
        let mut not_written: Vec<String> = Vec::new();
        for r in &self.regions {
            let name = r.name.as_str();
            let mut fields = Vec::new();
            match &r.solver {
                RegionSolver::OnePhase(f) => {
                    let (fl, trad) = f.solver.written_fields(&tn)?;
                    fields.extend(fl);
                    for (path, per_cell) in trad {
                        let (class, location, object) = ("scalarFieldField", format!("{tn}/{name}"), path.split_once('/').map_or(path.as_str(), |(_, o)| o).to_string());
                        add(path.clone(), write_list_file(class, &location, &object, &ListObject::FieldField(per_cell), opts));
                    }
                    for nm in ["powerDensityNeutronics", "powerDensityNeutronicsToLiquid"] {
                        if let Some(v) = r.fields.scalar(nm) {
                            fields.extend(field_file(&format!("{name}/{nm}"), &tn, Data::scalar(v.to_vec()), plain_patches(&r.fv, "zeroGradient")));
                        }
                    }
                    not_written.extend(["dgdt", "alphaRhoMagU"].iter().map(|n| format!("{name}/{n}")));
                    if r.remove_baffles {
                        not_written.push(format!("{name}.baffleLess/polyMesh/*"));
                    }
                }
                RegionSolver::Neutronics(n) => {
                    k_eff_p_target = Some((n.k_eff, n.p_target));
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
                    let points: Vec<[f64; 3]> = n.mesh.points().iter().map(|p| [p.x, p.y, p.z]).collect();
                    add(
                        format!("{name}/polyMesh/points"),
                        write_list_file("vectorField", &format!("{tn}/{name}/polyMesh"), "points", &ListObject::Plain(Data::vector(&points)), opts),
                    );
                    not_written.push(format!("{name}/meshPhi"));
                }
                RegionSolver::PointKinetics(_) => {
                    not_written.push(format!("{name}/*: the point-kinetics region's own state is not written (refused by name)"));
                }
                RegionSolver::ThermoMechanics(t) => {
                    fields.extend(t.solver.written_fields(&tn, &r.fv)?);
                    not_written.extend(["alpha", "gapWidth", "fuelDispVector"].iter().map(|n| format!("{name}/{n}")));
                }
            }
            for f in fields {
                let path = format!("{name}/{}", f.object);
                add(path, write_field_file(&f, opts)?);
            }
        }
        if let Some((k, p)) = k_eff_p_target {
            let entries = vec![("keff".to_string(), format_g(k, opts.precision)), ("pTarget".to_string(), format_g(p, opts.precision))];
            add("uniform/reactorState".to_string(), write_dictionary_file(&format!("{tn}/uniform"), "reactorState", &entries));
        }
        add("uniform/functionObjects/functionObjectProperties".to_string(), write_dictionary_file(&format!("{tn}/uniform/functionObjects"), "functionObjectProperties", &[]));
        add("uniform/time".to_string(), write_time_dict(self.time, &tn, self.clock.time_index(), self.clock.delta_t(), self.clock.delta_t0(), opts));
        state.files.sort();
        state.not_written = not_written;
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
