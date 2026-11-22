// SPDX-License-Identifier: GPL-3.0-only
//! Draw the dual of a primal `polyMesh` (gh:#1460; AGENTS.md: meshes are
//! drawn): four panels of the faces lying in the lowest-z plane, seen from
//! +z: the primal mesh, the default surface-preserving dual, the
//! upstream-mode dual, and the upstream-mode dual with `-concaveMultiCells`.
//! Dual points are dots (red), edges lines (blue); primal edges are grey.
//!
//! `draw_poly_dual_mesh PRIMAL_POLYMESH_DIR OUT.png FEATURE_ANGLE`
//!
//! Example binary: drawing glue, exempt from the reached-by-a-test rule (it
//! calls only the tested dual constructions).
use image::{Rgb, RgbImage};
use outram_foam_mesh::poly_dual_mesh::{poly_dual_mesh, PolyMesh as Primal};
use outram_foam_mesh::poly_dual_mesh_upstream::{poly_dual_mesh_upstream, UpstreamOptions};

const PANEL: u32 = 520;

fn line(img: &mut RgbImage, a: (f64, f64), b: (f64, f64), c: Rgb<u8>) {
    let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) as usize).max(1) * 2;
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let (x, y) = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
        if x >= 0.0 && y >= 0.0 && (x as u32) < img.width() && (y as u32) < img.height() {
            img.put_pixel(x as u32, y as u32, c);
        }
    }
}

fn dot(img: &mut RgbImage, p: (f64, f64), c: Rgb<u8>) {
    for dx in -2i32..=2 {
        for dy in -2i32..=2 {
            let (x, y) = (p.0 as i32 + dx, p.1 as i32 + dy);
            if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
                img.put_pixel(x as u32, y as u32, c);
            }
        }
    }
}

/// Bounding box `(xmin, ymin, xmax, ymax, zmin)` of a point set.
fn bbox(pts: &[outram_foam_basic_lib::primitives::Vector3]) -> (f64, f64, f64, f64, f64) {
    let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MAX);
    for p in pts {
        b = (b.0.min(p.x), b.1.min(p.y), b.2.max(p.x), b.3.max(p.y), b.4.min(p.z));
    }
    b
}

fn panel(
    img: &mut RgbImage,
    ox: u32,
    bb: (f64, f64, f64, f64, f64),
    pts: &[outram_foam_basic_lib::primitives::Vector3],
    faces: &[Vec<usize>],
    edge: Rgb<u8>,
    show_pts: bool,
) {
    let (xmin, ymin, xmax, ymax, zmin) = bb;
    let s = (PANEL as f64 - 40.0) / (xmax - xmin).max(ymax - ymin);
    let map = |p: &outram_foam_basic_lib::primitives::Vector3| {
        (ox as f64 + 20.0 + (p.x - xmin) * s, PANEL as f64 - 20.0 - (p.y - ymin) * s)
    };
    let tol = 1e-9 * (xmax - xmin).max(1.0);
    let mut used = std::collections::HashSet::new();
    for f in faces {
        if f.iter().all(|&v| (pts[v].z - zmin).abs() < tol) {
            for i in 0..f.len() {
                let (a, b) = (f[i], f[(i + 1) % f.len()]);
                line(img, map(&pts[a]), map(&pts[b]), edge);
                used.insert(a);
            }
        }
    }
    if show_pts {
        for v in used {
            dot(img, map(&pts[v]), Rgb([200, 30, 30]));
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, dir, out, angle] = a.as_slice() else {
        eprintln!("usage: draw_poly_dual_mesh PRIMAL_POLYMESH_DIR OUT.png FEATURE_ANGLE");
        std::process::exit(2);
    };
    let angle: f64 = angle.parse().unwrap();
    let io = outram_foam_basic_lib::io::PolyMesh::read(dir).expect("read primal");
    let primal = Primal {
        points: io.points.clone(),
        faces: io.faces.iter().map(|f| f.verts.clone()).collect(),
        owner: io.faces.iter().map(|f| f.owner).collect(),
        neighbour: io.faces.iter().take(io.n_internal_faces).map(|f| f.neighbour.unwrap()).collect(),
        n_internal_faces: io.n_internal_faces,
        patches: io.patches.clone(),
        n_cells: io.n_cells,
    };
    let bb = bbox(&primal.points);
    let default = poly_dual_mesh(&primal, angle).expect("default dual");
    let up = poly_dual_mesh_upstream(&primal, &UpstreamOptions::new(angle)).expect("upstream dual");
    let cv = poly_dual_mesh_upstream(&primal, &UpstreamOptions::new(angle).with_concave_multi_cells())
        .expect("upstream dual, concave multi-cells");
    let mut img = RgbImage::from_pixel(PANEL * 4, PANEL, Rgb([255, 255, 255]));
    panel(&mut img, 0, bb, &primal.points, &primal.faces, Rgb([110, 110, 110]), false);
    panel(&mut img, PANEL, bb, &default.points, &default.faces, Rgb([30, 60, 200]), true);
    panel(&mut img, 2 * PANEL, bb, &up.points, &up.faces, Rgb([30, 60, 200]), true);
    panel(&mut img, 3 * PANEL, bb, &cv.points, &cv.faces, Rgb([30, 60, 200]), true);
    for i in 1..4 {
        line(&mut img, ((i * PANEL) as f64, 0.0), ((i * PANEL) as f64, PANEL as f64), Rgb([0, 0, 0]));
    }
    img.save(out).expect("write png");
    println!(
        "primal {} cells | default dual {} cells {} points | upstream {} cells {} points | upstream+concave {} cells {} points",
        primal.n_cells,
        default.n_cells,
        default.points.len(),
        up.n_cells,
        up.points.len(),
        cv.n_cells,
        cv.points.len()
    );
}
