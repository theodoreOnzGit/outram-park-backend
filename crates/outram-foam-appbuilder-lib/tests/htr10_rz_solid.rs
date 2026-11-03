// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! HTR-10 R-Z solid model (gh:#1149 part B): the drawings, case (a) under
//! Breitbach and ZBS, its mesh convergence and the sensitivities.
//!
//! **Methodology, predictions, results:**
//! `verification_and_validation/htr10_rz_solid/README.md`. Each test prints
//! its table; the README records them. The assertions are the energy
//! balance (every solve, to 1e-6) and regression pins of the recorded
//! numbers (0.1 %); the comparison with 254.0 kW and 206 kW is reported, not
//! gated, because neither is the same quantity measured the same way (see the
//! README).
//!
//! **Drawings** (the crate's "geometry is DRAWN" HARD RULE), written into
//! `verification_and_validation/htr10_rz_solid/`: every pixel is coloured
//! from the ASSEMBLED mesh (`RzMesh::cell_at` at the pixel centre, then the
//! cell's region or temperature); mesh lines are drawn where neighbouring
//! pixels fall in different cells. Nothing is drawn from the rectangles.
//! `z` is TECDOC's (downward), so the images are drawn with `z` increasing
//! down the page, as in TECDOC-1694 Fig 2.5: up on the page is up in the
//! reactor.

