// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See the crate's LICENSE.

//! Draw the inlet rim of the gh:#1269 differential (Foam Studio pipe,
//! `CellCentre` + `Corners`), for a human to check (crate `CLAUDE.md`:
//! geometry is drawn before it is trusted).
//!
//! ```text
//! cargo run --release -p outram-park-fork-cfmesh --features cell-centre-dual \
//!     --example edge_extractor_images -- <out_dir>
//! ```
//!
//! For each h, two images, each with the nearest-triangle assignment on the
//! left and `edgeExtractor`'s on the right, both after the patch mapping:
//! `*_inlet_xy.png`, the inlet end seen along the axis (inlet faces filled
//! blue, wall faces within 1.5 h of the end outlined grey, faces whose patch
//! differs between the two rules filled red, the 48-gon black); and
//! `*_rim_unrolled.png`, the same faces unrolled (angle across, radius 9–10.05
//! mm down), a close-up of the rim's shape; and `*_rim_side.png`, the wall
//! near the inlet unrolled (angle across, z down). Everything is drawn from the
//! assembled meshes. The image code (PNG writer, font) is copied from
//! `tet_mesh_images.rs` (an example; it cannot be imported). `main` and
//! drawing glue: no test reaches them (workspace rule exception for GUI/CLI
//! glue); the computation is `edge_extractor::assignment_differential`,
//! reached by `diagnose_foam_studio_pipe_differential`.

use outram_park_fork_cfmesh::dual::{DualKind, FeaturePointRule};
use outram_park_fork_cfmesh::edge_extractor::{assignment_differential, AssignmentDifferential};
use outram_park_fork_cfmesh::math::Vec3;
use outram_park_fork_cfmesh::mesh_octree_creator::MeshDict;
use outram_park_fork_cfmesh::patches::SurfaceRegions;
use outram_park_fork_cfmesh::pipeline::tet_mesh::create_octree;
use outram_park_fork_cfmesh::pipeline::{surface_to_tet_dual_mesh_multipatch, TetDualOptions};
use outram_park_fork_cfmesh::shapes::cylinder_surface;
use outram_park_fork_cfmesh::surface_optimizer::map_mesh_to_surface;
use outram_park_fork_cfmesh::volume_mesh::VolumeMesh;
use std::f64::consts::PI;

type Rgb = [u8; 3];


struct Image {
    w: usize,
    h: usize,
    px: Vec<Rgb>,
}

impl Image {
    fn new(w: usize, h: usize, c: Rgb) -> Self {
        Image { w, h, px: vec![c; w * h] }
    }
    fn set(&mut self, x: i64, y: i64, c: Rgb) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }
    fn line(&mut self, a: (f64, f64), b: (f64, f64), c: Rgb) {
        let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as usize).max(1);
        for i in 0..=n {
            let t = i as f64 / n as f64;
            self.set((a.0 + (b.0 - a.0) * t).round() as i64, (a.1 + (b.1 - a.1) * t).round() as i64, c);
        }
    }
    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: Rgb) {
        for j in y..y + h {
            for i in x..x + w {
                self.set(i as i64, j as i64, c);
            }
        }
    }
    fn text(&mut self, x: usize, y: usize, s: &str, scale: usize, c: Rgb) {
        let mut cx = x;
        for ch in s.chars() {
            let g = glyph(ch);
            for (row, bits) in g.iter().enumerate() {
                for col in 0..3 {
                    if bits & (4 >> col) != 0 {
                        self.rect(cx + col * scale, y + row * scale, scale, scale, c);
                    }
                }
            }
            cx += 4 * scale;
        }
    }
    fn write_png(&self, path: &str) {
        let mut raw = Vec::with_capacity(self.h * (1 + 3 * self.w));
        for y in 0..self.h {
            raw.push(0u8);
            for x in 0..self.w {
                raw.extend_from_slice(&self.px[y * self.w + x]);
            }
        }
        let mut z = vec![0x78u8, 0x01];
        z.extend(deflate_fixed(&raw));
        let (mut a, mut b) = (1u32, 0u32);
        for &x in &raw {
            a = (a + x as u32) % 65521;
            b = (b + a) % 65521;
        }
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(self.w as u32).to_be_bytes());
        ihdr.extend_from_slice(&(self.h as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        std::fs::write(path, out).expect("write png");
        println!("wrote {path}");
    }
}

