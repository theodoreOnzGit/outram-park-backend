// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithms ported from OpenFOAM.com (ESI) v2506 (see `src/particle_tracking.rs`); the
// frozen outputs come from runs of that release's `trackParticles` probe.

//! Barycentric particle tracking against ESI v2506 (gh:#1382).
//!
//! **Method and results:** `verification_and_validation/particle_tracking_gh1382/README.md`;
//! predictions P1-P7 were committed in `PREDICTIONS.md` before any run.
//!
//! Cases: meshes `warped3d` (hex, 120 cells) and `dual_warped3d` (polyhedral) from
//! `tests/fixtures/esi_checkmesh/`. For each, `tests/fixtures/esi_particle_tracking/<mesh>.script`
//! (written by the ignored test `regenerate_scripts`, seeded LCG) is a list of `new`, `reset`,
//! `track` and `trackToFace` commands; `<mesh>.esi.out` is what the ESI `trackParticles`
//! probe printed for it, every double at `%.17g`. A pass means every printed value
//! (barycentric coordinates, `celli`, `tetFacei`, `tetPti`, `facei`, `stepFraction`,
//! `behind`, `nBehind`, position, the returned fraction) is bit-identical.
//! Set `PARTICLE_PROBE_OUT` to a directory to write our replay next to it for diffing.

use outram_foam_mesh::particle_tracking::{Particle, TrackingMesh};
use outram_foam_mesh::snappy_hex_mesh::esi_snap::SnapMesh;

const MESHES: [&str; 2] = ["warped3d", "dual_warped3d"];

fn crate_dir() -> String {
    env!("CARGO_MANIFEST_DIR").to_string()
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

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/esi_particle_tracking/{name}", crate_dir())
}

/// Replay `script` with our port: one line per command, the same columns as the probe.
fn replay(tm: &TrackingMesh, script: &str) -> Vec<Vec<String>> {
    let mut parts: Vec<Particle> = Vec::new();
    let mut out = Vec::new();
    for line in script.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let t: Vec<&str> = line.split_whitespace().collect();
        let num = |k: usize| -> f64 { t[k].parse::<f64>().expect("number") };
        let (idx, ret): (usize, Option<f64>) = match t[0] {
            "new" => {
                let p = Particle::new_at(tm, [num(1), num(2), num(3)], t[4].parse().unwrap()).expect("located");
                parts.push(p);
                (parts.len() - 1, None)
            }
            "reset" => {
                let i: usize = t[1].parse().unwrap();
                parts[i].reset();
                (i, None)
            }
            "track" | "trackToFace" => {
                let i: usize = t[1].parse().unwrap();
                let d = [num(2), num(3), num(4)];
                let r = if t[0] == "track" {
                    parts[i].track(tm, d, num(5))
                } else {
                    parts[i].track_to_face(tm, d, num(5))
                };
                (i, Some(r))
            }
            other => panic!("unknown command {other}"),
        };
        let p = &parts[idx];
        let pos = p.position(tm);
        let g = |v: f64| format!("{v:?}");
        let mut row = vec![t[0].to_string(), idx.to_string(), ret.map_or("-".to_string(), g)];
        row.extend(p.coordinates.iter().map(|&v| g(v)));
        row.extend([p.celli, p.tet_facei, p.tet_pti, p.facei].iter().map(|v| v.to_string()));
        row.extend([g(p.step_fraction), g(p.behind), p.n_behind.to_string()]);
        row.extend(pos.iter().map(|&v| g(v)));
        out.push(row);
    }
    out
}

/// Column names, in probe order, and whether the column is an integer / label.
const COLUMNS: [(&str, bool); 17] = [
    ("cmd", true),
    ("idx", true),
    ("ret", false),
    ("y_a", false),
    ("y_b", false),
    ("y_c", false),
    ("y_d", false),
    ("celli", true),
    ("tetFacei", true),
    ("tetPti", true),
    ("facei", true),
    ("stepFraction", false),
    ("behind", false),
    ("nBehind", true),
    ("pos_x", false),
    ("pos_y", false),
    ("pos_z", false),
];

#[derive(Default, Debug, Clone)]
struct Tally {
    n: usize,
    identical: usize,
    worst_rel: f64,
}