use outram_blender::csg::plot::annotate::draw_text;
use outram_blender::csg::plot::colour::{Rgb, BLACK, WHITE};
use outram_blender::csg::plot::image::ImageData;
use outram_blender::csg::plot::{annotate_image, LegendEntry};
use outram_foam_appbuilder_lib::htgr_closures::{BedConductivityModel, ZbsBed};
use outram_foam_appbuilder_lib::htr10_rz::geometry::{regions, N_REGIONS};
use outram_foam_appbuilder_lib::htr10_rz::{
    CoreVesselPlacement, HeadBoundary, Htr10RzCase, Htr10RzModel, Htr10RzOptions, Htr10RzResult,
    MaterialOptions, RccsView, SideReflectorBelow300C,
};
use uom::si::f64::{Length, Power, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::power::watt;
use uom::si::thermodynamic_temperature::kelvin;

fn kel(t: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(t)
}
fn held() -> Htr10RzCase {
    Htr10RzCase::HeldBed {
        temperature: kel(950.0),
    }
}
fn mean950() -> Htr10RzCase {
    Htr10RzCase::BedMeanTemperature {
        temperature: kel(950.0),
    }
}
fn decay() -> Htr10RzCase {
    Htr10RzCase::DecayPower {
        power: Power::new::<watt>(2.0e5),
    }
}
fn kw(p: Power) -> f64 {
    p.get::<watt>() / 1e3
}
fn zbs() -> MaterialOptions {
    MaterialOptions {
        bed: BedConductivityModel::Zbs(ZbsBed::htr10()),
        ..MaterialOptions::default()
    }
}

fn run(label: &str, opts: Htr10RzOptions, case: Htr10RzCase) -> (Htr10RzModel, Htr10RzResult) {
    let t0 = std::time::Instant::now();
    let mut m = Htr10RzModel::build(opts, case).unwrap();
    let r = m.solve_steady().unwrap();
    let flags: Vec<String> = r
        .range_flags()
        .iter()
        .map(|f| format!("{} {:.0}-{:.0} K", f.name, f.min.get::<kelvin>(), f.max.get::<kelvin>()))
        .collect();
    let probe = |rr: f64, z: f64| m.temperature_at_cm(rr, z).unwrap().get::<kelvin>() - 273.15;
    println!(
        "| {label} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} K | {:.1} K at ({:.1}, {:.1}) | {:.1} | {:.1} | {:.1} | {} | {} | {:.1e} | {} | {:.0} s | {}",
        r.n_cells,
        kw(r.rccs_side),
        kw(r.top_head),
        kw(r.bottom_head),
        kw(r.supplied_by_held_bed) + kw(r.generated),
        r.bed_mean_temperature.get::<kelvin>(),
        r.max_temperature.get::<kelvin>(),
        r.max_at_cm.0,
        r.max_at_cm.1,
        probe(93.0, 90.0),
        probe(180.0, 90.0),
        probe(214.0, 90.0),
        r.side_reflector_cells_below_300c,
        r.picard.iterations,
        r.relative_imbalance,
        flags.len(),
        t0.elapsed().as_secs_f64(),
        flags.join("; ")
    );
    assert!(r.relative_imbalance < 1e-6, "{label}: imbalance {}", r.relative_imbalance);
    (m, r)
}

const HEADER: &str = "| case | cells | RCCS kW | top head kW | bottom head kW | in kW | bed mean | hottest | T(93,90) C | T(180,90) C | T(214,90) C | SR cells < 300 C | Picard | imbalance | range flags | wall | flagged regions";

fn pin(label: &str, got: f64, want: f64) {
    assert!((got / want - 1.0).abs() < 1e-3, "{label}: {got} vs recorded {want}");
}

// ---------------------------------------------------------------- drawing --

fn palette(i: usize) -> Rgb {
    const P: [Rgb; N_REGIONS] = [
        Rgb::new(230, 100, 40),  // bed
        Rgb::new(60, 140, 60),   // side graphite
        Rgb::new(110, 190, 110), // top/bottom graphite
        Rgb::new(30, 100, 160),  // CR ring
        Rgb::new(90, 170, 230),  // risers
        Rgb::new(120, 90, 60),   // carbon brick
        Rgb::new(70, 70, 80),    // RPV
        Rgb::new(150, 150, 165), // core vessel
        Rgb::new(170, 220, 120), // upper bottom refl
        Rgb::new(140, 200, 90),  // lower bottom refl
        Rgb::new(185, 185, 200), // bottom steel 1
        Rgb::new(115, 115, 130), // bottom steel 2
        Rgb::new(255, 230, 130), // bottom annular He
        Rgb::new(200, 160, 200), // leak steel
        Rgb::new(30, 120, 90),   // side inner graphite
        Rgb::new(255, 250, 215), // top He cavity
        Rgb::new(255, 210, 160), // core entry plenum
        Rgb::new(250, 200, 90),  // bottom rounding He
        Rgb::new(240, 240, 190), // bottom He cavity
        Rgb::new(255, 120, 160), // gap vessel-RPV
        Rgb::new(255, 160, 60),  // hot plenum
        Rgb::new(220, 150, 230), // small plenum
        Rgb::new(90, 90, 40),    // tube throttle
        Rgb::new(20, 60, 120),   // CR exit throttle
        Rgb::new(160, 210, 160), // bottom refl solid
        Rgb::new(230, 30, 60),   // gap brick-vessel
        Rgb::new(180, 60, 200),  // He entry path
        Rgb::new(130, 40, 160),  // annular path
        Rgb::new(255, 255, 0),   // top plenum
        Rgb::new(200, 230, 60),  // top flow zone
        Rgb::new(250, 160, 120), // discharge tube
    ];
    P[i]
}

const MESH: Rgb = Rgb::new(40, 40, 40);

struct Window {
    r: [f64; 2],
    z: [f64; 2],
    px_per_cm: f64,
}

fn raster(m: &Htr10RzModel, w: &Window) -> (usize, usize, Vec<Option<usize>>) {
    let nx = ((w.r[1] - w.r[0]) * w.px_per_cm).round() as usize;
    let ny = ((w.z[1] - w.z[0]) * w.px_per_cm).round() as usize;
    let mut cells = Vec::with_capacity(nx * ny);
    for y in 0..ny {
        for x in 0..nx {
            let r = w.r[0] + (x as f64 + 0.5) / w.px_per_cm;
            let z = w.z[0] + (y as f64 + 0.5) / w.px_per_cm;
            cells.push(m.field().mesh().cell_at(
                Length::new::<centimeter>(r),
                Length::new::<centimeter>(z),
            ));
        }
    }
    (nx, ny, cells)
}

fn overlay(img: &mut ImageData, cells: &[Option<usize>]) {
    let (w, h) = (img.width, img.height);
    for y in 0..h {
        for x in 0..w {
            let here = cells[y * w + x];
            if (x + 1 < w && cells[y * w + x + 1] != here) || (y + 1 < h && cells[(y + 1) * w + x] != here) {
                img.set(x, y, MESH);
            }
        }
    }
}

/// Region boundaries only (where the region changes), for the map without
/// the mesh.
fn outline_regions(img: &mut ImageData, m: &Htr10RzModel, cells: &[Option<usize>]) {
    let (w, h) = (img.width, img.height);
    let reg = |c: Option<usize>| c.map(|c| m.field().mesh().cell_region(c));
    for y in 0..h {
        for x in 0..w {
            let here = reg(cells[y * w + x]);
            if (x + 1 < w && reg(cells[y * w + x + 1]) != here) || (y + 1 < h && reg(cells[(y + 1) * w + x]) != here) {
                img.set(x, y, BLACK);
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

fn region_legend(m: &Htr10RzModel, cells: &[Option<usize>]) -> Vec<LegendEntry> {
    let mut seen = [false; N_REGIONS];
    for c in cells.iter().flatten() {
        seen[m.field().mesh().cell_region(*c)] = true;
    }
    regions(m.options().vessel)
        .iter()
        .enumerate()
        .filter(|(i, _)| seen[*i])
        .map(|(i, s)| LegendEntry::new(palette(i), s.name))
        .collect()
}

fn region_image(m: &Htr10RzModel, w: &Window, mesh: bool, tick: f64, title: &str) -> ImageData {
    let (nx, ny, cells) = raster(m, w);
    let mut img = ImageData::filled(nx, ny, WHITE);
    for (p, c) in cells.iter().enumerate() {
        if let Some(c) = c {
            img.pixels[p] = palette(m.field().mesh().cell_region(*c));
        }
    }
    if mesh {
        overlay(&mut img, &cells);
    } else {
        outline_regions(&mut img, m, &cells);
    }
    let legend = region_legend(m, &cells);
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

fn temperature_image(m: &Htr10RzModel, w: &Window, title: &str) -> ImageData {
    let (nx, ny, cells) = raster(m, w);
    let ts: Vec<f64> = (0..m.field().mesh().n_cells())
        .map(|c| m.field().temperature(c).get::<kelvin>())
        .collect();
    let (lo, hi) = ts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), t| (a.min(*t), b.max(*t)));
    let mut img = ImageData::filled(nx, ny, WHITE);
    for (p, c) in cells.iter().enumerate() {
        if let Some(c) = c {
            img.pixels[p] = ramp((ts[*c] - lo) / (hi - lo));
        }
    }
    outline_regions(&mut img, m, &cells);
    let legend: Vec<LegendEntry> = (0..=5)
        .map(|i| {
            let s = f64::from(i) / 5.0;
            LegendEntry::new(ramp(s), format!("{:.0} C", lo + s * (hi - lo) - 273.15))
        })
        .collect();
    frame(&img, w, 50.0, title, &legend)
}

fn out_dir() -> std::path::PathBuf {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation")
        .join("htr10_rz_solid");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The drawings, the base cases (Breitbach and ZBS) and their temperature
/// maps.
#[test]
fn htr10_rz_drawings_and_case_a() {
    let dir = out_dir();
    let base = Htr10RzOptions::default();
    let whole = Window { r: [0.0, 218.0], z: [-351.0, 665.0], px_per_cm: 1.0 };
    let probe = Htr10RzModel::build(base, held()).unwrap();
    region_image(&probe, &whole, false, 50.0, "HTR-10 R-Z REGIONS (ASSEMBLED MESH, CELL LOOKUP)")
        .write_png(dir.join("htr10_rz_region_map.png"))
        .unwrap();
    let coarse = Htr10RzModel::build(Htr10RzOptions { max_cell: Some(Length::new::<centimeter>(10.0)), ..base }, held()).unwrap();
    let w2 = Window { px_per_cm: 2.0, ..whole };
    region_image(&coarse, &w2, true, 50.0, "HTR-10 R-Z MESH, 10 CM CELLS (FIG 2.5 LINES SUBDIVIDED)")
        .write_png(dir.join("htr10_rz_mesh_10cm.png"))
        .unwrap();
    region_image(&probe, &Window { r: [150.0, 218.0], z: [-60.0, 240.0], px_per_cm: 6.0 }, true, 10.0, "ZOOM: BRICK, GAPS, BARREL, RPV (5 CM MESH)")
        .write_png(dir.join("htr10_rz_zoom_gaps_vessel.png"))
        .unwrap();
    region_image(&probe, &Window { r: [0.0, 218.0], z: [170.0, 665.0], px_per_cm: 3.0 }, true, 25.0, "ZOOM: CONE, BOTTOM REFLECTOR, SUPPORTS (5 CM MESH)")
        .write_png(dir.join("htr10_rz_zoom_bottom.png"))
        .unwrap();
    region_image(&probe, &Window { r: [0.0, 218.0], z: [-351.0, 20.0], px_per_cm: 3.0 }, true, 25.0, "ZOOM: TOP REFLECTOR, BRICK, CAVITY, HEAD (5 CM MESH)")
        .write_png(dir.join("htr10_rz_zoom_top.png"))
        .unwrap();

    println!("{HEADER}");
    let (_, r) = run("A-MEAN950 Breitbach 5 cm", base, mean950());
    let (_, rz) = run("A-MEAN950 ZBS 5 cm", Htr10RzOptions { materials: zbs(), ..base }, mean950());
    let (m, rh) = run("A-HELD Breitbach 5 cm", base, held());
    temperature_image(&m, &whole, "A-HELD: CORE ISOTHERMAL AT 950 K, STAGNANT")
        .write_png(dir.join("htr10_rz_temperature_held_950k.png"))
        .unwrap();
    let (_, rhz) = run("A-HELD ZBS 5 cm", Htr10RzOptions { materials: zbs(), ..base }, held());
    let (m, rd) = run("A-DECAY 200 kW Breitbach 5 cm", base, decay());
    temperature_image(&m, &whole, "A-DECAY: 200 KW, STAGNANT, BREITBACH")
        .write_png(dir.join("htr10_rz_temperature_decay_200kw.png"))
        .unwrap();
    let (_, rdz) = run("A-DECAY 200 kW ZBS 5 cm", Htr10RzOptions { materials: zbs(), ..base }, decay());
    let (_, rdk) = run(
        "A-DECAY 200 kW Kugeler2017 5 cm",
        Htr10RzOptions {
            materials: MaterialOptions { bed: BedConductivityModel::Kugeler2017, ..MaterialOptions::default() },
            ..base
        },
        decay(),
    );
    for (name, x) in [("A-MEAN950 Breitbach", &r), ("A-MEAN950 ZBS", &rz), ("A-HELD Breitbach", &rh), ("A-HELD ZBS", &rhz)] {
        println!(
            "{name}: RCCS {:.1} kW, all {:.1} kW; RCCS against htgr_sim_v1 254.0 kW {:+.1} %, against Hu 206 kW {:+.1} %",
            kw(x.rccs_side),
            kw(x.rccs_side) + kw(x.top_head) + kw(x.bottom_head),
            100.0 * (kw(x.rccs_side) / 254.0 - 1.0),
            100.0 * (kw(x.rccs_side) / 206.0 - 1.0)
        );
    }
    // Regression pins of the recorded 5 cm numbers (README), 0.1 %.
    pin("A-MEAN950 Breitbach RCCS", kw(r.rccs_side), PIN_MEAN950_B);
    pin("A-MEAN950 ZBS RCCS", kw(rz.rccs_side), PIN_MEAN950_Z);
    pin("A-HELD Breitbach RCCS", kw(rh.rccs_side), PIN_HELD_B);
    pin("A-DECAY Breitbach hottest", rd.max_temperature.get::<kelvin>(), PIN_DECAY_B);
    pin("A-DECAY ZBS hottest", rdz.max_temperature.get::<kelvin>(), PIN_DECAY_Z);
    pin("A-DECAY Kugeler hottest", rdk.max_temperature.get::<kelvin>(), PIN_DECAY_K);
}

// Re-measured 2026-11-03 when RPV-cavity natural convection became the
// default (gh:#1173). Radiation-only values (`without_cavity_convection`):
// 102.526, 127.15, 235.398, 1557.877, 1433.2, 1577.1.
const PIN_MEAN950_B: f64 = 105.068;
const PIN_MEAN950_Z: f64 = 130.56;
const PIN_HELD_B: f64 = 242.34;
const PIN_DECAY_B: f64 = 1555.1;
const PIN_DECAY_Z: f64 = 1429.4;
const PIN_DECAY_K: f64 = 1574.0;

/// Mesh convergence of case (a): the Fig 2.5 mesh itself, then every
/// interval split to at most 10, 5, 2.5 and 1.25 cm. About 7 minutes in
/// release (the 1.25 cm A-MEAN950 power search alone is ~5 min), so it is
/// behind the crate's `long-tests` feature (on by default).
#[test]
#[cfg_attr(not(feature = "long-tests"), ignore = "long test: enable the long-tests feature")]
fn htr10_rz_mesh_convergence() {
    println!("{HEADER}");
    let levels = [
        ("native", None),
        ("10 cm", Some(10.0)),
        ("5 cm", Some(5.0)),
        ("2.5 cm", Some(2.5)),
        ("1.25 cm", Some(1.25)),
    ];
    let mut q = Vec::new();
    let mut qh = Vec::new();
    let mut t = Vec::new();
    for (label, h) in levels {
        let opts = Htr10RzOptions {
            max_cell: h.map(Length::new::<centimeter>),
            ..Htr10RzOptions::default()
        };
        q.push(kw(run(&format!("A-MEAN950 {label}"), opts, mean950()).1.rccs_side));
        qh.push(kw(run(&format!("A-HELD {label}"), opts, held()).1.rccs_side));
        t.push(run(&format!("A-DECAY {label}"), opts, decay()).1.max_temperature.get::<kelvin>());
    }
    // Observed order and Richardson extrapolation on the last three levels
    // (uniform halving from 5 cm down).
    for (name, v) in [("A-MEAN950 RCCS kW", &q), ("A-HELD RCCS kW", &qh), ("A-DECAY hottest K", &t)] {
        let (a, b, c) = (v[2], v[3], v[4]);
        let p = ((a - b) / (b - c)).abs().ln() / 2f64.ln();
        let ext = c + (c - b) / (2f64.powf(p) - 1.0);
        println!("{name}: 5/2.5/1.25 cm = {a:.3} / {b:.3} / {c:.3}; observed order {p:.2}; Richardson {ext:.3}");
    }
}

/// One change from the base at a time, on both cases.
#[test]
fn htr10_rz_sensitivities() {
    println!("{HEADER}");
    let base = Htr10RzOptions::default();
    let cases: [(&str, Htr10RzOptions); 4] = [
        (
            "SR-TOPBOTTOM",
            Htr10RzOptions {
                materials: MaterialOptions {
                    side_reflector_below_300c: SideReflectorBelow300C::TopBottomFit,
                    ..MaterialOptions::default()
                },
                ..base
            },
        ),
        ("HEADS-ADIABATIC", Htr10RzOptions { heads: HeadBoundary::Adiabatic, ..base }),
        ("RCCS-SHIELD", Htr10RzOptions { rccs: RccsView::InsulatingPlateShield, ..base }),
        ("N5-TAB23", Htr10RzOptions { vessel: CoreVesselPlacement::Tab23, ..base }),
    ];
    for (label, o) in cases {
        run(&format!("A-MEAN950 {label}"), o, mean950());
        run(&format!("A-DECAY {label}"), o, decay());
    }
    run("A-MEAN950 HE-2.94MPa (SS-ASRUN pressure)", Htr10RzOptions {
        materials: MaterialOptions { helium_pressure_bar: 29.4, ..MaterialOptions::default() },
        ..base
    }, mean950());
    // The side-reflector hold is exercised only if a cell is below 300 C;
    // a cold held bed (inlet-like 523 K) makes sure the switch is reached.
    let cold = Htr10RzCase::HeldBed { temperature: kel(523.15) };
    let (_, a) = run("COLD-HELD 523 K hold300", base, cold);
    let (_, b) = run("COLD-HELD 523 K SR-TOPBOTTOM", cases[0].1, cold);
    assert!(a.side_reflector_cells_below_300c > 0);
    println!(
        "cold held bed: SR-TOPBOTTOM moves RCCS by {:+.3} %",
        100.0 * (kw(b.rccs_side) / kw(a.rccs_side) - 1.0)
    );
    let _ = pin;
}
