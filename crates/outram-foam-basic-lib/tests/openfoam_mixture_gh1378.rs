//! Multicomponent-mixture differential test against upstream OpenFOAM-v2506,
//! gh:#1378 family 3.
//!
//! # Method
//! `verification_and_validation/upstream_thermo_gh1362/driver_mix_gh1378/mixDump1378.C`
//! mixes three species with the REAL `species::thermo<transport<hConstThermo<
//! eos<specie>>>>` operators (`Y*species`, `+=`) in the loops of
//! `multiComponentMixture::cellMixture` and `cellVolMixture` (copied, since the
//! class needs a mesh) for {perfectGas, rhoConst} x {const, sutherland}
//! transport x six mass-fraction sets (one with a zero fraction, one
//! un-normalised, one normalised by `correctMassFractions`) x 3 T x 2 p, and
//! prints Cp, Ha, Hs, Hc, S, rho, psi, Z, CpMCv, mu, kappa, alphah, W as hex
//! bit patterns. This test does the same through the port's
//! `thermophysics::mixture` and compares bit for bit (ESI gas constant).
//!
//! # Criterion and results
//! Bit-identity, counted and pinned. Results: the gh:#1378 family 3 section of
//! `verification_and_validation/upstream_thermo_gh1362/README.md`.

use outram_foam_basic_lib::thermophysics::constants::GasConstant;
use outram_foam_basic_lib::thermophysics::eos::{PerfectGas, RhoConst};
use outram_foam_basic_lib::thermophysics::imports::*;
use outram_foam_basic_lib::thermophysics::mixture::{
    cell_mixture, cell_vol_mixture, correct_mass_fractions, molar_mass_from_kg_per_kmol, Mixable,
    MixtureComponent,
};
use outram_foam_basic_lib::thermophysics::thermo::HConstThermo;
use outram_foam_basic_lib::thermophysics::transport::{
    ConstTransport, SutherlandTransport, TransportModel,
};
use std::collections::BTreeMap;

const REF_GZ: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_thermo_gh1362/reference/mix_esi_v2506_gh1378.txt.gz"
);

const TS: [f64; 3] = [300.0, 600.0, 1200.0];
const PS: [f64; 2] = [1e5, 5e6];
const W: [f64; 3] = [28.0, 44.01, 18.015];
const CP: [f64; 3] = [1040.0, 850.0, 1900.0];
const HF: [f64; 3] = [0.0, -8.94e6, -1.34e7];
const MU: [f64; 3] = [1.8e-5, 1.5e-5, 1.2e-5];
const PR: [f64; 3] = [0.7, 0.8, 0.9];
const AS: [f64; 3] = [1.46e-6, 1.5e-6, 1.2e-6];
const TSU: [f64; 3] = [110.0, 240.0, 150.0];
const RHO: [f64; 3] = [800.0, 1000.0, 998.0];
const YSETS: [[f64; 3]; 6] = [
    [0.5, 0.3, 0.2],
    [0.9, 0.05, 0.05],
    [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0],
    [0.7, 0.3, 0.0],
    [0.4, 0.4, 0.4],
    [0.4, 0.4, 0.4],
];
const QNAMES: [&str; 13] = [
    "Cp", "Ha", "Hs", "Hc", "S", "rho", "psi", "Z", "CpMCv", "mu", "kappa", "alphah", "W",
];

fn gunzip(data: &[u8]) -> String {
    let body = &data[10..data.len() - 8];
    let raw = miniz_oxide::inflate::decompress_to_vec(body).expect("inflate");
    String::from_utf8(raw).expect("utf8")
}

/// tag -> rows (iT * 2 + ip) of 13 bit patterns.
fn reference() -> BTreeMap<String, Vec<[u64; 13]>> {
    let mut map: BTreeMap<String, Vec<[u64; 13]>> = BTreeMap::new();
    for line in gunzip(REF_GZ).lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let mut it = line.split_whitespace();
        let tag = it.next().unwrap().to_string();
        let _ = (it.next(), it.next());
        let mut row = [0u64; 13];
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

fn q(m: &impl TransportModel, w: f64, t: f64, p: f64) -> [f64; 13] {
    let pp = Pressure::new::<pascal>(p);
    let tt = ThermodynamicTemperature::new::<kelvin>(t);
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
        w,
    ]
}

