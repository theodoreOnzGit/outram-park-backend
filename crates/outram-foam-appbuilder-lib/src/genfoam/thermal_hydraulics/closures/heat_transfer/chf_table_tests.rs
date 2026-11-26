// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! Unit tests of `lookUpTableCHF` ([`super::chf_table`]), the field-valued interpolation
//! ([`crate::genfoam::common::interpolate_table_nd`]) and the Cachard models ([`super::cachard`]).
//! The table is the shipped `chfTables` (provenance: `tests/data/genfoam_chf_gh1223/PROVENANCE.md`).
//! The comparison with the real upstream binary is `tests/genfoam_chf_trace_vs_upstream.rs`.

use outram_foam_basic_lib::io::dict::FoamFile;

use super::cachard::{CachardLiquid, CachardLiquidInputs, CachardPitch, CachardVapour, film_thickness};
use super::chf_table::{ChfTableError, LookUpTableChf, LookUpTableChfInputs};
use crate::genfoam::common::interpolate_table::{
    InterpolateTableError, InterpolationMethod, OutOfBounds, ScalarInterpolateTable,
};
use crate::genfoam::common::interpolate_table_nd::{check_table, interpolate_table};

const TABLES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/genfoam_chf_gh1223/chfTables"));

fn table_with(pitch_to_diameter: &str, extra: &str) -> LookUpTableChf {
    let text = format!(
        "PitchToDiameter {pitch_to_diameter};\n\
         pressureValues (100.0 300.0 500.0 1000.0 2000.0 3000.0 5000.0 7000.0 10000.0 12000.0 14000.0 16000.0 18000.0 20000.0 21000.0);\n\
         massFlowRateValues (0.0 50.0 100.0 300.0 500.0 750.0 1000.0 1500.0 2000.0 2500.0 3000.0 3500.0 4000.0 4500.0 5000.0 5500.0 6000.0 6500.0 7000.0 7500.0 8000.0);\n\
         qualityValues (-0.50 -0.40 -0.30 -0.20 -0.15 -0.10 -0.05 0.00 0.05 0.10 0.15 0.20 0.25 0.30 0.35 0.40 0.45 0.50 0.60 0.70 0.80 0.90 1.0);\n\
         {extra}\n{TABLES}"
    );
    let f = FoamFile::parse(&text).expect("the shipped table parses");
    LookUpTableChf::from_dict(&f.dict).expect("the shipped table is consistent")
}

/// Inputs with `x_e = xe`, `G = g`, at pressure `p` [Pa]; `Dh = 0.008` so `K1 = 1`.
fn inputs(p: f64, g: f64, xe: f64) -> LookUpTableChfInputs {
    let l = 1.0e6;
    LookUpTableChfInputs {
        flow_quality: 1.0,
        p,
        rho_liquid: 1000.0,
        rho_vapour: 10.0,
        mag_u_liquid: g / 1000.0,
        mag_u_vapour: 0.0,
        alpha_liquid: 1.0,
        dh: 0.008,
        h_liquid_saturated: 1.2e6,
        latent_heat: l,
        h_liquid: 1.2e6 + xe * l,
    }
}

#[test]
fn the_shipped_table_has_the_shape_its_axes_say() {
    let t = table_with("1.0", "");
    assert_eq!(t.pitch_to_diameter(), 1.0);
    // 15 pressures x 21 mass fluxes x 23 qualities
    let q = t.value(&inputs(1.0e5, 0.0, -0.5)).unwrap();
    assert!((q - 8111.0e3).abs() < 1e-6 * 8111.0e3, "first table entry, got {q}");
}

#[test]
fn a_knot_returns_the_table_entry_and_a_midpoint_the_mean() {
    let t = table_with("1.0", "");
    // p = 100 kPa, G = 0, x_e = 0.0 (index 7): 1142 kW/m^2
    let q = t.value(&inputs(1.0e5, 0.0, 0.0)).unwrap();
    assert!((q - 1142.0e3).abs() < 1e-6 * 1142.0e3, "{q}");
    // G = 25 is half way between the G = 0 and G = 50 rows: (8111 + 8317)/2 at x_e = -0.5
    let q = t.value(&inputs(1.0e5, 25.0, -0.5)).unwrap();
    assert!((q - 8214.0e3).abs() < 1e-6 * 8214.0e3, "{q}");
}

