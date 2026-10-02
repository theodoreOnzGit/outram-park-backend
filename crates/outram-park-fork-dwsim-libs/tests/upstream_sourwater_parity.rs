// SPDX-License-Identifier: GPL-3.0-only
//! # V&V — sour water and sour gas against upstream DWSIM (code-to-code)
//!
//! **Methodology.** Upstream DWSIM built from the pinned commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` and run headless on Linux, driver
//! `docs/upstream-harness/sourwater_driver.cs` (modes `spec`, `flash`,
//! `prflash`, `prphi`; procedure in that folder's README). Upstream numbers are
//! frozen below at full printed precision; this crate is recomputed on every
//! run. Three layers are compared:
//!
//! 1. **Sour-water chemistry** — `thermo::sour_water` against upstream's
//!    liquid-phase kernel `CalculateEquilibriumConcentrations`
//!    (`FlashAlgorithms/SourWater.vb:384-559`), called directly on the same
//!    total molalities: the SWEQ `K(T)`, the Henry volatilities
//!    (`PropertyPackages/SourWater.vb:205-249`), and the speciation / pH.
//! 2. **Sour-water VLE** — upstream's `SourWaterPropertyPackage` PT flash
//!    (`FlashAlgorithms/SourWater.vb:182-382`). This crate has **no** sour-water
//!    flash (module docs, "Honest scope"), so this layer records what upstream
//!    does, checked for consistency with its own correlations.
//! 3. **Sour gas on Peng-Robinson** — CH4 / CO2 / H2S / H2O at 300 K, 5 MPa,
//!    upstream `Calculator.CalcEquilibrium` vs this crate's PR with and without
//!    upstream's `k_ij`. This crate has **no H2S preset** (`component::reference`
//!    holds seven compounds), so all four compounds are built here from
//!    upstream's own ChemSep constants, printed by the driver: H2S
//!    `M = 34.08088 g/mol, Tc = 373.53 K, Pc = 8.96291 MPa, ω = 0.0941677,
//!    Tb = 212.8 K`.
//!
//! Cases are refinery sour-water conditions: a stripper **feed** at 313.15 K
//! (NH3 0.6, H2S 0.4, CO2 0.05 mol/kg, roughly 1 wt % NH3 and 1.4 wt % H2S),
//! the liquid of upstream's own **380 K / 1.8 bar** flash, and stripper
//! **bottoms** at 393.15 K (NH3 0.003, H2S 0.0003 mol/kg).
//!
//! **Results (measured 2026-10-02, release).**
//!
//! | quantity | worst gap | verdict |
//! |---|---|---|
//! | SWEQ `K1..K6, Kw` at 313.15 / 380 / 393.15 K | 2.3e-13 | agree |
//! | Henry volatilities NH3 / CO2 / H2S at 380 K | 2.0e-16 | agree |
//! | speciation vs upstream **diagnostic** build (`:475` fixed) | 1.8e-7 bottoms, 7.5e-9 hot | agree to upstream's charge tolerance |
//! | speciation vs **pristine** upstream | pH −0.592 / −0.212 / −0.114 | **upstream defect** (`:475`) |
//! | any CO2-bearing case | upstream: no answer | **upstream defect** (`:407`, `:535`) |
//! | sour-water PT flash | upstream y_H2S 573× its own SWEQ value | **upstream defect** (`:338`) |
//! | sour-gas PR, no `k_ij` vs upstream | K_CH4 −99.5 %, K_CO2 +531 %, K_H2S −77 %, K_H2O +36 % | cost of F28 |
//! | sour-gas PR, upstream's `k_ij` (i<j entries) | K_H2S −53 % | **upstream `k_ij` table is asymmetric** |
//!
//! **What the comparison found** (each traced to an upstream line, each
//! confirmed numerically rather than by reading):
//!
//! 1. **Upstream HS⁻ closure** (`FlashAlgorithms/SourWater.vb:475`):
//!    `HS⁻ = k5·[H2S]/([H⁺] + k5 + 2·k5·K6/[H⁺])` is the closed form for HS⁻
//!    from **total** sulfide (with `K6` doubled), applied to the **free** H2S of
//!    line 431. Upstream's result therefore violates its own reaction-5 mass
//!    action — at the feed `[HS⁻][H⁺]/[H2S] = 2.57e-10` against `K5 = 1.25e-7`.
//!    A build copy with line 475 written as `k5·conc0(H2S)/(H⁺ + k5 +
//!    k5·K6/H⁺)` (`docs/upstream-harness/sourwater_diagnostic_hs_closure.patch`)
//!    reproduces this crate to 1.8e-7, so line 475 is the whole disagreement.
//! 2. **Upstream carbon closure** (`:407`, `:535-541`): the CO2 balance compares
//!    `Σ c·M` (mass-weighted, carbamate halved — not a carbon mole balance) with
//!    `m0C = conc0(CO2)·m·44.01/1000`, where `m` is the kg of solution per mol
//!    of feed (`:271`). Every CO2 case hits "max iterations"; with `m` as the
//!    flash passes it, a 0.001 mol/kg case "converges" by deleting the carbon
//!    (1.9e-18 mol/kg left). This crate closes the carbon **mole** balance to
//!    1.4e-16.
//! 3. **Upstream sour-water flash discards the speciation** (`:338`
//!    `'Vxl = Vnl.NormalizeY` is commented out): the K-values come from Henry's
//!    law on the **total** (unspeciated) liquid molality of the initial
//!    NestedLoops flash (`:316-327` → `PropertyPackages/SourWater.vb:251-288`).
//!    SWEQ's volatilities apply to the **molecular** species, so at 380 K upstream
//!    puts 573× too much H2S and 565× too much CO2 in the vapour, and 3.0 % too
//!    much NH3. With and without the ionic species in the compound list
//!    upstream's flash agrees to 2e-5 — the chemistry does not reach the answer.
//!    At 313 K / 2 bar it returns a vapour mole fraction `y_H2S = 2.855`.
//! 4. **Upstream `k_ij` asymmetry** (`PengRobinson.vb:484-508`, `:920-921`):
//!    `RET_KIJ(i,j)` returns the `i→j` database entry first, and the database
//!    holds both orders with different values: `k(H2S,H2O) = 0.0394` but
//!    `k(H2O,H2S) = 0.0819`; `k(CO2,H2S) = 0.0978` but `k(H2S,CO2) = 0.1`. The
//!    mixture `a_m` (`:920`) sees only their mean, while ln φ_i
//!    (`aml2(i) = Σ_j x_j a(j,i)`, `:921`) sees the column — so ln φ is **not**
//!    the composition derivative of upstream's own `a_m`. Confirmed: this
//!    crate's mixture `Z` with the **mean** `k_ij` equals upstream's to 1.6e-15,
//!    and its liquid ln φ_H2S with the **column** `k_ij` is 4.5957 against
//!    upstream's 4.5962 (with the row value: 3.8505). No symmetric table
//!    reproduces upstream exactly; the closest (transposed) leaves K_H2O −0.64 %.
//!    The pure-component EOS agrees: pure water liquid `A`, `B`, `Z` to 1e-15
//!    and ln φ to 2.3e-6, the last from upstream's `√2 = 1.414213`
//!    (`PengRobinson.vb:1274-1275`).
//!
//! > Verification against upstream, not validation against experiment.

