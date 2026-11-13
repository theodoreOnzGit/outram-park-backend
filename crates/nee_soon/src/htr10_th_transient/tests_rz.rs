//! Part B tests (gh:#1151): the neutron source, the CRW reactivity, the
//! coupled R-Z core, and the LOFC / CRW validation with its sensitivity
//! cases. Methodology, predictions (written before the first coupled run)
//! and results: `crates/nee_soon/verification_and_validation/htr10_rz_transients/README.md`.
//! The long runs print the tables the README records and write the plots
//! (PNG) and traces (CSV) into the same folder.

// The plot helpers and some imports serve only the `long-tests` runs.
#![cfg_attr(not(feature = "long-tests"), allow(unused_imports, dead_code))]

use petir::mathf::RealMath;
use outram_blender::csg::plot::annotate::draw_text;
use outram_blender::csg::plot::colour::{Rgb, BLACK, WHITE};
use outram_blender::csg::plot::image::ImageData;
use outram_blender::csg::plot::{annotate_image, LegendEntry};
use uom::si::f64::{Length, Power, ThermodynamicTemperature, Time};
use uom::si::length::centimeter;
use uom::si::power::watt;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use super::data::*;
use super::*;
use tampines::htr10_primary_loop::data::TabulatedSeries;
use tampines::htr10_primary_loop::{CoreModel, RzCoreTimeStep};

fn k(t: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(t)
}
fn s(t: f64) -> Time {
    Time::new::<second>(t)
}

/// The two part B data copies are the input sheet's files, byte for byte,
/// and parse: Tab 2.60 ends at 4.9762 mk (held after 128 s), 17 traces.
#[test]
fn part_b_data_copies_and_parsers() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/reactor-scoping/htr10-th-data/");
    for (name, embedded) in [
        ("crw-reactivity-tab2-60.csv", CRW_REACTIVITY_TAB2_60),
        ("lofc-thermocouples-c09-figs8-11.csv", LOFC_THERMOCOUPLES_C09),
    ] {
        let sheet = std::fs::read_to_string(format!("{dir}{name}")).expect("sheet file");
        assert_eq!(sheet, embedded, "{name} drifted from the input sheet");
    }
    let r = crw_reactivity().unwrap();
    assert!((r.value_at(128.0) - 4.9762e-3).abs() < 1e-12);
    assert!((r.value_at(500.0) - 4.9762e-3).abs() < 1e-12);
    assert!((r.value_at(10.0) - 1.0542e-3).abs() < 1e-12);
    let traces = lofc_thermocouple_traces().unwrap();
    assert_eq!(traces.len(), 17);
    let positions = c09_thermocouple_positions();
    for t in &traces {
        assert!(positions.iter().any(|p| p.location == t.label), "{} has a position", t.label);
    }
    let tr1 = traces.iter().find(|t| t.label == "TR1_TR4").unwrap();
    println!("TR1_TR4: {:.1} C at t0, {:.1} C at 3 h", tr1.value_at(0.0), tr1.value_at(10_800.0));
    assert!(tr1.value_at(10_800.0) > tr1.value_at(0.0) + 150.0);
}

/// **Correct physics is the default (HARD RULE):** the start-up source is
/// ON in the base kinetics; `SRC-OFF` is the explicit ablation.
/// **Verification:** at a held subcritical reactivity the fission power
/// settles to the source-multiplication value
/// `P = Q_f w S / (nu |rho|)`; without the source it keeps falling.
/// Prediction (before the run): agreement to < 1e-3 relative after 1 h at
/// -500 pcm (the longest precursor period at -500 pcm is ~ 1 / 0.0124 s^-1
/// ~ 80 s, so 1 h is ~45 periods).
#[test]
fn neutron_source_is_default_and_sets_the_subcritical_floor() {
    let base = KineticsParameters::htr10_base();
    assert_eq!(base.neutron_source, NeutronSource::htr10_am_be_startup());
    assert_eq!(base.without_neutron_source().neutron_source, NeutronSource::None);
    let NeutronSource::Constant { neutrons_per_second, .. } = NeutronSource::htr10_intrinsic() else { panic!() };
    println!("intrinsic U-238 SF source: {neutrons_per_second:.1} n/s");
    assert!((neutrons_per_second - 775.0).abs() < 10.0);
    let refs = FeedbackTemperatures { fuel: k(900.0), moderator: k(850.0), reflector: k(500.0) };
    let sub = FeedbackTemperatures { moderator: k(850.0 + 500.0 / 16.2), ..refs };
    // Xenon off: with it the held -500 pcm drifts to ~-590 pcm in 1 h (the
    // first run of this test, 2026-11-03, read 0.847 of the expected value
    // for exactly that reason), so the check needs a constant reactivity.
    let mut no_xe = base;
    no_xe.xenon = XenonModel::Off;
    let mut out = Vec::new();
    for params in [no_xe, no_xe.without_neutron_source()] {
        let mut kin = Htr10Kinetics::new(params, Power::new::<watt>(3.315e6), refs).unwrap();
        for _ in 0..3600 {
            kin.step(s(1.0), &sub).unwrap();
        }
        out.push(kin.fission_power().get::<watt>());
    }
    // Expected: fission rate F = w S / (nu |rho|); P = Q_f F. Q_f is the
    // conversion's immediate energy, read back from the initial state:
    // P0 / F0 with F0 the initial fission rate.
    let kin0 = Htr10Kinetics::new(no_xe, Power::new::<watt>(3.315e6), refs).unwrap();
    let rho = 500.0e-5;
    let expected = kin0.fission_power().get::<watt>() * 4.4e7 / (2.43 * rho) / kin0.fission_rate().get::<uom::si::frequency::hertz>();
    println!(
        "after 1 h at -500 pcm: with source {:.6e} W (expected {expected:.6e} W), without {:.3e} W",
        out[0], out[1]
    );
    assert!((out[0] / expected - 1.0).abs() < 1e-3);
    assert!(out[1] < 1e-3 * out[0]);
}


