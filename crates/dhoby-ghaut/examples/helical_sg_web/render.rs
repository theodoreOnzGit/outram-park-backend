//! The drawing rule's review image for the helical coil (gh:#1157): slices of
//! the ASSEMBLED geometry, every pixel classified by
//! `HelicalCoilGeometry::region_at` (distance to the helix the march
//! integrates along), never by drawing the named constants.
//!
//! ```text
//! cargo run -p dhoby-ghaut --example helical_sg_web --release -- --render-geometry [out.png]
//! ```
//!
//! Panels: (A) the whole coil, x–z slice through the axis (y = 0); (B) the
//! bottom turns of (A), zoomed, with the pitch marked between two
//! centreline crossings; (C) the x–y slice at mid-height, where the plane
//! cuts the near-horizontal tube along an arc; (D) the derived dimensions.
//! Native only (it writes a PNG with `outram_blender`'s plot image).

use crate::engine::Params;
use outram_blender::csg::plot::annotate::draw_text;
use outram_blender::csg::plot::{ImageData, Rgb};
use tampines::components::helical_coil_otsg::{CoilRegion, HelicalCoilGeometry};
use uom::si::length::meter;
use uom::si::ratio::ratio;

pub const BORE: Rgb = Rgb::new(70, 140, 230);
pub const WALL: Rgb = Rgb::new(170, 170, 170);
pub const SHELL: Rgb = Rgb::new(40, 30, 30);
const BG: Rgb = Rgb::new(250, 250, 250);
const INK: Rgb = Rgb::new(10, 10, 10);
const MARK: Rgb = Rgb::new(220, 40, 40);

fn colour(r: CoilRegion) -> Rgb {
    match r {
        CoilRegion::Bore => BORE,
        CoilRegion::Wall => WALL,
        CoilRegion::Shell => SHELL,
    }
}

/// A pixel rectangle `[x0, y0, w, h]` showing world `[u0, u1] × [v0, v1]`
/// \[m\]; `slice` maps a world pair `(u, v)` to 3-D.
struct Panel {
    px: [usize; 4],
    u: [f64; 2],
    v: [f64; 2],
    slice: Slice,
}

/// The cutting plane of a panel (enum dispatch, not a closure: the
/// workspace's no-trait-object rule).
#[derive(Clone, Copy)]
enum Slice {
    /// x–z plane at y = 0: `(u, v) = (x, z)`.
    XzAtY0,
    /// x–y plane at height `z` \[m\]: `(u, v) = (x, y)`.
    XyAtZ(f64),
}

impl Slice {
    fn point(self, u: f64, v: f64) -> [f64; 3] {
        match self {
            Slice::XzAtY0 => [u, 0.0, v],
            Slice::XyAtZ(z) => [u, v, z],
        }
    }
}

/// Fill a panel by region lookup; returns the pixel count of each region
/// (bore, wall, shell) so the test can check the drawing.
fn fill(img: &mut ImageData, g: &HelicalCoilGeometry, p: &Panel) -> [usize; 3] {
    let [x0, y0, w, h] = p.px;
    let mut count = [0usize; 3];
    for j in 0..h {
        let v = p.v[1] - (j as f64 + 0.5) / h as f64 * (p.v[1] - p.v[0]);
        for i in 0..w {
            let u = p.u[0] + (i as f64 + 0.5) / w as f64 * (p.u[1] - p.u[0]);
            let r = g.region_at(p.slice.point(u, v));
            count[match r {
                CoilRegion::Bore => 0,
                CoilRegion::Wall => 1,
                CoilRegion::Shell => 2,
            }] += 1;
            img.set(x0 + i, y0 + j, colour(r));
        }
    }
    for i in 0..w {
        img.set(x0 + i, y0, INK);
        img.set(x0 + i, y0 + h - 1, INK);
    }
    for j in 0..h {
        img.set(x0, y0 + j, INK);
        img.set(x0 + w - 1, y0 + j, INK);
    }
    count
}

fn to_px(p: &Panel, u: f64, v: f64) -> (i64, i64) {
    let [x0, y0, w, h] = p.px;
    (
        x0 as i64 + ((u - p.u[0]) / (p.u[1] - p.u[0]) * w as f64) as i64,
        y0 as i64 + ((p.v[1] - v) / (p.v[1] - p.v[0]) * h as f64) as i64,
    )
}

fn vline(img: &mut ImageData, x: i64, y0: i64, y1: i64, c: Rgb) {
    for y in y0.min(y1)..=y0.max(y1) {
        if x >= 0 && y >= 0 && (x as usize) < img.width && (y as usize) < img.height {
            img.set(x as usize, y as usize, c);
        }
    }
}