use outram_park_fork_dwsim_libs::thermo::component::Component;
use outram_park_fork_dwsim_libs::thermo::cubic_eos::{BinaryInteraction, CubicEos, Phase, R};
use outram_park_fork_dwsim_libs::thermo::flash::{nested_loops_flash, FlashResult, NestedLoopsOptions};
use outram_park_fork_dwsim_libs::thermo::sour_water::{
    equilibrium_constants, henry_volatility, Species, SourWaterFeed, SourWaterResult,
    SourWaterSystem,
};

fn rel(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        a.abs()
    } else {
        ((a - b) / b).abs()
    }
}

/// Upstream `Reaction.EvaluateK(T)` for the seven finite-`K` reactions of
/// `swreactions.dwrxm`, in upstream order (CO2 ionisation, carbonate, NH3
/// ionisation, carbamate, H2S ionisation, sulfide, water).
const K_UPSTREAM: [(f64, [f64; 7]); 3] = [
    (
        313.15,
        [
            1.938_839_713_548_292_1e-7,
            2.622_945_601_462_505_6e-11,
            1_939_737_434.087_733_3,
            2.111_109_830_670_978_6,
            1.254_189_018_372_667_4e-7,
            5.005_282_660_888_408_3e-14,
            2.866_597_452_428_401_8e-14,
        ],
    ),
    (
        380.0,
        [
            1.707_932_595_223_416_9e-7,
            3.142_055_664_184_486_8e-11,
            59_577_555.389_498_718,
            0.715_903_049_929_643_26,
            2.818_442_019_710_345_8e-7,
            1.253_098_258_628_658_4e-12,
            6.285_583_023_692_576_5e-13,
        ],
    ),
    (
        393.15,
        [
            1.464_092_597_430_461_6e-7,
            2.874_182_605_294_187_6e-11,
            34_520_445.069_306_396,
            0.604_320_854_303_067_7,
            2.914_875_634_538_454_7e-7,
            2.253_618_271_868_455_4e-12,
            9.124_634_329_971_715_5e-13,
        ],
    ),
];