/// The R-Z core in the coupled loop on the Fig 2.5 mesh: with no trip the
/// coupled state holds for 30 s (flow and power to 1e-4), the loop flow
/// follows the measured speed after a trip, and the baffle stops it.
#[test]
fn rz_coupled_initial_state_and_coastdown() {
    let coarse = |o: outram_foam_appbuilder_lib::htr10_rz::TransientCoreOptions| {
        let mut o = o;
        o.solid.max_cell = None;
        o
    };
    let mut case = LofcCase::rz_base().unwrap().with_rz_options(coarse);
    case.loop_config.trip_time = None;
    let mut sim = CoupledTransient::new(&case).unwrap();
    let r0 = sim.initial_response();
    println!(
        "R-Z initial (Fig 2.5 mesh): outlet {:.2} C, RCCS {:.1} kW, fuel {:.1} C, moderator {:.1} C, reflector {:.1} C, head {:.1} Pa",
        r0.outlet_temperature.get::<kelvin>() - 273.15,
        r0.heat_to_rccs.get::<watt>() / 1e3,
        r0.feedback.fuel.get::<kelvin>() - 273.15,
        r0.feedback.moderator.get::<kelvin>() - 273.15,
        r0.feedback.reflector.get::<kelvin>() - 273.15,
        sim.primary_loop().initial_head().get::<uom::si::pressure::pascal>()
    );
    let mut last = None;
    for _ in 0..300 {
        last = Some(sim.step(s(0.1)).unwrap());
    }
    let l = last.unwrap();
    println!("after 30 s: flow {:.6} kg/s, total {:.3} W", l.mass_flow_kg_s, l.total_power_w);
    assert!((l.mass_flow_kg_s / 1.457 - 1.0).abs() < 1e-4);
    assert!((l.total_power_w / 3.315e6 - 1.0).abs() < 1e-3);
    assert!(matches!(sim.core(), CoreModel::RzPorous(_)));
    let case = LofcCase::rz_base().unwrap().with_rz_options(coarse);
    let mut sim = CoupledTransient::new(&case).unwrap();
    for i in 1..=150 {
        let x = sim.step(s(0.1)).unwrap();
        if i % 20 == 0 {
            println!("t {:5.1} s speed {:.4} W/W0 {:.4}", x.time_s, x.speed, x.mass_flow_kg_s / 1.457);
        }
        if x.time_s <= 11.95 {
            assert!((x.mass_flow_kg_s / 1.457 - x.speed).abs() < 0.03);
        }
    }
    assert_eq!(sim.primary_loop().mass_flow().value, 0.0);
}

// ------------------------------------------------------------------ plots --

struct Series {
    points: Vec<(f64, f64)>,
    colour: Rgb,
    label: String,
}

fn line(img: &mut ImageData, (x0, y0): (f64, f64), (x1, y1): (f64, f64), c: Rgb) {
    let n = ((x1 - x0).abs().max((y1 - y0).abs()) as usize).max(1);
    for k in 0..=n {
        let t = k as f64 / n as f64;
        let (x, y) = (x0 + t * (x1 - x0), y0 + t * (y1 - y0));
        if x >= 0.0 && y >= 0.0 && (x as usize) < img.width && (y as usize) < img.height {
            img.set(x as usize, y as usize, c);
        }
    }
}

/// A line plot as a PNG: linear x; linear or log10 y.
fn plot(path: &str, title: &str, xr: (f64, f64), yr: (f64, f64), log_y: bool, xlabel: &str, series: &[Series]) {
    let (w, h, l, b, t) = (900usize, 420usize, 80usize, 40usize, 10usize);
    let mut img = ImageData::filled(w + l + 20, h + b + t, WHITE);
    let fy = |y: f64| if log_y { y.max(1e-300).r_log10() } else { y };
    let (y0, y1) = (fy(yr.0), fy(yr.1));
    let px = |x: f64| l as f64 + (x - xr.0) / (xr.1 - xr.0) * w as f64;
    let py = |y: f64| t as f64 + (1.0 - (fy(y) - y0) / (y1 - y0)) * h as f64;
    for i in 0..=w {
        img.set(l + i, t + h, BLACK);
        img.set(l + i, t, Rgb::new(180, 180, 180));
    }
    for j in 0..=h {
        img.set(l, t + j, BLACK);
    }
    for k in 0..=6 {
        let x = xr.0 + (xr.1 - xr.0) * k as f64 / 6.0;
        let xx = px(x) as usize;
        for j in 0..h {
            if j % 4 == 0 {
                img.set(xx.min(img.width - 1), t + j, Rgb::new(220, 220, 220));
            }
        }
        draw_text(&mut img, xx as i64 - 14, (t + h + 8) as i64, &format!("{x:.0}"), BLACK);
    }
    for k in 0..=5 {
        let yv = y0 + (y1 - y0) * k as f64 / 5.0;
        let yy = t as f64 + (1.0 - k as f64 / 5.0) * h as f64;
        for i in 0..w {
            if i % 4 == 0 {
                img.set(l + i, yy as usize, Rgb::new(220, 220, 220));
            }
        }
        let lab = if log_y {
            format!("1E{yv:.1}")
        } else if (y1 - y0).abs() >= 10.0 {
            format!("{yv:.0}")
        } else {
            format!("{yv:.3}")
        };
        draw_text(&mut img, 2, yy as i64 - 4, &lab, BLACK);
    }
    draw_text(&mut img, (l + w / 2) as i64 - 40, (t + h + 24) as i64, xlabel, BLACK);
    for s in series {
        for p in s.points.windows(2) {
            let (a, c) = (p[0], p[1]);
            if !(a.1.is_finite() && c.1.is_finite()) || a.0 < xr.0 || c.0 > xr.1 {
                continue;
            }
            let ya = py(a.1).clamp(t as f64, (t + h) as f64);
            let yc = py(c.1).clamp(t as f64, (t + h) as f64);
            line(&mut img, (px(a.0), ya), (px(c.0), yc), s.colour);
        }
    }
    let legend: Vec<LegendEntry> = series.iter().map(|s| LegendEntry::new(s.colour, s.label.clone())).collect();
    annotate_image(&img, title, &legend).write_png(path).unwrap();
}

const RED: Rgb = Rgb::new(210, 30, 30);
const BLUE: Rgb = Rgb::new(30, 80, 200);
const GREEN: Rgb = Rgb::new(30, 150, 60);
const GREY: Rgb = Rgb::new(110, 110, 110);
const ORANGE: Rgb = Rgb::new(230, 130, 20);

fn vv_dir() -> String {
    let d = concat!(env!("CARGO_MANIFEST_DIR"), "/verification_and_validation/htr10_rz_transients");
    std::fs::create_dir_all(d).unwrap();
    d.to_string()
}

fn write_trace_csv(path: &str, trace: &[TransientSample]) {
    let mut out = String::from(
        "# gh:#1151 part B model trace (AI draft, unreviewed). Columns: t s, fission and total power W, decay W, reactivity parts pcm, loop flow kg/s, fuel/moderator/reflector C, outlet C, RCCS W, bed upflow kg/s\n\
         t_s,fission_w,total_w,decay_w,rho_fuel_pcm,rho_mod_pcm,rho_refl_pcm,rho_xe_pcm,rho_ext_pcm,rho_total_pcm,flow_kg_s,fuel_c,mod_c,refl_c,outlet_c,rccs_w,bed_upflow_kg_s\n",
    );
    for x in trace.iter().step_by(10) {
        let r = x.reactivity;
        out.push_str(&format!(
            "{:.1},{:.6e},{:.6e},{:.6e},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.5},{:.2},{:.2},{:.2},{:.2},{:.1},{:.5}\n",
            x.time_s,
            x.fission_power_w,
            x.total_power_w,
            x.decay_power_w,
            r.fuel * 1e5,
            r.moderator * 1e5,
            r.reflector * 1e5,
            r.xenon * 1e5,
            r.external * 1e5,
            r.total() * 1e5,
            x.mass_flow_kg_s,
            x.fuel_k - 273.15,
            x.moderator_k - 273.15,
            x.reflector_k - 273.15,
            x.core_outlet_k - 273.15,
            x.heat_to_rccs_w,
            x.bed_upflow_kg_s
        ));
    }
    std::fs::write(path, out).unwrap();
}