/// Bits packed least-significant first, as DEFLATE writes them.
struct BitWriter {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl BitWriter {
    fn bits(&mut self, v: u32, n: u32) {
        self.acc |= (v as u64) << self.n;
        self.n += n;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }
    /// A Huffman code, written most-significant bit first.
    fn code(&mut self, c: u32, n: u32) {
        let mut r = 0;
        for i in 0..n {
            r |= ((c >> i) & 1) << (n - 1 - i);
        }
        self.bits(r, n);
    }
}

/// One fixed-Huffman DEFLATE block (RFC 1951 §3.2.6). The only matches are
/// repeats at distance 3 (the previous RGB pixel), which is what flat
/// colour regions compress to.
fn deflate_fixed(data: &[u8]) -> Vec<u8> {
    const LBASE: [u32; 29] =
        [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
    const LEXTRA: [u32; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
    let mut w = BitWriter { out: Vec::new(), acc: 0, n: 0 };
    w.bits(1, 1); // final block
    w.bits(1, 2); // fixed Huffman
    let sym = |w: &mut BitWriter, s: u32| match s {
        0..=143 => w.code(0x30 + s, 8),
        144..=255 => w.code(0x190 + s - 144, 9),
        256..=279 => w.code(s - 256, 7),
        _ => w.code(0xC0 + s - 280, 8),
    };
    let mut i = 0;
    while i < data.len() {
        let mut run = 0;
        if i >= 3 {
            while run < 258 && i + run < data.len() && data[i + run] == data[i + run - 3] {
                run += 1;
            }
        }
        if run >= 3 {
            let k = LBASE.iter().rposition(|&b| b as usize <= run).expect("length code");
            sym(&mut w, 257 + k as u32);
            w.bits(run as u32 - LBASE[k], LEXTRA[k]);
            w.code(2, 5); // distance code 2: distance 3, no extra bits
            i += run;
        } else {
            sym(&mut w, data[i] as u32);
            i += 1;
        }
    }
    sym(&mut w, 256);
    if w.n > 0 {
        w.out.push(w.acc as u8);
    }
    w.out
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut kd = kind.to_vec();
    kd.extend_from_slice(data);
    out.extend_from_slice(&kd);
    out.extend_from_slice(&crc32(&kd).to_be_bytes());
}

/// 3 x 5 bitmap glyphs (bit 2 is the left column).
fn glyph(c: char) -> [u8; 5] {
    match c.to_ascii_uppercase() {
        '0' => [7, 5, 5, 5, 7],
        '1' => [2, 6, 2, 2, 7],
        '2' => [7, 1, 7, 4, 7],
        '3' => [7, 1, 7, 1, 7],
        '4' => [5, 5, 7, 1, 1],
        '5' => [7, 4, 7, 1, 7],
        '6' => [7, 4, 7, 5, 7],
        '7' => [7, 1, 1, 2, 2],
        '8' => [7, 5, 7, 5, 7],
        '9' => [7, 5, 7, 1, 7],
        'A' => [2, 5, 7, 5, 5],
        'B' => [6, 5, 6, 5, 6],
        'C' => [7, 4, 4, 4, 7],
        'D' => [6, 5, 5, 5, 6],
        'E' => [7, 4, 6, 4, 7],
        'F' => [7, 4, 6, 4, 4],
        'G' => [7, 4, 5, 5, 7],
        'H' => [5, 5, 7, 5, 5],
        'I' => [7, 2, 2, 2, 7],
        'J' => [1, 1, 1, 5, 7],
        'K' => [5, 5, 6, 5, 5],
        'L' => [4, 4, 4, 4, 7],
        'M' => [5, 7, 7, 5, 5],
        'N' => [6, 5, 5, 5, 5],
        'O' => [7, 5, 5, 5, 7],
        'P' => [7, 5, 7, 4, 4],
        'Q' => [7, 5, 5, 7, 1],
        'R' => [7, 5, 6, 5, 5],
        'S' => [7, 4, 7, 1, 7],
        'T' => [7, 2, 2, 2, 2],
        'U' => [5, 5, 5, 5, 7],
        'V' => [5, 5, 5, 5, 2],
        'W' => [5, 5, 7, 7, 5],
        'X' => [5, 5, 2, 5, 5],
        'Y' => [5, 5, 2, 2, 2],
        'Z' => [7, 1, 2, 4, 7],
        '.' => [0, 0, 0, 0, 2],
        '-' => [0, 0, 7, 0, 0],
        '=' => [0, 7, 0, 7, 0],
        ':' => [0, 2, 0, 2, 0],
        '/' => [1, 1, 2, 4, 4],
        '(' => [1, 2, 2, 2, 1],
        ')' => [4, 2, 2, 2, 4],
        _ => [0, 0, 0, 0, 0],
    }
}

/// Fill the polygon `pts` (pixel coordinates), even-odd scanline.
fn fill(img: &mut Image, pts: &[(f64, f64)], c: Rgb) {
    if pts.len() < 3 {
        return;
    }
    let y0 = pts.iter().map(|p| p.1).fold(f64::MAX, f64::min).floor().max(0.0) as i64;
    let y1 = pts.iter().map(|p| p.1).fold(f64::MIN, f64::max).ceil().min(img.h as f64 - 1.0) as i64;
    for y in y0..=y1 {
        let yc = y as f64 + 0.5;
        let mut xs = Vec::new();
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            if (a.1 <= yc) != (b.1 <= yc) {
                xs.push(a.0 + (yc - a.1) / (b.1 - a.1) * (b.0 - a.0));
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in xs.chunks(2) {
            if pair.len() == 2 {
                for x in pair[0].round() as i64..=pair[1].round() as i64 {
                    img.set(x, y, c);
                }
            }
        }
    }
}

fn pipe() -> (Vec<Vec3>, Vec<[usize; 3]>, SurfaceRegions) {
    let l = 0.2;
    let (p, t) = cylinder_surface(Vec3::ZERO, 0.01, l, 48);
    let labels: Vec<&str> = t
        .iter()
        .map(|tri| {
            let z = [p[tri[0]].z, p[tri[1]].z, p[tri[2]].z];
            let flat = (z[0] - z[1]).abs() < 1e-12 && (z[1] - z[2]).abs() < 1e-12;
            match (flat, z[0] < 0.5 * l) {
                (true, true) => "inlet",
                (true, false) => "outlet",
                _ => "walls",
            }
        })
        .collect();
    (p, t, SurfaceRegions::from_labels(&labels))
}

const BLUE: Rgb = [150, 190, 240];
const RED: Rgb = [230, 60, 50];
const GREY: Rgb = [150, 150, 150];
const DARK: Rgb = [40, 60, 110];
const BLACK: Rgb = [0, 0, 0];

/// The faces to draw: inlet faces, and wall faces within `zmax` of z = 0.
fn near_inlet(m: &VolumeMesh, reg: &[Option<usize>], inlet: usize, zmax: f64) -> Vec<(usize, bool)> {
    (0..m.face_count())
        .filter_map(|f| match reg[f] {
            Some(r) if r == inlet => Some((f, true)),
            Some(_) if m.faces[f].iter().all(|&v| m.points[v].z < zmax) => Some((f, false)),
            _ => None,
        })
        .collect()
}

/// One panel: `map` takes a point to pixel coordinates.
fn panel<F: Fn(Vec3) -> Option<(f64, f64)>>(img: &mut Image, m: &VolumeMesh, reg: &[Option<usize>], inlet: usize, h: f64, changed: &[usize], map: &F) {
    let faces = near_inlet(m, reg, inlet, 1.5 * h);
    for &(f, is_inlet) in &faces {
        let px: Option<Vec<(f64, f64)>> = m.faces[f].iter().map(|&v| map(m.points[v])).collect();
        let Some(px) = px else { continue };
        if changed.contains(&f) {
            fill(img, &px, RED);
        } else if is_inlet {
            fill(img, &px, BLUE);
        }
    }
    for &(f, is_inlet) in &faces {
        let px: Option<Vec<(f64, f64)>> = m.faces[f].iter().map(|&v| map(m.points[v])).collect();
        let Some(px) = px else { continue };
        for i in 0..px.len() {
            img.line(px[i], px[(i + 1) % px.len()], if is_inlet { DARK } else { GREY });
        }
    }
    // the faces whose patch differs, outlined thick in red
    for &f in changed.iter().filter(|f| faces.iter().any(|x| x.0 == **f)) {
        let px: Option<Vec<(f64, f64)>> = m.faces[f].iter().map(|&v| map(m.points[v])).collect();
        let Some(px) = px else { continue };
        for i in 0..px.len() {
            let (a, b) = (px[i], px[(i + 1) % px.len()]);
            for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)] {
                img.line((a.0 + dx, a.1 + dy), (b.0 + dx, b.1 + dy), RED);
            }
        }
    }
}

fn draw(d: &AssignmentDifferential, inlet: usize, h: f64, out: &str) {
    let changed: Vec<usize> = d.changed.iter().map(|x| x.0).collect();
    let r = 0.01;
    let polygon: Vec<Vec3> = (0..=48).map(|i| Vec3::new(r * (2.0 * PI * i as f64 / 48.0).cos(), r * (2.0 * PI * i as f64 / 48.0).sin(), 0.0)).collect();
    // x-y view of the inlet end
    let (w, s) = (720usize, 33_000.0);
    let mut img = Image::new(2 * w, w + 60, [255, 255, 255]);
    for (k, (m, reg, name)) in [(&d.nearest, &d.nearest_regions, "NEAREST TRIANGLE"), (&d.extracted, &d.extracted_regions, "EDGEEXTRACTOR")].into_iter().enumerate() {
        let ox = (k * w) as f64 + w as f64 / 2.0;
        let oy = 60.0 + w as f64 / 2.0;
        let map = |p: Vec3| Some((ox + p.x * s, oy - p.y * s));
        panel(&mut img, m, reg, inlet, h, &changed, &map);
        for i in 0..48 {
            img.line(map(polygon[i]).unwrap(), map(polygon[i + 1]).unwrap(), BLACK);
        }
        img.text(k * w + 20, 10, &format!("{name} H {:.0} MM: INLET SEEN ALONG Z", h * 1e3), 3, BLACK);
    }
    img.text(20, 40, "BLUE INLET  GREY WALL FACE EDGES  RED: PATCH DIFFERS  BLACK: 48-GON R 10 MM", 2, BLACK);
    img.write_png(&format!("{out}/foam_studio_h{:.0}_inlet_xy.png", h * 1e3));
    // unrolled rim: angle across, radius 7..10.2 mm up
    let (w, hh) = (1400usize, 300usize);
    let mut img = Image::new(w, 2 * hh + 100, [255, 255, 255]);
    for (k, (m, reg, name)) in [(&d.nearest, &d.nearest_regions, "NEAREST TRIANGLE"), (&d.extracted, &d.extracted_regions, "EDGEEXTRACTOR")].into_iter().enumerate() {
        let oy = 60.0 + (k * (hh + 20)) as f64;
        let (r0, r1) = (0.009, 0.01005);
        let map = |p: Vec3| {
            let a = p.y.atan2(p.x).rem_euclid(2.0 * PI);
            let rr = p.x.hypot(p.y);
            // faces across the 0 / 2 pi seam are dropped
            if a < 0.02 || a > 2.0 * PI - 0.02 {
                return None;
            }
            Some((a / (2.0 * PI) * w as f64, oy + ((r1 - rr) / (r1 - r0)).clamp(0.0, 1.0) * hh as f64))
        };
        panel(&mut img, m, reg, inlet, h, &changed, &map);
        for i in 0..48 {
            // the 48-gon's sides, r(theta) = R cos(pi/48) / cos(theta - mid)
            let n = 40;
            for j in 0..n {
                let t0 = 2.0 * PI * (i as f64 + j as f64 / n as f64) / 48.0;
                let t1 = 2.0 * PI * (i as f64 + (j + 1) as f64 / n as f64) / 48.0;
                let mid = 2.0 * PI * (i as f64 + 0.5) / 48.0;
                let rr = |t: f64| r * (PI / 48.0).cos() / (t - mid).cos();
                let p0 = Vec3::new(rr(t0) * t0.cos(), rr(t0) * t0.sin(), 0.0);
                let p1 = Vec3::new(rr(t1) * t1.cos(), rr(t1) * t1.sin(), 0.0);
                if let (Some(a), Some(b)) = (map(p0), map(p1)) {
                    img.line(a, b, BLACK);
                }
            }
        }
        img.text(20, oy as usize - 22, &format!("{name} H {:.0} MM: RIM UNROLLED (ANGLE 0-360 ACROSS, RADIUS 10.05 TO 9 MM DOWN, CLAMPED)", h * 1e3), 2, BLACK);
    }
    img.write_png(&format!("{out}/foam_studio_h{:.0}_rim_unrolled.png", h * 1e3));
    // side view of the rim: angle across, z from 0 down to 1.5 h
    let mut img = Image::new(w, 2 * hh + 100, [255, 255, 255]);
    for (k, (m, reg, name)) in [(&d.nearest, &d.nearest_regions, "NEAREST TRIANGLE"), (&d.extracted, &d.extracted_regions, "EDGEEXTRACTOR")].into_iter().enumerate() {
        let oy = 60.0 + (k * (hh + 20)) as f64;
        let map = |p: Vec3| {
            let a = p.y.atan2(p.x).rem_euclid(2.0 * PI);
            if a < 0.02 || a > 2.0 * PI - 0.02 {
                return None;
            }
            Some((a / (2.0 * PI) * w as f64, oy + (p.z / (1.5 * h)).clamp(0.0, 1.0) * hh as f64))
        };
        panel(&mut img, m, reg, inlet, h, &changed, &map);
        img.text(20, oy as usize - 22, &format!("{name} H {:.0} MM: WALL NEAR THE INLET UNROLLED (ANGLE ACROSS, Z 0 TO {:.1} MM DOWN)", h * 1e3, 1.5 * h * 1e3), 2, BLACK);
    }
    img.write_png(&format!("{out}/foam_studio_h{:.0}_rim_side.png", h * 1e3));
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    std::fs::create_dir_all(&out).expect("output dir");
    let (p, t, r) = pipe();
    let inlet = r.names.iter().position(|n| n == "inlet").unwrap();
    for h in [0.004, 0.005, 0.006] {
        let o = TetDualOptions {
            cell_size: h,
            n_layers: 0,
            dual_kind: DualKind::CellCentre,
            dual_feature_points: FeaturePointRule::Corners,
            feature_edges: false,
            ..Default::default()
        };
        let (m, _) = surface_to_tet_dual_mesh_multipatch(&p, &t, &r, &o).expect("dual");
        let (pre, _) = map_mesh_to_surface(&m, &p, &t, &r);
        let oct = create_octree(&p, &t, Some(&r), &MeshDict::with_max_cell_size(h));
        let d = assignment_differential(&pre, &p, &t, &r, &oct).expect("differential");
        println!("h {:.0} mm: {} faces differ", h * 1e3, d.changed.len());
        for &(f, a, b) in &d.changed {
            let c = pre.face_centroid(f);
            let s = pre.face_area_vector(f);
            println!(
                "  face {f}: {} -> {}, {} points, centroid r {:.3} mm theta {:.1} deg z {:.3} mm, |n_z| {:.3}",
                r.names[a],
                r.names[b],
                pre.faces[f].len(),
                c.x.hypot(c.y) * 1e3,
                c.y.atan2(c.x).to_degrees(),
                c.z * 1e3,
                (s.z / s.length()).abs()
            );
        }
        draw(&d, inlet, h, &out);
    }
}
