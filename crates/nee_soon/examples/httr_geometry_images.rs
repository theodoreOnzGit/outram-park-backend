//! **Draw the HTTR start-up core as the solver sees it** (HARD RULE,
//! `crates/nee_soon/CLAUDE.md`).
//!
//! Every image is a material slice of the ASSEMBLED geometry
//! (`nee_soon::httr::assemble_httr`), rendered by the ported OpenMC plotter,
//! never a drawing of the constants. Written to
//! `crates/nee_soon/verification_and_validation/httr_startup_core/images/`
//! (or `OUTRAM_HTTR_IMG_OUT`). That folder is gitignored: the images are
//! posted on GitHub issue #888, not committed.
//!
//! ```bash
//! cargo run --release -p nee_soon --example httr_geometry_images
//! ```
//!
//! The set, outermost first: the whole core radially (fuel layer 1, the top
//! reflector, the bottom reflector) and axially (through the centre rod, and
//! through a fuel column and an R3 rod); one fuel column and its neighbours;
//! a rotated column beside an unrotated one; a fuel block with its BP pins
//! (X-Y and X-Z); one fuel rod; the compact's TRISO layer; a control column
//! with its rods (X-Y and X-Z); the subcritical rod positions.
//!
//! GUI-free and data-free: `main` is drawing glue (no test reaches it,
//! per the workspace rule's exception for main/CLI glue); the geometry it
//! draws is tested by `nee_soon::httr::tests`.

use std::path::PathBuf;

use nee_soon::httr::geometry::{assemble_httr, CompactTracking};
use nee_soon::httr::materials::mat;
use nee_soon::httr::spec::{self, core_map, RodPositions};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;