fn summarise_lofc(name: &str, trace: &[TransientSample]) -> LofcMetrics {
    let m = lofc_metrics(trace);
    let up = trace.iter().filter(|x| x.time_s >= 1000.0).map(|x| x.bed_upflow_kg_s).fold(0.0, f64::max);
    println!(
        "| {name} | {} | {:.0} pcm @ {:.0} s | {} | {} | {} | {:.0} kW | {:.3} kg/s |",
        m.time_to_one_percent_s.map_or("-".into(), |t| format!("{t:.0} s")),
        m.min_reactivity.0 * 1e5,
        m.min_reactivity.1,
        m.reactivity_zero_crossing_s.map_or("none".into(), |t| format!("{t:.0} s")),
        m.power_return_s.map_or("none".into(), |t| format!("{t:.0} s")),
        m.first_peak.map_or("none".into(), |(v, t)| format!("{:.1} % @ {t:.0} s", 100.0 * v)),
        m.late_total_power_w / 1e3,
        up
    );
    m
}

/// **V&V: LOFC ATWS with the R-Z core, base case (`RZ-BASE`).** Recorded,
/// with one gate fixed in part A before any run and kept: the time to 1 %
/// within 330 +- 120 s. Everything else is compared and reported (README).
/// Writes `rz_lofc_power_*.png`, `rz_lofc_reactivity.png`,
/// `rz_lofc_thermocouples_*.png` and the traces as CSV.
#[cfg(feature = "long-tests")]
#[test]
fn rz_lofc_base_against_the_measured_traces() {
    let started = std::time::Instant::now();
    let case = LofcCase::rz_base().unwrap();
    let probes = c09_thermocouple_positions();
    let (sim, trace, rows) = run_case(&case, &probes, 180.0).unwrap();
    println!("RZ-BASE wall time {:.0} s", started.elapsed().as_secs_f64());
    let r0 = sim.initial_response();
    println!(
        "initial: outlet {:.2} C, RCCS {:.1} kW, fuel {:.1} C, moderator {:.1} C, reflector {:.1} C",
        r0.outlet_temperature.get::<kelvin>() - 273.15,
        r0.heat_to_rccs.get::<watt>() / 1e3,
        r0.feedback.fuel.get::<kelvin>() - 273.15,
        r0.feedback.moderator.get::<kelvin>() - 273.15,
        r0.feedback.reflector.get::<kelvin>() - 273.15
    );
    println!("| case | to 1 % | min rho | rho = 0 | power back > 1 % | first peak | mean P, last 30 min | max bed upflow after 1000 s |");
    let m = summarise_lofc(&case.name, &trace);
    let short = TabulatedSeries::from_csv_columns(POWER_SHORT_C09_FIG4, 0, 1).unwrap();
    let long = TabulatedSeries::from_csv_columns(POWER_LONG_C09_FIG6, 0, 1).unwrap();
    let p0 = trace[0].fission_power_w;
    println!("t (s) | model P/P0 | measured | fuel C | mod C | refl C | Xe pcm | rho pcm | decay kW | RCCS kW | upflow kg/s");
    for t in [10.0, 20.0, 50.0, 100.0, 200.0, 300.0, 400.0, 1000.0, 2000.0, 3000.0, 4000.0, 4400.0, 5000.0, 6000.0, 7200.0, 9000.0, 10_800.0] {
        let x = trace.iter().find(|x| x.time_s >= t - 1e-9).unwrap_or(trace.last().unwrap());
        let meas = if t <= 500.0 { short.value_at(t) } else { long.value_at(t) };
        println!(
            "{t:6.0} | {:.4e} | {:.4} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.4}",
            x.fission_power_w / p0,
            meas,
            x.fuel_k - 273.15,
            x.moderator_k - 273.15,
            x.reflector_k - 273.15,
            x.reactivity.xenon * 1e5,
            x.reactivity.total() * 1e5,
            x.decay_power_w / 1e3,
            x.heat_to_rccs_w / 1e3,
            x.bed_upflow_kg_s
        );
    }
    // Thermocouples: model against measured at every measured bin.
    let traces = lofc_thermocouple_traces().unwrap();
    println!("| TC | r, z cm | measured t0 / 3 h C | model t0 / 3 h C | residual t0 / 3 h K | RMS over 3 h K | max |res| K |");
    let mut all = Vec::new();
    for (k, p) in probes.iter().enumerate() {
        let meas = traces.iter().find(|t| t.label == p.location).unwrap();
        let model_at = |t: f64| -> f64 {
            let i = rows.iter().position(|r| r.0 >= t - 1e-6).unwrap_or(rows.len() - 1);
            if i == 0 {
                return rows[0].1[k];
            }
            let (a, b) = (&rows[i - 1], &rows[i]);
            a.1[k] + (b.1[k] - a.1[k]) * (t - a.0) / (b.0 - a.0)
        };
        let res: Vec<f64> = meas.rows.iter().map(|(t, v, _)| model_at(*t) - v).collect();
        let rms = (res.iter().map(|r| r * r).sum::<f64>() / res.len() as f64).sqrt();
        let mx = res.iter().fold(0.0_f64, |a, r| a.max(r.abs()));
        all.extend(res.iter().copied());
        let (m0, m3) = (meas.value_at(0.0), meas.value_at(10_800.0));
        let (o0, o3) = (rows[0].1[k], model_at(10_800.0));
        println!(
            "| {} | {:.0}, {:.0} | {m0:.1} / {m3:.1} | {o0:.1} / {o3:.1} | {:+.1} / {:+.1} | {rms:.1} | {mx:.1} |",
            p.location,
            p.r_cm,
            p.z_cm,
            o0 - m0,
            o3 - m3
        );
        let dir = vv_dir();
        let _ = dir;
    }
    let rms_all = (all.iter().map(|r| r * r).sum::<f64>() / all.len() as f64).sqrt();
    println!("all 17 TCs, every bin: RMS {rms_all:.1} K, mean {:+.1} K", all.iter().sum::<f64>() / all.len() as f64);

    // Plots and CSVs.
    let dir = vv_dir();
    write_trace_csv(&format!("{dir}/rz_lofc_base_trace.csv"), &trace);
    let mut csv = String::from("# gh:#1151 part B: R-Z model at the C09 thermocouple positions (deg C), RZ-BASE\nt_s");
    for p in &probes {
        csv.push_str(&format!(",{}", p.location));
    }
    csv.push('\n');
    for (t, v) in &rows {
        csv.push_str(&format!("{t:.0}"));
        for x in v {
            csv.push_str(&format!(",{x:.2}"));
        }
        csv.push('\n');
    }
    std::fs::write(format!("{dir}/rz_lofc_base_thermocouples.csv"), csv).unwrap();
    let model: Vec<(f64, f64)> = trace.iter().map(|x| (x.time_s, x.fission_power_w / p0)).collect();
    plot(
        &format!("{dir}/rz_lofc_power_short.png"),
        "LOFC ATWS, RELATIVE FISSION POWER, 0-500 S: R-Z MODEL VS MEASURED (C09 FIG 4)",
        (0.0, 500.0),
        (0.0, 1.05),
        false,
        "TIME [S]",
        &[
            Series { points: short.points().to_vec(), colour: RED, label: "MEASURED (C09 FIG 4, DIGITISED)".into() },
            Series { points: model.clone(), colour: BLUE, label: "R-Z MODEL, RZ-BASE".into() },
        ],
    );
    plot(
        &format!("{dir}/rz_lofc_power_long.png"),
        "LOFC ATWS, RELATIVE FISSION POWER, 0-3 H: R-Z MODEL VS MEASURED (C09 FIG 6)",
        (0.0, 10_800.0),
        (0.0, 0.4),
        false,
        "TIME [S]",
        &[
            Series { points: long.points().to_vec(), colour: RED, label: "MEASURED (C09 FIG 6, DIGITISED)".into() },
            Series { points: model.clone(), colour: BLUE, label: "R-Z MODEL, RZ-BASE".into() },
        ],
    );
    plot(
        &format!("{dir}/rz_lofc_power_log.png"),
        "LOFC ATWS, RELATIVE FISSION POWER (LOG): THE SOURCE FLOOR",
        (0.0, 10_800.0),
        (1e-9, 2.0),
        true,
        "TIME [S]",
        &[
            Series { points: long.points().iter().map(|p| (p.0, p.1.max(1e-9))).collect(), colour: RED, label: "MEASURED".into() },
            Series { points: model, colour: BLUE, label: "R-Z MODEL".into() },
        ],
    );
    let rho = |f: fn(&ReactivityBreakdown) -> f64| -> Vec<(f64, f64)> {
        trace.iter().map(|x| (x.time_s, 1e5 * f(&x.reactivity))).collect()
    };
    plot(
        &format!("{dir}/rz_lofc_reactivity.png"),
        "LOFC ATWS, R-Z MODEL REACTIVITY [PCM] BY PART",
        (0.0, 10_800.0),
        (-900.0, 300.0),
        false,
        "TIME [S]",
        &[
            Series { points: rho(|r| r.total()), colour: BLACK, label: "TOTAL".into() },
            Series { points: rho(|r| r.fuel), colour: RED, label: "FUEL".into() },
            Series { points: rho(|r| r.moderator), colour: BLUE, label: "MODERATOR".into() },
            Series { points: rho(|r| r.reflector), colour: GREEN, label: "REFLECTOR".into() },
            Series { points: rho(|r| r.xenon), colour: ORANGE, label: "XENON".into() },
        ],
    );
    for (file, labels, lo, hi) in [
        ("rz_lofc_thermocouples_top_bottom.png", vec!["TR1_TR4", "TR2_TR5", "TR3_TR6", "BR1_BR3", "BR2_BR4"], 150.0, 800.0),
        ("rz_lofc_thermocouples_side_upper.png", vec!["SR1", "SR2", "SR3", "SR4", "SR5", "SR6"], 200.0, 450.0),
        ("rz_lofc_thermocouples_side_lower.png", vec!["SR7", "SR8", "SR9", "SR10", "SR11", "SR12"], 200.0, 550.0),
    ] {
        let colours = [RED, BLUE, GREEN, ORANGE, GREY, BLACK];
        let mut series = Vec::new();
        for (i, lab) in labels.iter().enumerate() {
            let kk = probes.iter().position(|p| p.location == *lab).unwrap();
            let meas = traces.iter().find(|t| t.label == *lab).unwrap();
            series.push(Series {
                points: meas.rows.iter().map(|(t, v, _)| (*t, *v)).collect(),
                colour: colours[i],
                label: format!("{lab} MEASURED"),
            });
            series.push(Series {
                points: rows.iter().map(|(t, v)| (*t, v[kk])).collect(),
                colour: Rgb::new(colours[i].r / 2 + 100, colours[i].g / 2 + 100, colours[i].b / 2 + 100),
                label: format!("{lab} R-Z MODEL (PALE)"),
            });
        }
        plot(&format!("{dir}/{file}"), "LOFC ATWS THERMOCOUPLES [C]: MEASURED (C09 FIGS 8-11) VS R-Z MODEL (PALE)", (0.0, 10_800.0), (lo, hi), false, "TIME [S]", &series);
    }
    let t1 = m.time_to_one_percent_s.expect("power reaches 1 %");
    assert!((t1 - 330.0).abs() <= 120.0, "time to 1 % = {t1} s");
}

