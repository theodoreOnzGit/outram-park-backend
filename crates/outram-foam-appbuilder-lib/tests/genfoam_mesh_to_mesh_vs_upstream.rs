// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `meshToMesh` `imCellVolumeWeight` on the 3D_SmallESFR meshes, against OpenFOAM-ESI's own code (gh:#651, round 28)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506. `upstream_driver/mesh_to_mesh_dump/meshToMeshDump.C` builds
//! `meshToMesh(A, B, imCellVolumeWeight, pmAABB, false)` exactly as `meshHandler.C:192-201` does, for the five (A, B) of the
//! shipped `multiRegionCouplingDict` (fluid/neutro/thermoMechanical, 23322/23322/23340 cells) and for `neutroRegion` against a
//! copy moved by a closed-form point displacement through `fvMesh::movePoints` (real partial overlap on warped hexahedra), both
//! ways. For each object it dumps the normalised addressing and weights, the same with `normalise = false` (raw overlap
//! volumes), `V()`, and the cell fields produced by `mapTgtToSrc` / `mapSrcToTgt` with `plusEqOp` on closed-form
//! (`+ - * /` only) fields with a pre-existing target value. This test builds the same objects with
//! [`CellVolumeWeightAddressing`] on [`OverlapMesh`]es read from the committed meshes and compares item by item.
//!
//! Reference: `refs/mesh_to_mesh_esfr.ref.txt.gz`, a sample (every 211th cell, the last, and, in a list where 99 % of the cells
//! have one entry count, every cell that does not) of the full 107 MB dump (`scripts/subsample_mesh_to_mesh_dump.py`). When `GENFOAM_MESH_TO_MESH_DUMP` names the full dump
//! (`scripts/build_and_run_mesh_to_mesh_dump.sh <file>`), every cell is compared.
//!
//! Pass criterion: bit identity (the 1e-9 allowance only with an explained cause; none is claimed).
//! Predictions (committed before the first run) and results: README, "Round 28".

#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::multi_region::cell_overlap::{CellVolumeWeightAddressing, OverlapMesh};
use outram_foam_appbuilder_lib::genfoam::multi_region::mesh_deformation::{DeformableMesh, PointCellsOrder};
use outram_foam_appbuilder_lib::genfoam::multi_region::mesh_to_mesh::{MapCombine, MappingMethod, MeshToMesh};
use outram_foam_basic_lib::fields::vol_field::VolScalarField;
use outram_foam_basic_lib::prelude::Vector3;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> Vec<u8> {
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
    decompress_to_vec(&bytes[at..bytes.len() - 8]).unwrap_or_else(|_| panic!("inflate {}", path.display()))
}

/// Region name -> its polyMesh directory (gzipped lists) among the committed cases.
fn poly_dir(region: &str) -> PathBuf {
    let v = vv_dir();
    match region {
        "thermoMechanicalRegion" => v.join("cases/legacy_tm_esfr/constant/thermoMechanicalRegion/polyMesh"),
        r => v.join(format!("cases/mesh_deform_esfr/constant/{r}/polyMesh")),
    }
}

fn read_region(region: &str) -> DeformableMesh {
    let poly = poly_dir(region);
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh651_{region}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    for f in ["points", "faces", "owner", "neighbour"] {
        std::fs::write(root.join(f), gunzip(&poly.join(format!("{f}.gz")))).unwrap();
    }
    std::fs::copy(poly.join("boundary"), root.join("boundary")).unwrap();
    DeformableMesh::read(&root, PointCellsOrder::CellAscending).unwrap_or_else(|e| panic!("{region}: {e}"))
}

/// `neutroRegion` moved by the driver's closed-form point displacement (the expressions are copied from `meshToMeshDump.C`).
fn moved_neutro() -> DeformableMesh {
    let mut m = read_region("neutroRegion");
    let q: Vec<Vector3> = m
        .points()
        .iter()
        .map(|p| {
            let (x, y, z) = (p.x, p.y, p.z);
            Vector3::new(
                x + 0.03 * (1.0 + 0.5 * y * z - 0.2 * x),
                y + 0.025 * (1.0 - 0.4 * x * z + 0.3 * y),
                z + 0.02 * (1.0 + 0.3 * x * y - 0.1 * z),
            )
        })
        .collect();
    m.move_points(q).unwrap();
    m
}

