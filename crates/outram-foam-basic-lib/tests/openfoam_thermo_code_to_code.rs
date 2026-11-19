//! Thermophysics differential test against upstream OpenFOAM-v2506 (gh:#1362, row 4).
//!
//! # Method
//! `verification_and_validation/upstream_thermo_gh1362/driver/thermoDump.C` builds
//! every `thermo x equationOfState x transport` combination from a dictionary
//! on the UNMODIFIED v2506 (ESI) `libspecie` templates (hConst, janaf,
//! hPolynomial, hTabulated x perfectGas, rhoConst, icoPolynomial, PengRobinson
//! x const, sutherland, polynomial, tabulated transport) and prints the IEEE-754
//! bit pattern of Cp, Ha, Hs, Hc, S, rho, psi, Z, CpMCv, mu, kappa, alphah on a
//! 10 T x 4 p grid, plus an `extrap/` block (T outside every table and the
//! JANAF range). This test builds the same models through the port's
//! constructors and compares bit for bit.
//!
//! # Criterion
//! Bit-identity. For each quantity the test counts the bit-identical values,
//! the worst relative difference and the worst ulp distance, prints them
//! (`--nocapture`), and PINS them in both directions.

use outram_foam_basic_lib::polynomial::Polynomial;
use outram_foam_basic_lib::thermophysics::constants::GasConstant;
use outram_foam_basic_lib::thermophysics::eos::{
    EquationOfState, IcoPolynomial, PengRobinsonForm, PengRobinsonGas, PerfectGas, RhoConst,
};
use outram_foam_basic_lib::thermophysics::imports::*;
use outram_foam_basic_lib::thermophysics::thermo::{
    HConstThermo, HPolynomialThermo, HTabulatedEsiThermo, JanafThermo, ThermoModel,
};
use outram_foam_basic_lib::thermophysics::transport::{
    ConstTransport, PolynomialTransport, SutherlandTransport, TabulatedTransport, TransportModel,
};
use std::collections::BTreeMap;

const REF_GZ: &[u8] =
    include_bytes!("../verification_and_validation/upstream_thermo_gh1362/reference/thermo_esi_v2506.txt.gz");

const TS: [f64; 10] = [250.0, 280.0, 300.0, 350.0, 400.0, 600.0, 900.0, 1100.0, 1500.0, 1900.0];
const TS_EXTRAP: [f64; 5] = [120.0, 150.0, 2400.0, 3000.0, 6500.0];
const PS: [f64; 4] = [1e5, 1e6, 3e6, 8e6];
const QNAMES: [&str; 12] = [
    "Cp", "Ha", "Hs", "Hc", "S", "rho", "psi", "Z", "CpMCv", "mu", "kappa", "alphah",
];
const SUTH_AS: f64 = 1.458e-6;
const SUTH_TS: f64 = 110.4;

type Refs = BTreeMap<String, Vec<[u64; 12]>>;

fn gunzip(data: &[u8]) -> String {
    // `gzip -n`: 10-byte header, deflate stream, 8-byte trailer.
    let body = &data[10..data.len() - 8];
    let raw = miniz_oxide::inflate::decompress_to_vec(body).expect("inflate");
    String::from_utf8(raw).expect("utf8")
}

/// tag -> rows (ti * 4 + ip) of 12 bit patterns.
fn reference() -> Refs {
    let mut map: Refs = BTreeMap::new();
    for line in gunzip(REF_GZ).lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let mut it = line.split_whitespace();
        let tag = it.next().unwrap().to_string();
        let _ti = it.next();
        let _pi = it.next();
        let mut row = [0u64; 12];
        for slot in row.iter_mut() {
            *slot = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
        }
        map.entry(tag).or_default().push(row);
    }
    map
}

