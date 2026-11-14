// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithms ported from OpenFOAM.com (ESI) v2506 (see `src/dynamic_mesh.rs`); the
// frozen hashes come from runs of that release's `dumpMeshMotion` probe.

//! Solid-body mesh motion and `meshPhi` against ESI v2506 (gh:#1380).
//!
//! **Method and results:** `verification_and_validation/dynamic_mesh_gh1380/README.md`;
//! predictions D1-D7 were committed in `PREDICTIONS.md` before any run.
//!
//! Cases: meshes `warped3d` (hex) and `dual_warped3d` (polyhedral) from
//! `tests/fixtures/esi_checkmesh/`, motions `linear`, `rotating`, `axisRotation`,
//! `oscillatingLinear` (`dynamicMeshDict`s in `verification_and_validation/
//! dynamic_mesh_gh1380/cases/`), deltaT 0.01, five steps. After each step the ESI
//! probe wrote `points`, `meshPhi`, `faceCentres`, `faceAreas`, `cellCentres`,
//! `cellVolumes` at `%.17g`; `esi_dynmesh_hashes.txt` holds the FNV-1a of the
//! IEEE-754 bit patterns per field, so a pass means every value is bit-identical.
//! Set `DYNMESH_PROBE_DIR` to a `run_esi.sh` output directory to compare the raw
//! probe text value by value as well, and to regenerate the table with the ignored
//! test `regenerate_hash_table`.

use outram_foam_basic_lib::io::dict::FoamFile;
use outram_foam_mesh::dynamic_mesh::{SolidBodyMeshMotion, SolidBodyMotion};
use outram_foam_mesh::snappy_hex_mesh::esi_snap::SnapMesh;

const MESHES: [&str; 2] = ["warped3d", "dual_warped3d"];
const MOTIONS: [&str; 4] = ["linear", "rotating", "axisRotation", "oscillatingLinear"];
const TIMES: [&str; 5] = ["0.01", "0.02", "0.03", "0.04", "0.05"];
const FIELDS: [&str; 6] = ["points", "meshPhi", "faceCentres", "faceAreas", "cellCentres", "cellVolumes"];
const DELTA_T: f64 = 0.01;

fn crate_dir() -> String {
    env!("CARGO_MANIFEST_DIR").to_string()
}

fn fnv(values: &[f64]) -> u64 {
    values.iter().fold(0xcbf29ce484222325u64, |h, v| (h ^ v.to_bits()).wrapping_mul(0x100000001b3))
}

fn flat(v: &[[f64; 3]]) -> Vec<f64> {
    v.iter().flat_map(|p| p.iter().copied()).collect()
}

fn load(mesh: &str) -> SnapMesh {
    let dir = format!("{}/tests/fixtures/esi_checkmesh/{mesh}", crate_dir());
    let io = outram_foam_basic_lib::io::PolyMesh::read(&dir).expect("read polyMesh");
    SnapMesh {
        points: io.points.iter().map(|p| [p.x, p.y, p.z]).collect(),
        faces: io.faces.iter().map(|f| f.verts.clone()).collect(),
        owner: io.faces.iter().map(|f| f.owner).collect(),
        neighbour: io.faces.iter().take(io.n_internal_faces).map(|f| f.neighbour.unwrap()).collect(),
    }
}

fn motion(name: &str) -> SolidBodyMotion {
    let text = std::fs::read_to_string(format!(
        "{}/verification_and_validation/dynamic_mesh_gh1380/cases/{name}.dynamicMeshDict",
        crate_dir()
    ))
    .expect("dynamicMeshDict");
    let dict = FoamFile::parse(&text).expect("parse").dict;
    SolidBodyMotion::from_dynamic_mesh_dict(&dict).expect("solidBody motion")
}

/// Our fields after each of the five steps: `[time][field]` in `FIELDS` order.
fn run(mesh: &str, motion_name: &str) -> Vec<Vec<Vec<f64>>> {
    let mut sim = SolidBodyMeshMotion::new(load(mesh), motion(motion_name), 0.0);
    (0..TIMES.len())
        .map(|_| {
            let s = sim.advance(DELTA_T);
            vec![
                flat(&s.points),
                s.mesh_phi.clone(),
                flat(&s.geometry.face_centres),
                flat(&s.geometry.face_areas),
                flat(&s.geometry.cell_centres),
                s.geometry.cell_volumes.clone(),
            ]
        })
        .collect()
}

fn table_path() -> String {
    format!("{}/tests/fixtures/esi_dynmesh_hashes.txt", crate_dir())
}

