// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! # V&V: Edwards–O'Brien pipe blowdown on [`DriftFlux1d`] (drift flux, 4 eqn)
//!
//! This is the **drift-flux** half of a deliberate test-ownership split: the
//! HEM (three-equation) Edwards case lives in
//! `crates/tampines-steam-tables/tests/edwards_blowdown.rs`; the four-equation
//! drift-flux and the (unwritten) six-equation two-fluid cases live here. The
//! two crates run the *same* experiment through *different* physics so the
//! rungs of the system-code ladder can be compared on one benchmark.
//!
//! # Methodology
//!
//! - **What is computed.** The 600 ms transient depressurisation of a
//!   horizontal water-filled pipe after a rupture disc bursts, marched by
//!   [`DriftFlux1d`] — mixture mass / momentum / energy plus a vapour-mass
//!   equation with the algebraic Zuber-Findlay slip closure taken from
//!   [`outram_foam_multiphase::drift_flux::SlipModel`].
//! - **Geometry** (Tomlinson & Aumiller B-T-3271 Table 1, the modified
//!   RELAP5-3D nodalisation of Edwards & O'Brien 1970): `L = 4.096 m`,
//!   `D = 0.073 m`, horizontal, `A = πD²/4 = 4.185e-3 m²`, 24 uniform cells
//!   (one per RELAP volume). Closed at `x = 0`, break at `x = L`.
//! - **Initial condition.** `p = 7.00 MPa` (1000 psig) uniform; the **Hendrie
//!   (1973) non-isothermal axial temperature profile** (436.0–452.5 degF)
//!   interpolated onto the cell centres and flashed through IF97 — not
//!   isothermal, which is the single most important modelling detail of the
//!   benchmark.
//! - **Break boundary.** [`AxialBoundary::ChokedOutlet`] with the 13 %
//!   glass-disc residue (`A_break = 0.87 A`) opened over the 1.0 ms linear
//!   rupture-disc ramp, re-imposed every step, discharging to 0.1 MPa ambient.
//!   **The break model is HEM even though the pipe is drift-flux** — the
//!   boundary condition delegates to the crate's HEM critical-flow dispatcher.
//!   That inconsistency is documented at the boundary condition itself and is
//!   restated here because it materially shapes the result below.
//! - **Timestep** `dt = 30 µs`, fixed — the same value the HEM case uses, so
//!   the two are compared at the same discretisation.
//! - **Reference data.** The digitised Edwards experimental "Data" pressure at
//!   gauge station **GS-1** (nearest the break), 16 points over 0–0.30 s. See
//!   [`GS1_DATA_PSIA`] for its full provenance: transcribed 2026-08-11 from the
//!   HEM test file, which traced it from B-T-3271 Figure 3 with graphreader.com.
//!   It is a digitisation of a published figure, not raw instrument data, and
//!   carries no error bars.
//! - **Pass criterion** (gated by
//!   [`edwards_drift_flux_gs1_pressure_history`]): the full 20 000-step
//!   transient completes; GS-1 pressure RMSE against the digitised curve
//!   `<= 32.0` psia and worst point deviation `<= 60` psia over all 16 points;
//!   the flashing plateau (mean GS-1 over 0.02–0.06 s) inside 330–380 psia;
//!   the rarefaction reaches GS-1 before GS-7; the pipe drains `> 99 %` and
//!   every gauge ends within 13–16 psia of the 14.5 psia ambient; no cold tail
//!   (min gauge `T > 350 K` past 0.42 s) and no tail pressure rebound
//!   (`<= 1` psia); void everywhere in `[0, 1]`, material Courant `< 1`; and
//!   the slip closure is measurably active (`peak |v_g − v_l| > 1 m/s`), so a
//!   silently-inert closure cannot pass as a drift-flux result.
//!
//! # Results (measured 2026-08-11, release, 24 cells, `dt = 30 µs`, full 600 ms)
//!
//! The full transient completes — 20 000 steps, **39.00 s wall clock** (three
//! runs on 2026-08-11: 39.00 s, 39.79 s, 36.15 s; the spread is machine load,
//! and all three produced identical numbers, the last of them against a
//! freshly rebuilt `tampines-steam-tables`), no NaN, no step refused, max
//! material Courant `0.0254`.
//!
//! GS-1 pressure against the digitised experimental curve:
//!
//! ```text
//!   t [s] | drift flux [psia] | Edwards data [psia] |  diff | GS-1 void | break mdot [kg/s]
//!   0.000 |            1015.3 |               985.0 | +30.3 |    0.0000 |   0.000
//!   0.020 |             341.7 |               350.8 |  -9.1 |    0.3774 |  41.547
//!   0.040 |             356.4 |               364.0 |  -7.6 |    0.4094 |  42.757
//!   0.060 |             360.4 |               367.4 |  -6.9 |    0.5095 |  41.743
//!   0.080 |             364.5 |               343.0 | +21.5 |    0.5206 |  41.872
//!   0.100 |             364.3 |               312.2 | +52.0 |    0.5175 |  41.793   <- worst
//!   0.120 |             342.6 |               298.2 | +44.4 |    0.6151 |  38.133
//!   0.140 |             336.2 |               296.0 | +40.2 |    0.6519 |  37.039
//!   0.160 |             330.9 |               288.7 | +42.3 |    0.6784 |  36.070
//!   0.180 |             325.4 |               288.4 | +37.0 |    0.7043 |  35.043
//!   0.200 |             318.3 |               289.0 | +29.3 |    0.7320 |  33.789
//!   0.220 |             306.3 |               283.1 | +23.2 |    0.7610 |  31.954
//!   0.240 |             278.5 |               271.4 |  +7.1 |    0.8070 |  28.147
//!   0.260 |             252.8 |               253.0 |  -0.1 |    0.8454 |  24.722
//!   0.280 |             237.6 |               228.0 |  +9.6 |    0.8666 |  22.916
//!   0.300 |             219.7 |               190.1 | +29.7 |    0.8801 |  20.857
//! ```
//!
//! - **GS-1 RMSE vs Data: 29.0 psia** over all 16 points (0–0.30 s), i.e. about
//!   **8 %** of the ~355 psia plateau level. Worst deviation **+52.0 psia at
//!   t = 0.100 s**, where the model holds the plateau ~50 psia too long before
//!   starting its decline.
//! - **Flashing plateau: 354.3 psia** (mean GS-1, 0.02–0.06 s) against the
//!   ~350–367 psia experimental plateau — **within the experimental band**, and
//!   close to the saturation pressure of the local initial temperature.
//! - **Rarefaction propagation**, GS-1 leading GS-7 as it must (psia): at
//!   `t = 1 ms` GS-1 362.6 / GS-2 361.3 / GS-3 931.4 / GS-4…GS-7 still 1015.3;
//!   at `t = 2 ms` the wave has reached GS-4 (917.9) with GS-7 at 1015.3; by
//!   `t ≈ 4 ms` the whole pipe has depressurised. That is ~4.1 m in ~3.5 ms,
//!   an apparent wave speed of ~1.2 km/s, the right order for subcooled water.
//! - **Break flow: peak 45.79 kg/s (101.0 lbm/s) at t = 0.002 s**, choked from
//!   the disc opening until the pipe reaches ambient.
//! - **Emptying:** inventory 14.2319 kg → 0.0465 kg (**99.7 % drained**); every
//!   gauge ends at 14.4–14.6 psia against the 14.5 psia ambient.
//! - **No artificial-cooling tail.** Minimum gauge temperature past 0.42 s is
//!   **372.6 K (99.5 degC)** — nowhere near the ~274 K collapse an explicit
//!   scheme shows — and the GS-1 tail rebound is **+0.0 psia**.
//! - **Mechanical non-equilibrium is real and large in the velocity field:**
//!   peak `|v_g − v_l| = 88.4 m/s` over the run (57.6 m/s within the first
//!   0.3 s), concentrated at the near-break cells.
//!
//! # Multi-fidelity comparison — drift flux vs HEM vs experiment
//!
//! **The HEM figures below are QUOTED, not measured here.** They are the
//! numbers recorded in the doc comments of
//! `crates/tampines-steam-tables/tests/edwards_blowdown.rs` (measured
//! 2026-07-16 by that crate's authors, 24 cells, `dt = 30 µs`), read on
//! 2026-08-11. The 897 s HEM run was **not** re-executed for this comparison,
//! so the two columns come from different sessions and possibly different
//! machines; treat differences smaller than a few psia as noise.
//!
//! ```text
//! metric (GS-1)                    drift flux (measured   HEM PIMPLE   HEM Hybrid   experiment
//!                                   here, 2026-08-11)     (quoted)     (quoted)
//! RMSE vs digitised Data [psia]              29.0            ~60          30.6         --
//! flashing plateau 0.02-0.06 s [psia]       354.3           ~393         387.7      ~350-367
//! break flow peak [lbm/s]                   101.0          127.7           --       ~96-111 *
//! min tail T past 0.42 s [K]                372.6           ~388         ~372         --
//! GS-1 tail pressure rebound [psia]           +0.0           +0.0          --          --
//! ```
//!
//! `*` the ~96–111 lbm/s Henry-Fauske / experimental band is itself quoted from
//! the HEM test's doc comment, not independently sourced here.
//!
//! **Where they agree.** Both codes reproduce the qualitative structure —
//! immediate drop to a flashing plateau within 1 ms, a plateau held for
//! ~0.1 s, then a decline as the pipe empties, with no non-physical cold tail
//! or pressure rebound. Both are choked at the break for essentially the whole
//! transient. Both put the rarefaction at GS-1 well before GS-7.
//!
//! **Where they diverge, and by how much.** The plateau level: drift flux sits
//! at **354.3 psia**, HEM PIMPLE at **~393 psia** — HEM is **~39 psia (11 %)
//! higher**, and outside the ~350–367 psia experimental band that drift flux
//! lands inside. That carries the RMSE difference (29.0 vs ~60 psia). The
//! HEM *hybrid* mode narrows the gap (RMSE 30.6 psia, plateau 387.7 psia), so
//! the honest statement is that drift flux matches the digitised data about as
//! well as the best HEM configuration and better than the HEM baseline, with
//! its advantage concentrated in the plateau *level* rather than the shape.
//! The break flow differs the same way: 101.0 lbm/s here versus 127.7 lbm/s for
//! HEM, with the drift-flux value inside the quoted experimental band.
//!
//! **But the cause is NOT mechanical non-equilibrium** — and this is the most
//! important finding of this case. The sensitivity sweep
//! ([`edwards_drift_flux_slip_and_tau_sensitivity`], measured 2026-08-11 over
//! 0–0.30 s) turns the slip closure off entirely (`C₀ = 1`, `V_gj = 0`, i.e.
//! the HEM-with-void-transport limit) and the GS-1 pressure moves by **at most
//! 1.44 psia** — RMSE 28.9 psia either way, plateau 354.3 psia either way —
//! even though the peak slip velocity goes from 57.6 m/s to exactly 0. So on
//! this benchmark, at this gauge, **the drift-flux slip closure is measurably
//! active and measurably immaterial**: the ~39 psia plateau difference against
//! HEM comes from this solver's own numerics and closures (the vapour-relaxation
//! void equation, the semi-implicit pressure solve, the drift momentum flux),
//! not from letting the phases move at different speeds. Edwards-O'Brien is
//! therefore a **poor discriminator of mechanical non-equilibrium** and should
//! not be cited as evidence that drift flux beats HEM *because of slip*.
//!
//! # Sensitivity of the reported error (measured 2026-08-11, 0–0.30 s window)
//!
//! Recorded so the headline RMSE is not mistaken for a tuned result. Nothing
//! was tuned: the baseline uses the reference crate's own documented typical
//! Zuber-Findlay band and the solver's default `τ`, and it was the first
//! configuration run.
//!
//! ```text
//! variant                                 RMSE  worst  plateau  peak|vg-vl|  max|dp| vs base  max|dalpha|
//!                                        [psia] [psia]  [psia]      [m/s]         [psia]          [-]
//! baseline C0=1.13 Vgj=0.1 tau=1e-3        28.9   52.0    354.3       57.56           --            --
//! no slip  C0=1.00 Vgj=0.0                 28.9   51.8    354.3        0.00          1.44        0.0159
//! strong slip C0=1.20 Vgj=0.3              28.9   51.9    354.7       82.33          1.88        0.0110
//! tau = 1e-2 s (10x slower flashing)       28.7   51.8    354.6       25.54          1.63        0.1613
//! tau = 1e-4 s (10x faster flashing)       27.5   51.3    354.2       73.28          5.32        0.0257
//! mesh refinement: 48 cells                27.2   53.2    353.6       58.93          6.89        0.0713
//! ```
//!
//! Reading: **every model-parameter variation moves GS-1 by less than the
//! mesh refinement does** (6.89 psia). The result is dominated by
//! discretisation and by the equilibrium `(p, h)` flash, not by the drift-flux
//! model parameters — which is exactly why the parameters were not, and need
//! not be, tuned. `τ` reaches the void field far more than the pressure
//! (`max |Δα| = 0.16` at `τ = 10 ms`), so a case that reports *void* rather
//! than pressure would be far more `τ`-sensitive than this one.
//!
//! # What this does and does not establish
//!
//! - **Established, and gated by a test:** on the Edwards-O'Brien geometry and
//!   initial condition, [`DriftFlux1d`] reproduces the digitised GS-1 pressure
//!   history to an RMSE of 29.0 psia (~8 % of the plateau) over 0–0.30 s, holds
//!   a flashing plateau inside the experimental band, empties the pipe to
//!   ambient over 600 ms, and does so without a cold tail or a pressure
//!   rebound. That is the *only* claim the test gates.
//! - **NOT established.** GS-2…GS-7 are recorded and printed but **not** gated
//!   against data — the digitised curve transcribed here covers GS-1 only. Void
//!   fraction is not compared against anything (Edwards' void data is not
//!   digitised in this workspace). The break flow is not gated. The reference
//!   is a graph digitisation, so agreement below a few psia is not meaningful.
//!   And because the break boundary is HEM, this case does **not** validate a
//!   drift-flux critical-flow model — there isn't one.
//! - **Reproducibility caveat.** Repeat runs of the same build agree exactly;
//!   values shift by up to ~0.3 psia between *rebuilds* of this file
//!   (RMSE 28.9 vs 29.0 psia across two builds on 2026-08-11), so the gates
//!   carry headroom over the measured values rather than pinning them to the
//!   last digit.
//!
//! # Runtime knobs (V&V exploration, not part of any pass criterion)
//!
//! [`edwards_drift_flux_explore`] is an `#[ignore]`d scratch runner taking
//! `EDF_CELLS`, `EDF_DT_US`, `EDF_TEND`, `EDF_C0`, `EDF_VGJ` and `EDF_TAU`
//! from the environment.