/// Render the review image for `params`' geometry. Returns the image and
/// the region pixel counts of the zoomed axial panel (B).
pub fn render(params: &Params) -> (ImageData, [usize; 3]) {
    let g = params.geometry();
    let (w, h) = (1400usize, 1000usize);
    let mut img = ImageData::filled(w, h, BG);
    let r = 0.5 * params.coil_diameter_m;
    let d_o = g.tube_outer_diameter.get::<meter>();
    let height = g.coil_height().get::<meter>();
    // (A) whole coil, equal scales on both axes.
    let z0 = -0.05 * height - d_o;
    let z1 = height * 1.05 + d_o;
    let scale_a = 880.0 / (z1 - z0);
    let half_w = 100.0 / scale_a;
    let pa = Panel { px: [40, 70, 200, 880], u: [-half_w, half_w], v: [z0, z1], slice: Slice::XzAtY0 };
    fill(&mut img, &g, &pa);
    draw_text(&mut img, 40, 26, "A: WHOLE COIL", INK);
    draw_text(&mut img, 40, 48, "X-Z AT Y=0", INK);
    // (B) the bottom four turns on the +x side, equal scales.
    let span = (4.0 * params.pitch_m).max(3.0 * d_o);
    let pb = Panel {
        px: [300, 70, 500, 500],
        u: [r - 0.5 * span, r + 0.5 * span],
        v: [-0.25 * span, 0.75 * span],
        slice: Slice::XzAtY0,
    };
    let counts = fill(&mut img, &g, &pb);
    draw_text(&mut img, 300, 40, "B: BOTTOM TURNS, +X SIDE, ZOOMED", INK);
    // Pitch marker between the centreline crossings at phi = 2 pi and 4 pi.
    let c1 = g.centreline_point(2.0 * std::f64::consts::PI);
    let c2 = g.centreline_point(4.0 * std::f64::consts::PI);
    let (x1, y1) = to_px(&pb, c1[0], c1[2]);
    let (_, y2) = to_px(&pb, c2[0], c2[2]);
    vline(&mut img, x1, y1, y2, MARK);
    draw_text(&mut img, x1 + 6, (y1 + y2) / 2, &format!("PITCH {:.1} MM", (c2[2] - c1[2]) * 1e3), MARK);
    // (C) x-y slice at mid-height.
    let zm = 0.5 * height;
    let half_c = r + 2.0 * d_o;
    let pc = Panel { px: [860, 70, 500, 500], u: [-half_c, half_c], v: [-half_c, half_c], slice: Slice::XyAtZ(zm) };
    fill(&mut img, &g, &pc);
    draw_text(&mut img, 860, 40, &format!("C: X-Y AT Z = {:.3} M (MID-HEIGHT)", zm), INK);
    // (D) legend and derived dimensions, read from the geometry methods.
    let lines = [
        "LEGEND: BLUE = BORE (WATER), GREY = TUBE WALL, DARK = SHELL (HOT SIDE)".to_string(),
        "EACH PIXEL: HelicalCoilGeometry::region_at (DISTANCE TO THE HELIX)".to_uppercase(),
        format!("HELIX DIAMETER D_W {:.1} MM, PITCH {:.1} MM, TURNS {:.2}", params.coil_diameter_m * 1e3, params.pitch_m * 1e3, g.turns),
        format!("TUBE D_I {:.2} MM, D_O {:.2} MM, {} PARALLEL TUBES", g.tube_inner_diameter.get::<meter>() * 1e3, d_o * 1e3, g.parallel_tubes),
        format!("DERIVED: TUBE LENGTH {:.3} M, COIL HEIGHT {:.3} M", g.tube_length().get::<meter>(), height),
        format!("DERIVED: CURVATURE DIAMETER D_C {:.2} MM, D_I/D_C {:.4}, SIN(HELIX ANGLE) {:.4}", g.curvature_diameter().get::<meter>() * 1e3, g.curvature_ratio().get::<ratio>(), g.elevation_sine()),
        format!("PANEL B PIXELS: BORE {}, WALL {}, BORE/WALL {:.3} (CIRCLES: {:.3})", counts[0], counts[1], counts[0] as f64 / counts[1].max(1) as f64, {
            let di = g.tube_inner_diameter.get::<meter>();
            di * di / (d_o * d_o - di * di)
        }),
    ];
    for (k, l) in lines.iter().enumerate() {
        draw_text(&mut img, 300, 640 + 30 * k as i64, l, INK);
    }
    (img, counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The image shows the assembled coil: in the zoomed axial slice the bore
    /// and wall pixels stand in the ratio of the two circles' areas (the
    /// helix angle is small, so a y = 0 cut of the tube is nearly circular),
    /// and the default coil fills about 2/3 of panel B's width with shell.
    #[test]
    fn the_review_image_is_drawn_from_the_assembled_geometry() {
        let p = Params::default();
        let (img, counts) = render(&p);
        assert_eq!((img.width, img.height), (1400, 1000));
        let g = p.geometry();
        let di = g.tube_inner_diameter.get::<meter>();
        let d_o = g.tube_outer_diameter.get::<meter>();
        let expect = di * di / (d_o * d_o - di * di);
        let got = counts[0] as f64 / counts[1] as f64;
        assert!((got / expect - 1.0).abs() < 0.08, "bore/wall {got} against {expect}");
        assert!(counts[2] > counts[0] + counts[1], "mostly shell between turns");
        assert!(!img.to_png_bytes().is_empty());
    }
}
