// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `pointKineticNeutronics` (solid fuel) against the real class (gh:#1864, gh:#1361 round 36)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da` on OpenFOAM-ESI v2506, the unmodified `solvers::pointKineticNeutronics`
//! (`libNeutronicsSolvers.so`), driven by `upstream_driver/pkRun1864/pkRun.C` on the `neutroRegion` mesh of
//! `Tutorials/reactorCases/1D_MSR_pointKinetics` (640 cells, five cell zones) with `liquidFuel false`:
//! 120 calls of the real `correctPhysics()`, `dt` cycling 1e-3..0.2, the six per-cell feedback fields set each
//! step from integer formulas that this test repeats. Ten case variants, one per option
//! (`scripts/run_pk_1864.sh`, `refs/pk1864_<variant>.ref.txt.gz`). The reference holds, per step, the real class'
//! total reactivity, fission/decay/total power, external source power and modulation, beam power and intensity,
//! `maxDeltaT()`, sums of the rescaled fields, references and precursor powers; from the second step also the
//! seven flux-squared weighted averages and every reactivity term, printed by running upstream's own
//! `computeFeedbackFieldValues.H` and `correctReactivity.H` verbatim. The cell volumes and the zones are in the
//! reference too (this test needs no mesh). `t` and `dt` are inputs from the reference.
//!
//! The port runs its OWN loop (free run: nothing of upstream's per-step state is an input after the constructor).
//!
//! Results and predictions: README "Round 36". Run with `--nocapture` to print every tally.

#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::common::TimeProfile;
use outram_foam_appbuilder_lib::genfoam::neutronics::point_kinetics::model::{
    feedback_cell_field, ExternalSourceConfig, ExternalSourceMode, FeedbackCoefficients, PkConfig,
    PkFeedbackFields, PkMeshData, PointKineticNeutronics,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::point_kinetics::LinearSolveForm;
use uom::si::f64::Time;
use uom::si::time::second;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")).expect("utf8")
}

/// One reference: the lines by tag.
struct Reference {
    cfg: Vec<Vec<String>>,
    vol: Vec<f64>,
    zones: HashMap<String, Vec<usize>>,
    prof: Vec<Vec<f64>>,
    terms: HashMap<usize, Vec<f64>>,
    steps: Vec<Vec<f64>>,
}

fn nums(parts: &[&str]) -> Vec<f64> {
    parts.iter().map(|w| w.parse::<f64>().unwrap_or_else(|_| panic!("number {w}"))).collect()
}

fn read_reference(variant: &str) -> Reference {
    let text = gunzip(&vv_dir().join(format!("refs/pk1864_{variant}.ref.txt.gz")));
    let mut r = Reference {
        cfg: vec![],
        vol: vec![],
        zones: HashMap::new(),
        prof: vec![],
        terms: HashMap::new(),
        steps: vec![],
    };
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w[0] {
            "CFG" => r.cfg.push(w[1..].iter().map(|s| s.to_string()).collect()),
            "VOL" => r.vol = nums(&w[1..]),
            "ZONE" => {
                let n: usize = w[2].parse().unwrap();
                r.zones.insert(w[1].to_string(), w[3..3 + n].iter().map(|s| s.parse().unwrap()).collect());
            }
            "PROF" => r.prof.push(nums(&w[1..])),
            "T" => {
                let idx: usize = w[1].parse().unwrap();
                let flat: Vec<f64> = w[2..].iter().filter(|s| **s != "|").map(|s| s.parse().unwrap()).collect();
                r.terms.insert(idx, flat);
            }
            "K" => {
                // K idx t dt | A(14) | B(8) | C(8)
                let flat: Vec<f64> =
                    w[1..].iter().filter(|s| **s != "|").map(|s| s.parse().unwrap()).collect();
                r.steps.push(flat);
            }
            _ => {}
        }
    }
    r
}

fn cfg_value(r: &Reference, key: &str) -> f64 {
    for l in &r.cfg {
        if matches!(l[0].as_str(), "beta" | "lambda" | "rod" | "gemmap" | "prec0" | "found") {
            continue;
        }
        if let Some(p) = l.iter().position(|w| w == key) {
            return l[p + 1].parse().unwrap();
        }
    }
    panic!("no CFG {key}")
}

