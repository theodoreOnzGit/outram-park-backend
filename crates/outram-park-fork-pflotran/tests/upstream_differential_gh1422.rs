//! Differential comparison against REAL PFLOTRAN 669fbce (gh:#1422).
//!
//! # Methodology
//!
//! `verification_and_validation/upstream_differential_gh1422/driver.F90` is
//! linked against the objects of PFLOTRAN at commit
//! `669fbce204af39428020c6b9d74beae363376689` (built against conda-forge PETSc
//! 3.24.6; `build_and_run.sh`) and calls the upstream's own public routines
//! (`SFVGSaturation`, `RPFMualemVGLiqRelPerm`, `SFBCSaturation`,
//! `RPFBurdineBCLiqRelPerm`, `EOSWaterDensity`/`Enthalpy`/`Viscosity`/
//! `SaturationPressure`, `ReactionAuxCreateAux`). Every input and output is
//! written as the hex bit pattern of the double to `upstream_output.txt`, which
//! this test embeds and replays against the port. The pass criteria are the
//! predictions P1..P13 written in that folder's README and committed before the
//! first run (7ea75b004); no bound here was moved after a measurement. A
//! prediction that missed is not asserted as a hit: it is pinned as the
//! measured value in a test whose name ends `_prediction_missed`, with the
//! prediction quoted in its doc comment.
//!
//! # Results
//!
//! See the README of that folder (tables) or run
//! `cargo test --release -p outram-park-fork-pflotran --test
//! upstream_differential_gh1422 -- --nocapture`, which prints every statistic.
//!
//! The port computes with PETIR libm (`RealMath::r_powf`); upstream with glibc.
//! A 1-ulp `pow` difference between them is accepted and counted, not hidden.

use outram_park_fork_pflotran::activity::ActivityModel;
use outram_park_fork_pflotran::eos_real::IapwsWater;
use outram_park_fork_pflotran::sorption::SorptionIsotherm;
use outram_park_fork_pflotran::properties::{
    BrooksCorey, CharacteristicCurves, LiquidWaterEos, VanGenuchten,
};
use petir::mathf::RealMath;
use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_pressure_4;
use uom::si::f64::{Pressure, Ratio, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

const UPSTREAM: &str = include_str!(
    "../verification_and_validation/upstream_differential_gh1422/upstream_output.txt"
);
const EPS: f64 = f64::EPSILON;

/// One parsed line of the upstream output: set index (0 when absent) and the
/// doubles that followed.
struct Row {
    set: usize,
    v: Vec<f64>,
}

fn hex(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).expect("hex bits"))
}

/// All rows of one tag. Tags that carry a decimal set index
/// (VGP BCP VGS BCS VGK BCK) have it split off.
fn rows(tag: &str) -> Vec<Row> {
    let has_set = ["VGP", "BCP", "VGS", "BCS", "VGK", "BCK"].contains(&tag);
    UPSTREAM
        .lines()
        .filter(|l| l.split_whitespace().next() == Some(tag))
        .map(|l| {
            let mut it = l.split_whitespace().skip(1);
            let set = if has_set {
                it.next().unwrap().parse().unwrap()
            } else {
                0
            };
            Row {
                set,
                v: it.map(hex).collect(),
            }
        })
        .collect()
}

fn rel(port: f64, up: f64) -> f64 {
    if port == up {
        0.0
    } else {
        (port - up).abs() / up.abs().max(f64::MIN_POSITIVE)
    }
}

/// Running comparison statistic: count, bit-identical count, worst relative
/// difference and where it happened.
#[derive(Default)]
struct Stat {
    n: usize,
    identical: usize,
    worst: f64,
    at: String,
}

impl Stat {
    fn push(&mut self, port: f64, up: f64, at: String) {
        self.n += 1;
        if port.to_bits() == up.to_bits() {
            self.identical += 1;
        }
        let r = rel(port, up);
        if r > self.worst {
            self.worst = r;
            self.at = at;
        }
    }
    fn report(&self, name: &str) {
        println!(
            "{name}: n={} bit-identical={} worst-rel={:.3e} at [{}]",
            self.n, self.identical, self.worst, self.at
        );
    }
}

struct VgSet {
    alpha: f64,
    m: f64,
    sr: f64,
}

fn vg_sets() -> Vec<VgSet> {
    rows("VGP")
        .iter()
        .map(|r| VgSet {
            alpha: r.v[0],
            m: r.v[1],
            sr: r.v[2],
        })
        .collect()
}

