//! The drawing rule's review images for the TetGen `-p` route (gh:#1794):
//! z- and x-sections of the **TetGen tetrahedral mesh** of Mesh Studio's
//! pipe, fuel channel and pebble surfaces (size 2), drawn from the
//! assembled mesh, never from the named dimensions.
//!
//! ```text
//! cargo run -p dhoby-ghaut --example mesh_studio --release -- --tetgen-render <dir> [switches]
//! ```
//!
//! `switches` defaults to `pzA` (stage 5); stage 6 (gh:#1795) draws the
//! refined meshes with `pq1.414a0.0026zA` (`-A` only colours the regions).
//!
//! The chain is the crate's, end to end: [`super::model::reactor_surface`]
//! (the surface Mesh Studio meshes), `PlcInput::from_triangle_soup` with one
//! marker per patch, `tetgen::tetrahedralize_plc` under `-pzA` (every
//! enclosed region gets an attribute), `tetgen::to_volume_mesh`, then
//! [`super::section`] (`outram_blender`'s converters and
//! `outram_foam_mesh::section::plane_section`, Foam Studio's cut). Each cut
//! tetrahedron is one polygon, filled by its region attribute and outlined,
//! so the element edges, the regions and any hole are visible.
//!
//! `graded` as the switches (gh:#1825) draws the page's wall-graded TetGen
//! meshes instead (`?grade=on`: the design's `-pq1.414a<V>m` and the
//! `-m` sizes and seed points of `pipeline::tet_source::tetgen_plc`, with
//! `-A` for the colours), adding a zoom of the z-cut at the walls.
//!
//! Native only (it writes PNGs with `outram_blender`'s plot image).

use super::model::{reactor_surface, GeomKind};
use super::section::{cut, to_poly};
use outram_blender::csg::plot::{annotate_image, ImageData, LegendEntry, Rgb};
use outram_park_fork_cfmesh::patches::SurfaceRegions;
use outram_park_fork_cfmesh::pipeline::tet_source::tetgen_plc;
use outram_park_fork_cfmesh::tetgen::{
    tetrahedralize_plc, to_volume_mesh, Behavior, PlcInput, Switches,
};

const BG: Rgb = Rgb::new(255, 255, 255);
const EDGE: Rgb = Rgb::new(30, 30, 30);
/// Region fill colours, by attribute order (1st, 2nd, ... region found).
const REGION: [Rgb; 4] = [
    Rgb::new(150, 200, 240),
    Rgb::new(250, 190, 120),
    Rgb::new(170, 230, 160),
    Rgb::new(230, 160, 210),
];

/// Mean-edge colours of a graded mesh, small (dark) to large (light).
const SIZE: [Rgb; 6] = [
    Rgb::new(68, 1, 84),
    Rgb::new(65, 68, 135),
    Rgb::new(42, 120, 142),
    Rgb::new(34, 168, 132),
    Rgb::new(122, 209, 81),
    Rgb::new(253, 231, 37),
];

/// The mean edge length of every cell of a tetrahedral mesh, from its
/// faces (each cell's distinct vertices, every pair an edge).
fn cell_mean_edges(vm: &outram_park_fork_cfmesh::volume_mesh::VolumeMesh) -> Vec<f64> {
    let mut verts: Vec<Vec<usize>> = vec![Vec::new(); vm.n_cells];
    for (f, face) in vm.faces.iter().enumerate() {
        let mut add = |c: usize| {
            for &v in face {
                if !verts[c].contains(&v) {
                    verts[c].push(v);
                }
            }
        };
        add(vm.owner[f]);
        if let Some(n) = vm.neighbour[f] {
            add(n);
        }
    }
    verts
        .iter()
        .map(|v| {
            let (mut s, mut n) = (0.0, 0usize);
            for i in 0..v.len() {
                for j in i + 1..v.len() {
                    s += vm.points[v[i]].sub(vm.points[v[j]]).length();
                    n += 1;
                }
            }
            s / n.max(1) as f64
        })
        .collect()
}

