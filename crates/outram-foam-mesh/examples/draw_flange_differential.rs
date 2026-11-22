// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! gh:#1465/#1466: draw the final castellated flange mesh, ESI v2506 against
//! the port, as axis-aligned slices (AGENTS.md: meshes are drawn).
//!
//! `cargo run --release -p outram-foam-mesh --example draw_flange_differential -- OUT_DIR`
//!
//! Draws from the assembled meshes (ESI: the committed reference cells; port:
//! `castellate` output), never from constants. Colours: cells present in both
//! meshes are shaded by level (level 0 pale, 1 mid, 2 dark); cells only in ESI
//! are blue, only in the port red; the STL cut by the plane is black; ticks
//! every 10 mm (the box is 60 x 60 x 40 mm, the cell edges are 3 x 3.75 x
//! 3.33 mm at level 0). Writes `slice_z.png` (z = -0.009583, through the two
//! cells only the port keeps), `slice_y.png` (y = 0.01734375, same cells) and
//! `slice_z2.png` (z = -0.015).
//!
//! **Round 2 (2026-11-12):** the port side is now `with_esi_split_mesh_removal()` with
//! `with_surface_region_patches`, so the cells equal ESI's (no red or blue) and the
//! patch faces are drawn as dots coloured by patch (flange_patch1 orange, 2
//! magenta, 3 cyan, 4 yellow). A dot marks a patch face whose normal lies in the
//! plane and whose centre is within 0.8 mm of it, so dots can sit beside cells the
//! plane does not cut (faces of cells at nearby z).
//!
//! Example binary: drawing code, exempt from the reached-by-a-test rule (it
//! calls the tested `castellate` and `from_zoned`).

use std::collections::HashSet;

use image::{Rgb, RgbImage};
use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_mesh::snappy_hex_mesh::stl::read_stl_zoned_bytes;
use outram_foam_mesh::snappy_hex_mesh::{
    castellate, BackgroundMesh, Bounds, CastellationControls, TriangleSoup,
};

const ORG: [f64; 3] = [-0.03, -0.03, -0.03];
const SIZE0: [f64; 3] = [0.06 / 20.0, 0.06 / 16.0, 0.04 / 12.0];
const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/esi_v2506_flange");

type Cell = (usize, i64, i64, i64);

fn gunzip(bytes: &[u8]) -> Vec<u8> {
    miniz_oxide::inflate::decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

fn lo_hi(c: Cell) -> ([f64; 3], [f64; 3]) {
    let mut lo = [0.0; 3];
    let mut hi = [0.0; 3];
    let idx = [c.1, c.2, c.3];
    for a in 0..3 {
        let s = SIZE0[a] / (1usize << c.0) as f64;
        lo[a] = ORG[a] + idx[a] as f64 * s;
        hi[a] = lo[a] + s;
    }
    (lo, hi)
}

fn draw(
    path: &str,
    axis: usize,
    at: f64,
    both: &HashSet<Cell>,
    only_esi: &HashSet<Cell>,
    only_port: &HashSet<Cell>,
    surface: &TriangleSoup,
    faces: &[(Vector3, Vector3, usize)],
) {
    let (u, v) = match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    let ext = |a: usize| if a == 2 { (-0.03, 0.01) } else { (-0.03, 0.03) };
    let px_per_m = 15000.0;
    let (w, h) = (
        ((ext(u).1 - ext(u).0) * px_per_m) as u32 + 40,
        ((ext(v).1 - ext(v).0) * px_per_m) as u32 + 40,
    );
    let mut img = RgbImage::from_pixel(w, h, Rgb([255, 255, 255]));
    let pos = |x: f64, y: f64| -> (i64, i64) {
        (
            20 + ((x - ext(u).0) * px_per_m) as i64,
            (h as i64 - 20) - ((y - ext(v).0) * px_per_m) as i64,
        )
    };
    let mut fill = |c: Cell, col: Rgb<u8>| {
        let (lo, hi) = lo_hi(c);
        if !(lo[axis] <= at && at < hi[axis]) {
            return;
        }
        let (x0, y1) = pos(lo[u], lo[v]);
        let (x1, y0) = pos(hi[u], hi[v]);
        for x in x0..=x1 {
            for y in y0..=y1 {
                let edge = x == x0 || x == x1 || y == y0 || y == y1;
                let col = if edge { Rgb([60, 60, 60]) } else { col };
                if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                    img.put_pixel(x as u32, y as u32, col);
                }
            }
        }
    };
    let shade = [Rgb([225, 235, 225]), Rgb([160, 205, 160]), Rgb([90, 160, 90])];
    for &c in both {
        fill(c, shade[c.0]);
    }
    for &c in only_esi {
        fill(c, Rgb([70, 110, 230]));
    }
    for &c in only_port {
        fill(c, Rgb([230, 60, 60]));
    }
    // STL cut by the plane.
    for t in &surface.triangles {
        let p = [t.a, t.b, t.c];
        let g = |q: Vector3, a: usize| [q.x, q.y, q.z][a];
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for e in 0..3 {
            let (a, b) = (p[e], p[(e + 1) % 3]);
            let (da, db) = (g(a, axis) - at, g(b, axis) - at);
            if (da < 0.0) != (db < 0.0) {
                let s = da / (da - db);
                pts.push((
                    g(a, u) + s * (g(b, u) - g(a, u)),
                    g(a, v) + s * (g(b, v) - g(a, v)),
                ));
            }
        }
        if pts.len() == 2 {
            let (x0, y0) = pos(pts[0].0, pts[0].1);
            let (x1, y1) = pos(pts[1].0, pts[1].1);
            let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
            for i in 0..=n {
                let x = x0 + (x1 - x0) * i / n;
                let y = y0 + (y1 - y0) * i / n;
                if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                    img.put_pixel(x as u32, y as u32, Rgb([0, 0, 0]));
                }
            }
        }
    }
    // Surface-patch faces (round 2): a dot at the centre of every patch face whose
    // normal lies in the plane and whose centre is within 0.8 mm of the plane,
    // coloured by patch (flange_patch1 orange, 2 magenta, 3 cyan, 4 yellow).
    let patch_cols = [Rgb([240, 140, 0]), Rgb([200, 0, 200]), Rgb([0, 200, 220]), Rgb([230, 210, 0])];
    for (fc, area, patch) in faces {
        let g = |q: Vector3, a: usize| [q.x, q.y, q.z][a];
        if g(*area, axis).abs() > 0.0 || (g(*fc, axis) - at).abs() > 0.0008 {
            continue;
        }
        let (cx, cy) = pos(g(*fc, u), g(*fc, v));
        for dx in -2..=2i64 {
            for dy in -2..=2i64 {
                let (x, y) = (cx + dx, cy + dy);
                if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                    img.put_pixel(x as u32, y as u32, patch_cols[*patch % 4]);
                }
            }
        }
    }
    // Ticks every 10 mm on the lower and left borders.
    for k in 0..=6 {
        let (x, _) = pos(ext(u).0 + 0.01 * k as f64, ext(v).0);
        for d in 0..12 {
            if x >= 0 && (x as u32) < w {
                img.put_pixel(x as u32, (h - 1 - d).min(h - 1), Rgb([0, 0, 0]));
            }
        }
    }
    for k in 0..=((ext(v).1 - ext(v).0) / 0.01).round() as i64 {
        let (_, y) = pos(ext(u).0, ext(v).0 + 0.01 * k as f64);
        for d in 0..12 {
            if y >= 0 && (y as u32) < h {
                img.put_pixel(d, y as u32, Rgb([0, 0, 0]));
            }
        }
    }
    // Legend (top right): both level 0/1/2, only ESI, only port.
    let cols = [shade[0], shade[1], shade[2], Rgb([70, 110, 230]), Rgb([230, 60, 60])];
    for (i, c) in cols.iter().enumerate() {
        for x in 0..18 {
            for y in 0..18 {
                img.put_pixel(w - 30 - 24 * i as u32 + x, 10 + y, *c);
            }
        }
    }
    img.save(path).expect("png");
}