/// The integer-formula field of the driver: `double((a*i + b*k) % m) / m`.
fn frac(i: usize, k: usize, a: usize, b: usize, m: usize) -> f64 {
    ((a * i + b * k) % m) as f64 / m as f64
}

fn table(rows: &[(f64, f64)], start: f64) -> TimeProfile {
    TimeProfile::table(
        rows.iter().map(|r| Time::new::<second>(r.0)).collect(),
        rows.iter().map(|r| r.1).collect(),
    )
    .unwrap()
    .with_start_time(Time::new::<second>(start))
}

const BETA: [f64; 8] = [2.114580e-04, 4.231390e-04, 3.737000e-04, 5.789020e-04, 8.929690e-04, 1.426090e-04, 1.926010e-04, 3.738980e-05];
const LAMBDA: [f64; 8] = [1.246670e-02, 2.829170e-02, 4.252440e-02, 1.330420e-01, 2.924670e-01, 6.664880e-01, 1.634780e+00, 3.099610e+00];

/// The case variant of `scripts/run_pk_1864.sh`, written again here as Rust values.
fn variant_config(v: &str) -> (PkConfig, Vec<&'static str>) {
    let mut rods: Vec<(f64, f64)> = vec![];
    let mut drive_zones: Vec<&'static str> = vec![];
    let mut driveline_coeff = 0.0;
    let mut cfg = PkConfig {
        prompt_generation_time: 1.26e-6,
        betas: BETA.to_vec(),
        lambdas: LAMBDA.to_vec(),
        fast_neutrons: true,
        coefficients: FeedbackCoefficients {
            doppler: 4.0e-3,
            t_fuel: -3.54253e-05,
            t_clad: -1.0e-05,
            t_cool: -2.0e-05,
            rho_cool: 1.5e-07,
            t_struct: -4.0e-05,
            t_struct_mech: 3.0e-05,
            driveline_expansion: 0.0,
        },
        control_rod_map: vec![],
        gem_map: vec![],
        external_reactivity: None,
        boron_reactivity: None,
        decay_power: None,
        p_target: 1.0e9,
        subcritical_index: None,
        ksrc: None,
        precursor_powers: None,
        external_source: None,
        energy_groups: 6,
        start_time: 0.0,
    };
    let src = |mode, profile, mf, delay| ExternalSourceConfig {
        mode,
        modulation_profile: profile,
        nu_source: 30.0,
        beam_energy: 1.0e9,
        nu_fission: 2.45,
        energy_per_fission: 3.2e-11,
        modulation_factor: mf,
        power_modulation_delay: delay,
    };
    match v {
        "feedback" => {}
        "thermal" => cfg.fast_neutrons = false,
        "rods_a" | "combined" => {
            rods = vec![(-0.01, 1e-3), (0.0, 0.0), (0.01, -1e-3)];
            driveline_coeff = 1.0e-3;
            drive_zones = vec!["pump", "hx"];
        }
        "rods_d" => {
            rods = vec![(0.01, -1e-3), (0.0, 0.0), (-0.01, 1e-3)];
            driveline_coeff = 1.0e-3;
            drive_zones = vec!["pump", "hx"];
        }
        "profiles" => {
            cfg.external_reactivity =
                Some(table(&[(0.0, 0.0), (1.0, 1e-4), (5.0, -2e-4), (30.0, 3e-4)], 0.05));
            cfg.boron_reactivity = Some(table(&[(0.0, 0.0), (2.0, -1.5e-4), (60.0, 1e-4)], 0.0));
        }
        "decay" => {
            cfg.decay_power = Some(table(&[(0.0, 6e7), (3.0, 5e7), (10.0, 3e7), (100.0, 1e7)], 0.02));
        }
        "src_transient" => {
            cfg.subcritical_index = Some(0.02);
            cfg.external_source = Some(src(
                ExternalSourceMode::Transient,
                Some(table(&[(0.0, 1.0), (1.0, 1.5), (3.0, 0.5), (30.0, 2.0)], 0.0)),
                0.0,
                0.0,
            ));
        }
        "src_monitor" => {
            cfg.ksrc = Some(0.97);
            cfg.external_source = Some(src(ExternalSourceMode::PowerMonitoring, None, 2.0, 0.05));
        }
        "restart_both" => {
            cfg.ksrc = Some(0.97);
            cfg.subcritical_index = Some(0.05);
            cfg.precursor_powers =
                Some(vec![4.1e7, 8.5e7, 7.0e7, 4.6e7, 2.5e7, 3.1e6, 1.2e6, 1.0e5]);
            cfg.external_source = Some(src(
                ExternalSourceMode::Transient,
                Some(table(&[(0.0, 1.0), (30.0, 1.2)], 0.0)),
                0.0,
                0.0,
            ));
        }
        other => panic!("unknown variant {other}"),
    }
    if v == "combined" {
        cfg.external_reactivity = Some(table(&[(0.0, 0.0), (1.0, 1e-4), (30.0, -1e-4)], 0.0));
        cfg.subcritical_index = Some(0.01);
        cfg.decay_power = Some(table(&[(0.0, 5e7), (100.0, 1e7)], 0.0));
        cfg.external_source = Some(src(ExternalSourceMode::PowerMonitoring, None, 1.0, 0.1));
    }
    cfg.control_rod_map = rods;
    cfg.coefficients.driveline_expansion = driveline_coeff;
    (cfg, drive_zones)
}

