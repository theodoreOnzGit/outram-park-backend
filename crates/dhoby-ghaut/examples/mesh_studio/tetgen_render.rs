//! The drawing rule's review images for the TetGen `-p` route (gh:#1794):
//! z- and x-sections of the **TetGen tetrahedral mesh** of Mesh Studio's
//! pipe, fuel channel and pebble surfaces (size 2), drawn from the
//! assembled mesh, never from the named dimensions.
//!
//! ```text
//! cargo run -p dhoby-ghaut --example mesh_studio --release -- --tetgen-render <dir>
//! ```
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
//! Native only (it writes PNGs with `outram_blender`'s plot image).

use super::model::{reactor_surface, GeomKind};
use super::section::{cut, to_poly};
use outram_blender::csg::plot::{annotate_image, ImageData, LegendEntry, Rgb};
use outram_park_fork_cfmesh::patches::SurfaceRegions;
use outram_park_fork_cfmesh::tetgen::{tetrahedralize_plc, to_volume_mesh, PlcInput, Switches};

const BG: Rgb = Rgb::new(255, 255, 255);
const EDGE: Rgb = Rgb::new(30, 30, 30);
/// Region fill colours, by attribute order (1st, 2nd, ... region found).
const REGION: [Rgb; 4] = [
    Rgb::new(150, 200, 240),
    Rgb::new(250, 190, 120),
    Rgb::new(170, 230, 160),
    Rgb::new(230, 160, 210),
];

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
    let (points, tris, labels, _walls) = reactor_surface(kind, size);
    let regions = SurfaceRegions::from_labels(&labels);
    let pts: Vec<[f64; 3]> = points.iter().map(|p| [p.x, p.y, p.z]).collect();
    let markers: Vec<i32> = regions.region_of_tri.iter().map(|&r| r as i32).collect();
    let plc = PlcInput::from_triangle_soup(&pts, &tris, &markers);
    let b = Switches::parse("pzA")?.behavior();
    let r = tetrahedralize_plc(&plc, &b).map_err(|e| format!("tetgen: {e:?}"))?;
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
    let colour_of = |c: usize| {
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
    for (axis, offset, tag, what) in [
        (2u8, zmid, "z", format!("Z = {zmid:.3} M (MID-HEIGHT)")),
        (
            0u8,
            xoff,
            "x",
            format!("X = {xoff:.3} M (THROUGH THE AXIS)"),
        ),
    ] {
        let sections = cut(&poly, axis, offset);
        cut_cells.push(sections.len());
        // In-plane extent: (x, y) for a z-cut, (y, z) for an x-cut.
        let (u, v) = if axis == 2 {
            ([lo[0], hi[0]], [lo[1], hi[1]])
        } else {
            ([lo[1], hi[1]], [lo[2], hi[2]])
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
        let legend: Vec<LegendEntry> = distinct
            .iter()
            .enumerate()
            .map(|(k, a)| {
                let n = attrs.iter().filter(|&&x| x == *a).count();
                LegendEntry::new(
                    REGION[k % REGION.len()],
                    format!("REGION ATTRIBUTE {a} ({n} TETS)"),
                )
            })
            .collect();
        let title = format!(
            "TETGEN -PZA, {} SIZE {size}: {what}, {} CUT TETS OF {}, {} NODES",
            kind.slug().to_uppercase(),
            sections.len(),
            vm.n_cells,
            out.points.len()
        );
        images.push((
            format!("{}_{tag}_cut.png", kind.slug()),
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
pub fn render_all(dir: &std::path::Path) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut lines = Vec::new();
    for kind in [GeomKind::Pipe, GeomKind::FuelChannel, GeomKind::Pebble] {
        let d = draw(kind, 2.0)?;
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
