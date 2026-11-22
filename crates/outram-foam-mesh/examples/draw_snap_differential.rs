// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! gh:#1467 round 3: draw the snapped flange, port stages against ESI v2506
//! (AGENTS.md: meshes are drawn).
//!
//! `cargo run --release -p outram-foam-mesh --example draw_snap_differential -- OUT_DIR`
//!
//! Slices the mesh at the cell-face planes `z = -0.0133333` (`snap_slice_z.png`)
//! and `y = 0.0` (`snap_slice_y.png`): every mesh edge lying in the plane in the
//! castellated mesh is drawn at its moved position, projected on the plane.
//! Grey: the castellated mesh. Red: the port after `preSmoothPatch` x3, surface
//! attraction and `smoothDisplacement` x300 at scale 1 (ESI's time-35 mesh; the
//! port equals it bit for bit, so red hides under blue where they agree). Blue: ESI's
//! snapped mesh of time 36 (scaling 0.75 at the faces that failed the quality
//! check). Black: the STL cut by the plane. Wherever red shows beside blue, the
//! port's mesh is the unscaled one: the difference the missing `scaleMesh` check
//! leaves (README, P24). Ticks every 10 mm.
//!
//! Example binary: drawing code, exempt from the reached-by-a-test rule (it
//! calls the tested stage functions and reads the committed ESI fixtures).

use image::{Rgb, RgbImage};
use outram_foam_mesh::snappy_hex_mesh::esi_snap::{
    calc_nearest_surface, calc_snap_distance, displacement_field, mesh_edges, patch_addressing,
    smooth_displacement, SnapMesh, V,
};
use outram_foam_mesh::snappy_hex_mesh::stl::read_stl_zoned_bytes;
use outram_foam_mesh::snappy_hex_mesh::TriangleSoup;

const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/esi_v2506_flange");
const PX: usize = 900;

fn fixture(name: &str) -> Vec<u8> {
    let b = std::fs::read(format!("{FIX}/{name}")).expect("fixture");
    miniz_oxide::inflate::decompress_to_vec(&b[10..b.len() - 8]).expect("inflate")
}

fn vectors(name: &str) -> Vec<V> {
    fixture(name)
        .chunks_exact(24)
        .map(|c| {
            let f = |i: usize| f64::from_le_bytes(c[8 * i..8 * i + 8].try_into().unwrap());
            [f(0), f(1), f(2)]
        })
        .collect()
}

fn line(img: &mut RgbImage, a: (f64, f64), b: (f64, f64), colour: Rgb<u8>) {
    let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) * 1.5).ceil().max(1.0) as usize;
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let (x, y) = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
        if x >= 0.0 && y >= 0.0 && (x as usize) < PX && (y as usize) < PX {
            img.put_pixel(x as u32, y as u32, colour);
        }
    }
}

