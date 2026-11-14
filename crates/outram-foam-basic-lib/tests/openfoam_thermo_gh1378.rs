//! Thermophysics differential test against upstream OpenFOAM-v2506, gh:#1378
//! family 1 (continues gh:#1362 row 4, `openfoam_thermo_code_to_code.rs`).
//!
//! # Method
//! `verification_and_validation/upstream_thermo_gh1362/driver_gh1378/thermoDump1378.C`
//! builds, on the UNMODIFIED v2506 (ESI) `libspecie` templates from
//! dictionaries, {perfectGas, rhoConst, incompressiblePerfectGas, Boussinesq,
//! adiabaticPerfectFluid, perfectFluid, rPolynomial, linear} x {hConst, hPower}
//! x {const, WLF, logPolynomial}, and {the same eight} x eConst, and prints the
//! IEEE-754 bit pattern of Cp, Ha, Hs, Hc, S, rho, psi, Z, CpMCv, mu, kappa,
//! alphah (eConst rows: Cv, Ea, Es, Hc, 0, rho, psi, Z, CpMCv, 0, 0, 0) on a
//! 10 T x 4 p grid. This test builds the same models with the port's
//! constructors (ESI gas constant, exactly round-tripping `W`; see
//! `openfoam_thermo_code_to_code.rs`) and compares bit for bit.
//!
//! # Criterion
//! Bit-identity. Counts of bit-identical values and the worst ulp distance are
//! printed (`--nocapture`) and pinned in both directions. A non-identical value
//! is only accepted where its first divergent operation is a PETIR-against-glibc
//! `pow`/`exp`/`ln` (see the README, gh:#1378 section).
//!
//! # Results
//! See the gh:#1378 section of
//! `verification_and_validation/upstream_thermo_gh1362/README.md`.

use outram_foam_basic_lib::polynomial::Polynomial;
use outram_foam_basic_lib::thermophysics::constants::GasConstant;
use outram_foam_basic_lib::thermophysics::eos::{
    AdiabaticPerfectFluid, BoussinesqEos, EquationOfState, IncompressiblePerfectGas, LinearEos,
    PerfectFluid, PerfectGas, RPolynomialEos, RhoConst,
};
use outram_foam_basic_lib::thermophysics::imports::*;
use outram_foam_basic_lib::thermophysics::thermo::{
    EConstThermo, HConstThermo, HPowerThermo, ThermoModel,
};
use outram_foam_basic_lib::thermophysics::transport::{
    ConstTransport, LogPolynomialTransport, TransportModel, WlfTransport,
};
use std::collections::BTreeMap;

const REF_GZ: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_thermo_gh1362/reference/thermo_esi_v2506_gh1378.txt.gz"
);

const TS: [f64; 10] = [250.0, 280.0, 300.0, 350.0, 400.0, 600.0, 900.0, 1100.0, 1500.0, 1900.0];
const PS: [f64; 4] = [1e5, 1e6, 3e6, 8e6];
const QNAMES: [&str; 12] = [
    "Cp", "Ha", "Hs", "Hc", "S", "rho", "psi", "Z", "CpMCv", "mu", "kappa", "alphah",
];

type Refs = BTreeMap<String, Vec<[u64; 12]>>;

fn gunzip(data: &[u8]) -> String {
    // `gzip -n`: 10-byte header, deflate stream, 8-byte trailer.
    let body = &data[10..data.len() - 8];
    let raw = miniz_oxide::inflate::decompress_to_vec(body).expect("inflate");
    String::from_utf8(raw).expect("utf8")
}

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
    worst_ulp: u64,
}

type Results = BTreeMap<String, [Stat; 12]>;

fn pt(t: f64, p: f64) -> (Pressure, ThermodynamicTemperature) {
    (Pressure::new::<pascal>(p), ThermodynamicTemperature::new::<kelvin>(t))
}