/// The SWEQ equilibrium constants. **Result (2026-10-02):** worst relative gap
/// 2.3e-13 over 21 values; gate 1e-12.
#[test]
fn sweq_constants_match_upstream() {
    let mut worst = 0.0_f64;
    for (t, up) in K_UPSTREAM {
        let port = equilibrium_constants(t);
        for i in 0..7 {
            worst = worst.max(rel(port[i], up[i]));
        }
    }
    eprintln!("worst K gap {worst:e}");
    assert!(worst < 1e-12, "worst K gap {worst:e}");
}

/// Henry volatilities \[psia per mol/kg\] at 380 K on the liquid of upstream's
/// 380 K / 1.8 bar flash (`CAS` = 0.450336, `CC` = 0.000490, `CS` = 0.011074
/// mol/kg; upstream's `AUX_PVAPi_SW` divided back to psia/molal).
/// **Result (2026-10-02):** NH3 2.0e-16, CO2 0, H2S 1.3e-16; gate 1e-14.
#[test]
fn henry_volatilities_match_upstream() {
    let (nh3, co2, h2s) = henry_volatility(
        380.0,
        0.450_335_695_882_561_23,
        0.000_490_275_430_878_950_44,
        0.011_074_167_004_418_444,
    );
    for (port, up) in [
        (nh3, 4.454_646_840_121_302_6),
        (co2, 1_282.923_027_532_209_7),
        (h2s, 446.331_816_825_145_38),
    ] {
        assert!(rel(port, up) < 1e-14, "port {port} vs upstream {up}");
    }
}

/// Order used for the speciation comparisons below.
const SPECIES: [Species; 6] = [
    Species::HPlus,
    Species::Nh3,
    Species::Nh4Plus,
    Species::H2s,
    Species::HsMinus,
    Species::SMinus2,
];

fn speciate(t: f64, nh3: f64, h2s: f64) -> SourWaterResult {
    SourWaterSystem::at_temperature(t)
        .speciate(&SourWaterFeed::new(0.0, nh3, h2s))
        .expect("speciation converges")
}

