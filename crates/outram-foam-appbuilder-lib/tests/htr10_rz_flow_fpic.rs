// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! HTR-10 FPIC full-power case (gh:#1150): R-Z porous gas flow with the
//! bypass paths, coupled to the gh:#1149 solid field; pebble-internal fuel
//! temperature; the 22 thermocouples; sensitivities; the drawings.
//!
//! **Methodology, predictions (written before the runs), results:**
//! `verification_and_validation/htr10_rz_flow_fpic/README.md`. Each test
//! prints its table and the README records it. Assertions: the plant energy
//! balance of every coupled solve (1e-5), the outer-iteration convergence,
//! and regression pins of the recorded numbers (0.1 K / 0.1 %). The
//! comparison with the measurements is **reported, not gated**: TECDOC-1694
//! gives no measurement uncertainty and so no acceptance criterion
//! (Sec 7.1.1.2).
//!
//! **Drawings** (the crate's "geometry is DRAWN" HARD RULE), written into
//! `verification_and_validation/htr10_rz_flow_fpic/`: every pixel is
//! coloured from the ASSEMBLED mesh (`RzMesh::cell_at` at the pixel centre)
//! and the ASSEMBLED gas network (`GasNetwork::cell_flow`, the flow region
//! each cell actually carries), never from the region constants. `z` is
//! TECDOC's (downward), drawn increasing down the page.