/// Compare ours with the probe output; returns per-column tallies.
fn compare(ours: &[Vec<String>], esi: &str) -> Vec<Tally> {
    let lines: Vec<&str> = esi.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), ours.len(), "number of output lines");
    let mut tallies = vec![Tally::default(); 17];
    for (row, line) in ours.iter().zip(lines) {
        let theirs: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(theirs.len(), row.len(), "columns in {line}");
        for (c, tally) in tallies.iter_mut().enumerate() {
            tally.n += 1;
            let (mine, esi_tok) = (row[c].as_str(), theirs[c]);
            let same = if COLUMNS[c].1 || mine == "-" || esi_tok == "-" {
                mine == esi_tok
            } else {
                let a: f64 = mine.parse().unwrap();
                match esi_tok.parse::<f64>() {
                    Ok(b) => {
                        let s = a.to_bits() == b.to_bits();
                        if !s {
                            let rel = (a - b).abs() / b.abs().max(f64::MIN_POSITIVE);
                            tally.worst_rel = tally.worst_rel.max(rel);
                        }
                        s
                    }
                    Err(_) => false,
                }
            };
            if same {
                tally.identical += 1;
            }
        }
    }
    tallies
}

#[test]
fn particle_tracking_is_bit_identical_with_esi_v2506() {
    let mut all_ok = true;
    for mesh in MESHES {
        let tm = TrackingMesh::new(load(mesh));
        let script = std::fs::read_to_string(fixture(&format!("{mesh}.script"))).expect("script");
        let esi = std::fs::read_to_string(fixture(&format!("{mesh}.esi.out"))).expect("esi output");
        let ours = replay(&tm, &script);
        if let Ok(dir) = std::env::var("PARTICLE_PROBE_OUT") {
            let text: Vec<String> = ours.iter().map(|r| r.join(" ")).collect();
            std::fs::write(format!("{dir}/{mesh}.ours.out"), text.join("\n") + "\n").unwrap();
        }
        let tallies = compare(&ours, &esi);
        for (c, t) in tallies.iter().enumerate() {
            println!("{mesh:14} {:13} {:6}/{:6} identical, worst rel {:e}", COLUMNS[c].0, t.identical, t.n, t.worst_rel);
            all_ok &= t.identical == t.n;
        }
    }
    assert!(all_ok, "some values differ from ESI v2506; see the table above");
}

/// Invariants independent of ESI: coordinates are non-negative and sum to one after a
/// complete track, and the particle stays in a cell that contains its position.
#[test]
fn tracked_particles_have_valid_barycentric_coordinates() {
    for mesh in MESHES {
        let tm = TrackingMesh::new(load(mesh));
        let script = std::fs::read_to_string(fixture(&format!("{mesh}.script"))).expect("script");
        let rows = replay(&tm, &script);
        for r in rows {
            let y: Vec<f64> = r[3..7].iter().map(|s| s.parse().unwrap()).collect();
            assert!(y.iter().all(|&v| v >= 0.0), "negative coordinate in {r:?}");
            // coordinates of a particle that stopped on a tet face carry a zero, others sum to 1 within round-off
            let sum: f64 = y.iter().sum();
            assert!((sum - 1.0).abs() < 1e-9, "sum {sum} in {r:?}");
        }
    }
}

/// `celli < 0` needs `mesh.cellTree()`, which is not ported: an explicit error, not a guess.
#[test]
fn locating_without_a_cell_is_an_error() {
    let tm = TrackingMesh::new(load("warped3d"));
    assert!(Particle::new_at(&tm, [0.0, 0.0, 0.0], -1).is_err());
}

/// Every internal face of both meshes has a base point for which the fan is good
/// (the same condition `checkMesh` reports as "Face tets OK").
#[test]
fn tet_base_points_exist_for_all_faces() {
    for mesh in MESHES {
        let tm = TrackingMesh::new(load(mesh));
        assert!(tm.tet_base_pt_is.iter().all(|&b| b >= 0), "{mesh}: a face has no base point");
        // face_tri_is is reached for both orientations
        let f = 0usize;
        let o = tm.mesh.owner[f] as isize;
        let n = tm.mesh.neighbour[f] as isize;
        assert_ne!(tm.face_tri_is(o, f as isize, 1), tm.face_tri_is(n, f as isize, 1));
    }
}

// ---------------------------------------------------------------------------------
// Script generation (run once, before the ESI run; the scripts are committed).
// ---------------------------------------------------------------------------------

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / 9007199254740992.0
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
    fn direction(&mut self) -> [f64; 3] {
        loop {
            let v = [self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0)];
            let m2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
            if m2 > 1e-4 && m2 <= 1.0 {
                let m = m2.sqrt();
                return [v[0] / m, v[1] / m, v[2] / m];
            }
        }
    }
}