/// NH3/H2S speciation against upstream's kernel on the **diagnostic** build,
/// in which only line 475 differs (the HS⁻ closure, see the module docs).
///
/// **Cases.** Stripper bottoms, 393.15 K, NH3 0.003 / H2S 0.0003 mol/kg; and
/// upstream's own 380 K flash liquid, NH3 0.45034 / H2S 0.011074 mol/kg (no
/// CO2 — upstream cannot solve a CO2 case, finding 2).
///
/// **Gate 1e-6 per species.** Upstream stops on `|charge| < 1e-10 mol/kg`
/// absolute (`SourWater.vb:501`), which is 2e-7 relative at the bottoms' ion
/// level (~5e-4 mol/kg); 1e-6 is that with margin. This crate's bisection
/// stops at 1e-13.
///
/// **Result (2026-10-02).** Bottoms: worst 1.8e-7 (S²⁻), pH 8.2734823 vs
/// 8.2734824. Hot liquid: worst 7.5e-9, pH 9.32284741 both.
#[test]
fn speciation_matches_upstream_diagnostic_build() {
    let cases: [(f64, f64, f64, [f64; 6]); 2] = [
        (
            393.15,
            0.003,
            0.0003,
            [
                5.327_428_331_784_143_3e-9,
                0.002_533_986_680_460_639,
                0.000_466_013_319_888_534_27,
                5.382_358_935_378_5e-6,
                0.000_294_493_064_076_660_54,
                1.245_769_832_664_134_4e-7,
            ],
        ),
        (
            380.0,
            0.45034,
            0.011074,
            [
                4.755_022_592_870_667_8e-10,
                0.437_933_662_454_724_93,
                0.012_406_337_545_728_896,
                1.860_265_101_465_488_6e-5,
                0.011_026_339_470_667_812,
                2.905_787_831_683_198_4e-5,
            ],
        ),
    ];
    for (t, nh3, h2s, upstream) in cases {
        let r = speciate(t, nh3, h2s);
        let worst = SPECIES
            .iter()
            .zip(upstream)
            .map(|(s, u)| rel(r.m(*s), u))
            .fold(0.0, f64::max);
        eprintln!("T = {t}: worst species gap {worst:e}");
        assert!(worst < 1e-6, "T = {t}: worst species gap {worst:e}");
    }
}

/// The same chemistry against the **pristine** upstream build: an upstream
/// defect, recorded and pinned (not parity).
///
/// **Result (2026-10-02).**
///
/// | case | upstream pH | this crate | HS⁻ upstream | HS⁻ this crate |
/// |---|---|---|---|---|
/// | feed 313.15 K, NH3 0.6 / H2S 0.4 | 9.589020 | 8.996895 | 0.19974 | 0.39679 |
/// | 380 K liquid, NH3 0.45034 / H2S 0.011074 | 9.534702 | 9.322847 | 0.0054987 | 0.011026 |
/// | bottoms 393.15 K, NH3 0.003 / H2S 0.0003 | 8.387544 | 8.273482 | 1.4883e-4 | 2.9449e-4 |
///
/// Predicted before measuring, from line 475 alone: at the feed, correct mass
/// action puts nearly all sulfide in HS⁻ (`K5/[H⁺] ≈ 487`), so this crate needs
/// more NH4⁺ to balance charge and sits near `pKa(NH4⁺) + log(0.2/0.4) ≈ 8.99`.
/// Upstream's result satisfies its own line-475 expression and violates `K5`
/// by a factor of 487; this crate satisfies `K5` to round-off.
#[test]
fn pristine_upstream_hs_closure_defect_is_recorded() {
    // Upstream pristine, feed case: H+, H2S, HS-, S-2 and pH.
    const H: f64 = 2.576_204_316_426_840_6e-10;
    const H2S: f64 = 0.200_223_256_648_825_15;
    const HS: f64 = 0.199_735_371_471_228_5;
    const PH_UP: [f64; 3] = [
        9.589_019_696_445_005_2,
        9.534_701_767_808_567,
        8.387_544_097_881_287_2,
    ];
    let k = equilibrium_constants(313.15);
    let (k5, k6) = (k[4], k[5]);

    // Upstream violates K5 and satisfies its own line 475.
    let k5_upstream = HS * H / H2S;
    assert!(
        k5 / k5_upstream > 480.0,
        "upstream K5 ratio {}",
        k5 / k5_upstream
    );
    let line_475 = k5 * H2S / (H + k5 + 2.0 * k5 * k6 / H);
    assert!(rel(line_475, HS) < 1e-4, "line 475 {line_475} vs {HS}");

    // This crate satisfies K5 and gives the recorded pH.
    let port = [
        speciate(313.15, 0.6, 0.4),
        speciate(380.0, 0.45034, 0.011074),
        speciate(393.15, 0.003, 0.0003),
    ];
    let k5_port = port[0].m(Species::HsMinus) * port[0].m(Species::HPlus) / port[0].m(Species::H2s);
    assert!(rel(k5_port, k5) < 1e-12, "port K5 {k5_port} vs {k5}");
    for ((r, up), dph) in port
        .iter()
        .zip(PH_UP)
        .zip([-0.592_124, -0.211_854, -0.114_062])
    {
        let d = r.ph - up;
        eprintln!("pH port {} upstream {up} delta {d}", r.ph);
        assert!((d - dph).abs() < 1e-5, "recorded pH gap moved: {d}");
    }
}

