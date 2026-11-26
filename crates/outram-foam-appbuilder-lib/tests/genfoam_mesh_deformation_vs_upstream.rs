// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `deformMesh` on the 3D_SmallESFR meshes, against GeN-Foam / OpenFOAM-ESI's own code (gh:#1784, round 26)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506. `upstream_driver/mesh_deform_dump/meshDeformDump.C` applies
//! upstream's `deformMesh()` (`volPointInterpolation::interpolate`, then `fvMesh::movePoints(originalPoints_ + pointDisp)`) to
//! `neutroRegion` and `fluidRegion` (the five upstream lines, verbatim) and, through the REAL compiled
//! `solvers::legacyThermoMechanics::deformMesh()`, to `thermoMechanicalRegion`, for two steps of a closed-form displacement
//! (`+ - * /` only; the same expressions are evaluated below). Reference: `refs/mesh_deform_esfr.ref.txt.gz`: every 13th item
//! of the interpolated point displacement, the moved points, face centres, face area vectors, cell centres and cell volumes
//! (17 digits), and the WHOLE `pointCells` of each mesh. The complete dump (73 MB) is too large to commit: when the
//! environment variable `GENFOAM_MESH_DEFORM_DUMP` names it (`scripts/build_and_run_mesh_deform_dump.sh 2 <file>`), every
//! item is compared too, and the tallies below say which.
//!
//! The pass criterion is bit identity (round-off allowance 1e-9 only with an explained cause; none is claimed).
//! Predictions (committed before the first run) and results: README, "Round 26".

#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::multi_region::mesh_deformation::{
    parse_disp_patch_conditions, DeformableMesh, MeshDeformationError, PointCellsOrder, VolDisplacement,
};
use outram_foam_basic_lib::io::dict::FoamFile;
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

/// Region -> (polyMesh dir with the .gz files, `0/<region>/disp` file).
fn region_sources(region: &str) -> (PathBuf, PathBuf) {
    let v = vv_dir();
    match region {
        "thermoMechanicalRegion" => (
            v.join("cases/legacy_tm_esfr/constant/thermoMechanicalRegion/polyMesh"),
            v.join("cases/legacy_tm_esfr/0/thermoMechanicalRegion/disp"),
        ),
        r => (v.join(format!("cases/mesh_deform_esfr/constant/{r}/polyMesh")), v.join(format!("cases/mesh_deform_esfr/0/{r}/disp"))),
    }
}

/// Stage the polyMesh gunzipped and read it.
fn read_region(region: &str, order: PointCellsOrder) -> (DeformableMesh, Vec<outram_foam_appbuilder_lib::genfoam::multi_region::mesh_deformation::DispPatchCondition>) {
    let (poly, disp) = region_sources(region);
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1784_{region}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    for f in ["points", "faces", "owner", "neighbour"] {
        std::fs::write(root.join(f), gunzip(&poly.join(format!("{f}.gz")))).unwrap();
    }
    std::fs::copy(poly.join("boundary"), root.join("boundary")).unwrap();
    let m = DeformableMesh::read(&root, order).unwrap_or_else(|e| panic!("{region}: {e}"));
    let dict = FoamFile::parse(&std::fs::read_to_string(&disp).unwrap()).unwrap().dict;
    let patches = m.mesh().patches.clone();
    let cond = parse_disp_patch_conditions(&dict, &patches).unwrap_or_else(|e| panic!("{region}: {e}"));
    (m, cond)
}

/// The driver's `setDisp`, expression for expression, on the ORIGINAL cell centres.
fn closed_form(c0: &[Vector3], step: usize) -> Vec<Vector3> {
    let n = step as f64;
    c0.iter()
        .map(|c| {
            let (x, y, z) = (c.x, c.y, c.z);
            let s = (z + 0.2) / 2.51;
            let r2 = x * x + y * y;
            Vector3::new(
                1.0e-3 * n * (x * s + 0.05 * y * (1.0 - s)),
                1.0e-3 * n * (y * s - 0.05 * x * s * s),
                2.0e-3 * n * s * (1.0 + 0.3 * r2),
            )
        })
        .collect()
}