// the driver's closed-form fields
fn f_s(c: Vector3) -> f64 {
    let (x, y, z) = (c.x, c.y, c.z);
    300.0 + 100.0 * x - 40.0 * y + 25.0 * z + 10.0 * x * y * z
}
fn g_s(c: Vector3) -> f64 {
    let (x, y, z) = (c.x, c.y, c.z);
    5.0 + 3.0 * x - 2.0 * y + z / 7.0
}
fn u_v(c: Vector3) -> Vector3 {
    let (x, y, z) = (c.x, c.y, c.z);
    Vector3::new(x * y + 1.0, z - x / 3.0, y * z * x - 2.0)
}
fn w_v(c: Vector3) -> Vector3 {
    let (x, y, z) = (c.x, c.y, c.z);
    Vector3::new(0.5 + x, y / 5.0, 2.0 - z)
}

type ListEntry = (Vec<usize>, Vec<f64>);

#[derive(Default)]
struct Reference {
    values: HashMap<String, f64>,
    lists: HashMap<(String, String), HashMap<usize, ListEntry>>,
    /// (obj, name) -> (ncomp, n items, idx -> components)
    fields: HashMap<(String, String), (usize, usize, HashMap<usize, [f64; 3]>)>,
}

fn parse_reference(text: &str) -> Reference {
    let mut r = Reference::default();
    for line in text.lines() {
        let t: Vec<&str> = line.split(' ').collect();
        match t[0] {
            "V" => {
                r.values.insert(t[1].to_string(), t[2].parse().unwrap());
            }
            "L" => {
                let cell: usize = t[3].parse().unwrap();
                let n: usize = t[4].parse().unwrap();
                let mut a = Vec::with_capacity(n);
                let mut w = Vec::with_capacity(n);
                for k in 0..n {
                    a.push(t[5 + 2 * k].parse().unwrap());
                    w.push(t[6 + 2 * k].parse().unwrap());
                }
                r.lists.entry((t[1].to_string(), t[2].to_string())).or_default().insert(cell, (a, w));
            }
            "S" => {
                let nc: usize = t[3].parse().unwrap();
                let n: usize = t[4].parse().unwrap();
                let e = r.fields.entry((t[1].to_string(), t[2].to_string())).or_insert((nc, n, HashMap::new()));
                for item in &t[5..] {
                    let (i, vs) = item.split_once(':').unwrap();
                    let mut c = [0.0; 3];
                    for (k, s) in vs.split(',').enumerate() {
                        c[k] = s.parse().unwrap();
                    }
                    e.2.insert(i.parse().unwrap(), c);
                }
            }
            "M" => {
                let nc = if t[2] == "tgtToSrcVector" { 3 } else { 1 };
                let vals: Vec<f64> = t[3..].iter().map(|s| s.parse().unwrap()).collect();
                let n = vals.len() / nc;
                let mut m = HashMap::new();
                for i in 0..n {
                    let mut c = [0.0; 3];
                    c[..nc].copy_from_slice(&vals[i * nc..(i + 1) * nc]);
                    m.insert(i, c);
                }
                r.fields.insert((t[1].to_string(), t[2].to_string()), (nc, n, m));
            }
            _ => {}
        }
    }
    r
}

fn reference() -> &'static (Reference, bool) {
    static R: OnceLock<(Reference, bool)> = OnceLock::new();
    R.get_or_init(|| {
        if let Ok(p) = std::env::var("GENFOAM_MESH_TO_MESH_DUMP") {
            let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"));
            (parse_reference(&text), true)
        } else {
            let bytes = gunzip(&vv_dir().join("refs/mesh_to_mesh_esfr.ref.txt.gz"));
            (parse_reference(&String::from_utf8(bytes).unwrap()), false)
        }
    })
}