/// **V&V: the R-Z LOFC sensitivity and ablation cases**, one change from
/// `RZ-BASE` each (input sheet Sec 9.2 plus the part B ablations and the
/// numerical checks). Recorded, not gated.
#[cfg(feature = "long-tests")]
#[test]
fn rz_lofc_sensitivity_cases() {
    use outram_foam_appbuilder_lib::htgr_closures::{BedConductivityModel, ZbsBed};
    use outram_foam_appbuilder_lib::htr10_rz::ReflectorAverage;
    let base = LofcCase::rz_base().unwrap();
    let mut cases: Vec<LofcCase> = Vec::new();
    cases.push(base.clone().with_rz_options(|o| o.without_buoyancy()).named("NC-OFF"));
    let mut c = base.clone().named("SRC-OFF");
    c.kinetics = c.kinetics.without_neutron_source();
    cases.push(c);
    let mut c = base.clone().named("SRC-INTRINSIC");
    c.kinetics.neutron_source = NeutronSource::htr10_intrinsic();
    cases.push(c);
    cases.push(base.clone().with_rz_options(|o| o.with_quasi_steady_fuel()).named("FUEL-QS"));
    cases.push(
        base.clone()
            .with_rz_options(|mut o| {
                o.reflector = ReflectorAverage::SideOnly;
                o
            })
            .named("REFL-SIDE"),
    );
    cases.push(
        base.clone()
            .with_rz_options(|mut o| {
                o.solid.materials.bed = BedConductivityModel::Zbs(ZbsBed::htr10());
                o
            })
            .named("BED-ZBS"),
    );
    for (name, b) in [("BYP-G02", tampines::htr10_primary_loop::BypassSplit::g02()), ("BYP-NOLEAK", tampines::htr10_primary_loop::BypassSplit::no_leakage())] {
        let mut c = base.clone().named(name);
        c.bypass = b;
        cases.push(c);
    }
    cases.push(base.clone().named("TR-SPEC").with_initial(InitialState::spec()));
    let mut c = base.clone().named("KIN-ALPHA-G02-EQ");
    c.kinetics.coefficients = FeedbackCoefficients::g02_equilibrium_core();
    cases.push(c);
    let mut c = base.clone().named("XE-OFF");
    c.kinetics.xenon = XenonModel::Off;
    cases.push(c);
    cases.push(
        base.clone()
            .with_rz_time_step(RzCoreTimeStep { early: s(0.5), late: s(2.5), switch_at: s(1000.0) })
            .named("DT-HALF"),
    );
    println!("| case | to 1 % | min rho | rho = 0 | power back > 1 % | first peak | mean P, last 30 min | max bed upflow after 1000 s |");
    println!("|---|---|---|---|---|---|---|---|");
    for case in &cases {
        let started = std::time::Instant::now();
        match run_lofc(case) {
            Ok((_, trace)) => {
                summarise_lofc(&case.name, &trace);
                if case.name == "NC-OFF" || case.name == "SRC-OFF" {
                    write_trace_csv(&format!("{}/rz_lofc_{}_trace.csv", vv_dir(), case.name.to_lowercase()), &trace);
                }
            }
            Err(e) => println!("| {} | FAILED: {e} |", case.name),
        }
        println!("  ({} wall {:.0} s)", case.name, started.elapsed().as_secs_f64());
    }
}

