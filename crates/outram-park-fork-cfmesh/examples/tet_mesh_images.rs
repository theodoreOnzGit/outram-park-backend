// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See the crate's LICENSE.

//! Draw the meshes of the ported cfMesh `tetMesh` and `pMesh` workflows
//! (gh:#1272, #1273) as PNG slices, for a human to check (crate
//! `CLAUDE.md`: geometry is drawn before it is trusted).
//!
//! ```text
//! cargo run --release -p outram-park-fork-cfmesh --example tet_mesh_images -- <out_dir>
//! ```
//!
//! Each image is the octree's leaf types under the slice (grey borders), the
//! mesh faces cut by the slice plane (blue) and the surface trace (black),
//! all from the assembled structures. The image code (PNG writer, font,
//! slice, `draw`) is copied from `mesh_octree_images.rs` (gh:#1266), which
//! is an example and cannot be imported. `main` and drawing glue: no test
//! reaches them (workspace rule exception for GUI/CLI glue).

use outram_park_fork_cfmesh::math::Vec3;
use outram_park_fork_cfmesh::mesh_octree::{CubeType, MeshOctree};
use outram_park_fork_cfmesh::mesh_octree_creator::MeshDict;
use outram_park_fork_cfmesh::patches::SurfaceRegions;
use outram_park_fork_cfmesh::pipeline::p_mesh::p_mesh;
use outram_park_fork_cfmesh::pipeline::tet_mesh::{create_octree, tet_mesh, TetMeshOptions};
use outram_park_fork_cfmesh::cartesian::cartesian_box;
use outram_park_fork_cfmesh::fundamental_sheets::{create_fundamental_sheets, SheetsKind};
use outram_park_fork_cfmesh::mesh_surface_edge_extractor_fun::FunSheets;
use outram_park_fork_cfmesh::shapes::{box_surface, cylinder_surface, sphere_surface};
use outram_park_fork_cfmesh::topology::TopoMesh;
use outram_park_fork_cfmesh::tet_creator_octree::tet_creator_octree;
use outram_park_fork_cfmesh::tet_mesh_extractor_octree::tet_mesh_extractor_octree;
use outram_park_fork_cfmesh::voronoi_mesh_extractor::voronoi_mesh_extractor;
use outram_park_fork_cfmesh::volume_mesh::VolumeMesh;

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

/// A slice plane: `axis` normal (0 x, 1 y, 2 z) at `at`, drawn over
/// `[lo, hi]` in the two remaining axes (in increasing axis order).
struct Slice {
    axis: usize,
    at: f64,
    lo: (f64, f64),
    hi: (f64, f64),
    px: usize,
}

impl Slice {
    fn axes(&self) -> (usize, usize) {
        match self.axis {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        }
    }
    fn point(&self, i: usize, j: usize) -> Vec3 {
        let (a, b) = self.axes();
        let u = self.lo.0 + (self.hi.0 - self.lo.0) * (i as f64 + 0.5) / self.px as f64;
        let v = self.hi.1 - (self.hi.1 - self.lo.1) * (j as f64 + 0.5) / self.px as f64;
        let mut c = [0.0; 3];
        c[self.axis] = self.at;
        c[a] = u;
        c[b] = v;
        Vec3::new(c[0], c[1], c[2])
    }
    fn to_px(&self, p: Vec3) -> (f64, f64) {
        let (a, b) = self.axes();
        let c = [p.x, p.y, p.z];
        (
            (c[a] - self.lo.0) / (self.hi.0 - self.lo.0) * self.px as f64,
            (self.hi.1 - c[b]) / (self.hi.1 - self.lo.1) * self.px as f64,
        )
    }
    fn name(&self) -> String {
        let n = ["X", "Y", "Z"];
        let (a, b) = self.axes();
        format!("{}-{} AT {}={}", n[a], n[b], n[self.axis], self.at)
    }
}

