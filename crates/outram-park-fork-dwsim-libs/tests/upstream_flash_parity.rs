// SPDX-License-Identifier: GPL-3.0-only
//! # V&V — PT-flash parity against upstream DWSIM (code-to-code)
//!
//! **Methodology.** Upstream DWSIM 9.0.5.0 was built from the pinned commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` and run headless on Linux (the
//! procedure is in this crate's `CLAUDE.md`). An equimolar methane/ethane
//! binary was flashed at four (T, P) states through
//! `Calculator.CalcEquilibrium(PressureTemperature, …)` with the
//! Peng-Robinson package, and the same states were flashed here through
//! [`PropertyPackageModel::PengRobinson::flash_pt`]. Upstream's numbers are
//! recorded inline as the reference.
//!
//! **Result (measured 2026-09-13, release).** Vapour fraction `β` agrees to
//! 2-3 significant figures, not the 4 this crate's maturity bar requires:
//!
//! | T (K) | P (MPa) | β upstream | β here | Δ |
//! |---|---|---|---|---|
//! | 200 | 2 | 0.24348802 | 0.25028405 | +6.796e-3 |
//! | 220 | 3 | 0.29612708 | 0.30257322 | +6.446e-3 |
//! | 180 | 1 | 0.30683985 | 0.31266531 | +5.825e-3 |
//! | 250 | 5 | 0.44068245 | 0.44813218 | +7.450e-3 |
//!
//! Vapour compositions agree to ~1e-3, liquid compositions to ~5e-3.
//!
//! **Interpretation — the cause is identified, and it is not the EOS.**
//! Upstream's `Z_PR` and this crate's PR EOS agree to 6 significant figures
//! (see `thermo::cubic_eos`), and every compound constant is identical:
//! methane Tc 190.56 K, Pc 4.599 MPa, ω 0.011; ethane Tc 305.32 K,
//! Pc 4.872 MPa, ω 0.099. The difference is the **binary interaction
//! parameter**. Upstream's `Assets/kij_pr.dat` gives
//! `k(methane, ethane) = -0.0033`; this crate applies `k_ij = 0`, because the
//! parameter is structurally unreachable — `None` is always passed (recorded
//! in `docs/multi-thermo-ownership-audit.md`). A negative `k_ij` raises the
//! mixture attraction and lowers `β`, which is the direction and rough
//! magnitude of every row above.
//!
//! **This test asserts the gap rather than hiding it.** It fails if the
//! deviation grows, and is written to be tightened to a 4-significant-figure
//! equality once `k_ij` is reachable and populated. Do not "fix" it by
//! loosening the bound.
//!
//! > Verification against upstream, not validation against experiment. Neither
//! > code is checked against measured VLE data here.

use outram_park_fork_dwsim_libs::prelude::*;

/// Upstream DWSIM reference: (T [K], P [Pa], β, y_methane, x_methane).
const UPSTREAM: [(f64, f64, f64, f64, f64); 4] = [
    (200.0, 2e6, 0.24348802, 0.88797377, 0.37513155),
    (220.0, 3e6, 0.29612708, 0.81249291, 0.36853890),
    (180.0, 1e6, 0.30683985, 0.92558685, 0.31161067),
    (250.0, 5e6, 0.44068245, 0.65487999, 0.37798112),
];

#[test]
fn pt_flash_tracks_upstream_dwsim_to_the_kij_difference() {
    let comps = vec![reference::methane(), reference::ethane()];
    let z = [0.5, 0.5];
    let mut worst_beta: f64 = 0.0;
    let mut worst_y: f64 = 0.0;

    for (t, p, beta_up, y0_up, x0_up) in UPSTREAM {
        let r = PropertyPackageModel::PengRobinson
            .flash_pt(&comps, &z, t, p)
            .unwrap_or_else(|e| panic!("flash failed at T={t} K, P={p} Pa: {e:?}"));

        let d_beta = (r.beta - beta_up).abs();
        let d_y = (r.y[0] - y0_up).abs();
        worst_beta = worst_beta.max(d_beta);
        worst_y = worst_y.max(d_y);

        // Both codes must at least agree that this state is two-phase.
        assert!(
            r.beta > 0.0 && r.beta < 1.0,
            "T={t} K, P={p} Pa: expected two phases, got β={}",
            r.beta
        );
        // Same sign of deviation every time — the k_ij signature, not noise.
        assert!(
            r.beta > beta_up,
            "T={t} K, P={p} Pa: β={} should exceed upstream {beta_up} while k_ij = 0",
            r.beta
        );
        assert!(
            (r.x[0] - x0_up).abs() < 1e-2,
            "T={t} K, P={p} Pa: liquid composition drifted beyond the k_ij gap"
        );
    }

    // Pins today's measured gap. Tighten to 1e-4 once k_ij is reachable.
    assert!(
        worst_beta < 1e-2,
        "β gap widened to {worst_beta:.3e}; upstream parity regressed"
    );
    assert!(
        worst_y < 5e-3,
        "vapour-composition gap widened to {worst_y:.3e}"
    );
    println!("worst Δβ = {worst_beta:.3e}, worst Δy = {worst_y:.3e}");
}