/// **V&V: the short-term power and the CRW first peak (gh:#1174).** The
/// kinetics-input audit cases and the feedback-temperature cases, on the
/// LOFC and the CRW, one change each from the base (the two-temperature
/// bed and the all-graphite moderator reading are the defaults since
/// gh:#1174):
///
/// - `DUMMY-IN-CELL`: no dummy-pebble node (all-graphite moderator kept);
/// - `MOD-OUTSIDE-FZ`: the part B moderator reading (dummy node kept): the
///   dummy-lag hypothesis of README Sec 3.4 as stated;
/// - `PARTB`: both, i.e. the gh:#1151 part B core (with gh:#1173's cavity
///   convection);
/// - `KIN-LAMBDA-H06`, `KIN-DN-TEHO`, `KIN-DN-1G`: the kinetics inputs.
///
/// Prints the relative fission power at 20, 50, 100 and 200 s (measured
/// LOFC 0.905 / 0.496 / 0.204 / 0.047, [C09] Fig 6 digitised) and the CRW
/// first peak (216 % at 35 s [H06] / 241 % at 30 s [J09]). Predictions
/// (written and committed before this ran) and results: README Sec 8.
#[cfg(feature = "long-tests")]
#[test]
fn rz_short_term_feedback_cases() {
    type Edit = fn(LofcCase) -> LofcCase;
    let edits: [(&str, Edit); 7] = [
        ("BASE", |c| c),
        ("DUMMY-IN-CELL", |c| c.with_rz_options(|o| o.without_dummy_pebble_lag())),
        ("MOD-OUTSIDE-FZ", |c| c.with_rz_options(|o| o.with_moderator_outside_fuel_zones())),
        ("PARTB", |c| c.with_rz_options(|o| o.without_dummy_pebble_lag().with_moderator_outside_fuel_zones())),
        ("KIN-LAMBDA-H06", |mut c| {
            c.kinetics.generation_time = Time::new::<second>(1.68e-4);
            c
        }),
        ("KIN-DN-TEHO", |mut c| {
            c.kinetics.delayed_set = DelayedNeutronSet::TehOPrkeU235;
            c
        }),
        ("KIN-DN-1G", |mut c| {
            c.kinetics.delayed_set = DelayedNeutronSet::OneGroupKeepin;
            c
        }),
    ];
    println!("| case | to 1 % | min rho | rho = 0 | power back > 1 % | first peak | mean P, last 30 min | max bed upflow after 1000 s |");
    for (test, base) in [("LOFC", LofcCase::rz_base().unwrap()), ("CRW", LofcCase::crw_base().unwrap())] {
        for (name, edit) in edits {
            let case = edit(base.clone()).named(&format!("{test}-{name}"));
            let started = std::time::Instant::now();
            let trace = match run_lofc(&case) {
                Ok((_, t)) => t,
                Err(e) => {
                    println!("| {} | FAILED: {e} |", case.name);
                    continue;
                }
            };
            summarise_lofc(&case.name, &trace);
            let p0 = trace[0].fission_power_w;
            let rel = |t: f64| trace.iter().find(|x| x.time_s >= t - 1e-6).map_or(f64::NAN, |x| x.fission_power_w / p0);
            let x100 = trace.iter().find(|x| x.time_s >= 100.0 - 1e-6).unwrap();
            println!(
                "  {}: P/P0 at 20 / 50 / 100 / 200 s = {:.4} / {:.4} / {:.4} / {:.4}; at 100 s rho fuel {:+.1}, moderator {:+.1}, reflector {:+.1} pcm, fuel {:.2} C, moderator {:.2} C",
                case.name,
                rel(20.0),
                rel(50.0),
                rel(100.0),
                rel(200.0),
                x100.reactivity.fuel * 1e5,
                x100.reactivity.moderator * 1e5,
                x100.reactivity.reflector * 1e5,
                x100.fuel_k - 273.15,
                x100.moderator_k - 273.15
            );
            if test == "CRW" {
                let m = crw_metrics(&trace);
                println!(
                    "  {} CRW: first peak {:.1} % @ {:.0} s; P/P0 at 30 / 35 s {:.3} / {:.3}; decay level {}; re-criticality {}",
                    case.name,
                    100.0 * m.first_peak.0,
                    m.first_peak.1,
                    rel(30.0),
                    rel(35.0),
                    m.decay_level_s.map_or("none".into(), |t| format!("{t:.0} s")),
                    m.recriticality_s.map_or("none".into(), |t| format!("{t:.0} s"))
                );
            }
            println!("  ({} wall {:.0} s)", case.name, started.elapsed().as_secs_f64());
        }
    }
}

/// Probes on the RPV side wall (`r = 216 cm`, the outer half of the 8 cm
/// wall) at three heights, for the cavity-convection record (gh:#1173).
fn rpv_wall_probes() -> Vec<outram_foam_appbuilder_lib::htr10_rz::Thermocouple> {
    let template = c09_thermocouple_positions().into_iter().next().unwrap();
    [("RPV_z-200", -200.0), ("RPV_z90", 90.0), ("RPV_z400", 400.0)]
        .into_iter()
        .map(|(name, z)| {
            let mut t = template.clone();
            t.location = name.into();
            t.r_cm = 216.0;
            t.z_cm = z;
            t
        })
        .collect()
}