/// CO2-bearing sour water. Upstream gives no answer (finding 2); this crate
/// is checked against its own balances and mass-action laws.
///
/// **Upstream (2026-10-02).** Kernel at 313.15 K with `m = 1000`: NH3/H2S/CO2
/// 0.6/0.4/0.05, 0.6/0.4/0.001, 0.6/0/0.05 and 0/0/0.05 all throw "Flash PT:
/// max iterations" (or NaN); `m = 0.0183` (as the flash passes it): the 0.05
/// case throws, the 0.001 case returns total carbon 1.95e-18 mol/kg. The full
/// flash of the feed (313.15 K, 2 bar) throws with and without ions.
///
/// **This crate (2026-10-02).** Feed 0.6/0.4/0.05: pH 8.795376; C, N, S
/// balances 1.4e-16, 1.9e-16, 1.4e-16; net charge −1.5e-15 mol/kg; the seven
/// mass-action laws to ≤ 2.3e-13. Gates 1e-13 (balances) and 1e-11 (mass
/// action).
#[test]
fn co2_bearing_case_closes_balances() {
    let t = 313.15;
    let r = SourWaterSystem::at_temperature(t)
        .speciate(&SourWaterFeed::new(0.05, 0.6, 0.4))
        .unwrap();
    let m = |s| r.m(s);
    let carbon = m(Species::Co2)
        + m(Species::Hco3Minus)
        + m(Species::Co3Minus2)
        + m(Species::CarbamateMinus);
    let nitrogen = m(Species::Nh3) + m(Species::Nh4Plus) + m(Species::CarbamateMinus);
    let sulfur = m(Species::H2s) + m(Species::HsMinus) + m(Species::SMinus2);
    assert!(rel(carbon, 0.05) < 1e-13 && rel(nitrogen, 0.6) < 1e-13 && rel(sulfur, 0.4) < 1e-13);
    assert!(r.net_charge.abs() < 1e-13);
    let k = equilibrium_constants(t);
    let h = m(Species::HPlus);
    let laws = [
        rel(h * m(Species::Hco3Minus) / m(Species::Co2), k[0]),
        rel(h * m(Species::Co3Minus2) / m(Species::Hco3Minus), k[1]),
        rel(m(Species::Nh4Plus) / (h * m(Species::Nh3)), k[2]),
        rel(
            m(Species::CarbamateMinus) / (m(Species::Hco3Minus) * m(Species::Nh3)),
            k[3],
        ),
        rel(h * m(Species::HsMinus) / m(Species::H2s), k[4]),
        rel(h * m(Species::SMinus2) / m(Species::HsMinus), k[5]),
        rel(h * m(Species::OhMinus), k[6]),
    ];
    assert!(laws.iter().all(|&g| g < 1e-11), "{laws:?}");
    assert!((r.ph - 8.795_376).abs() < 1e-6, "pH {}", r.ph);
}