fn ulp_distance(a: f64, b: f64) -> u64 {
    if a.to_bits() == b.to_bits() {
        return 0;
    }
    if a.is_nan() || b.is_nan() || (a.is_sign_negative() != b.is_sign_negative()) {
        return u64::MAX;
    }
    a.to_bits().abs_diff(b.to_bits())
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Stat {
    identical: usize,
    total: usize,
    worst_rel: f64,
    worst_ulp: u64,
}

fn q(m: &impl TransportModel, has_s: bool, t: f64, p: f64) -> [f64; 12] {
    let pp = Pressure::new::<pascal>(p);
    let tt = ThermodynamicTemperature::new::<kelvin>(t);
    [
        m.cp(pp, tt).get::<joule_per_kilogram_kelvin>(),
        m.ha(pp, tt).get::<joule_per_kilogram>(),
        m.hs(pp, tt).get::<joule_per_kilogram>(),
        m.hc().get::<joule_per_kilogram>(),
        if has_s { m.s(pp, tt).get::<joule_per_kilogram_kelvin>() } else { 0.0 },
        m.rho(pp, tt).get::<kilogram_per_cubic_meter>(),
        m.psi(pp, tt).value,
        m.z(pp, tt).get::<ratio>(),
        m.cp_m_cv(pp, tt).get::<joule_per_kilogram_kelvin>(),
        m.mu(pp, tt).get::<pascal_second>(),
        m.kappa(pp, tt).get::<watt_per_meter_kelvin>(),
        m.alpha_h(pp, tt).get::<pascal_second>(),
    ]
}

type Results = BTreeMap<String, [Stat; 12]>;

struct Setup<'a> {
    refs: &'a Refs,
    ts: &'a [f64],
    esi_forms: bool,
}

fn compare(results: &mut Results, su: &Setup, tag: &str, m: &impl TransportModel, has_s: bool) {
    let rows = su.refs.get(tag).unwrap_or_else(|| panic!("no reference for {tag}"));
    assert_eq!(rows.len(), su.ts.len() * 4, "{tag}");
    let mut stats = [Stat::default(); 12];
    for (it, t) in su.ts.iter().enumerate() {
        // Above the last table knot upstream's nonUniformTable reads one past the
        // end of its list (undefined behaviour), so those rows are not a reference.
        let upstream_ub = *t > 2000.0 && (tag.contains("hTabulated") || tag.ends_with("/tabulated"));
        if upstream_ub {
            continue;
        }
        for (ip, p) in PS.iter().enumerate() {
            let mine = q(m, has_s, *t, *p);
            let theirs = &rows[it * 4 + ip];
            for k in 0..12 {
                let u = f64::from_bits(theirs[k]);
                let s = &mut stats[k];
                s.total += 1;
                let d = ulp_distance(mine[k], u);
                if d == 0 {
                    s.identical += 1;
                } else {
                    let rel = ((mine[k] - u) / u).abs();
                    let rel = if rel.is_nan() { f64::INFINITY } else { rel };
                    if rel > s.worst_rel {
                        s.worst_rel = rel;
                    }
                    if d > s.worst_ulp {
                        s.worst_ulp = d;
                    }
                }
            }
        }
    }
    results.insert(tag.to_string(), stats);
}

fn transports<T: ThermoModel + Clone>(
    results: &mut Results,
    su: &Setup,
    prefix: &str,
    th: T,
    has_s: bool,
) {
    let c = ConstTransport::new(
        th.clone(),
        DynamicViscosity::new::<pascal_second>(1.8e-5),
        Ratio::new::<ratio>(0.7),
    );
    compare(results, su, &format!("{prefix}/const"), &c, has_s);
    let s = SutherlandTransport::new(th.clone(), SUTH_AS, SUTH_TS);
    compare(results, su, &format!("{prefix}/sutherland"), &s, has_s);
    let p = PolynomialTransport::new(
        th.clone(),
        Polynomial::<8>::new([1.0e-5, 4.0e-8, -1.0e-11, 0.0, 0.0, 0.0, 0.0, 0.0]),
        Polynomial::<8>::new([0.01, 6.0e-5, -1.0e-8, 0.0, 0.0, 0.0, 0.0, 0.0]),
    );
    compare(results, su, &format!("{prefix}/polynomial"), &p, has_s);
    let mu_t = vec![200.0, 500.0, 1000.0, 2000.0];
    let mu_v = vec![1.0e-5, 2.4e-5, 4.0e-5, 6.5e-5];
    let k_t = vec![200.0, 600.0, 1200.0, 2000.0];
    let k_v = vec![0.02, 0.045, 0.08, 0.12];
    let t = if su.esi_forms {
        TabulatedTransport::new_esi_v2506(
            th,
            mu_t.iter().copied().zip(mu_v.iter().copied()).collect(),
            k_t.iter().copied().zip(k_v.iter().copied()).collect(),
        )
    } else {
        TabulatedTransport::new(th, (mu_t, mu_v), (k_t, k_v))
    };
    compare(results, su, &format!("{prefix}/tabulated"), &t, has_s);
}

