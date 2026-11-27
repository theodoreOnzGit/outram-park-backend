// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam function objects against the real classes (gh:#1871, round 51)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da` on OpenFOAM-ESI v2506. `upstream_driver/fo1871_dump/fo1871Dump.C` constructs the
//! REAL `massFlow`, `TBulk`, `pressureDrop`, `patchScalarFieldValue`, `fieldDiffExtents` and `stopIfMaxFieldDiff`
//! (`functionObject::New`, UNMODIFIED `libFFSSrc2`) on the 24-cell graded box of `cases/fo1871/polyMesh`, drives every
//! field they read from closed-form functions (only `+ - * /`), calls `execute()` / `write()` for 12 steps, and the
//! reference holds the inputs at `%.17g` plus the files the classes wrote under `postProcessing/` at
//! `writePrecision 17` (`refs/fo1871.ref.txt.gz`, `scripts/build_and_run_fo1871_dump.sh`). The port is fed the same
//! inputs (`Cp` comes from the real `hPolynomial` thermo and is read from the dump) and each output is compared with
//! `to_bits`. `fieldIntegralToFMU` is compared UP TO THE FMU BOUNDARY only: the class is not compiled upstream, the
//! driver evaluates its own statement `gSum(fieldZone*volZone)` on a real cell zone.
//!
//! **Pass criterion: bit identity.** Controls (each must differ): the earlier `Iterator::sum` seed (`-0.0`),
//! `scaleFactor` dropped, uniform `Cp`, un-interpolated `p`, `>=` mask, cells-only stop test, the set read in sorted
//! order. Predictions and results: README, "Round 51".

#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::function_objects::field_diff_extents::{
    cell_mask, cell_mask_vector, internal_extents, patch_extents, patch_mask, patch_mask_vector, Extents,
};
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::function_objects::field_integral::volume_integral_over_cells;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::function_objects::mass_flow::{
    total_mass_flow, total_mass_flow_scaled, MassFlowRegion,
};
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::function_objects::patch_scalar_value::patch_field_values;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::function_objects::pressure_drop::{
    pressure_drop_regions, PressureRegion,
};
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::function_objects::stop_if_max_field_diff::{
    evaluate_stop_if_max_field_diff, evaluate_stop_if_max_field_diff_cells_only,
};
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::function_objects::t_bulk::{
    bulk_temperature, bulk_temperature_faces,
};
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_basic_lib::fields::{
    BoundaryCondition, Field, PatchField, SurfaceScalarField, VolField, VolScalarField, VolVectorField,
};
use outram_foam_basic_lib::fv_operators::fvc;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

const N_STEPS: usize = 12;
const PATCHES: [&str; 4] = ["inlet", "outlet", "walls", "caps"];

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let flg = bytes[3];
    let mut at = 10;
    if flg & 4 != 0 {
        at += 2 + usize::from(bytes[at]) + 256 * usize::from(bytes[at + 1]);
    }
    for bit in [8u8, 16u8] {
        if flg & bit != 0 {
            while bytes[at] != 0 {
                at += 1;
            }
            at += 1;
        }
    }
    if flg & 2 != 0 {
        at += 2;
    }
    let raw = decompress_to_vec(&bytes[at..bytes.len() - 8]).unwrap_or_else(|_| panic!("inflate {}", path.display()));
    String::from_utf8(raw).unwrap()
}

/// The parsed reference.
struct Reference {
    /// `M 0 name` mesh quantities and `I step name` inputs: key `(step, name)`.
    nums: HashMap<(usize, String), Vec<f64>>,
    /// `ORD name faces...`
    order: HashMap<String, Vec<usize>>,
    /// `STOP name step running`
    stop: HashMap<(String, usize), bool>,
    /// `O step name value`
    out: HashMap<(usize, String), f64>,
    /// `FILE path` sections: the lines of each file
    files: HashMap<String, Vec<String>>,
}