use uom::si::angle::radian;
use uom::si::f64::{Angle, Length, Pressure, ThermodynamicTemperature, Time};
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_multiphase::drift_flux::SlipModel;

use super::drift_flux::{AxialBoundary, DriftFlux1d};
use super::geometry::Pipe1d;
use super::two_fluid::TwoFluid1d;

// ─────────────────────────────────────────────────────────────────────────────
//  Case constants — transcribed 2026-08-11 from
//  `crates/tampines-steam-tables/tests/edwards_blowdown.rs` (the HEM Edwards
//  case), which in turn cites Tomlinson & Aumiller B-T-3271 Table 1 (the
//  modified RELAP5-3D nodalisation of the Edwards & O'Brien 1970 experiment).
//  Transcribed rather than imported: `tampines` does not depend on that crate's
//  test target, and the two crates must be able to drift apart only visibly.
// ─────────────────────────────────────────────────────────────────────────────

/// Total pipe length \[m\] (13.44 ft).
const PIPE_LENGTH_M: f64 = 4.096;
/// Inside diameter \[m\] (2.88 in).
const PIPE_ID_M: f64 = 0.073;
/// Break flow area fraction \[-\] — 13 % glass-disc residue.
const BREAK_AREA_FRACTION: f64 = 0.87;
/// Rupture-disc opening time \[s\] — 1.0 ms linear ramp.
const DISC_OPEN_TIME_S: f64 = 1.0e-3;
/// Initial pipe pressure \[Pa abs\] — 1000 psig.
const P_INIT_PA: f64 = 7.0e6;
/// Containment / ambient back-pressure \[Pa abs\].
const P_AMBIENT_PA: f64 = 1.0e5;