/// The log bin of `x` in `[lo, hi]`, `0..SIZE.len()`.
fn size_bin(x: f64, lo: f64, hi: f64) -> usize {
    let t = if hi > lo {
        (x / lo).ln() / (hi / lo).ln()
    } else {
        0.0
    };
    ((t * SIZE.len() as f64) as usize).min(SIZE.len() - 1)
}

/// The lower edge of log bin `k` (the upper of `k - 1`).
fn bin_edge(k: usize, lo: f64, hi: f64) -> f64 {
    lo * (hi / lo).powf(k as f64 / SIZE.len() as f64)
}

/// One case's mesh and what the image reports about it.
pub struct Drawn {
    /// The images: (file name, image).
    pub images: Vec<(String, ImageData)>,
    /// Tetrahedra, nodes, distinct region attributes.
    pub counts: (usize, usize, Vec<f64>),
    /// Cut cells per image.
    pub cut_cells: Vec<usize>,
}

/// Mesh one Mesh Studio surface with TetGen (`-pzA`) and draw its z-cut at
/// mid-height and its x-cut through the axis (`x = 1e-3 size`, off the
/// axis vertices).
///
/// # Errors
/// When TetGen or a converter refuses the input (the message says which).
pub fn draw(kind: GeomKind, size: f64) -> Result<Drawn, String> {
    draw_with(kind, size, "pzA")
}

/// [`draw`] under the TetGen switches `sw` (e.g. `"pq1.414a0.0026zA"`, the
/// Delaunay-refined mesh of stage 6, gh:#1795).
///
/// # Errors
/// As [`draw`], and when `sw` is not a switch string the port takes.
pub fn draw_with(kind: GeomKind, size: f64, sw: &str) -> Result<Drawn, String> {
    let (points, tris, labels, _walls) = reactor_surface(kind, size);
    let regions = SurfaceRegions::from_labels(&labels);
    let pts: Vec<[f64; 3]> = points.iter().map(|p| [p.x, p.y, p.z]).collect();
    let markers: Vec<i32> = regions.region_of_tri.iter().map(|&r| r as i32).collect();
    let plc = PlcInput::from_triangle_soup(&pts, &tris, &markers);
    let b = Switches::parse(sw)?.behavior();
    draw_plc(kind, size, &plc, &b, &regions, sw, "", false)
}

/// The page's wall-graded TetGen mesh of `kind` (gh:#1825): the design
/// `shape=<kind>&grade=on` (walls half the cell size, growth 1.2, the
/// cell size inside), its PLC with the `-m` sizes and seed points
/// ([`tetgen_plc`]), plus `-A` for the colours; drawn as [`draw_with`]
/// does, with a third image zoomed on the z-cut at the walls.
///
/// # Errors
/// As [`draw`].
pub fn draw_graded(kind: GeomKind, size: f64) -> Result<Drawn, String> {
    let q: Vec<(String, String)> = [("shape", kind.slug()), ("grade", "on")]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    let mut d = super::engine::Design::from_query(&q);
    d.size = size;
    let src = d.tet_source();
    let (points, tris, labels, walls) = reactor_surface(kind, size);
    let regions = SurfaceRegions::from_labels(&labels);
    let run = tetgen_plc(&points, &tris, Some(&regions), "walls", &walls, src)?;
    let mut b = run.behavior;
    b.regionattrib = true;
    let sw = src.switches().unwrap_or_default() + "A";
    draw_plc(kind, size, &run.plc, &b, &regions, &sw, "_graded", true)
}

