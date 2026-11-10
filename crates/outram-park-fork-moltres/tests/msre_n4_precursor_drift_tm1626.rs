//! **MSRE N-4 (gh:#1130): reactivity loss from fuel circulation, moltres's
//! precursor drift driven by TM-1626's published flux and importance.**
//! **Depends on unreviewed moltres** (maintainer, 2026-11-03).
//!
//! # Methodology
//!
//! **Reference.** B. E. Prince, *Period Measurements on the Molten Salt
//! Reactor Experiment During Fuel Circulation: Theory and Experiment*,
//! ORNL-TM-1626 (October 1966), the maintainer's chosen N-4 source
//! (gh:#1130, 2026-11-03). Measured loss **0.212 ± 0.004 % δk/k** (U-235,
//! zero power, report p. 26, PDF p. 32); Prince's own line model gives
//! **0.222 %** (Eq. 14, w = 0, same page).
//!
//! **What is tested.** Moltres's diffusion solver needs MSRE axial diffusion
//! constants. None was found published in TM-0728, ORNL-4233, TM-1070 or
//! TM-1626, so this test uses the part of moltres that carries N-4's
//! physics, [`PrecursorDrift`] (upwind FV advection and decay on a ring).
//! It is driven by **TM-1626's own** fission density and importance, which
//! amounts to Prince's line model rebuilt on moltres.
//!
//! **Inputs, all TM-1626:**
//! - **Fig. 4** (report p. 21, PDF p. 27): fission density in salt
//!   `P°φs(z)` and fast adjoint `φ1†(z)`. Digitised by eye on 2026-11-09
//!   from a 250 dpi render (`pdftoppm -r 250`) at the points in
//!   [`FIG4`]; reading error about ±0.02 in amplitude. Linear in between.
//! - **Region extents** read from the bands over Figs. 4 and 5: bottom
//!   plenum −22…0 cm, channeled (graphite) region 0…168 cm, top plenum
//!   168…190 cm.
//! - **Table 1** (report p. 24, PDF p. 30), read from the scan: `β_i`,
//!   `λ_i` and the residence times 3.8 s (bottom plenum), 9.4 s (core),
//!   3.9 s (top plenum), 8.1 s (external loop).
//!
//! **Model.**
//! - **Coordinate.** Transit time `t` along the flow path, plug flow in
//!   every region (Prince's assumption outside the lower plenum). The
//!   salt volume per path element is `Q dt`, so the regions' fuel volume
//!   fractions (0.225, 1.0, 0.91) drop out exactly, and every integral is
//!   a plain sum over equal-time cells.
//! - **Ring.** 25.2 m at unit speed (1 m ≡ 1 s), `D_C = 0`,
//!   `S_f(t) = P°φs(z(t))`, zero in the external loop.
//! - **Loss** `= β − Σ_i λ_i ⟨φ†, C_i⟩ / ⟨φ†, S_f⟩`. Prompt and delayed
//!   neutrons are both weighted by the fast adjoint (TM-1626 p. 20); the
//!   static case gives exactly `β`.
//! - **Reference in this file.** [`exact_loss`] integrates
//!   `dc/dt = β_i S_f(t) − λ_i c` exactly around the loop on a fine
//!   grid (exponential integrator, piecewise-linear source, periodic
//!   closure). It also gives Prince's **well-stirred lower plenum**, which
//!   moltres's ring cannot represent.
//!
//! **Pass criteria (verification of moltres only):** the moltres loss
//! converges to the exact one at first order, and lies within `1e-5`
//! (0.001 % δk/k) of it at N = 10080. **The comparison with 0.212 % and
//! 0.222 % is reported, not gated.**
//!
//! # Results (2026-11-09, release build)
//!
//! | quantity | value (% δk/k) |
//! |---|---|
//! | exact line model, lower plenum plug flow | 0.2293 |
//! | exact, lower plenum well-stirred (Prince's treatment) | 0.2299 |
//! | moltres, N = 2520 / 10080 | 0.2293 / 0.2293 (errors −1.75e-7 / −4.40e-8, ratio 3.97) |
//! | Prince, TM-1626 Eq. 14 | 0.222 |
//! | **measured** | **0.212 ± 0.004** |
//!
//! - **Moltres is verified** for this problem: first-order convergence to
//!   the exact solution, 4e-8 δk/k at N = 10080.
//! - **Digitisation uncertainty about ±0.005 %:** core height 166/170 cm
//!   gives 0.2267/0.2322; plenum importance ±0.02 gives 0.2258/0.2328
//!   (`verification_and_validation/msre_n4_precursor_drift/scripts/sensitivity.py`).
//! - **Against Prince's 0.222 %:** +0.007 ± 0.005, consistent. The
//!   circulating emission shape matches his Fig. 5 at z = 0, 120, 168 and
//!   190 cm (0.111, 0.625, 0.512, 0.246 of the stationary peak against
//!   about 0.10, 0.63, 0.51, 0.26 read from the figure).
//! - **Against the measurement:** +0.017 ± 0.006 % (+8 %), inside the
//!   prediction posted on gh:#1130 (0.214–0.230 %).
//! - **Plenum ablation:** with zero importance in both plenums the loss is
//!   0.265 %; the plenums' delayed neutrons are worth 0.036 % δk/k.

