// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam boundary conditions against the real classes (gh:#1871, round 51)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da` on OpenFOAM-ESI v2506. `upstream_driver/bc1871_dump/bc1871Dump.C` reads fields whose
//! patches are the REAL `blackBodyRadiation`, `timeFieldTable` and `velocityRundown` (UNMODIFIED `libFFSSrc2`) on the
//! 24-cell box of `cases/fo1871/polyMesh`, drives the cell values from closed-form functions (only `+ - * /`), calls
//! `correctBoundaryConditions()` at t = 1..12 and prints every patch value at `%.17g`
//! (`refs/bc1871.ref.txt.gz`, `scripts/build_and_run_bc1871_dump.sh`). The port is fed the same inputs and each patch
//! value compared with `to_bits`.
//!
//! **Upstream facts the record depends on.** (1) `blackBodyRadiation`'s dictionary constructor never reads `value`:
//! the initial patch value is uninitialised memory (the dump prints it, `Tbb.initial`: `5.65e55` on one face); the port
//! is therefore started from the dumped initial state and the comparison covers the map state -> state. (2)
//! `velocityRundown`'s constructor divides `U0` by the rundown factor when the field is read after `t0`. (3)
//! `timeFieldTable` reads past its list for `t` above the last node (undefined behaviour): not run, the port panics.
//! (4) `NusseltThermalBaffle1D` is not run as a class: see the README, "Round 51".
//!
//! **Pass criterion: bit identity**; a 1-ulp petir-vs-glibc `pow` difference is the only allowance
//! (petir `AGENTS.md`). Controls (each must differ): the earlier CODATA sigma / operation order, the earlier per-face
//! `ScalarInterpolateTable` form, `Urun2` without the construction division.

#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use petir::mathf::RealMath;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::boundary_conditions::blackbody_radiation::openfoam_stefan_boltzmann_sigma;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::boundary_conditions::{
    BlackBodyRadiationBc, TimeFieldTable, UpstreamTimeFieldTable, VelocityRundownBc,
};
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_basic_lib::fv_operators::fvm::DeltaCoeff;
use outram_foam_basic_lib::primitives::Vector3;
use uom::si::f64::{Frequency, Ratio, ThermalConductivity, ThermodynamicTemperature, Time};
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

const N_STEPS: usize = 12;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let flg = bytes[3];
    let mut at = 10;
    if flg & 4 != 0 {
        at += 2 + usize::from(bytes[at]) + 256 * usize::from(bytes[at + 1]);
    }
    for bit in [8u8, 16u8] {
        if flg & bit != 0 {
            while bytes[at] != 0 {
                at += 1;
            }
            at += 1;
        }
    }
    if flg & 2 != 0 {
        at += 2;
    }
    String::from_utf8(decompress_to_vec(&bytes[at..bytes.len() - 8]).unwrap()).unwrap()
}

fn load() -> HashMap<(usize, String), Vec<f64>> {
    let text = gunzip(&vv_dir().join("refs/bc1871.ref.txt.gz"));
    let mut me = HashMap::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if matches!(it.next(), Some("M") | Some("B")) {
            let step: usize = it.next().unwrap().parse().unwrap();
            let name = it.next().unwrap().to_string();
            me.insert((step, name), it.map(|x| x.parse().unwrap()).collect());
        }
    }
    me
}

#[derive(Default)]
struct Tally {
    n: usize,
    exact: usize,
    one_ulp: usize,
    worst_rel: f64,
    first_diff: Option<String>,
}

impl Tally {
    fn cmp(&mut self, what: &str, port: f64, up: f64) {
        self.n += 1;
        if port.to_bits() == up.to_bits() {
            self.exact += 1;
        } else {
            if (port.to_bits() as i64 - up.to_bits() as i64).abs() == 1 {
                self.one_ulp += 1;
            }
            let rel = (port - up).abs() / up.abs().max(1e-300);
            if rel > self.worst_rel {
                self.worst_rel = rel;
            }
            if self.first_diff.is_none() {
                self.first_diff = Some(format!("{what}: port {port:?} upstream {up:?}"));
            }
        }
    }

