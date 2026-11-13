//! Verification of the homogeneous-equilibrium (HEM) two-phase speed of
//! sound against Kieffer (1977) Fig. 9b, at five saturation pressures
//! (gh #29, #1317 item 4).
//!
//! # Source
//!
//! Kieffer, S. W. (1977). Sound speed in liquid-gas mixtures: water-air and
//! water-steam. *Journal of Geophysical Research*, 82(20), 2895-2904,
//! **Fig. 9b**: equilibrium sound speed against steam mass fraction, isobars
//! at 5, 10, 50, 100 and 200 bar.
//!
//! The points were digitised by the maintainer with graphreader.com
//! (<https://www.graphreader.com/>) and posted on GitHub issue #29 on
//! 2026-08-21. They are used here **exactly as posted**. graphreader keeps no
//! axis-calibration record, so these points carry no `FigureSource` or
//! `PlotCalibration` record from `kovan-digitise`. The paper is not in the
//! open literature submodule (checked 2026-11-10 against
//! `reactor-literature` at `a1b8a4df`: none of the 161 files matches), so the
//! figure could not be re-read in this change.
//!
//! # Methodology
//!
//! For each digitised `(x, w)` point at pressure `p`:
//!
//! 1. `h = x h_g + (1 - x) h_f` at `T_sat(p)`. The saturated end states come
//!    from `h_tp_eqm_two_phase`, so 200 bar is handled in region 3.
//! 2. `s = s_ph_eqm(p, h)`.
//! 3. The model speed of sound comes from two routes:
//!    `w_ps_eqm_region4_kieffer` (Kieffer eq. 28, saturation-line
//!    derivatives) and `w_ps_eqm_region4_finite_diff_vol`
//!    (`c = v / sqrt(-dv/dp|_s)` by central difference).
//! 4. The deviation is `log10 w_model - log10 w_fig9b`.
//!
//! **Pass criterion, fixed before the points were run** (posted on #29 on
//! 2026-11-10): `|deviation| <= 0.1` (a factor of 1.26) at every point of all
//! five isobars, for both routes. This is the tolerance of `w_px_eqm_1_bar`.
//! It is not widened per isobar.
//!
//! # Results (2026-11-10, `tampines-steam-tables` on develop `3ec6c3592`)
//!
//! Worst signed deviation `dlog10` per isobar, with the quality where it
//! occurs (the criterion is `|dlog10| <= 0.1`):
//!
//! | isobar | points | Kieffer eq. 28 route | finite-difference-volume route |
//! |---|---|---|---|
//! | 5 bar   | 20 | +0.0358 at x = 0.0177 | +0.0358 at x = 0.0177 |
//! | 10 bar  | 19 | +0.0309 at x = 0.0198 | +0.0309 at x = 0.0198 |
//! | 50 bar  | 19 | +0.0519 at x = 0.00286 | +0.0519 at x = 1.24e-5 |
//! | 100 bar | 13 | -0.0433 at x = 0.264 | +0.0894 at x = 1.20e-5 |
//! | 200 bar | 10 | +0.0395 at x = 0.886 | **+0.1089 at x = 1.11e-5: FAILS** |
//!
//! **Interpretation.** The Kieffer eq. 28 route meets the criterion at all
//! 81 points, with a worst deviation of 0.052 (12 %). That is about the
//! scatter of graph-read data on a log axis. Above `x ≈ 1e-4`, the
//! finite-difference-volume route agrees with it to four figures at 5-100 bar.
//! At 200 bar the two routes differ by about 3 % (region 3 against the
//! region-1/2 equations extrapolated past their limit). Below `x ≈ 1e-4` the
//! finite-difference route rises sharply, because its pressure stencil
//! crosses the saturated-liquid line.
//! At 200 bar this breaks the criterion at one point
//! (see `finite_diff_route_misses_fig9b_200_bar_only_at_lowest_quality`).
//! The criterion was not widened. The miss is pinned and reported as a
//! defect of that route.
//!
//! The prediction was right about 5-50 bar (worst deviation 0.03-0.06,
//! predicted 0.03-0.06), and 100 bar passed although it was flagged as at
//! risk. **The prediction for 200 bar was wrong.** It said the Kieffer
//! route might miss at 200 bar, because it evaluates the region-1 equations
//! at `T_sat` = 638.9 K, above their 623.15 K limit. It does not miss: its
//! worst point there is +0.040. The miss came instead from the route the
//! prediction expected to be safe, and its cause was not foreseen.
//!
//! The results are also recorded in
//! `verification_and_validation/hem_sound_speed_vs_kieffer_fig9b.md`.