/// Upstream's sour-water PT flash at 380 K, 1.8 bar (feed water 55.508, NH3
/// 0.6, H2S 0.4, CO2 0.05 mol): `V = 0.0363167`. Recorded, not parity — this
/// crate has no sour-water flash.
///
/// **Check 1 — upstream's K-values are Henry's law on TOTAL molality.**
/// `y_i = H_i(CAS = total NH3) · m_i,total / P` reproduces upstream's vapour
/// to 1e-6 for NH3, H2S and CO2: the speciation is not used.
///
/// **Check 2 — what SWEQ intends.** This crate's speciation of the same
/// liquid (with upstream's K corrections, `speciate_corrected`) gives pH
/// 9.3064, free NH3 0.43736, free H2S 1.932e-5, free CO2 8.69e-7 mol/kg.
/// Henry's law on those molecular molalities gives `y = 0.07460 / 3.305e-4 /
/// 4.27e-5` (NH3 / H2S / CO2) against upstream's `0.07684 / 0.18933 /
/// 0.024093`: upstream's vapour carries **573×** the H2S and **565×** the CO2,
/// and +3.0 % NH3, at this fixed liquid. (A re-equilibrated flash would move
/// the liquid too; this is a consistency check, not a flash result.)
#[test]
fn upstream_sour_water_flash_uses_unspeciated_henry_law() {
    const P_PSIA: f64 = 180_000.0 * 0.000_145_038;
    // Upstream liquid molalities (total) and vapour fractions.
    let (m_nh3, m_h2s, m_co2) = (
        0.450_335_695_882_561_23,
        0.011_074_167_004_418_444,
        0.000_490_275_430_878_950_44,
    );
    let (y_nh3, y_h2s, y_co2) = (
        0.076_841_413_386_571_467,
        0.189_327_895_636_054_07,
        0.024_092_752_708_021_414,
    );

    // Check 1: Henry on total molality reproduces upstream.
    let (hn, hc, hs) = henry_volatility(380.0, m_nh3, m_co2, m_h2s);
    for (calc, up) in [
        (hn * m_nh3 / P_PSIA, y_nh3),
        (hs * m_h2s / P_PSIA, y_h2s),
        (hc * m_co2 / P_PSIA, y_co2),
    ] {
        assert!(rel(calc, up) < 1e-6, "{calc} vs {up}");
    }

    // Check 2: Henry on the molecular species.
    let r = SourWaterSystem::at_temperature(380.0)
        .speciate_corrected(&SourWaterFeed::new(m_co2, m_nh3, m_h2s), 1e-12, 200)
        .unwrap();
    let cc = r.m(Species::Co2)
        + r.m(Species::Hco3Minus)
        + r.m(Species::Co3Minus2)
        + r.m(Species::CarbamateMinus);
    let (hn, hc, hs) = henry_volatility(380.0, r.m(Species::Nh3), cc, m_h2s);
    let ratio_h2s = y_h2s / (hs * r.m(Species::H2s) / P_PSIA);
    let ratio_co2 = y_co2 / (hc * r.m(Species::Co2) / P_PSIA);
    let ratio_nh3 = y_nh3 / (hn * r.m(Species::Nh3) / P_PSIA);
    eprintln!("upstream/SWEQ: H2S {ratio_h2s}, CO2 {ratio_co2}, NH3 {ratio_nh3}");
    assert!(
        (ratio_h2s - 572.8).abs() < 1.0,
        "recorded H2S ratio moved: {ratio_h2s}"
    );
    assert!(
        (ratio_co2 - 564.5).abs() < 1.0,
        "recorded CO2 ratio moved: {ratio_co2}"
    );
    assert!(
        (ratio_nh3 - 1.030).abs() < 1e-3,
        "recorded NH3 ratio moved: {ratio_nh3}"
    );
}

