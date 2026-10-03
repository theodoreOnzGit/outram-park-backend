// SPDX-License-Identifier: GPL-3.0-only
//! # V&V — Gibbs-reactor parity against upstream DWSIM (code-to-code)
//!
//! **Methodology.** Upstream DWSIM, built from the pinned commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` and run headless on Linux
//! (driver `docs/upstream-harness/gibbs_driver.cs`). Each run builds a real
//! `Reactor_Gibbs` on a headless flowsheet (inlet, vapour and liquid outlets,
//! energy stream; Peng-Robinson; isothermal; reactive phase = vapour), sets the
//! reacting compounds CH4, H2O, CO, CO2, H2 and the C/H/O formula matrix, and
//! calls `Calculate()`. Feed 1 mol/s CH4 + 3 mol/s H2O, 1100 K, at 1, 10 and
//! 20 bar — steam-methane-reforming equilibrium, all vapour.
//!
//! Upstream has two solution methods and both were run:
//!
//! - **`Calculate_GibbsMin`** (default, `Gibbs.vb:1059`): direct minimisation
//!   over species flows of `exp(G) + 100·Σ(element residual)² + 100·(mass
//!   residual)²` (`Gibbs.vb:1405-1449`). The default solver is IPOPT, which has
//!   no Linux native library in the upstream tree (`PlatformFiles/Linux` has
//!   only `liblpsolve55.so`), so `UseIPOPTSolver = False` selects upstream's
//!   managed BFGS-B on the **same** objective (`Gibbs.vb:1427-1449`).
//! - **`Calculate_Lagrange`** (`AlternateSolvingMethod = True`,
//!   `Gibbs.vb:1743`): element potentials + Newton with successive substitution
//!   of fugacity coefficients — the formulation this crate's RAND minimiser
//!   implements (exact `A n = b`, `μ_i/RT = Σ_k a_ki π_k`).
//!
//! **What is held equal, so that solvers are compared rather than data.** This
//! crate's [`GibbsReactor`] takes caller-supplied `g°_i(T)` and has no compound
//! database for CO or H2 (none of its seven reference compounds carries
//! formation data), so it is given upstream's own values: `DELGF_i =
//! AUX_DELGF_T(298.15, T, i)·MW_i`, the dimensionless `g°_i(T)/RT` both upstream
//! methods use (`Gibbs.vb:1371`, `:2132`; `PropertyPackage.vb:8075-8119`,
//! Gibbs–Helmholtz with the compound's ideal-gas Cp integral), as
//! `GibbsFormation::Constant(DELGF_i·R·T)` with this crate's `R` so that `g°/RT`
//! is bit-equal. The reference pressure is upstream's `P0 = 101 325` Pa
//! (`Gibbs.vb:386`). Non-ideality: upstream evaluates PR fugacity coefficients
//! at the current composition; this crate is given upstream's PR `ln φ_i` *at
//! upstream's converged outlet* as `FugacityModel::FrozenLnPhi`. At a converged
//! minimum the stationarity conditions with `φ(x*)` are the same whether `φ`
//! is frozen or live (Gibbs–Duhem), so this is exact at the solution.
//!
//! **Results (measured 2026-10-02, release).**
//!
//! | comparison | 1 bar | 10 bar | 20 bar |
//! |---|---|---|---|
//! | vs `Calculate_GibbsMin`, worst per-species gap | 8.5e-6 | 7.6e-6 | 5.9e-6 |
//! | upstream GibbsMin element imbalance (relative) | 2.05e-6 | 2.08e-6 | 2.07e-6 |
//! | this crate's element imbalance | 2.0e-14 | 7.5e-15 | 6.7e-15 |
//! | vs `Calculate_Lagrange`, with its pressure term | **ConvergenceError** | 3.5e-7 | 1.6e-9 |
//! | vs `Calculate_Lagrange`, without it | — | 3.6e-4 | 3.2e-4 |
//!
//! Methane at equilibrium (this crate, PR φ): 0.001277934, 0.088231077,
//! 0.203302668 mol/s at 1, 10, 20 bar.
//!
//! **What the comparison found.**
//!
//! 1. **The port's minimiser is right.** Against the default upstream path the
//!    residual gap (≤ 8.5e-6) is of the order of upstream's own element
//!    imbalance (2e-6), which comes from enforcing `A n = b` by a weight-100
//!    penalty: no exact solver can reproduce it. This crate closes the element
//!    balance to 1e-14.
//! 2. **Upstream defect — dimensional slip in the Lagrange path.**
//!    `igcp(i) = DELGF_i + Log(P/P0)/(8.314·T)` (`Gibbs.vb:1998`, `:2132`,
//!    `:2246`), and `FunctionValue2N` then adds `Log(φ·P/P0)` again
//!    (`Gibbs.vb:559`). The extra term is dimensionally a log divided by `RT`;
//!    the LP initialiser two lines away uses `+ Log(P/P0)` without it
//!    (`Gibbs.vb:2038`). Predicted before measuring: a uniform `+c` on every
//!    `μ°/RT` lowers the reforming `K` by `exp(−2c)` (`Δν = +2`), so upstream's
//!    methane should come out *high*. Measured: it is high (0.2033671 vs
//!    0.2033027 at 20 bar), and adding exactly `c` to this crate's `g°/RT`
//!    reproduces upstream to 1.6e-9 (20 bar) and 3.5e-7 (10 bar; upstream's
//!    loose outer-loop stop, element imbalance 6.8e-8). Small at these
//!    conditions (3e-4), but a units error, and it grows with `ln P`.
//! 3. **Upstream defect — the Lagrange path does not converge at low
//!    pressure.** It throws `ConvergenceError` (`Gibbs.vb:2345-2353`) at 1.013,
//!    1.2 and 2 bar on this case, independent of `InternalTolerance` (1e-20,
//!    1e-12, 1e-8 all fail), and succeeds at 5, 10 and 20 bar. Root cause not
//!    traced; the suspect is the four-phase `nv/nl1/nl2/ns` Newton system
//!    (`FunctionValue2N`, `Gibbs.vb:530-598`) with absent phases seeded at
//!    `1e-4·N0` (`Gibbs.vb:2185-2188`).
//! 4. **Upstream outlet artefact.** At 10 bar both methods write a round-off
//!    mass flow (2.3e-17 and −1.6e-17 kg/s) to the *liquid* outlet of an
//!    all-vapour reactor, and that stream's own calculation then returns NaN
//!    molar flows. The driver treats a non-finite liquid outlet as zero and
//!    says so (`FL_NONFINITE`); a flowsheet consumer would see NaN.
//!
//! **What this crate's modelling choices cost** (methane, relative to the
//! PR-φ result above; `modelling_choices_have_recorded_costs`):
//!
//! | choice | 1 bar | 10 bar | 20 bar |
//! |---|---|---|---|
//! | `FugacityModel::IdealGas` instead of PR φ | −6.1e-4 | −4.5e-3 | −6.4e-3 |
//! | default `p_ref = 1e5` instead of upstream's 101 325 Pa | +2.7e-2 | +1.9e-2 | +1.3e-2 |
//! | `GibbsFormation::EnthalpyEntropy` from 25 °C data (no Cp) | **+10.2×** | +2.7× | +1.3× |
//!
//! The last row is the cost of the port's own two-parameter `g°` model: with
//! only 25 °C formation data it misses the heat-capacity integral, which at
//! 1100 K moves reforming `ΔG°` by enough to put methane 2.3× to 11× too high.
//! It is a data/model choice for the caller, not a solver defect; the module
//! docs of `reactors::gibbs_reactor` now say so.
//!
//! **Not compared:** the IPOPT solver itself (no Linux native library), the
//! adiabatic and outlet-temperature modes (not ported, coverage row R7), and
//! multi-phase / solid-carbon equilibria (not ported).
//!
//! > Verification against upstream, not validation against experiment.
//!
//! [`GibbsReactor`]: outram_park_fork_dwsim_libs::reactors::GibbsReactor