use petir::mathf::RealMath;
use uom::si::f64::*;
use uom::si::pressure::bar;
use uom::si::velocity::meter_per_second;

use crate::interfaces::functional_programming::ph_flash_eqm::s_ph_eqm;
use crate::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_two_phase;
use crate::region_4_vap_liq_equilibrium::{
    sat_temp_4, w_ps_eqm_region4_finite_diff_vol, w_ps_eqm_region4_kieffer,
};

/// The fixed pass criterion: `|log10 w_model - log10 w_fig9b| <= 0.1`.
const LOG10_TOLERANCE: f64 = 0.1;

/// Fig. 9b, 5 bar: `(steam quality x, HEM speed of sound in m/s)`.
const FIG9B_5_BAR: [(f64, f64); 20] = [
    (0.00001105, 4.5984097),
    (0.00002125, 4.53594371),
    (0.00005182, 4.5984097),
    (0.00011782, 4.72593424),
    (0.00032559, 4.99169106),
    (0.0008629, 5.64556942),
    (0.00196217, 7.32095714),
    (0.00337732, 9.6242741),
    (0.00706424, 15.11430491),
    (0.01043237, 18.55616683),
    (0.01770805, 27.21500603),
    (0.02615099, 39.37210142),
    (0.03970996, 53.92730683),
    (0.06647221, 81.28471924),
    (0.10235253, 115.99761622),
    (0.15760028, 156.72172209),
    (0.23274174, 208.86675518),
    (0.38959644, 274.58034014),
    (0.59989247, 337.10836387),
    (0.89833381, 431.21080511),
];

/// Fig. 9b, 10 bar.
const FIG9B_10_BAR: [(f64, f64); 19] = [
    (0.0000112, 8.62676437),
    (0.00001849, 8.74556641),
    (0.00004324, 8.86600451),
    (0.00008319, 8.74556641),
    (0.000174, 8.74556641),
    (0.00042419, 8.86600451),
    (0.00092512, 9.75681318),
    (0.00201758, 11.18685602),
    (0.00372307, 13.18220678),
    (0.00658917, 16.86196668),
    (0.01215907, 24.062923),
    (0.01979462, 31.63359136),
    (0.0292324, 42.73942065),
    (0.04628251, 60.16290238),
    (0.06375276, 78.01693639),
    (0.10524286, 115.99761622),
    (0.18368534, 172.46828178),
    (0.32964799, 252.94692432),
    (0.87366242, 419.5750189),
];

/// Fig. 9b, 50 bar.
const FIG9B_50_BAR: [(f64, f64); 19] = [
    (0.00001235, 33.41246554),
    (0.00002038, 33.41246554),
    (0.000047, 32.95858196),
    (0.0001054, 32.95858196),
    (0.0002, 32.51086405),
    (0.00040684, 32.95858196),
    (0.00078277, 32.51086405),
    (0.00148524, 32.51086405),
    (0.00285764, 32.95858196),
    (0.00668155, 38.3096852),
    (0.01477608, 47.03364885),
    (0.02405502, 56.18608212),
    (0.04083133, 69.93085606),
    (0.06200189, 81.28471924),
    (0.10093726, 114.42187457),
    (0.14496886, 146.36227974),
    (0.26751291, 220.61210759),
    (0.49364503, 314.82520735),
    (0.91092955, 425.35312589),
];