#[test]
fn the_rod_bundle_and_large_diameter_corrections() {
    let ptd = 1.32631578947;
    let t = table_with("1.32631578947", "");
    let q = t.value(&inputs(1.0e5, 0.0, 0.0)).unwrap();
    let k2 = 2.0 * ptd - 1.5; // exp(-|0|^3/2) = 1
    assert!((q - 1142.0e3 * k2).abs() < 1e-9 * q, "K2: {q}");
    // PitchToDiameter == 1: K1 = max(0.6, sqrt(0.008/Dh)); Dh = 0.032 gives sqrt(0.25) = 0.5 -> 0.6
    let t1 = table_with("1.0", "");
    let mut i = inputs(1.0e5, 0.0, 0.0);
    i.dh = 0.032;
    assert!((t1.value(&i).unwrap() - 1142.0e3 * 0.6).abs() < 1e-9 * 1142.0e3);
    // Dh = 0.002 gives sqrt(4) = 2
    i.dh = 0.002;
    assert!((t1.value(&i).unwrap() - 1142.0e3 * 2.0).abs() < 1e-9 * 1142.0e3);
}

#[test]
fn out_of_range_queries_are_upstreams_error_policy() {
    let t = table_with("1.0", "");
    for i in [inputs(5.0e4, 0.0, 0.0), inputs(1.0e5, 9000.0, 0.0), inputs(1.0e5, 0.0, 1.5)] {
        assert!(matches!(t.value(&i), Err(ChfTableError::Table(InterpolateTableError::OutOfBounds { .. }))));
    }
}

#[test]
fn a_missing_key_and_a_bad_method_are_reported() {
    let f = FoamFile::parse("pressureValues (1 2);").unwrap();
    assert!(matches!(LookUpTableChf::from_dict(&f.dict), Err(ChfTableError::Dict(_))));
    let text = format!(
        "PitchToDiameter 1; pressureInterpolationMethod cubic; pressureValues (1 2); massFlowRateValues (1 2); \
         qualityValues (1 2); data ( ((1 2)(3 4)) ((5 6)(7 8)) );"
    );
    let f = FoamFile::parse(&text).unwrap();
    assert!(matches!(LookUpTableChf::from_dict(&f.dict), Err(ChfTableError::Dict(_))));
    let text = text.replace("cubic", "step");
    let f = FoamFile::parse(&text).unwrap();
    let t = LookUpTableChf::from_dict(&f.dict).unwrap();
    // a 1 kPa pressure query lies at the first knot: the step table returns the first plane
    let i = inputs(1000.0, 1.0, 1.0);
    assert!(t.value(&i).is_ok(), "{:?}", t.value(&i));
    // mismatched axes are rejected at construction
    let bad = text.replace("qualityValues (1 2)", "qualityValues (1 2 3)");
    let f = FoamFile::parse(&bad).unwrap();
    assert!(LookUpTableChf::from_dict(&f.dict).is_err());
}

#[test]
fn the_field_valued_interpolation_agrees_with_the_scalar_table() {
    let xs = vec![0.0, 1.0, 2.5, 4.0];
    let ys = vec![3.0, -1.0, 7.0, 2.0];
    let scalar = ScalarInterpolateTable::new(xs.clone(), ys.clone(), InterpolationMethod::Linear, OutOfBounds::Extrapolation)
        .unwrap();
    // a field-valued table holding the scalar table in its first element and its negative in the second
    let fields: Vec<Vec<f64>> = ys.iter().map(|y| vec![*y, -*y]).collect();
    check_table(&xs, fields.len()).unwrap();
    for x in [-0.5, 0.0, 0.3, 1.0, 1.7, 2.5, 3.9, 4.0, 5.0] {
        let v = interpolate_table(&xs, &fields, InterpolationMethod::Linear, OutOfBounds::Extrapolation, x).unwrap();
        let s = scalar.interpolate(x).unwrap();
        assert_eq!(v[0], s, "x = {x}");
        assert_eq!(v[1], -s, "x = {x}");
        let step = interpolate_table(&xs, &ys, InterpolationMethod::Step, OutOfBounds::Fixed, x).unwrap();
        assert!(ys.contains(&step));
    }
    assert!(check_table(&[1.0, 1.0], 2).is_err());
    assert!(check_table(&[1.0], 1).is_err());
    assert!(check_table(&[1.0, 2.0], 3).is_err());
}

