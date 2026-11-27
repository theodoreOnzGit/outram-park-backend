// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
//! # The hydrogen properties against the real `Foam::H2` class (gh:#1872, round 50)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da` on OpenFOAM-ESI v2506. `upstream_driver/h2_dump/h2Dump.C` constructs the real
//! `Foam::H2` (unmodified `libFFSSrc2`) and prints every public method at `%.17g` on 47 temperatures
//! (14 K .. 10 000 K) x 10 pressures (1 kPa .. 100 MPa) = 470 points (`scripts/build_and_run_h2_dump.sh`,
//! reference `refs/h2_dump.ref.txt.gz`). This module evaluates the port's functions on the same points and
//! compares `to_bits`. It is a child of `hydrogen`, so it reaches the `pub(super)` internals that the public
//! enum does not expose.
//!
//! **Pass criterion:** bit identity for every ported property at every point, with NaN equal to NaN; a residual
//! needs the 1e-9 allowance and an explained cause (here: petir/musl `cbrt` and `cos` against glibc, amplified by the cubic root's cancellation).
//! **Controls:** a 1-ulp change of `p` or `T` must be seen on a fixed fraction of points.
//! Predictions and results: V&V README, "Round 50".

use super::collision_integrals::{
    a12, b12, c12, omega11_11, omega11_12, omega22_11, omega22_12, omega22_22,
};
use super::diffusion::{binary_diffusion_h_h2, self_diffusion_h2};
use super::dissociation::{equilibrium_constant_kp, mass_fraction_atomic_h, mole_fraction_atomic_h};
use super::equation_of_state::{
    compressibility, cp_minus_cv, density, density_atomic_h, density_molecular_h2, ReducedCubic,
};
use super::heat_capacity::{
    cp_perfect_gas_h2, cv_perfect_gas_h2, heat_capacity_atomic_h, heat_capacity_molecular_h2, second_virial_coefficient,
    specific_heat_capacity,
};
use super::thermal_conductivity::{
    molecular_h2_thermal_conductivity, prandtl_number, thermal_conductivity, thermal_diffusivity,
};
use super::thermodynamics::{
    chemical_enthalpy, entropy_molecular_h2, ideal_entropy_atomic_h, ideal_entropy_molecular_h2, specific_enthalpy,
    specific_enthalpy_atomic_h, specific_enthalpy_molecular_h2, specific_entropy,
};
use super::viscosity::{atomic_hydrogen_viscosity, dynamic_viscosity, excess_viscosity, zero_density_viscosity};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// One grid point of the reference: `(p, T, [values in the order of the header])`.
struct Point {
    p: f64,
    t: f64,
    v: Vec<f64>,
}

fn gunzip(bytes: &[u8]) -> Vec<u8> {
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
    miniz_oxide::inflate::decompress_to_vec(&bytes[at..bytes.len() - 8]).expect("inflate h2 dump")
}

fn parse_f64(s: &str) -> f64 {
    match s {
        "nan" | "-nan" => f64::NAN,
        "inf" => f64::INFINITY,
        "-inf" => f64::NEG_INFINITY,
        _ => s.parse().unwrap_or_else(|_| panic!("number {s}")),
    }
}

/// Reads `refs/h2_dump.ref.txt.gz`: the constants (`K` lines), the column names and the grid points.
fn load() -> (BTreeMap<String, f64>, Vec<String>, Vec<Point>) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/genfoam_vs_upstream_gh1361/refs/h2_dump.ref.txt.gz");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let text = String::from_utf8(gunzip(&bytes)).expect("utf8");
    let mut consts = BTreeMap::new();
    let mut names = Vec::new();
    let mut points = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("K") => {
                let k = it.next().unwrap().to_string();
                consts.insert(k, parse_f64(it.next().unwrap()));
            }
            Some("N") => names = it.map(str::to_string).collect(),
            Some("P") => {
                let p = parse_f64(it.next().unwrap());
                let t = parse_f64(it.next().unwrap());
                points.push(Point { p, t, v: it.map(parse_f64).collect() });
            }
            _ => {}
        }
    }
    (consts, names, points)
}