/// Fig. 9b, 100 bar.
const FIG9B_100_BAR: [(f64, f64); 13] = [
    (0.00001201, 69.93085606),
    (0.00003658, 68.04384283),
    (0.0002, 67.11951766),
    (0.00068102, 68.9808972),
    (0.00213314, 68.9808972),
    (0.00757358, 70.89389711),
    (0.02151936, 80.18052657),
    (0.05246146, 97.10218158),
    (0.09680781, 120.85624101),
    (0.1470015, 152.49274538),
    (0.26381391, 223.65023022),
    (0.41191079, 278.36167723),
    (0.80363969, 408.25321259),
];

/// Fig. 9b, 200 bar.
const FIG9B_200_BAR: [(f64, f64); 10] = [
    (0.00001105, 146.36227974),
    (0.00003139, 146.36227974),
    (0.00016005, 146.36227974),
    (0.00056826, 146.36227974),
    (0.00277916, 144.37405663),
    (0.00973078, 150.42124445),
    (0.03503289, 152.49274538),
    (0.11283086, 165.53476587),
    (0.34370953, 195.06048071),
    (0.88591224, 263.5417472),
];

/// Which model route to compare against the figure.
#[derive(Clone, Copy, Debug)]
enum HemRoute {
    /// `w_ps_eqm_region4_kieffer` (Kieffer eq. 28).
    KiefferEq28,
    /// `w_ps_eqm_region4_finite_diff_vol`.
    FiniteDiffVolume,
}

/// Signed `log10 w_model - log10 w_fig9b` at every point of one isobar.
fn log10_deviations(p_bar: f64, points: &[(f64, f64)], route: HemRoute) -> Vec<(f64, f64, f64)> {
    let p = Pressure::new::<bar>(p_bar);
    let t_sat = sat_temp_4(p);
    let h_liq = h_tp_eqm_two_phase(t_sat, p, 0.0);
    let h_vap = h_tp_eqm_two_phase(t_sat, p, 1.0);
    points
        .iter()
        .map(|&(x, w_fig)| {
            let h = x * h_vap + (1.0 - x) * h_liq;
            let s = s_ph_eqm(p, h);
            let w = match route {
                HemRoute::KiefferEq28 => w_ps_eqm_region4_kieffer(p, s),
                HemRoute::FiniteDiffVolume => w_ps_eqm_region4_finite_diff_vol(p, s),
            }
            .get::<meter_per_second>();
            (x, w, w.r_log10() - w_fig.r_log10())
        })
        .collect()
}

/// Runs one isobar on one route, prints every point and returns the
/// signed deviations, with the worst one printed last.
fn measure_isobar(p_bar: f64, points: &[(f64, f64)], route: HemRoute) -> Vec<(f64, f64, f64)> {
    let devs = log10_deviations(p_bar, points, route);
    let mut worst = (0.0_f64, 0.0_f64);
    for ((x, w, d), (_, w_fig)) in devs.iter().zip(points.iter()) {
        println!(
            "MEASURE Fig9b {p_bar} bar {route:?}: x = {x:.8}, w = {w:.3} m/s, fig = {w_fig:.3} m/s, dlog10 = {d:+.4}"
        );
        if d.abs() > worst.0.abs() {
            worst = (*d, *x);
        }
    }
    println!(
        "MEASURE Fig9b {p_bar} bar {route:?}: worst dlog10 = {:+.4} at x = {:.8} ({} points)",
        worst.0,
        worst.1,
        points.len()
    );
    devs
}

/// Asserts the pre-registered criterion at every point of one isobar.
fn check_isobar(p_bar: f64, points: &[(f64, f64)], route: HemRoute) {
    for (x, _, d) in measure_isobar(p_bar, points, route).iter() {
        assert!(
            d.abs() <= LOG10_TOLERANCE,
            "Fig. 9b {p_bar} bar {route:?}: x = {x}, dlog10 = {d:+.4} > {LOG10_TOLERANCE}"
        );
    }
}