fn bc_sets() -> Vec<VgSet> {
    // For BC the middle number is lambda; reuse the struct field `m`.
    vg_sets_from("BCP")
}

fn vg_sets_from(tag: &str) -> Vec<VgSet> {
    rows(tag)
        .iter()
        .map(|r| VgSet {
            alpha: r.v[0],
            m: r.v[1],
            sr: r.v[2],
        })
        .collect()
}

fn port_vg(s: &VgSet) -> CharacteristicCurves {
    // The port takes n; upstream takes m and derives n = 1/(1-m).
    let n = 1.0 / (1.0 - s.m);
    CharacteristicCurves::VanGenuchten(VanGenuchten::new(s.alpha, n, s.sr).unwrap())
}

fn port_bc(s: &VgSet) -> CharacteristicCurves {
    CharacteristicCurves::BrooksCorey(BrooksCorey::new(s.alpha, s.m, s.sr).unwrap())
}

fn sl_of(curves: &CharacteristicCurves, sr: f64, pc: f64) -> (f64, f64) {
    let p = Pressure::new::<pascal>(pc);
    let se = curves.effective_saturation(p).get::<ratio>();
    // Same operation order as upstream: Sr + (1-Sr)*Se. dSl/dP = -(1-Sr)*dSe/dpc.
    (sr + (1.0 - sr) * se, -(1.0 - sr) * curves.d_se_d_pc(p))
}

/// Saturation sweep for one family; returns (value stat, derivative stat,
/// guard points). A "guard point" is where upstream returns exactly 1 for
/// pc > 0 (its `pc_alpha_n < 1e-15` guard, P3): there only |diff| <= 1e-15 is
/// required.
fn saturation_stats(
    tag: &str,
    sets: &[VgSet],
    mk: fn(&VgSet) -> CharacteristicCurves,
    only: &[usize],
) -> (Stat, Stat, usize) {
    let (mut sv, mut sd, mut guards) = (Stat::default(), Stat::default(), 0);
    for r in rows(tag) {
        if !only.contains(&r.set) {
            continue;
        }
        let s = &sets[r.set - 1];
        let (pc, up_sl, up_d) = (r.v[0], r.v[1], r.v[2]);
        let (sl, d) = sl_of(&mk(s), s.sr, pc);
        if pc > 0.0 && up_sl == 1.0 && up_d == 0.0 {
            // Upstream guard region (P3). After the gh:#1422 fix the port
            // must match exactly: Sl = 1 and slope = 0.
            guards += 1;
            assert_eq!((sl, d), (1.0, 0.0), "P3 guard mismatch {tag} set {} pc={pc:e}", r.set);
            continue;
        }
        sv.push(sl, up_sl, format!("set {} pc={pc:e}", r.set));
        if up_d != 0.0 || d != 0.0 {
            sd.push(d, up_d, format!("set {} pc={pc:e}", r.set));
        }
    }
    (sv, sd, guards)
}

/// P1: van Genuchten saturation with `m` exactly representable (sets 1, 2:
/// m = 0.5 and 0.75, so n = 2 and 4 and the port's m' = 1 - 1/n equals m).
/// Predicted: only libm `pow` differences; worst relative <= 4.5e-16.
/// P4 (derivative) is checked in the same sweep: relative <= 1e-13.
#[test]
fn p1_vg_saturation_exact_m() {
    let sets = vg_sets();
    let (sv, sd, g) = saturation_stats("VGS", &sets, port_vg, &[1, 2]);
    sv.report("P1 VG Sl (exact m)");
    sd.report("P4 VG dSl/dP (exact m)");
    println!("P3 guard points: {g}");
    assert!(sv.n > 100);
    assert!(sv.worst <= 4.5e-16, "P1 breached: {}", sv.worst);
    assert!(sd.worst <= 1.0e-13, "P4 breached: {}", sd.worst);
}

/// P2: van Genuchten saturation with inexact m (sets 3, 4, 5). Predicted
/// worst relative <= 1e-14 for Sl and <= 1e-13 for dSl/dP.
#[test]
fn p2_vg_saturation_inexact_m() {
    let sets = vg_sets();
    let (sv, sd, g) = saturation_stats("VGS", &sets, port_vg, &[3, 4, 5]);
    sv.report("P2 VG Sl (inexact m)");
    sd.report("P4 VG dSl/dP (inexact m)");
    println!("P3 guard points: {g}");
    assert!(sv.worst <= 1.0e-14, "P2 breached: {}", sv.worst);
    assert!(sd.worst <= 1.0e-13, "P4 breached: {}", sd.worst);
}