/// Mesh `plc` under `b` and draw its z- and x-cuts (and, with `zoom`, the
/// z-cut at the walls: the box from 0.45 to 1.02 of the channel radius in
/// x, a quarter of it either side in y). `label` goes into the file names.
#[allow(clippy::too_many_arguments)]
fn draw_plc(
    kind: GeomKind,
    size: f64,
    plc: &PlcInput,
    b: &Behavior,
    regions: &SurfaceRegions,
    sw: &str,
    label: &str,
    zoom: bool,
) -> Result<Drawn, String> {
    let r = tetrahedralize_plc(plc, b).map_err(|e| format!("tetgen: {e:?}"))?;
    let out = r.output.ok_or("no output")?;
    let vm = to_volume_mesh(&out, Some(&regions.names));
    let attrs = out.tet_attributes.clone().unwrap_or_default();
    let mut distinct: Vec<f64> = Vec::new();
    for &a in &attrs {
        if !distinct.contains(&a) {
            distinct.push(a);
        }
    }
    distinct.sort_by(|a, b| a.total_cmp(b));
    // A graded mesh (gh:#1825) is coloured by each tetrahedron's mean
    //   edge length (log bins between the mesh's extremes), so that the
    //   grading is read off the colours; others by region.
    let sizes = if zoom {
        Some(cell_mean_edges(&vm))
    } else {
        None
    };
    let size_bins = sizes.as_ref().map(|m| {
        let lo = m.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = m.iter().copied().fold(0.0, f64::max);
        (lo, hi)
    });
    let colour_of = |c: usize| {
        if let (Some(m), Some((lo, hi))) = (&sizes, size_bins) {
            return SIZE[size_bin(m[c], lo, hi)];
        }
        let a = attrs.get(c).copied().unwrap_or(0.0);
        let k = distinct.iter().position(|&d| d == a).unwrap_or(0);
        REGION[k % REGION.len()]
    };
    let poly = to_poly(&vm)?;
    let (lo, hi) = super::section::bounds(&vm);
    let zmid = 0.5 * (lo[2] + hi[2]);
    let xoff = 1e-3 * size;
    let mut images = Vec::new();
    let mut cut_cells = Vec::new();
    let rc = 0.5 * size;
    let mut views = vec![
        (
            2u8,
            zmid,
            "z",
            format!("Z = {zmid:.3} M (MID-HEIGHT)"),
            None,
        ),
        (
            0u8,
            xoff,
            "x",
            format!("X = {xoff:.3} M (THROUGH THE AXIS)"),
            None,
        ),
    ];
    if zoom {
        views.push((
            2u8,
            zmid,
            "zoom",
            format!("Z = {zmid:.3} M, ZOOM AT THE WALLS"),
            Some(([0.45 * rc, 1.02 * rc], [-0.25 * rc, 0.25 * rc])),
        ));
    }
    for (axis, offset, tag, what, window) in views {
        let sections = cut(&poly, axis, offset);
        cut_cells.push(sections.len());
        // In-plane extent: (x, y) for a z-cut, (y, z) for an x-cut.
        let (u, v) = match window {
            Some(w) => w,
            None if axis == 2 => ([lo[0], hi[0]], [lo[1], hi[1]]),
            None => ([lo[1], hi[1]], [lo[2], hi[2]]),
        };
        let scale = 760.0 / (u[1] - u[0]).max(v[1] - v[0]);
        let (w, h) = (
            ((u[1] - u[0]) * scale) as usize + 41,
            ((v[1] - v[0]) * scale) as usize + 41,
        );
        let mut img = ImageData::filled(w, h, BG);
        let to_px = |p: [f64; 2]| {
            (
                20.0 + (p[0] - u[0]) * scale,
                h as f64 - 20.0 - (p[1] - v[0]) * scale,
            )
        };
        for s in &sections {
            let ring: Vec<(f64, f64)> = s.polygon.iter().map(|&p| to_px(p)).collect();
            fill_polygon(&mut img, &ring, colour_of(s.cell));
        }
        for s in &sections {
            let ring: Vec<(f64, f64)> = s.polygon.iter().map(|&p| to_px(p)).collect();
            for k in 0..ring.len() {
                line(&mut img, ring[k], ring[(k + 1) % ring.len()], EDGE);
            }
        }
        let legend: Vec<LegendEntry> = match (&sizes, size_bins) {
            (Some(m), Some((lo, hi))) => (0..SIZE.len())
                .map(|k| {
                    let (a, b) = (bin_edge(k, lo, hi), bin_edge(k + 1, lo, hi));
                    let n = m.iter().filter(|&&x| size_bin(x, lo, hi) == k).count();
                    LegendEntry::new(SIZE[k], format!("MEAN EDGE {a:.3} TO {b:.3} M ({n} TETS)"))
                })
                .collect(),
            _ => distinct
                .iter()
                .enumerate()
                .map(|(k, a)| {
                    let n = attrs.iter().filter(|&&x| x == *a).count();
                    LegendEntry::new(
                        REGION[k % REGION.len()],
                        format!("REGION ATTRIBUTE {a} ({n} TETS)"),
                    )
                })
                .collect(),
        };
        let title = format!(
            "TETGEN -{}, {} SIZE {size}: {what}, {} CUT TETS OF {}, {} NODES",
            sw.to_uppercase(),
            kind.slug().to_uppercase(),
            sections.len(),
            vm.n_cells,
            out.points.len()
        );
        images.push((
            format!("{}{label}_{tag}_cut.png", kind.slug()),
            annotate_image(&img, &title, &legend),
        ));
    }
    Ok(Drawn {
        images,
        counts: (out.tets.len(), out.points.len(), distinct),
        cut_cells,
    })
}