fn cells_of(r: &Reference, names: &[&str]) -> Vec<Vec<usize>> {
    names.iter().map(|n| r.zones[*n].clone()).collect()
}

fn mesh_data(r: &Reference, drive_zones: &[&str]) -> PkMeshData {
    let n = r.vol.len();
    PkMeshData {
        volume: r.vol.clone(),
        fuel: feedback_cell_field(n, &cells_of(r, &["core", "HotLeg"])),
        // The case lists `coolantFeedbackZones ("core" "HotLeg" "hx")`, but upstream reads the key
        // `coolFeedbackZones` (pointKineticNeutronics.C, `setFeedbackCellField(..., "coolFeedbackZones")`):
        // the list is ignored and every cell is a coolant cell (CFG sumCoolCells = 640). The shipped
        // `1D_MSR_pointKinetics` and `2D_FFTF` use `coolantFeedbackZones` too.
        cool: feedback_cell_field(n, &[]),
        structure: feedback_cell_field(n, &[]),
        structure_mech: feedback_cell_field(n, &cells_of(r, &["hx"])),
        driveline: feedback_cell_field(n, &cells_of(r, drive_zones)),
    }
}

fn initial_flux(r: &Reference) -> Vec<f64> {
    let mut f = vec![0.0; r.vol.len()];
    for (name, value) in [("core", 1.0), ("HotLeg", 0.7), ("pump", 0.3), ("hx", 0.1), ("ColdLeg", 0.4)] {
        for &c in &r.zones[name] {
            f[c] = value;
        }
    }
    f
}

fn step_fields(n: usize, k: usize) -> PkFeedbackFields {
    PkFeedbackFields {
        t_fuel: (0..n).map(|i| 900.0 + 150.0 * frac(i, k, 3, 5, 97)).collect(),
        t_clad: (0..n).map(|i| 850.0 + 60.0 * frac(i, k, 5, 3, 89)).collect(),
        t_cool: (0..n).map(|i| 800.0 + 50.0 * frac(i, k, 7, 2, 83)).collect(),
        rho_cool: (0..n).map(|i| 3300.0 - 80.0 * frac(i, k, 2, 9, 79)).collect(),
        t_struct: (0..n).map(|i| 780.0 + 30.0 * frac(i, k, 11, 4, 73)).collect(),
        t_struct_mech: (0..n).map(|i| 700.0 + 20.0 * frac(i, k, 13, 6, 71)).collect(),
    }
}

/// Bit-identity tally of one quantity.
#[derive(Default, Clone)]
struct Tally {
    n: usize,
    identical: usize,
    worst_rel: f64,
    first_diff: Option<usize>,
}

impl Tally {
    fn add(&mut self, step: usize, port: f64, up: f64) {
        self.n += 1;
        if port.to_bits() == up.to_bits() || (port == 0.0 && up == 0.0) {
            self.identical += 1;
        } else {
            let rel = if up == 0.0 { (port - up).abs() } else { ((port - up) / up).abs() };
            if rel > self.worst_rel {
                self.worst_rel = rel;
            }
            if self.first_diff.is_none() {
                self.first_diff = Some(step);
            }
        }
    }
    fn show(&self, name: &str) -> String {
        format!(
            "{name:>22}: {}/{} bit-identical, worst rel {:.3e}, first diff step {:?}",
            self.identical, self.n, self.worst_rel, self.first_diff
        )
    }
}