const FT_TO_M: f64 = 0.3048;
const PA_TO_PSI: f64 = 1.0 / 6894.757_293;

/// Volume-centre axial position \[ft from the closed end\], RELAP volumes 1–24.
const NODE_CENTRE_FT: [f64; 24] = [
    0.260, 0.780, 1.300, 1.820, 2.385, 3.000, 3.615, 4.215, 4.820, 5.425, 6.025, 6.640, 7.255,
    7.820, 8.340, 8.940, 9.630, 10.320, 10.920, 11.440, 11.905, 12.370, 12.890, 13.295,
];
/// Hendrie (1973) non-isothermal initial temperature profile \[degF\] at
/// [`NODE_CENTRE_FT`]. The single most important modelling detail of the case.
const NODE_T_DEGF: [f64; 24] = [
    447.5, 448.4, 449.4, 450.3, 451.4, 452.5, 451.7, 450.8, 450.0, 449.2, 448.3, 447.5, 448.0,
    448.5, 448.9, 449.4, 450.0, 446.5, 443.4, 440.8, 438.4, 436.0, 437.0, 437.8,
];

/// Gauge-station axial positions \[ft from the closed end\], GS-1 … GS-7.
/// GS-1 is nearest the break (`x = L`); GS-7 sits at the closed end.
const GS_POS_FT: [(&str, f64); 7] = [
    ("GS-1", 12.890),
    ("GS-2", 12.370),
    ("GS-3", 9.630),
    ("GS-4", 6.640),
    ("GS-5", 4.820),
    ("GS-6", 3.000),
    ("GS-7", 0.260),
];

/// Digitised Edwards experimental "Data" pressure at GS-1 \[(t \[s\], psia)\].
///
/// **Provenance.** Transcribed verbatim on 2026-08-11 from `GS1_DATA_PSIA` in
/// `crates/tampines-steam-tables/tests/edwards_blowdown.rs`, whose own
/// provenance note reads: traced from Tomlinson & Aumiller B-T-3271 Figure 3
/// with graphreader.com, subsampled every 0.02 s, series "Data" of
/// `benchmark_digitised.csv`. It is a **digitisation of a published figure**,
/// not raw instrument data, so its own uncertainty is a few psia at best and it
/// carries no error bars.
const GS1_DATA_PSIA: [(f64, f64); 16] = [
    (0.000, 985.0),
    (0.020, 350.777),
    (0.040, 364.038),
    (0.060, 367.358),
    (0.080, 343.024),
    (0.100, 312.239),
    (0.120, 298.234),
    (0.140, 296.0),
    (0.160, 288.68),
    (0.180, 288.394),
    (0.200, 289.0),
    (0.220, 283.112),
    (0.240, 271.433),
    (0.260, 252.96),
    (0.280, 228.0),
    (0.300, 190.093),
];

fn degf_to_k(f: f64) -> f64 {
    (f - 32.0) * 5.0 / 9.0 + 273.15
}

/// Piecewise-linear interpolation with flat extrapolation outside the range.
fn lerp(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[xs.len() - 1] {
        return ys[ys.len() - 1];
    }
    for i in 0..xs.len() - 1 {
        if x >= xs[i] && x <= xs[i + 1] {
            let w = (x - xs[i]) / (xs[i + 1] - xs[i]);
            return ys[i] * (1.0 - w) + ys[i + 1] * w;
        }
    }
    *ys.last().unwrap()
}

/// Break area fraction at time `t` \[-\]: the 1 ms linear rupture-disc ramp,
/// then held at [`BREAK_AREA_FRACTION`].
fn break_area_fraction(t: f64) -> f64 {
    (t / DISC_OPEN_TIME_S).clamp(0.0, 1.0) * BREAK_AREA_FRACTION
}