fn load() -> Reference {
    let text = gunzip(&vv_dir().join("refs/fo1871.ref.txt.gz"));
    let mut me = Reference {
        nums: HashMap::new(),
        order: HashMap::new(),
        stop: HashMap::new(),
        out: HashMap::new(),
        files: HashMap::new(),
    };
    let mut cur: Option<String> = None;
    for line in text.lines() {
        if let Some(p) = line.strip_prefix("FILE ") {
            cur = Some(p.to_string());
            me.files.insert(p.to_string(), Vec::new());
            continue;
        }
        if let Some(c) = &cur {
            me.files.get_mut(c).unwrap().push(line.to_string());
            continue;
        }
        let mut it = line.split_whitespace();
        match it.next() {
            Some("M") | Some("I") => {
                let step: usize = it.next().unwrap().parse().unwrap();
                let name = it.next().unwrap().to_string();
                me.nums.insert((step, name), it.map(|x| x.parse().unwrap()).collect());
            }
            Some("ORD") => {
                let name = it.next().unwrap().to_string();
                me.order.insert(name, it.map(|x| x.parse().unwrap()).collect());
            }
            Some("STOP") => {
                let name = it.next().unwrap().to_string();
                let step: usize = it.next().unwrap().parse().unwrap();
                let running: i32 = it.next().unwrap().parse().unwrap();
                me.stop.insert((name, step), running != 0);
            }
            Some("O") => {
                let step: usize = it.next().unwrap().parse().unwrap();
                let name = it.next().unwrap().to_string();
                me.out.insert((step, name), it.next().unwrap().parse().unwrap());
            }
            _ => {}
        }
    }
    me
}

impl Reference {
    fn get(&self, step: usize, name: &str) -> &Vec<f64> {
        self.nums.get(&(step, name.to_string())).unwrap_or_else(|| panic!("reference lacks {step} {name}"))
    }

    /// The numeric values of the data lines of a `postProcessing` file (comment lines `#` skipped).
    fn series(&self, name: &str, dat: &str) -> Vec<Vec<f64>> {
        let key = format!("postProcessing/{name}/0/{dat}");
        let lines = self.files.get(&key).unwrap_or_else(|| panic!("reference lacks file {key}"));
        lines
            .iter()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#') && !l.starts_with("Time = "))
            .map(|l| {
                l.replace(['(', ')', '='], " ")
                    .split_whitespace()
                    .filter_map(|w| w.parse::<f64>().ok())
                    .collect()
            })
            .collect()
    }
}

#[derive(Default, Debug)]
struct Tally {
    n: usize,
    exact: usize,
    worst_rel: f64,
    first_diff: Option<String>,
}

impl Tally {
    fn cmp(&mut self, what: &str, port: f64, up: f64) {
        self.n += 1;
        if port.to_bits() == up.to_bits() {
            self.exact += 1;
        } else {
            let rel = (port - up).abs() / up.abs().max(1e-300);
            if rel > self.worst_rel {
                self.worst_rel = rel;
            }
            if self.first_diff.is_none() {
                self.first_diff = Some(format!("{what}: port {port:?} upstream {up:?}"));
            }
        }
    }

    fn report(&self, label: &str) {
        println!(
            "{label}: {}/{} bit-identical, worst relative difference {:e}{}",
            self.exact,
            self.n,
            self.worst_rel,
            self.first_diff.as_ref().map(|d| format!(", first: {d}")).unwrap_or_default()
        );
    }

    fn identical(&self) -> bool {
        self.n > 0 && self.exact == self.n
    }
}

fn mesh() -> Arc<FvMesh> {
    read_poly_mesh(&vv_dir().join("cases/fo1871/polyMesh")).unwrap()
}

fn patch_values(r: &Reference, step: usize, base: &str, p: &str) -> Vec<f64> {
    r.get(step, &format!("{base}.{p}")).clone()
}

fn vol_scalar(r: &Reference, m: &Arc<FvMesh>, step: usize, name: &str) -> VolScalarField {
    let boundary = PATCHES
        .iter()
        .map(|p| {
            let v = patch_values(r, step, name, p);
            PatchField { bc: BoundaryCondition::FixedField(Field::new(v.clone())), values: Field::new(v) }
        })
        .collect();
    VolField::new(name, Arc::clone(m), Field::new(r.get(step, &format!("{name}.cell")).clone()), boundary)
}