#[derive(Debug, Default, Clone, Copy)]
struct Tally {
    compared: usize,
    identical: usize,
    worst_rel: f64,
    shape_mismatch: usize,
}

impl Tally {
    fn add(&mut self, mine: f64, theirs: f64) {
        self.compared += 1;
        if mine.to_bits() == theirs.to_bits() || mine == theirs {
            self.identical += 1;
        } else {
            let rel = (mine - theirs).abs() / mine.abs().max(theirs.abs());
            if rel > self.worst_rel {
                self.worst_rel = rel;
            }
        }
    }
    fn merge(&mut self, o: Tally) {
        self.compared += o.compared;
        self.identical += o.identical;
        self.shape_mismatch += o.shape_mismatch;
        if o.worst_rel > self.worst_rel {
            self.worst_rel = o.worst_rel;
        }
    }
    fn bit_identical(&self) -> bool {
        self.compared > 0 && self.identical == self.compared && self.shape_mismatch == 0
    }
}

fn compare_list(r: &Reference, obj: &str, name: &str, addr: &[Vec<usize>], wght: &[Vec<f64>]) -> (Tally, Tally) {
    let mut at = Tally::default();
    let mut wt = Tally::default();
    let list = r.lists.get(&(obj.to_string(), name.to_string())).unwrap_or_else(|| panic!("no reference list {obj} {name}"));
    for (cell, (ra, rw)) in list {
        if addr[*cell] != *ra {
            at.shape_mismatch += 1;
            wt.shape_mismatch += 1;
            continue;
        }
        at.compared += 1;
        at.identical += 1;
        for (k, w) in rw.iter().enumerate() {
            wt.add(wght[*cell][k], *w);
        }
    }
    (at, wt)
}

fn compare_field(r: &Reference, obj: &str, name: &str, mine: &[[f64; 3]], ncomp: usize) -> Tally {
    let mut t = Tally::default();
    let (nc, n, items) = r.fields.get(&(obj.to_string(), name.to_string())).unwrap_or_else(|| panic!("no reference field {obj} {name}"));
    assert_eq!((*nc, *n), (ncomp, mine.len()), "{obj} {name}: shape");
    for (i, c) in items {
        for k in 0..ncomp {
            t.add(mine[*i][k], c[k]);
        }
    }
    t
}

/// Everything the port computes for one upstream object `meshToMesh(A, B, ...)`.
struct Mine {
    obj: String,
    raw: CellVolumeWeightAddressing,
    norm: CellVolumeWeightAddressing,
    scalar_t2s: Vec<[f64; 3]>,
    vector_t2s: Vec<[f64; 3]>,
    scalar_s2t: Vec<[f64; 3]>,
}

fn build(obj: &str, a: &OverlapMesh, b: &OverlapMesh) -> Mine {
    let raw = CellVolumeWeightAddressing::calculate(a, b);
    let mut norm = raw.clone();
    norm.normalise();
    let ca = a.cell_centres();
    let cb = b.cell_centres();
    let fb: Vec<f64> = cb.iter().map(|&c| f_s(c)).collect();
    let mut ra: Vec<f64> = ca.iter().map(|&c| g_s(c)).collect();
    norm.map_tgt_to_src_scalar(&fb, &mut ra);
    let ub: Vec<Vector3> = cb.iter().map(|&c| u_v(c)).collect();
    let mut rav: Vec<Vector3> = ca.iter().map(|&c| w_v(c)).collect();
    norm.map_tgt_to_src_vector(&ub, &mut rav);
    let fa: Vec<f64> = ca.iter().map(|&c| f_s(c)).collect();
    let mut rb: Vec<f64> = cb.iter().map(|&c| g_s(c)).collect();
    norm.map_src_to_tgt_scalar(&fa, &mut rb);
    Mine {
        obj: obj.to_string(),
        raw,
        norm,
        scalar_t2s: ra.iter().map(|&x| [x, 0.0, 0.0]).collect(),
        vector_t2s: rav.iter().map(|v| [v.x, v.y, v.z]).collect(),
        scalar_s2t: rb.iter().map(|&x| [x, 0.0, 0.0]).collect(),
    }
}