/// What one Edwards run produces: gauge histories plus break diagnostics.
struct EdwardsHistory {
    /// Sample times \[s\].
    times: Vec<f64>,
    /// Gauge pressures \[psia\], GS-1 … GS-7 per row.
    p_gs: Vec<[f64; 7]>,
    /// Gauge void fractions \[-\], GS-1 … GS-7 per row.
    a_gs: Vec<[f64; 7]>,
    /// Gauge temperatures \[K\], GS-1 … GS-7 per row — the artificial-cooling
    /// diagnostic the HEM case reports.
    t_gs: Vec<[f64; 7]>,
    /// Total pipe inventory \[kg\] at each sample.
    inventory: Vec<f64>,
    /// Slip velocity `v_g − v_l` \[m/s\] at the GS-1 cell at each sample — the
    /// mechanical non-equilibrium drift flux represents and HEM cannot.
    slip_gs1: Vec<f64>,
    /// Largest `|v_g − v_l|` \[m/s\] anywhere, over the whole run.
    max_slip: f64,
    /// Break mass flow \[kg/s\] at each sample, positive outward.
    mdot: Vec<f64>,
    /// Whether the break was choked at each sample.
    choked: Vec<bool>,
    /// Largest material Courant number seen over the whole run \[-\].
    max_courant: f64,
    /// Steps completed before the run ended (or a step failed).
    steps: usize,
    /// The error that stopped the march, if any.
    stopped: Option<String>,
}

/// One Edwards run's settings — mesh, timestep, and the two *model parameters*
/// the drift-flux model exposes.
///
/// The slip parameters and `tau` are called out as a struct rather than buried
/// in literals precisely because they are **not measured constants**: the
/// sensitivity of the reported error to them is part of the V&V result (see
/// [`edwards_drift_flux_slip_and_tau_sensitivity`]).
#[derive(Debug, Clone, Copy)]
struct EdwardsCase {
    /// Number of uniform axial cells (24 = one per RELAP volume).
    n_cells: usize,
    /// Fixed timestep \[s\].
    dt_s: f64,
    /// End of the integration \[s\].
    t_end: f64,
    /// Gauge sampling interval \[s\].
    sample_every: f64,
    /// Zuber-Findlay distribution parameter `C₀` \[-\].
    c0: f64,
    /// Zuber-Findlay drift velocity `V_gj` along `+x` \[m/s\].
    vgj_x: f64,
    /// Vapour-generation relaxation time `τ` \[s\].
    tau_s: f64,
}

impl Default for EdwardsCase {
    /// The baseline case: 24 cells, `dt = 30 µs` (the HEM case's timestep, so
    /// the two are compared at the same discretisation), churn-turbulent
    /// Zuber-Findlay values from the reference crate's own documented typical
    /// band (`C₀ ≈ 1.13`, `V_gj ≈ 0.1–0.3 m/s`, taken at the low end), and the
    /// solver's default `τ`.
    fn default() -> Self {
        Self {
            n_cells: 24,
            dt_s: 30.0e-6,
            t_end: 0.600,
            sample_every: 0.001,
            c0: 1.13,
            vgj_x: 0.1,
            tau_s: super::drift_flux::DEFAULT_VAPOUR_RELAXATION_TIME,
        }
    }
}

/// Drive [`DriftFlux1d`] through the Edwards blowdown and return the histories.
///
/// A failing step stops the march and is recorded in
/// [`EdwardsHistory::stopped`] rather than panicking, so a characterization
/// test can report exactly how far the solver got.
fn run_edwards(case: EdwardsCase) -> EdwardsHistory {
    let EdwardsCase {
        n_cells,
        dt_s,
        t_end,
        sample_every,
        c0,
        vgj_x,
        tau_s,
    } = case;

    let pipe = Pipe1d::circular(
        Length::new::<meter>(PIPE_LENGTH_M),
        Length::new::<meter>(PIPE_ID_M),
        Angle::new::<radian>(0.0),
        n_cells,
    )
    .expect("the Edwards pipe is well posed");

    // Zuber-Findlay churn-turbulent bubbly values, the reference crate's own
    // typical band (C0 ~ 1.13, V_gj ~ 0.1-0.3 m/s). Horizontal pipe, so the
    // buoyant rise contributes nothing axially; V_gj is set along +x so the
    // axial slip is present but not invented large.
    let slip = SlipModel::ZuberFindlay {
        c0,
        vgj: Vector3::new(vgj_x, 0.0, 0.0),
    };

    let centres_m: Vec<f64> = NODE_CENTRE_FT.iter().map(|&ft| ft * FT_TO_M).collect();
    let temps_k: Vec<f64> = NODE_T_DEGF.iter().map(|&f| degf_to_k(f)).collect();
    let t_mean = temps_k.iter().sum::<f64>() / temps_k.len() as f64;

    let mut solver = DriftFlux1d::new(
        pipe,
        slip,
        Pressure::new::<pascal>(P_INIT_PA),
        ThermodynamicTemperature::new::<kelvin>(t_mean),
        Time::new::<second>(dt_s),
    )
    .expect("the Edwards initial state is inside IF97");

    let profile: Vec<ThermodynamicTemperature> = (0..n_cells)
        .map(|i| {
            let xc = solver.pipe().cell_centre(i);
            ThermodynamicTemperature::new::<kelvin>(lerp(&centres_m, &temps_k, xc))
        })
        .collect();
    solver
        .set_temperature_profile(&profile)
        .expect("the Hendrie profile is inside IF97 at 7 MPa");
    solver
        .set_vapour_relaxation_time(Time::new::<second>(tau_s))
        .expect("tau is positive");

    solver.set_left_boundary(AxialBoundary::Closed);
    solver.set_right_boundary(AxialBoundary::ChokedOutlet {
        area_fraction: break_area_fraction(0.0),
        ambient_pressure: P_AMBIENT_PA,
    });

    let gs_cells: Vec<usize> = GS_POS_FT
        .iter()
        .map(|&(_, ft)| {
            solver
                .pipe()
                .nearest_cell(Length::new::<meter>(ft * FT_TO_M))
        })
        .collect();

    let n_steps = (t_end / dt_s).round() as usize;
    let sample_stride = (sample_every / dt_s).round().max(1.0) as usize;

    let mut hist = EdwardsHistory {
        times: Vec::new(),
        p_gs: Vec::new(),
        a_gs: Vec::new(),
        t_gs: Vec::new(),
        inventory: Vec::new(),
        slip_gs1: Vec::new(),
        max_slip: 0.0,
        mdot: Vec::new(),
        choked: Vec::new(),
        max_courant: 0.0,
        steps: 0,
        stopped: None,
    };

    let record = |solver: &DriftFlux1d, hist: &mut EdwardsHistory, t: f64, m: f64, ch: bool| {
        let mut pr = [0.0; 7];
        let mut ar = [0.0; 7];
        let mut tr = [0.0; 7];
        for (i, &c) in gs_cells.iter().enumerate() {
            pr[i] = solver.pressure()[c] * PA_TO_PSI;
            ar[i] = solver.void_fraction()[c];
            tr[i] = solver
                .temperature(c)
                .map(|t| t.get::<kelvin>())
                .unwrap_or(f64::NAN);
        }
        hist.times.push(t);
        hist.p_gs.push(pr);
        hist.a_gs.push(ar);
        hist.t_gs.push(tr);
        hist.inventory
            .push(solver.inventory().get::<uom::si::mass::kilogram>());
        // The reconstructed phase velocities: how much mechanical
        // non-equilibrium the drift-flux closure is actually producing.
        match solver.phase_velocities() {
            Ok((v_g, v_l)) => {
                hist.slip_gs1.push(v_g[gs_cells[0]] - v_l[gs_cells[0]]);
                let worst = v_g
                    .iter()
                    .zip(v_l.iter())
                    .map(|(g, l)| (g - l).abs())
                    .fold(0.0_f64, f64::max);
                hist.max_slip = hist.max_slip.max(worst);
            }
            Err(_) => hist.slip_gs1.push(f64::NAN),
        }
        hist.mdot.push(m);
        hist.choked.push(ch);
    };

    record(&solver, &mut hist, 0.0, 0.0, false);

    let mut time_s = 0.0_f64;
    for step in 1..=n_steps {
        // Rupture-disc ramp: the break area fraction is re-imposed each step.
        solver.set_right_boundary(AxialBoundary::ChokedOutlet {
            area_fraction: break_area_fraction(time_s),
            ambient_pressure: P_AMBIENT_PA,
        });

        match solver.step() {
            Ok(report) => {
                time_s = report.time;
                hist.steps = step;
                hist.max_courant = hist.max_courant.max(report.max_courant);
                if step % sample_stride == 0 {
                    record(
                        &solver,
                        &mut hist,
                        time_s,
                        report.outlet_mass_flow,
                        report.outlet_choked,
                    );
                }
            }
            Err(e) => {
                hist.stopped = Some(format!("step {step} at t = {time_s:.6} s: {e}"));
                break;
            }
        }
    }

    hist
}