/// One field of the reference: component count, item count, and the (item index, values) it holds.
struct RefField {
    nc: usize,
    n: usize,
    items: Vec<(usize, Vec<f64>)>,
}

struct Reference {
    fields: HashMap<(usize, String), RefField>,
    point_cells: HashMap<String, Vec<Vec<usize>>>,
    state: HashMap<String, String>,
}

fn parse_reference(path: &Path) -> Reference {
    let text = String::from_utf8(gunzip(path)).expect("utf8");
    let mut r = Reference { fields: HashMap::new(), point_cells: HashMap::new(), state: HashMap::new() };
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("S") => {
                let step: usize = it.next().unwrap().parse().unwrap();
                let key = it.next().unwrap().to_string();
                let nc: usize = it.next().unwrap().parse().unwrap();
                let n: usize = it.next().unwrap().parse().unwrap();
                let _stride = it.next().unwrap();
                let items = it
                    .map(|t| {
                        let (i, v) = t.split_once(':').unwrap();
                        (i.parse().unwrap(), v.split(',').map(|x| x.parse::<f64>().unwrap()).collect())
                    })
                    .collect();
                r.fields.insert((step, key), RefField { nc, n, items });
            }
            Some("P") => {
                let region = it.next().unwrap().to_string();
                let v: Vec<usize> = it.map(|t| t.parse().unwrap()).collect();
                let (mut at, mut lists) = (0, Vec::new());
                while at < v.len() {
                    lists.push(v[at + 1..at + 1 + v[at]].to_vec());
                    at += 1 + v[at];
                }
                r.point_cells.insert(region, lists);
            }
            Some("I") => {
                let region = it.next().unwrap().to_string();
                r.state.insert(region, it.collect::<Vec<_>>().join(" "));
            }
            _ => {}
        }
    }
    r
}

/// The full dump, when the environment names it: same keys, every item.
fn full_dump() -> Option<Reference> {
    let path = std::env::var("GENFOAM_MESH_DEFORM_DUMP").ok()?;
    let text = std::fs::read_to_string(path).ok()?;
    let nc = |k: &str| if k.ends_with(":V") { 1 } else { 3 };
    let mut r = Reference { fields: HashMap::new(), point_cells: HashMap::new(), state: HashMap::new() };
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if it.next() == Some("F") {
            let step: usize = it.next().unwrap().parse().unwrap();
            let key = it.next().unwrap().to_string();
            let vals: Vec<f64> = it.map(|t| t.parse().unwrap()).collect();
            let c = nc(&key);
            let items = vals.chunks(c).enumerate().map(|(i, v)| (i, v.to_vec())).collect();
            r.fields.insert((step, key), RefField { nc: c, n: vals.len() / c, items });
        }
    }
    Some(r)
}

/// Tally of one quantity.
#[derive(Debug, Default, Clone)]
struct Tally {
    compared: usize,
    exact: usize,
    worst_abs: f64,
    scale: f64,
    first_diff: Option<(usize, usize)>,
}

impl Tally {
    fn rel(&self) -> f64 {
        if self.scale > 0.0 {
            self.worst_abs / self.scale
        } else {
            self.worst_abs
        }
    }
}

fn compare(port: &[f64], nc: usize, r: &RefField) -> Tally {
    let mut t = Tally::default();
    assert_eq!(port.len(), r.n * nc, "item count");
    assert_eq!(r.nc, nc);
    for (i, vals) in &r.items {
        for (k, &rv) in vals.iter().enumerate() {
            let pv = port[i * nc + k];
            t.compared += 1;
            t.scale = t.scale.max(rv.abs());
            if pv == rv {
                t.exact += 1;
            } else {
                t.worst_abs = t.worst_abs.max((pv - rv).abs());
                t.first_diff.get_or_insert((*i, k));
            }
        }
    }
    t
}

fn flat(v: &[Vector3]) -> Vec<f64> {
    v.iter().flat_map(|p| [p.x, p.y, p.z]).collect()
}

