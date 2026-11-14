//! Reaction-rate differential test against upstream OpenFOAM-v2506, gh:#1378
//! family 2.
//!
//! # Method
//! `verification_and_validation/upstream_thermo_gh1362/driver_rates_gh1378/rateDump1378.C`
//! evaluates the UNMODIFIED v2506 (ESI) `ArrheniusReactionRate`,
//! `thirdBodyArrheniusReactionRate`, `LandauTellerReactionRate`,
//! `powerSeriesReactionRate`, `JanevReactionRate`,
//! `LangmuirHinshelwoodReactionRate`, `infiniteReactionRate`,
//! `FallOffReactionRate` and `ChemicallyActivatedReactionRate` (Lindemann, Troe,
//! SRI) on 8 temperatures x 4 concentration vectors of 3 species and prints the
//! bit pattern of each rate. This test builds the same rates through the port
//! and compares bit for bit.
//!
//! # Criterion
//! Bit-identity; identical counts and the worst ulp distance are printed
//! (`--nocapture`) and pinned in both directions.
//!
//! # Results
//! See the gh:#1378 family 2 section of
//! `verification_and_validation/upstream_thermo_gh1362/README.md`.

use outram_foam_basic_lib::thermophysics::imports::*;
use outram_foam_basic_lib::thermophysics::reaction_rate::{
    ArrheniusReactionRate, ChemicallyActivatedReactionRate, FallOffFunction, FallOffReactionRate,
    InfiniteReactionRate, JanevReactionRate, LandauTellerReactionRate,
    LangmuirHinshelwoodReactionRate, PowerSeriesReactionRate, ReactionRate,
    ThirdBodyArrheniusReactionRate, ThirdBodyEfficiencies,
};
use std::collections::BTreeMap;

const REF_GZ: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_thermo_gh1362/reference/rates_esi_v2506_gh1378.txt.gz"
);

const TS: [f64; 8] = [250.0, 300.0, 500.0, 800.0, 1000.0, 1500.0, 2000.0, 3000.0];
const CS: [[f64; 3]; 4] = [
    [1e-2, 2e-2, 5e-3],
    [1e-4, 3e-4, 2e-4],
    [1.0, 0.5, 0.25],
    [1e-8, 1e-8, 1e-8],
];

fn gunzip(data: &[u8]) -> String {
    let body = &data[10..data.len() - 8];
    let raw = miniz_oxide::inflate::decompress_to_vec(body).expect("inflate");
    String::from_utf8(raw).expect("utf8")
}

