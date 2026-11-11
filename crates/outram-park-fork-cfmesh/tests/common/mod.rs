//! Shared helpers of the `cfmesh_vs_upstream_*` tests (gh:#1385): inflate and
//! read the committed upstream `polyMesh` files, read the committed sphere STL
//! as upstream does (single precision), and compare a port mesh with an
//! upstream mesh at three levels (counts, points, topology).
#![allow(dead_code)]

use outram_foam_basic_lib::io::poly_mesh::PolyMesh;
use outram_park_fork_cfmesh::math::Vec3;
use outram_park_fork_cfmesh::volume_mesh::VolumeMesh;
use std::collections::HashMap;
use std::path::PathBuf;

/// `verification_and_validation/upstream_cfmesh_gh1385`.
pub fn upstream_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/upstream_cfmesh_gh1385")
}

/// Inflate a `gzip -n` file (10-byte header, 8-byte trailer).
pub fn gunzip(bytes: &[u8]) -> Vec<u8> {
    assert!(bytes.len() > 18 && bytes[0] == 0x1f && bytes[1] == 0x8b, "not gzip");
    assert_eq!(bytes[3], 0, "gzip -n expected (no name/extra flags)");
    miniz_oxide::inflate::decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

/// Read an upstream run's `constant/polyMesh` (`runs/<run>` relative to the record).
pub fn read_upstream(run: &str) -> PolyMesh {
    let src = upstream_dir().join("runs").join(run).join("constant/polyMesh");
    let dst = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("upstream_gh1385").join(run.replace('/', "_"));
    std::fs::create_dir_all(&dst).unwrap();
    for f in ["points", "faces", "owner", "neighbour", "boundary"] {
        let gz = std::fs::read(src.join(format!("{f}.gz"))).unwrap_or_else(|e| panic!("{}: {e}", src.join(f).display()));
        let mut text = String::from_utf8(gunzip(&gz)).unwrap();
        if f == "neighbour" {
            text = internal_neighbours_only(&text);
        }
        std::fs::write(dst.join(f), text).unwrap();
    }
    PolyMesh::read(&dst).expect("read polyMesh")
}

/// cfMesh writes `neighbour` for every face (-1 on a boundary face; internal
/// faces first), OpenFOAM's reader wants the internal faces only. Keeps the
/// header and rewrites the list up to the first -1.
fn internal_neighbours_only(text: &str) -> String {
    let open = text.find("\n(\n").expect("neighbour list");
    let before = &text[..open];
    let count_start = before.rfind('\n').unwrap() + 1;
    let body = &text[open + 3..];
    let end = body.find("\n)").expect("list end");
    let vals: Vec<i64> = body[..end].split_whitespace().map(|s| s.parse().unwrap()).collect();
    let n = vals.iter().position(|&v| v < 0).unwrap_or(vals.len());
    assert!(vals[n..].iter().all(|&v| v < 0), "internal faces must come first");
    let mut out = String::from(&text[..count_start]);
    out.push_str(&format!("{n}\n(\n"));
    for v in &vals[..n] {
        out.push_str(&format!("{v}\n"));
    }
    out.push_str(")\n");
    out
}

/// The committed sphere STL as the port reads it (outram-foam-mesh's STL reader).
pub fn read_sphere() -> (Vec<Vec3>, Vec<[usize; 3]>) {
    let gz = std::fs::read(upstream_dir().join("inputs/sphere_r3_24x48.stl.gz")).unwrap();
    let z = outram_foam_mesh::snappy_hex_mesh::stl::read_stl_zoned_bytes(&gunzip(&gz)).unwrap();
    (z.points.iter().map(|p| Vec3::new(p.x, p.y, p.z)).collect(), z.triangles.clone())
}

/// The result of comparing one port mesh with one upstream mesh.
#[derive(Debug, Clone, Default)]
pub struct Diff {
    pub counts: ((usize, usize, usize, usize), (usize, usize, usize, usize)),
    pub counts_equal: bool,
    /// Same-index points: number bit-identical, max abs difference (None when point counts differ).
    pub index_points: Option<(usize, f64)>,
    /// Nearest-neighbour matching: max distance upstream point -> port point, number of port points used twice.
    pub nearest: Option<(f64, usize)>,
    /// Faces equal as vertex sets after the nearest-point map: matched, total upstream faces.
    pub faces_matched: Option<(usize, usize)>,
    /// Faces (same index) with identical vertex ring and owner/neighbour.
    pub index_faces_identical: Option<usize>,
}

pub fn counts_port(m: &VolumeMesh) -> (usize, usize, usize, usize) {
    (m.points.len(), m.faces.len(), m.n_cells, m.n_internal_faces())
}

pub fn counts_up(u: &PolyMesh) -> (usize, usize, usize, usize) {
    (u.points.len(), u.faces.len(), u.n_cells, u.n_internal_faces)
}

pub fn compare(m: &VolumeMesh, u: &PolyMesh) -> Diff {
    let mut d = Diff { counts: (counts_port(m), counts_up(u)), ..Default::default() };
    d.counts_equal = d.counts.0 == d.counts.1;
    if m.points.len() == u.points.len() {
        let mut same = 0;
        let mut worst = 0.0f64;
        for (a, b) in m.points.iter().zip(&u.points) {
            if a.x.to_bits() == b.x.to_bits() && a.y.to_bits() == b.y.to_bits() && a.z.to_bits() == b.z.to_bits() {
                same += 1;
            }
            worst = worst.max((a.x - b.x).abs()).max((a.y - b.y).abs()).max((a.z - b.z).abs());
        }
        d.index_points = Some((same, worst));
    }
    // nearest-neighbour map upstream -> port through a hash grid
    let h = 0.02;
    let key = |x: f64, y: f64, z: f64| ((x / h).floor() as i64, (y / h).floor() as i64, (z / h).floor() as i64);
    let mut grid: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for (i, p) in m.points.iter().enumerate() {
        grid.entry(key(p.x, p.y, p.z)).or_default().push(i);
    }
    let mut map = vec![usize::MAX; u.points.len()];
    let mut used = vec![0usize; m.points.len()];
    let mut worst = 0.0f64;
    for (i, q) in u.points.iter().enumerate() {
        let k = key(q.x, q.y, q.z);
        let mut best = (f64::INFINITY, usize::MAX);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(v) = grid.get(&(k.0 + dx, k.1 + dy, k.2 + dz)) {
                        for &j in v {
                            let p = m.points[j];
                            let dist = ((p.x - q.x).powi(2) + (p.y - q.y).powi(2) + (p.z - q.z).powi(2)).sqrt();
                            if dist < best.0 {
                                best = (dist, j);
                            }
                        }
                    }
                }
            }
        }
        map[i] = best.1;
        if best.1 != usize::MAX {
            used[best.1] += 1;
        }
        worst = worst.max(best.0);
    }
    d.nearest = Some((worst, used.iter().filter(|&&c| c > 1).count()));
    if map.iter().all(|&j| j != usize::MAX) {
        let mut set: HashMap<Vec<usize>, usize> = HashMap::new();
        for f in &m.faces {
            let mut v = f.clone();
            v.sort_unstable();
            *set.entry(v).or_default() += 1;
        }
        let mut matched = 0;
        for f in &u.faces {
            let mut v: Vec<usize> = f.verts.iter().map(|&p| map[p]).collect();
            v.sort_unstable();
            if let Some(c) = set.get_mut(&v) {
                if *c > 0 {
                    *c -= 1;
                    matched += 1;
                }
            }
        }
        d.faces_matched = Some((matched, u.faces.len()));
    }
    if m.faces.len() == u.faces.len() {
        let n = m
            .faces
            .iter()
            .zip(&u.faces)
            .enumerate()
            .filter(|(i, (a, b))| **a == b.verts && m.owner[*i] == b.owner && m.neighbour[*i] == b.neighbour)
            .count();
        d.index_faces_identical = Some(n);
    }
    d
}

impl Diff {
    /// One line: counts, points (bit-identical / total, max abs), faces.
    pub fn summary(&self) -> String {
        let pts = match self.index_points {
            Some((same, worst)) => format!("points {same}/{} bit-identical, max |d| {worst:.3e}", (self.counts.1).0),
            None => format!("points {} vs {}", (self.counts.0).0, (self.counts.1).0),
        };
        let faces = match (self.faces_matched, self.index_faces_identical) {
            (Some((m, n)), Some(i)) => format!("faces {m}/{n} as vertex sets, {i}/{n} index-identical with owner/neighbour"),
            (Some((m, n)), None) => format!("faces {m}/{n} as vertex sets"),
            _ => "faces not matched".to_string(),
        };
        format!("counts port {:?} upstream {:?}; {pts}; {faces}", self.counts.0, self.counts.1)
    }
}
