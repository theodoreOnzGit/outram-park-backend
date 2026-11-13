// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See the crate's LICENSE.

//! Draw the ported cfMesh octree (gh:#1262–#1265) and the cartesian meshes
//! extracted from it (gh:#1266) as PNG slices, for a human to check
//! (crate `CLAUDE.md`: geometry is drawn before it is trusted).
//!
//! ```text
//! cargo run --release -p outram-park-fork-cfmesh --example mesh_octree_images -- <out_dir>
//! cargo run --release -p outram-park-fork-cfmesh --example mesh_octree_images -- <out_dir> remainders
//! ```
//!
//! The second form draws only the octree remainders (gh:#1278 automatic
//! refinement, gh:#1264–#1266 sources, glued regions, cell selection).
//!
//! Every image is rendered from the **assembled** structure: each pixel
//! asks the octree for the leaf containing it (`findLeafContainingVertex`),
//! leaf borders are where the leaf changes between neighbouring pixels, the
//! surface is the triangles cut by the slice plane, and mesh edges are the
//! mesh faces cut by the plane. Each image carries a legend and its slice
//! extent in metres.
//!
//! The PNG writer (one fixed-Huffman DEFLATE block with distance-3 runs,
//! CRC-32, Adler-32) and the 3 x 5 font are
//! here because this crate depends on nothing (`outram-blender` has a PNG
//! writer but depends on this crate). `main` and drawing glue: no test
//! reaches them (workspace rule exception for GUI/CLI glue).

use outram_park_fork_cfmesh::cartesian_mesh::{cartesian_mesh_extractor, cartesian_mesh_extractor_from_dict, CartesianMeshOptions};
use outram_park_fork_cfmesh::math::Vec3;
use outram_park_fork_cfmesh::mesh_octree::{CubeType, MeshOctree, TriSurf};
use outram_park_fork_cfmesh::mesh_octree_automatic_refinement::MeshOctreeAutomaticRefinement;
use outram_park_fork_cfmesh::mesh_octree_creator::{EdgeMesh, EdgeMeshRefinement, MeshDict, MeshOctreeCreator};
use outram_park_fork_cfmesh::object_refinement::{ObjectRefinement, ObjectShape};
use outram_park_fork_cfmesh::shapes::{box_surface, box_surface_subdivided, cylinder_surface, sphere_surface};
use outram_park_fork_cfmesh::volume_mesh::VolumeMesh;
use std::sync::Arc;

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

fn creator(surf: Arc<TriSurf>, dict: MeshDict) -> MeshOctree {
    let mut o = MeshOctree::new(surf);
    let mut c = MeshOctreeCreator::new(&o, Some(dict));
    c.create_octree_boxes(&mut o);
    o
}