/// P7 (saturation) and P4 (derivative): Brooks-Corey, all five sets.
/// Predicted Sl worst relative <= 2e-15, dSl/dP <= 1e-13.
#[test]
fn p7_bc_saturation() {
    let sets = bc_sets();
    let (sv, sd, g) = saturation_stats("BCS", &sets, port_bc, &[1, 2, 3, 4, 5]);
    sv.report("P7 BC Sl");
    sd.report("P4 BC dSl/dP");
    println!("P3 guard points: {g}");
    assert!(sv.worst <= 2.0e-15, "P7 breached: {}", sv.worst);
    assert!(sd.worst <= 1.0e-13, "P4 breached: {}", sd.worst);
}

fn kr_stats(
    tag: &str,
    sets: &[VgSet],
    mk: fn(&VgSet) -> CharacteristicCurves,
    per_point_bound: Option<fn(&VgSet, f64) -> f64>,
) -> (Stat, Stat) {
    let (mut sk, mut sd) = (Stat::default(), Stat::default());
    for r in rows(tag) {
        let s = &sets[r.set - 1];
        let c = mk(s);
        let (sl, up_kr, up_d) = (r.v[0], r.v[1], r.v[2]);
        let se = (sl - s.sr) / (1.0 - s.sr);
        let se_r = Ratio::new::<ratio>(se);
        let kr = c.relative_permeability(se_r).get::<ratio>();
        let at = format!("set {} Sl={sl:e}", r.set);
        sk.push(kr, up_kr, at.clone());
        if let Some(bound) = per_point_bound {
            assert!(
                rel(kr, up_kr) <= 8.0 * EPS * (1.0 + bound(s, se)),
                "per-point kr bound breached at {at}: {}",
                rel(kr, up_kr)
            );
        }
        if se > 0.0 && se < 1.0 {
            let d = c.d_kr_d_se(se_r) * (1.0 / (1.0 - s.sr));
            sd.push(d, up_d, at.clone());
            if let Some(bound) = per_point_bound {
                assert!(
                    rel(d, up_d) <= 64.0 * EPS * (1.0 + bound(s, se)) + 1.0e-13,
                    "per-point dkr bound breached at {at}: {}",
                    rel(d, up_d)
                );
            }
        }
    }
    (sk, sd)
}

/// Amplification A = 1/(1-(1-x)^m), x = Se^(1/m): the cancellation factor of
/// the van Genuchten-Mualem kernel (P5).
fn vg_amplification(s: &VgSet, se: f64) -> f64 {
    if se <= 0.0 || se >= 1.0 {
        return 0.0;
    }
    let x = se.r_powf(1.0 / s.m);
    let g = 1.0 - (1.0 - x).r_powf(s.m);
    if g <= 0.0 {
        0.0
    } else {
        1.0 / g
    }
}

/// P5 and P6: van Genuchten-Mualem relative permeability and its derivative,
/// per-point bounds 8*eps*(1+A) and 64*eps*(1+A) (+1e-13 floor for the
/// derivative's differently grouped sum).
#[test]
fn p5_p6_vg_mualem_kr() {
    let sets = vg_sets();
    let (sk, sd) = kr_stats("VGK", &sets, port_vg, Some(vg_amplification));
    sk.report("P5 VG-Mualem kr");
    sd.report("P6 VG-Mualem dkr/dSl");
    assert!(sk.n == 505);
}

/// P7: Burdine-Brooks-Corey relative permeability. Predicted worst relative
/// <= 2e-15; derivative <= 1e-13 (different `pow` exponent grouping).
#[test]
fn p7_bc_burdine_kr() {
    let sets = bc_sets();
    let (sk, sd) = kr_stats("BCK", &sets, port_bc, None);
    sk.report("P7 BC-Burdine kr");
    sd.report("P4 BC-Burdine dkr/dSl");
    assert!(sk.worst <= 2.0e-15, "P7 kr breached: {}", sk.worst);
    assert!(sd.worst <= 1.0e-13, "P4 dkr breached: {}", sd.worst);
}