/// The port's value of the upstream property `name` at `(p, t)`; `None` where the port deliberately has no such
/// function (see the module docs of `hydrogen`; recorded in the V&V README).
fn port_value(name: &str, p: f64, t: f64) -> Option<f64> {
    Some(match name {
        "Kp" => equilibrium_constant_kp(t),
        "x_H" => mole_fraction_atomic_h(p, t),
        "w_H" => mass_fraction_atomic_h(p, t),
        "omega11_11" => omega11_11(t),
        "omega22_11" => omega22_11(t),
        "omega22_22" => omega22_22(t),
        "C_12" => c12(t),
        "psi" => compressibility(p, t),
        "diffusionH2" => self_diffusion_h2(p, t),
        "cvPerfectGasH2" => cv_perfect_gas_h2(t),
        "Hc" => chemical_enthalpy(),
        "omega11_12" => omega11_12(t),
        "omega22_12" => omega22_12(t),
        "A_12" => a12(t),
        "B_12" => b12(t),
        "densityH" => density_atomic_h(p, t),
        "densityH2" => density_molecular_h2(p, t),
        "rho" => density(p, t),
        "CpMCv" => cp_minus_cv(p, t),
        "Hs" | "Ha" | "h" => specific_enthalpy(p, t),
        "idealEntropyH2" => ideal_entropy_molecular_h2(p, t),
        "idealEntropyH" => ideal_entropy_atomic_h(p, t),
        "entropyH2" => entropy_molecular_h2(p, t),
        "S" => specific_entropy(p, t),
        "specificEnthalpyH" => specific_enthalpy_atomic_h(t),
        "specificEnthalpyH2" => specific_enthalpy_molecular_h2(p, t),
        "cpPerfectGasH2" => cp_perfect_gas_h2(t),
        "heatCapacityH" => heat_capacity_atomic_h(t),
        "heatCapacityH2" => heat_capacity_molecular_h2(p, t),
        "Cp" => specific_heat_capacity(p, t),
        "B" => second_virial_coefficient(p, t),
        "viscosity0" => zero_density_viscosity(t),
        "viscosity1" => excess_viscosity(t),
        "viscosityH" => atomic_hydrogen_viscosity(t),
        "mu" => dynamic_viscosity(p, t),
        "thermalConductivityH2" => molecular_h2_thermal_conductivity(p, t),
        "kappa" => thermal_conductivity(p, t),
        "D" => binary_diffusion_h_h2(p, t),
        "alphah" => thermal_diffusivity(p, t),
        "Pr" => prandtl_number(p, t),
        _ => return None,
    })
}

/// Bit-identity tally of one property.
#[derive(Default, Debug, Clone)]
struct Tally {
    n: usize,
    exact: usize,
    worst_rel: f64,
    worst_at: String,
    nan_mismatch: usize,
}