fn triples(v: &[f64]) -> Vec<Vector3> {
    v.chunks(3).map(|c| Vector3::new(c[0], c[1], c[2])).collect()
}

fn vol_vector(r: &Reference, m: &Arc<FvMesh>, step: usize, name: &str) -> VolVectorField {
    let boundary = PATCHES
        .iter()
        .map(|p| {
            let v = triples(&patch_values(r, step, name, p));
            PatchField { bc: BoundaryCondition::FixedField(Field::new(v.clone())), values: Field::new(v) }
        })
        .collect();
    VolField::new(name, Arc::clone(m), Field::new(triples(r.get(step, &format!("{name}.cell")))), boundary)
}

fn alpha_rho_phi(r: &Reference, m: &Arc<FvMesh>, step: usize) -> SurfaceScalarField {
    let boundary = PATCHES
        .iter()
        .map(|p| {
            let v = patch_values(r, step, "arp", p);
            PatchField { bc: BoundaryCondition::FixedField(Field::new(v.clone())), values: Field::new(v) }
        })
        .collect();
    SurfaceScalarField::new("alphaRhoPhi", Arc::clone(m), Field::new(r.get(step, "arp.int").clone()), boundary)
}

fn patch_index(p: &str) -> usize {
    PATCHES.iter().position(|x| *x == p).unwrap()
}

fn sorted(v: &[usize]) -> Vec<usize> {
    let mut s = v.to_vec();
    s.sort_unstable();
    s
}

/// F0: the port's mesh reader gives the real mesh's geometry bit for bit.
#[test]
fn the_box_mesh_geometry_matches_the_real_mesh() {
    let r = load();
    let m = mesh();
    let mut t = Tally::default();
    for (c, v) in r.get(0, "V").iter().enumerate() {
        t.cmp("V", m.cell_volumes[c], *v);
    }
    for (c, v) in triples(r.get(0, "C")).iter().enumerate() {
        t.cmp("Cx", m.cell_centres[c].x, v.x);
        t.cmp("Cy", m.cell_centres[c].y, v.y);
        t.cmp("Cz", m.cell_centres[c].z, v.z);
    }
    for (f, v) in triples(r.get(0, "Cf")).iter().enumerate() {
        t.cmp("Cf", m.face_centres[f].x, v.x);
        t.cmp("Cf", m.face_centres[f].y, v.y);
        t.cmp("Cf", m.face_centres[f].z, v.z);
    }
    for (f, v) in triples(r.get(0, "Sf")).iter().enumerate() {
        t.cmp("Sf", m.face_area_vectors[f].x, v.x);
        t.cmp("Sf", m.face_area_vectors[f].y, v.y);
        t.cmp("Sf", m.face_area_vectors[f].z, v.z);
    }
    for (f, v) in r.get(0, "magSf").iter().enumerate() {
        t.cmp("magSf", m.face_areas[f], *v);
    }
    for (pi, name) in PATCHES.iter().enumerate() {
        let start = m.patches[pi].start;
        for (i, v) in r.get(0, &format!("magSf.{name}")).iter().enumerate() {
            t.cmp("magSf.patch", m.face_areas[start + i], *v);
        }
        for (i, v) in triples(r.get(0, &format!("Cf.{name}"))).iter().enumerate() {
            t.cmp("Cf.patch", m.face_centres[start + i].x, v.x);
            t.cmp("Cf.patch", m.face_centres[start + i].y, v.y);
            t.cmp("Cf.patch", m.face_centres[start + i].z, v.z);
        }
    }
    t.report("F0 geometry");
    let mut w = Tally::default();
    for (f, v) in r.get(0, "weights").iter().enumerate() {
        w.cmp("weights", fvc::linear_weight(&m, f), *v);
    }
    w.report("F0b weights");
    assert!(t.identical(), "mesh geometry differs: {:?}", t.first_diff);
    assert!(w.identical(), "weights differ: {:?}", w.first_diff);
}