/// **V&V: RPV-cavity natural convection (gh:#1173).** LOFC and CRW with
/// the convection (the default since gh:#1173) and the ablation
/// `CAVITY-NC-OFF` (radiation only, every record before it), on the 20 cm
/// base mesh and on 10 cm. Prints the usual metrics, the relative fission
/// power at 100 s, the heat to the RCCS and heads at 1, 2 and 3 h, and the
/// RPV wall at three heights. Predictions (written and committed before
/// this ran) and results: README Sec 7.
#[cfg(feature = "long-tests")]
#[test]
fn rz_cavity_convection_cases() {
    let probes = rpv_wall_probes();
    let lofc = LofcCase::rz_base().unwrap();
    let crw = LofcCase::crw_base().unwrap();
    let off = |c: LofcCase, n: &str| c.with_rz_options(|mut o| { o.solid = o.solid.without_cavity_convection(); o }).named(n);
    let ten = |c: LofcCase, n: &str| c.with_rz_options(|o| o.with_max_cell(Length::new::<centimeter>(10.0))).named(n);
    let cases = vec![
        (lofc.clone().named("RZ-BASE"), false),
        (off(lofc.clone(), "CAVITY-NC-OFF"), false),
        (ten(lofc.clone(), "RZ-BASE-10CM"), false),
        (off(ten(lofc, "x"), "CAVITY-NC-OFF-10CM"), false),
        (crw.clone().named("CRW-BASE"), true),
        (off(crw, "CRW-CAVITY-NC-OFF"), true),
    ];
    println!("| case | to 1 % | min rho | rho = 0 | power back > 1 % | first peak | mean P, last 30 min | max bed upflow after 1000 s |");
    for (case, is_crw) in &cases {
        let started = std::time::Instant::now();
        let (_, trace, rows) = match run_case(case, &probes, 600.0) {
            Ok(x) => x,
            Err(e) => {
                println!("| {} | FAILED: {e} |", case.name);
                continue;
            }
        };
        summarise_lofc(&case.name, &trace);
        let at = |t: f64| trace.iter().find(|x| x.time_s >= t - 1e-6).unwrap();
        let p0 = trace[0].fission_power_w;
        if *is_crw {
            let m = crw_metrics(&trace);
            println!(
                "  {} CRW: first peak {:.1} % @ {:.0} s, decay level {}, re-criticality {}, P(7200 s) {:.0} kW",
                case.name,
                100.0 * m.first_peak.0,
                m.first_peak.1,
                m.decay_level_s.map_or("none".into(), |t| format!("{t:.0} s")),
                m.recriticality_s.map_or("none".into(), |t| format!("{t:.0} s")),
                m.power_at_7200_w / 1e3
            );
        }
        println!(
            "  {}: P(100 s)/P0 {:.4}; RCCS+heads at 0 / 1 / 2 / 3 h: {:.1} / {:.1} / {:.1} / {:.1} kW",
            case.name,
            at(100.0).fission_power_w / p0,
            trace[0].heat_to_rccs_w / 1e3,
            at(3600.0).heat_to_rccs_w / 1e3,
            at(7200.0).heat_to_rccs_w / 1e3,
            at(10_800.0).heat_to_rccs_w / 1e3
        );
        for t in [1800.0, 3600.0, 5400.0, 7200.0] {
            let x = at(t);
            let r = &x.reactivity;
            println!(
                "  {} at {t:.0} s: rho fuel {:+.1} / moderator {:+.1} / reflector {:+.1} / Xe {:+.1} / total {:+.1} pcm; fuel {:.2} C, moderator {:.2} C, reflector {:.2} C",
                case.name,
                r.fuel * 1e5,
                r.moderator * 1e5,
                r.reflector * 1e5,
                r.xenon * 1e5,
                r.total() * 1e5,
                x.fuel_k - 273.15,
                x.moderator_k - 273.15,
                x.reflector_k - 273.15
            );
        }
        for (t, v) in rows.iter().filter(|(t, _)| [0.0, 3600.0, 7200.0, 10_800.0].iter().any(|x| (t - x).abs() < 1e-6)) {
            println!("  {} RPV wall at {t:.0} s (z -200 / 90 / 400 cm): {:.1} / {:.1} / {:.1} degC", case.name, v[0], v[1], v[2]);
        }
        println!("  ({} wall {:.0} s)", case.name, started.elapsed().as_secs_f64());
    }
}

/// **Mesh study** of `RZ-BASE` (20 cm): 30, 20, 15 and 10 cm cells, the
/// metrics per mesh and the cell counts (the README's mesh table and its
/// convergence statement). ~~The 10 cm mesh is in
/// [`rz_lofc_sensitivity_cases`]; its gas-flow Picard stalls at a relative
/// change ~7e-7 against the 1e-7 tolerance, so the check brackets 20 cm
/// from both sides with the meshes that run.~~ **CORRECTED 2026-11-03
/// (gh:#1175):** the stall was the flux measure's round-off floor; with
/// the Picard measured on the resistances the 10 cm mesh runs, and is here.
#[cfg(feature = "long-tests")]
#[test]
fn rz_lofc_mesh_check() {
    println!("| case | to 1 % | min rho | rho = 0 | power back > 1 % | first peak | mean P, last 30 min | max bed upflow after 1000 s |");
    for cm in [30.0, 20.0, 15.0, 10.0] {
        let case = LofcCase::rz_base()
            .unwrap()
            .with_rz_options(|o| o.with_max_cell(Length::new::<centimeter>(cm)))
            .named(&format!("MESH-{cm:.0}CM"));
        let started = std::time::Instant::now();
        match run_lofc(&case) {
            Ok((sim, trace)) => {
                if let CoreModel::RzPorous(c) = sim.core() {
                    println!("  ({} cells)", c.core().solid().field().mesh().n_cells());
                }
                summarise_lofc(&case.name, &trace);
            }
            Err(e) => println!("| {} | FAILED: {e} |", case.name),
        }
        println!("  ({} wall {:.0} s)", case.name, started.elapsed().as_secs_f64());
    }
}

fn summarise_crw(name: &str, trace: &[TransientSample], tr: &[(f64, f64)]) -> CrwMetrics {
    let m = crw_metrics(trace);
    let rise = |t: f64| tr.iter().find(|p| p.0 >= t - 1e-6).map_or(f64::NAN, |p| p.1) - tr[0].1;
    println!(
        "| {name} | {:.1} % @ {:.0} s ({:.0} kW total) | {} | {:.0} pcm @ {:.0} s | {} | {} | {:.0} kW | {:.0} kW | {:+.0} K |",
        100.0 * m.first_peak.0,
        m.first_peak.1,
        m.first_peak_total_w / 1e3,
        m.decay_level_s.map_or("none".into(), |t| format!("{t:.0} s")),
        m.min_reactivity.0 * 1e5,
        m.min_reactivity.1,
        m.recriticality_s.map_or("none".into(), |t| format!("{t:.0} s")),
        m.recriticality_peak.map_or("none".into(), |(v, t)| format!("{:.1} % @ {t:.0} s", 100.0 * v)),
        m.power_at_7200_w / 1e3,
        m.late_total_power_w / 1e3,
        rise(7200.0)
    );
    m
}