fn thermos<E: EquationOfState + Clone>(
    results: &mut Results,
    su: &Setup,
    pre: &str,
    eos_name: &str,
    eos: E,
) {
    let hc = HConstThermo::new(
        eos.clone(),
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1004.5),
        AvailableEnergy::new::<joule_per_kilogram>(2.5e5),
        ThermodynamicTemperature::new::<kelvin>(298.15),
        AvailableEnergy::new::<joule_per_kilogram>(0.0),
    );
    transports(results, su, &format!("{pre}hConst/{eos_name}"), hc, true);

    let r = eos.r().get::<joule_per_kilogram_kelvin>();
    let high = [
        2.95257637, 1.3969004e-3, -4.92631603e-7, 7.8600091e-11, -4.60755204e-15, -923.948688,
        5.87188762,
    ]
    .map(|a: f64| a * r);
    let low = [
        3.53100528, -1.23660988e-4, -5.02999433e-7, 2.43530612e-9, -1.40881235e-12, -1046.97628,
        2.96747038,
    ]
    .map(|a: f64| a * r);
    let jn = JanafThermo::new(eos.clone(), 200.0, 6000.0, 1000.0, low, high)
        .with_temperature_clamp(!su.esi_forms);
    transports(results, su, &format!("{pre}janaf/{eos_name}"), jn, true);

    let cps = Polynomial::<8>::new([1000.0, 0.1, -2.0e-5, 3.0e-9, 0.0, 0.0, 0.0, 0.0]);
    let hf = AvailableEnergy::new::<joule_per_kilogram>(1.0e5);
    let hp = if su.esi_forms {
        HPolynomialThermo::new_esi_v2506(
            eos.clone(),
            cps,
            hf,
            ThermodynamicTemperature::new::<kelvin>(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.0),
        )
    } else {
        HPolynomialThermo::new(
            eos.clone(),
            cps,
            hf,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(200.0),
        )
    };
    transports(results, su, &format!("{pre}hPolynomial/{eos_name}"), hp, true);

    if su.esi_forms {
        // S is not compared: upstream v2506 cannot compile hTabulatedThermo::S.
        let ht = HTabulatedEsiThermo::new(
            eos,
            vec![
                (200.0, 1000.0),
                (400.0, 1020.0),
                (800.0, 1100.0),
                (1200.0, 1180.0),
                (2000.0, 1250.0),
            ],
            hf,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(200.0),
        );
        transports(results, su, &format!("{pre}hTabulated/{eos_name}"), ht, false);
    }
}

/// `W` in kg/mol such that `W * 1e3` is exactly the kg/kmol value upstream
/// reads. A `gram_per_mole` input is stored as `W * 0.001`, and
/// `(W * 0.001) * 1e3 != W` for some W (28.96 is one): a unit-conversion ulp,
/// not an arithmetic difference. Searches the neighbouring doubles.
fn exact_w(w_kmol: f64) -> MolarMass {
    let w0 = w_kmol * 1.0e-3;
    for d in 0..8i64 {
        for sgn in [1i64, -1] {
            let cand = f64::from_bits((w0.to_bits() as i64 + sgn * d) as u64);
            if cand * 1.0e3 == w_kmol {
                return MolarMass::new::<kilogram_per_mole>(cand);
            }
        }
    }
    panic!("no exact kg/mol for {w_kmol}");
}

fn run(gc: GasConstant, esi_forms: bool, exact: bool) -> Results {
    let refs = reference();
    let mut results = Results::new();
    let air = || if exact { exact_w(28.96) } else { MolarMass::new::<gram_per_mole>(28.96) };
    let co2 = || if exact { exact_w(44.01) } else { MolarMass::new::<gram_per_mole>(44.01) };
    let form = if esi_forms { PengRobinsonForm::EsiV2506 } else { PengRobinsonForm::Port };
    let su = Setup { refs: &refs, ts: &TS, esi_forms };
    thermos(&mut results, &su, "", "perfectGas", PerfectGas::new(air()).with_gas_constant(gc));
    thermos(
        &mut results,
        &su,
        "",
        "rhoConst",
        RhoConst::new(air(), MassDensity::new::<kilogram_per_cubic_meter>(998.2))
            .with_gas_constant(gc),
    );
    thermos(
        &mut results,
        &su,
        "",
        "icoPolynomial",
        IcoPolynomial::<8>::new(
            air(),
            Polynomial::<8>::new([1010.0, -0.4, -2.0e-3, 1.0e-6, 0.0, 0.0, 0.0, 0.0]),
        )
        .with_gas_constant(gc),
    );
    thermos(
        &mut results,
        &su,
        "",
        "PengRobinson",
        PengRobinsonGas::new(
            co2(),
            ThermodynamicTemperature::new::<kelvin>(304.12),
            Pressure::new::<pascal>(7.374e6),
            0.2239,
        )
        .with_gas_constant(gc)
        .with_form(form),
    );
    // extrapolation block: T outside every table and the JANAF range
    let ex = Setup { refs: &refs, ts: &TS_EXTRAP, esi_forms };
    thermos(
        &mut results,
        &ex,
        "extrap/",
        "perfectGas",
        PerfectGas::new(air()).with_gas_constant(gc),
    );
    results
}