    fn report(&self, label: &str) {
        println!(
            "{label}: {}/{} bit-identical ({} differ by 1 ulp), worst relative difference {:e}{}",
            self.exact,
            self.n,
            self.one_ulp,
            self.worst_rel,
            self.first_diff.as_ref().map(|d| format!(", first: {d}")).unwrap_or_default()
        );
    }
}

fn triples(v: &[f64]) -> Vec<Vector3> {
    v.chunks(3).map(|c| Vector3::new(c[0], c[1], c[2])).collect()
}

/// G1: `blackBodyRadiation`: sigma, deltaCoeffs, and the patch value over 12 steps (closed and open loop).
#[test]
fn black_body_radiation_matches_the_real_class() {
    let r = load();
    let m = read_poly_mesh(&vv_dir().join("cases/fo1871/polyMesh")).unwrap();
    let sigma = r[&(0, "sigma".to_string())][0];
    let mut t = Tally::default();
    t.cmp("sigma", openfoam_stefan_boltzmann_sigma(), sigma);
    println!(
        "G1 sigma: upstream {sigma:e}, port old constant {:e} (relative {:e})",
        5.670_374_419e-8_f64,
        (5.670_374_419e-8_f64 - sigma).abs() / sigma
    );
    let outlet = m.patches.iter().position(|p| p.name == "outlet").unwrap();
    let start = m.patches[outlet].start;
    let inv_d: Vec<f64> = r[&(0, "deltaCoeffs.outlet".to_string())].clone();
    for (i, up) in inv_d.iter().enumerate() {
        t.cmp("deltaCoeffs", DeltaCoeff::Orthogonal.boundary_delta_coeff(&m, start + i), *up);
    }
    let bc = BlackBodyRadiationBc::new(
        Ratio::new::<ratio>(0.78),
        ThermalConductivity::new::<watt_per_meter_kelvin>(10.0),
        ThermodynamicTemperature::new::<kelvin>(300.0),
    );
    let mut closed = Tally::default();
    let mut open = Tally::default();
    let mut tp = r[&(0, "Tbb.initial".to_string())].clone();
    let mut old_form_diff = 0usize;
    let mut old_tp = tp.clone();
    for n in 1..=N_STEPS {
        let ti = &r[&(n, "Ti.outlet".to_string())];
        let up = &r[&(n, "Tbb.outlet".to_string())];
        let up_prev = if n == 1 { r[&(0, "Tbb.initial".to_string())].clone() } else { r[&(n - 1, "Tbb.outlet".to_string())].clone() };
        tp = bc.update_patch_upstream(&tp, ti, &inv_d);
        let open_loop = bc.update_patch_upstream(&up_prev, ti, &inv_d);
        for i in 0..tp.len() {
            closed.cmp(&format!("closed step {n} face {i}"), tp[i], up[i]);
            open.cmp(&format!("open step {n} face {i}"), open_loop[i], up[i]);
        }
        // control: the earlier form (CODATA sigma, `kappa (Tc - Tf) / d`, powi(4))
        let kappa = 10.0;
        for i in 0..old_tp.len() {
            let q = (kappa * (ti[i] - old_tp[i]) / (1.0 / inv_d[i])).abs();
            old_tp[i] = (q / (5.670_374_419e-8 * 0.78) + 300.0_f64 * 300.0 * 300.0 * 300.0).r_powf(0.25);
            if old_tp[i].to_bits() != up[i].to_bits() {
                old_form_diff += 1;
            }
        }
    }
    t.report("G1 sigma + deltaCoeffs");
    closed.report("G1 patch value, closed loop");
    open.report("G1 patch value, open loop");
    println!("G1 control: the earlier form differs from upstream in {old_form_diff} of {} values", N_STEPS * 6);
    assert_eq!(t.exact, t.n, "sigma / deltaCoeffs: {:?}", t.first_diff);
    assert_eq!(open.exact + open.one_ulp, open.n, "open loop: {:?}", open.first_diff);
    assert_eq!(closed.exact + closed.one_ulp, closed.n, "closed loop: {:?}", closed.first_diff);
    assert!(old_form_diff > 0, "control must differ");
}

