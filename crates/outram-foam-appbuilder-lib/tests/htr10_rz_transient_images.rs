// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Drawings of the transient HTR-10 R-Z core (gh:#1151 part B) as the
//! solver assembles it on the 20 cm transient mesh, and of the in-vessel
//! natural circulation after the loop stops (the crate's "geometry is
//! DRAWN" HARD RULE). Thermal-hydraulics only: the power is a fixed input
//! (3315 kW for the as-run steady state, then 150 kW, about the decay heat
//! 10 min after the LOFC trip, with the loop stopped).
//!
//! Every pixel is coloured from the ASSEMBLED mesh (`RzMesh::cell_at` at the
//! pixel centre) and the ASSEMBLED gas network (`GasNetwork::cell_flow`).
//! The drawing helpers are copied from `tests/htr10_rz_flow_fpic.rs`
//! (integration-test files cannot share code) with the model type changed.
//! Written into `crates/nee_soon/verification_and_validation/htr10_rz_transients/`,
//! the record of the coupled runs that use this mesh.

use outram_blender::csg::plot::annotate::draw_text;
use outram_blender::csg::plot::colour::{Rgb, BLACK, WHITE};
use outram_blender::csg::plot::image::ImageData;
use outram_blender::csg::plot::{annotate_image, LegendEntry};
use outram_foam_appbuilder_lib::htr10_rz::{FlowKind, Htr10RzTransientCore, OperatingPoint, TransientCoreOptions};
use uom::si::f64::{Length, MassRate, Power, Pressure, ThermodynamicTemperature, Time};
use uom::si::length::centimeter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

// ---------------------------------------------------------------- drawing --

struct Window {
    r: [f64; 2],
    z: [f64; 2],
    px_per_cm: f64,
}

fn raster(m: &Htr10RzTransientCore, w: &Window) -> (usize, usize, Vec<Option<usize>>) {
    let nx = ((w.r[1] - w.r[0]) * w.px_per_cm).round() as usize;
    let ny = ((w.z[1] - w.z[0]) * w.px_per_cm).round() as usize;
    let mesh = m.solid().field().mesh();
    let mut cells = Vec::with_capacity(nx * ny);
    for y in 0..ny {
        for x in 0..nx {
            let r = w.r[0] + (x as f64 + 0.5) / w.px_per_cm;
            let z = w.z[0] + (y as f64 + 0.5) / w.px_per_cm;
            cells.push(mesh.cell_at(Length::new::<centimeter>(r), Length::new::<centimeter>(z)));
        }
    }
    (nx, ny, cells)
}

fn outline(img: &mut ImageData, key: &[Option<u32>], colour: Rgb) {
    let (w, h) = (img.width, img.height);
    for y in 0..h {
        for x in 0..w {
            let here = key[y * w + x];
            if (x + 1 < w && key[y * w + x + 1] != here) || (y + 1 < h && key[(y + 1) * w + x] != here) {
                img.set(x, y, colour);
            }
        }
    }
}

fn frame(img: &ImageData, w: &Window, tick_cm: f64, title: &str, legend: &[LegendEntry]) -> ImageData {
    let (left, bottom, top) = (60usize, 50usize, 10usize);
    let mut out = ImageData::filled(img.width + left + 30, img.height + bottom + top, WHITE);
    for y in 0..img.height {
        for x in 0..img.width {
            out.set(left + x, top + y, img.get(x, y));
        }
    }
    let mut v = (w.z[0] / tick_cm).ceil() * tick_cm;
    while v <= w.z[1] + 1e-9 {
        let y = top + ((v - w.z[0]) * w.px_per_cm) as usize;
        for x in left - 8..left {
            out.set(x, y.min(out.height - 1), BLACK);
        }
        draw_text(&mut out, 2, y as i64 - 7, &format!("{v:.0}"), BLACK);
        v += tick_cm;
    }
    let mut v = (w.r[0] / tick_cm).ceil() * tick_cm;
    while v <= w.r[1] + 1e-9 {
        let x = left + (((v - w.r[0]) * w.px_per_cm) as usize).min(img.width - 1);
        for y in top + img.height..top + img.height + 8 {
            out.set(x, y, BLACK);
        }
        draw_text(&mut out, x as i64 - 12, (top + img.height + 12) as i64, &format!("{v:.0}"), BLACK);
        v += tick_cm;
    }
    draw_text(&mut out, (left + img.width / 2) as i64 - 30, (top + img.height + 32) as i64, "R [CM]", BLACK);
    let mut titled = annotate_image(&out, title, legend);
    draw_text(&mut titled, 4, 22, "Z [CM] DOWN", BLACK);
    titled
}