/// Draw one slice: `axis` is the fixed axis, `value` its coordinate; `(u, v)` the
/// other two axes in image x and y.
fn slice(path: &str, axis: usize, value: f64, mesh0: &SnapMesh, full: &[V], esi36: &[V], surface: &TriangleSoup) {
    let (u, v) = match axis {
        2 => (0, 1),
        1 => (0, 2),
        _ => (1, 2),
    };
    let (lo, hi) = ([-0.03, -0.03], [0.03, 0.03]);
    let to_px = |p: V| -> (f64, f64) {
        (
            (p[u] - lo[0]) / (hi[0] - lo[0]) * (PX as f64 - 1.0),
            (PX as f64 - 1.0) - (p[v] - lo[1]) / (hi[1] - lo[1]) * (PX as f64 - 1.0),
        )
    };
    let mut img = RgbImage::from_pixel(PX as u32, PX as u32, Rgb([255, 255, 255]));
    // Edges lying in the plane in the castellated mesh.
    let me = mesh_edges(mesh0);
    let in_plane: Vec<[usize; 2]> = me
        .edges
        .iter()
        .copied()
        .filter(|e| e.iter().all(|&p| (mesh0.points[p][axis] - value).abs() < 1e-9))
        .collect();
    for e in &in_plane {
        line(&mut img, to_px(mesh0.points[e[0]]), to_px(mesh0.points[e[1]]), Rgb([200, 200, 200]));
    }
    for e in &in_plane {
        line(&mut img, to_px(full[e[0]]), to_px(full[e[1]]), Rgb([220, 40, 40]));
    }
    for e in &in_plane {
        line(&mut img, to_px(esi36[e[0]]), to_px(esi36[e[1]]), Rgb([40, 80, 220]));
    }
    // The STL cut.
    for t in &surface.triangles {
        let c = [[t.a.x, t.a.y, t.a.z], [t.b.x, t.b.y, t.b.z], [t.c.x, t.c.y, t.c.z]];
        let mut pts: Vec<V> = Vec::new();
        for i in 0..3 {
            let (p, q) = (c[i], c[(i + 1) % 3]);
            let (dp, dq) = (p[axis] - value, q[axis] - value);
            if dp * dq < 0.0 {
                let s = dp / (dp - dq);
                pts.push([p[0] + s * (q[0] - p[0]), p[1] + s * (q[1] - p[1]), p[2] + s * (q[2] - p[2])]);
            }
        }
        if pts.len() == 2 {
            line(&mut img, to_px(pts[0]), to_px(pts[1]), Rgb([0, 0, 0]));
        }
    }
    img.save(path).expect("png");
}

fn main() {
    let out = std::env::args().nth(1).expect("OUT_DIR");
    // ESI castellated mesh, as in the tests.
    let text = String::from_utf8(fixture("esi_final_faces.txt.gz")).unwrap();
    let (mut faces, mut owner, mut neighbour) = (Vec::new(), Vec::new(), Vec::new());
    for l in text.lines() {
        let v: Vec<i64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
        owner.push(v[0] as usize);
        if v[1] >= 0 {
            neighbour.push(v[1] as usize);
        }
        faces.push(v[2..].iter().map(|&p| p as usize).collect::<Vec<_>>());
    }
    let mesh0 = SnapMesh { points: vectors("snap_points_32.bin.gz"), faces, owner, neighbour };
    let surface = TriangleSoup::from_zoned(&read_stl_zoned_bytes(&fixture("flange.stl.gz")).unwrap());
    let pa = patch_addressing(&mesh0);
    let snap_dist = calc_snap_distance(&mesh0, &pa, 1.0);
    // Stage inputs: the port's own preSmoothPatch is checked bit-identical in the tests; here the
    // ESI time-33 points (equal to it) keep the example short.
    let mut mesh = mesh0.clone();
    mesh.points = vectors("snap_points_33_n3.bin.gz");
    let local: Vec<V> = pa.mesh_points.iter().map(|&p| mesh.points[p]).collect();
    let (disp, _) = calc_nearest_surface(&local, &snap_dist, &surface);
    let mut field = displacement_field(&mesh, &pa, &disp);
    let me = mesh_edges(&mesh);
    smooth_displacement(&mesh, &me, &pa, &mut field, 300);
    let full: Vec<V> = mesh.points.iter().zip(&field).map(|(p, d)| [p[0] + d[0], p[1] + d[1], p[2] + d[2]]).collect();
    let esi36 = vectors("snap_points_36.bin.gz");
    let worst = full
        .iter()
        .zip(&esi36)
        .map(|(a, b)| (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f64::max))
        .fold(0.0, f64::max);
    println!("largest |port (scale 1) - ESI time 36| = {worst:e} m");
    slice(&format!("{out}/snap_slice_z.png"), 2, -0.03 + 5.0 * 0.04 / 12.0, &mesh0, &full, &esi36, &surface);
    slice(&format!("{out}/snap_slice_y.png"), 1, 0.0, &mesh0, &full, &esi36, &surface);
}