/// G2: `timeFieldTable`: values at t = 1..12 on, before and between the nodes.
#[test]
fn time_field_table_matches_the_real_class() {
    let r = load();
    let times = vec![2.0, 5.0, 9.0, 12.0];
    let rows = vec![
        vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0],
        vec![20.0, 18.5, 17.0, 15.5, 14.0, 12.5],
        vec![7.25, 8.75, 10.5, 3.125, 9.0, 1.5],
        vec![30.0, 29.0, 28.0, 27.0, 26.0, 25.0],
    ];
    let mut table = UpstreamTimeFieldTable::new(times.clone(), rows.clone());
    let mut t = Tally::default();
    for (i, v) in table.value_at(0.0).iter().enumerate() {
        t.cmp("construction", *v, r[&(0, "Ttab.initial".to_string())][i]);
    }
    let old = TimeFieldTable::new(times.iter().map(|x| Time::new::<second>(*x)).collect(), rows).unwrap();
    let mut old_diff = 0usize;
    for n in 1..=N_STEPS {
        let up = &r[&(n, "Ttab.inlet".to_string())];
        let port = table.value_at(n as f64);
        let o = old.value(Time::new::<second>(n as f64));
        for i in 0..6 {
            t.cmp(&format!("step {n} face {i}"), port[i], up[i]);
            if o[i].to_bits() != up[i].to_bits() {
                old_diff += 1;
            }
        }
    }
    t.report("G2 timeFieldTable");
    println!("G2 control: the earlier per-face interpolation differs in {old_diff} of {} values", N_STEPS * 6);
    assert_eq!(t.exact, t.n, "{:?}", t.first_diff);
    assert!(old_diff > 0, "control must differ");
}

/// A time above the last node is upstream undefined behaviour; the port refuses it by panicking.
#[test]
#[should_panic(expected = "above the last node")]
fn time_field_table_refuses_a_time_past_the_last_node() {
    let mut table = UpstreamTimeFieldTable::new(vec![0.0, 1.0], vec![vec![1.0], vec![2.0]]);
    let _ = table.value_at(5.0);
}

/// G3: `velocityRundown`: `U(t) = U0 (A + B (t - t0))^C`, constructed before and after `t0`.
#[test]
fn velocity_rundown_matches_the_real_class() {
    let r = load();
    let bc = VelocityRundownBc::new(
        Ratio::new::<ratio>(1.0),
        Frequency::new::<hertz>(-0.04),
        1.5,
        Time::new::<second>(3.0),
    );
    let u0 = Vector3::new(1.0, 0.5, 0.25);
    let mut t = Tally::default();
    let mut no_division_diff = 0usize;
    // Urun constructed at t = 0 <= t0
    let ref0 = bc.reference_velocity_at_construction(u0, Time::new::<second>(0.0));
    for n in 1..=N_STEPS {
        let up = triples(&r[&(n, "Urun.inlet".to_string())]);
        let port = bc.velocity(ref0, Time::new::<second>(n as f64));
        for u in &up {
            t.cmp("Urun x", port.x, u.x);
            t.cmp("Urun y", port.y, u.y);
            t.cmp("Urun z", port.z, u.z);
        }
    }
    // Urun2 constructed at t = 6 > t0
    let ref6 = bc.reference_velocity_at_construction(u0, Time::new::<second>(6.0));
    let up = triples(&r[&(6, "Urun2.constructed".to_string())]);
    t.cmp("Urun2 constructed x", ref6.x, up[0].x);
    t.cmp("Urun2 constructed y", ref6.y, up[0].y);
    t.cmp("Urun2 constructed z", ref6.z, up[0].z);
    for n in 6..=N_STEPS {
        let up = triples(&r[&(n, "Urun2.inlet".to_string())]);
        let port = bc.velocity(ref6, Time::new::<second>(n as f64));
        let plain = bc.velocity(u0, Time::new::<second>(n as f64));
        for u in &up {
            t.cmp("Urun2 x", port.x, u.x);
            t.cmp("Urun2 y", port.y, u.y);
            t.cmp("Urun2 z", port.z, u.z);
            if plain.x.to_bits() != u.x.to_bits() {
                no_division_diff += 1;
            }
        }
    }
    t.report("G3 velocityRundown");
    println!("G3 control: Urun2 without the construction division differs in {no_division_diff} of {} x values", (N_STEPS - 5) * 6);
    assert!(t.exact + t.one_ulp == t.n, "{:?}", t.first_diff);
    assert!(no_division_diff > 0, "control must differ");
}