fn flow_colour(tab25: u8) -> Rgb {
    const P: [Rgb; 20] = [
        Rgb::new(200, 200, 200), // 0 unused
        Rgb::new(230, 100, 40),  // 1 bed
        Rgb::new(170, 220, 120), // 2 upper bottom refl
        Rgb::new(110, 170, 70),  // 3 lower bottom refl
        Rgb::new(255, 160, 60),  // 4 hot plenum
        Rgb::new(255, 210, 160), // 5 entry plenum
        Rgb::new(200, 200, 200), // 6 non-flowing
        Rgb::new(240, 240, 170), // 7 bottom cavity
        Rgb::new(255, 230, 100), // 8 bottom annular
        Rgb::new(250, 190, 70),  // 9 rounding
        Rgb::new(90, 170, 230),  // 10 risers
        Rgb::new(90, 90, 40),    // 11 tube throttle
        Rgb::new(20, 60, 120),   // 12 CR exit throttle
        Rgb::new(30, 100, 200),  // 13 CR channels
        Rgb::new(255, 255, 0),   // 14 top plenum
        Rgb::new(220, 150, 230), // 15 small plenum
        Rgb::new(180, 60, 200),  // 16 entry
        Rgb::new(130, 40, 160),  // 17 annulus
        Rgb::new(200, 230, 60),  // 18 top flow zone
        Rgb::new(230, 30, 60),   // 19 leak gaps
    ];
    P[tab25 as usize]
}

const FLOW_NAMES: [&str; 20] = [
    "",
    "F1 PEBBLE BED + TUBE (KTA)",
    "F2 UPPER BOT REFL CHANNELS",
    "F3 LOWER BOT REFL CHANNELS",
    "F4 HOT PLENUM (OUTLET R 90)",
    "F5 CORE ENTRY PLENUM",
    "NON-FLOWING (SOLID ONLY)",
    "F7 RPV BOTTOM CAVITY (CHAIN)",
    "F8 BOTTOM ANNULAR (CHAIN)",
    "F9 ROUNDING PATH (CHAIN)",
    "F10 RISERS (CHAIN)",
    "F11 TUBE THROTTLE",
    "F12 CR EXIT THROTTLE",
    "F13 CR CHANNELS",
    "F14 TOP PLENUM (CHAIN)",
    "F15 SMALL PLENUM",
    "F16 HE ENTRY (CHAIN)",
    "F17 ANNULUS (CHAIN)",
    "F18 TOP REFL FLOW ZONE",
    "F19 LEAK GAPS",
];

fn flow_key(m: &Htr10RzTransientCore, c: usize) -> u8 {
    m.network().cell_flow(c).map_or(6, |(f, _)| f.tab25)
}

fn flow_region_image(m: &Htr10RzTransientCore, w: &Window, mesh_lines: bool, tick: f64, title: &str) -> ImageData {
    let (nx, ny, cells) = raster(m, w);
    let mut img = ImageData::filled(nx, ny, WHITE);
    let mut seen = [false; 20];
    let mut key = Vec::with_capacity(cells.len());
    for (p, cell) in cells.iter().enumerate() {
        if let Some(cell) = cell {
            let k = flow_key(m, *cell);
            seen[k as usize] = true;
            img.pixels[p] = flow_colour(k);
            // Channel regions hatched: every 6th pixel column darkened.
            if let Some((f, _)) = m.network().cell_flow(*cell) {
                if matches!(f.kind, FlowKind::AxialChannels { .. }) && (p % nx) % 6 == 0 {
                    img.pixels[p] = Rgb::new(40, 40, 40);
                }
            }
            key.push(Some(if mesh_lines { *cell as u32 } else { u32::from(k) }));
        } else {
            key.push(None);
        }
    }
    outline(&mut img, &key, BLACK);
    let legend: Vec<LegendEntry> = (1..20)
        .filter(|k| seen[*k])
        .map(|k| LegendEntry::new(flow_colour(k as u8), FLOW_NAMES[k]))
        .collect();
    frame(&img, w, tick, title, &legend)
}