/// **V&V: CRW ATWS with the R-Z core** (input sheet Sec 8): `CRW-BASE`
/// (as run, trip at 12 s), `CRW-TRIP9` (spec), and the `SRC-OFF` and
/// `NC-OFF` ablations. The first peak is reported against BOTH published
/// normalisations (N7: 216 % [H06] / 241 % [J09]); the top-reflector rise
/// at the TR1&TR4 position against [H06]'s +215 K. Writes
/// `rz_crw_power_*.png` and the trace CSV.
#[cfg(feature = "long-tests")]
#[test]
fn rz_crw_cases() {
    let probes: Vec<_> = c09_thermocouple_positions().into_iter().filter(|p| p.location == "TR1_TR4").collect();
    let mut cases = vec![LofcCase::crw_base().unwrap(), LofcCase::crw_trip9().unwrap()];
    let mut c = LofcCase::crw_base().unwrap().named("CRW-SRC-OFF");
    c.kinetics = c.kinetics.without_neutron_source();
    cases.push(c);
    cases.push(LofcCase::crw_base().unwrap().with_rz_options(|o| o.without_buoyancy()).named("CRW-NC-OFF"));
    println!("| case | first peak | to decay level | min rho | re-criticality | re-criticality peak | P at 7200 s | mean P, last 30 min | TR1&TR4 rise at 2 h |");
    println!("|---|---|---|---|---|---|---|---|---|");
    let dir = vv_dir();
    for case in &cases {
        let started = std::time::Instant::now();
        let (_, trace, rows) = match run_case(case, &probes, 60.0) {
            Ok(x) => x,
            Err(e) => {
                println!("| {} | FAILED: {e} |", case.name);
                continue;
            }
        };
        let tr: Vec<(f64, f64)> = rows.iter().map(|(t, v)| (*t, v[0])).collect();
        let m = summarise_crw(&case.name, &trace, &tr);
        println!("  ({} wall {:.0} s)", case.name, started.elapsed().as_secs_f64());
        if case.name == "CRW-BASE" {
            let p0 = trace[0].fission_power_w;
            println!(
                "CRW-BASE first peak: {:.1} % of the model's 3315 kW; as a fraction of 3000 kW ([H06]'s stated initial power) {:.1} %; total {:.0} kW against [H06]'s 7230 kW",
                100.0 * m.first_peak.0,
                100.0 * m.first_peak.0 * 3315.0 / 3000.0,
                m.first_peak_total_w / 1e3
            );
            for t in [5.0, 10.0, 12.0, 20.0, 30.0, 40.0, 60.0, 100.0, 200.0, 500.0, 1000.0] {
                let x = trace.iter().find(|x| x.time_s >= t - 1e-9).unwrap();
                println!(
                    "t {t:6.0} s: P/P0 {:.4}, rho ext {:.1} pcm, total {:.1} pcm, fuel {:.1} C, mod {:.1} C, flow {:.3} kg/s",
                    x.fission_power_w / p0,
                    x.reactivity.external * 1e5,
                    x.reactivity.total() * 1e5,
                    x.fuel_k - 273.15,
                    x.moderator_k - 273.15,
                    x.mass_flow_kg_s
                );
            }
            write_trace_csv(&format!("{dir}/rz_crw_base_trace.csv"), &trace);
            let model: Vec<(f64, f64)> = trace.iter().map(|x| (x.time_s, x.fission_power_w / p0)).collect();
            plot(
                &format!("{dir}/rz_crw_power_short.png"),
                "CRW ATWS, R-Z MODEL RELATIVE FISSION POWER, 0-300 S (MEASURED PEAK: 216 % AT 35 S H06 / 241 % AT 30 S J09)",
                (0.0, 300.0),
                (0.0, 2.6),
                false,
                "TIME [S]",
                &[
                    Series { points: model.clone(), colour: BLUE, label: "R-Z MODEL, CRW-BASE".into() },
                    Series { points: vec![(35.0, 2.16), (35.5, 0.0)], colour: RED, label: "H06 PEAK 216 % AT 35 S".into() },
                    Series { points: vec![(30.0, 2.41), (30.5, 0.0)], colour: ORANGE, label: "J09 PEAK 241 % AT 30 S".into() },
                ],
            );
            plot(
                &format!("{dir}/rz_crw_power_long.png"),
                "CRW ATWS, R-Z MODEL RELATIVE FISSION POWER, 0-3 H (MEASURED: RE-CRITICALITY 2400-3400 S, PEAK ~27 %)",
                (0.0, 10_800.0),
                (0.0, 0.5),
                false,
                "TIME [S]",
                &[
                    Series { points: model, colour: BLUE, label: "R-Z MODEL, CRW-BASE".into() },
                    Series { points: vec![(3420.0, 0.27), (3421.0, 0.0)], colour: ORANGE, label: "J09 RE-CRITICALITY PEAK 27 % AT 3420 S".into() },
                    Series { points: vec![(2400.0, 0.4), (2401.0, 0.0)], colour: RED, label: "H06 RE-CRITICALITY 2400 S".into() },
                ],
            );
        }
    }
}

/// Observed order and Richardson extrapolation from three meshes refined
/// by a constant ratio `r` (coarse `f1`, medium `f2`, fine `f3`; Roache
/// 1998, the generalised Richardson form): `p = ln((f1 - f2)/(f2 - f3)) /
/// ln r`, `f_ext = f3 + (f3 - f2)/(r^p - 1)`. `None` when the differences
/// change sign (oscillatory: no order exists) or vanish.
fn richardson(f1: f64, f2: f64, f3: f64, r: f64) -> Option<(f64, f64)> {
    let (d12, d23) = (f1 - f2, f2 - f3);
    if d12 == 0.0 || d23 == 0.0 || d12.signum() != d23.signum() {
        return None;
    }
    let p = (d12 / d23).r_ln() / r.r_ln();
    Some((p, f3 + (f3 - f2) / (r.r_powf(p) - 1.0)))
}

/// The Richardson helper on manufactured sequences: `f = 1 + h^2` gives
/// order 2 and the limit 1 exactly, `f = 1 + h` order 1; an oscillating
/// sequence is refused.
#[test]
fn richardson_recovers_a_known_order() {
    let f = |h: f64| 1.0 + h * h;
    let (p, e) = richardson(f(0.4), f(0.2), f(0.1), 2.0).unwrap();
    assert!((p - 2.0).abs() < 1e-12 && (e - 1.0).abs() < 1e-12, "{p} {e}");
    let (p, e) = richardson(1.4, 1.2, 1.1, 2.0).unwrap();
    assert!((p - 1.0).abs() < 1e-12 && (e - 1.0).abs() < 1e-12, "{p} {e}");
    assert!(richardson(1.0, 2.0, 1.5, 2.0).is_none());
}

