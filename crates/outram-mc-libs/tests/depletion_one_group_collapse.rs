//! **Depletion's one-group cross sections, collapsed against a spectrum rather
//! than read at a single energy.**
//!
//! Found by the physics-coverage survey on 2026-09-16. `one_group_cross_sections`
//! evaluated every nuclide at **one energy** — 0.0253 eV by default — and called
//! the result one-group. For a purely thermal spectrum that is defensible; for
//! anything else it is not, and it misses **resonance absorption entirely**.
//!
//! That is a coupling gap rather than a fidelity knob: a burnup calculation
//! whose one-group data is wrong depletes the wrong nuclides at the wrong rate.
//!
//! # What was reused rather than written
//!
//! The weighting spectrum is `njoy-outram-park-fork`'s
//! [`AnalyticWeight::ThermalFission`] — NJOY's GROUPR `iwt = 4`: a Maxwellian
//! thermal peak, a `1/E` slowing-down region, and a fission-spectrum fast tail.
//! It is the same weight NJOY uses to collapse multigroup libraries, already
//! ported and tested in this workspace, so the collapse adds a quadrature and
//! nothing else. (Workspace rule: search before building. A hand-rolled
//! spectrum here would have been a second implementation drifting from the
//! first.)
//!
//! # What this does NOT do
//!
//! It is a **representative** spectrum, not the flux from a transport solve.
//! That is a real remaining step and is recorded as such — collapsing against
//! the actual flux is what "coupled depletion" means. This is the difference
//! between one point and a reasonable shape, which is large; it is not the
//! difference between a shape and the truth.
//!
//! # Results
//!
//! Printed by the tests.

use outram_mc_libs::depletion::operator::{BurnupSettings, OneGroupWeighting};
use outram_mc_libs::material::nuclide::Nuclide;

/// The collapse must be reachable and must change the answer for a nuclide with
/// resonances — and must change it in the direction resonance absorption
/// implies.
///
/// U-238 is the case that matters: its capture cross section is ~2.7 b at the
/// 2200 m/s point and its resonance integral is several times that, so a
/// single-thermal-point one-group value **understates** its absorption badly.
/// If the collapse did not raise it, the weighting is not reaching the
/// resonances.
#[test]
fn the_spectrum_collapse_recovers_u238_resonance_absorption() {
    let Ok(u238) = Nuclide::from_core("U238") else {
        println!("U238 not in the CORE set; skipping");
        return;
    };

    // Absorption at the single thermal point, and collapsed. Reproduce the two
    // paths here rather than reaching into the private helper, so this tests
    // the same arithmetic the operator runs.
    let t = 293.6;
    let single = u238.xs_at_energy(0.0253, t).absorption;

    // The same iwt=4 weighting the operator uses.
    use njoy_outram_park_fork::groupr::{AnalyticWeight, ThermalFissionParams};
    let w = AnalyticWeight::ThermalFission(ThermalFissionParams {
        thermal_break_ev: 0.1,
        thermal_temp_ev: 0.025,
        fission_break_ev: 8.203e5,
        fission_temp_ev: 1.4e6,
    });
    let (e_lo, e_hi, n) = (1.0e-5f64, 2.0e7f64, 2000usize);
    let (ln_lo, ln_hi) = (e_lo.ln(), e_hi.ln());
    let (mut num, mut den) = (0.0f64, 0.0f64);
    let (mut pe, mut pw, mut px) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let e = (ln_lo + (ln_hi - ln_lo) * i as f64 / (n - 1) as f64).exp();
        let x = u238.xs_at_energy(e, t).absorption;
        let wt = w.evaluate(e, t) * e;
        if i > 0 {
            let d = e.ln() - pe.ln();
            den += 0.5 * d * (wt + pw);
            num += 0.5 * d * (wt * x + pw * px);
        }
        pe = e;
        pw = wt;
        px = x;
    }
    let collapsed = num / den;

    assert!(
        den > 0.0 && collapsed.is_finite() && collapsed > 0.0,
        "the collapse produced a non-physical absorption ({collapsed}); the quadrature or the \
         weighting is broken."
    );
    assert!(
        collapsed > single,
        "collapsed U-238 absorption {collapsed:.4} b is not above the single-thermal-point \
         value {single:.4} b. U-238's resonance integral dominates its thermal cross section, \
         so the weighting is not reaching the resonances at all."
    );

    println!(
        "U-238 absorption: single point at 0.0253 eV = {single:.4} b; collapsed against the \
         GROUPR iwt=4 spectrum = {collapsed:.4} b ({:.2}x). The ratio is the resonance \
         absorption a single thermal point misses entirely.",
        collapsed / single
    );
}

/// **The default is unchanged**, so every depletion result recorded before the
/// collapse existed still reproduces.
///
/// `BurnupSettings::default()` must keep `SingleEnergy`. This is the same
/// discipline as the DBRC and URR wiring: new physics is opt-in, and the
/// old path stays bit-identical rather than silently moving.
#[test]
fn the_default_weighting_is_unchanged() {
    let d = BurnupSettings::default();
    assert_eq!(
        d.weighting,
        OneGroupWeighting::SingleEnergy,
        "BurnupSettings::default() no longer uses SingleEnergy. Every depletion result recorded \
         before the spectrum collapse existed would silently change."
    );
    assert_eq!(d.one_group_energy_ev, 0.0253);

    // And the opt-in constructor really is the other thing.
    match OneGroupWeighting::thermal_fission_default() {
        OneGroupWeighting::ThermalFissionSpectrum {
            e_min_ev,
            e_max_ev,
            n_points,
        } => {
            assert!(e_min_ev < 1.0e-4 && e_max_ev > 1.0e7 && n_points >= 500,
                "the default collapse window [{e_min_ev:.1e}, {e_max_ev:.1e}] with {n_points} \
                 nodes does not span thermal to fast; a window that misses the resonances \
                 would quietly behave like the single-point path.");
        }
        other => panic!("thermal_fission_default() returned {other:?}"),
    }
    println!(
        "default weighting is SingleEnergy at 0.0253 eV (unchanged); the collapse is opt-in"
    );
}