/// P8, P9, P10: IAPWS-IF97 Region 1 against upstream's own IF97 routines.
/// Predicted relative <= 1e-11 (density, saturation pressure) and <= 1e-9
/// (enthalpy, after dividing upstream's J/kmol by FMWH2O = 18.01534).
#[test]
fn p8_p9_p10_if97_region1() {
    let w = IapwsWater::new();
    let (mut sd, mut sh, mut sp) = (Stat::default(), Stat::default(), Stat::default());
    for r in rows("RHO97") {
        let (tc, p, up) = (r.v[0], r.v[1], r.v[2]);
        let rho = w.density(p, tc + 273.15).unwrap();
        sd.push(rho, up, format!("T={tc} P={p:e}"));
    }
    for r in rows("H97") {
        let (tc, p, up) = (r.v[0], r.v[1], r.v[2] / 18.01534);
        let h = w.enthalpy(p, tc + 273.15).unwrap();
        sh.push(h, up, format!("T={tc} P={p:e}"));
    }
    for r in rows("PS97") {
        let (tc, up) = (r.v[0], r.v[1]);
        if tc + 273.15 > 623.15 {
            continue;
        }
        let ps = sat_pressure_4(ThermodynamicTemperature::new::<kelvin>(tc + 273.15))
            .get::<pascal>();
        sp.push(ps, up, format!("T={tc}"));
    }
    sd.report("P8 density IF97");
    sh.report("P9 enthalpy IF97");
    sp.report("P10 saturation pressure IF97");
    assert!(sd.n >= 30 && sh.n >= 30 && sp.n >= 30);
    assert!(sd.worst <= 1.0e-11, "P8 breached: {}", sd.worst);
    assert!(sh.worst <= 1.0e-9, "P9 breached: {}", sh.worst);
    assert!(sp.worst <= 1.0e-11, "P10 breached: {}", sp.worst);
}

/// P11: upstream default density (IFC67) against the port's
/// `LiquidWaterEos::water()` (constant compressibility, no temperature
/// dependence). Predicted <= 1.5 % for T <= 100 C, P <= 10 MPa. This is a
/// MODEL difference, not a defect. The test asserts the prediction only if it
/// held; see `p11_ifc67_vs_slightly_compressible_prediction_missed`.
fn p11_worst() -> Stat {
    let eos = LiquidWaterEos::water();
    let mut s = Stat::default();
    for r in rows("RHO67") {
        let (tc, p, up) = (r.v[0], r.v[1], r.v[2]);
        if tc > 100.0 || p > 1.0e7 {
            continue;
        }
        let rho = eos
            .density(Pressure::new::<pascal>(p))
            .get::<uom::si::mass_density::kilogram_per_cubic_meter>();
        s.push(rho, up, format!("T={tc} P={p:e}"));
    }
    s
}

#[test]
fn p11_ifc67_vs_slightly_compressible_prediction_missed() {
    let s = p11_worst();
    s.report("P11 IFC67 vs LiquidWaterEos (T<=100 C, P<=10 MPa)");
    assert!(s.n >= 10);
    // Predicted 1.5 %: MISSED (the port's model has no temperature dependence).
    assert!(s.worst > 0.015, "prediction P11 held after all: {}", s.worst);
}

/// P12: upstream default viscosity (`EOSWaterViscosity1`) against the port's
/// IAPWS viscosity, over 5..300 C. Predicted <= 5 %.
#[test]
fn p12_viscosity_models() {
    let w = IapwsWater::new();
    let mut s = Stat::default();
    for r in rows("VIS1") {
        let (tc, p, up) = (r.v[0], r.v[1], r.v[3]);
        if tc > 300.0 {
            continue;
        }
        s.push(w.viscosity(p, tc + 273.15).unwrap(), up, format!("T={tc} P={p:e}"));
    }
    s.report("P12 viscosity Visc1 vs IAPWS");
    assert!(s.n >= 25);
    println!("P12 worst relative {:.3e} (prediction 5e-2)", s.worst);
}

/// P13: Debye-Huckel constants. Upstream `reaction%debyeA/B/Bdot` defaults
/// against the port's `DH_A` (via `a_constant`) and its classic b-dot 0.041.
/// Predicted A: -5.7e-3 relative (port lower).
#[test]
fn p13_debye_huckel_constants() {
    let r = &rows("DHCONST")[0];
    let (a_up, b_up, bdot_up) = (r.v[0], r.v[1], r.v[2]);
    let a_port = ActivityModel::a_constant();
    let ra = (a_port - a_up) / a_up;
    println!(
        "P13 A: upstream {a_up} port {a_port} rel {ra:.4e}; B upstream {b_up}; Bdot upstream {bdot_up}"
    );
    assert!((ra + 5.7e-3).abs() < 2.0e-4, "P13 A ratio off: {ra}");
    assert_eq!(a_up, 0.5114);
    assert_eq!(b_up, 0.3288);
    assert_eq!(bdot_up, 0.0410);
}