use outram_park_fork_dwsim_libs::reactors::{GibbsFormation, GibbsReactor, ReactorFeed};
use outram_park_fork_dwsim_libs::thermo::gibbs::{FugacityModel, GibbsOptions, GibbsSystem};
use outram_park_fork_dwsim_libs::thermo::ideal_props::R;

/// Temperature of every case [K].
const T: f64 = 1100.0;
/// Upstream's standard pressure `P0` (`Gibbs.vb:386`, `:1922`) [Pa].
const P0_UPSTREAM: f64 = 101_325.0;
/// Upstream's gas constant in the Lagrange path's pressure term (`Gibbs.vb:2132`).
const R_UPSTREAM: f64 = 8.314;

/// Upstream `AUX_DELGF_T(298.15, 1100 K, i) · MW_i` = `g°_i(T)/RT`
/// (`PropertyPackage.vb:8075-8119`), species CH4, H2O, CO, CO2, H2.
const DELGF: [f64; 5] = [
    -1.701_294_912_377_592,
    -23.560_955_565_319_727,
    -24.906_442_399_338_31,
    -46.421_653_180_741_92,
    -2.033_141_428_509_660_7,
];
/// Upstream 25 °C ideal-gas Gibbs energy / enthalpy of formation [J/mol].
const DGF25: [f64; 5] = [
    -50_490.0,
    -228_590.000_000_000_03,
    -137_150.0,
    -394_370.000_000_000_06,
    0.0,
];
const DHF25: [f64; 5] = [-74_520.0, -241_814.0, -110_530.0, -393_510.0, 0.0];