/// The octree remainders (gh:#1278 second half, gh:#1264/#1265/#1266
/// remainders): automatic refinement, refinement sources, glued regions and
/// the patch cell-selection options.
fn remainders(out: &str) {
    let p = |n: &str| format!("{out}/{n}");
    let sized = |surf: Arc<TriSurf>, dict: &MeshDict| {
        let mut o = MeshOctree::new(surf);
        let mut c = MeshOctreeCreator::new(&o, Some(dict.clone()));
        c.set_root_cube_size_and_ref_parameters(&mut o);
        c.refine_boundary(&mut o);
        o
    };

    // Proximity: slab [0,1]^2 x [0, 0.05], maxCellSize 0.125, minCellSize 0.005.
    let (sp, st, _) = box_surface_subdivided(Vec3::ZERO, Vec3::new(1.0, 1.0, 0.05), 0.02);
    let slab = Arc::new(TriSurf::new(sp, st, None));
    let dict = MeshDict { min_cell_size: Some(0.005), ..MeshDict::with_max_cell_size(0.125) };
    let before = sized(Arc::clone(&slab), &dict);
    let mut after = before.clone();
    let mut a = MeshOctreeAutomaticRefinement::new(&after, dict.min_cell_size, dict.max_cell_size, false, false);
    a.proximity_refinement(&mut after);
    let xz = Slice { axis: 1, at: 0.5001, lo: (-0.05, -0.2), hi: (1.05, 0.25), px: 900 };
    let xz_zoom = Slice { axis: 1, at: 0.5001, lo: (0.35, -0.05), hi: (0.5, 0.1), px: 800 };
    draw(&before, &xz, Colouring::Level, None, "SLAB GAP 0.05 BEFORE PROXIMITY", &p("slab_xz_level_before.png"));
    draw(&after, &xz, Colouring::Level, None, "SLAB GAP 0.05 PROXIMITY", &p("slab_xz_level.png"));
    draw(&after, &xz_zoom, Colouring::Level, None, "SLAB GAP ZOOM", &p("slab_xz_level_zoom.png"));

    // Curvature: sphere R 3, maxCellSize 1, minCellSize 0.1.
    let (qp, qt) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
    let sphere = Arc::new(TriSurf::new(qp, qt, None));
    let sd = MeshDict { min_cell_size: Some(0.1), ..MeshDict::with_max_cell_size(1.0) };
    let s0 = sized(Arc::clone(&sphere), &sd);
    let mut s1 = s0.clone();
    MeshOctreeAutomaticRefinement::new(&s1, sd.min_cell_size, sd.max_cell_size, false, false).curvature_refinement(&mut s1);
    let sxz = Slice { axis: 1, at: 0.01, lo: (-4.0, -4.0), hi: (4.0, 4.0), px: 800 };
    draw(&s0, &sxz, Colouring::Level, None, "SPHERE R3 BEFORE CURVATURE", &p("sphere_xz_level_before_curvature.png"));
    draw(&s1, &sxz, Colouring::Level, None, "SPHERE R3 CURVATURE", &p("sphere_xz_level_curvature.png"));

    // Edge-mesh source in the cube [0, 8]^3 (z = 4.1), 2 levels, thickness 0 and 1.
    let (bp, bt) = box_surface(Vec3::ZERO, Vec3::new(8.0, 8.0, 8.0));
    let cube = Arc::new(TriSurf::new(bp, bt, None));
    let (s, e) = (Vec3::new(1.3, 2.1, 4.1), Vec3::new(6.7, 5.9, 4.1));
    for (th, name) in [(0.0, "edge_source_xy_level.png"), (1.0, "edge_source_thick_xy_level.png")] {
        let d = MeshDict {
            edge_mesh_refinement: vec![EdgeMeshRefinement {
                name: "line".into(),
                edge_mesh: Arc::new(EdgeMesh { points: vec![s, e], edges: vec![[0, 1]] }),
                cell_size: None,
                additional_levels: Some(2),
                refinement_thickness: th,
            }],
            ..MeshDict::with_max_cell_size(1.0)
        };
        let o = creator(Arc::clone(&cube), d);
        let z = Slice { axis: 2, at: 4.1, lo: (-0.5, -0.5), hi: (8.5, 8.5), px: 800 };
        draw(&o, &z, Colouring::Level, None, &format!("EDGE SOURCE THICKNESS {th}"), &p(name));
    }

    // Glued regions: two unit cubes 0.05 apart, DATA boxes kept.
    let (mut gp, mut gt) = box_surface(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
    let (gp2, gt2) = box_surface(Vec3::new(1.05, 0.0, 0.0), Vec3::new(2.05, 1.0, 1.0));
    let off = gp.len();
    gp.extend(gp2);
    gt.extend(gt2.iter().map(|t| [t[0] + off, t[1] + off, t[2] + off]));
    let regions: Vec<usize> = (0..gt.len()).map(|i| i / 12).collect();
    let boxes = Arc::new(TriSurf::new(gp, gt, Some(regions)));
    let gd = MeshDict { keep_cells_intersecting_boundary: true, ..MeshDict::with_max_cell_size(0.2) };
    let go = creator(boxes, gd.clone());
    let opts = CartesianMeshOptions { use_data_boxes: true, ..CartesianMeshOptions::default() };
    let glued = cartesian_mesh_extractor_from_dict(&go, &opts, &gd);
    let checked = cartesian_mesh_extractor_from_dict(&go, &opts, &MeshDict { check_for_glued_mesh: true, ..gd });
    let gz = Slice { axis: 2, at: 0.5001, lo: (-0.3, -0.3), hi: (2.35, 1.3), px: 900 };
    draw(&go, &gz, Colouring::Type, Some(&glued.mesh), "TWO BOXES GLUED", &p("two_boxes_xy_glued.png"));
    draw(&go, &gz, Colouring::Type, Some(&checked.mesh), "TWO BOXES CHECKFORGLUEDMESH", &p("two_boxes_xy_checked.png"));

    // removeCellsIntersectingPatches [inlet] on the pipe, DATA boxes kept.
    let (pp, pt) = cylinder_surface(Vec3::ZERO, 0.01, 0.06, 48);
    let pr: Vec<usize> = pt
        .iter()
        .map(|t| {
            let z = [pp[t[0]].z, pp[t[1]].z, pp[t[2]].z];
            let flat = (z[0] - z[1]).abs() < 1e-12 && (z[1] - z[2]).abs() < 1e-12;
            match (flat, z[0] < 0.03) {
                (true, true) => 1,
                (true, false) => 2,
                _ => 0,
            }
        })
        .collect();
    let pipe_surf = Arc::new(TriSurf::new(pp, pt, Some(pr)));
    let inlet = pipe_surf.patches[1].name.clone();
    let pd = MeshDict { boundary_cell_size: Some(0.001), ..MeshDict::with_max_cell_size(0.004) };
    let po = creator(pipe_surf, pd);
    let rd = MeshDict { keep_cells_intersecting_boundary: true, remove_cells_intersecting_patches: vec![inlet], ..MeshDict::default() };
    let pm = cartesian_mesh_extractor_from_dict(&po, &opts, &rd);
    let pxz = Slice { axis: 1, at: 0.0001, lo: (-0.015, -0.005), hi: (0.015, 0.012), px: 800 };
    draw(&po, &pxz, Colouring::Type, Some(&pm.mesh), "PIPE DATA BOXES REMOVE INLET", &p("pipe_xz_remove_inlet.png"));
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "mesh_octree_images".into());
    std::fs::create_dir_all(&out).expect("output dir");
    if std::env::args().nth(2).as_deref() == Some("remainders") {
        remainders(&out);
        return;
    }
    let p = |n: &str| format!("{out}/{n}");

    // Sphere R 3 m, maxCellSize 1.0, boundaryCellSize 0.25 (gh:#1264 test).
    let (sp, st) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
    let sphere = creator(Arc::new(TriSurf::new(sp, st, None)), MeshDict { boundary_cell_size: Some(0.25), ..MeshDict::with_max_cell_size(1.0) });
    let s = Slice { axis: 1, at: 0.01, lo: (-4.0, -4.0), hi: (4.0, 4.0), px: 800 };
    draw(&sphere, &s, Colouring::Type, None, "SPHERE R3 TYPE", &p("sphere_xz_type.png"));
    draw(&sphere, &s, Colouring::Level, None, "SPHERE R3 LEVEL", &p("sphere_xz_level.png"));

    // 48-sided pipe R 10 mm, L 60 mm, maxCellSize 4 mm, boundaryCellSize 1 mm.
    let (pp, pt) = cylinder_surface(Vec3::ZERO, 0.01, 0.06, 48);
    let regions: Vec<usize> = pt
        .iter()
        .map(|t| {
            let z = [pp[t[0]].z, pp[t[1]].z, pp[t[2]].z];
            let flat = (z[0] - z[1]).abs() < 1e-12 && (z[1] - z[2]).abs() < 1e-12;
            match (flat, z[0] < 0.03) {
                (true, true) => 1,
                (true, false) => 2,
                _ => 0,
            }
        })
        .collect();
    let pipe_surf = Arc::new(TriSurf::new(pp, pt, Some(regions)));
    let pipe = creator(Arc::clone(&pipe_surf), MeshDict { boundary_cell_size: Some(0.001), ..MeshDict::with_max_cell_size(0.004) });
    let xy = Slice { axis: 2, at: 0.0301, lo: (-0.013, -0.013), hi: (0.013, 0.013), px: 800 };
    let xz = Slice { axis: 1, at: 0.0001, lo: (-0.035, -0.005), hi: (0.035, 0.065), px: 800 };
    draw(&pipe, &xy, Colouring::Type, None, "PIPE48 TYPE", &p("pipe_xy_type.png"));
    draw(&pipe, &xy, Colouring::Level, None, "PIPE48 LEVEL", &p("pipe_xy_level.png"));
    draw(&pipe, &xz, Colouring::Type, None, "PIPE48 TYPE", &p("pipe_xz_type.png"));

    // Object refinement: the cube [0, 8]^3 with a sphere object (gh:#1265).
    let (bp, bt) = box_surface(Vec3::ZERO, Vec3::new(8.0, 8.0, 8.0));
    let cube = Arc::new(TriSurf::new(bp, bt, None));
    let obj = ObjectRefinement::with_cell_size("ball", ObjectShape::Sphere { centre: Vec3::new(4.0, 4.0, 4.0), radius: 1.3 }, 0.25);
    let line = ObjectRefinement::with_cell_size("line", ObjectShape::Line { p0: Vec3::new(1.2, 2.0, 4.0), p1: Vec3::new(6.9, 6.5, 4.0) }, 0.25);
    let objects = creator(cube, MeshDict { object_refinements: vec![obj, line], ..MeshDict::with_max_cell_size(1.0) });
    let oz = Slice { axis: 2, at: 4.01, lo: (-0.5, -0.5), hi: (8.5, 8.5), px: 800 };
    draw(&objects, &oz, Colouring::Level, None, "OBJECTS SPHERE+LINE", &p("objects_xy_level.png"));

    // Extracted cartesian meshes (gh:#1266), edges over the octree.
    let opts = CartesianMeshOptions::default();
    let m = cartesian_mesh_extractor(&sphere, &opts);
    draw(&sphere, &s, Colouring::Type, Some(&m.mesh), "SPHERE CARTESIANMESH", &p("sphere_xz_cartesian_mesh.png"));
    let mp = cartesian_mesh_extractor(&pipe, &opts);
    draw(&pipe, &xy, Colouring::Type, Some(&mp.mesh), "PIPE48 CARTESIANMESH", &p("pipe_xy_cartesian_mesh.png"));
    draw(&pipe, &xz, Colouring::Type, Some(&mp.mesh), "PIPE48 CARTESIANMESH", &p("pipe_xz_cartesian_mesh.png"));
    let split = CartesianMeshOptions { decompose_split_hexes: true, ..opts };
    let ms = cartesian_mesh_extractor(&pipe, &split);
    draw(&pipe, &xy, Colouring::Level, Some(&ms.mesh), "PIPE48 SPLIT HEXES DECOMPOSED", &p("pipe_xy_cartesian_mesh_decomposed.png"));
}