use outram_park_fork_moltres::prelude::{DelayedFamily, PrecursorDrift, RingMesh};
use outram_foam_basic_lib::prelude::{Field, SolverSettings, VolScalarField};

/// TM-1626 Table 1: `(β_i, λ_i [1/s])`, β_i read as 10⁴β_i = 2.23, 14.57,
/// 13.07, 26.28, 7.66, 2.80.
const TABLE1: [(f64, f64); 6] = [
    (2.23e-4, 0.0124),
    (14.57e-4, 0.0305),
    (13.07e-4, 0.1114),
    (26.28e-4, 0.3013),
    (7.66e-4, 1.140),
    (2.80e-4, 3.010),
];

/// TM-1626 Table 1 residence times (s): bottom plenum, core, top plenum,
/// external loop.
const TAU: [f64; 4] = [3.8, 9.4, 3.9, 8.1];

/// Region extents along z (cm), read from Figs. 4 and 5.
const Z_BOTTOM: f64 = -22.0;
const Z_CORE_TOP: f64 = 168.0;
const Z_TOP: f64 = 190.0;

/// TM-1626 Fig. 4, digitised: `(z cm, P°φs, φ1†)`.
const FIG4: [(f64, f64, f64); 26] = [
    (-22.0, 0.015, 0.045),
    (-20.0, 0.015, 0.055),
    (-10.0, 0.035, 0.130),
    (-5.0, 0.070, 0.190),
    (0.0, 0.120, 0.260),
    (10.0, 0.330, 0.370),
    (20.0, 0.500, 0.500),
    (30.0, 0.620, 0.620),
    (40.0, 0.730, 0.730),
    (50.0, 0.830, 0.830),
    (60.0, 0.900, 0.900),
    (70.0, 0.950, 0.950),
    (80.0, 0.985, 0.985),
    (90.0, 0.990, 0.990),
    (100.0, 0.970, 0.970),
    (110.0, 0.925, 0.925),
    (120.0, 0.860, 0.860),
    (130.0, 0.780, 0.780),
    (140.0, 0.680, 0.690),
    (150.0, 0.560, 0.585),
    (160.0, 0.390, 0.440),
    (165.0, 0.250, 0.380),
    (170.0, 0.110, 0.340),
    (180.0, 0.050, 0.170),
    (185.0, 0.025, 0.120),
    (190.0, 0.008, 0.070),
];

fn families() -> Vec<DelayedFamily> {
    TABLE1.iter().map(|&(beta, lambda)| DelayedFamily { beta, lambda }).collect()
}

fn loop_time() -> f64 {
    TAU.iter().sum()
}

/// An axial shape for the line model: `(z cm, fission density in salt,
/// importance)` points, linear in between, and the region extents (cm):
/// the lower plenum's bottom, the top of the graphite, the upper plenum's
/// top. The graphite starts at z = 0.
#[derive(Clone, Debug)]
struct Shape {
    table: Vec<(f64, f64, f64)>,
    z_bottom: f64,
    z_core_top: f64,
    z_top: f64,
}

impl Shape {
    /// TM-1626 Fig. 4 and its region extents.
    fn prince() -> Shape {
        Shape { table: FIG4.to_vec(), z_bottom: Z_BOTTOM, z_core_top: Z_CORE_TOP, z_top: Z_TOP }
    }

    /// Linear interpolation in the table: `(S_f, φ†)` at `z` (cm).
    fn at(&self, z: f64) -> (f64, f64) {
        let t = &self.table;
        if z <= t[0].0 {
            return (t[0].1, t[0].2);
        }
        for w in t.windows(2) {
            let (z0, p0, a0) = w[0];
            let (z1, p1, a1) = w[1];
            if z <= z1 {
                let f = (z - z0) / (z1 - z0);
                return (p0 + f * (p1 - p0), a0 + f * (a1 - a0));
            }
        }
        let last = t[t.len() - 1];
        (last.1, last.2)
    }