/// F1: `massFlow` (patch, faceZone, faceSet; `scaleFactor`), plus the `-0.0` control.
#[test]
fn mass_flow_matches_the_real_class() {
    let r = load();
    let m = mesh();
    let mut t = Tally::default();
    let mut dropped_scale = 0usize;
    let mut sorted_diff = 0usize;
    for (name, scale) in [("mf", 1.0), ("mf180", 180.0)] {
        for p in PATCHES {
            let up = r.series(&format!("{name}_{p}"), "massFlow.dat");
            for n in 1..=N_STEPS {
                let a = alpha_rho_phi(&r, &m, n);
                let region = MassFlowRegion::Patch(patch_index(p));
                let port = total_mass_flow_scaled(&a, &region, scale).get::<kilogram_per_second>();
                t.cmp(&format!("{name}_{p} step {n}"), port, up[n - 1][1]);
                if scale != 1.0 && total_mass_flow(&a, &region).get::<kilogram_per_second>().to_bits() != up[n - 1][1].to_bits() {
                    dropped_scale += 1;
                }
            }
        }
    }
    let faces = |nm: &str| r.order.get(nm).unwrap().clone();
    for (name, list, scale) in [
        ("mf_zA", faces("zone.zA"), 180.0),
        ("mf_zB", faces("zone.zB"), 1.0),
        ("mf_sA", faces("set.fsA"), 1.0),
        ("mf_sB", faces("set.fsB"), 180.0),
    ] {
        let up = r.series(name, "massFlow.dat");
        for n in 1..=N_STEPS {
            let a = alpha_rho_phi(&r, &m, n);
            let port = total_mass_flow_scaled(&a, &MassFlowRegion::Faces(list.clone()), scale).get::<kilogram_per_second>();
            t.cmp(&format!("{name} step {n}"), port, up[n - 1][1]);
            let ps = total_mass_flow_scaled(&a, &MassFlowRegion::Faces(sorted(&list)), scale).get::<kilogram_per_second>();
            if ps.to_bits() != up[n - 1][1].to_bits() {
                sorted_diff += 1;
            }
        }
    }
    t.report("F1 massFlow");
    println!("F1 control: scaleFactor dropped differs in {dropped_scale} of 48 scaled values; sorted set order differs in {sorted_diff} of 48");
    // the zero-flux patch: upstream +0.0; `Iterator::sum` of the earlier form seeds -0.0
    let a = alpha_rho_phi(&r, &m, 1);
    let old: f64 = a.boundary[patch_index("walls")].values.iter().map(|v| v.abs()).sum();
    let up_walls = r.series("mf_walls", "massFlow.dat")[0][1];
    println!("F1 control (earlier `sum()` form) on the zero-flux patch: {old:?} against upstream {up_walls:?}");
    assert!(t.identical(), "massFlow not bit-identical: {:?}", t.first_diff);
    assert!(dropped_scale > 0, "control: dropping scaleFactor must differ");
}

fn cp_uniform(m: &Arc<FvMesh>, v: f64) -> VolScalarField {
    let boundary = PATCHES
        .iter()
        .enumerate()
        .map(|(pi, _)| {
            let vals = vec![v; m.patches[pi].size];
            PatchField { bc: BoundaryCondition::FixedField(Field::new(vals.clone())), values: Field::new(vals) }
        })
        .collect();
    VolField::new("Cp", Arc::clone(m), Field::new(vec![v; m.n_cells]), boundary)
}