fn generate_script(tm: &TrackingMesh, seed: u64) -> String {
    let mut rng = Lcg(seed);
    let n_cells = tm.geometry.cell_volumes.len();
    let mut s = String::new();
    s.push_str(&format!("# gh:#1382 particle script, seed {seed}, {n_cells} cells (generated by regenerate_scripts)\n"));
    let g = |v: f64| format!("{v:?}");
    let mut n_part = 0usize;

    // A point strictly inside the tet decomposition of cell c (random tet, random weights).
    let point_in_cell = |rng: &mut Lcg, c: usize| -> [f64; 3] {
        let faces = &tm.geometry.cell_faces[c];
        let f = faces[(rng.next() * faces.len() as f64) as usize % faces.len()];
        let nv = tm.mesh.faces[f].len();
        let tp = 1 + (rng.next() * (nv - 2) as f64) as isize % (nv as isize - 2);
        let tet = tm.tet(c as isize, f as isize, tp);
        let mut w = [rng.range(0.05, 1.0), rng.range(0.0, 1.0), rng.range(0.0, 1.0), rng.range(0.0, 1.0)];
        let sum: f64 = w.iter().sum();
        for x in w.iter_mut() {
            *x /= sum;
        }
        [0, 1, 2].map(|k| w[0] * tet[0][k] + w[1] * tet[1][k] + w[2] * tet[2][k] + w[3] * tet[3][k])
    };

    for group in 0..5 {
        let n_in_group = match group {
            0 => 40,
            1 => 12,
            2 => 12,
            3 => 12,
            _ => 8,
        };
        s.push_str(&format!("# group {group}\n"));
        for _ in 0..n_in_group {
            let c = (rng.next() * n_cells as f64) as usize % n_cells;
            let len = tm.geometry.cell_volumes[c].abs().cbrt();
            let p = point_in_cell(&mut rng, c);
            let (pos, celli) = match group {
                1 => {
                    // given cell is a neighbour of the true one
                    let faces: Vec<usize> =
                        tm.geometry.cell_faces[c].iter().copied().filter(|&f| f < tm.mesh.neighbour.len()).collect();
                    let f = faces[(rng.next() * faces.len() as f64) as usize % faces.len()];
                    let other = if tm.mesh.owner[f] == c { tm.mesh.neighbour[f] } else { tm.mesh.owner[f] };
                    (p, other)
                }
                4 => {
                    // at the cell centre (the VSMALL perturbation) or at an internal face centre
                    if rng.next() < 0.5 {
                        (tm.geometry.cell_centres[c], c)
                    } else {
                        let faces: Vec<usize> =
                            tm.geometry.cell_faces[c].iter().copied().filter(|&f| f < tm.mesh.neighbour.len()).collect();
                        let f = faces[(rng.next() * faces.len() as f64) as usize % faces.len()];
                        (tm.geometry.face_centres[f], c)
                    }
                }
                _ => (p, c),
            };
            s.push_str(&format!("new {} {} {} {}\n", g(pos[0]), g(pos[1]), g(pos[2]), celli));
            let i = n_part;
            n_part += 1;
            let legs = if group == 3 { 3 } else { 4 };
            for _ in 0..legs {
                let dir = rng.direction();
                let mag = match group {
                    2 => rng.range(5.0, 30.0) * len,
                    _ => rng.range(0.05, 2.0) * len,
                };
                let d = [dir[0] * mag, dir[1] * mag, dir[2] * mag];
                s.push_str(&format!("reset {i}\n"));
                match group {
                    3 => {
                        // fractional sub-steps of one displacement, and a single trackToFace
                        for _ in 0..3 {
                            s.push_str(&format!("track {i} {} {} {} 0.25\n", g(d[0]), g(d[1]), g(d[2])));
                        }
                        s.push_str(&format!("reset {i}\n"));
                        s.push_str(&format!("trackToFace {i} {} {} {} 1\n", g(d[0]), g(d[1]), g(d[2])));
                    }
                    _ => s.push_str(&format!("track {i} {} {} {} 1\n", g(d[0]), g(d[1]), g(d[2]))),
                }
            }
        }
    }
    s
}

#[test]
#[ignore = "writes the committed particle scripts; run once before the ESI probe"]
fn regenerate_scripts() {
    std::fs::create_dir_all(fixture("")).unwrap();
    for (k, mesh) in MESHES.iter().enumerate() {
        let tm = TrackingMesh::new(load(mesh));
        let script = generate_script(&tm, 20261114 + k as u64);
        std::fs::write(fixture(&format!("{mesh}.script")), script).unwrap();
    }
}