/// One mesh-study row (gh:#1176): the LOFC metrics plus the quantities
/// that say where a mesh change acts.
#[cfg(feature = "long-tests")]
struct MeshRow {
    cells: usize,
    recrit_s: f64,
    late_kw: f64,
    rccs_3h_kw: f64,
    min_pcm: f64,
    mod_c: [f64; 4],
    p100: f64,
}

/// Run `case` and print its mesh-study row: cells, the LOFC metrics,
/// P(100 s), the heat to the RCCS and heads at 0 and 3 h, the initial
/// fuel / moderator / reflector temperatures and the moderator reading at
/// 1800, 3600 and 7200 s, the largest bed upflow after 1000 s, and the wall
/// time.
#[cfg(feature = "long-tests")]
fn mesh_row(case: &LofcCase) -> Option<MeshRow> {
    let started = std::time::Instant::now();
    let (sim, trace) = match run_lofc(case) {
        Ok(x) => x,
        Err(e) => {
            println!("| {} | FAILED: {e} | ({:.0} s) |", case.name, started.elapsed().as_secs_f64());
            return None;
        }
    };
    let cells = match sim.core() {
        CoreModel::RzPorous(c) => c.core().solid().field().mesh().n_cells(),
        _ => 0,
    };
    let m = summarise_lofc(&case.name, &trace);
    let last = *trace.last().unwrap();
    let at = |t: f64| *trace.iter().find(|x| x.time_s >= t - 1e-6).unwrap_or(&last);
    let p0 = trace[0].fission_power_w;
    let x0 = trace[0];
    let mod_c = [0.0, 1800.0, 3600.0, 7200.0].map(|t| at(t).moderator_k - 273.15);
    let up = trace.iter().filter(|x| x.time_s >= 1000.0).map(|x| x.bed_upflow_kg_s).fold(0.0, f64::max);
    let row = MeshRow {
        cells,
        recrit_s: m.reactivity_zero_crossing_s.unwrap_or(f64::NAN),
        late_kw: m.late_total_power_w / 1e3,
        rccs_3h_kw: at(10_800.0).heat_to_rccs_w / 1e3,
        min_pcm: m.min_reactivity.0 * 1e5,
        mod_c,
        p100: at(100.0).fission_power_w / p0,
    };
    println!(
        "  {}: {} cells; P(100 s) {:.4}; RCCS+heads 0 / 3 h {:.2} / {:.2} kW; initial fuel / moderator / reflector {:.2} / {:.2} / {:.2} C; moderator at 1800 / 3600 / 7200 s {:.2} / {:.2} / {:.2} C; max bed upflow after 1000 s {:.4} kg/s; wall {:.0} s",
        case.name,
        row.cells,
        row.p100,
        x0.heat_to_rccs_w / 1e3,
        row.rccs_3h_kw,
        x0.fuel_k - 273.15,
        x0.moderator_k - 273.15,
        x0.reflector_k - 273.15,
        mod_c[1],
        mod_c[2],
        mod_c[3],
        up,
        started.elapsed().as_secs_f64()
    );
    Some(row)
}

/// **V&V: geometric mesh study of `RZ-BASE` (gh:#1176).** The 20 cm mesh
/// (`REF-1X`, 1452 cells) with every cell split 2 x 2 (`REF-2X`) and
/// 4 x 4 (`REF-4X`): ratio 2, every region boundary on a face (unlike the
/// `max_cell` sequence of [`rz_lofc_mesh_check`], which refines different
/// Fig 2.5 intervals by different ratios). Prints each row, then the
/// observed order and the Richardson-extrapolated value of the
/// re-criticality, the mean power of the last 30 min, the heat to the RCCS
/// and heads at 3 h, the minimum reactivity, the moderator reading at
/// 3600 s and P(100 s). Predictions (committed before this ran) and
/// results: README Sec 10.
#[cfg(feature = "long-tests")]
#[test]
fn rz_lofc_geometric_mesh_study() {
    println!("| case | to 1 % | min rho | rho = 0 | power back > 1 % | first peak | mean P, last 30 min | max bed upflow after 1000 s |");
    let base = LofcCase::rz_base().unwrap();
    let rows: Vec<Option<MeshRow>> = [1, 2, 4]
        .into_iter()
        .map(|f| mesh_row(&base.clone().with_rz_options(|o| o.with_refinement(f, f)).named(&format!("REF-{f}X"))))
        .collect();
    if let [Some(a), Some(b), Some(c)] = &rows[..] {
        type Get = fn(&MeshRow) -> f64;
        let metrics: [(&str, Get); 6] = [
            ("re-criticality s", |r| r.recrit_s),
            ("mean P last 30 min kW", |r| r.late_kw),
            ("RCCS+heads at 3 h kW", |r| r.rccs_3h_kw),
            ("min rho pcm", |r| r.min_pcm),
            ("moderator at 3600 s C", |r| r.mod_c[2]),
            ("P(100 s)/P0", |r| r.p100),
        ];
        println!("cells {} / {} / {}", a.cells, b.cells, c.cells);
        for (name, get) in metrics {
            let (f1, f2, f3) = (get(a), get(b), get(c));
            match richardson(f1, f2, f3, 2.0) {
                Some((p, e)) => println!("{name}: {f1:.4} / {f2:.4} / {f3:.4}; observed order {p:.2}; Richardson {e:.4}"),
                None => println!("{name}: {f1:.4} / {f2:.4} / {f3:.4}; not monotone: no order"),
            }
        }
    }
}

/// **Diagnostics of the mesh dependence (gh:#1176):** which part of the
/// model carries it. Against `REF-1X` / `REF-2X` of
/// [`rz_lofc_geometric_mesh_study`]: `NC-OFF` at 1x and 2x (if the change
/// vanishes without buoyancy, the gas circulation carries it); radial-only
/// (`REF-R2`) and axial-only (`REF-Z2`) halving. Predictions (committed
/// before this ran) and results: README Sec 10.
#[cfg(feature = "long-tests")]
#[test]
fn rz_lofc_mesh_diagnostics() {
    println!("| case | to 1 % | min rho | rho = 0 | power back > 1 % | first peak | mean P, last 30 min | max bed upflow after 1000 s |");
    let base = LofcCase::rz_base().unwrap();
    let cases = [
        base.clone().with_rz_options(|o| o.without_buoyancy()).named("NC-OFF-1X"),
        base.clone().with_rz_options(|o| o.without_buoyancy().with_refinement(2, 2)).named("NC-OFF-2X"),
        base.clone().with_rz_options(|o| o.with_refinement(2, 1)).named("REF-R2"),
        base.clone().with_rz_options(|o| o.with_refinement(1, 2)).named("REF-Z2"),
    ];
    for c in &cases {
        mesh_row(c);
    }
}
