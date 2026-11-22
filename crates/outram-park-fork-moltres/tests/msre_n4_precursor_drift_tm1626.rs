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

/// **Our own shape** (gh:#1130, 2026-11-10): the N-1 model's R-Z tally
/// (`msre_n1_first_criticality --mo natural --seeds 1-2 --axial-csv`, arm C,
/// RVS, 30P; `verification_and_validation/msre_n4_precursor_drift/data/n1_axial_tally.csv`)
/// through `scripts/axial_shape.py`: fission per unit transit time (inner
/// ring in the graphite, both rings in the plenums) and, as a one-group
/// importance proxy, the scalar flux per unit volume inside the graphite
/// radius. `(z cm, S_f, φ)` at bin centres, each normalised to 1.
/// z_bottom -16.6124, z_core_top 162.5600, z_top 180.0030; seeds ['1', '2']
const MC_AXIAL: [(f64, f64, f64); 40] = [
    (-14.5358, 0.04484, 0.12797),
    (-10.3828, 0.07199, 0.18728),
    (-6.2297, 0.10749, 0.23832),
    (-2.0766, 0.16044, 0.27702),
    (2.5400, 0.24172, 0.34140),
    (7.6200, 0.35668, 0.41386),
    (12.7000, 0.45328, 0.48339),
    (17.7800, 0.53702, 0.55118),
    (22.8600, 0.60990, 0.61565),
    (27.9400, 0.67454, 0.67575),
    (33.0200, 0.73143, 0.73193),
    (38.1000, 0.78604, 0.78378),
    (43.1800, 0.83356, 0.83136),
    (48.2600, 0.87516, 0.87357),
    (53.3400, 0.91002, 0.90934),
    (58.4200, 0.93871, 0.93794),
    (63.5000, 0.96249, 0.96186),
    (68.5800, 0.97940, 0.98108),
    (73.6600, 0.99120, 0.99393),
    (78.7400, 0.99781, 0.99996),
    (83.8200, 1.00000, 1.00000),
    (88.9000, 0.99359, 0.99402),
    (93.9800, 0.98062, 0.98198),
    (99.0600, 0.96187, 0.96352),
    (104.1400, 0.94041, 0.94162),
    (109.2200, 0.91152, 0.91173),
    (114.3000, 0.87739, 0.87677),
    (119.3800, 0.83406, 0.83530),
    (124.4600, 0.78778, 0.78766),
    (129.5400, 0.73636, 0.73644),
    (134.6200, 0.67784, 0.68047),
    (139.7000, 0.61494, 0.62131),
    (144.7800, 0.54249, 0.55728),
    (149.8600, 0.45972, 0.49144),
    (154.9400, 0.36265, 0.42134),
    (160.0200, 0.24569, 0.34887),
    (164.7404, 0.16579, 0.28413),
    (169.1011, 0.10934, 0.24210),
    (173.4618, 0.07324, 0.18893),
    (177.8226, 0.04539, 0.12876),
];