/// Every field of every case at every step is bit-identical to the ESI probe
/// (D1 to D5): 2 meshes x 4 motions x 5 steps x 6 fields = 240 hashes.
#[test]
fn solid_body_motion_and_mesh_phi_are_bit_identical_to_the_probe() {
    let table = std::fs::read_to_string(table_path()).unwrap();
    let mut checked = 0;
    for mesh in MESHES {
        for m in MOTIONS {
            let case = format!("{mesh}_{m}");
            let ours = run(mesh, m);
            for (ti, time) in TIMES.iter().enumerate() {
                for (fi, field) in FIELDS.iter().enumerate() {
                    let values = &ours[ti][fi];
                    let line = table
                        .lines()
                        .find(|l| {
                            let t: Vec<&str> = l.split_whitespace().collect();
                            t.len() == 5 && t[0] == case && t[1] == *time && t[2] == *field
                        })
                        .unwrap_or_else(|| panic!("{case} {time} {field} not in the table"));
                    let t: Vec<&str> = line.split_whitespace().collect();
                    assert_eq!(values.len(), t[3].parse::<usize>().unwrap(), "{case} {time} {field}: length");
                    let hash = u64::from_str_radix(t[4].trim_start_matches("0x"), 16).unwrap();
                    assert_eq!(fnv(values), hash, "{case} {time} {field}: not bit-identical to OpenFOAM.com v2506");
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 240);
}

fn read_probe(dir: &str, case: &str, time: &str, field: &str) -> Vec<f64> {
    let text = std::fs::read_to_string(format!("{dir}/{case}/probe/{time}/{field}")).expect("probe file");
    text.split_whitespace().map(|t| t.parse::<f64>().unwrap()).collect()
}

/// With the raw probe output available, compare value by value (bit patterns),
/// not only by hash. Skipped, with a note, when `DYNMESH_PROBE_DIR` is unset.
#[test]
fn raw_probe_text_is_reproduced_value_by_value_when_available() {
    let Ok(dir) = std::env::var("DYNMESH_PROBE_DIR") else {
        eprintln!("DYNMESH_PROBE_DIR not set: raw comparison skipped (the hash test above still ran)");
        return;
    };
    let mut n_values = 0usize;
    for mesh in MESHES {
        for m in MOTIONS {
            let ours = run(mesh, m);
            for (ti, time) in TIMES.iter().enumerate() {
                for (fi, field) in FIELDS.iter().enumerate() {
                    let theirs = read_probe(&dir, &format!("{mesh}_{m}"), time, field);
                    assert_eq!(theirs.len(), ours[ti][fi].len());
                    for (k, (a, b)) in theirs.iter().zip(&ours[ti][fi]).enumerate() {
                        assert_eq!(a.to_bits(), b.to_bits(), "{mesh}_{m} {time} {field}[{k}]: {a:e} vs {b:e}");
                        n_values += 1;
                    }
                }
            }
        }
    }
    eprintln!("raw probe: {n_values} values bit-identical");
    assert!(n_values > 100_000, "suspiciously few values compared: {n_values}");
}

/// Rewrite the hash table from a probe directory (`DYNMESH_PROBE_DIR`).
#[test]
#[ignore = "regenerates tests/fixtures/esi_dynmesh_hashes.txt from DYNMESH_PROBE_DIR"]
fn regenerate_hash_table() {
    let dir = std::env::var("DYNMESH_PROBE_DIR").expect("DYNMESH_PROBE_DIR");
    let mut out = String::from("# case time field count fnv1a(bits); ESI v2506 dumpMeshMotion, gh:#1380\n");
    for mesh in MESHES {
        for m in MOTIONS {
            for time in TIMES {
                for field in FIELDS {
                    let v = read_probe(&dir, &format!("{mesh}_{m}"), time, field);
                    out.push_str(&format!("{mesh}_{m} {time} {field} {} 0x{:016x}\n", v.len(), fnv(&v)));
                }
            }
        }
    }
    std::fs::write(table_path(), out).unwrap();
}

/// D6, independent of ESI: the geometric conservation law. For every cell the
/// `meshPhi` of its faces (owner +, neighbour -) times deltaT equals the change of
/// the cell volume, to round-off, at every step of every case.
#[test]
fn geometric_conservation_law_holds_for_every_cell() {
    for mesh in MESHES {
        let base = load(mesh);
        let n_cells = base.owner.iter().chain(base.neighbour.iter()).copied().max().unwrap() + 1;
        for m in MOTIONS {
            let mut sim = SolidBodyMeshMotion::new(base.clone(), motion(m), 0.0);
            let mut v_old = outram_foam_mesh::check_mesh::esi_geometry(&base).cell_volumes;
            for _ in 0..TIMES.len() {
                let s = sim.advance(DELTA_T);
                let mut net = vec![0.0; n_cells];
                for (f, &o) in base.owner.iter().enumerate() {
                    net[o] += s.mesh_phi[f];
                }
                for (f, &n) in base.neighbour.iter().enumerate() {
                    net[n] -= s.mesh_phi[f];
                }
                for c in 0..n_cells {
                    let dv = s.geometry.cell_volumes[c] - v_old[c];
                    let err = (net[c] * DELTA_T - dv).abs();
                    // rigid motion: dv is round-off, the sweep is exact to ~1e-15 of |phi|
                    assert!(err < 1e-12 * v_old[c].max(1e-3), "{mesh} {m} cell {c}: GCL error {err:e}");
                }
                v_old = s.geometry.cell_volumes.clone();
            }
        }
    }
}