/// F2: `TBulk` (patch, faceZone, faceSet), plus the uniform-`Cp` control.
#[test]
fn t_bulk_matches_the_real_class() {
    let r = load();
    let m = mesh();
    let mut t = Tally::default();
    let mut uniform_cp_diff = 0usize;
    for n in 1..=N_STEPS {
        let tt = vol_scalar(&r, &m, n, "T");
        let cp = vol_scalar(&r, &m, n, "Cp");
        let a = alpha_rho_phi(&r, &m, n);
        for p in PATCHES {
            let up = r.series(&format!("tb_{p}"), "TBulk.dat")[n - 1][1];
            let port = bulk_temperature(patch_index(p), &tt, &cp, &a).get::<kelvin>();
            t.cmp(&format!("tb_{p} step {n}"), port, up);
            let pu = bulk_temperature(patch_index(p), &tt, &cp_uniform(&m, 1500.0), &a).get::<kelvin>();
            if pu.to_bits() != up.to_bits() {
                uniform_cp_diff += 1;
            }
        }
        for (name, key) in [("tb_zA", "zone.zA"), ("tb_zB", "zone.zB"), ("tb_sA", "set.fsA"), ("tb_sB", "set.fsB")] {
            let up = r.series(name, "TBulk.dat")[n - 1][1];
            let port = bulk_temperature_faces(&tt, &cp, &a, r.order.get(key).unwrap()).get::<kelvin>();
            t.cmp(&format!("{name} step {n}"), port, up);
        }
    }
    t.report("F2 TBulk");
    println!("F2 control: uniform Cp differs from upstream in {uniform_cp_diff} of 48 patch values");
    assert!(t.identical(), "TBulk not bit-identical: {:?}", t.first_diff);
    assert!(uniform_cp_diff > 0, "control: uniform Cp must differ");
}

/// F3: `pressureDrop` (patch/patch, faceSet/faceSet, patch/faceSet), plus the un-interpolated control.
#[test]
fn pressure_drop_matches_the_real_class() {
    let r = load();
    let m = mesh();
    let mut t = Tally::default();
    let mut cell_value_diff = 0usize;
    let fsa = r.order.get("set.fsA").unwrap().clone();
    let fsb = r.order.get("set.fsB").unwrap().clone();
    for n in 1..=N_STEPS {
        let p = vol_scalar(&r, &m, n, "p_rgh");
        let cases = [
            ("pd_pp", PressureRegion::Patch(patch_index("inlet")), PressureRegion::Patch(patch_index("outlet"))),
            ("pd_ss", PressureRegion::FaceSet(fsa.clone()), PressureRegion::FaceSet(fsb.clone())),
            ("pd_ps", PressureRegion::Patch(patch_index("caps")), PressureRegion::FaceSet(fsb.clone())),
        ];
        for (name, r1, r2) in cases {
            let up = r.series(name, "pressureDrop.dat")[n - 1][0];
            let port = pressure_drop_regions(&m, &p, &r1, &r2).get::<pascal>();
            t.cmp(&format!("{name} step {n}"), port, up);
        }
        // control: faceSet side with the owner-cell value instead of the interpolated one
        let mut area = 0.0;
        let mut w = 0.0;
        for &f in &fsa {
            area += m.face_areas[f];
            w += p.internal[m.owner[f]] * m.face_areas[f];
        }
        let mut area2 = 0.0;
        let mut w2 = 0.0;
        for &f in &fsb {
            area2 += m.face_areas[f];
            w2 += p.internal[m.owner[f]] * m.face_areas[f];
        }
        let up = r.series("pd_ss", "pressureDrop.dat")[n - 1][0];
        if (w / area - w2 / area2).to_bits() != up.to_bits() {
            cell_value_diff += 1;
        }
    }
    t.report("F3 pressureDrop");
    println!("F3 control: owner-cell values instead of fvc::interpolate differ in {cell_value_diff} of {N_STEPS} steps");
    assert!(t.identical(), "pressureDrop not bit-identical: {:?}", t.first_diff);
    assert!(cell_value_diff > 0, "control: un-interpolated pressure must differ");
}