/// [`MC_AXIAL`]'s fission source with the **fast flux** (above 0.625 eV,
/// the conventional two-group thermal cutoff; TM-1626 gives no boundary) as
/// the importance, a closer stand-in for Prince's fast-group adjoint. From
/// one seed with the energy-split tally
/// (`data/n1_axial_tally_energy_split.csv`, `scripts/axial_shape.py ... fast`).
/// importance flux_fast_inner; z_bottom -16.6124, z_core_top 162.5600, z_top 180.0030; seeds ['1']
const MC_AXIAL_FAST: [(f64, f64, f64); 40] = [
    (-14.5358, 0.04474, 0.16667),
    (-10.3828, 0.07185, 0.24068),
    (-6.2297, 0.10767, 0.29996),
    (-2.0766, 0.16039, 0.33605),
    (2.5400, 0.24141, 0.38469),
    (7.6200, 0.35822, 0.43951),
    (12.7000, 0.45484, 0.49702),
    (17.7800, 0.53950, 0.55757),
    (22.8600, 0.61342, 0.61875),
    (27.9400, 0.67720, 0.67759),
    (33.0200, 0.73720, 0.73439),
    (38.1000, 0.79128, 0.78558),
    (43.1800, 0.83701, 0.83230),
    (48.2600, 0.87899, 0.87345),
    (53.3400, 0.91320, 0.90895),
    (58.4200, 0.94076, 0.93686),
    (63.5000, 0.96181, 0.96100),
    (68.5800, 0.97983, 0.98032),
    (73.6600, 0.99252, 0.99412),
    (78.7400, 0.99932, 1.00000),
    (83.8200, 1.00000, 0.99918),
    (88.9000, 0.99490, 0.99274),
    (93.9800, 0.98171, 0.98052),
    (99.0600, 0.96013, 0.96217),
    (104.1400, 0.93882, 0.93885),
    (109.2200, 0.90948, 0.90662),
    (114.3000, 0.87550, 0.87080),
    (119.3800, 0.83224, 0.82953),
    (124.4600, 0.78521, 0.78008),
    (129.5400, 0.73393, 0.72864),
    (134.6200, 0.67504, 0.67506),
    (139.7000, 0.61156, 0.61794),
    (144.7800, 0.53861, 0.55865),
    (149.8600, 0.45642, 0.50050),
    (154.9400, 0.36038, 0.44278),
    (160.0200, 0.24420, 0.39015),
    (164.7404, 0.16453, 0.34275),
    (169.1011, 0.10852, 0.30346),
    (173.4618, 0.07244, 0.24185),
    (177.8226, 0.04521, 0.16683),
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

    /// The N-1 Monte Carlo shape, [`MC_AXIAL`], with the model's own region
    /// extents (cold dimensions: plenums of 10.0 and 10.5 ft³ in a 58 in
    /// vessel, 64 in of graphite).
    fn n1_monte_carlo() -> Shape {
        Shape { table: MC_AXIAL.to_vec(), z_bottom: -16.6124, z_core_top: 162.56, z_top: 180.003 }
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

/// **N-4 on our own shapes:** the same line model and moltres drift, driven
/// by the N-1 Monte Carlo model's axial fission and flux ([`MC_AXIAL`])
/// instead of TM-1626 Fig. 4. Verification gate as above (moltres within
/// 1e-5 of the exact loss); the loss itself is reported, not gated.
///
/// **Result (2026-11-10, release build):** see the V&V README
/// (`verification_and_validation/msre_n4_precursor_drift/README.md`) and
/// gh:#1130.
#[test]
fn n4_on_the_n1_monte_carlo_shape() {
    let sh = Shape::n1_monte_carlo();
    let exact = exact_loss(&sh, 252_000, LowerPlenum::PlugFlow);
    let stirred = exact_loss(&sh, 252_000, LowerPlenum::WellStirred);
    let (m2, _) = moltres_loss(&sh, 10080, 1.0);
    let mut no_plenum = sh.clone();
    for row in no_plenum.table.iter_mut() {
        if row.0 < 0.0 || row.0 > sh.z_core_top {
            row.2 = 0.0;
        }
    }
    let ablated = exact_loss(&no_plenum, 252_000, LowerPlenum::PlugFlow);
    let fast = Shape { table: MC_AXIAL_FAST.to_vec(), ..sh.clone() };
    let exact_fast = exact_loss(&fast, 252_000, LowerPlenum::PlugFlow);
    let (m_fast, _) = moltres_loss(&fast, 10080, 1.0);
    println!("N-4 on the N-1 Monte Carlo shape, loss in % dk/k");
    println!("  exact, plug flow / well-stirred   {:.4} / {:.4}", exact * 100.0, stirred * 100.0);
    println!("  moltres N = 10080                 {:.4} (error {:+.2e})", m2 * 100.0, m2 - exact);
    println!("  plenum importance zeroed          {:.4}", ablated * 100.0);
    println!("  fast-flux importance (> 0.625 eV)  {:.4} (moltres {:.4})", exact_fast * 100.0, m_fast * 100.0);
    assert!((m2 - exact).abs() < 1e-5, "moltres error {:e}", m2 - exact);
    assert!((m_fast - exact_fast).abs() < 1e-5, "moltres error {:e}", m_fast - exact_fast);
}