/// Root-mean-square error \[psia\] of the simulated GS-1 pressure against the
/// digitised experimental curve, over the samples the run actually reached.
///
/// Returns `(rmse, worst_abs, worst_t, n_points)`.
fn gs1_error(times: &[f64], p_gs1: &[f64]) -> (f64, f64, f64, usize) {
    let t_max = *times.last().unwrap_or(&0.0);
    let mut sq = 0.0;
    let mut n = 0usize;
    let mut worst = 0.0_f64;
    let mut worst_t = f64::NAN;
    for &(td, pd) in GS1_DATA_PSIA.iter() {
        if td > t_max {
            continue;
        }
        let ps = lerp(times, p_gs1, td);
        let d = ps - pd;
        sq += d * d;
        n += 1;
        if d.abs() > worst {
            worst = d.abs();
            worst_t = td;
        }
    }
    if n == 0 {
        return (f64::NAN, f64::NAN, f64::NAN, 0);
    }
    ((sq / n as f64).sqrt(), worst, worst_t, n)
}

/// Print the GS-1 comparison table and the headline metrics for a run.
fn report(label: &str, hist: &EdwardsHistory) {
    let p_gs1: Vec<f64> = hist.p_gs.iter().map(|r| r[0]).collect();
    println!("\n===== Edwards-O'Brien drift flux: {label} =====");
    println!(
        "steps = {}, samples = {}, t_end = {:.4} s, max material Courant = {:.4}",
        hist.steps,
        hist.times.len(),
        hist.times.last().copied().unwrap_or(0.0),
        hist.max_courant
    );
    if let Some(ref s) = hist.stopped {
        println!("STOPPED EARLY: {s}");
    }
    println!("   t [s] |  GS-1 sim [psia] |  GS-1 data [psia] |  diff |  GS-1 void |  mdot [kg/s] | choked");
    for &(td, pd) in GS1_DATA_PSIA.iter() {
        if td > *hist.times.last().unwrap_or(&0.0) {
            continue;
        }
        let ps = lerp(&hist.times, &p_gs1, td);
        let idx = hist
            .times
            .iter()
            .enumerate()
            .min_by(|a, b| {
                (a.1 - td)
                    .abs()
                    .partial_cmp(&(b.1 - td).abs())
                    .expect("finite times")
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        println!(
            "  {td:6.3} | {ps:16.1} | {pd:17.1} | {:+6.1} | {:10.4} | {:12.3} | {}",
            ps - pd,
            hist.a_gs[idx][0],
            hist.mdot[idx],
            hist.choked[idx]
        );
    }
    let (rmse, worst, worst_t, n) = gs1_error(&hist.times, &p_gs1);
    println!("GS-1 RMSE vs Data: {rmse:.1} psia over {n} points; worst |diff| {worst:.1} psia at t = {worst_t:.3} s");

    // Axial ordering: the rarefaction must reach GS-1 before GS-7.
    println!("early axial pressure profile [psia] (rarefaction propagation):");
    println!("   t [s] |    GS-1 |    GS-2 |    GS-3 |    GS-4 |    GS-5 |    GS-6 |    GS-7");
    for k in [1usize, 2, 3, 5, 10] {
        if k < hist.times.len() {
            print!("  {:6.4} |", hist.times[k]);
            for v in hist.p_gs[k] {
                print!(" {v:7.1} |");
            }
            println!();
        }
    }

    let last = hist.p_gs.last().copied().unwrap_or([f64::NAN; 7]);
    print!("final gauge pressures [psia]:");
    for (i, (name, _)) in GS_POS_FT.iter().enumerate() {
        print!(" {name} {:.1}", last[i]);
    }
    println!();
    let last_a = hist.a_gs.last().copied().unwrap_or([f64::NAN; 7]);
    print!("final gauge void [-]:        ");
    for (i, (name, _)) in GS_POS_FT.iter().enumerate() {
        print!(" {name} {:.4}", last_a[i]);
    }
    println!();
    let last_t = hist.t_gs.last().copied().unwrap_or([f64::NAN; 7]);
    print!("final gauge T [K]:           ");
    for (i, (name, _)) in GS_POS_FT.iter().enumerate() {
        print!(" {name} {:.1}", last_t[i]);
    }
    println!();
    println!(
        "inventory: {:.4} kg initial -> {:.4} kg final ({:.1} % drained)",
        hist.inventory.first().copied().unwrap_or(f64::NAN),
        hist.inventory.last().copied().unwrap_or(f64::NAN),
        100.0
            * (1.0
                - hist.inventory.last().copied().unwrap_or(f64::NAN)
                    / hist.inventory.first().copied().unwrap_or(f64::NAN))
    );
    println!(
        "plateau (mean GS-1, 0.02-0.06 s): {:.1} psia",
        plateau(&hist.times, &p_gs1)
    );
    let (peak_mdot, peak_mdot_t) =
        hist.mdot
            .iter()
            .zip(hist.times.iter())
            .fold(
                (0.0_f64, f64::NAN),
                |acc, (&m, &t)| {
                    if m > acc.0 {
                        (m, t)
                    } else {
                        acc
                    }
                },
            );
    println!(
        "break flow: peak {peak_mdot:.2} kg/s ({:.1} lbm/s) at t = {peak_mdot_t:.3} s",
        peak_mdot * 2.204_622_6
    );
    let peak_slip_gs1 = hist
        .slip_gs1
        .iter()
        .copied()
        .fold(0.0_f64, |a, b| a.max(b.abs()));
    println!(
        "mechanical non-equilibrium: peak |v_g - v_l| = {:.3} m/s anywhere, {peak_slip_gs1:.3} m/s at GS-1",
        hist.max_slip
    );
    if let Some((min_t, reb)) = tail_diagnostics(hist, 0.42) {
        println!(
            "tail (t > 0.42 s): min gauge T = {min_t:.1} K ({:.1} degC), GS-1 rebound = {reb:+.1} psia",
            min_t - 273.15
        );
    }
}

/// Minimum gauge temperature \[K\] and the GS-1 pressure rebound \[psia\] over
/// the tail `t > t0` — the artificial-cooling diagnostic the HEM case reports.
///
/// "Rebound" is `max(p_GS-1 over the tail) − p_GS-1(t0)`: a large positive
/// value is the non-physical re-pressurisation an explicit scheme shows once
/// the pipe empties. Returns `None` if the run never reached `t0`.
fn tail_diagnostics(hist: &EdwardsHistory, t0: f64) -> Option<(f64, f64)> {
    let idx = hist.times.iter().position(|&t| t >= t0)?;
    let min_t = hist.t_gs[idx..]
        .iter()
        .flat_map(|r| r.iter().copied())
        .fold(f64::INFINITY, f64::min);
    let p_at = hist.p_gs[idx][0];
    let tail_max = hist.p_gs[idx..]
        .iter()
        .map(|r| r[0])
        .fold(f64::NEG_INFINITY, f64::max);
    Some((min_t, tail_max - p_at))
}

/// Mean GS-1 pressure \[psia\] over the flashing-plateau window 0.02–0.06 s.
fn plateau(times: &[f64], p_gs1: &[f64]) -> f64 {
    let vals: Vec<f64> = times
        .iter()
        .zip(p_gs1.iter())
        .filter(|(&t, _)| (0.02..=0.06).contains(&t))
        .map(|(_, &p)| p)
        .collect();
    if vals.is_empty() {
        f64::NAN
    } else {
        vals.iter().sum::<f64>() / vals.len() as f64
    }
}

/// Scratch exploration runner for the Edwards case — **not a V&V gate**.
///
/// Reads `EDF_CELLS`, `EDF_DT_US`, `EDF_TEND`, `EDF_C0`, `EDF_VGJ`, `EDF_TAU`
/// from the environment (defaults: 24 cells, 100 µs, 0.02 s, `C₀ = 1.13`,
/// `V_gj = 0.1 m/s`, the solver's default `τ`) and prints the same report the
/// gated test does. It asserts nothing, so it can be pointed at a
/// configuration that fails: [`run_edwards`] records a refused step in
/// [`EdwardsHistory::stopped`] and prints how far the march got instead of
/// panicking. Kept `#[ignore]`d so it never runs in an ordinary suite.
#[test]
#[ignore = "exploration harness, asserts nothing; run explicitly with EDF_* set"]
fn edwards_drift_flux_explore() {
    let cells: usize = std::env::var("EDF_CELLS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(24);
    let dt_us: f64 = std::env::var("EDF_DT_US")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100.0);
    let t_end: f64 = std::env::var("EDF_TEND")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.02);
    let c0: f64 = std::env::var("EDF_C0")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.13);
    let vgj_x: f64 = std::env::var("EDF_VGJ")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.1);
    let tau_s: f64 = std::env::var("EDF_TAU")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(super::drift_flux::DEFAULT_VAPOUR_RELAXATION_TIME);
    let hist = run_edwards(EdwardsCase {
        n_cells: cells,
        dt_s: dt_us * 1.0e-6,
        t_end,
        c0,
        vgj_x,
        tau_s,
        ..EdwardsCase::default()
    });
    report(
        &format!("{cells} cells, dt = {dt_us} us, t_end = {t_end} s, C0 = {c0}, Vgj = {vgj_x} m/s, tau = {tau_s} s"),
        &hist,
    );
    let p_gs1: Vec<f64> = hist.p_gs.iter().map(|r| r[0]).collect();
    println!("plateau = {:.1} psia", plateau(&hist.times, &p_gs1));
}