type Stats = BTreeMap<String, [(usize, u64); 13]>;

fn ct<T: outram_foam_basic_lib::thermophysics::thermo::ThermoModel>(th: T, i: usize) -> ConstTransport<T> {
    ConstTransport::new(
        th,
        DynamicViscosity::new::<pascal_second>(MU[i]),
        Ratio::new::<ratio>(PR[i]),
    )
}

fn run() -> Stats {
    let refs = reference();
    let mut stats = Stats::new();
    let gc = GasConstant::EsiV2506;
    let eos_pg = |i: usize| PerfectGas::new(molar_mass_from_kg_per_kmol(W[i])).with_gas_constant(gc);
    let eos_rc = |i: usize| {
        RhoConst::new(
            molar_mass_from_kg_per_kmol(W[i]),
            MassDensity::new::<kilogram_per_cubic_meter>(RHO[i]),
        )
        .with_gas_constant(gc)
    };
    let hconst = |e: PerfectGas, i: usize| {
        HConstThermo::new(
            e,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(CP[i]),
            AvailableEnergy::new::<joule_per_kilogram>(HF[i]),
            ThermodynamicTemperature::new::<kelvin>(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        )
    };
    let hconst_rc = |e: RhoConst, i: usize| {
        HConstThermo::new(
            e,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(CP[i]),
            AvailableEnergy::new::<joule_per_kilogram>(HF[i]),
            ThermodynamicTemperature::new::<kelvin>(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        )
    };
    let pg_const: Vec<_> = (0..3)
        .map(|i| MixtureComponent::new(ct(hconst(eos_pg(i), i), i), W[i]))
        .collect();
    let pg_suth: Vec<_> = (0..3)
        .map(|i| MixtureComponent::new(SutherlandTransport::new(hconst(eos_pg(i), i), AS[i], TSU[i]), W[i]))
        .collect();
    let rc_const: Vec<_> = (0..3)
        .map(|i| MixtureComponent::new(ct(hconst_rc(eos_rc(i), i), i), W[i]))
        .collect();
    let rc_suth: Vec<_> = (0..3)
        .map(|i| {
            MixtureComponent::new(
                SutherlandTransport::new(hconst_rc(eos_rc(i), i), AS[i], TSU[i]),
                W[i],
            )
        })
        .collect();

    run_combo(&mut stats, &refs, "perfectGas/const", &pg_const);
    run_combo(&mut stats, &refs, "perfectGas/sutherland", &pg_suth);
    run_combo(&mut stats, &refs, "rhoConst/const", &rc_const);
    run_combo(&mut stats, &refs, "rhoConst/sutherland", &rc_suth);
    stats
}

/// One (eos, transport) combination over the six `Y` sets and both mixing rules.
fn run_combo<M>(
    stats: &mut Stats,
    refs: &BTreeMap<String, Vec<[u64; 13]>>,
    name: &str,
    species: &[MixtureComponent<M>],
) where
    M: Mixable + TransportModel,
{
    for (iy, ys) in YSETS.iter().enumerate() {
        let mut y = *ys;
        if iy == 5 {
            correct_mass_fractions(&mut y).unwrap();
        }
        for mode in 0..2 {
            let tag = format!("{name}/{}/{iy}", if mode == 0 { "cell" } else { "vol" });
            let rows = refs.get(&tag).unwrap_or_else(|| panic!("no reference for {tag}"));
            assert_eq!(rows.len(), 6, "{tag}");
            let mut st = [(0usize, 0u64); 13];
            for (it, t) in TS.iter().enumerate() {
                for (ip, p) in PS.iter().enumerate() {
                    let pp = Pressure::new::<pascal>(*p);
                    let tt = ThermodynamicTemperature::new::<kelvin>(*t);
                    let mix = if mode == 0 {
                        cell_mixture(species, &y)
                    } else {
                        let rho: Vec<f64> = species
                            .iter()
                            .map(|s| s.model().rho(pp, tt).get::<kilogram_per_cubic_meter>())
                            .collect();
                        cell_vol_mixture(species, &y, &rho)
                    };
                    let mine = q(mix.model(), mix.w_kg_per_kmol(), *t, *p);
                    let theirs = &rows[it * 2 + ip];
                    for k in 0..13 {
                        let d = ulp_distance(mine[k], f64::from_bits(theirs[k]));
                        if d == 0 {
                            st[k].0 += 1;
                        } else if d > st[k].1 {
                            st[k].1 = d;
                        }
                    }
                }
            }
            stats.insert(tag, st);
        }
    }
}

fn line_of(tag: &str, st: &[(usize, u64); 13]) -> String {
    let mut line = tag.to_string();
    for (k, (same, worst)) in st.iter().enumerate() {
        if *same != 6 {
            line.push_str(&format!(" {}:{same}/6 ulp{worst}", QNAMES[k]));
        }
    }
    line
}

#[test]
fn mixture_gh1378_report() {
    let stats = run();
    let (mut all, mut ident) = (0, 0);
    for (tag, st) in &stats {
        all += 1;
        ident += usize::from(st.iter().all(|(s, _)| *s == 6));
        println!("{}", line_of(tag, st));
    }
    println!("TAGS {all} fully bit-identical {ident}");
}

#[test]
fn mixture_gh1378_pins() {
    let stats = run();
    let got: Vec<String> = stats
        .iter()
        .filter(|(_, st)| st.iter().any(|(s, _)| *s != 6))
        .map(|(tag, st)| line_of(tag, st))
        .collect();
    let expected: Vec<&str> = PINS.to_vec();
    assert_eq!(got, expected, "\nactual:\n{}", got.join("\n"));
}

const PINS: [&str; 2] = [
    "perfectGas/const/cell/3 S:3/6 ulp4 rho:0/6 ulp1 psi:0/6 ulp2 CpMCv:0/6 ulp1",
    "perfectGas/sutherland/cell/3 S:3/6 ulp4 rho:0/6 ulp1 psi:0/6 ulp2 CpMCv:0/6 ulp1",
];

/// The two residuals have one cause: for the mixture molecular weight of
/// `Y = (0.7, 0.3, 0)` (cell mixture) no double `w` in kg/mol satisfies
/// `w * 1e3 == W` exactly (`W` in kg/kmol is upstream's unit), so
/// `R = RR/W` is formed from a `W` one unit-conversion ulp away. `W` itself is
/// bit-identical (13th column) and so are Cp, Ha, Hs, Hc, mu, kappa, alphah;
/// the differing quantities are exactly those through `R`.
#[test]
fn residuals_are_the_kg_per_mol_unit_round_trip() {
    let comps: Vec<MixtureComponent<PerfectGas>> = (0..3)
        .map(|i| MixtureComponent::new(PerfectGas::new(molar_mass_from_kg_per_kmol(W[i])), W[i]))
        .collect();
    let w_up = cell_mixture(&comps, &[0.7, 0.3, 0.0]).w_kg_per_kmol();
    let w = molar_mass_from_kg_per_kmol(w_up).get::<kilogram_per_mole>();
    assert_ne!(w * 1.0e3, w_up, "a round-tripping kg/mol exists, so the residual has another cause");
    // and the other Y sets of the cell mixture do round-trip
    for y in [[0.5, 0.3, 0.2], [0.9, 0.05, 0.05]] {
        let wk = cell_mixture(&comps, &y).w_kg_per_kmol();
        assert_eq!(molar_mass_from_kg_per_kmol(wk).get::<kilogram_per_mole>() * 1.0e3, wk);
    }
}