struct Meshes {
    fluid: OverlapMesh,
    neutro: OverlapMesh,
    tm: OverlapMesh,
    moved: OverlapMesh,
    neutro_dm: DeformableMesh,
    moved_dm: DeformableMesh,
}

fn meshes() -> &'static Meshes {
    static M: OnceLock<Meshes> = OnceLock::new();
    M.get_or_init(|| {
        let fluid = read_region("fluidRegion");
        let neutro = read_region("neutroRegion");
        let tm = read_region("thermoMechanicalRegion");
        let moved = moved_neutro();
        Meshes {
            fluid: OverlapMesh::from_deformable(&fluid).unwrap(),
            neutro: OverlapMesh::from_deformable(&neutro).unwrap(),
            tm: OverlapMesh::from_deformable(&tm).unwrap(),
            moved: OverlapMesh::from_deformable(&moved).unwrap(),
            neutro_dm: neutro,
            moved_dm: moved,
        }
    })
}

fn all_objects() -> &'static Vec<Mine> {
    static O: OnceLock<Vec<Mine>> = OnceLock::new();
    O.get_or_init(|| {
        let m = meshes();
        let t0 = std::time::Instant::now();
        let out = vec![
            build("fluidRegion__neutroRegion", &m.fluid, &m.neutro),
            build("neutroRegion__fluidRegion", &m.neutro, &m.fluid),
            build("neutroRegion__thermoMechanicalRegion", &m.neutro, &m.tm),
            build("thermoMechanicalRegion__fluidRegion", &m.tm, &m.fluid),
            build("thermoMechanicalRegion__neutroRegion", &m.tm, &m.neutro),
            build("neutroRegion__neutroMovedRegion", &m.neutro, &m.moved),
            build("neutroMovedRegion__neutroRegion", &m.moved, &m.neutro),
        ];
        eprintln!("seven objects built in {:.1} s", t0.elapsed().as_secs_f64());
        out
    })
}

/// Compare one object with the reference; returns (addresses, weights, raw weights, V, fields).
fn compare_object(mine: &Mine) -> (Tally, Tally, Tally, Tally, Tally) {
    let (r, _) = reference();
    let o = &mine.obj;
    let mut addr = Tally::default();
    let mut wght = Tally::default();
    let mut raw = Tally::default();
    let mut fields = Tally::default();
    let mut vt = Tally::default();
    for (name, lists) in [
        ("srcToTgtAddr", (&mine.norm.src_to_tgt_addr, &mine.norm.src_to_tgt_wght)),
        ("tgtToSrcAddr", (&mine.norm.tgt_to_src_addr, &mine.norm.tgt_to_src_wght)),
    ] {
        let (a, w) = compare_list(r, o, name, lists.0, lists.1);
        addr.merge(a);
        wght.merge(w);
    }
    for (name, lists) in [
        ("srcToTgtRaw", (&mine.raw.src_to_tgt_addr, &mine.raw.src_to_tgt_wght)),
        ("tgtToSrcRaw", (&mine.raw.tgt_to_src_addr, &mine.raw.tgt_to_src_wght)),
    ] {
        let (a, w) = compare_list(r, o, name, lists.0, lists.1);
        addr.merge(a);
        raw.merge(w);
    }
    vt.add(mine.norm.v, r.values[o]);
    vt.add(mine.raw.v, r.values[&format!("{o}__raw")]);
    fields.merge(compare_field(r, o, "tgtToSrcScalar", &mine.scalar_t2s, 1));
    fields.merge(compare_field(r, o, "tgtToSrcVector", &mine.vector_t2s, 3));
    fields.merge(compare_field(r, o, "srcToTgtScalar", &mine.scalar_s2t, 1));
    (addr, wght, raw, vt, fields)
}