/// One colour and name per material, in `httr::materials::mat` order.
fn palette() -> Vec<(Rgb, &'static str)> {
    let kernel = |k: u8| Rgb::new(255, 200 - 12 * k, 40);
    let names = [
        "kernel 3.4%", "kernel 3.9%", "kernel 4.3%", "kernel 4.8%", "kernel 5.2%",
        "kernel 5.9%", "kernel 6.3%", "kernel 6.7%", "kernel 7.2%", "kernel 7.9%",
        "kernel 9.4%", "kernel 9.9%",
    ];
    let mut p: Vec<(Rgb, &'static str)> = (0..12u8).map(|k| (kernel(k), names[k as usize])).collect();
    p.extend([
        (Rgb::new(120, 120, 200), "buffer"),
        (Rgb::new(70, 70, 70), "IPyC"),
        (Rgb::new(230, 230, 60), "SiC"),
        (Rgb::new(40, 40, 40), "OPyC"),
        (Rgb::new(150, 110, 80), "overcoat"),
        (Rgb::new(175, 135, 95), "compact matrix"),
        (Rgb::new(90, 140, 90), "IG-110 sleeve"),
        (Rgb::new(200, 40, 200), "BP 2.0 wt% B"),
        (Rgb::new(120, 0, 160), "BP 2.5 wt% B"),
        (Rgb::new(140, 170, 140), "graphite disks"),
        (Rgb::new(220, 20, 20), "B4C/C absorber"),
        (Rgb::new(160, 200, 230), "Alloy 800H"),
        (Rgb::new(110, 110, 110), "IG-110 block"),
        (Rgb::new(170, 170, 150), "PGX reflector"),
        (Rgb::new(255, 255, 255), "helium"),
    ]);
    assert_eq!(p.len(), mat::COUNT);
    p
}

fn centre(name: &str) -> (f64, f64) {
    core_map()
        .into_iter()
        .find(|c| c.name == name)
        .map(|c| c.centre())
        .unwrap_or_else(|| panic!("no column {name}"))
}

fn main() {
    let out = std::env::var("OUTRAM_HTTR_IMG_OUT").map_or_else(
        |_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("verification_and_validation/httr_startup_core/images")
        },
        PathBuf::from,
    );
    std::fs::create_dir_all(&out).expect("create output directory");
    let t0 = std::time::Instant::now();
    let core = assemble_httr(RodPositions::CRITICAL, CompactTracking::Delta);
    let sub = assemble_httr(RodPositions::SUBCRITICAL, CompactTracking::Delta);
    println!(
        "assembled: {} surfaces, {} cells, {} universes, {} lattices; {} fuel rods x {} \
         particles ({:.1} s)",
        core.geometry.surfaces.len(),
        core.geometry.cells.len(),
        core.geometry.universes.len(),
        core.geometry.lattices.len(),
        core.fuel_rods,
        core.particles_per_rod,
        t0.elapsed().as_secs_f64()
    );
    let pal = palette();
    let render = |g: &Geometry, name: &str, plot: SlicePlot, what: &str| {
        let t = std::time::Instant::now();
        let (_raw, img) = render_material_slice(g, &plot, &pal, &format!("HTTR: {what}"));
        let path = out.join(name);
        img.write_png(&path).expect("write png");
        println!(
            "wrote {} ({}x{} px, {:.1} s)",
            path.display(),
            plot.pixels[0],
            plot.pixels[1],
            t.elapsed().as_secs_f64()
        );
    };
    let g = &core.geometry;
    let px = |w: f64, cm: f64| (w / cm).round() as usize;
    let xy = |x: f64, y: f64, z: f64, w: f64, cm: f64| {
        SlicePlot::new(PlotBasis::Xy, Position::new(x, y, z), [w, w], [px(w, cm), px(w, cm)])
    };
    let xz = |x: f64, y: f64, z: f64, w: f64, h: f64, cm: f64| {
        SlicePlot::new(PlotBasis::Xz, Position::new(x, y, z), [w, h], [px(w, cm), px(h, cm)])
    };

    // Whole core.
    let w = 2.0 * spec::PERMANENT_REFLECTOR_R + 10.0;
    render(g, "httr_xy_fuel_layer1.png", xy(0.0, 0.0, 116.0, w, 0.25), "X-Y, FUEL LAYER 1 (Z = 116 CM)");
    render(g, "httr_xy_layer5.png", xy(0.0, 0.0, -116.0, w, 0.25), "X-Y, FUEL LAYER 5 (Z = -116 CM)");
    render(g, "httr_xy_top_reflector.png", xy(0.0, 0.0, 200.0, w, 0.25), "X-Y, TOP REFLECTOR (Z = 200 CM)");
    render(g, "httr_xy_bottom_reflector.png", xy(0.0, 0.0, -200.0, w, 0.25), "X-Y, BOTTOM REFLECTOR (Z = -200 CM)");
    let h = 2.0 * spec::CORE_HALF_HEIGHT + 10.0;
    render(g, "httr_xz_y0.png", xz(0.0, 0.0, 0.0, w, h, 0.3), "X-Z AT Y = 0 (CENTRE ROD, C01/C07 R1 RODS)");
    // Through E01 (an R3 column, rod near the top) and the fuel beside it.
    let (ex, ey) = centre("E01");
    render(g, "httr_xz_through_e01.png", xz(0.0, ey, 0.0, w, h, 0.3), "X-Z THROUGH E01 (R3 ROD AT 4049 MM)");
    let _ = ex;

    // One fuel column with its neighbours, and a rotated beside an unrotated
    // column.
    let (bx, by) = centre("B04");
    render(g, "httr_xy_b04_and_neighbours.png", xy(bx, by, 116.0, 110.0, 0.05), "X-Y, B04 (UNROTATED) AND NEIGHBOURS, LAYER 1");
    let (b1x, b1y) = centre("B01");
    render(g, "httr_xy_b01_rotated_180.png", xy(b1x, b1y, 116.0, 40.0, 0.02), "X-Y, B01 (ROTATED 180 DEG), LAYER 1");
    render(g, "httr_xy_b04_unrotated.png", xy(bx, by, 116.0, 40.0, 0.02), "X-Y, B04 (UNROTATED), LAYER 1");
    let (dx, dy) = centre("D07");
    render(g, "httr_xy_d07_31_holes.png", xy(dx, dy, 116.0, 40.0, 0.02), "X-Y, D07 (ZONE 4, 31 HOLES), LAYER 1");
    // A block axially, through its empty-BP corner and a fuel row.
    render(g, "httr_xz_b04_block.png", xz(bx, by, 116.0, 40.0, 70.0, 0.04), "X-Z THROUGH B04, LAYER 1 BLOCK (Y = CENTRE)");
    let s = spec::PIN_PITCH;
    let bp_y = by - 3.0 * s * 3.0_f64.sqrt() / 2.0;
    render(g, "httr_xz_b04_bp_row.png", xz(bx, bp_y, 116.0, 40.0, 70.0, 0.04), "X-Z THROUGH B04'S BP PIN ROW");

    // One fuel rod, its compact and the TRISO layer.
    let (fx, fy) = (bx + s, by);
    render(g, "httr_xy_fuel_rod.png", xy(fx, fy, 116.05, 5.0, 0.004), "X-Y, ONE FUEL ROD (B04, LAYER 1)");
    render(g, "httr_xz_fuel_rod_end.png", xz(fx, fy, 116.0 + 26.0, 5.0, 6.0, 0.005), "X-Z, TOP OF A FUEL ROD'S COMPACT STACK");
    render(g, "httr_xy_triso.png", xy(fx - 0.9, fy, 116.05, 0.6, 0.0006), "X-Y, TRISO PARTICLES IN THE COMPACT");

    // A control column.
    render(g, "httr_xy_centre_column_rods.png", xy(0.0, 0.0, 100.0, 40.0, 0.02), "X-Y, CENTRE COLUMN A01 AT Z = 100 CM (TWO RODS, RSS HOLE)");
    let zb = spec::FUEL_BOTTOM_Z + RodPositions::CRITICAL.c;
    render(g, "httr_xz_centre_rod_bottom.png", xz(0.0, 0.0, zb, 40.0, 80.0, 0.04), "X-Z, A01 ROD BOTTOM AT 1775 MM ABOVE THE FUEL BOTTOM");
    let zs = spec::FUEL_BOTTOM_Z + RodPositions::SUBCRITICAL.c;
    render(&sub.geometry, "httr_xz_subcritical_rods.png", xz(0.0, 0.0, 0.0, w, h, 0.3), "SUBCRITICAL: ALL RODS AT -55 MM, X-Z AT Y = 0");
    render(&sub.geometry, "httr_xz_subcritical_rod_bottom.png", xz(0.0, 0.0, zs, 40.0, 80.0, 0.04), "SUBCRITICAL: A01 ROD BOTTOM AT -55 MM");
    println!("done in {:.1} s", t0.elapsed().as_secs_f64());
}
