// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See the crate's LICENSE.

//! Draw the surfaces the gh:#1276 / gh:#1277 surface tools produce, for a
//! human to check (crate `CLAUDE.md`: geometry is drawn before it is
//! trusted).
//!
//! ```text
//! cargo run --release -p outram-park-fork-cfmesh --example surface_tools_images -- <out_dir>
//! ```
//!
//! Each image shows the assembled surface from two sides (from above and
//! from below; the L prism from two sides above; an orthographic view, facets painted back to front and shaded
//! by their normal), coloured by **patch** unless the title says otherwise;
//! black lines are the stored feature edges, grey lines every other edge.
//! - `pipe_patches_45deg.png`: the unlabelled pipe after `surfaceFeatureEdges`
//!   at 45° (3 patches);
//! - `pipe_patches_5deg.png`: the same at 5° (50 patches);
//! - `box_seeded_defects.png`: the gh:#1277 test box before cleaning, by
//!   defect (red: duplicated facet 0 and its copy; yellow: flipped facet;
//!   magenta: stray part; orange: facets on open edges; light grey: the
//!   rest);
//! - `box_cleaned.png`: the same box after `cleanup_duplicates`, the
//!   orientation flip and the removal of `region_1`, by patch;
//! - `l_prism_edge_types.png`: `triSurfaceClassifyEdges` on the L prism
//!   (blue convex, red concave, light grey flat).
//!
//! The image code (PNG writer, font) is copied from
//! `edge_extractor_images.rs` (an example; it cannot be imported). `main`
//! and drawing glue: no test reaches them (workspace rule exception for
//! GUI/CLI glue); every surface drawn is built by the routines that
//! `surface::tests` reaches.

use outram_park_fork_cfmesh::math::Vec3;
use outram_park_fork_cfmesh::mesh_octree::{MeshOctree, TriSurf};
use outram_park_fork_cfmesh::mesh_octree_creator::{MeshDict, MeshOctreeCreator};
use outram_park_fork_cfmesh::shapes::{box_surface, cylinder_surface};
use outram_park_fork_cfmesh::surface::checks::{check_disconnected_parts_subset, check_for_holes, check_orientation};
use outram_park_fork_cfmesh::surface::cleanup::{cleanup_duplicates, TriSurfaceRemoveFacets};
use outram_park_fork_cfmesh::surface::features::{surface_feature_edges, EdgeType, FeatureEdgesOutput, TriSurfaceClassifyEdges};
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


/// A distinct colour for patch `i` (golden-ratio hue walk).
fn palette(i: usize) -> Rgb {
    let h = (i as f64 * 0.618_033_988_75).fract() * 6.0;
    let (s, v) = (0.65, 0.95);
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as usize {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8]
}

fn shade(c: Rgb, f: f64) -> Rgb {
    [(c[0] as f64 * f) as u8, (c[1] as f64 * f) as u8, (c[2] as f64 * f) as u8]
}