/// Feed [mol/s]: CH4, H2O, CO, CO2, H2.
const FEED: [f64; 5] = [1.0, 3.0, 0.0, 0.0, 0.0];

/// One upstream run: pressure, outlet flows, and upstream's own PR vapour
/// `ln φ_i` at that outlet.
struct Run {
    p: f64,
    flows: [f64; 5],
    lnphi: [f64; 5],
}

// Calculate_GibbsMin, UseIPOPTSolver = False (managed BFGS-B), the default
// objective (Gibbs.vb:1059-1547). 2026-10-02.
const GIBBSMIN: [Run; 3] = [
    Run {
        p: 1.0e5,
        flows: [
            0.001_277_923_020_237_617_8,
            1.670_305_533_596_313_1,
            0.667_743_489_791_835_73,
            0.330_978_561_583_038_23,
            3.327_140_584_335_132_3,
        ],
        lnphi: [
            0.000_278_884_944_902_443_04,
            -6.409_041_330_284_358_7e-5,
            0.000_308_552_413_172_996_65,
            0.000_213_185_048_438_487_12,
            0.000_171_688_388_730_135_81,
        ],
    },
    Run {
        p: 1.0e6,
        flows: [
            0.088_230_402_747_922,
            1.758_751_906_154_170_5,
            0.582_284_651_836_974,
            0.329_484_836_943_289_67,
            3.064_786_728_961_202_3,
        ],
        lnphi: [
            0.002_830_887_991_242_757,
            -0.000_706_630_415_236_055_91,
            0.003_136_273_957_358_450_9,
            0.002_135_444_232_663_286_9,
            0.001_746_327_185_828_114_4,
        ],
    },
    Run {
        p: 2.0e6,
        flows: [
            0.203_301_462_780_391_43,
            1.880_206_518_279_380_8,
            0.473_597_143_236_258_4,
            0.323_101_274_322_570_69,
            2.713_189_314_974_306_5,
        ],
        lnphi: [
            0.005_791_199_738_876_728_4,
            -0.001_598_116_616_925_019_9,
            0.006_426_999_794_614_825_4,
            0.004_287_840_890_526_408_9,
            0.003_585_713_135_297_117_6,
        ],
    },
];

// Calculate_Lagrange (AlternateSolvingMethod = True, Gibbs.vb:1743). At 1 bar
// it throws ConvergenceError (Gibbs.vb:2345-2353); see the module docs.
const LAGRANGE: [Run; 2] = [
    Run {
        p: 1.0e6,
        flows: [
            0.088_262_613_885_522_187,
            1.758_779_998_858_332_4,
            0.582_254_433_790_234_34,
            0.329_482_886_008_612_31,
            3.064_694_822_140_836_5,
        ],
        lnphi: [
            0.002_830_902_024_352_177_1,
            -0.000_706_655_274_752_031_39,
            0.003_136_291_845_257_752_9,
            0.002_135_444_416_454_614_5,
            0.001_746_337_629_635_482_1,
        ],
    },
    Run {
        p: 2.0e6,
        flows: [
            0.203_367_106_957_532_43,
            1.880_271_904_198_783_4,
            0.473_537_692_299_835_39,
            0.323_095_201_165_154_67,
            2.712_993_881_689_36,
        ],
        lnphi: [
            0.005_791_273_425_145_473_9,
            -0.001_598_228_750_726_442_1,
            0.006_427_090_317_704_472,
            0.004_287_848_080_383_449_4,
            0.003_585_768_667_373_419_6,
        ],
    },
];

/// Formula matrix, elements C, H, O by species CH4, H2O, CO, CO2, H2.
const EMAT: [[f64; 5]; 3] = [
    [1.0, 0.0, 1.0, 1.0, 0.0],
    [4.0, 2.0, 0.0, 0.0, 2.0],
    [0.0, 1.0, 1.0, 2.0, 0.0],
];