/// F4: `patchScalarFieldValue` face values.
#[test]
fn patch_scalar_field_value_matches_the_real_class() {
    let r = load();
    let m = mesh();
    let mut t = Tally::default();
    let mut permuted = 0usize;
    for (name, field, patch) in [("psv_inlet", "p_rgh", "inlet"), ("psv_outlet", "p_rgh", "outlet"), ("psv_caps", "B", "caps")] {
        let up = r.series(name, "patchScalarFieldValue.dat");
        assert_eq!(up.len(), N_STEPS);
        for n in 1..=N_STEPS {
            let f = vol_scalar(&r, &m, n, field);
            let port = patch_field_values(&f, patch_index(patch));
            // row = (time v0 v1 ...)
            t.cmp(&format!("{name} time {n}"), n as f64, up[n - 1][0]);
            assert_eq!(port.len(), up[n - 1].len() - 1);
            for (i, v) in port.iter().enumerate() {
                t.cmp(&format!("{name} step {n} face {i}"), *v, up[n - 1][1 + i]);
            }
            let mut rev = port.clone();
            rev.reverse();
            if rev.iter().zip(up[n - 1][1..].iter()).any(|(a, b)| a.to_bits() != b.to_bits()) {
                permuted += 1;
            }
        }
    }
    t.report("F4 patchScalarFieldValue");
    println!("F4 control: reversed face order differs in {permuted} of 36 rows");
    assert!(t.identical());
    assert!(permuted > 0);
}

fn box_of(v: &[f64]) -> Extents {
    Extents { min: Vector3::new(v[0], v[1], v[2]), max: Vector3::new(v[3], v[4], v[5]) }
}

/// F5: `fieldDiffExtents` for a scalar and a vector pair, internal and per patch, with a tie and an empty-mask step.
#[test]
fn field_diff_extents_matches_the_real_class() {
    let r = load();
    let m = mesh();
    let c0 = Vector3::new(0.1, 0.2, 0.3);
    let up = r.series("fde", "fieldDiffExtents.dat");
    // per step two data rows: the scalar field A, then the vector field VA; each 5 boxes x 6 numbers
    let mut t = Tally::default();
    let mut ge_diff = 0usize;
    let mut empty_steps = 0usize;
    // the series() filter keeps the single-number time lines too: they and the box rows alternate
    let boxes: Vec<&Vec<f64>> = up.iter().filter(|v| v.len() == 30).collect();
    assert_eq!(boxes.len(), 2 * N_STEPS);
    for n in 1..=N_STEPS {
        let (a, b) = (vol_scalar(&r, &m, n, "A"), vol_scalar(&r, &m, n, "B"));
        let (va, vb) = (vol_vector(&r, &m, n, "VA"), vol_vector(&r, &m, n, "VB"));
        for (row, vector) in [(boxes[2 * (n - 1)], false), (boxes[2 * (n - 1) + 1], true)] {
            let (cm, pm): (Vec<bool>, Vec<Vec<bool>>) = if vector {
                (cell_mask_vector(&va, &vb), (0..4).map(|p| patch_mask_vector(&va, &vb, p)).collect())
            } else {
                (cell_mask(&a, &b), (0..4).map(|p| patch_mask(&a, &b, p)).collect())
            };
            let mut port = vec![internal_extents(&m, &cm, c0)];
            for (p, mask) in pm.iter().enumerate() {
                port.push(patch_extents(&m, p, mask, c0));
            }
            for (k, e) in port.iter().enumerate() {
                let u = box_of(&row[6 * k..6 * k + 6]);
                for (c, (pp, uu)) in [(e.min.x, u.min.x), (e.min.y, u.min.y), (e.min.z, u.min.z), (e.max.x, u.max.x), (e.max.y, u.max.y), (e.max.z, u.max.z)]
                    .iter()
                    .enumerate()
                {
                    t.cmp(&format!("step {n} vector {vector} box {k} comp {c}"), *pp, *uu);
                }
            }
            if !vector {
                // control: `>=` instead of `>`
                let ge: Vec<bool> = (0..a.internal.len()).map(|c| a.internal[c] - b.internal[c] >= 0.0).collect();
                let e = internal_extents(&m, &ge, c0);
                let u = box_of(&row[0..6]);
                if e.min.x.to_bits() != u.min.x.to_bits()
                    || e.min.y.to_bits() != u.min.y.to_bits()
                    || e.min.z.to_bits() != u.min.z.to_bits()
                    || e.max.x.to_bits() != u.max.x.to_bits()
                    || e.max.y.to_bits() != u.max.y.to_bits()
                    || e.max.z.to_bits() != u.max.z.to_bits()
                {
                    ge_diff += 1;
                }
                if !cm.iter().any(|x| *x) {
                    empty_steps += 1;
                }
            }
        }
    }
    t.report("F5 fieldDiffExtents");
    println!("F5 control: `>=` mask differs in {ge_diff} of {N_STEPS} internal scalar boxes; empty-mask steps (scalar): {empty_steps}");
    assert!(t.identical(), "fieldDiffExtents not bit-identical: {:?}", t.first_diff);
    assert!(ge_diff > 0, "control: >= must differ");
    assert!(empty_steps > 0);
}