/// Run one variant; returns the named tallies. `form` selects the dense solver (the control).
fn run_variant(v: &str, form: LinearSolveForm) -> Vec<(String, Tally)> {
    let r = read_reference(v);
    let (cfg, dz) = variant_config(v);
    let n = r.vol.len();
    let pd: Vec<f64> = (0..n).map(|i| 1.0e6 + 3.0e5 * frac(i, 1, 17, 1, 61)).collect();
    let spd: Vec<f64> = (0..n).map(|i| 2.0e5 + 1.0e5 * frac(i, 1, 19, 1, 59)).collect();
    let mut m = PointKineticNeutronics::new(cfg.clone(), mesh_data(&r, &dz), initial_flux(&r), pd, spd, form)
        .unwrap();
    let mut t: HashMap<String, Tally> = HashMap::new();
    let mut add = |name: &str, step: usize, port: f64, up: f64| {
        t.entry(name.to_string()).or_default().add(step, port, up);
    };
    // constructor
    add("ctor.beta", 0, m.beta(), cfg_value(&r, "beta"));
    add("ctor.subcritical", 0, m.subcritical_index(), cfg_value(&r, "subcritical"));
    add("ctor.ksrc", 0, m.ksrc(), cfg_value(&r, "ksrc"));
    add("ctor.gain", 0, m.intrinsic_gain(), cfg_value(&r, "gain"));
    add("ctor.srcRef", 0, m.external_source_power_ref(), cfg_value(&r, "srcRef"));
    add("ctor.fission0", 0, m.fission_power(), cfg_value(&r, "fission0"));
    add("ctor.decay0", 0, m.decay_power(), cfg_value(&r, "decay0"));
    let prec0: Vec<f64> = r.cfg.iter().find(|l| l[0] == "prec0").unwrap()[1..].iter().map(|s| s.parse().unwrap()).collect();
    for (i, p) in m.precursor_powers().iter().enumerate() {
        add("ctor.prec0", i, *p, prec0[i]);
    }
    // profiles
    for row in &r.prof {
        let (tt, dec, ext, bor, md) = (row[0], row[1], row[2], row[3], row[4]);
        let tm = Time::new::<second>(tt);
        if let Some(p) = cfg.decay_power.as_ref() {
            add("profile.decay", 0, p.value(tm), dec);
        }
        add("profile.extReact", 0, cfg.external_reactivity.as_ref().map_or(0.0, |p| p.value(tm)), ext);
        add("profile.boron", 0, cfg.boron_reactivity.as_ref().map_or(0.0, |p| p.value(tm)), bor);
        if let Some(es) = cfg.external_source.as_ref() {
            if let Some(p) = es.modulation_profile.as_ref() {
                add("profile.modulation", 0, p.value(tm), md);
            }
        }
    }
    for k in 0..r.steps.len() {
        let s = &r.steps[k];
        let (tt, dt) = (s[1], s[2]);
        let fields = step_fields(n, k);
        let report = m.step(tt, dt, &fields).unwrap();
        if k > 0 {
            let up = &r.terms[&k];
            let vals = [
                report.values.t_fuel,
                report.values.t_clad,
                report.values.t_cool,
                report.values.rho_cool,
                report.values.t_struct,
                report.values.t_struct_mech,
                report.values.t_driveline,
            ];
            for (i, name) in ["avg.TFuel", "avg.TClad", "avg.TCool", "avg.rhoCool", "avg.TStruct", "avg.TStructMech", "avg.TDriveline"].iter().enumerate() {
                add(name, k, vals[i], up[i]);
            }
            let tm = &report.terms;
            let terms = [
                tm.doppler,
                tm.t_fuel,
                tm.t_clad,
                tm.t_cool,
                tm.rho_cool,
                tm.t_struct,
                tm.t_struct_mech,
                tm.driveline_expansion,
                tm.driveline,
                tm.gem,
                tm.external,
                tm.boron,
            ];
            for (i, name) in ["term.Doppler", "term.TFuel", "term.TClad", "term.TCool", "term.rhoCool", "term.TStruct", "term.TStructMech", "term.drivelineExp", "term.driveline", "term.GEM", "term.external", "term.boron"].iter().enumerate() {
                add(name, k, terms[i], up[7 + i]);
            }
        }
        // K: [idx t dt | total fission decay power srcP srcMod beamP beamI maxDt sumFlux sumFluxLast sumPD sumSecPD pOld | refs7 fOld | C0..C7]
        add("total reactivity", k, report.terms.total, s[3]);
        add("fission power", k, m.fission_power(), s[4]);
        add("decay power", k, m.decay_power(), s[5]);
        add("power", k, m.power(), s[6]);
        let (sp, sm, bp, bi) = m.external_source();
        add("src power", k, sp, s[7]);
        add("src modulation", k, sm, s[8]);
        add("beam power", k, bp, s[9]);
        add("beam intensity", k, bi, s[10]);
        add("maxDeltaT", k, m.max_delta_t(dt, None, None), s[11]);
        let sum = |v: &Vec<f64>| {
            let mut a = 0.0;
            for x in v {
                a += *x;
            }
            a
        };
        add("sum oneGroupFlux", k, sum(&m.one_group_flux), s[12]);
        add("sum flux[last]", k, sum(&m.fluxes[m.fluxes.len() - 1]), s[13]);
        add("sum powerDensity", k, sum(&m.power_density), s[14]);
        add("sum secondaryPD", k, sum(&m.secondary_power_density), s[15]);
        let rf = m.references().unwrap();
        let refs = [rf.t_fuel, rf.t_clad, rf.t_cool, rf.rho_cool, rf.t_struct, rf.t_struct_mech, rf.t_driveline];
        for (i, name) in ["ref.TFuel", "ref.TClad", "ref.TCool", "ref.rhoCool", "ref.TStruct", "ref.TStructMech", "ref.TDriveline"].iter().enumerate() {
            add(name, k, refs[i], s[17 + i]);
        }
        for (i, c) in m.precursor_powers().iter().enumerate() {
            add("precursor powers", k, *c, s[25 + i]);
        }
    }
    let mut out: Vec<(String, Tally)> = t.into_iter().collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn report(v: &str, form: LinearSolveForm) -> Vec<(String, Tally)> {
    let t = run_variant(v, form);
    eprintln!("--- variant {v}, {form:?}");
    for (name, tally) in &t {
        eprintln!("{}", tally.show(name));
    }
    t
}

const VARIANTS: [&str; 10] = [
    "feedback", "thermal", "rods_a", "rods_d", "profiles", "decay", "src_transient", "src_monitor", "restart_both",
    "combined",
];

/// Gate: every quantity of every variant is bit-identical to the real class (measured 2026-11-27:
/// 504 tallies, all of them 100 %; README "Round 36").
#[test]
fn every_variant_is_bit_identical_to_the_real_class() {
    for v in VARIANTS {
        for (name, t) in report(v, LinearSolveForm::OpenFoamGaussian) {
            assert_eq!(t.identical, t.n, "{v}: {}", t.show(&name));
        }
    }
}

/// Control 1 (P1): upstream's rows with the port's earlier Crout LU. The comparison CAN fail: the step is
/// no longer bit-identical, and it stays within 1e-9 (round-off of two different eliminations).
#[test]
fn control_crout_lu_is_not_bit_identical_but_close() {
    let t = report("feedback", LinearSolveForm::CroutLu);
    let power = &t.iter().find(|(n, _)| n == "power").unwrap().1;
    assert!(power.identical < power.n, "the control must fail the bit-identity gate: {}", power.show("power"));
    assert!(power.worst_rel < 1.0e-9, "{}", power.show("power"));
    let prec = &t.iter().find(|(n, _)| n == "precursor powers").unwrap().1;
    assert!(prec.identical < prec.n);
}

/// Control 2 (P9): weights WITHOUT squaring the flux give a different average from step 2 on.
#[test]
fn control_unsquared_flux_weights_differ_from_the_real_class() {
    let r = read_reference("feedback");
    let (cfg, dz) = variant_config("feedback");
    let n = r.vol.len();
    let mesh = mesh_data(&r, &dz);
    let pd = vec![0.0; n];
    let mut m = PointKineticNeutronics::new(
        cfg,
        mesh.clone(),
        initial_flux(&r),
        pd.clone(),
        pd,
        LinearSolveForm::OpenFoamGaussian,
    )
    .unwrap();
    let mut worst: f64 = 0.0;
    for k in 0..4 {
        let s = &r.steps[k];
        let f = step_fields(n, k);
        if k > 0 {
            let (mut num, mut den) = (0.0, 0.0);
            for i in 0..n {
                num += mesh.volume[i] * ((m.one_group_flux[i] * f.t_fuel[i]) * mesh.fuel[i]);
                den += mesh.volume[i] * (m.one_group_flux[i] * mesh.fuel[i]);
            }
            let up = r.terms[&k][0];
            worst = worst.max(((num / den - up) / up).abs());
        }
        m.step(s[1], s[2], &f).unwrap();
    }
    assert!(worst > 1.0e-6, "unsquared weights must differ from upstream: worst {worst:e}");
}

/// Control 3 (P10): the TOTAL power compared with upstream's FISSION power (`decay` variant) differs by the
/// decay fraction, so the gate would catch stepping the wrong power.
#[test]
fn control_total_power_is_not_the_fission_power_with_decay_heat() {
    let r = read_reference("decay");
    let (cfg, dz) = variant_config("decay");
    let n = r.vol.len();
    let mut m = PointKineticNeutronics::new(
        cfg,
        mesh_data(&r, &dz),
        initial_flux(&r),
        vec![0.0; n],
        vec![0.0; n],
        LinearSolveForm::OpenFoamGaussian,
    )
    .unwrap();
    let s = &r.steps[0];
    m.step(s[1], s[2], &step_fields(n, 0)).unwrap();
    let rel = ((m.power() - s[4]) / s[4]).abs();
    assert!(rel > 1.0e-2, "total vs fission power must differ by about decay/total: {rel:e}");
    assert_eq!(m.fission_power().to_bits(), s[4].to_bits());
}

/// Analytic check (closed form): one delayed group, zero feedback, no source, one backward-Euler step.
/// With `a = 1/dt - (rho-beta)/L`, `d = 1/dt + lambda` the 2x2 system has
/// `P1 = (P0 d / dt + lambda C0 / dt) / (a d - lambda beta / L)` and
/// `C1 = (C0/dt + beta P1 / L) / d` (Cramer). Compared to 1e-12 (the closed form orders the
/// operations differently, so it is not bit-identical by design).
#[test]
fn one_group_step_matches_the_closed_form() {
    let r = read_reference("feedback");
    let (mut cfg, dz) = variant_config("feedback");
    let (beta, lambda, l) = (0.0065, 0.08, 1.0e-4);
    cfg.betas = vec![beta];
    cfg.lambdas = vec![lambda];
    cfg.prompt_generation_time = l;
    cfg.energy_groups = 1;
    cfg.coefficients = FeedbackCoefficients {
        doppler: 0.0,
        t_fuel: 0.0,
        t_clad: 0.0,
        t_cool: 0.0,
        rho_cool: 0.0,
        t_struct: 0.0,
        t_struct_mech: 0.0,
        driveline_expansion: 0.0,
    };
    cfg.external_reactivity = Some(table(&[(0.0, 2.0e-3), (100.0, 2.0e-3)], 0.0));
    let n = r.vol.len();
    let mut m = PointKineticNeutronics::new(
        cfg.clone(),
        mesh_data(&r, &dz),
        initial_flux(&r),
        vec![0.0; n],
        vec![0.0; n],
        LinearSolveForm::OpenFoamGaussian,
    )
    .unwrap();
    let dt = 0.05;
    let p0 = m.fission_power();
    let c0 = m.precursor_powers()[0];
    m.step(dt, dt, &step_fields(n, 0)).unwrap();
    let rho = 2.0e-3;
    let a = 1.0 / dt - (rho - beta) / l;
    let d = 1.0 / dt + lambda;
    let p1 = (p0 * d / dt + lambda * c0 / dt) / (a * d - lambda * beta / l);
    let c1 = (c0 / dt + beta * p1 / l) / d;
    assert!(((m.fission_power() - p1) / p1).abs() < 1.0e-12, "{} vs {p1}", m.fission_power());
    assert!(((m.precursor_powers()[0] - c1) / c1).abs() < 1.0e-12);
    assert!(m.fission_power() > p0, "positive reactivity raises the power");
}