/// **Methodology.** The full Edwards–O'Brien blowdown on [`DriftFlux1d`] —
/// B-T-3271 geometry, 24 cells, 7.00 MPa with the Hendrie non-isothermal
/// profile, HEM-choked break through `0.87 A` opened over a 1 ms ramp,
/// `dt = 30 µs`, integrated over the full 600 ms — compared against the
/// digitised Edwards experimental GS-1 pressure curve ([`GS1_DATA_PSIA`]). The
/// module header carries the complete methodology, provenance and the
/// multi-fidelity comparison against the HEM baseline; this comment records
/// what the assertions below actually gate.
///
/// Pass criterion (all gated): 20 000 steps complete with no refused step;
/// GS-1 RMSE `<= 32.0` psia and worst point deviation `<= 60` psia over all 16
/// data points; flashing plateau within 330–380 psia; rarefaction at GS-1
/// ahead of GS-7 at `t = 2 ms`; `> 99 %` of the inventory drained and every
/// gauge ending at 13–16 psia; minimum tail temperature `> 350 K` past 0.42 s
/// with `<= 1` psia rebound; void in `[0, 1]` and material Courant `< 1`
/// everywhere; and peak `|v_g − v_l| > 1 m/s` so an inert slip closure cannot
/// pass.
///
/// The tolerances are **written around measured behaviour with headroom, not
/// chosen to make a model look good** — nothing in the solver or the case was
/// tuned to hit them (see the sensitivity table in the module header).
///
/// **Results (measured 2026-08-11, release, this machine).** Passes.
/// 20 000 steps, **39.00 s wall clock** (three runs: 39.00 s, 39.79 s,
/// 36.15 s, all with identical numbers). GS-1 **RMSE 29.0 psia** over 16 points
/// (0–0.30 s), worst **+52.0 psia at t = 0.100 s**; flashing plateau
/// **354.3 psia** (experimental ~350–367 psia); break flow peak **45.79 kg/s
/// (101.0 lbm/s)** at `t = 0.002 s`; inventory **14.2319 → 0.0465 kg
/// (99.7 % drained)**, all gauges ending at 14.4–14.6 psia; minimum tail
/// temperature **372.6 K** with **+0.0 psia** rebound; max material Courant
/// **0.0254**; peak `|v_g − v_l|` **88.4 m/s**. The per-time-point table is in
/// the module header.
#[test]
fn edwards_drift_flux_gs1_pressure_history() {
    let case = EdwardsCase::default();
    let hist = run_edwards(case);
    report("baseline, full 600 ms", &hist);

    // ── The run must complete, not merely start ─────────────────────────────
    assert!(
        hist.stopped.is_none(),
        "the march stopped early: {:?}",
        hist.stopped
    );
    let expected_steps = (case.t_end / case.dt_s).round() as usize;
    assert_eq!(
        hist.steps, expected_steps,
        "expected {expected_steps} steps of the full transient"
    );
    assert!(
        hist.max_courant < 1.0,
        "material Courant number {} exceeded 1 -- donor-cell transport stepped past a whole cell",
        hist.max_courant
    );

    // ── Physical bounds at every sampled gauge ──────────────────────────────
    for (k, row) in hist.a_gs.iter().enumerate() {
        for (g, &a) in row.iter().enumerate() {
            assert!(
                a.is_finite() && (0.0..=1.0).contains(&a),
                "void fraction {a} out of [0,1] at sample {k}, gauge {g}"
            );
        }
    }
    for (k, row) in hist.p_gs.iter().enumerate() {
        for (g, &p) in row.iter().enumerate() {
            assert!(
                p.is_finite() && p > 0.0,
                "gauge pressure {p} psia non-physical at sample {k}, gauge {g}"
            );
        }
    }

    // ── The comparison against the digitised experimental curve ─────────────
    let p_gs1: Vec<f64> = hist.p_gs.iter().map(|r| r[0]).collect();
    let (rmse, worst, worst_t, n) = gs1_error(&hist.times, &p_gs1);
    assert_eq!(n, GS1_DATA_PSIA.len(), "all 16 data points must be covered");
    assert!(
        rmse <= 32.0,
        "GS-1 RMSE vs the digitised Edwards data is {rmse:.1} psia, above the pinned 32.0 psia \
         (measured 28.9 psia on 2026-08-11)"
    );
    assert!(
        worst <= 60.0,
        "worst GS-1 deviation is {worst:.1} psia at t = {worst_t:.3} s, above the pinned 60.0 psia \
         (measured 52.0 psia at t = 0.100 s on 2026-08-11)"
    );

    // ── The flashing plateau: the physics the case exists to test ───────────
    let plat = plateau(&hist.times, &p_gs1);
    assert!(
        (330.0..=380.0).contains(&plat),
        "GS-1 flashing plateau is {plat:.1} psia, outside the pinned 330-380 psia band \
         (measured 354.3 psia; experimental plateau ~350-367 psia)"
    );

    // ── Rarefaction propagation: GS-1 must lead GS-7 ────────────────────────
    let k2 = hist
        .times
        .iter()
        .position(|&t| t >= 0.002)
        .expect("the run reaches 2 ms");
    assert!(
        hist.p_gs[k2][0] < 0.5 * hist.p_gs[k2][6],
        "at t = {:.4} s the wave has not propagated as measured: GS-1 {:.1} psia, GS-7 {:.1} psia",
        hist.times[k2],
        hist.p_gs[k2][0],
        hist.p_gs[k2][6]
    );

    // ── The pipe must actually empty to ambient ─────────────────────────────
    let drained = 1.0 - hist.inventory.last().unwrap() / hist.inventory.first().unwrap();
    assert!(
        drained > 0.99,
        "only {:.1} % of the inventory drained over 600 ms",
        100.0 * drained
    );
    for (g, &p) in hist.p_gs.last().unwrap().iter().enumerate() {
        assert!(
            (13.0..=16.0).contains(&p),
            "gauge {g} ends at {p:.1} psia rather than near the 14.5 psia ambient"
        );
    }

    // ── No artificial-cooling / rebound tail (the HEM case's diagnostic) ────
    let (min_tail_t, rebound) = tail_diagnostics(&hist, 0.42).expect("the run reaches 0.42 s");
    assert!(
        min_tail_t > 350.0,
        "cold tail: minimum gauge temperature past 0.42 s is {min_tail_t:.1} K \
         (measured 372.6 K on 2026-08-11; the explicit-scheme artefact drives it toward 274 K)"
    );
    assert!(
        rebound <= 1.0,
        "non-physical GS-1 pressure rebound of {rebound:+.1} psia in the tail \
         (measured +0.0 psia on 2026-08-11)"
    );

    // ── Mechanical non-equilibrium is present, i.e. the closure is not inert ─
    assert!(
        hist.max_slip > 1.0,
        "peak |v_g - v_l| is only {:.3} m/s -- the slip closure is doing nothing, so this is \
         not a drift-flux result (measured 57.56 m/s on 2026-08-11)",
        hist.max_slip
    );
}