// ---------------------------------------------------------------------------
// Sour gas on Peng-Robinson
// ---------------------------------------------------------------------------

/// CH4, CO2, H2S, H2O with upstream's ChemSep constants (printed by the
/// driver's `prflash` mode). Cp is irrelevant to a PT flash and left zero.
fn sour_gas_components() -> Vec<Component> {
    let c = |n: &str, mw: f64, tc: f64, pc: f64, w: f64, tb: f64| {
        Component::new(n, mw / 1000.0, tc, pc, f64::NAN, w, tb, [0.0; 5], f64::NAN).unwrap()
    };
    vec![
        c("Methane", 16.04246, 190.56, 4_599_000.0, 0.011, 111.66),
        c(
            "Carbon dioxide",
            44.0095,
            304.21,
            7_383_000.0,
            0.223621,
            194.686,
        ),
        c(
            "Hydrogen sulfide",
            34.08088,
            373.53,
            8_962_910.0,
            0.094_167_699_999_999_993,
            212.8,
        ),
        c("Water", 18.01528, 647.14, 22_064_000.0, 0.344, 373.15),
    ]
}

/// A symmetric `k_ij` table with upstream's CH4–CO2, CH4–H2O and CO2–H2O
/// values and the given H2S–H2O and CO2–H2S values (CH4–H2S is absent
/// upstream, i.e. 0).
fn kij(h2s_water: f64, co2_h2s: f64) -> BinaryInteraction {
    let mut k = BinaryInteraction::zeros(4);
    for (i, j, v) in [
        (0, 1, 0.0793),
        (0, 3, 0.5),
        (1, 2, co2_h2s),
        (1, 3, -0.12155),
        (2, 3, h2s_water),
    ] {
        k.set(i, j, v);
        k.set(j, i, v);
    }
    k
}

fn pr_flash(k: &BinaryInteraction) -> FlashResult {
    let comps = sour_gas_components();
    let (kk, cs) = (k.clone(), comps.clone());
    let closure = move |x: &[f64], y: &[f64], t: f64, p: f64| {
        let l = CubicEos::PengRobinson
            .ln_phi(&cs, x, t, p, Phase::Liquid, Some(&kk))
            .unwrap();
        let v = CubicEos::PengRobinson
            .ln_phi(&cs, y, t, p, Phase::Vapor, Some(&kk))
            .unwrap();
        l.iter()
            .zip(&v)
            .map(|(a, b)| (a - b).exp())
            .collect::<Vec<f64>>()
    };
    nested_loops_flash(
        &[0.78, 0.08, 0.12, 0.02],
        &comps,
        300.0,
        5.0e6,
        closure,
        NestedLoopsOptions::default(),
    )
    .expect("sour-gas flash converges")
}

/// Upstream `CalcEquilibrium` PT flash, z = CH4 0.78 / CO2 0.08 / H2S 0.12 /
/// H2O 0.02, 300 K, 5 MPa: vapour + aqueous liquid.
const UP_BETA: f64 = 0.980_778_810_768_901_5;
const UP_K: [f64; 4] = [
    4_867_825.353_504_478,
    50.343_247_097_743_145,
    141.002_809_886_740_81,
    0.000_844_040_247_067_782_47,
];

