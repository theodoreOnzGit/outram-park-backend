// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! gh:#1465: draw the castellated mesh of a sphere in a box under both
//! refinement rules (`DistanceBand`, this port's historical rule, and
//! `SurfaceIntersection`, OpenFOAM's `surfaceOnlyRefine`), as the mid-plane
//! slice (z = 0.5), cells coloured by level, sphere outline in red. Prints the
//! per-level cell counts.
//!
//! `cargo run --release -p outram-foam-mesh --example draw_castellation_refinement -- OUT_DIR`
//!
//! Example binary: drawing code, exempt from the reached-by-a-test rule (it
//! calls only the tested `castellate` API).

use image::{Rgb, RgbImage};
use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_mesh::snappy_hex_mesh::{
    castellate, BackgroundMesh, Bounds, CastellationControls, TriangleSoup,
};

fn main() {
    let out = std::env::args().nth(1).expect("OUT_DIR");
    std::fs::create_dir_all(&out).unwrap();
    let centre = Vector3::new(0.5, 0.5, 0.5);
    let surface = TriangleSoup::uv_sphere(centre, 0.3, 32, 32);
    let domain = Bounds::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 1.0, 1.0));
    for (name, upstream) in [("distance_band", false), ("surface_intersection", true)] {
        let bg = BackgroundMesh::uniform(domain, 8, 8, 8);
        let mut c = CastellationControls::new(bg, 2, Vector3::new(0.02, 0.02, 0.02));
        if upstream {
            c = c.with_surface_intersection_refinement();
        }
        let m = castellate(&surface, &c).unwrap();
        println!("{name}: {} cells, by level {:?}", m.n_cells(), m.cells_by_level);
        let px = 800u32;
        let mut img = RgbImage::from_pixel(px, px, Rgb([255, 255, 255]));
        let cols = [Rgb([200, 220, 255]), Rgb([120, 190, 120]), Rgb([240, 190, 90])];
        let to = |v: f64| (v * px as f64) as i64;
        for (cc, vol) in m.fv_mesh.cell_centres.iter().zip(&m.fv_mesh.cell_volumes) {
            let s = vol.cbrt();
            if (cc.z - 0.5).abs() > 0.5 * s {
                continue;
            }
            let lvl = ((0.125 / s).log2().round() as usize).min(2);
            let (x0, x1) = (to(cc.x - 0.5 * s), to(cc.x + 0.5 * s));
            let (y0, y1) = (to(cc.y - 0.5 * s), to(cc.y + 0.5 * s));
            for x in x0..x1 {
                for y in y0..y1 {
                    let edge = x == x0 || y == y0;
                    let col = if edge { Rgb([0, 0, 0]) } else { cols[lvl] };
                    img.put_pixel(x as u32, (px as i64 - 1 - y) as u32, col);
                }
            }
        }
        for a in 0..4000 {
            let t = a as f64 / 4000.0 * std::f64::consts::TAU;
            let (x, y) = (to(0.5 + 0.3 * t.cos()), to(0.5 + 0.3 * t.sin()));
            img.put_pixel(x as u32, (px as i64 - 1 - y) as u32, Rgb([220, 0, 0]));
        }
        img.save(format!("{out}/castellation_{name}.png")).unwrap();
    }
}