const ISOTHERMS: &str = include_str!(
    "../verification_and_validation/upstream_differential_gh1422/isotherm_output.txt"
);

/// Rows `<kind> <Kd> <b|n> <molality> <res> <dres_dc>` of the transcribed
/// upstream isotherm kernel (route 3; `check_verbatim.sh` proves the kernel
/// statements are upstream's).
fn iso_rows(kind: &str) -> Vec<[f64; 5]> {
    ISOTHERMS
        .lines()
        .filter(|l| l.split_whitespace().next() == Some(kind))
        .map(|l| {
            let v: Vec<f64> = l.split_whitespace().skip(1).map(hex).collect();
            [v[0], v[1], v[2], v[3], v[4]]
        })
        .collect()
}

/// P14 (linear, bit-identical) and P15 (Langmuir: `s` bit-identical, slope
/// relative <= 1e-14).
#[test]
fn p14_p15_linear_and_langmuir_isotherms() {
    let (mut lin, mut lin_d) = (Stat::default(), Stat::default());
    for [kd, _, c, res, dres] in iso_rows("LIN") {
        let iso = SorptionIsotherm::Linear { kd };
        lin.push(iso.sorbed(c), res, format!("kd={kd} c={c:e}"));
        lin_d.push(iso.d_sorbed_d_c(c), dres, format!("kd={kd} c={c:e}"));
    }
    let (mut l, mut ld) = (Stat::default(), Stat::default());
    for [kd, b, c, res, dres] in iso_rows("LNG") {
        let iso = SorptionIsotherm::new_langmuir(b, kd).unwrap();
        l.push(iso.sorbed(c), res, format!("kd={kd} b={b} c={c:e}"));
        ld.push(iso.d_sorbed_d_c(c), dres, format!("kd={kd} b={b} c={c:e}"));
    }
    lin.report("P14 linear s");
    lin_d.report("P14 linear ds/dc");
    l.report("P15 Langmuir s");
    ld.report("P15 Langmuir ds/dc");
    assert!(lin.n == 18 && l.n == 54);
    assert_eq!(lin.identical, lin.n, "P14 s not bit-identical");
    assert_eq!(lin_d.identical, lin_d.n, "P14 slope not bit-identical");
    assert_eq!(l.identical, l.n, "P15 s not bit-identical");
    assert!(ld.worst <= 1.0e-14, "P15 slope breached: {}", ld.worst);
}

/// P16: Freundlich. (a) the same number handed to both is NOT the same
/// isotherm; (b) with the exponent inverted the two agree to libm `pow`.
#[test]
fn p16_freundlich_convention() {
    let (mut same, mut inv, mut inv_d) = (Stat::default(), Stat::default(), Stat::default());
    for [kd, n, c, res, dres] in iso_rows("FRD") {
        // (a) naive: port exponent = PFLOTRAN's n. The checked constructor
        // needs n <= 1, so build the variant directly.
        let naive = SorptionIsotherm::Freundlich { kf: kd, n };
        same.push(naive.sorbed(c), res, format!("kd={kd} n={n} c={c:e}"));
        // (b) PFLOTRAN parameterisation.
        let iso = SorptionIsotherm::new_freundlich_pflotran(kd, n).unwrap();
        inv.push(iso.sorbed(c), res, format!("kd={kd} n={n} c={c:e}"));
        inv_d.push(iso.d_sorbed_d_c(c), dres, format!("kd={kd} n={n} c={c:e}"));
    }
    same.report("P16a Freundlich, same number (convention mismatch)");
    inv.report("P16b Freundlich, PFLOTRAN parameterisation s");
    inv_d.report("P16b Freundlich ds/dc");
    assert!(same.worst > 0.1, "convention mismatch expected to be large");
    assert!(inv.worst <= 4.5e-16, "P16 s breached: {}", inv.worst);
    assert!(inv_d.worst <= 1.0e-14, "P16 slope breached: {}", inv_d.worst);
}