/// Fill a convex (or star-shaped about its first vertex) polygon in pixel
/// coordinates: each pixel centre inside by the crossing rule.
fn fill_polygon(img: &mut ImageData, ring: &[(f64, f64)], c: Rgb) {
    if ring.len() < 3 {
        return;
    }
    let (mut y0, mut y1) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut x0, mut x1) = (f64::INFINITY, f64::NEG_INFINITY);
    for &(x, y) in ring {
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    for py in (y0.floor().max(0.0) as usize)
        ..=(y1.ceil().max(0.0) as usize).min(img.height.saturating_sub(1))
    {
        for px in (x0.floor().max(0.0) as usize)
            ..=(x1.ceil().max(0.0) as usize).min(img.width.saturating_sub(1))
        {
            let (x, y) = (px as f64 + 0.5, py as f64 + 0.5);
            let mut inside = false;
            for k in 0..ring.len() {
                let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
                if (a.1 > y) != (b.1 > y) && x < a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0) {
                    inside = !inside;
                }
            }
            if inside {
                img.set(px, py, c);
            }
        }
    }
}

/// A one-pixel line (DDA).
fn line(img: &mut ImageData, a: (f64, f64), b: (f64, f64), c: Rgb) {
    let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as usize).max(1);
    for k in 0..=n {
        let t = k as f64 / n as f64;
        let (x, y) = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
        if x >= 0.0 && y >= 0.0 && (x as usize) < img.width && (y as usize) < img.height {
            img.set(x as usize, y as usize, c);
        }
    }
}