fn ramp(t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let (r, g, b) = if t < 1.0 / 3.0 {
        (0.0, 3.0 * t, 1.0)
    } else if t < 2.0 / 3.0 {
        let s = 3.0 * t - 1.0;
        (s, 1.0, 1.0 - s)
    } else {
        let s = 3.0 * t - 2.0;
        (1.0, 1.0 - s, 0.0)
    };
    Rgb::new((255.0 * r) as u8, (255.0 * g) as u8, (255.0 * b) as u8)
}

/// A per-cell scalar (None = not drawn, grey), region outlines in black.
fn scalar_image(
    m: &Htr10RzTransientCore,
    w: &Window,
    values: &[Option<f64>],
    (lo, hi): (f64, f64),
    label: impl Fn(f64) -> String,
    title: &str,
    with_arrows: bool,
) -> ImageData {
    let (nx, ny, cells) = raster(m, w);
    let mut img = ImageData::filled(nx, ny, WHITE);
    let mut key = Vec::with_capacity(cells.len());
    for (p, cell) in cells.iter().enumerate() {
        if let Some(cell) = cell {
            img.pixels[p] = values[*cell].map_or(Rgb::new(215, 215, 215), |v| ramp((v - lo) / (hi - lo)));
            key.push(Some(m.solid().field().mesh().cell_region(*cell) as u32));
        } else {
            key.push(None);
        }
    }
    outline(&mut img, &key, BLACK);
    if with_arrows {
        arrows(&mut img, m, w);
    }
    let legend: Vec<LegendEntry> = (0..=5)
        .map(|i| {
            let s = f64::from(i) / 5.0;
            LegendEntry::new(ramp(s), label(lo + s * (hi - lo)))
        })
        .collect();
    frame(&img, w, 50.0, title, &legend)
}

/// Draw an arrow of the 2-D mass flux at the centre of every `step`-th gas
/// cell (length ~ log of |G|).
fn arrows(img: &mut ImageData, m: &Htr10RzTransientCore, w: &Window) {
    let g = m.gas().unwrap();
    let mesh = m.solid().field().mesh();
    let nr = mesh.r_edges().n_cells();
    for c in 0..mesh.n_cells() {
        let (i, j) = (c % nr, c / nr);
        if i % 2 != 0 || j % 2 != 0 {
            continue;
        }
        let Some((_, true)) = m.network().cell_flow(c) else { continue };
        let (gr, gz) = g.mass_flux[c];
        let mag = gr.hypot(gz);
        if mag < 1e-4 {
            continue;
        }
        let (r, z) = mesh.cell_centre(c);
        let (r, z) = (r.get::<centimeter>(), z.get::<centimeter>());
        if r < w.r[0] || r > w.r[1] || z < w.z[0] || z > w.z[1] {
            continue;
        }
        let len = 4.0 * (2.0 + mag.log10()).clamp(0.5, 3.0);
        let (x0, y0) = ((r - w.r[0]) * w.px_per_cm, (z - w.z[0]) * w.px_per_cm);
        for s in 0..=(len as usize * 2) {
            let t = s as f64 / 2.0;
            let x = x0 + t * gr / mag;
            let y = y0 + t * gz / mag;
            if x >= 0.0 && y >= 0.0 && (x as usize) < img.width && (y as usize) < img.height {
                img.set(x as usize, y as usize, BLACK);
            }
        }
        let (x, y) = (x0 + len * gr / mag, y0 + len * gz / mag);
        for (dx, dy) in [(0i64, 0i64), (1, 0), (0, 1), (-1, 0), (0, -1)] {
            let (xx, yy) = (x as i64 + dx, y as i64 + dy);
            if xx >= 0 && yy >= 0 && (xx as usize) < img.width && (yy as usize) < img.height {
                img.set(xx as usize, yy as usize, Rgb::new(255, 255, 255));
            }
        }
    }
}

// ------------------------------------------------------------------ cases --

/// The FPIC base case on the 5 cm mesh: outer-iteration convergence,