fn system() -> GibbsSystem {
    GibbsSystem::new(
        &["CH4", "H2O", "CO", "CO2", "H2"],
        &["C", "H", "O"],
        &[&EMAT[0], &EMAT[1], &EMAT[2]],
    )
    .unwrap()
}

/// This crate's reactor given upstream's `g°/RT` (plus an optional uniform
/// shift in `g°/RT`), reference pressure and fugacity model.
fn reactor(g_over_rt: [f64; 5], p_ref: f64, fug: FugacityModel) -> GibbsReactor {
    let g = g_over_rt
        .iter()
        .map(|d| GibbsFormation::Constant(d * R * T))
        .collect();
    let opts = GibbsOptions {
        tol: 1e-13,
        ..GibbsOptions::default()
    };
    GibbsReactor::new(system(), g)
        .with_p_ref(p_ref)
        .with_fugacity(fug)
        .with_options(opts)
}

fn solve(r: &GibbsReactor, p: f64) -> Vec<f64> {
    r.solve(&ReactorFeed::new(FEED.to_vec(), T, p, 0.0))
        .expect("Gibbs reactor converges")
        .molar_flows
}

fn worst_gap(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| ((x - y) / y).abs())
        .fold(0.0, f64::max)
}

/// Worst relative element imbalance of an outlet against the feed.
fn element_residual(flows: &[f64]) -> f64 {
    EMAT.iter()
        .map(|row| {
            let b0: f64 = row.iter().zip(FEED).map(|(a, n)| a * n).sum();
            let b: f64 = row.iter().zip(flows).map(|(a, n)| a * n).sum();
            ((b - b0) / b0).abs()
        })
        .fold(0.0, f64::max)
}

fn shifted(c: f64) -> [f64; 5] {
    DELGF.map(|d| d + c)
}

/// Upstream's Lagrange-path extra term `ln(P/P0)/(8.314·T)` (`Gibbs.vb:1998`,
/// `:2132`, `:2246`), added to every species' `g°/RT`.
fn lagrange_slip(p: f64) -> f64 {
    (p / P0_UPSTREAM).ln() / (R_UPSTREAM * T)
}

/// Gate for the Lagrange comparison: it must resolve the 2.5e-4 to 3.3e-4
/// pressure-term effect it identifies by at least two orders of magnitude.
const LAGRANGE_GATE: f64 = 1e-6;

/// Upstream `Calculate_Lagrange` at 10 and 20 bar against this crate given
/// upstream's `g°/RT` **plus** the path's extra `ln(P/P0)/(8.314·T)` term.
///
/// **Result (2026-10-02).** With the term: worst gap 3.5e-7 (10 bar), 1.6e-9
/// (20 bar). Without it: 3.6e-4 and 3.2e-4 — the comparison resolves the term
/// by three orders of magnitude, which is what identifies it as the cause.
/// Gate [`LAGRANGE_GATE`] = 1e-6, set to resolve that ~3e-4 effect by more
/// than 100×; the test also asserts that omitting the term fails by 100× the
/// gate, so the finding cannot silently disappear.
#[test]
fn lagrange_path_matches_once_its_pressure_term_is_included() {
    for run in &LAGRANGE {
        let fug = FugacityModel::FrozenLnPhi(run.lnphi.to_vec());
        let with_slip = solve(
            &reactor(shifted(lagrange_slip(run.p)), P0_UPSTREAM, fug.clone()),
            run.p,
        );
        let without = solve(&reactor(DELGF, P0_UPSTREAM, fug), run.p);
        let gap_with = worst_gap(&with_slip, &run.flows);
        let gap_without = worst_gap(&without, &run.flows);
        eprintln!(
            "lagrange P={}: slip c={:e}; gap with slip {gap_with:e}, without {gap_without:e}; upstream element residual {:e}",
            run.p,
            lagrange_slip(run.p),
            element_residual(&run.flows)
        );
        assert!(gap_with < LAGRANGE_GATE, "P={}: {gap_with:e}", run.p);
        assert!(
            gap_without > 100.0 * LAGRANGE_GATE,
            "P={}: {gap_without:e}",
            run.p
        );
    }
}