/// **Methodology.** The headline RMSE of
/// [`edwards_drift_flux_gs1_pressure_history`] means little on its own if it
/// swings with the model parameters — so re-run the same case over the
/// 0–0.30 s comparison window while varying the only knobs the drift-flux model
/// exposes, plus one mesh refinement as a numerical control:
///
/// 1. baseline `C₀ = 1.13`, `V_gj = 0.1 m/s`, `τ = 1 ms`, 24 cells;
/// 2. **no slip** `C₀ = 1.0`, `V_gj = 0` — the HEM-with-void-transport limit;
/// 3. strong slip `C₀ = 1.2`, `V_gj = 0.3 m/s` (top of the reference band);
/// 4. `τ = 10 ms` and 5. `τ = 0.1 ms` — ten times slower / faster flashing;
/// 6. 48 cells at the baseline parameters.
///
/// Each variant reports GS-1 RMSE against the digitised data, the worst point
/// deviation, the plateau, the peak reconstructed slip velocity, and the
/// largest deviation from the baseline's own pressure and void series sampled
/// on the data timestamps. Assertion: the reported metrics stay finite. The
/// **numbers are the deliverable**, not a pass/fail — the point is to bound how
/// much of the headline result is model parameter and how much is physics.
///
/// `#[ignore]`d because it is six 10 000-step runs; **measured wall clock
/// 132.20 s** (2026-08-11, release).
///
/// **Results (measured 2026-08-11, release, 15 data points inside the window).**
///
/// ```text
/// variant                              RMSE  worst  plateau  peak|vg-vl|  max|dp| vs base  max|dalpha|
///                                     [psia] [psia]  [psia]      [m/s]         [psia]          [-]
/// baseline C0=1.13 Vgj=0.1 tau=1e-3     28.9   52.0    354.3       57.56           --            --
/// no slip  C0=1.00 Vgj=0.0              28.9   51.8    354.3        0.00          1.44        0.0159
/// strong slip C0=1.20 Vgj=0.3           28.9   51.9    354.7       82.33          1.88        0.0110
/// tau = 1e-2 s                          28.7   51.8    354.6       25.54          1.63        0.1613
/// tau = 1e-4 s                          27.5   51.3    354.2       73.28          5.32        0.0257
/// 48 cells                              27.2   53.2    353.6       58.93          6.89        0.0713
/// ```
///
/// Interpretation: turning the slip closure **off entirely** moves GS-1 by at
/// most **1.44 psia** and leaves the RMSE and the plateau unchanged to the
/// tenth of a psi, even though the peak slip velocity drops from 57.6 m/s to
/// exactly zero. Mesh refinement moves it more (6.89 psia) than any model
/// parameter does. So the Edwards GS-1 pressure history is **not a
/// discriminator of mechanical non-equilibrium**, and the agreement with the
/// data is not a tuned one. `τ` reaches the *void* field far more than the
/// pressure (`max |Δα| = 0.16` at `τ = 10 ms`).
#[test]
#[ignore = "long-running sensitivity sweep (measured 132.20 s on 2026-08-11); run explicitly"]
fn edwards_drift_flux_slip_and_tau_sensitivity() {
    let base = EdwardsCase {
        t_end: 0.300,
        ..EdwardsCase::default()
    };
    let variants: [(&str, EdwardsCase); 6] = [
        ("baseline C0=1.13 Vgj=0.1 tau=1e-3, 24 cells", base),
        (
            "no slip   C0=1.00 Vgj=0.0 (HEM-with-void limit)",
            EdwardsCase {
                c0: 1.0,
                vgj_x: 0.0,
                ..base
            },
        ),
        (
            "strong slip C0=1.20 Vgj=0.3",
            EdwardsCase {
                c0: 1.2,
                vgj_x: 0.3,
                ..base
            },
        ),
        (
            "tau = 1e-2 s (10x slower flashing)",
            EdwardsCase {
                tau_s: 1.0e-2,
                ..base
            },
        ),
        (
            "tau = 1e-4 s (10x faster flashing)",
            EdwardsCase {
                tau_s: 1.0e-4,
                ..base
            },
        ),
        (
            "mesh refinement: 48 cells",
            EdwardsCase {
                n_cells: 48,
                ..base
            },
        ),
    ];

    // The baseline series every variant is differenced against, sampled on the
    // digitised-data timestamps so meshes with different sampling compare.
    let mut baseline_p: Vec<f64> = Vec::new();
    let mut baseline_a: Vec<f64> = Vec::new();
    let mut metrics: Vec<(String, f64, f64, f64, f64, f64, f64)> = Vec::new();

    for (label, case) in variants {
        let hist = run_edwards(case);
        let p_gs1: Vec<f64> = hist.p_gs.iter().map(|r| r[0]).collect();
        let a_gs1: Vec<f64> = hist.a_gs.iter().map(|r| r[0]).collect();
        let (rmse, worst, worst_t, n) = gs1_error(&hist.times, &p_gs1);
        let plat = plateau(&hist.times, &p_gs1);
        let peak_slip = hist.max_slip;

        let on_data_times: Vec<f64> = GS1_DATA_PSIA
            .iter()
            .map(|&(td, _)| lerp(&hist.times, &p_gs1, td))
            .collect();
        let on_data_void: Vec<f64> = GS1_DATA_PSIA
            .iter()
            .map(|&(td, _)| lerp(&hist.times, &a_gs1, td))
            .collect();
        if baseline_p.is_empty() {
            baseline_p = on_data_times.clone();
            baseline_a = on_data_void.clone();
        }
        let dp_max = on_data_times
            .iter()
            .zip(baseline_p.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f64, f64::max);
        let da_max = on_data_void
            .iter()
            .zip(baseline_a.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f64, f64::max);

        println!(
            "{label:48} -> RMSE {rmse:6.1} psia ({n} pts), worst {worst:6.1} @ {worst_t:.3} s, \
             plateau {plat:6.1} psia, peak |v_g-v_l| {peak_slip:6.2} m/s, \
             max |dp| vs baseline {dp_max:6.2} psia, max |dalpha| {da_max:.4}, \
             steps {}, stopped {:?}",
            hist.steps, hist.stopped
        );
        metrics.push((
            label.to_string(),
            rmse,
            worst,
            plat,
            peak_slip,
            dp_max,
            da_max,
        ));
    }

    for (label, rmse, _, plat, _, _, _) in &metrics {
        assert!(
            rmse.is_finite() && plat.is_finite(),
            "{label}: reported metrics went non-finite"
        );
    }
}