/// Draw the three cases into `dir` and return one summary line each.
/// Start-up glue for `--tetgen-render`; `draw` is what the test reaches.
pub fn render_all(dir: &std::path::Path, sw: &str) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut lines = Vec::new();
    for kind in [GeomKind::Pipe, GeomKind::FuelChannel, GeomKind::Pebble] {
        let d = if sw == "graded" {
            draw_graded(kind, 2.0)?
        } else {
            draw_with(kind, 2.0, sw)?
        };
        for (name, img) in &d.images {
            img.write_png(dir.join(name)).map_err(|e| e.to_string())?;
        }
        lines.push(format!(
            "{}: {} tets, {} nodes, region attributes {:?}, cut cells z/x {:?}",
            kind.slug(),
            d.counts.0,
            d.counts.1,
            d.counts.2,
            d.cut_cells
        ));
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The refined mesh (stage 6, gh:#1795) is drawn too: more cells than
    /// the `-p` mesh, and the switches in the title.
    #[test]
    fn refined_sections_are_drawn() {
        let p = draw(GeomKind::Pipe, 2.0).unwrap();
        let q = draw_with(GeomKind::Pipe, 2.0, "pq1.414a0.0026zA").unwrap();
        assert!(
            q.counts.0 > 10 * p.counts.0,
            "{} vs {}",
            q.counts.0,
            p.counts.0
        );
        assert!(q.cut_cells.iter().zip(&p.cut_cells).all(|(a, b)| a > b));
    }

    /// The wall-graded mesh (gh:#1825) is drawn: three images (the zoom at
    /// the walls among them) from more tetrahedra than the ungraded mesh
    /// at the same volume bound. That the cells shrink toward the walls is
    /// the cfMesh crate's property test
    /// (`tetgen_tets_shrink_toward_the_walls_when_graded`) and the images.
    #[test]
    fn graded_sections_are_drawn_with_a_wall_zoom() {
        let g = draw_graded(GeomKind::Pipe, 2.0).unwrap();
        let u = draw_with(GeomKind::Pipe, 2.0, "pq1.414a0.00184zA").unwrap();
        assert_eq!(g.images.len(), 3);
        assert!(g
            .images
            .iter()
            .any(|(n, _)| n == "pipe_graded_zoom_cut.png"));
        assert!(g.counts.0 > u.counts.0, "{} vs {}", g.counts.0, u.counts.0);
    }

    /// The images are drawn from the TetGen mesh: the fuel channel's pin
    /// is a hole (one region, and the z-cut has no cell near the axis), the
    /// pebble is meshed as a second region, and the cut cells cover the
    /// section (cut area equal to the annulus area of the polygonal rings).
    #[test]
    fn tetgen_sections_show_the_regions_and_the_hole() {
        let fc = draw(GeomKind::FuelChannel, 2.0).unwrap();
        assert_eq!(fc.counts.2.len(), 1, "the pin is a hole, not a region");
        let pb = draw(GeomKind::Pebble, 2.0).unwrap();
        assert_eq!(pb.counts.2.len(), 2, "pebble and channel are two regions");
        assert!(fc.cut_cells.iter().all(|&n| n > 0));
        assert!(fc.images.iter().all(|(_, i)| !i.to_png_bytes().is_empty()));
        // The z-cut's polygons cover the 32-gon annulus (radii 1 and 0.5).
        let (points, tris, labels, _) = reactor_surface(GeomKind::FuelChannel, 2.0);
        let regions = SurfaceRegions::from_labels(&labels);
        let pts: Vec<[f64; 3]> = points.iter().map(|p| [p.x, p.y, p.z]).collect();
        let markers: Vec<i32> = regions.region_of_tri.iter().map(|&r| r as i32).collect();
        let plc = PlcInput::from_triangle_soup(&pts, &tris, &markers);
        let r = tetrahedralize_plc(&plc, &Switches::parse("pz").unwrap().behavior()).unwrap();
        let vm = to_volume_mesh(r.output.as_ref().unwrap(), Some(&regions.names));
        let poly = to_poly(&vm).unwrap();
        let area: f64 = cut(&poly, 2, 2.0)
            .iter()
            .map(|s| {
                let p = &s.polygon;
                0.5 * (0..p.len())
                    .map(|k| p[k][0] * p[(k + 1) % p.len()][1] - p[(k + 1) % p.len()][0] * p[k][1])
                    .sum::<f64>()
                    .abs()
            })
            .sum();
        let ngon = |r: f64| 0.5 * 32.0 * r * r * (2.0 * std::f64::consts::PI / 32.0).sin();
        let expect = ngon(1.0) - ngon(0.5);
        assert!(
            (area - expect).abs() < 1e-9 * expect,
            "cut area {area} != {expect}"
        );
    }
}