/// Sour gas: what the missing `k_ij` database (coverage row F28) costs, and the
/// upstream `k_ij` asymmetry.
///
/// **Result (2026-10-02)** — this crate's K vs upstream, by `k_ij` table:
///
/// | table | β_V | K_CH4 | K_CO2 | K_H2S | K_H2O |
/// |---|---|---|---|---|---|
/// | all zero (what the port does unprompted) | +2.7e-4 | −99.53 % | +531 % | −76.6 % | +35.8 % |
/// | upstream's `i<j` entries | −2.1e-5 | −2.6 % | −1.0 % | **−53.2 %** | +0.03 % |
/// | transposed (`H2O→H2S` 0.0819, `H2S→CO2` 0.1) | −6.3e-6 | −0.10 % | −0.07 % | −0.09 % | −0.64 % |
///
/// With no `k_ij`, CH4 is 214× too soluble in the water and the gas carries
/// 36 % more water — the number a dehydration design depends on. The
/// transposed row is the closest any symmetric table gets to upstream
/// (module finding 4). The gates pin those recorded gaps to ±0.5 % of
/// their value.
#[test]
fn sour_gas_kij_cost_is_recorded() {
    let rows: [(&str, BinaryInteraction, [f64; 4]); 3] = [
        (
            "zero",
            BinaryInteraction::zeros(4),
            [-0.995_341, 5.314_668, -0.766_493, 0.358_044],
        ),
        (
            "upstream i<j",
            kij(0.0394, 0.0978),
            [-0.025_669, -0.009_947, -0.532_163, 0.000_283],
        ),
        (
            "transposed",
            kij(0.0819, 0.1),
            [-0.001_052, -0.000_733, -0.000_904, -0.006_406],
        ),
    ];
    for (label, k, recorded) in rows {
        let r = pr_flash(&k);
        let gaps: Vec<f64> = r.k.iter().zip(UP_K).map(|(p, u)| p / u - 1.0).collect();
        eprintln!(
            "{label}: beta gap {:e}, K gaps {gaps:?}",
            r.beta / UP_BETA - 1.0
        );
        for (g, want) in gaps.iter().zip(recorded) {
            assert!(
                (g - want).abs() <= 0.005 * want.abs().max(1e-3),
                "{label}: K gap {g} moved from {want}"
            );
        }
    }
}

/// The EOS layer under the sour-gas flash: pure-water liquid at 300 K, 5 MPa,
/// and the mixture `Z` at upstream's aqueous composition with the **mean**
/// `k_ij` (which is what upstream's `a_m` sees).
///
/// **Result (2026-10-02).** Pure water: `A`, `B` equal to 1e-15, `Z =
/// 0.0425851202139790` both, ln φ −7.3736604 vs −7.3736581 (2.3e-6, from
/// upstream's `√2 = 1.414213`). Mixture: `Z` 0.0426425919444650 vs
/// 0.0426425919444649 (1.6e-15). Gates 1e-5 (ln φ) and 1e-12 (Z).
#[test]
fn sour_gas_eos_layer_matches_upstream() {
    let comps = sour_gas_components();
    let eos = CubicEos::PengRobinson;
    let (t, p) = (300.0, 5.0e6);

    let water = &comps[3..4];
    let ln_phi = eos
        .ln_phi(water, &[1.0], t, p, Phase::Liquid, None)
        .unwrap()[0];
    assert!(
        (ln_phi - (-7.373_658_104_477_972_8)).abs() < 1e-5,
        "water ln phi {ln_phi}"
    );

    let x = [
        1.633_762_493_242_006_9e-7,
        0.001_619_604_716_976_397_3,
        0.000_867_605_760_527_840_82,
        0.997_512_626_146_246_36,
    ];
    let k = kij((0.0394 + 0.0819) / 2.0, (0.0978 + 0.1) / 2.0);
    let a = eos.a_mix(&comps, &x, t, Some(&k)) * p / (R * t).powi(2);
    let b = eos.b_mix(&comps, &x) * p / (R * t);
    let z = eos.z_roots(a, b)[0];
    assert!(rel(z, 0.042_642_591_944_464_944) < 1e-12, "mixture Z {z}");
}