fn same_bits(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

/// What the control does to the inputs before the port is called (an enum, not a closure object).
#[derive(Clone, Copy)]
enum Perturb {
    None,
    TemperatureUlp,
    PressureUlp,
}

impl Perturb {
    fn apply(self, p: f64, t: f64) -> (f64, f64) {
        let up = |x: f64| f64::from_bits(x.to_bits() + 1);
        match self {
            Self::None => (p, t),
            Self::TemperatureUlp => (p, up(t)),
            Self::PressureUlp => (up(p), t),
        }
    }
}

/// Compares every ported property over the grid; `perturb` transforms `(p, t)` before the port is called
/// (identity for the real comparison, a 1-ulp step for the control). Returns the tally per property name,
/// split into the tabulated region (`T <= 6000 K`, `p < 100 MPa`) and the extrapolation region.
fn compare(perturb: Perturb) -> (BTreeMap<String, Tally>, BTreeMap<String, Tally>) {
    let (_, names, points) = load();
    let mut tab: BTreeMap<String, Tally> = BTreeMap::new();
    let mut ext: BTreeMap<String, Tally> = BTreeMap::new();
    for pt in &points {
        let extrapolated = pt.t > 6000.0 || pt.p >= 1.0e8;
        let (pp, tt) = perturb.apply(pt.p, pt.t);
        for (name, &up) in names.iter().zip(&pt.v) {
            let Some(mine) = port_value(name, pp, tt) else { continue };
            let tally = if extrapolated { &mut ext } else { &mut tab }.entry(name.clone()).or_default();
            tally.n += 1;
            if mine.is_nan() != up.is_nan() {
                tally.nan_mismatch += 1;
            }
            if same_bits(mine, up) {
                tally.exact += 1;
            } else {
                let r = rel(mine, up);
                if r > tally.worst_rel || tally.worst_at.is_empty() {
                    tally.worst_rel = r;
                    tally.worst_at = format!("p={} T={} port={mine:e} upstream={up:e}", pt.p, pt.t);
                }
            }
        }
    }
    (tab, ext)
}

fn report(title: &str, tallies: &BTreeMap<String, Tally>) {
    eprintln!("--- {title}");
    for (name, t) in tallies {
        eprintln!(
            "{name:24} {:4}/{:4} bit-identical, worst rel {:.3e} nan-mismatch {} {}",
            t.exact, t.n, t.worst_rel, t.nan_mismatch, t.worst_at
        );
    }
}

#[test]
fn reference_has_the_expected_grid_and_every_method_of_the_real_class_is_translated() {
    let (consts, names, points) = load();
    assert_eq!(points.len(), 470, "47 temperatures x 10 pressures");
    assert_eq!(names.len(), 42);
    for pt in &points {
        assert_eq!(pt.v.len(), names.len());
    }
    for name in &names {
        let ported = port_value(name, 1.0e6, 500.0).is_some();
        assert!(ported, "{name} of the real class is not translated");
    }
    // the class constants the port derives (n_, alpha0_, b_, c_) are the real class' values, bit for bit
    assert_eq!(super::constants::n().to_bits(), consts["n_"].to_bits(), "n_");
    assert_eq!(super::constants::alpha0().to_bits(), consts["alpha0_"].to_bits(), "alpha0_");
    assert_eq!(super::constants::b().to_bits(), consts["b_"].to_bits(), "b_");
    assert_eq!(super::constants::c().to_bits(), consts["c_"].to_bits(), "c_");
    assert_eq!(super::constants::R.to_bits(), consts["R_"].to_bits());
    assert_eq!(super::constants::W.to_bits(), consts["W"].to_bits());
    assert_eq!(super::constants::TC.to_bits(), consts["Tc"].to_bits());
    assert_eq!(super::constants::PC.to_bits(), consts["Pc"].to_bits());
    assert_eq!(super::constants::VC.to_bits(), consts["Vc"].to_bits());
    assert_eq!(super::constants::OMEGA.to_bits(), consts["omega"].to_bits(), "acentric factor");
}

/// Properties that go through the cubic equation of state's root (`densityH2`, by Cardano's cube roots or the
/// trigonometric solution), with the number of grid points (tabulated region, extrapolation region) at which the
/// port is bit-identical to the real class. The remaining points differ because petir's `cbrt` and `cos`
/// (the `libm` crate, musl) round differently from glibc's (measured: `cbrt` differs on 47.6 % of 200 000 random
/// arguments, `cos` on the arguments of the cubic), by one ulp, and the root is `a + b - c` with a cancellation of
/// up to ~1e6 at low pressure (`2*sqrt(-p/3)*cos(..) - B/(3A)` = 1.4e-3 from terms of 655), so one ulp becomes up to
/// 1.6e-10 relative. Every other property (T-only, `x_H`, mixture rules, `ln`/`exp`/`pow` chains) is bit-identical
/// at every point. The pins are a floor: a change that loses a bit-identical point fails; one that gains is welcome.
const CUBIC_DEPENDENT: [(&str, usize, usize); 17] = [
    ("psi", 287, 68),
    ("densityH2", 287, 68),
    ("rho", 287, 68),
    ("CpMCv", 307, 69),
    ("Ha", 291, 70),
    ("Hs", 291, 70),
    ("h", 291, 70),
    ("entropyH2", 300, 69),
    ("S", 300, 69),
    ("specificEnthalpyH2", 291, 70),
    ("heatCapacityH2", 292, 69),
    ("Cp", 292, 69),
    ("thermalConductivityH2", 301, 68),
    ("kappa", 308, 68),
    ("mu", 319, 69),
    ("alphah", 289, 68),
    ("Pr", 289, 68),
];

#[test]
fn every_ported_property_is_bit_identical_to_the_real_class_on_the_grid() {
    let (tab, ext) = compare(Perturb::None);
    report("tabulated region (T <= 6000 K, p < 100 MPa)", &tab);
    report("extrapolation region (T > 6000 K or p = 100 MPa)", &ext);
    let cubic = |name: &str| CUBIC_DEPENDENT.iter().find(|(n, _, _)| *n == name);
    let mut bad = Vec::new();
    for (name, t) in tab.iter().chain(ext.iter()) {
        assert_eq!(t.nan_mismatch, 0, "{name}: NaN/finite mismatch");
        assert!(t.worst_rel < 1e-9, "{name}: {:.3e} exceeds the 1e-9 round-off allowance", t.worst_rel);
        if cubic(name).is_none() && t.exact != t.n {
            bad.push(format!("{name}: {}/{} worst {:.3e} at {}", t.exact, t.n, t.worst_rel, t.worst_at));
        }
    }
    for (name, tab_min, ext_min) in CUBIC_DEPENDENT {
        assert!(tab[name].exact >= tab_min, "{name}: {} < pinned {tab_min} (tabulated)", tab[name].exact);
        assert!(ext[name].exact >= ext_min, "{name}: {} < pinned {ext_min} (extrapolation)", ext[name].exact);
    }
    assert!(bad.is_empty(), "not bit-identical:\n{}", bad.join("\n"));
}

/// Both branches of the cubic are exercised by the grid (so the two sources of the residual above are real), and
/// the compile-time-folded `pow(0.297, 3.0)` of the real class equals petir's runtime `powf`.
#[test]
fn the_grid_covers_both_cubic_branches_and_the_folded_constant_matches() {
    let (consts, _, points) = load();
    let trig = points.iter().filter(|pt| ReducedCubic::at(pt.p, pt.t).discriminant < 0.0).count();
    eprintln!("cubic branches: {trig} trigonometric, {} Cardano of {}", points.len() - trig, points.len());
    assert!(trig > 0 && trig < points.len());
    use petir::mathf::RealMath;
    assert_eq!(0.297f64.r_powf(3.0).to_bits(), consts["pow_0p297_3"].to_bits());
}

/// Control: the comparison resolves the last bit. One ulp of `T` (or `p`) must change at least one point of
/// every property that depends on it (the rest of the tally is printed).
#[test]
fn control_one_ulp_of_the_input_is_detected() {
    let (tab_t, _) = compare(Perturb::TemperatureUlp);
    let (tab_p, _) = compare(Perturb::PressureUlp);
    report("control: T + 1 ulp", &tab_t);
    report("control: p + 1 ulp", &tab_p);
    for name in ["Kp", "rho", "Cp", "h", "S", "mu", "kappa", "B"] {
        assert!(tab_t[name].exact < tab_t[name].n, "{name} did not see T + 1 ulp");
    }
    for name in ["x_H", "rho", "Cp", "h", "S", "mu", "kappa"] {
        assert!(tab_p[name].exact < tab_p[name].n, "{name} did not see p + 1 ulp");
    }
}