/// Cut segments of a polygon (closed ring) with the slice plane.
fn cut(ring: &[Vec3], s: &Slice) -> Vec<(Vec3, Vec3)> {
    let comp = |p: Vec3| [p.x, p.y, p.z][s.axis] - s.at;
    let mut pts = Vec::new();
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        let (da, db) = (comp(a), comp(b));
        if (da <= 0.0 && db > 0.0) || (da > 0.0 && db <= 0.0) {
            let t = da / (da - db);
            pts.push(a.add(b.sub(a).scale(t)));
        }
    }
    pts.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0], c[1])).collect()
}

const TYPE_COLOURS: [(u8, Rgb, &str); 4] = [
    (CubeType::INSIDE, [120, 190, 120], "INSIDE"),
    (CubeType::DATA, [240, 160, 60], "DATA"),
    (CubeType::OUTSIDE, [205, 205, 205], "OUTSIDE"),
    (CubeType::UNKNOWN, [220, 60, 60], "UNKNOWN"),
];
const LEVEL_COLOURS: [Rgb; 10] = [
    [49, 54, 149], [69, 117, 180], [116, 173, 209], [171, 217, 233], [224, 243, 248],
    [254, 224, 144], [253, 174, 97], [244, 109, 67], [215, 48, 39], [165, 0, 38],
];

#[derive(Clone, Copy)]
enum Colouring {
    Type,
    Level,
}

/// Draw `o` on `s`, with the surface trace and optional mesh edges.
fn draw(o: &MeshOctree, s: &Slice, how: Colouring, mesh: Option<&VolumeMesh>, title: &str, path: &str) {
    let legend = 70;
    let mut img = Image::new(s.px, s.px + legend, [255, 255, 255]);
    let mut leaf_at = vec![usize::MAX; s.px * s.px];
    let (mut lmin, mut lmax) = (u8::MAX, 0u8);
    for j in 0..s.px {
        for i in 0..s.px {
            if let Some(l) = o.find_leaf_containing_vertex(s.point(i, j)) {
                leaf_at[j * s.px + i] = l;
                let lv = o.return_leaf(l).coords.level;
                lmin = lmin.min(lv);
                lmax = lmax.max(lv);
            }
        }
    }
    for j in 0..s.px {
        for i in 0..s.px {
            let l = leaf_at[j * s.px + i];
            if l == usize::MAX {
                continue;
            }
            let leaf = o.return_leaf(l);
            let c = match how {
                Colouring::Type => TYPE_COLOURS.iter().find(|t| leaf.cube_type & t.0 != 0).map_or([0, 0, 0], |t| t.1),
                Colouring::Level => LEVEL_COLOURS[(leaf.coords.level as usize).min(9)],
            };
            img.set(i as i64, j as i64, c);
            let right = if i + 1 < s.px { leaf_at[j * s.px + i + 1] } else { l };
            let down = if j + 1 < s.px { leaf_at[(j + 1) * s.px + i] } else { l };
            if right != l || down != l {
                img.set(i as i64, j as i64, [90, 90, 90]);
            }
        }
    }
    if let Some(m) = mesh {
        for f in &m.faces {
            let ring: Vec<Vec3> = f.iter().map(|&p| m.points[p]).collect();
            for (a, b) in cut(&ring, s) {
                img.line(s.to_px(a), s.to_px(b), [20, 20, 120]);
            }
        }
    }
    let surf = &o.surface;
    for t in &surf.triangles {
        let ring = [surf.points[t[0]], surf.points[t[1]], surf.points[t[2]]];
        for (a, b) in cut(&ring, s) {
            img.line(s.to_px(a), s.to_px(b), [0, 0, 0]);
        }
    }
    let y0 = s.px + 6;
    img.text(6, y0, &format!("{title}  {}", s.name()), 2, [0, 0, 0]);
    img.text(
        6,
        y0 + 14,
        &format!("U {:.4}..{:.4}  V {:.4}..{:.4} M  BLACK=SURFACE", s.lo.0, s.hi.0, s.lo.1, s.hi.1),
        2,
        [0, 0, 0],
    );
    let mut x = 6;
    match how {
        Colouring::Type => {
            for (_, c, name) in TYPE_COLOURS {
                img.rect(x, y0 + 32, 14, 14, c);
                img.text(x + 18, y0 + 35, name, 2, [0, 0, 0]);
                x += 18 + 8 * name.len() + 14;
            }
        }
        Colouring::Level => {
            for lv in lmin..=lmax {
                img.rect(x, y0 + 32, 14, 14, LEVEL_COLOURS[(lv as usize).min(9)]);
                img.text(x + 18, y0 + 35, &format!("L{lv}"), 2, [0, 0, 0]);
                x += 60;
            }
        }
    }
    if mesh.is_some() {
        img.text(x + 10, y0 + 35, "BLUE=MESH EDGES", 2, [20, 20, 120]);
    }
    img.write_png(path);
}