fn report(label: &str, gc: GasConstant, esi_forms: bool, exact: bool) {
    let results = run(gc, esi_forms, exact);
    println!("=== {label}");
    for (tag, st) in &results {
        let mut line = format!("{tag:42}");
        for (k, s) in st.iter().enumerate() {
            if s.identical != s.total {
                line.push_str(&format!(
                    " {}:{}/{} ulp{} rel{:.1e}",
                    QNAMES[k], s.identical, s.total, s.worst_ulp, s.worst_rel
                ));
            }
        }
        println!("{line}");
    }
    for tag in PIN_TAGS {
        println!("PIN {label} | {tag} | {:?}", ident(&results, tag));
    }
}

const PIN_TAGS: [&str; 8] = [
    "hConst/perfectGas/const",
    "hPolynomial/perfectGas/const",
    "janaf/perfectGas/const",
    "hConst/rhoConst/const",
    "hConst/PengRobinson/const",
    "janaf/PengRobinson/tabulated",
    "extrap/janaf/perfectGas/const",
    "extrap/hConst/perfectGas/tabulated",
];

fn ident(results: &Results, tag: &str) -> [usize; 12] {
    let st = &results[tag];
    let mut out = [0usize; 12];
    for k in 0..12 {
        out[k] = st[k].identical;
    }
    out
}

#[test]
fn thermo_vs_openfoam_report() {
    report("default port", GasConstant::Codata2019, false, false);
    report("ESI forms, gram_per_mole W", GasConstant::EsiV2506, true, false);
    report("ESI forms, exact W", GasConstant::EsiV2506, true, true);
}

/// Every PengRobinson (ESI form, exact W) mismatch sits on a row where `Z`
/// itself differs: `Z` is the only quantity with `acos`, `cos` and `cbrt`,
/// whose PETIR and glibc results differ by 1 ulp at some points. A python
/// replay of upstream's `Z` with glibc reproduces all 40 reference values
/// (scratch check, recorded in the README), so the closed form is translated
/// correctly and only the libm differs.
#[test]
fn peng_robinson_mismatches_follow_z() {
    let refs = reference();
    let rows = &refs["hConst/PengRobinson/const"];
    let pr = PengRobinsonGas::new(
        exact_w(44.01),
        ThermodynamicTemperature::new::<kelvin>(304.12),
        Pressure::new::<pascal>(7.374e6),
        0.2239,
    )
    .with_gas_constant(GasConstant::EsiV2506)
    .with_form(PengRobinsonForm::EsiV2506);
    let m = ConstTransport::new(
        HConstThermo::new(
            pr,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1004.5),
            AvailableEnergy::new::<joule_per_kilogram>(2.5e5),
            ThermodynamicTemperature::new::<kelvin>(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        ),
        DynamicViscosity::new::<pascal_second>(1.8e-5),
        Ratio::new::<ratio>(0.7),
    );
    let (mut z_diff, mut other_diff_with_z_same) = (0usize, 0usize);
    for (it, t) in TS.iter().enumerate() {
        for (ip, p) in PS.iter().enumerate() {
            let mine = q(&m, true, *t, *p);
            let theirs = &rows[it * 4 + ip];
            if mine[7].to_bits() != theirs[7] {
                z_diff += 1;
            } else if (0..12).any(|k| mine[k].to_bits() != theirs[k]) {
                other_diff_with_z_same += 1;
            }
        }
    }
    println!(
        "PR: rows with Z differing {z_diff}/40, other-quantity differences with Z identical {other_diff_with_z_same}"
    );
    assert_eq!((z_diff, other_diff_with_z_same), (10, 0));
}