/// Upstream default `Calculate_GibbsMin` (BFGS-B) at 1, 10, 20 bar against this
/// crate with upstream's `g°/RT`, `P0` and converged PR `ln φ`.
///
/// **Result (2026-10-02).** Worst per-species gap 8.5e-6, 7.6e-6, 5.9e-6.
/// Upstream's outlets carry a 2.05e-6 to 2.08e-6 relative element imbalance
/// from its penalty formulation; this crate's is ≤ 2e-14 (asserted < 1e-12).
/// Gate [`GIBBSMIN_GATE`] = 5e-5 (reasoning on the constant).
#[test]
fn default_path_matches_within_its_penalty_residual() {
    for run in &GIBBSMIN {
        let fug = FugacityModel::FrozenLnPhi(run.lnphi.to_vec());
        let port = solve(&reactor(DELGF, P0_UPSTREAM, fug), run.p);
        let gap = worst_gap(&port, &run.flows);
        let up_res = element_residual(&run.flows);
        let port_res = element_residual(&port);
        eprintln!(
            "gibbsmin P={}: worst gap {gap:e}; element residual upstream {up_res:e}, port {port_res:e}; port {port:?}",
            run.p
        );
        assert!(port_res < 1e-12, "port element residual {port_res:e}");
        assert!(gap < GIBBSMIN_GATE, "P={}: {gap:e}", run.p);
    }
}

/// Gate for the default-path comparison: below the smallest model effect this
/// file measures (the 2.5e-4 Lagrange pressure term), so a real modelling
/// difference cannot hide in it, and above upstream's penalty-induced element
/// imbalance (up to 2.0e-6 relative), which no solver can reproduce exactly.
const GIBBSMIN_GATE: f64 = 5e-5;

/// What three of this crate's modelling choices cost against the PR-φ result,
/// recorded rather than gated: `IdealGas` fugacity, the default `p_ref = 1e5`
/// Pa, and the two-parameter `GibbsFormation::EnthalpyEntropy` fed upstream's
/// 25 °C data. Numbers in the module table (measured 2026-10-02). Pinned
/// loosely below so a change in any of them is noticed.
#[test]
fn modelling_choices_have_recorded_costs() {
    // Recorded 2026-10-02: (ideal gas, p_ref 1e5, two-parameter) relative CH4
    // shifts at 1, 10, 20 bar. Asserted to 1 % of each value.
    const RECORDED: [[f64; 3]; 3] = [
        [-6.054e-4, 2.653e-2, 10.22],
        [-4.490e-3, 1.893e-2, 2.657],
        [-6.417e-3, 1.281e-2, 1.324],
    ];
    for (run, rec) in GIBBSMIN.iter().zip(RECORDED) {
        let frozen = solve(
            &reactor(
                DELGF,
                P0_UPSTREAM,
                FugacityModel::FrozenLnPhi(run.lnphi.to_vec()),
            ),
            run.p,
        );
        let ideal = solve(&reactor(DELGF, P0_UPSTREAM, FugacityModel::IdealGas), run.p);
        let pref_1bar = solve(
            &reactor(DELGF, 1.0e5, FugacityModel::FrozenLnPhi(run.lnphi.to_vec())),
            run.p,
        );
        // Two-parameter g° = ΔH_f − T·ΔS_f from upstream's own 25 °C data
        // (no heat-capacity integral), via the port's EnthalpyEntropy model.
        let two_param: Vec<GibbsFormation> = (0..5)
            .map(|i| GibbsFormation::EnthalpyEntropy {
                delta_h_f: DHF25[i],
                delta_s_f: (DHF25[i] - DGF25[i]) / 298.15,
            })
            .collect();
        let tp = GibbsReactor::new(system(), two_param)
            .with_p_ref(P0_UPSTREAM)
            .with_fugacity(FugacityModel::FrozenLnPhi(run.lnphi.to_vec()))
            .with_options(GibbsOptions {
                tol: 1e-13,
                ..GibbsOptions::default()
            });
        let two = solve(&tp, run.p);
        let rel = |v: &[f64]| (v[0] - frozen[0]) / frozen[0];
        eprintln!(
            "costs P={}: CH4 frozen-phi {:.9}; ideal gas {:+.3e}; p_ref 1e5 {:+.3e}; two-parameter g° {:+.3e} (CH4 {:.6})",
            run.p,
            frozen[0],
            rel(&ideal),
            rel(&pref_1bar),
            rel(&two),
            two[0]
        );
        for (got, want) in [rel(&ideal), rel(&pref_1bar), rel(&two)]
            .into_iter()
            .zip(rec)
        {
            assert!(
                ((got - want) / want).abs() < 1e-2,
                "P={}: recorded cost moved: {got:e} vs {want:e}",
                run.p
            );
        }
    }
}