#[test]
fn cachard_upstream_binds_the_pitch_to_the_pressure() {
    // alpha = 0: the film has zero thickness whatever `pi` is
    assert_eq!(film_thickness(0.01, 0.0, 1.5e7), 0.0);
    let dh = 0.0078395462;
    let alpha = 0.3;
    // upstream: pi = pressure: (p/0.01)^2 ~ 2e18, an enormous film
    let upstream = film_thickness(dh, alpha, 1.5e7);
    assert!(upstream > 1.0e5, "{upstream}");
    // the intended form: a 12.7 mm pitch
    let pitch = 0.0127_f64;
    let r = pitch / 0.01;
    let want = dh / 2.0 * ((1.0 + alpha * ((4.0 / core::f64::consts::PI) * (r * r) - 1.0)).sqrt() - 1.0);
    assert!((film_thickness(dh, alpha, pitch) - want).abs() < 1e-12 * want);
    // the vapour model: h = 2 k / max(delta, 1e-6)
    let v = CachardVapour::new();
    assert_eq!(v.pitch, CachardPitch::UpstreamPressure);
    let h_up = v.value(alpha, 1.5e7, 0.05, dh);
    assert!((h_up - 2.0 * 0.05 / upstream).abs() < 1e-12 * h_up);
    let h_fix = CachardVapour::default().with_pitch(pitch).value(alpha, 1.5e7, 0.05, dh);
    assert!((h_fix - 2.0 * 0.05 / film_thickness(dh, alpha, pitch)).abs() < 1e-9 * h_fix);
    // zero film: the 1e-6 floor
    assert_eq!(v.value(0.0, 1.5e7, 0.05, dh), 2.0 * 0.05 / 1e-6);
}

#[test]
fn cachard_liquid_is_radiation_plus_film_conduction_and_writes_h_gamma() {
    let m = CachardLiquid::new(0.84, 0.95);
    let i = CachardLiquidInputs {
        t_wall: 900.0,
        t_sat: 615.0,
        t_liquid: 600.0,
        alpha_vapour: 0.3,
        p: 1.47e7,
        rho_liquid: 600.0,
        rho_vapour: 100.0,
        kappa_vapour: 0.08,
        mu_vapour: 3e-5,
        dh_vapour: 0.0078,
    };
    let (h, h_gamma) = m.value(&i);
    // radiation term by hand
    let a = (0.95 * (1.0_f64 - 0.3).sqrt()).max(1e-6);
    let b = 1.0 / 0.84 - 1.0;
    let rad = 5.670e-8 * (900.0 * 900.0 + 615.0 * 615.0) * (900.0 + 615.0) / (1.0 / a + b).max(1e-6);
    assert!(h_gamma >= 0.0);
    assert!((h - (h_gamma + rad)).abs() < 1e-9 * h.abs().max(1.0), "h {h} = {h_gamma} + {rad}");
    assert!(rad > 10.0 && rad < 1.0e3, "radiation htc {rad}");
    // the intended pitch gives a finite film and so a finite conduction term; upstream's gives ~ 0
    let fixed = m.with_pitch(0.0127);
    assert_eq!(fixed.pitch, CachardPitch::Pitch(0.0127));
    let (_, hg_fixed) = fixed.value(&i);
    assert!(hg_fixed >= h_gamma);
    // wall below the liquid: dT clamps with the sign
    let mut cold = i;
    cold.t_wall = 590.0;
    let (hc, _) = m.value(&cold);
    assert!(hc.is_finite());
}