fn out_dir() -> std::path::PathBuf {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../nee_soon/verification_and_validation/htr10_rz_transients");
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn op(power_w: f64, flow: f64) -> OperatingPoint {
    OperatingPoint {
        power: Power::new::<watt>(power_w),
        inlet: ThermodynamicTemperature::new::<kelvin>(485.15),
        pressure: Pressure::new::<pascal>(2.476e6),
        mass_flow: MassRate::new::<kilogram_per_second>(flow),
    }
}

/// The transient mesh and its flow regions as assembled, the as-run steady
/// state, and the stopped-loop natural circulation at a fixed 150 kW.
/// Prints the cell counts and the circulation measures (recorded in the
/// part B README, Sec 2 and the drawing section).
#[test]
fn transient_core_drawings() {
    let dir = out_dir();
    let mut m = Htr10RzTransientCore::build(TransientCoreOptions::htr10_lofc()).unwrap();
    let mesh = m.solid().field().mesh();
    println!(
        "transient mesh: {} x {} = {} cells; {} 2-D gas cells, {} chain nodes; {} heated cells",
        mesh.r_edges().n_cells(),
        mesh.z_edges().n_cells(),
        mesh.n_cells(),
        m.network().n_gas_cells(),
        m.network().n_chain_nodes(),
        m.heated_cells().len()
    );
    let whole = Window { r: [0.0, 218.0], z: [-351.0, 665.0], px_per_cm: 1.0 };
    flow_region_image(&m, &whole, true, 50.0, "HTR-10 TRANSIENT R-Z CORE, 20 CM MESH: FLOW REGIONS + CELLS AS ASSEMBLED")
        .write_png(dir.join("rz_transient_mesh_flow_regions.png"))
        .unwrap();
    let r = m.solve_steady(&op(3.315e6, 1.457)).unwrap();
    println!(
        "as-run steady: outlet {:.2} C, RCCS+heads {:.1} kW",
        r.outlet_temperature.get::<kelvin>() - 273.15,
        r.heat_to_rccs_and_heads.get::<watt>() / 1e3
    );
    let n = m.solid().field().mesh().n_cells();
    let ts = |m: &Htr10RzTransientCore| -> Vec<Option<f64>> {
        (0..n).map(|c| Some(m.solid().field().temperature(c).get::<kelvin>())).collect()
    };
    let core = Window { r: [0.0, 218.0], z: [-200.0, 520.0], px_per_cm: 2.0 };
    let t0 = ts(&m);
    scalar_image(&m, &core, &t0, (450.0, 1000.0), |v| format!("{:.0} C", v - 273.15), "SOLID T, AS-RUN STEADY STATE (3315 KW, 1.457 KG/S)", false)
        .write_png(dir.join("rz_transient_solid_t_steady.png"))
        .unwrap();
    let stopped = op(1.5e5, 0.0);
    let mut last = None;
    for _ in 0..120 {
        last = Some(m.step(Time::new::<second>(5.0), &stopped).unwrap());
    }
    let c = last.unwrap().circulation;
    println!(
        "600 s stopped at 150 kW: bed upflow {:.4} kg/s, downflow {:.4} kg/s, max |G| {:.4} kg/(m2 s), enthalpy up through bed top {:.1} kW",
        c.bed_upflow.get::<kilogram_per_second>(),
        c.bed_downflow.get::<kilogram_per_second>(),
        c.max_bed_mass_flux,
        c.enthalpy_up_through_bed_top.get::<watt>() / 1e3
    );
    let g = m.gas().unwrap();
    let gmag: Vec<Option<f64>> = (0..n)
        .map(|c| {
            let (a, b) = g.mass_flux[c];
            g.temperature[c].is_finite().then(|| a.hypot(b).max(1e-5).log10())
        })
        .collect();
    scalar_image(&m, &core, &gmag, (-5.0, -1.0), |v| format!("G {:.0e} KG/M2S", 10f64.powf(v)), "NATURAL CIRCULATION, LOOP STOPPED, 600 S AT 150 KW: |G| (LOG) + DIRECTION", true)
        .write_png(dir.join("rz_transient_natural_circulation.png"))
        .unwrap();
    let t1 = ts(&m);
    scalar_image(&m, &core, &t1, (450.0, 1000.0), |v| format!("{:.0} C", v - 273.15), "SOLID T AFTER 600 S, LOOP STOPPED, 150 KW", false)
        .write_png(dir.join("rz_transient_solid_t_stopped.png"))
        .unwrap();
    assert!(c.bed_upflow.get::<kilogram_per_second>() > 0.0);
}