use outram_blender::csg::plot::annotate::draw_text;
use outram_blender::csg::plot::colour::{Rgb, BLACK, WHITE};
use outram_blender::csg::plot::image::ImageData;
use outram_blender::csg::plot::{annotate_image, LegendEntry};
use outram_foam_appbuilder_lib::htgr_closures::{BedConductivityModel, ZbsBed};
use outram_foam_appbuilder_lib::htr10_rz::{
    BypassSplit, CarbonBrickConductivity, FlowKind, FpicOptions, FpicResult, Htr10FpicModel, Htr10RzCase,
    Htr10RzModel, Htr10RzOptions, MaterialOptions, OperatingPoint, PebbleMatrix,
};
use uom::si::f64::{Length, ThermalConductivity, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::power::watt;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

fn c(t: ThermodynamicTemperature) -> f64 {
    t.get::<kelvin>() - 273.15
}

fn brick30() -> MaterialOptions {
    MaterialOptions {
        carbon_brick: CarbonBrickConductivity::Constant(ThermalConductivity::new::<watt_per_meter_kelvin>(30.0)),
        ..MaterialOptions::default()
    }
}

fn zbs() -> MaterialOptions {
    MaterialOptions {
        bed: BedConductivityModel::Zbs(ZbsBed::htr10()),
        ..MaterialOptions::default()
    }
}

/// Residual statistics over the 22 thermocouples: mean, RMS, max |res|,
/// count within +/-100 K, count inside the participants' range.
struct Stats {
    mean: f64,
    rms: f64,
    max_abs: (f64, usize),
    within_100: usize,
    inside_range: usize,
}

fn stats(res: &[(usize, f64)], inside: usize) -> Stats {
    let n = res.len() as f64;
    let mean = res.iter().map(|r| r.1).sum::<f64>() / n;
    let rms = (res.iter().map(|r| r.1 * r.1).sum::<f64>() / n).sqrt();
    let max_abs = res.iter().fold((0.0, 0), |a, r| if r.1.abs() > a.0 { (r.1.abs(), r.0) } else { a });
    Stats {
        mean,
        rms,
        max_abs,
        within_100: res.iter().filter(|r| r.1.abs() <= 100.0).count(),
        inside_range: inside,
    }
}

fn model_stats(r: &FpicResult) -> Stats {
    let res: Vec<(usize, f64)> = r.thermocouples.iter().map(|t| (t.tc.number, t.residual())).collect();
    stats(&res, r.thermocouples.iter().filter(|t| t.inside_participant_range()).count())
}

fn run(label: &str, opts: FpicOptions) -> (Htr10FpicModel, FpicResult) {
    let t0 = std::time::Instant::now();
    let mut m = Htr10FpicModel::build(opts).unwrap();
    let r = m.solve().unwrap();
    let s = model_stats(&r);
    let tc = |k: usize| r.thermocouples[k - 1].model_c;
    println!(
        "| {label} | {} | {} | {:.2} | {:.2} | {:.1} | {:.1} ({:.1}, {:.1}) | {:.1} | {:.1} | {:.1} | {:+.1} | {:.1} | {} | {} | {:.0} | {:.0} | {:.0} | {:.0} | {:.0} | {:.0} | {:.0} | {:.0} | {:.0} | {:.0} | {:.1e} | {:.0} s |",
        r.solid.n_cells,
        r.outer_iterations,
        c(r.outlet_temperature),
        c(r.bed_outlet_temperature),
        r.heat_to_rccs_and_heads.get::<watt>() / 1e3,
        r.max_fuel_centre.0.benchmark_centre - 273.15,
        r.max_fuel_centre.1 .0,
        r.max_fuel_centre.1 .1,
        r.max_fuel_surface.0 - 273.15,
        r.max_fuel_centre.0.kernel_max - 273.15,
        r.core_pressure_drop,
        s.mean,
        s.rms,
        s.within_100,
        s.inside_range,
        tc(2),
        tc(3),
        tc(6),
        tc(12),
        tc(13),
        tc(14),
        tc(17),
        tc(19),
        tc(21),
        tc(22),
        r.plant_imbalance,
        t0.elapsed().as_secs_f64()
    );
    assert!(r.plant_imbalance < 1e-5, "{label}: plant imbalance {}", r.plant_imbalance);
    (m, r)
}

const HEADER: &str = "| case | cells | outer passes | outlet C | bed outlet C | RCCS+heads kW | max fuel centre C (r, z cm) | max surface C | max kernel C | core dp Pa | mean res K | RMS K | within 100 K | in range | TC2 | TC3 | TC6 | TC12 | TC13 | TC14 | TC17 | TC19 | TC21 | TC22 | imbalance | wall |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|";

fn pin(label: &str, got: f64, want: f64, tol: f64) {
    assert!((got - want).abs() <= tol, "{label}: {got} vs recorded {want}");
}

// ---------------------------------------------------------------- drawing --

struct Window {
    r: [f64; 2],
    z: [f64; 2],
    px_per_cm: f64,
}

fn raster(m: &Htr10FpicModel, w: &Window) -> (usize, usize, Vec<Option<usize>>) {
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

fn flow_key(m: &Htr10FpicModel, c: usize) -> u8 {
    m.network().cell_flow(c).map_or(6, |(f, _)| f.tab25)
}

fn flow_region_image(m: &Htr10FpicModel, w: &Window, mesh_lines: bool, tick: f64, title: &str) -> ImageData {
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
    m: &Htr10FpicModel,
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

fn out_dir() -> std::path::PathBuf {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation")
        .join("htr10_rz_flow_fpic");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Draw an arrow of the 2-D mass flux at the centre of every `step`-th gas
/// cell (length ~ log of |G|).
fn arrows(img: &mut ImageData, m: &Htr10FpicModel, w: &Window) {
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
/// thermocouples, maxima, drawings.
#[test]
fn fpic_base_case_thermocouples_and_drawings() {
    let dir = out_dir();
    println!("{HEADER}");
    let (m, r) = run("BASE (spec, 87/1/2/10, Breitbach, Tab 2.12 brick) 5 cm", FpicOptions::default());

    // Convergence history.
    let h = &r.outer_history;
    let show: Vec<String> = [1usize, 2, 5, 10, 20, 40, 80, 120, 160, 200, 300]
        .iter()
        .filter(|k| **k <= h.len())
        .map(|k| format!("{k}: {:.2e}", h[k - 1]))
        .collect();
    println!(
        "outer iteration: {} passes to {:.1e} K; max change by pass {}; solid Picard (last) {} passes; gas flow Picard {:?}",
        r.outer_iterations,
        h[h.len() - 1],
        show.join(", "),
        r.solid.picard.iterations,
        m.gas().unwrap().flow_picard
    );
    println!(
        "energy: P 10000 kW; RCCS {:.2} kW, top head {:.2} kW, bottom head {:.2} kW; to gas (solid side) {:.2} kW; W cp dT {:.2} kW; plant imbalance {:.2e}; solid imbalance {:.2e}",
        r.solid.rccs_side.get::<watt>() / 1e3,
        r.solid.top_head.get::<watt>() / 1e3,
        r.solid.bottom_head.get::<watt>() / 1e3,
        r.heat_to_gas.get::<watt>() / 1e3,
        r.enthalpy_rise.get::<watt>() / 1e3,
        r.plant_imbalance,
        r.solid.relative_imbalance
    );
    println!(
        "taps: bottom cavity {:.2} C, leak take-off {:.2} C, top plenum {:.2} C; side reflector surface max {:.1} C; bottom reflector max {:.1} C",
        r.tap_temperatures[0] - 273.15,
        r.tap_temperatures[1] - 273.15,
        r.tap_temperatures[2] - 273.15,
        r.max_side_reflector_surface - 273.15,
        r.max_bottom_reflector - 273.15
    );
    let p = &r.max_fuel_centre.0;
    println!(
        "hottest pebble at (r {:.1}, z {:.1}) cm: surface {:.1} C, fuel-zone surface {:.1} C, centre {:.1} C, benchmark centre {:.1} C, kernel {:.1} C, k_fz {:.2} W/(m K), {:.1} W/pebble",
        r.max_fuel_centre.1 .0,
        r.max_fuel_centre.1 .1,
        p.surface - 273.15,
        p.fuel_zone_surface - 273.15,
        p.centre - 273.15,
        p.benchmark_centre - 273.15,
        p.kernel_max - 273.15,
        p.fuel_zone_conductivity,
        p.pebble_power
    );

    // Thermocouple table.
    println!("| TC | location | r | z | cells | instrument | measured C | ours C | residual K | solid C | gas C | INET C | INET res K | participants min-max C | inside |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    let mut inet = Vec::new();
    let mut part_mean = Vec::new();
    let mut inet_inside = 0;
    for t in &r.thermocouples {
        let (lo, hi) = t.tc.participant_range();
        let pm: Vec<f64> = t.tc.participants.iter().flatten().copied().collect();
        inet.push((t.tc.number, t.tc.inet_c - t.tc.measured_c));
        part_mean.push((t.tc.number, pm.iter().sum::<f64>() / pm.len() as f64 - t.tc.measured_c));
        if t.tc.inet_c >= lo && t.tc.inet_c <= hi {
            inet_inside += 1;
        }
        println!(
            "| {} | {} | {} | {} | {} | {:?} | {:.1} | {:.1} | {:+.1} | {:.1} | {:.1} | {:.1} | {:+.1} | {:.0}-{:.0} | {} |",
            t.tc.number,
            t.tc.location,
            t.tc.r_cm,
            t.tc.z_cm,
            t.regions.join(" / "),
            t.instrument,
            t.tc.measured_c,
            t.model_c,
            t.residual(),
            t.solid_c,
            t.gas_c,
            t.tc.inet_c,
            t.tc.inet_c - t.tc.measured_c,
            lo,
            hi,
            if t.inside_participant_range() { "yes" } else { "no" }
        );
    }
    let ours = model_stats(&r);
    let si = stats(&inet, inet_inside);
    let sp = stats(&part_mean, 22);
    for (name, s) in [("ours", &ours), ("INET (Tab 2.14)", &si), ("participant mean (all submitted)", &sp)] {
        println!(
            "{name}: mean residual {:+.1} K, RMS {:.1} K, max |res| {:.1} K (TC {}), within 100 K {}/22, inside participants' range {}/22",
            s.mean, s.rms, s.max_abs.0, s.max_abs.1, s.within_100, s.inside_range
        );
    }
    // Side-reflector subset (TC 1-12).
    let side: Vec<(usize, f64)> = r.thermocouples[..12].iter().map(|t| (t.tc.number, t.residual())).collect();
    let ss = stats(&side, 0);
    println!("ours, side reflector TC 1-12: mean {:+.1} K, RMS {:.1} K", ss.mean, ss.rms);

    // Drawings.
    let whole = Window { r: [0.0, 218.0], z: [-351.0, 665.0], px_per_cm: 1.0 };
    flow_region_image(&m, &whole, false, 50.0, "HTR-10 FLOW REGIONS (ASSEMBLED GAS NETWORK, CELL LOOKUP)")
        .write_png(dir.join("htr10_fpic_flow_region_map.png"))
        .unwrap();
    flow_region_image(
        &m,
        &Window { r: [0.0, 218.0], z: [150.0, 520.0], px_per_cm: 3.0 },
        true,
        25.0,
        "ZOOM: CONE, BOTTOM REFLECTOR, PLENA, LEAK, THROTTLE (5 CM MESH)",
    )
    .write_png(dir.join("htr10_fpic_flow_regions_zoom_bottom.png"))
    .unwrap();
    let g = m.gas().unwrap();
    let n = m.solid().field().mesh().n_cells();
    let gmag: Vec<Option<f64>> = (0..n)
        .map(|c| {
            let (a, b) = g.mass_flux[c];
            g.temperature[c].is_finite().then(|| a.hypot(b).max(1e-3).log10())
        })
        .collect();
    let core = Window { r: [0.0, 218.0], z: [-100.0, 520.0], px_per_cm: 2.0 };
    scalar_image(&m, &core, &gmag, (-3.0, 1.0), |v| format!("G {:.0e} KG/M2S", 10f64.powf(v)), "SUPERFICIAL MASS FLUX MAGNITUDE (LOG); ARROWS: 2-D DOMAIN DIRECTION, WHITE TIP", true)
        .write_png(dir.join("htr10_fpic_mass_flux.png"))
        .unwrap();
    let tg: Vec<Option<f64>> = (0..n).map(|c| g.temperature[c].is_finite().then(|| g.temperature[c])).collect();
    let ts: Vec<Option<f64>> = (0..n).map(|c| Some(m.solid().field().temperature(c).get::<kelvin>())).collect();
    let span = |v: &[Option<f64>]| v.iter().flatten().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), t| (a.min(*t), b.max(*t)));
    scalar_image(&m, &whole, &tg, span(&tg), |v| format!("{:.0} C", v - 273.15), "GAS TEMPERATURE (GREY: NO GAS FLOW)", false)
        .write_png(dir.join("htr10_fpic_gas_temperature.png"))
        .unwrap();
    scalar_image(&m, &whole, &ts, span(&ts), |v| format!("{:.0} C", v - 273.15), "SOLID TEMPERATURE (BED: PEBBLE SURFACE)", false)
        .write_png(dir.join("htr10_fpic_solid_temperature.png"))
        .unwrap();

    // Regression pins (recorded 2026-11-03; see the README).
    pin("outlet C", c(r.outlet_temperature), PIN_OUTLET_C, 0.1);
    pin("max fuel centre C", p.benchmark_centre - 273.15, PIN_MAX_FUEL_C, 0.1);
    pin("RMS residual K", ours.rms, PIN_RMS_K, 0.1);
}

// Re-measured 2026-11-03 with RPV-cavity natural convection ON by default
// (gh:#1173); radiation-only record: 689.27, 912.6, 49.4.
const PIN_OUTLET_C: f64 = 688.75;
const PIN_MAX_FUEL_C: f64 = 912.0;
const PIN_RMS_K: f64 = 49.8;

/// One-at-a-time sensitivities of the FPIC case (D3, D4, D5, the brick
/// ablation, the pebble rule, the flow and the mesh).
#[test]
fn fpic_sensitivities() {
    println!("{HEADER}");
    let base = FpicOptions::default();
    let (_, b) = run("BASE 5 cm", base);
    let cases: Vec<(&str, FpicOptions)> = vec![
        ("SS-ASRUN (10.3 MW, 236 C, 2.94 MPa, 4.273 kg/s)", FpicOptions { operating_point: OperatingPoint::fpic_as_run(), ..base }),
        ("FLOW-EB (4.278 kg/s)", FpicOptions { operating_point: OperatingPoint::fpic_energy_balance_flow(), ..base }),
        ("BYP-G02 (86 / 14)", FpicOptions { split: BypassSplit::gao_2002(), ..base }),
        ("BYP-NOLEAK (97/1/2/0)", FpicOptions { split: BypassSplit::no_leakage(), ..base }),
        ("ZBS bed (D3 comparison)", FpicOptions { solid: Htr10RzOptions { materials: zbs(), ..base.solid }, ..base }),
        ("BRICK-30 (ablation, unsourced)", FpicOptions { solid: Htr10RzOptions { materials: brick30(), ..base.solid }, ..base }),
        ("PEBBLE Tab 2.9 homogeneous", FpicOptions { pebble: PebbleMatrix::Tab29Homogeneous, ..base }),
        ("MESH 10 cm", FpicOptions { solid: Htr10RzOptions { max_cell: Some(Length::new::<centimeter>(10.0)), ..base.solid }, ..base }),
        ("MESH Fig 2.5", FpicOptions { solid: Htr10RzOptions { max_cell: None, ..base.solid }, ..base }),
    ];
    for (label, o) in cases {
        let (_, r) = run(label, o);
        let d: Vec<String> = r
            .thermocouples
            .iter()
            .zip(&b.thermocouples)
            .map(|(a, bb)| format!("{}:{:+.1}", a.tc.number, a.model_c - bb.model_c))
            .collect();
        println!("  {label} minus BASE per TC: {}", d.join(" "));
    }
}

/// The carbon-brick ablation inside the gh:#1149 stagnant A-MEAN950 case:
/// how much of the 102.5 kW vs 254.0 kW (htgr_sim_v1) gap the unsourced
/// 30 W/(m K) carries in THIS model. `htgr_sim_v1` is not touched.
#[test]
fn brick_ablation_in_the_stagnant_mean950_case() {
    let case = Htr10RzCase::BedMeanTemperature {
        temperature: ThermodynamicTemperature::new::<kelvin>(950.0),
    };
    let mut out = Vec::new();
    for (label, mats) in [("Tab 2.12 (5-8 W/(m K))", MaterialOptions::default()), ("30 W/(m K) (htgr_sim_v1, unsourced)", brick30())] {
        let mut m = Htr10RzModel::build(Htr10RzOptions { materials: mats, ..Htr10RzOptions::default() }, case).unwrap();
        let r = m.solve_steady().unwrap();
        let probe = |rr: f64| m.temperature_at_cm(rr, 90.0).unwrap().get::<kelvin>() - 273.15;
        println!(
            "A-MEAN950 brick {label}: RCCS {:.2} kW, heads {:.2}/{:.2} kW, in {:.2} kW; T(93) {:.1} C, T(167) {:.1} C, T(189) {:.1} C, T(214) {:.1} C; imbalance {:.1e}",
            r.rccs_side.get::<watt>() / 1e3,
            r.top_head.get::<watt>() / 1e3,
            r.bottom_head.get::<watt>() / 1e3,
            r.generated.get::<watt>() / 1e3,
            probe(93.0),
            probe(167.0),
            probe(189.0),
            probe(214.0),
            r.relative_imbalance
        );
        assert!(r.relative_imbalance < 1e-6);
        out.push(r.rccs_side.get::<watt>() / 1e3);
    }
    let gap = 254.0 - out[0];
    println!(
        "brick 30 moves the RCCS duty {:.2} -> {:.2} kW ({:+.1} %): {:.1} % of the {:.1} kW gap to htgr_sim_v1's 254.0 kW",
        out[0],
        out[1],
        100.0 * (out[1] / out[0] - 1.0),
        100.0 * (out[1] - out[0]) / gap,
        gap
    );
    // 105.07 since gh:#1173 (cavity convection ON); 102.53 radiation only.
    pin("A-MEAN950 base RCCS kW (gh:#1149 record)", out[0], 105.07, 0.11);
    pin("A-MEAN950 brick-30 RCCS kW", out[1], PIN_BRICK30_KW, 0.12);
}

// 113.02 since gh:#1173 (cavity convection ON); 110.15 radiation only.
const PIN_BRICK30_KW: f64 = 113.02;

/// Mesh study of the FPIC case at 2.5 cm (41 580 cells), against the 5 cm
/// base and the 10 cm / Fig 2.5 meshes of [`fpic_sensitivities`]. Behind the
/// crate's `long-tests` feature (on by default) because of its run time.
#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "long test: enable the long-tests feature")]
fn fpic_mesh_2p5cm() {
    println!("{HEADER}");
    let base = FpicOptions::default();
    let (_, r) = run(
        "MESH 2.5 cm",
        FpicOptions { solid: Htr10RzOptions { max_cell: Some(Length::new::<centimeter>(2.5)), ..base.solid }, ..base },
    );
    let d: Vec<String> = r.thermocouples.iter().map(|t| format!("{}:{:.1}", t.tc.number, t.model_c)).collect();
    println!("  MESH 2.5 cm per TC (C): {}", d.join(" "));
}