fn q(m: &impl TransportModel, t: f64, p: f64) -> [f64; 12] {
    let (pp, tt) = pt(t, p);
    [
        m.cp(pp, tt).get::<joule_per_kilogram_kelvin>(),
        m.ha(pp, tt).get::<joule_per_kilogram>(),
        m.hs(pp, tt).get::<joule_per_kilogram>(),
        m.hc().get::<joule_per_kilogram>(),
        m.s(pp, tt).get::<joule_per_kilogram_kelvin>(),
        m.rho(pp, tt).get::<kilogram_per_cubic_meter>(),
        m.psi(pp, tt).value,
        m.z(pp, tt).get::<ratio>(),
        m.cp_m_cv(pp, tt).get::<joule_per_kilogram_kelvin>(),
        m.mu(pp, tt).get::<pascal_second>(),
        m.kappa(pp, tt).get::<watt_per_meter_kelvin>(),
        m.alpha_h(pp, tt).get::<pascal_second>(),
    ]
}

/// `eConst` rows: `Cv Ea Es Hc 0 rho psi Z CpMCv 0 0 0` (the ESI-comparable members).
fn qe<E: EquationOfState>(m: &EConstThermo<E>, t: f64, p: f64) -> [f64; 12] {
    let (pp, tt) = pt(t, p);
    [
        m.cv(pp, tt).get::<joule_per_kilogram_kelvin>(),
        m.ea(pp, tt).get::<joule_per_kilogram>(),
        m.es(pp, tt).get::<joule_per_kilogram>(),
        m.hc().get::<joule_per_kilogram>(),
        0.0,
        m.rho(pp, tt).get::<kilogram_per_cubic_meter>(),
        m.psi(pp, tt).value,
        m.z(pp, tt).get::<ratio>(),
        m.cp_m_cv(pp, tt).get::<joule_per_kilogram_kelvin>(),
        0.0,
        0.0,
        0.0,
    ]
}

fn compare_rows(
    results: &mut Results,
    refs: &Refs,
    tag: &str,
    mut eval: impl FnMut(f64, f64) -> [f64; 12],
) {
    let rows = refs.get(tag).unwrap_or_else(|| panic!("no reference for {tag}"));
    assert_eq!(rows.len(), 40, "{tag}");
    let mut stats = [Stat::default(); 12];
    for (it, t) in TS.iter().enumerate() {
        for (ip, p) in PS.iter().enumerate() {
            let mine = eval(*t, *p);
            let theirs = &rows[it * 4 + ip];
            for k in 0..12 {
                let u = f64::from_bits(theirs[k]);
                let s = &mut stats[k];
                s.total += 1;
                let d = ulp_distance(mine[k], u);
                if d == 0 {
                    s.identical += 1;
                } else if d > s.worst_ulp {
                    s.worst_ulp = d;
                }
            }
        }
    }
    results.insert(tag.to_string(), stats);
}

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