#[test]
fn hem_sound_speed_vs_kieffer_fig9b_5_bar() {
    check_isobar(5.0, &FIG9B_5_BAR, HemRoute::KiefferEq28);
    check_isobar(5.0, &FIG9B_5_BAR, HemRoute::FiniteDiffVolume);
}

#[test]
fn hem_sound_speed_vs_kieffer_fig9b_10_bar() {
    check_isobar(10.0, &FIG9B_10_BAR, HemRoute::KiefferEq28);
    check_isobar(10.0, &FIG9B_10_BAR, HemRoute::FiniteDiffVolume);
}

#[test]
fn hem_sound_speed_vs_kieffer_fig9b_50_bar() {
    check_isobar(50.0, &FIG9B_50_BAR, HemRoute::KiefferEq28);
    check_isobar(50.0, &FIG9B_50_BAR, HemRoute::FiniteDiffVolume);
}

#[test]
fn hem_sound_speed_vs_kieffer_fig9b_100_bar() {
    check_isobar(100.0, &FIG9B_100_BAR, HemRoute::KiefferEq28);
    check_isobar(100.0, &FIG9B_100_BAR, HemRoute::FiniteDiffVolume);
}

#[test]
fn hem_sound_speed_vs_kieffer_fig9b_200_bar() {
    check_isobar(200.0, &FIG9B_200_BAR, HemRoute::KiefferEq28);
}

/// **Known miss, pinned (2026-11-10).** On the finite-difference-volume
/// route at 200 bar, exactly one Fig. 9b point fails the pre-registered
/// `0.1` criterion: the lowest quality, `x = 1.105e-5`, at
/// `dlog10 = +0.1089` (188.06 m/s against 146.36 m/s; the Kieffer eq. 28
/// route gives 145.70 m/s there, `-0.0020`).
///
/// Cause: the central-difference stencil `dp = 1e-4 p` (2 kPa at 200 bar)
/// crosses the saturated-liquid line. `ds_f/dp` is about
/// `6.6e-3 kJ/(kg K)` per bar at 200 bar (IF97 `s_f`: 3.685 at 150 bar,
/// 4.014 at 200 bar), so `s_f` rises by about `1.3e-4` across `+dp`, while
/// `s - s_f = x s_fg` is only about `1.0e-5` at this quality (`s_fg` = 0.913). The `+dp`
/// state is therefore subcooled liquid. Its far smaller compressibility
/// raises the computed speed. The same estimate puts the crossing at
/// `x ≈ 1.4e-4`, and the measured jump agrees: `x = 3.1e-5` gives `+0.0903`
/// and `x = 1.6e-4` gives `-0.0003`. At 100 bar the same artefact gives
/// `+0.0894` at `x = 1.2e-5`, inside the criterion.
///
/// This is a defect of `w_ps_eqm_region4_finite_diff_vol` near `x = 0` (gh #1325),
/// not of the figure. The criterion is not widened. This test asserts that
/// every other 200-bar point passes and that the one miss is still the
/// recorded one, so a fix to the stencil makes this test fail and prompts
/// its update.
#[test]
fn finite_diff_route_misses_fig9b_200_bar_only_at_lowest_quality() {
    let devs = measure_isobar(200.0, &FIG9B_200_BAR, HemRoute::FiniteDiffVolume);
    for (x, _, d) in devs.iter().skip(1) {
        assert!(
            d.abs() <= LOG10_TOLERANCE,
            "Fig. 9b 200 bar FiniteDiffVolume: x = {x}, dlog10 = {d:+.4} > {LOG10_TOLERANCE}"
        );
    }
    let (x0, _, d0) = devs[0];
    assert_eq!(x0, 0.00001105);
    assert!(
        d0 > LOG10_TOLERANCE && d0 < 0.12,
        "recorded miss moved: dlog10 = {d0:+.4} (recorded +0.1089)"
    );
}
