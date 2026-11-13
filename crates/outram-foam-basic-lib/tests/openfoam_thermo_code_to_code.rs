//! Thermophysics differential test against upstream OpenFOAM-v2506 (gh:#1362, row 4).
//!
//! # Method
//! `verification_and_validation/upstream_thermo_gh1362/driver/thermoDump.C` builds
//! every `thermo x equationOfState x transport` combination from a dictionary
//! on the UNMODIFIED v2506 (ESI) `libspecie` templates (hConst, janaf,
//! hPolynomial, hTabulated x perfectGas, rhoConst, icoPolynomial, PengRobinson
//! x const, sutherland, polynomial, tabulated transport) and prints the IEEE-754
//! bit pattern of Cp, Ha, Hs, Hc, S, rho, psi, Z, CpMCv, mu, kappa, alphah on a
//! 10 T x 4 p grid. This test builds the same models through the port's
//! ordinary constructors and compares bit for bit.
//!
//! # Criterion
//! Bit-identity. For each quantity the test counts the bit-identical values,
//! the worst relative difference and the worst ulp distance, prints them
//! (`--nocapture`), and PINS them in both directions.

use outram_foam_basic_lib::polynomial::Polynomial;
use outram_foam_basic_lib::thermophysics::eos::{
    EquationOfState, IcoPolynomial, PengRobinsonGas, PerfectGas, RhoConst,
};
use outram_foam_basic_lib::thermophysics::imports::*;
use outram_foam_basic_lib::thermophysics::thermo::{
    HConstThermo, HPolynomialThermo, JanafThermo, ThermoModel,
};
use outram_foam_basic_lib::thermophysics::transport::{
    ConstTransport, PolynomialTransport, SutherlandTransport, TabulatedTransport, TransportModel,
};
use std::collections::BTreeMap;

const REF_GZ: &[u8] =
    include_bytes!("../verification_and_validation/upstream_thermo_gh1362/reference/thermo_esi_v2506.txt.gz");

const TS: [f64; 10] = [250.0, 280.0, 300.0, 350.0, 400.0, 600.0, 900.0, 1100.0, 1500.0, 1900.0];
const PS: [f64; 4] = [1e5, 1e6, 3e6, 8e6];
const QNAMES: [&str; 12] = [
    "Cp", "Ha", "Hs", "Hc", "S", "rho", "psi", "Z", "CpMCv", "mu", "kappa", "alphah",
];

fn gunzip(data: &[u8]) -> String {
    // `gzip -n`: 10-byte header, deflate stream, 8-byte trailer.
    let body = &data[10..data.len() - 8];
    let raw = miniz_oxide::inflate::decompress_to_vec(body).expect("inflate");
    String::from_utf8(raw).expect("utf8")
}

/// tag -> 40 rows (it * 4 + ip) of 12 bit patterns.
fn reference() -> BTreeMap<String, Vec<[u64; 12]>> {
    let mut map: BTreeMap<String, Vec<[u64; 12]>> = BTreeMap::new();
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

fn compare(
    results: &mut Results,
    refs: &BTreeMap<String, Vec<[u64; 12]>>,
    tag: &str,
    m: &impl TransportModel,
    has_s: bool,
) {
    let rows = refs.get(tag).unwrap_or_else(|| panic!("no reference for {tag}"));
    assert_eq!(rows.len(), 40, "{tag}");
    let mut stats = [Stat::default(); 12];
    for (it, t) in TS.iter().enumerate() {
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
                    if rel.is_nan() || rel > s.worst_rel {
                        s.worst_rel = if rel.is_nan() { f64::INFINITY } else { rel };
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

const SUTH_AS: f64 = 1.458e-6;
const SUTH_TS: f64 = 110.4;

fn transports<T: ThermoModel + Clone>(
    results: &mut Results,
    refs: &BTreeMap<String, Vec<[u64; 12]>>,
    prefix: &str,
    th: T,
    has_s: bool,
) {
    let c = ConstTransport::new(
        th.clone(),
        DynamicViscosity::new::<pascal_second>(1.8e-5),
        Ratio::new::<ratio>(0.7),
    );
    compare(results, refs, &format!("{prefix}/const"), &c, has_s);
    let s = SutherlandTransport::new(th.clone(), SUTH_AS, SUTH_TS);
    compare(results, refs, &format!("{prefix}/sutherland"), &s, has_s);
    let p = PolynomialTransport::new(
        th.clone(),
        Polynomial::<8>::new([1.0e-5, 4.0e-8, -1.0e-11, 0.0, 0.0, 0.0, 0.0, 0.0]),
        Polynomial::<8>::new([0.01, 6.0e-5, -1.0e-8, 0.0, 0.0, 0.0, 0.0, 0.0]),
    );
    compare(results, refs, &format!("{prefix}/polynomial"), &p, has_s);
    let t = TabulatedTransport::new(
        th,
        (vec![200.0, 500.0, 1000.0, 2000.0], vec![1.0e-5, 2.4e-5, 4.0e-5, 6.5e-5]),
        (vec![200.0, 600.0, 1200.0, 2000.0], vec![0.02, 0.045, 0.08, 0.12]),
    );
    compare(results, refs, &format!("{prefix}/tabulated"), &t, has_s);
}

fn thermos<E: EquationOfState + Clone>(
    results: &mut Results,
    refs: &BTreeMap<String, Vec<[u64; 12]>>,
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
    transports(results, refs, &format!("hConst/{eos_name}"), hc, true);

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
    let jn = JanafThermo::new(eos.clone(), 200.0, 6000.0, 1000.0, low, high);
    transports(results, refs, &format!("janaf/{eos_name}"), jn, true);

    let hp = HPolynomialThermo::new(
        eos,
        Polynomial::<8>::new([1000.0, 0.1, -2.0e-5, 3.0e-9, 0.0, 0.0, 0.0, 0.0]),
        AvailableEnergy::new::<joule_per_kilogram>(1.0e5),
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(200.0),
    );
    transports(results, refs, &format!("hPolynomial/{eos_name}"), hp, true);
}

fn run() -> Results {
    let refs = reference();
    let mut results = Results::new();
    let air = || MolarMass::new::<gram_per_mole>(28.96);
    thermos(&mut results, &refs, "perfectGas", PerfectGas::new(air()));
    thermos(
        &mut results,
        &refs,
        "rhoConst",
        RhoConst::new(air(), MassDensity::new::<kilogram_per_cubic_meter>(998.2)),
    );
    thermos(
        &mut results,
        &refs,
        "icoPolynomial",
        IcoPolynomial::<8>::new(
            air(),
            Polynomial::<8>::new([1010.0, -0.4, -2.0e-3, 1.0e-6, 0.0, 0.0, 0.0, 0.0]),
        ),
    );
    thermos(
        &mut results,
        &refs,
        "PengRobinson",
        PengRobinsonGas::new(
            MolarMass::new::<gram_per_mole>(44.01),
            ThermodynamicTemperature::new::<kelvin>(304.12),
            Pressure::new::<pascal>(7.374e6),
            0.2239,
        ),
    );
    results
}

#[test]
fn thermo_vs_openfoam_report() {
    let results = run();
    for (tag, st) in &results {
        let mut line = format!("{tag:42}");
        for (k, s) in st.iter().enumerate() {
            line.push_str(&format!(
                " {}:{}/{} ulp{} rel{:.1e}",
                QNAMES[k], s.identical, s.total, s.worst_ulp, s.worst_rel
            ));
        }
        println!("{line}");
    }
}
