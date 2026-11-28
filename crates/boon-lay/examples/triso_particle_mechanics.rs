// SPDX-License-Identifier: AGPL-3.0-only
//! Draw the ASSEMBLED radial mesh of boon-lay particle mechanics, and the
//! stress profile of one illustrative run, as SVG for a human to check
//! (boon-lay `AGENTS.md`: reactor geometry is drawn before it is trusted).
//!
//! ```text
//! cargo run --release -p boon-lay --example triso_particle_mechanics -- <out_dir>
//! ```
//!
//! Writes `<out_dir>/particle_mesh.svg` (the five-layer conduction mesh and
//! the three-layer mechanics mesh of the verification tests, coloured from
//! `ParticleMesh::element_layer`, i.e. from what the solver integrates over,
//! with every element boundary drawn) and `<out_dir>/pyc_shrinkage_stress.svg`
//! (radial and tangential stress at every Gauss point after 0.5·10²⁵ n/m² of
//! BISON 2013 dense-PyC shrinkage and creep, the case of
//! `tests::shrinking_pyc_compresses_the_sic`).
//!
//! **The inputs are synthetic** (round radii and elastic constants, not
//! cited): the stress picture shows the mechanism, not a property of any
//! real particle.
//!
//! No test reaches this file: it is `main` glue and drawing code, the two
//! exceptions of the workspace "every function is reached by a test" rule.

use boon_lay::triso_particle_mechanics::geometry::{LayerSpec, ParticleMesh};
use boon_lay::triso_particle_mechanics::history::{run_history, HistoryPoint};
use boon_lay::triso_particle_mechanics::material::{CreepLawSign, DimensionalChange, IrradiationCreep, LayerMaterial};
use boon_lay::triso_particle_mechanics::mechanics::ParticleSolver;
use std::fmt::Write as _;
use uom::si::f64::{Length, Pressure, ThermodynamicTemperature};
use uom::si::length::micrometer;
use uom::si::pressure::gigapascal;
use uom::si::thermodynamic_temperature::kelvin;

const COLOURS: [&str; 5] = ["#d95f02", "#7f7f7f", "#1b9e77", "#7570b3", "#66a61e"];

fn material(e_gpa: f64, nu: f64) -> LayerMaterial {
    match LayerMaterial::elastic(Pressure::new::<gigapascal>(e_gpa), nu) {
        Ok(m) => m,
        Err(err) => panic!("synthetic material is valid: {err}"),
    }
}

fn layer(label: &str, a: f64, b: f64, n: usize, m: LayerMaterial) -> LayerSpec {
    match LayerSpec::new(label, Length::new::<micrometer>(a), Length::new::<micrometer>(b), n, m) {
        Ok(l) => l,
        Err(err) => panic!("synthetic layer is valid: {err}"),
    }
}

fn mesh(layers: Vec<LayerSpec>) -> ParticleMesh {
    match ParticleMesh::new(layers) {
        Ok(m) => m,
        Err(err) => panic!("synthetic mesh is valid: {err}"),
    }
}