    /// Position `z` (cm) at transit time `t` (s, from the bottom-plenum
    /// inlet), or `None` in the external loop. Plug flow in each region.
    fn z_at(&self, t: f64) -> Option<f64> {
        let t = t.rem_euclid(loop_time());
        let (t1, t2, t3) = (TAU[0], TAU[0] + TAU[1], TAU[0] + TAU[1] + TAU[2]);
        if t < t1 {
            Some(self.z_bottom + (0.0 - self.z_bottom) * t / TAU[0])
        } else if t < t2 {
            Some(self.z_core_top * (t - t1) / TAU[1])
        } else if t < t3 {
            Some(self.z_core_top + (self.z_top - self.z_core_top) * (t - t2) / TAU[2])
        } else {
            None
        }
    }

    /// `(S_f, φ†)` at transit time `t`; zero in the external loop.
    fn source_and_importance(&self, t: f64) -> (f64, f64) {
        self.z_at(t).map_or((0.0, 0.0), |z| self.at(z))
    }
}

/// How the lower plenum is mixed in [`exact_loss`].
#[derive(Clone, Copy, Debug, PartialEq)]
enum LowerPlenum {
    /// Plug flow, as on moltres's ring.
    PlugFlow,
    /// Prince's well-stirred tank: uniform precursors, emission weighted by
    /// the plenum's mean importance.
    WellStirred,
}

/// Exact loss (δk/k) of the line model, integrated on `m` equal time steps.
fn exact_loss(sh: &Shape, m: usize, lp: LowerPlenum) -> f64 {
    let t_loop = loop_time();
    let dt = t_loop / m as f64;
    let n_lp = (TAU[0] / dt).round() as usize;
    let samples: Vec<(f64, f64)> = (0..=m).map(|j| sh.source_and_importance(j as f64 * dt)).collect();
    // Denominator: <φ†, S_f> by the trapezoid rule over the loop.
    let denom: f64 = (0..m).map(|j| 0.5 * (samples[j].0 * samples[j].1 + samples[j + 1].0 * samples[j + 1].1) * dt).sum();
    let p_lp_mean: f64 = (0..n_lp).map(|j| 0.5 * (samples[j].0 + samples[j + 1].0)).sum::<f64>() / n_lp as f64;
    let a_lp_mean: f64 = (0..n_lp).map(|j| 0.5 * (samples[j].1 + samples[j + 1].1)).sum::<f64>() / n_lp as f64;
    let mut delayed = 0.0;
    for &(beta, lambda) in &TABLE1 {
        let e = (-lambda * dt).exp();
        // Exact step for dc/dt = beta S(t) - lambda c with S linear on the step:
        // c1 = e c0 + beta [S0 g0 + S1 g1].
        // g1 = int_0^h exp(-lambda (h - s)) s/h ds.
        let g1 = 1.0 / lambda - (1.0 - e) / (lambda * lambda * dt);
        let g0 = (1.0 - e) / lambda - g1;
        let step = |c0: f64, s0: f64, s1: f64| e * c0 + beta * (s0 * g0 + s1 * g1);
        let tank = |c_in: f64| (c_in / TAU[0] + beta * p_lp_mean) / (lambda + 1.0 / TAU[0]);
        // One loop from the external-loop exit (t = 0) as an affine map
        // c(T) = a c(0) + b; the periodic solution is b / (1 - a).
        let run = |c0: f64, record: bool| -> (f64, f64) {
            let mut c = c0;
            let mut acc = 0.0;
            let start = if lp == LowerPlenum::WellStirred {
                let ct = tank(c);
                if record {
                    acc += lambda * ct * a_lp_mean * TAU[0];
                }
                c = ct;
                n_lp
            } else {
                0
            };
            for j in start..m {
                let c1 = step(c, samples[j].0, samples[j + 1].0);
                if record {
                    acc += 0.5 * lambda * (c * samples[j].1 + c1 * samples[j + 1].1) * dt;
                }
                c = c1;
            }
            (c, acc)
        };
        let b = run(0.0, false).0;
        let a = run(1.0, false).0 - b;
        let c_periodic = b / (1.0 - a);
        delayed += run(c_periodic, true).1;
    }
    let beta: f64 = TABLE1.iter().map(|x| x.0).sum();
    beta - delayed / denom
}