/// Run both steps on a region and tally every quantity against `reference` (sampled or full).
fn run_region(region: &str, reference: &Reference, order: PointCellsOrder, label: &str) -> Vec<(String, Tally)> {
    let (mut m, cond) = read_region(region, order);
    let c0 = m.mesh().cell_centres.clone();
    let mut out = Vec::new();
    for step in 1..=2usize {
        let disp = VolDisplacement::from_cells(&m, closed_form(&c0, step), &cond).unwrap();
        let pd = m.deform_mesh(&disp).unwrap();
        let g = m.mesh().clone();
        let quantities: [(&str, Vec<f64>, usize); 6] = [
            ("pointDisp", flat(&pd), 3),
            ("points", flat(m.points()), 3),
            ("Cf", flat(&g.face_centres), 3),
            ("Sf", flat(&g.face_area_vectors), 3),
            ("C", flat(&g.cell_centres), 3),
            ("V", g.cell_volumes.clone(), 1),
        ];
        for (name, vals, nc) in quantities {
            let key = (step, format!("{region}:{name}"));
            let t = compare(&vals, nc, &reference.fields[&key]);
            println!(
                "{label} step {step} {region:>22}:{name:<9} compared {:>7} exact {:>7} worst rel {:.3e} first diff {:?}",
                t.compared,
                t.exact,
                t.rel(),
                t.first_diff
            );
            out.push((format!("{step}:{region}:{name}"), t));
        }
    }
    out
}

const REGIONS: [&str; 3] = ["neutroRegion", "fluidRegion", "thermoMechanicalRegion"];

fn reference() -> Reference {
    parse_reference(&vv_dir().join("refs/mesh_deform_esfr.ref.txt.gz"))
}

/// R1: the `pointCells` of ESI (whole lists) equal the cell-ascending order, and differ from the point-face walk.
#[test]
fn point_cells_order_matches_esi() {
    let r = reference();
    for region in REGIONS {
        let (m, _) = read_region(region, PointCellsOrder::CellAscending);
        let up = &r.point_cells[region];
        assert_eq!(m.topology().point_cells(), up.as_slice(), "{region}: pointCells differ from ESI's");
        println!("{region}: pointCells of {} points identical to ESI's (state before first use: {})", up.len(), r.state[region]);
        let (w, _) = read_region(region, PointCellsOrder::PointFaceWalk);
        assert_ne!(w.topology().point_cells(), up.as_slice(), "{region}: the point-face walk must be a different order here");
    }
}

/// R2-R6: every quantity of every region and step against the committed (sampled) reference, and the full dump when named.
#[test]
fn deform_mesh_is_bit_identical_to_upstream() {
    let r = reference();
    let mut worst: Vec<(String, Tally)> = Vec::new();
    for region in REGIONS {
        worst.extend(run_region(region, &r, PointCellsOrder::CellAscending, "sampled"));
    }
    if let Some(full) = full_dump() {
        for region in REGIONS {
            worst.extend(run_region(region, &full, PointCellsOrder::CellAscending, "FULL   "));
        }
    } else {
        println!("GENFOAM_MESH_DEFORM_DUMP not set: only the committed every-13th-item reference was compared");
    }
    for (name, t) in &worst {
        assert_eq!(t.exact, t.compared, "{name}: {} of {} values differ (worst rel {:.3e}, first {:?})", t.compared - t.exact, t.compared, t.rel(), t.first_diff);
    }
}

/// R7: magnitudes and signs of the closed-form field on the neutronics mesh.
#[test]
fn magnitude_and_sign_of_the_deformation() {
    let (mut m, cond) = read_region("neutroRegion", PointCellsOrder::CellAscending);
    let c0 = m.mesh().cell_centres.clone();
    let v0 = m.mesh().cell_volumes.clone();
    let mut max_pd = [0.0_f64; 2];
    let mut max_dv = [0.0_f64; 2];
    let mut mean_dv = [0.0_f64; 2];
    for step in 1..=2usize {
        let disp = VolDisplacement::from_cells(&m, closed_form(&c0, step), &cond).unwrap();
        let pd = m.deform_mesh(&disp).unwrap();
        max_pd[step - 1] = pd.iter().map(|p| p.mag()).fold(0.0, f64::max);
        let dv: Vec<f64> = m.mesh().cell_volumes.iter().zip(&v0).map(|(a, b)| (a - b) / b).collect();
        max_dv[step - 1] = dv.iter().cloned().fold(f64::MIN, f64::max);
        mean_dv[step - 1] = dv.iter().sum::<f64>() / dv.len() as f64;
        println!("step {step}: max |pointDisp| {:.4e} m, max dV/V {:.3e}, mean dV/V {:.3e}", max_pd[step - 1], max_dv[step - 1], mean_dv[step - 1]);
    }
    assert!(mean_dv[0] > 0.0 && mean_dv[1] > 0.0, "the field is a dilatation: mean volume change must be positive");
    assert!(max_pd[1] > 1.5 * max_pd[0] && max_pd[1] < 2.5 * max_pd[0], "step 2 is about twice step 1 (not cumulative)");
}

