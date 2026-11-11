// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Draw a `blockMeshDict` mesh as a PNG (the lowest-z plane of the mesh, cell
//! edges only, boundary faces coloured by patch), so a human can check the
//! geometry the solver sees (crate `AGENTS.md`, "Reactor geometry is DRAWN").
//!
//! ```text
//! cargo run --release -p outram-foam-mesh --example draw_block_mesh -- \
//!     DICT OUT.png [--ignore-edges] [--box xmin xmax ymin ymax]
//! ```
//!
//! `--ignore-edges` removes the `edges` list first, which reproduces the mesh
//! this crate built before curved edges were ported (every edge straight).
//! `--cut Z` draws the section of the mesh by the plane z = Z instead of the
//! lowest-z plane: every face is intersected with the plane and the cut
//! segment drawn (boundary faces coloured by patch). Needed for projected
//! spheres, whose lowest-z plane is a point (gh:#1462).
//! `--esi` follows OpenFOAM.com v2506 where it differs from dev (projection).
//! Example binary: drawing code, exempt from the reached-by-a-test rule (it
//! calls only the tested `block_mesh` library API).

use image::{Rgb, RgbImage};
use outram_foam_mesh::block_mesh::BlockMeshDict;

fn line(img: &mut RgbImage, a: (f64, f64), b: (f64, f64), col: Rgb<u8>) {
    let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as usize).max(1) * 2;
    for k in 0..=n {
        let t = k as f64 / n as f64;
        let x = a.0 + (b.0 - a.0) * t;
        let y = a.1 + (b.1 - a.1) * t;
        if x >= 0.0 && y >= 0.0 && (x as u32) < img.width() && (y as u32) < img.height() {
            img.put_pixel(x as u32, y as u32, col);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dict_path, out) = (&args[0], &args[1]);
    let ignore_edges = args.iter().any(|a| a == "--ignore-edges");
    let flavour = if args.iter().any(|a| a == "--esi") {
        outram_foam_mesh::block_projection::ProjectionFlavour::Esi
    } else {
        outram_foam_mesh::block_projection::ProjectionFlavour::Dev
    };
    let mut dict = BlockMeshDict::parse_with_flavour(
        &std::fs::read_to_string(dict_path).unwrap(),
        flavour,
    )
    .unwrap();
    let cut: Option<f64> = args
        .iter()
        .position(|a| a == "--cut")
        .map(|i| args[i + 1].parse().unwrap());
    if ignore_edges {
        dict.edges.clear();
    }
    let m = dict.build().unwrap();
    let zmin = m.points.iter().map(|p| p.z).fold(f64::MAX, f64::min);
    let ztol = 1e-9 * (1.0 + zmin.abs());
    let on_plane = |f: &outram_foam_mesh::block_mesh::MeshFace| {
        f.verts.iter().all(|&v| (m.points[v].z - zmin).abs() < ztol)
    };
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    if let Some(i) = args.iter().position(|a| a == "--box") {
        let v: Vec<f64> = args[i + 1..i + 5].iter().map(|s| s.parse().unwrap()).collect();
        (x0, x1, y0, y1) = (v[0], v[1], v[2], v[3]);
    } else {
        for p in &m.points {
            x0 = x0.min(p.x);
            x1 = x1.max(p.x);
            y0 = y0.min(p.y);
            y1 = y1.max(p.y);
        }
    }
    let w = 1400u32;
    let h = ((w as f64) * (y1 - y0) / (x1 - x0)).clamp(200.0, 1400.0) as u32;
    let mut img = RgbImage::from_pixel(w, h, Rgb([255, 255, 255]));
    let sx = (w as f64 - 20.0) / (x1 - x0);
    let sy = (h as f64 - 20.0) / (y1 - y0);
    let px = |p: outram_foam_basic_lib::primitives::Vector3| {
        (10.0 + (p.x - x0) * sx, h as f64 - 10.0 - (p.y - y0) * sy)
    };
    if let Some(zc) = cut {
        let seg = |f: &outram_foam_mesh::block_mesh::MeshFace| {
            let mut pts = Vec::new();
            for k in 0..f.verts.len() {
                let a = m.points[f.verts[k]];
                let b = m.points[f.verts[(k + 1) % f.verts.len()]];
                if (a.z - zc) * (b.z - zc) < 0.0 {
                    let t = (zc - a.z) / (b.z - a.z);
                    pts.push(a + (b - a) * t);
                }
            }
            pts
        };
        for f in &m.faces {
            let p = seg(f);
            if p.len() == 2 {
                line(&mut img, px(p[0]), px(p[1]), Rgb([150, 150, 150]));
            }
        }
        let palette = [[200, 30, 30], [30, 30, 200], [20, 140, 20]];
        for (pi, patch) in m.patches.iter().enumerate() {
            for f in &m.faces[patch.start..patch.start + patch.size] {
                let p = seg(f);
                if p.len() == 2 {
                    line(&mut img, px(p[0]), px(p[1]), Rgb(palette[pi % 3]));
                }
            }
        }
        img.save(out).unwrap();
        println!("section z={zc}: {} points, {} cells", m.points.len(), m.n_cells);
        return;
    }
    // All cell edges of the z-min plane, in grey.
    for f in m.faces.iter().filter(|f| on_plane(f)) {
        for k in 0..f.verts.len() {
            let a = m.points[f.verts[k]];
            let b = m.points[f.verts[(k + 1) % f.verts.len()]];
            line(&mut img, px(a), px(b), Rgb([150, 150, 150]));
        }
    }
    // Boundary edges, coloured by patch (side faces seen edge-on: their z-min edge).
    let palette = [[200, 30, 30], [30, 30, 200], [20, 140, 20], [200, 120, 0], [140, 0, 160]];
    for (pi, patch) in m.patches.iter().enumerate() {
        let col = Rgb(palette[pi % palette.len()]);
        for f in &m.faces[patch.start..patch.start + patch.size] {
            let vs: Vec<_> = f.verts.iter().filter(|&&v| (m.points[v].z - zmin).abs() < ztol).collect();
            if vs.len() == 2 {
                line(&mut img, px(m.points[*vs[0]]), px(m.points[*vs[1]]), col);
            }
        }
    }
    img.save(out).unwrap();
    println!(
        "{} points, {} cells, {} patches: {:?}",
        m.points.len(),
        m.n_cells,
        m.patches.len(),
        m.patches.iter().map(|p| (&p.name, p.kind)).collect::<Vec<_>>()
    );
}