fn report_and_gate(objs: &[&str]) {
    let (_, full) = reference();
    for mine in all_objects().iter().filter(|m| objs.contains(&m.obj.as_str())) {
        let (a, w, raw, v, f) = compare_object(mine);
        let sizes = (
            mine.norm.src_to_tgt_addr.iter().map(Vec::len).max().unwrap_or(0),
            mine.norm.src_to_tgt_addr.iter().filter(|x| x.is_empty()).count(),
            mine.norm.tgt_to_src_addr.iter().filter(|x| x.is_empty()).count(),
        );
        eprintln!(
            "{} ({}): addr {}/{} weights {}/{} (worst {:.2e}) raw {}/{} (worst {:.2e}) V {}/{} (worst {:.2e}) fields {}/{} (worst {:.2e}); max entries {}, empty src cells {}, empty tgt cells {}",
            mine.obj, if *full { "full dump" } else { "sampled" },
            a.identical, a.compared, w.identical, w.compared, w.worst_rel, raw.identical, raw.compared, raw.worst_rel,
            v.identical, v.compared, v.worst_rel, f.identical, f.compared, f.worst_rel, sizes.0, sizes.1, sizes.2
        );
        assert!(a.bit_identical(), "{}: addressing differs ({a:?})", mine.obj);
        assert!(w.bit_identical(), "{}: normalised weights differ ({w:?})", mine.obj);
        assert!(raw.bit_identical(), "{}: raw overlap volumes differ ({raw:?})", mine.obj);
        assert!(v.bit_identical(), "{}: V differs ({v:?})", mine.obj);
        assert!(f.bit_identical(), "{}: mapped fields differ ({f:?})", mine.obj);
    }
}

/// The five `meshToMesh` objects of the shipped 3D_SmallESFR `multiRegionCouplingDict`: addressing (with order), normalised
/// and raw weights, `V`, and the mapped scalar/vector fields are bit-identical to ESI's. The thermoMechanical pairs also
/// pin the keep-value rule on the 18 cells that overlap nothing.
#[test]
fn shipped_esfr_objects_match_upstream() {
    report_and_gate(&[
        "fluidRegion__neutroRegion",
        "neutroRegion__fluidRegion",
        "neutroRegion__thermoMechanicalRegion",
        "thermoMechanicalRegion__fluidRegion",
        "thermoMechanicalRegion__neutroRegion",
    ]);
    let tm_src = all_objects().iter().find(|m| m.obj == "thermoMechanicalRegion__fluidRegion").unwrap();
    let empty = tm_src.norm.src_to_tgt_addr.iter().filter(|x| x.is_empty()).count();
    assert_eq!(empty, 18, "the 18 extra thermoMechanical cells overlap nothing");
}

/// Genuine partial overlap on real geometry: `neutroRegion` against a copy moved by `fvMesh::movePoints`, both ways.
#[test]
fn moved_neutro_partial_overlap_matches_upstream() {
    report_and_gate(&["neutroRegion__neutroMovedRegion", "neutroMovedRegion__neutroRegion"]);
    let o = all_objects().iter().find(|m| m.obj == "neutroRegion__neutroMovedRegion").unwrap();
    let multi = o.norm.src_to_tgt_addr.iter().filter(|x| x.len() > 1).count();
    assert!(multi > 20000, "most cells must overlap several moved cells, got {multi}");
}

/// Control 1: the comparison can fail. One weight moved by one ulp is detected, as is a wrong address.
#[test]
fn control_one_ulp_and_wrong_address_are_detected() {
    let (r, _) = reference();
    let o = all_objects().iter().find(|m| m.obj == "neutroRegion__neutroMovedRegion").unwrap();
    let list = r.lists.get(&(o.obj.clone(), "srcToTgtAddr".to_string())).unwrap();
    let cell = *list.keys().min().unwrap();
    let mut w = o.norm.src_to_tgt_wght.clone();
    w[cell][0] = f64::from_bits(w[cell][0].to_bits() + 1);
    let (a, wt) = compare_list(r, &o.obj, "srcToTgtAddr", &o.norm.src_to_tgt_addr, &w);
    assert!(a.bit_identical());
    assert!(!wt.bit_identical() && wt.worst_rel > 0.0, "a 1-ulp change must be detected: {wt:?}");
    let mut ad = o.norm.src_to_tgt_addr.clone();
    ad[cell].swap(0, 1);
    let (a2, _) = compare_list(r, &o.obj, "srcToTgtAddr", &ad, &o.norm.src_to_tgt_wght);
    assert!(a2.shape_mismatch > 0, "a reordered address list must be detected");
}