/// Control 1: the comparison can fail. One last-bit change in one reference value is found, in the right place.
#[test]
fn control_a_one_ulp_change_is_detected() {
    let r = reference();
    let (mut m, cond) = read_region("neutroRegion", PointCellsOrder::CellAscending);
    let c0 = m.mesh().cell_centres.clone();
    let disp = VolDisplacement::from_cells(&m, closed_form(&c0, 1), &cond).unwrap();
    m.deform_mesh(&disp).unwrap();
    let field = &r.fields[&(1, "neutroRegion:V".to_string())];
    let mut bent = RefField { nc: field.nc, n: field.n, items: field.items.clone() };
    let (i, v) = bent.items[7].clone();
    bent.items[7] = (i, vec![f64::from_bits(v[0].to_bits() + 1)]);
    let t = compare(&m.mesh().cell_volumes, 1, &bent);
    assert_eq!(t.compared - t.exact, 1, "exactly the doctored value");
    assert_eq!(t.first_diff, Some((i, 0)));
    println!("control A: 1 ulp on cell {i} found; worst rel {:.3e}", t.rel());
}

/// Control 2: weights of the ORIGINAL geometry, applied to step 2 (what a port that forgot the mesh object's `movePoints()` would
/// do), do NOT reproduce upstream; the comparison sees it.
#[test]
fn control_b_stale_weights_do_not_reproduce_step_2() {
    let r = reference();
    let (mut m, cond) = read_region("neutroRegion", PointCellsOrder::CellAscending);
    let c0 = m.mesh().cell_centres.clone();
    let stale = m.weights();
    let d1 = VolDisplacement::from_cells(&m, closed_form(&c0, 1), &cond).unwrap();
    m.deform_mesh(&d1).unwrap();
    let d2 = VolDisplacement::from_cells(&m, closed_form(&c0, 2), &cond).unwrap();
    let pd_stale = stale.interpolate(m.topology(), &d2);
    let t = compare(&flat(&pd_stale), 3, &r.fields[&(2, "neutroRegion:pointDisp".to_string())]);
    println!("control B: stale weights: exact {} of {}, worst rel {:.3e}", t.exact, t.compared, t.rel());
    assert!(t.exact < t.compared / 2, "most interior points must differ");
    assert!(t.rel() > 1e-9, "and by more than round-off: {:.3e}", t.rel());
    let fresh = m.interpolate_displacement(&d2);
    let t2 = compare(&flat(&fresh), 3, &r.fields[&(2, "neutroRegion:pointDisp".to_string())]);
    assert_eq!(t2.exact, t2.compared, "the weights of the moved geometry reproduce it");
}

/// R8: a polyMesh with an unported patch type is refused by name (read from the raw `type`, which `PatchKind` would hide).
#[test]
fn unported_patch_types_are_refused_by_name() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("gh1784_refusal");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("boundary"),
        "FoamFile{version 2.0;format ascii;class polyBoundaryMesh;object boundary;}\n1\n(\n a\n {\n type cyclicAMI;\n nFaces 1;\n startFace 0;\n }\n)\n",
    )
    .unwrap();
    match DeformableMesh::read(&root, PointCellsOrder::CellAscending) {
        Err(MeshDeformationError::Refused { what, .. }) => assert!(what.contains("cyclicAMI"), "{what}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}