/// **Methodology.** [`TwoFluid1d`] is a documented no-physics scaffold
/// (bead `op-dt3.13`): its `step()` must refuse rather than return a plausible
/// number. Pin that contract so a partial implementation that starts marching
/// without its interfacial closures is caught by a failing test rather than
/// discovered in a plot.
///
/// This test is also the **placeholder for the future Edwards–O'Brien
/// two-fluid (six-equation) case**. When the solver exists, the case to write
/// here is the one [`edwards_drift_flux_gs1_pressure_history`] runs — same
/// B-T-3271 geometry, same Hendrie profile, same choked break, same digitised
/// GS-1 reference — via a [`run_edwards`]-shaped harness, with the extra
/// deliverable that a six-equation model can report a non-zero
/// [`TwoFluidReport::max_thermal_nonequilibrium`](super::two_fluid::TwoFluidReport::max_thermal_nonequilibrium),
/// which neither HEM nor drift flux can represent. Note what this crate's
/// drift-flux result already says about that case: the slip sweep in
/// [`edwards_drift_flux_slip_and_tau_sensitivity`] shows *mechanical*
/// non-equilibrium changes GS-1 pressure by `<= 1.9` psia here, so a
/// six-equation Edwards case should be judged on whether *thermal*
/// non-equilibrium changes anything, not on the pressure RMSE alone.
///
/// Pass criterion: `step()` returns `TampinesError::NotYetImplemented` naming
/// the component, and `time()` stays at zero.
///
/// **Results (measured 2026-08-11, release).** Passes, in `0.00 s`:
/// `Err(NotYetImplemented { component: "multiphase_1d::two_fluid::TwoFluid1d::step" })`
/// and `time() = 0 s`. The six-equation solver is still a scaffold — do not
/// read any two-fluid Edwards result into this crate.
#[test]
fn two_fluid_step_still_refuses_to_pretend() {
    let pipe = Pipe1d::circular(
        Length::new::<meter>(PIPE_LENGTH_M),
        Length::new::<meter>(PIPE_ID_M),
        Angle::new::<radian>(0.0),
        24,
    )
    .expect("the Edwards pipe is well posed");
    let mut solver = TwoFluid1d::new(pipe);
    let outcome = solver.step();
    println!("TwoFluid1d::step() -> {outcome:?}");
    assert!(
        matches!(
            outcome,
            Err(crate::TampinesError::NotYetImplemented { component })
                if component == "multiphase_1d::two_fluid::TwoFluid1d::step"
        ),
        "the six-equation scaffold must refuse, got {outcome:?}"
    );
    assert_eq!(
        solver.time().get::<second>(),
        0.0,
        "nothing advanced, so the clock must not have moved"
    );
}