/// tag -> 32 bit patterns, index `iT * 4 + ic`.
fn reference() -> BTreeMap<String, Vec<u64>> {
    let mut map: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for line in gunzip(REF_GZ).lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let mut it = line.split_whitespace();
        let tag = it.next().unwrap().to_string();
        let _ = (it.next(), it.next());
        map.entry(tag)
            .or_default()
            .push(u64::from_str_radix(it.next().unwrap(), 16).unwrap());
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

fn models() -> Vec<(&'static str, ReactionRate)> {
    let tbes = || ThirdBodyEfficiencies::new(vec![1.0, 2.5, 0.4]);
    let k0 = ArrheniusReactionRate::new(6.366e20, -1.72, 524.8);
    let k_inf = ArrheniusReactionRate::new(1.475e12, 0.6, 0.0);
    let troe = FallOffFunction::Troe { alpha: 0.5, tsss: 1e-30, ts: 1e30, tss: 1e100 };
    let troe_fin = FallOffFunction::Troe { alpha: 0.7346, tsss: 94.0, ts: 1756.0, tss: 5182.0 };
    let troe_small = FallOffFunction::Troe { alpha: 1.0, tsss: 1e-30, ts: 1e-30, tss: 1e100 };
    let sri = FallOffFunction::Sri { a: 0.45, b: 797.0, c: 979.0, d: 1.0, e: 0.0 };
    let sri_e = FallOffFunction::Sri { a: 0.5, b: 300.0, c: 1000.0, d: 1.2, e: 0.1 };
    let fo = |f: FallOffFunction| ReactionRate::FallOff(FallOffReactionRate::new(k0, k_inf, f, tbes()));
    let ca = |f: FallOffFunction| {
        ReactionRate::ChemicallyActivated(ChemicallyActivatedReactionRate::new(k0, k_inf, f, tbes()))
    };
    vec![
        ("Arrhenius/full", ReactionRate::Arrhenius(ArrheniusReactionRate::new(3.547e15, -0.406, 8353.0))),
        ("Arrhenius/noBeta", ReactionRate::Arrhenius(ArrheniusReactionRate::new(1.0e10, 0.0, 5000.0))),
        ("Arrhenius/noTa", ReactionRate::Arrhenius(ArrheniusReactionRate::new(1.475e12, 0.6, 0.0))),
        ("Arrhenius/plain", ReactionRate::Arrhenius(ArrheniusReactionRate::new(2.0, 0.0, 0.0))),
        (
            "thirdBodyArrhenius",
            ReactionRate::ThirdBodyArrhenius(ThirdBodyArrheniusReactionRate::new(2.65e16, -0.67, 17041.0, tbes())),
        ),
        ("LandauTeller/full", ReactionRate::LandauTeller(LandauTellerReactionRate::new(1.0e9, 0.3, 1200.0, 35.0, -80.0))),
        ("LandauTeller/noTa", ReactionRate::LandauTeller(LandauTellerReactionRate::new(1.0e9, 0.0, 0.0, 35.0, 0.0))),
        ("LandauTeller/plain", ReactionRate::LandauTeller(LandauTellerReactionRate::new(5.0, 0.0, 0.0, 0.0, 0.0))),
        (
            "powerSeries",
            ReactionRate::PowerSeries(PowerSeriesReactionRate::new(1.0e3, 0.5, 99.0, [-9000.0, 2.0e6, -3.0e8, 1.0e10])),
        ),
        (
            "Janev",
            ReactionRate::Janev(JanevReactionRate::new(
                1.0e-8,
                0.4,
                3000.0,
                [-1.0, 0.8, -0.05, 0.003, -1.0e-4, 5.0e-6, -2.0e-7, 1.0e-8, -3.0e-10],
            )),
        ),
        ("infinite", ReactionRate::Infinite(InfiniteReactionRate)),
        (
            "LangmuirHinshelwood",
            ReactionRate::LangmuirHinshelwood(LangmuirHinshelwoodReactionRate::new(
                [1.0e3, 2.0e1, 3.0e1, 4.0e4, 5.0e2],
                [100.0, 500.0, 700.0, 900.0, 300.0],
                0,
                1,
                2,
            )),
        ),
        ("FallOff/Lindemann", fo(FallOffFunction::Lindemann)),
        ("FallOff/Troe", fo(troe)),
        ("FallOff/Troe/finite", fo(troe_fin)),
        ("FallOff/Troe/small", fo(troe_small)),
        ("FallOff/SRI", fo(sri)),
        ("FallOff/SRI/e", fo(sri_e)),
        ("ChemicallyActivated/Lindemann", ca(FallOffFunction::Lindemann)),
        ("ChemicallyActivated/Troe", ca(troe_fin)),
        ("ChemicallyActivated/SRI", ca(sri)),
    ]
}

/// `(identical, total, worst ulp)` per tag.
fn run() -> BTreeMap<String, (usize, usize, u64)> {
    let refs = reference();
    let mut out = BTreeMap::new();
    let p = Pressure::new::<pascal>(1.0e5);
    for (tag, model) in models() {
        let rows = refs.get(tag).unwrap_or_else(|| panic!("no reference for {tag}"));
        assert_eq!(rows.len(), 32, "{tag}");
        let (mut same, mut worst) = (0usize, 0u64);
        for (it, t) in TS.iter().enumerate() {
            for (ic, c) in CS.iter().enumerate() {
                let mine = model.rate(p, ThermodynamicTemperature::new::<kelvin>(*t), c);
                let d = ulp_distance(mine, f64::from_bits(rows[it * 4 + ic]));
                if d == 0 {
                    same += 1;
                } else if d > worst {
                    worst = d;
                }
            }
        }
        out.insert(tag.to_string(), (same, 32, worst));
    }
    out
}

#[test]
fn rates_gh1378_report() {
    let r = run();
    let mut all = 0;
    for (tag, (same, total, worst)) in &r {
        println!("{tag:34} {same}/{total} worst ulp {worst}");
        all += usize::from(same == total);
    }
    println!("TAGS {} fully bit-identical {all}", r.len());
}

#[test]
fn rates_gh1378_pins() {
    let r = run();
    let mut got: Vec<String> = Vec::new();
    for (tag, (same, total, worst)) in &r {
        if same != total {
            got.push(format!("{tag} {same}/{total} ulp{worst}"));
        }
    }
    let expected: Vec<&str> = PINS.to_vec();
    assert_eq!(got, expected, "\nactual pins:\n{}", got.join("\n"));
}

const PINS: [&str; 6] = [
    "ChemicallyActivated/Troe 30/32 ulp1",
    "FallOff/SRI/e 31/32 ulp1",
    "FallOff/Troe/finite 30/32 ulp1",
    "LandauTeller/full 28/32 ulp6",
    "LandauTeller/noTa 28/32 ulp5",
    "LangmuirHinshelwood 30/32 ulp2",
];


// ---------------------------------------------------------------------------
// Localisation. `reference/libm_calls_rates_gh1378.txt` lists every distinct
// libm call (function, argument bits, glibc result bits) made by the formulas
// above, produced by `scripts/replay_rates_glibc.py` (Python's `math` and `**`
// are glibc's functions). That replay reproduces all 672 reference values, so
// the port's formulas are exact; the only thing left to differ is PETIR against
// glibc in a named function, counted here argument by argument.

use petir::mathf::RealMath;

const LIBM_CALLS: &str = include_str!(
    "../verification_and_validation/upstream_thermo_gh1362/reference/libm_calls_rates_gh1378.txt"
);

/// `(function, calls, calls where PETIR differs from glibc, worst ulp)`.
fn libm_tally() -> Vec<(String, usize, usize, u64)> {
    let mut out: BTreeMap<String, (usize, usize, u64)> = BTreeMap::new();
    for line in LIBM_CALLS.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        let bits = |s: &str| f64::from_bits(u64::from_str_radix(s, 16).unwrap());
        let (mine, glibc) = match f[0] {
            "pow" => (bits(f[1]).r_powf(bits(f[2])), bits(f[3])),
            "exp" => (bits(f[1]).r_exp(), bits(f[2])),
            "log" => (bits(f[1]).r_ln(), bits(f[2])),
            "log10" => (bits(f[1]).r_log10(), bits(f[2])),
            "cbrt" => (bits(f[1]).r_cbrt(), bits(f[2])),
            other => panic!("unknown libm function {other}"),
        };
        let e = out.entry(f[0].to_string()).or_default();
        e.0 += 1;
        let d = ulp_distance(mine, glibc);
        if d != 0 {
            e.1 += 1;
            e.2 = e.2.max(d);
        }
    }
    out.into_iter().map(|(k, v)| (k, v.0, v.1, v.2)).collect()
}

#[test]
fn libm_localisation_report_and_pins() {
    let tally = libm_tally();
    for (f, n, bad, worst) in &tally {
        println!("libm {f:6} calls {n:4} PETIR differs from glibc on {bad:3} worst ulp {worst}");
    }
    let got: Vec<String> = tally.iter().map(|(f, n, b, w)| format!("{f} {n} {b} {w}")).collect();
    let expected: Vec<&str> = LIBM_PINS.to_vec();
    assert_eq!(got, expected, "\nactual:\n{}", got.join("\n"));
}

const LIBM_PINS: [&str; 5] = ["cbrt 8 2 1", "exp 163 1 1", "log 8 0 0", "log10 42 2 1", "pow 348 1 1"];