fn pipe() -> (Vec<Vec3>, Vec<[usize; 3]>, SurfaceRegions) {
    let (p, t) = cylinder_surface(Vec3::ZERO, 0.01, 0.06, 48);
    let labels: Vec<&str> = t
        .iter()
        .map(|tri| {
            let z = [p[tri[0]].z, p[tri[1]].z, p[tri[2]].z];
            let flat = (z[0] - z[1]).abs() < 1e-12 && (z[1] - z[2]).abs() < 1e-12;
            match (flat, z[0] < 0.03) {
                (true, true) => "inlet",
                (true, false) => "outlet",
                _ => "walls",
            }
        })
        .collect();
    let r = SurfaceRegions::from_labels(&labels);
    (p, t, r)
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "tet_mesh_images".into());
    std::fs::create_dir_all(&out).expect("output dir");
    let p = |n: &str| format!("{out}/{n}");

    // Sphere R 3 m at maxCellSize 0.5 (the V&V setup).
    let (sp, st) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
    let dict = MeshDict::with_max_cell_size(0.5);
    let o = create_octree(&sp, &st, None, &dict);
    let tc = tet_creator_octree(&o);
    let template = tet_mesh_extractor_octree(&tc).expect("template").mesh;
    let voronoi = voronoi_mesh_extractor(&tc).expect("voronoi").mesh;
    let opts = TetMeshOptions { dict: dict.clone(), ..Default::default() };
    let (tm, _) = tet_mesh(&sp, &st, None, &opts).expect("tet_mesh");
    let (pm, _) = p_mesh(&sp, &st, None, &opts).expect("p_mesh");
    let s = Slice { axis: 1, at: 0.01, lo: (-4.0, -4.0), hi: (4.0, 4.0), px: 900 };
    draw(&o, &s, Colouring::Type, Some(&template), "SPHERE TET TEMPLATE", &p("sphere_xz_tet_template.png"));
    draw(&o, &s, Colouring::Type, Some(&tm), "SPHERE TETMESH", &p("sphere_xz_tet_mesh.png"));
    draw(&o, &s, Colouring::Type, Some(&voronoi), "SPHERE VORONOI TEMPLATE", &p("sphere_xz_voronoi_template.png"));
    draw(&o, &s, Colouring::Type, Some(&pm), "SPHERE PMESH", &p("sphere_xz_p_mesh.png"));
    let z = Slice { axis: 2, at: 0.01, lo: (-4.0, -4.0), hi: (4.0, 4.0), px: 900 };
    draw(&o, &z, Colouring::Type, Some(&tm), "SPHERE TETMESH", &p("sphere_xy_tet_mesh.png"));

    // Pipe R 10 mm, L 60 mm, maxCellSize 2 mm (the V&V setup), and the
    // graded template (1 mm boundary cells).
    let (pp, pt, pr) = pipe();
    let dict = MeshDict::with_max_cell_size(0.002);
    let o = create_octree(&pp, &pt, Some(&pr), &dict);
    let opts = TetMeshOptions { dict: dict.clone(), ..Default::default() };
    let (tm, _) = tet_mesh(&pp, &pt, Some(&pr), &opts).expect("tet_mesh");
    let (pm, _) = p_mesh(&pp, &pt, Some(&pr), &opts).expect("p_mesh");
    let xy = Slice { axis: 2, at: 0.0301, lo: (-0.013, -0.013), hi: (0.013, 0.013), px: 900 };
    let xz = Slice { axis: 1, at: 0.0001, lo: (-0.013, -0.005), hi: (0.013, 0.065), px: 900 };
    draw(&o, &xy, Colouring::Type, Some(&tm), "PIPE TETMESH", &p("pipe_xy_tet_mesh.png"));
    draw(&o, &xz, Colouring::Type, Some(&tm), "PIPE TETMESH", &p("pipe_xz_tet_mesh.png"));
    draw(&o, &xy, Colouring::Type, Some(&pm), "PIPE PMESH", &p("pipe_xy_p_mesh.png"));
    let graded = MeshDict { boundary_cell_size: Some(0.001), ..MeshDict::with_max_cell_size(0.002) };
    let og = create_octree(&pp, &pt, Some(&pr), &graded);
    let tg = tet_mesh_extractor_octree(&tet_creator_octree(&og)).expect("graded template").mesh;
    draw(&og, &xy, Colouring::Level, Some(&tg), "PIPE GRADED TET TEMPLATE", &p("pipe_xy_graded_tet_template.png"));

    // gh:#1281: pMesh with and without createFundamentalSheetsJFS, and the
    // rim (inlet corner) in x-z.
    let without = TetMeshOptions { fun_sheets: FunSheets::Without, ..opts.clone() };
    let (pw, _) = p_mesh(&pp, &pt, Some(&pr), &without).expect("p_mesh without sheets");
    draw(&o, &xy, Colouring::Type, Some(&pw), "PIPE PMESH WITHOUT SHEETS", &p("pipe_xy_p_mesh_without_sheets.png"));
    let rim = Slice { axis: 1, at: 0.0001, lo: (0.004, -0.002), hi: (0.0115, 0.0055), px: 900 };
    draw(&o, &rim, Colouring::Type, Some(&pm), "PIPE PMESH RIM", &p("pipe_xz_rim_p_mesh.png"));
    draw(&o, &rim, Colouring::Type, Some(&pw), "PIPE PMESH RIM WITHOUT SHEETS", &p("pipe_xz_rim_p_mesh_without_sheets.png"));

    // gh:#1280/#1281: the fundamental sheets on a 3x3x3 unit box with six
    // patches (the unit tests' case), z = 0.5 and z = 0.02 slices.
    let (bp, bt) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
    let ob = create_octree(&bp, &bt, None, &MeshDict::with_max_cell_size(0.5));
    let bw = TopoMesh::from_volume_mesh(&cartesian_box(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0), [3, 3, 3]));
    let bz = Slice { axis: 2, at: 0.5, lo: (-0.15, -0.15), hi: (1.15, 1.15), px: 900 };
    let bz0 = Slice { axis: 2, at: 0.02, lo: (-0.15, -0.15), hi: (1.15, 1.15), px: 900 };
    for (kind, wrapper, name) in [(SheetsKind::Jfs, false, "jfs"), (SheetsKind::Fj, true, "fj")] {
        let (w, _) = create_fundamental_sheets(&bw, kind, wrapper).expect("sheets");
        let (m, _) = w.into_volume_mesh();
        draw(&ob, &bz, Colouring::Type, Some(&m), &format!("BOX 3X3X3 SHEETS {}", name.to_uppercase()), &p(&format!("box_xy_sheets_{name}.png")));
        draw(&ob, &bz0, Colouring::Type, Some(&m), &format!("BOX 3X3X3 SHEETS {}", name.to_uppercase()), &p(&format!("box_xy_z002_sheets_{name}.png")));
    }
}