/// Moltres's loss on an `n`-cell ring at loop speed `speed` (1.0 = the
/// TM-1626 residence times; 0.0 = fuel stationary).
fn moltres_loss(sh: &Shape, n: usize, speed: f64) -> (f64, Vec<f64>) {
    let t_loop = loop_time();
    let ring = RingMesh::new(t_loop, 1.0, n).expect("ring");
    let fams = families();
    let drift = PrecursorDrift::new(
        ring.mesh.clone(),
        fams.clone(),
        ring.uniform_flux(speed),
        0.0,
        SolverSettings {
            tolerance: 1e-11,
            max_iter: 200_000,
        },
    )
    .expect("drift");
    let (s, imp): (Vec<f64>, Vec<f64>) = (0..n).map(|i| sh.source_and_importance(ring.arc_centre(i))).unzip();
    let s_f = VolScalarField::new("Sf", ring.mesh.clone(), Field::new(s.clone()), vec![]);
    let c = drift.solve_steady(&s_f, 1.0).expect("solve");
    let emission = drift.delayed_source(&c);
    let em = emission.internal.as_slice();
    let denom: f64 = s.iter().zip(&imp).map(|(a, b)| a * b).sum();
    let num: f64 = em.iter().zip(&imp).map(|(a, b)| a * b).sum();
    let beta: f64 = fams.iter().map(|f| f.beta).sum();
    (beta - num / denom, em.to_vec())
}

/// **Verification of moltres's precursor drift, and the N-4 comparison.**
///
/// **Result (2026-11-09, release build):** see the module docs' table
/// (moltres 0.2293 % against exact 0.2293 %, error 4.4e-8 at N = 10080);
/// record in `verification_and_validation/msre_n4_precursor_drift/` and on
/// gh:#1130.
#[test]
fn n4_moltres_drift_against_exact_and_tm1626() {
    let sh = Shape::prince();
    // Static fuel: no loss, exactly.
    let (static_loss, _) = moltres_loss(&sh, 2520, 0.0);
    assert!(static_loss.abs() < 1e-12, "static loss {static_loss:e}");

    let exact = exact_loss(&sh, 252_000, LowerPlenum::PlugFlow);
    let exact_stirred = exact_loss(&sh, 252_000, LowerPlenum::WellStirred);
    // The exact integrator converges: a 4x coarser grid agrees closely.
    let exact_coarse = exact_loss(&sh, 63_000, LowerPlenum::PlugFlow);
    assert!((exact - exact_coarse).abs() < 1e-7, "{exact} {exact_coarse}");

    let (m1, _) = moltres_loss(&sh, 2520, 1.0);
    let (m2, em) = moltres_loss(&sh, 10080, 1.0);
    let (e1, e2) = (m1 - exact, m2 - exact);
    println!("N-4 (TM-1626), loss in % dk/k");
    println!("  exact, lower plenum plug flow      {:.4}", exact * 100.0);
    println!("  exact, lower plenum well-stirred   {:.4}", exact_stirred * 100.0);
    println!("  moltres N = 2520                   {:.4} (error {:+.2e})", m1 * 100.0, e1);
    println!("  moltres N = 10080                  {:.4} (error {:+.2e}), ratio {:.2}", m2 * 100.0, e2, e1 / e2);
    println!("  TM-1626: Prince Eq. 14 0.222, measured 0.212 +/- 0.004");

    // Fig. 5 shape check (reported): circulating emission in salt relative
    // to the stationary peak, at z = 0, 120, 168 and 190 cm.
    let n = em.len();
    let dt = loop_time() / n as f64;
    let beta: f64 = TABLE1.iter().map(|x| x.0).sum();
    let static_peak = beta * FIG4.iter().map(|x| x.1).fold(0.0, f64::max);
    for z in [0.0, 120.0, 168.0 - 1e-9, 190.0 - 1e-9] {
        let t = if z <= 0.0 {
            TAU[0] - 1e-9
        } else if z < Z_CORE_TOP {
            TAU[0] + TAU[1] * z / Z_CORE_TOP
        } else {
            TAU[0] + TAU[1] + TAU[2] * (z - Z_CORE_TOP) / (Z_TOP - Z_CORE_TOP)
        };
        let i = ((t / dt) as usize).min(n - 1);
        println!("  Fig. 5 check: z = {:5.1} cm, circulating / stationary peak = {:.3}", z, em[i] / static_peak);
    }

    // First-order convergence to the exact solution, and agreement at N = 10080.
    assert!(e2.abs() < 1e-5, "moltres error at N=10080: {e2:e}");
    let ratio = e1 / e2;
    assert!((3.0..5.0).contains(&ratio), "convergence ratio {ratio}");
}
