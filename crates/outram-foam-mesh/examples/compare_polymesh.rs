// SPDX-License-Identifier: GPL-3.0-only
//! Compare this crate's `blockMesh` with an upstream `blockMesh` run, file by
//! file (gh:#1464, gh:#1364): `points` (bit-identical count, worst absolute
//! difference), `faces`, `owner`, `neighbour` (exact), `boundary` (patch
//! names, starts, sizes).
//!
//! ```text
//! cargo run --release -p outram-foam-mesh --example compare_polymesh -- \
//!     DICT UPSTREAM_POLYMESH_DIR
//! ```
//! The upstream directory is `constant/polyMesh` of a case written with
//! `writePrecision 17`. Example binary: reporting glue, exempt from the
//! reached-by-a-test rule (it calls only the tested `block_mesh` API).
use outram_foam_mesh::block_mesh::block_mesh_from_file;

/// The body of an OpenFOAM list file: everything between the first
/// `N` + `(` after the header and the matching `)`, as whitespace tokens with
/// `(` and `)` split off.
fn body_tokens(path: &std::path::Path) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut s = String::new();
    for line in text.lines() {
        if !line.trim_start().starts_with("//") {
            s.push_str(line);
            s.push('\n');
        }
    }
    let after_header = match s.find("FoamFile") {
        Some(i) => {
            let close = s[i..].find('}').expect("header close") + i + 1;
            &s[close..]
        }
        None => &s[..],
    };
    let spaced = after_header.replace('(', " ( ").replace(')', " ) ");
    spaced.split_whitespace().map(str::to_string).collect()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, dict, dir] = a.as_slice() else {
        eprintln!("usage: compare_polymesh DICT UPSTREAM_POLYMESH_DIR");
        std::process::exit(2);
    };
    let dir = std::path::Path::new(dir);
    let m = block_mesh_from_file(dict).expect("our blockMesh");

    // points
    let t = body_tokens(&dir.join("points"));
    let n: usize = t[0].parse().unwrap();
    let nums: Vec<f64> = t[2..].iter().filter(|x| *x != "(" && *x != ")").map(|x| x.parse().unwrap()).collect();
    assert_eq!(nums.len(), 3 * n);
    let (mut identical, mut worst, mut worst_i) = (0usize, 0.0f64, 0usize);
    for i in 0..n.min(m.points.len()) {
        let p = m.points[i];
        let d = [p.x - nums[3 * i], p.y - nums[3 * i + 1], p.z - nums[3 * i + 2]];
        let dm = d.iter().fold(0.0f64, |w, x| w.max(x.abs()));
        if p.x.to_bits() == nums[3 * i].to_bits()
            && p.y.to_bits() == nums[3 * i + 1].to_bits()
            && p.z.to_bits() == nums[3 * i + 2].to_bits()
        {
            identical += 1;
        }
        if dm > worst {
            worst = dm;
            worst_i = i;
        }
    }
    println!("points: ours {} upstream {n}; bit-identical {identical}; worst |diff| {worst:e} at {worst_i}", m.points.len());

    // faces
    let t = body_tokens(&dir.join("faces"));
    let nf: usize = t[0].parse().unwrap();
    let mut up_faces: Vec<Vec<usize>> = Vec::with_capacity(nf);
    let mut k = 2;
    while up_faces.len() < nf {
        // "4(a b c d)" tokenises to "4" "(" a b c d ")"
        let cnt: usize = t[k].parse().unwrap();
        assert_eq!(t[k + 1], "(");
        up_faces.push(t[k + 2..k + 2 + cnt].iter().map(|x| x.parse().unwrap()).collect());
        k += 3 + cnt;
    }
    let mut same = 0usize;
    let mut same_set = 0usize;
    let mut first_diff: Option<usize> = None;
    for (i, f) in m.faces.iter().enumerate() {
        if i >= up_faces.len() {
            break;
        }
        if f.verts == up_faces[i] {
            same += 1;
        } else if first_diff.is_none() {
            first_diff = Some(i);
        }
        let (mut x, mut y) = (f.verts.clone(), up_faces[i].clone());
        x.sort_unstable();
        y.sort_unstable();
        if x == y {
            same_set += 1;
        }
    }
    println!("faces: ours {} upstream {nf}; identical loops {same}; same point set {same_set}; first differing face {first_diff:?}", m.faces.len());
    if let Some(i) = first_diff {
        println!("  face {i}: ours {:?} upstream {:?}", m.faces[i].verts, up_faces[i]);
    }

    // owner / neighbour
    for (name, ours) in [
        ("owner", m.faces.iter().map(|f| f.owner).collect::<Vec<_>>()),
        ("neighbour", m.faces.iter().filter_map(|f| f.neighbour).collect::<Vec<_>>()),
    ] {
        let t = body_tokens(&dir.join(name));
        let cnt: usize = t[0].parse().unwrap();
        let up: Vec<usize> = t[2..2 + cnt].iter().map(|x| x.parse().unwrap()).collect();
        let eq = ours.iter().zip(&up).filter(|(a, b)| a == b).count();
        println!("{name}: ours {} upstream {cnt}; equal {eq}", ours.len());
    }

    // boundary
    let t = body_tokens(&dir.join("boundary"));
    let mut up: Vec<(String, usize, usize)> = Vec::new();
    let mut i = 0;
    while i < t.len() {
        if t[i] == "nFaces" {
            // find the preceding patch name: the token before the `{`
            let mut j = i;
            while t[j] != "{" {
                j -= 1;
            }
            let name = t[j - 1].clone();
            let nfaces: usize = t[i + 1].trim_end_matches(';').parse().unwrap();
            let start: usize = t[i + 3].trim_end_matches(';').parse().unwrap();
            up.push((name, start, nfaces));
        }
        i += 1;
    }
    let ours: Vec<(String, usize, usize)> = m.patches.iter().map(|p| (p.name.clone(), p.start, p.size)).collect();
    println!("boundary: ours {ours:?}\n          upstream {up:?}\n          equal: {}", ours == up);
}