/// F6: `stopIfMaxFieldDiff`: the step at which the run stops.
#[test]
fn stop_if_max_field_diff_stops_at_the_same_step() {
    let r = load();
    let m = mesh();
    let zero = |name: &str| {
        let boundary = PATCHES
            .iter()
            .enumerate()
            .map(|(pi, _)| {
                let vals = vec![0.0; m.patches[pi].size];
                PatchField { bc: BoundaryCondition::FixedField(Field::new(vals.clone())), values: Field::new(vals) }
            })
            .collect();
        VolField::new(name, Arc::clone(&m), Field::new(vec![0.0; m.n_cells]), boundary)
    };
    let mut first_up = HashMap::new();
    let mut first_port = HashMap::new();
    let mut first_cells_only = HashMap::new();
    let mut t = Tally::default();
    for (name, field) in [("stopCells", "SC1"), ("stopBnd", "SB1")] {
        for n in 1..=N_STEPS {
            let f1 = vol_scalar(&r, &m, n, field);
            let f2 = zero("zero");
            let d = evaluate_stop_if_max_field_diff(&f1, &f2);
            let c = evaluate_stop_if_max_field_diff_cells_only(&f1, &f2);
            let up_stopped = !r.stop[&(name.to_string(), n)];
            t.cmp(&format!("{name} step {n} stop"), if d.stop { 1.0 } else { 0.0 }, if up_stopped { 1.0 } else { 0.0 });
            if up_stopped {
                first_up.entry(name).or_insert(n);
            }
            if d.stop {
                first_port.entry(name).or_insert(n);
            }
            if c.stop {
                first_cells_only.entry(name).or_insert(n);
            }
        }
    }
    t.report("F6 stopIfMaxFieldDiff decisions");
    println!("F6 first stop step: upstream {first_up:?}, port {first_port:?}, cells-only control {first_cells_only:?}");
    assert!(t.identical());
    assert_eq!(first_up.get("stopBnd"), Some(&9));
    assert_eq!(first_up.get("stopCells"), Some(&6));
    assert_ne!(first_cells_only.get("stopBnd"), first_up.get("stopBnd"), "control: the cells-only form must miss the boundary stop");
}

/// F7: `fieldIntegralToFMU`, up to the FMU boundary: upstream's own statement on a real cell zone.
#[test]
fn field_integral_zone_sum_matches_the_upstream_statement() {
    let r = load();
    let m = mesh();
    let cells = r.order.get("cellzone.czA").unwrap().clone();
    let mut t = Tally::default();
    let mut rev_diff = 0usize;
    for n in 1..=N_STEPS {
        let a = vol_scalar(&r, &m, n, "A");
        let port = volume_integral_over_cells(&m, &a, &cells);
        let up = r.out[&(n, "fmu_integral".to_string())];
        t.cmp(&format!("step {n}"), port, up);
        let mut rev = cells.clone();
        rev.reverse();
        if volume_integral_over_cells(&m, &a, &rev).to_bits() != up.to_bits() {
            rev_diff += 1;
        }
    }
    t.report("F7 fieldIntegralToFMU (statement only)");
    println!("F7 control: reversed zone order differs in {rev_diff} of {N_STEPS} steps (informational)");
    assert!(t.identical());
}