/// One panel: a cross-section disc (annuli per element, coloured by the
/// element's layer) and, below it, a radial strip with every node.
fn draw_mesh(svg: &mut String, m: &ParticleMesh, x0: f64, title: &str) {
    let rmax = m.outer_radius_m();
    let scale = 200.0 / rmax; // px per metre: outer radius 200 px
    let (cx, cy) = (x0 + 220.0, 250.0);
    let _ = writeln!(svg, r#"<text x="{}" y="24" font-size="16" text-anchor="middle">{title}</text>"#, cx);
    // Outside in, so inner annuli paint over outer ones.
    for e in (0..m.n_elements()).rev() {
        let r = m.node_radius_m(e + 1) * scale;
        let c = COLOURS[m.element_layer(e) % COLOURS.len()];
        let _ = writeln!(svg, r##"<circle cx="{cx}" cy="{cy}" r="{r:.3}" fill="{c}" stroke="#000" stroke-width="0.3"/>"##);
    }
    if m.inner_radius_m() > 0.0 {
        let r = m.inner_radius_m() * scale;
        let _ = writeln!(svg, r##"<circle cx="{cx}" cy="{cy}" r="{r:.3}" fill="#fff" stroke="#000" stroke-width="0.6"/>"##);
        let _ = writeln!(svg, r#"<text x="{cx}" y="{cy}" font-size="11" text-anchor="middle">not meshed</text>"#);
    }
    // Radial strip: nodes as ticks from 0 to r_max.
    let (sx, sy) = (cx - 200.0, 490.0);
    let _ = writeln!(svg, r##"<line x1="{sx}" y1="{sy}" x2="{}" y2="{sy}" stroke="#000"/>"##, sx + 400.0);
    for e in 0..m.n_elements() {
        let a = sx + 2.0 * m.node_radius_m(e) * scale;
        let b = sx + 2.0 * m.node_radius_m(e + 1) * scale;
        let c = COLOURS[m.element_layer(e) % COLOURS.len()];
        let _ = writeln!(svg, r#"<rect x="{a:.3}" y="{}" width="{:.3}" height="12" fill="{c}"/>"#, sy - 6.0, b - a);
    }
    for r in m.node_radii_m() {
        let x = sx + 2.0 * r * scale;
        let _ = writeln!(svg, r##"<line x1="{x:.3}" y1="{}" x2="{x:.3}" y2="{}" stroke="#000" stroke-width="0.5"/>"##, sy - 8.0, sy + 8.0);
    }
    let _ = writeln!(svg, r#"<text x="{sx}" y="{}" font-size="11">0</text>"#, sy + 24.0);
    let _ = writeln!(svg, r#"<text x="{}" y="{}" font-size="11" text-anchor="end">r = {:.0} um; every node is a tick</text>"#, sx + 400.0, sy + 24.0, rmax * 1e6);
    // Legend with each layer's radii.
    for (il, l) in m.layers().iter().enumerate() {
        let y = 540.0 + 18.0 * il as f64;
        let _ = writeln!(svg, r##"<rect x="{sx}" y="{}" width="12" height="12" fill="{}"/>"##, y - 10.0, COLOURS[il % COLOURS.len()]);
        let _ = writeln!(
            svg,
            r#"<text x="{}" y="{y}" font-size="12">{}: {:.1}-{:.1} um, {} elements</text>"#,
            sx + 18.0,
            l.label,
            l.inner_radius_m * 1e6,
            l.outer_radius_m * 1e6,
            l.n_elements
        );
    }
}

fn main() {
    let out_dir = match std::env::args().nth(1) {
        Some(d) => std::path::PathBuf::from(d),
        None => std::path::PathBuf::from("."),
    };
    let five = mesh(vec![
        layer("kernel", 0.0, 250.0, 8, material(200.0, 0.3)),
        layer("buffer", 250.0, 350.0, 8, material(20.0, 0.23)),
        layer("IPyC", 350.0, 390.0, 8, material(30.0, 0.23)),
        layer("SiC", 390.0, 425.0, 8, material(400.0, 0.13)),
        layer("OPyC", 425.0, 465.0, 8, material(30.0, 0.23)),
    ]);
    let pyc = material(30.0, 0.23)
        .with_dimensional_change(DimensionalChange::Bison2013DensePyc)
        .with_irradiation_creep(IrradiationCreep::Bison2013Pyc { density_g_per_cm3: 1.9, sign: CreepLawSign::VolumeConserving });
    let three = mesh(vec![
        layer("IPyC", 200.0, 240.0, 16, pyc),
        layer("SiC", 240.0, 275.0, 16, material(400.0, 0.13)),
        layer("OPyC", 275.0, 315.0, 16, pyc),
    ]);
    let mut svg = String::new();
    let _ = writeln!(svg, r##"<svg xmlns="http://www.w3.org/2000/svg" width="900" height="640" font-family="sans-serif"><rect width="900" height="640" fill="#fff"/>"##);
    draw_mesh(&mut svg, &five, 0.0, "Conduction-test mesh (5 layers, assembled)");
    draw_mesh(&mut svg, &three, 450.0, "Mechanics-test mesh (IPyC/SiC/OPyC, assembled)");
    svg.push_str("</svg>\n");
    let p1 = out_dir.join("particle_mesh.svg");
    match std::fs::write(&p1, &svg) {
        Ok(()) => println!("wrote {}", p1.display()),
        Err(err) => panic!("cannot write {}: {err}", p1.display()),
    }

    // Stress profile after PyC shrinkage, no pressure.
    let mut solver = ParticleSolver::new(three, ThermodynamicTemperature::new::<kelvin>(300.0));
    let history = [
        HistoryPoint { fast_fluence_m2: 0.0, inner_pressure_pa: 0.0, outer_pressure_pa: 0.0, temperature_k: 1273.15 },
        HistoryPoint { fast_fluence_m2: 0.5e25, inner_pressure_pa: 0.0, outer_pressure_pa: 0.0, temperature_k: 1273.15 },
    ];
    match run_history(&mut solver, &history, 10, &[]) {
        Ok(_) => {}
        Err(err) => panic!("history failed: {err}"),
    }
    let pts = solver.points();
    let (r0, r1) = (solver.mesh().inner_radius_m(), solver.mesh().outer_radius_m());
    let smax = pts.iter().map(|p| p.stress_pa[0].abs().max(p.stress_pa[1].abs())).fold(0.0, f64::max);
    let (w, h, l, t) = (800.0, 400.0, 80.0, 40.0);
    let x = |r: f64| l + (r - r0) / (r1 - r0) * w;
    let y = |s: f64| t + h / 2.0 - s / smax * h / 2.0;
    let mut s2 = String::new();
    let _ = writeln!(s2, r##"<svg xmlns="http://www.w3.org/2000/svg" width="960" height="520" font-family="sans-serif"><rect width="960" height="520" fill="#fff"/>"##);
    let _ = writeln!(s2, r#"<text x="480" y="24" font-size="15" text-anchor="middle">Stress after 0.5e25 n/m^2 of BISON 2013 dense-PyC shrinkage + creep (synthetic elastic constants)</text>"#);
    for (il, lay) in solver.mesh().layers().iter().enumerate() {
        let _ = writeln!(s2, r#"<rect x="{:.2}" y="{t}" width="{:.2}" height="{h}" fill="{}" opacity="0.15"/>"#, x(lay.inner_radius_m), x(lay.outer_radius_m) - x(lay.inner_radius_m), COLOURS[(il + 2) % 5]);
        let _ = writeln!(s2, r#"<text x="{:.2}" y="{}" font-size="12" text-anchor="middle">{}</text>"#, 0.5 * (x(lay.inner_radius_m) + x(lay.outer_radius_m)), t + h + 18.0, lay.label);
    }
    let _ = writeln!(s2, r##"<line x1="{l}" y1="{}" x2="{}" y2="{}" stroke="#000"/>"##, y(0.0), l + w, y(0.0));
    for (k, col, name) in [(0usize, "#1f77b4", "sigma_r"), (1, "#d62728", "sigma_theta")] {
        for p in pts {
            let _ = writeln!(s2, r#"<circle cx="{:.2}" cy="{:.2}" r="2" fill="{col}"/>"#, x(p.radius_m), y(p.stress_pa[k]));
        }
        let _ = writeln!(s2, r#"<text x="{}" y="{}" font-size="13" fill="{col}">{name}</text>"#, l + w - 100.0, t + 20.0 + 18.0 * k as f64);
    }
    let _ = writeln!(s2, r#"<text x="{}" y="{}" font-size="11" text-anchor="end">+{:.0} MPa</text>"#, l - 4.0, t + 4.0, smax / 1e6);
    let _ = writeln!(s2, r#"<text x="{}" y="{}" font-size="11" text-anchor="end">0</text>"#, l - 4.0, y(0.0) + 4.0);
    let _ = writeln!(s2, r#"<text x="{}" y="{}" font-size="11" text-anchor="end">-{:.0} MPa</text>"#, l - 4.0, t + h, smax / 1e6);
    let _ = writeln!(s2, r#"<text x="{}" y="{}" font-size="12" text-anchor="middle">r from {:.0} to {:.0} um</text>"#, l + w / 2.0, t + h + 40.0, r0 * 1e6, r1 * 1e6);
    s2.push_str("</svg>\n");
    let p2 = out_dir.join("pyc_shrinkage_stress.svg");
    match std::fs::write(&p2, &s2) {
        Ok(()) => println!("wrote {}", p2.display()),
        Err(err) => panic!("cannot write {}: {err}", p2.display()),
    }
    let sic = match solver.layer_stress_range(1) {
        Ok(r) => r,
        Err(err) => panic!("{err}"),
    };
    println!("SiC tangential stress range: {:.4e} .. {:.4e} Pa", sic.tangential_min_pa, sic.tangential_max_pa);
}
