// SPDX-License-Identifier: GPL-3.0-only
//! # V&V — rigorous-column parity against upstream DWSIM (code-to-code)
//!
//! **Methodology.** Upstream DWSIM 9.0.5.0, built from the pinned commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` and run headless on Linux
//! (procedure and driver: `docs/upstream-harness/`). A 10-stage equimolar
//! methane/ethane distillation column at 500 kPa, feed on stage 5, total
//! condenser, reflux ratio 2.0, bottoms rate 0.5 mol/s, Peng-Robinson.
//!
//! **Both solvers were given byte-identical initial estimates** — the profile
//! this crate's `RigorousColumn::distillation` generates — so what is compared
//! is the solver, not the starting guess. Upstream ran `WangHenkeMethod`, this
//! crate ran `ColumnSolverMethod::default()`.
//!
//! **Result (measured 2026-09-13, release).** Both converge, to *qualitatively
//! the same* column and *quantitatively different* numbers:
//!
//! | quantity | this crate | upstream DWSIM |
//! |---|---|---|
//! | iterations | 84 | 69 |
//! | final error | 8.841e-7 | 8.927e-8 |
//! | top-stage T (K) | 135.558 | 136.164 |
//! | bottom-stage T (K) | 214.567 | 204.631 |
//! | distillate x(CH4) | 0.983714 | 0.953814 |
//! | bottoms x(CH4) | 0.016282 | 0.046185 |
//!
//! Both give a methane-rich distillate and an ethane-rich bottoms with a
//! near-symmetric split, so the physics is the same shape. Top temperature
//! agrees to 0.6 K (0.4 %); bottom temperature differs by 9.9 K (4.9 %) and
//! distillate purity by 3.0 percentage points.
//!
//! One structural difference worth recording: upstream's converged profile has
//! **exactly zero vapour flow on stages 6-9** and a flat 0.5 mol/s liquid below
//! the feed — its stripping section collapses — while this crate retains a
//! small but non-zero vapour flow (about 0.04 mol/s) there.
//!
//! **Interpretation.** This is the same defect already measured one layer down,
//! compounded. The PT flash on this binary differs between the two codes
//! because upstream applies `k(methane, ethane) = -0.0033` from
//! `Assets/kij_pr.dat` and this crate applies `k_ij = 0` — the parameter is
//! structurally unreachable (`None` is always passed). See
//! `tests/upstream_flash_parity.rs` for that measurement and
//! `docs/multi-thermo-ownership-audit.md` for the defect. A column integrates
//! the flash over ten stages, so a ~2 % per-stage K difference becoming a ~5 %
//! bottom-temperature difference is the expected amplification, not a separate
//! fault. The EOS itself is identical between the two codes to 6 significant
//! figures.
//!
//! **This test pins today's agreement so it cannot silently worsen.** It is
//! deliberately *not* a 4-significant-figure equality: the bar cannot be met
//! while `k_ij` is unreachable. Tighten it once that lands — do not loosen it.
//!
//! > Verification against upstream, not validation against experiment. Neither
//! > code is checked against measured VLE or column data here.

use outram_park_fork_dwsim_libs::prelude::*;
use uom::si::catalytic_activity::katal;
use uom::si::molar_energy::joule_per_mole;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

/// Upstream DWSIM, same initial estimates, `WangHenkeMethod`.
const DWSIM_T_TOP: f64 = 136.1635;
const DWSIM_T_BOT: f64 = 204.6307;
const DWSIM_X0_CH4: f64 = 0.953814;
const DWSIM_X9_CH4: f64 = 0.046185;

#[test]
fn rigorous_column_tracks_upstream_dwsim_to_the_kij_difference() {
    let comps = vec![reference::methane(), reference::ethane()];
    let p = StagePressure::new::<pascal>(5.0e5);
    let t = StageTemperature::new::<kelvin>(160.0);
    let n = 10usize;
    let mut stages: Vec<Stage> = (0..n)
        .map(|i| Stage::new(format!("s{i}"), p, t, 2))
        .collect();
    stages[5] = stages[5].clone().with_feed(
        MolarFlowRate::new::<katal>(1.0),
        vec![0.5, 0.5],
        MolarEnthalpy::new::<joule_per_mole>(0.0),
    );
    let column = RigorousColumn::distillation(
        comps,
        PropertyPackageModel::PengRobinson,
        stages,
        ColumnSpec::reflux_ratio(2.0),
        ColumnSpec::product_molar_flow(MolarFlowRate::new::<katal>(0.5)),
    );

    let input = column.solver_input().expect("solver input");
    let out = ColumnSolverMethod::default()
        .solve(&input)
        .expect("this case converges; see the module docs for the operating point");

    let t_top = out.stage_temperatures[0];
    let t_bot = out.stage_temperatures[n - 1];
    let x0 = out.liquid_compositions[0][0];
    let x9 = out.liquid_compositions[n - 1][0];

    // Same column, qualitatively: methane up, ethane down, near-symmetric.
    assert!(x0 > 0.9, "distillate should be methane-rich, got x(CH4)={x0}");
    assert!(x9 < 0.1, "bottoms should be ethane-rich, got x(CH4)={x9}");
    assert!(
        t_bot > t_top + 50.0,
        "expected a real temperature gradient, got {t_top} -> {t_bot}"
    );

    // Quantitatively: pinned against upstream at today's k_ij = 0.
    assert!(
        (t_top - DWSIM_T_TOP).abs() < 2.0,
        "top-stage T drifted from upstream: ours {t_top}, DWSIM {DWSIM_T_TOP}"
    );
    assert!(
        (t_bot - DWSIM_T_BOT).abs() < 15.0,
        "bottom-stage T drifted from upstream: ours {t_bot}, DWSIM {DWSIM_T_BOT}"
    );
    assert!(
        (x0 - DWSIM_X0_CH4).abs() < 0.05,
        "distillate purity drifted from upstream: ours {x0}, DWSIM {DWSIM_X0_CH4}"
    );
    assert!(
        (x9 - DWSIM_X9_CH4).abs() < 0.05,
        "bottoms purity drifted from upstream: ours {x9}, DWSIM {DWSIM_X9_CH4}"
    );

    println!(
        "ours: it={} err={:e} Ttop={t_top:.4} Tbot={t_bot:.4} x0={x0:.6} x9={x9:.6}",
        out.iterations_taken, out.final_error
    );
}