fn main() {
    let out = std::env::args().nth(1).expect("OUT_DIR");
    std::fs::create_dir_all(&out).unwrap();
    let stl = gunzip(&std::fs::read(format!("{FIX}/flange.stl.gz")).unwrap());
    let surface = TriangleSoup::from_zoned(&read_stl_zoned_bytes(&stl).unwrap());
    let domain = Bounds::new(Vector3::new(-0.03, -0.03, -0.03), Vector3::new(0.03, 0.03, 0.01));
    let c = CastellationControls::new(
        BackgroundMesh::uniform(domain, 20, 16, 12),
        2,
        Vector3::new(-9.23149e-05, -0.0025, -0.0025),
    )
    .with_surface_intersection_refinement();
    // Round 2: ESI's whole removal chain and one patch per STL region.
    let zoned = read_stl_zoned_bytes(&stl).unwrap();
    let names: Vec<String> = zoned.names.iter().map(|n| format!("flange_{n}")).collect();
    let c = c.with_esi_split_mesh_removal().with_surface_region_patches(names.clone(), zoned.zones.clone());
    let m = castellate(&surface, &c).unwrap();
    let mut faces: Vec<(Vector3, Vector3, usize)> = Vec::new();
    for p in &m.fv_mesh.patches {
        if let Some(r) = names.iter().position(|n| n == &p.name) {
            for f in p.start..p.start + p.size {
                faces.push((m.fv_mesh.face_centres[f], m.fv_mesh.face_area_vectors[f], r));
            }
        }
    }
    let v0 = SIZE0[0] * SIZE0[1] * SIZE0[2];
    let port: HashSet<Cell> = m
        .fv_mesh
        .cell_centres
        .iter()
        .zip(&m.fv_mesh.cell_volumes)
        .map(|(cc, vol)| {
            let l = ((v0 / vol).log2() / 3.0).round() as usize;
            let one = |x: f64, a: usize| {
                (((x - ORG[a]) / (SIZE0[a] / (1usize << l) as f64)) - 0.5).round() as i64
            };
            (l, one(cc.x, 0), one(cc.y, 1), one(cc.z, 2))
        })
        .collect();
    let text = String::from_utf8(gunzip(&std::fs::read(format!("{FIX}/esi_final_cells.txt.gz")).unwrap())).unwrap();
    let esi: HashSet<Cell> = text
        .lines()
        .map(|l| {
            let v: Vec<i64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
            (v[0] as usize, v[1], v[2], v[3])
        })
        .collect();
    let both: HashSet<Cell> = esi.intersection(&port).copied().collect();
    let only_esi: HashSet<Cell> = esi.difference(&port).copied().collect();
    let only_port: HashSet<Cell> = port.difference(&esi).copied().collect();
    println!("both {} only ESI {} only port {}", both.len(), only_esi.len(), only_port.len());
    draw(&format!("{out}/slice_z.png"), 2, -0.009583333333, &both, &only_esi, &only_port, &surface, &faces);
    draw(&format!("{out}/slice_y.png"), 1, 0.01734375, &both, &only_esi, &only_port, &surface, &faces);
    draw(&format!("{out}/slice_z2.png"), 2, -0.015, &both, &only_esi, &only_port, &surface, &faces);
}