/// Control 2: the direction matters. Upstream's object `(neutro, moved)` is not `(moved, neutro)`: using one's addressing
/// against the other's reference fails.
#[test]
fn control_wrong_direction_fails() {
    let (r, _) = reference();
    let a = all_objects().iter().find(|m| m.obj == "neutroRegion__neutroMovedRegion").unwrap();
    let b = all_objects().iter().find(|m| m.obj == "neutroMovedRegion__neutroRegion").unwrap();
    let t = compare_field(r, &a.obj, "tgtToSrcScalar", &b.scalar_t2s, 1);
    assert!(!t.bit_identical() && t.worst_rel > 1.0e-9, "{t:?}");
}

/// Control 3 (the issue's old form): the pre-gh:#651 nearest-cell + global-rescale mapping, run on the SAME meshes and fields,
/// does not reproduce upstream on the partially overlapping pair, while it is (nearly) exact on a pair of identical meshes
/// where it was never wrong. Prints the worst relative difference of each.
#[test]
#[allow(deprecated)]
fn control_old_form_fails_on_partial_overlap() {
    let (r, _) = reference();
    let m = meshes();
    // field of the moved mesh onto neutro: MeshToMesh(source = moved, target = neutro)
    let old = MeshToMesh::new(m.moved_dm.mesh().clone(), m.neutro_dm.mesh().clone(), MappingMethod::NearestCellGlobalRescale).unwrap();
    let mut src = VolScalarField::uniform("f", m.moved_dm.mesh().clone(), 0.0);
    for (i, c) in m.moved_dm.mesh().cell_centres.iter().enumerate() {
        src.internal[i] = f_s(*c);
    }
    let mut out = VolScalarField::uniform("g", m.neutro_dm.mesh().clone(), 0.0);
    for (i, c) in m.neutro_dm.mesh().cell_centres.iter().enumerate() {
        out.internal[i] = g_s(*c);
    }
    old.map_scalar(&src, MapCombine::Replace, &mut out);
    let mine: Vec<[f64; 3]> = out.internal.as_slice().iter().map(|&x| [x, 0.0, 0.0]).collect();
    let t = compare_field(r, "neutroRegion__neutroMovedRegion", "tgtToSrcScalar", &mine, 1);
    eprintln!("old form vs upstream, moved pair: {}/{} identical, worst relative difference {:.3e}", t.identical, t.compared, t.worst_rel);
    assert!(t.worst_rel > 1.0e-4, "the old form must fail on partial overlap: {t:?}");

    // identical meshes (fluid and neutro have the same cells): the old form's nearest cell is the coincident cell
    let old2 = MeshToMesh::new(m.neutro_dm.mesh().clone(), m.neutro_dm.mesh().clone(), MappingMethod::NearestCellGlobalRescale).unwrap();
    let mut s2 = VolScalarField::uniform("f", m.neutro_dm.mesh().clone(), 0.0);
    for (i, c) in m.neutro_dm.mesh().cell_centres.iter().enumerate() {
        s2.internal[i] = f_s(*c);
    }
    let mut o2 = VolScalarField::uniform("g", m.neutro_dm.mesh().clone(), 0.0);
    old2.map_scalar(&s2, MapCombine::Replace, &mut o2);
    let worst = (0..s2.internal.len()).map(|i| (o2.internal[i] - s2.internal[i]).abs() / s2.internal[i].abs()).fold(0.0, f64::max);
    eprintln!("old form on identical meshes: worst relative difference from the identity {worst:.3e}");
    assert!(worst < 1.0e-12, "the old form is exact on identical meshes: {worst}");
}