/// Pinned counts (bit-identical values out of 40, or out of 20 / 8 for the
/// `extrap/` rows), measured 2026-11-13 in release. The order is
/// `Cp Ha Hs Hc S rho psi Z CpMCv mu kappa alphah`.
const PINS_DEFAULT: [(&str, [usize; 12]); 8] = [
    // R = CODATA 2019 in the port against upstream's 9.0e-7 larger constant: rho, psi, CpMCv never match.
    ("hConst/perfectGas/const", [40, 40, 40, 40, 10, 0, 0, 40, 0, 40, 40, 40]),
    // Foundation hPolynomial form: Ha, Hs, S differ from ESI structurally.
    ("hPolynomial/perfectGas/const", [40, 0, 0, 40, 0, 0, 0, 40, 0, 40, 40, 40]),
    ("janaf/perfectGas/const", [0, 0, 0, 0, 0, 0, 0, 40, 0, 40, 0, 40]),
    ("hConst/rhoConst/const", [40; 12]),
    ("hConst/PengRobinson/const", [0, 0, 0, 40, 0, 0, 0, 25, 0, 40, 0, 40]),
    ("janaf/PengRobinson/tabulated", [0, 0, 0, 0, 0, 0, 0, 25, 0, 40, 40, 0]),
    ("extrap/janaf/perfectGas/const", [0, 0, 0, 0, 0, 0, 0, 20, 0, 20, 0, 20]),
    // clamped table lookup against upstream's linear extrapolation: mu, kappa never match.
    ("extrap/hConst/perfectGas/tabulated", [8, 8, 8, 8, 2, 0, 0, 8, 0, 0, 0, 0]),
];

/// ESI forms, `W` entered through `gram_per_mole`: the residual is the
/// unit-conversion ulp (`(28.96 * 0.001) * 1e3 != 28.96`), in R and so in rho,
/// psi, CpMCv, S and the JANAF coefficients scaled by R.
const PINS_ESI_GRAM: [(&str, [usize; 12]); 8] = [
    ("hConst/perfectGas/const", [40, 40, 40, 40, 11, 4, 0, 40, 0, 40, 40, 40]),
    ("hPolynomial/perfectGas/const", [40, 40, 40, 40, 14, 4, 0, 40, 0, 40, 40, 40]),
    ("janaf/perfectGas/const", [0, 0, 4, 40, 2, 4, 0, 40, 0, 40, 16, 40]),
    ("hConst/rhoConst/const", [40; 12]),
    ("hConst/PengRobinson/const", [35, 38, 33, 40, 36, 31, 31, 30, 35, 40, 36, 40]),
    ("janaf/PengRobinson/tabulated", [35, 33, 33, 40, 39, 31, 31, 30, 35, 40, 40, 35]),
    ("extrap/janaf/perfectGas/const", [4, 8, 8, 20, 7, 4, 0, 20, 0, 20, 4, 20]),
    ("extrap/hConst/perfectGas/tabulated", [8, 8, 8, 8, 3, 2, 0, 8, 0, 8, 8, 8]),
];

/// ESI forms and an exactly round-tripping `W`: everything is bit-identical
/// except PengRobinson, whose residual is PETIR against glibc `acos`, `cos`,
/// `cbrt` and `log` (see `peng_robinson_mismatches_follow_z`).
const PINS_ESI_EXACT: [(&str, [usize; 12]); 8] = [
    ("hConst/perfectGas/const", [40; 12]),
    ("hPolynomial/perfectGas/const", [40; 12]),
    ("janaf/perfectGas/const", [40; 12]),
    ("hConst/rhoConst/const", [40; 12]),
    ("hConst/PengRobinson/const", [35, 38, 33, 40, 36, 31, 31, 30, 35, 40, 36, 40]),
    ("janaf/PengRobinson/tabulated", [35, 33, 33, 40, 39, 31, 31, 30, 35, 40, 40, 35]),
    ("extrap/janaf/perfectGas/const", [20; 12]),
    ("extrap/hConst/perfectGas/tabulated", [8; 12]),
];

fn check_pins(results: &Results, pins: &[(&str, [usize; 12])]) {
    for (tag, expected) in pins {
        assert_eq!(&ident(results, tag), expected, "{tag}");
    }
}

#[test]
fn thermo_vs_openfoam_pins() {
    check_pins(&run(GasConstant::Codata2019, false, false), &PINS_DEFAULT);
    check_pins(&run(GasConstant::EsiV2506, true, false), &PINS_ESI_GRAM);
    let exact = run(GasConstant::EsiV2506, true, true);
    check_pins(&exact, &PINS_ESI_EXACT);
    // every non-PengRobinson combination, all four transports, all grid points
    let mut bad = Vec::new();
    for (tag, st) in &exact {
        if tag.contains("PengRobinson") {
            for s in st.iter() {
                // libm-level only: below 5e-14 relative (the 256-ulp Hs entries are
                // cancellation in Ha - Hc)
                assert!(s.worst_rel < 5e-14, "{tag}: {}", s.worst_rel);
            }
        } else if st.iter().any(|s| s.identical != s.total) {
            bad.push(tag.clone());
        }
    }
    assert!(bad.is_empty(), "not bit-identical: {bad:?}");
}