fn log_polys() -> (Polynomial<8>, Polynomial<8>) {
    (
        Polynomial::<8>::new([-13.1, 0.7, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
        Polynomial::<8>::new([-9.2, 0.8, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    )
}

fn transports<T: ThermoModel + Clone>(results: &mut Results, refs: &Refs, prefix: &str, th: T) {
    let c = ConstTransport::new(
        th.clone(),
        DynamicViscosity::new::<pascal_second>(1.8e-5),
        Ratio::new::<ratio>(0.7),
    );
    compare_rows(results, refs, &format!("{prefix}/const"), |t, p| q(&c, t, p));
    let w = WlfTransport::new(
        th.clone(),
        DynamicViscosity::new::<pascal_second>(1.0e3),
        ThermodynamicTemperature::new::<kelvin>(300.0),
        17.44,
        51.6,
        Ratio::new::<ratio>(10.0),
    );
    compare_rows(results, refs, &format!("{prefix}/WLF"), |t, p| q(&w, t, p));
    let (mu, ka) = log_polys();
    let l = LogPolynomialTransport::new(th, mu, ka);
    compare_rows(results, refs, &format!("{prefix}/logPolynomial"), |t, p| q(&l, t, p));
}

fn family<E: EquationOfState + Clone>(results: &mut Results, refs: &Refs, name: &str, eos: E) {
    let hc = HConstThermo::new(
        eos.clone(),
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1004.5),
        AvailableEnergy::new::<joule_per_kilogram>(2.5e5),
        ThermodynamicTemperature::new::<kelvin>(298.15),
        AvailableEnergy::new::<joule_per_kilogram>(0.0),
    );
    transports(results, refs, &format!("hConst/{name}"), hc);
    let hp = HPowerThermo::new(
        eos.clone(),
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1000.0),
        0.7,
        ThermodynamicTemperature::new::<kelvin>(300.0),
        AvailableEnergy::new::<joule_per_kilogram>(1.0e5),
    );
    transports(results, refs, &format!("hPower/{name}"), hp);
    let ec = EConstThermo::new(
        eos,
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(718.0),
        AvailableEnergy::new::<joule_per_kilogram>(2.5e5),
        ThermodynamicTemperature::new::<kelvin>(298.15),
        AvailableEnergy::new::<joule_per_kilogram>(0.0),
    );
    compare_rows(results, refs, &format!("eConst/{name}"), |t, p| qe(&ec, t, p));
}

fn run() -> Results {
    let refs = reference();
    let mut results = Results::new();
    let gc = GasConstant::EsiV2506;
    let air = || exact_w(28.96);
    let pa = |v: f64| Pressure::new::<pascal>(v);
    let rho = |v: f64| MassDensity::new::<kilogram_per_cubic_meter>(v);
    family(&mut results, &refs, "perfectGas", PerfectGas::new(air()).with_gas_constant(gc));
    family(&mut results, &refs, "rhoConst", RhoConst::new(air(), rho(998.2)).with_gas_constant(gc));
    family(
        &mut results,
        &refs,
        "incompressiblePerfectGas",
        IncompressiblePerfectGas::new(air(), pa(1.0e5)).with_gas_constant(gc),
    );
    family(
        &mut results,
        &refs,
        "Boussinesq",
        BoussinesqEos::new(air(), rho(1000.0), ThermodynamicTemperature::new::<kelvin>(300.0), 2e-4)
            .with_gas_constant(gc),
    );
    family(
        &mut results,
        &refs,
        "adiabaticPerfectFluid",
        AdiabaticPerfectFluid::new(air(), pa(1.0e5), rho(1000.0), 7.15, pa(3.0e8))
            .with_gas_constant(gc),
    );
    family(
        &mut results,
        &refs,
        "perfectFluid",
        PerfectFluid::new(
            air(),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(3000.0),
            rho(900.0),
        ),
    );
    family(
        &mut results,
        &refs,
        "rPolynomial",
        RPolynomialEos::new(air(), [1.0e-3, 2.0e-7, 1.0e-9, 4.0e-13, 1.0e-16]).with_gas_constant(gc),
    );
    family(
        &mut results,
        &refs,
        "linear",
        LinearEos::new(
            air(),
            MassDensity::new::<kilogram_per_cubic_meter>(4.5e-7) / Pressure::new::<pascal>(1.0),
            rho(998.0),
        )
        .with_gas_constant(gc),
    );
    results
}

fn line_of(tag: &str, st: &[Stat; 12]) -> String {
    let mut line = format!("{tag:46}");
    for (k, s) in st.iter().enumerate() {
        if s.identical != s.total {
            line.push_str(&format!(" {}:{}/{} ulp{}", QNAMES[k], s.identical, s.total, s.worst_ulp));
        }
    }
    line
}

#[test]
fn thermo_gh1378_report() {
    let results = run();
    let (mut combos, mut identical_combos) = (0usize, 0usize);
    for (tag, st) in &results {
        combos += 1;
        if st.iter().all(|s| s.identical == s.total) {
            identical_combos += 1;
        }
        println!("{}", line_of(tag, st));
    }
    println!("COMBOS {combos} fully bit-identical {identical_combos}");
}

/// Pinned: the set of combinations that are NOT fully bit-identical, with the
/// quantities that differ, their identical counts and worst ulp. Everything
/// else must be bit-identical in every quantity.
#[test]
fn thermo_gh1378_pins() {
    let results = run();
    let mut got: Vec<String> = Vec::new();
    for (tag, st) in &results {
        if st.iter().any(|s| s.identical != s.total) {
            got.push(line_of(tag, st).split_whitespace().collect::<Vec<_>>().join(" "));
        }
    }
    let expected: Vec<&str> = PINS.to_vec();
    assert_eq!(got, expected, "\nactual pins:\n{}", got.join("\n"));
}

const PINS: [&str; 0] = [];