/// Draw `s` from two sides, facets coloured by `fc`, each facet edge in the
/// colour `ec` gives it (`None`: grey). `legend` lines go under the title.
fn render<F: Fn(usize) -> Option<Rgb>>(s: &TriSurf, fc: &[Rgb], ec: &F, title: &str, legend: &[(&str, Rgb)], views: [Vec3; 2], path: &str) {
    let (w, h, top) = (640usize, 600usize, 70usize);
    let mut img = Image::new(2 * w, h + top, [255, 255, 255]);
    img.text(10, 8, title, 3, [0, 0, 0]);
    let mut lx = 10;
    for (name, c) in legend {
        img.rect(lx, 40, 14, 14, *c);
        img.text(lx + 18, 42, name, 2, [0, 0, 0]);
        lx += 18 + 8 * name.len() + 20;
    }
    let nrm = s.facet_normals();
    for (vi, eye) in views.iter().enumerate() {
        let d = eye.scale(-1.0).normalize();
        let right = d.cross(Vec3::new(0.0, 0.0, 1.0)).normalize();
        let up = right.cross(d);
        let pr: Vec<(f64, f64)> = s.points.iter().map(|p| (p.dot(right), -p.dot(up))).collect();
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for &(x, y) in &pr {
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        let sc = 0.9 * (w as f64 / (x1 - x0)).min(h as f64 / (y1 - y0));
        let ox = (vi * w) as f64 + 0.5 * (w as f64 - sc * (x1 - x0));
        let oy = top as f64 + 0.5 * (h as f64 - sc * (y1 - y0));
        let px = |p: usize| (ox + sc * (pr[p].0 - x0), oy + sc * (pr[p].1 - y0));
        let mut order: Vec<usize> = (0..s.size()).collect();
        let depth = |f: usize| s.triangles[f].iter().map(|&p| s.points[p].dot(d)).sum::<f64>();
        order.sort_by(|&a, &b| depth(b).partial_cmp(&depth(a)).unwrap());
        for f in order {
            let t = s.triangles[f];
            let cosv = nrm[f].dot(d);
            // Facing the viewer: cos < 0. Back faces are drawn darker.
            let k = if cosv < 0.0 { 0.55 + 0.45 * (-cosv) } else { 0.35 + 0.2 * cosv };
            fill(&mut img, &[px(t[0]), px(t[1]), px(t[2])], shade(fc[f], k));
            for i in 0..3 {
                let e = s.facet_edges[f][i];
                let c = ec(e).unwrap_or([150, 150, 150]);
                img.line(px(t[i]), px(t[(i + 1) % 3]), c);
            }
        }
        img.text(vi * w + 10, top + 4, &format!("VIEW FROM ({:.1} {:.1} {:.1}). DARKER: FACET FACES AWAY", eye.x, eye.y, eye.z), 2, [80, 80, 80]);
    }
    img.write_png(path);
}

fn by_patch(s: &TriSurf) -> Vec<Rgb> {
    s.regions.iter().map(|&r| palette(r)).collect()
}

fn box6() -> TriSurf {
    let (p, t) = box_surface(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
    let r = (0..t.len()).map(|i| i / 2).collect();
    TriSurf::new(p, t, Some(r))
}

/// The defect-seeded box of `surface::tests::seeded_box` (the stray
/// triangle moved to (1.6, 1.6, 1.6) to stay in frame); returns the
/// surface, the duplicated facet's copy and the flipped facet.
fn seeded_box() -> (TriSurf, usize, usize) {
    let b = box6();
    let mut p = b.points.clone();
    let mut t = b.triangles.clone();
    let mut r = b.regions.clone();
    p.push(p[6]);
    let k = t.iter().position(|tr| tr.contains(&6)).unwrap();
    for v in t[k].iter_mut() {
        if *v == 6 {
            *v = 8;
        }
    }
    t.push(t[0]);
    r.push(r[0]);
    let dup = t.len() - 1;
    t[4].swap(1, 2);
    let n = p.len();
    p.push(Vec3::new(1.6, 1.6, 1.6));
    p.push(Vec3::new(1.9, 1.6, 1.6));
    p.push(Vec3::new(1.6, 1.9, 1.6));
    t.push([n, n + 1, n + 2]);
    r.push(0);
    let mut s = TriSurf::new(p, t, Some(r));
    s.patches = b.patches.clone();
    (s, dup, 4)
}

fn l_prism() -> TriSurf {
    let poly = [(0.0, 0.0), (0.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0), (1.0, 0.0)];
    let n = poly.len();
    let mut p: Vec<Vec3> = poly.iter().map(|&(x, y)| Vec3::new(x, y, 0.0)).collect();
    p.extend(poly.iter().map(|&(x, y)| Vec3::new(x, y, 1.0)));
    let mut t = Vec::new();
    for i in 1..n - 1 {
        t.push([0, i + 1, i]);
        t.push([n, n + i, n + i + 1]);
    }
    for i in 0..n {
        let j = (i + 1) % n;
        t.push([i, j, n + j]);
        t.push([i, n + j, n + i]);
    }
    TriSurf::new(p, t, None)
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "verification_and_validation/surface_tools_gh1276".into());
    std::fs::create_dir_all(&out).expect("out dir");
    let (p, t) = cylinder_surface(Vec3::ZERO, 1.0, 4.0, 48);
    let pipe = TriSurf::new(p, t, None);
    for a in [45.0, 5.0] {
        let s = surface_feature_edges(&pipe, a, FeatureEdgesOutput::SplitIntoPatches);
        // Patch split drops the stored feature edges (as upstream); show the
        // ones it split at.
        let st = surface_feature_edges(&pipe, a, FeatureEdgesOutput::StoreFeatureEdges);
        let fl = st.stored_feature_edge_flags();
        let ec = |e: usize| {
            let [a0, b0] = s.edges[e];
            st.find_edge(a0, b0).filter(|&k| fl[k]).map(|_| [0, 0, 0])
        };
        let title = format!("UNLABELLED PIPE R1 L4 48 SIDES: SURFACEFEATUREEDGES AT {a:.0} DEG: {} PATCHES", s.patches.len());
        render(&s, &by_patch(&s), &ec, &title, &[("BLACK: FEATURE EDGES", [0, 0, 0])], [Vec3::new(1.0, -1.5, 1.0), Vec3::new(-1.0, 1.5, -1.0)], &format!("{out}/pipe_patches_{a:.0}deg.png"));
    }
    let (sb, dup, flipped) = seeded_box();
    let holes: Vec<usize> = check_for_holes(&sb);
    let mut fc = vec![[215, 215, 215]; sb.size()];
    for &f in &holes {
        fc[f] = [255, 150, 40];
    }
    fc[0] = [220, 30, 30];
    fc[dup] = [220, 30, 30];
    fc[flipped] = [240, 220, 30];
    fc[sb.size() - 1] = [220, 40, 220];
    let open = |e: usize| if sb.edge_facets[e].len() != 2 { Some([255, 0, 0]) } else { None };
    render(
        &sb,
        &fc,
        &open,
        "GH1277 BOX WITH SEEDED DEFECTS (BEFORE CLEANING)",
        &[("DUP FACET", [220, 30, 30]), ("FLIPPED", [240, 220, 30]), ("STRAY", [220, 40, 220]), ("ON OPEN EDGE", [255, 150, 40])],
        [Vec3::new(1.0, -1.5, 1.0), Vec3::new(-1.0, 1.5, -1.0)],
        &format!("{out}/box_seeded_defects.png"),
    );
    let mut o = MeshOctree::new(Arc::new(sb.clone()));
    MeshOctreeCreator::new(&o, None).create_octree_with_refined_boundary(&mut o, 20, 2);
    let (mut c, _) = cleanup_duplicates(&o, 1e-6);
    let (g, n) = check_orientation(&c);
    let mut size = vec![0usize; n];
    for x in g.iter().flatten() {
        size[*x] += 1;
    }
    let big = (0..n).max_by_key(|&i| size[i]).unwrap_or(0);
    let tris: Vec<[usize; 3]> = c.triangles.iter().enumerate().map(|(f, t)| if g[f] != Some(big) { [t[0], t[2], t[1]] } else { *t }).collect();
    let mut flippedc = TriSurf::from_parts(c.points.clone(), tris, c.regions.clone(), c.patches.clone(), Vec::new());
    std::mem::swap(&mut c, &mut flippedc);
    check_disconnected_parts_subset(&mut c, "region_");
    let mut rf = TriSurfaceRemoveFacets::default();
    rf.select_facets_in_subset("region_1");
    rf.remove_facets(&mut c);
    let vol = outram_park_fork_cfmesh::shapes::surface_volume(&c.points, &c.triangles);
    let none = |_e: usize| None;
    render(&c, &by_patch(&c), &none, &format!("GH1277 BOX AFTER CLEANING: {} FACETS {} POINTS VOLUME {vol}", c.size(), c.n_points()), &[], [Vec3::new(1.0, -1.5, 1.0), Vec3::new(-1.0, 1.5, -1.0)], &format!("{out}/box_cleaned.png"));
    let mut lo = MeshOctree::new(Arc::new(l_prism()));
    let mut cr = MeshOctreeCreator::new(&lo, Some(MeshDict::with_max_cell_size(0.25)));
    cr.create_octree_boxes(&mut lo);
    let cl = TriSurfaceClassifyEdges::new(&lo);
    let ls = &*lo.surface;
    let ec = |e: usize| {
        let t = cl.edge_types[e];
        Some(if t & EdgeType::CONCAVE != 0 {
            [230, 0, 0]
        } else if t & EdgeType::CONVEX != 0 {
            [0, 60, 230]
        } else {
            [200, 200, 200]
        })
    };
    render(
        ls,
        &vec![[235, 235, 235]; ls.size()],
        &ec,
        "L PRISM: TRISURFACECLASSIFYEDGES",
        &[("CONVEX", [0, 60, 230]), ("CONCAVE", [230, 0, 0]), ("FLAT", [200, 200, 200])],
        [Vec3::new(1.0, -1.5, 1.0), Vec3::new(1.5, 1.0, 1.2)],
        &format!("{out}/l_prism_edge_types.png"),
    );
}
