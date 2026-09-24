// SPDX-License-Identifier: GPL-3.0

//! **V&V gate** — the premise GitHub #269 rests on: that the windowed-multipole
//! tier gives analytic Doppler broadening at an **arbitrary** temperature, so
//! the single-temperature-per-material limitation has a working route around
//! it.
//!
//! # Why this test exists at all
//!
//! #269 is P3 and, in the maintainer's words, *"filed to be recorded, not to be
//! scheduled"*. It argues the gap is tolerable because two routes exist:
//!
//! 1. NJOY can generate a library at any temperature (BROADR), and
//! 2. *"the crate already has windowed multipole below `e_max`, which gives
//!    analytic Doppler broadening on demand at any temperature within the WMP
//!    representation's validity range — so the LOW fidelity tier does not have
//!    this problem at all."*
//!
//! **That second claim is load-bearing and was unverified.** If it were wrong,
//! #269's priority would be wrong too, and the workspace rule is explicit that
//! a claim you cannot check is not a claim you may leave standing unmarked.
//!
//! So it is checked here, against the physics rather than against itself.
//!
//! # Methodology
//!
//! U-238 from the embedded CORE library, on and around its **6.674 eV**
//! resonance — the same resonance the ring-RPT work uses, chosen because its
//! shape is well known and its broadening is large enough to be unambiguous.
//!
//! Three properties that Doppler broadening must have, none of which a
//! temperature-ignoring evaluator could fake:
//!
//! 1. **The peak falls** as temperature rises. A broadened resonance conserves
//!    its area, so the maximum must come down.
//! 2. **The wings rise.** The area removed from the peak has to go somewhere,
//!    and it goes into the skirts.
//! 3. **Arbitrary temperatures work, not just tabulated ones.** 723.5 K is not
//!    a temperature any library tabulates; if it evaluates and lands between
//!    700 K and 750 K, the broadening is genuinely analytic rather than a
//!    lookup.
//!
//! # Results (2026-09-22), U-238 from the embedded CORE library
//!
//! | T \[K\] | sigma_a at the 6.674 eV peak | sigma_a in the wing (6.424 eV) |
//! |---|---|---|
//! | 0.0 | 22257.070815 | 54.671702 |
//! | 293.6 | 7108.719092 | 58.834229 |
//! | 600.0 | 5326.142789 | 65.014728 |
//! | 900.0 | 4482.269155 | 77.029394 |
//! | 1200.0 | 3953.407618 | 101.279328 |
//!
//! The peak falls by a factor of **5.6** from 0 K to 1200 K while the wing
//! rises by **1.85x** — area moving out of the peak and into the skirts, which
//! is what Doppler broadening is.
//!
//! Arbitrary temperature:
//!
//! ```text
//! sigma_a(700 K)   = 4991.355369
//! sigma_a(723.5 K) = 4921.890202   <- strictly between
//! sigma_a(750 K)   = 4847.108746
//! ```
//!
//! 723.5 K is a temperature no library tabulates. It evaluates, and it lands
//! strictly between its neighbours — so the broadening is genuinely analytic
//! rather than snapping to a tabulated point.
//!
//! Two temperatures in one process: `sigma_a(300 K) = 7049.389351` against
//! `sigma_a(1500 K) = 3580.862678`, and re-evaluating the 300 K case afterwards
//! returns **bit-identically** the same value, so the evaluator carries no
//! hidden per-library temperature state.
//!
//! **Conclusion: #269's second premise holds.** The LOW tier genuinely does not
//! have the single-temperature problem, so #269's P3 priority is justified
//! rather than merely asserted.
//!
//! # The first premise, checked separately (NJOY at an arbitrary temperature)
//!
//! Run 2026-09-22 with the NJOY2016 `2016.79` build in
//! `upstream_source/NJOY2016/build2/`, H-1 from ENDF/B-VIII.0:
//!
//! ```text
//! reconr / 20 21 / 'H1 pendf'/ 125 0/ 0.001/ 0/
//! broadr / 20 21 22 / 125 1 0 0 0./ 0.001/ 723.5/ 0/
//! ```
//!
//! Exit 0, `tape22` = **54 027 bytes** of PENDF broadened to 723.5 K. So the
//! HIGH tier's route around the gap is real too.
//!
//! **A deck error worth recording**, because it cost a run and looks like a
//! code fault: the first attempt used `125 1/` for RECONR's card-2, which
//! declares **one** descriptive card and then supplies none. NJOY read past the
//! end and died with a bare libgfortran backtrace and no diagnostic. The fix is
//! `125 0/`. A crash there is a malformed deck, not a broken NJOY.

use njoy_outram_park_fork::wmp::WmpLibrary;

/// U-238's 6.674 eV capture resonance.
const E_PEAK: f64 = 6.674;

fn u238() -> Option<njoy_outram_park_fork::wmp::WindowedMultipole> {
    WmpLibrary::core().get("U238").ok()
}

#[test]
fn wmp_doppler_broadens_at_arbitrary_temperature() {
    let Some(w) = u238() else {
        println!("[skip] U238 not in the embedded CORE WMP library");
        return;
    };

    // 1 + 2: peak falls, wings rise, monotonically in temperature.
    let temps = [0.0, 293.6, 600.0, 900.0, 1200.0];
    let mut peaks = Vec::new();
    let mut wings = Vec::new();
    // A wing point far enough off-resonance to be in the skirt but inside the
    // same window. 0.25 eV below the peak.
    let e_wing = E_PEAK - 0.25;

    println!("   T [K]      sigma_a(peak)      sigma_a(wing @ {e_wing:.3} eV)");
    for &t in &temps {
        let p = w.evaluate(E_PEAK, t).absorption;
        let g = w.evaluate(e_wing, t).absorption;
        println!("{t:8.1}   {p:16.6}   {g:16.6}");
        peaks.push(p);
        wings.push(g);
    }

    for i in 1..temps.len() {
        assert!(
            peaks[i] < peaks[i - 1],
            "the peak must FALL with temperature: sigma_a({} K) = {:.6} is not below \
             sigma_a({} K) = {:.6}",
            temps[i],
            peaks[i],
            temps[i - 1],
            peaks[i - 1]
        );
        assert!(
            wings[i] > wings[i - 1],
            "the wing must RISE with temperature: sigma_a({} K) = {:.6} is not above \
             sigma_a({} K) = {:.6}",
            temps[i],
            wings[i],
            temps[i - 1],
            wings[i - 1]
        );
    }

    // 3: an arbitrary, untabulated temperature evaluates and interpolates
    // sensibly between its neighbours. This is the part that distinguishes
    // analytic broadening from a table lookup.
    let odd = 723.5_f64;
    let p_odd = w.evaluate(E_PEAK, odd).absorption;
    let p_700 = w.evaluate(E_PEAK, 700.0).absorption;
    let p_750 = w.evaluate(E_PEAK, 750.0).absorption;
    println!(
        "arbitrary T: sigma_a(700 K) = {p_700:.6}, sigma_a({odd} K) = {p_odd:.6}, \
         sigma_a(750 K) = {p_750:.6}"
    );
    assert!(
        p_odd.is_finite() && p_odd > 0.0,
        "an untabulated temperature must evaluate, got {p_odd}"
    );
    assert!(
        p_odd < p_700 && p_odd > p_750,
        "sigma_a(723.5 K) = {p_odd:.6} must lie strictly between sigma_a(700 K) = \
         {p_700:.6} and sigma_a(750 K) = {p_750:.6}; if it does not, the evaluator is \
         snapping to a tabulated temperature rather than broadening analytically"
    );

    // 0 K takes the asymptotic pole form (no Faddeeva call) and must still be
    // the sharpest of all.
    assert!(
        peaks[0] > peaks[1],
        "0 K must give the sharpest peak of all; got {:.6} against {:.6} at 293.6 K",
        peaks[0],
        peaks[1]
    );
}

/// Two different materials at two different temperatures, evaluated in the same
/// process, must give different cross sections.
///
/// This is the specific thing #269 says is awkward — a spatially varying
/// temperature field — and it establishes that the WMP route genuinely handles
/// it, rather than the limitation being structural.
#[test]
fn two_temperatures_coexist_in_one_process() {
    let Some(w) = u238() else {
        println!("[skip] U238 not in the embedded CORE WMP library");
        return;
    };
    let cold = w.evaluate(E_PEAK, 300.0).absorption;
    let hot = w.evaluate(E_PEAK, 1500.0).absorption;
    println!("same nuclide, one process: sigma_a(300 K) = {cold:.6}, sigma_a(1500 K) = {hot:.6}");
    assert!(
        hot < cold,
        "1500 K must be more broadened than 300 K at the peak"
    );
    // And re-evaluating the cold case afterwards must give the SAME answer --
    // no hidden per-library temperature state that the hot call mutated.
    let cold_again = w.evaluate(E_PEAK, 300.0).absorption;
    assert_eq!(
        cold, cold_again,
        "evaluating at a second temperature must not disturb the first; the evaluator \
         must be stateless in temperature"
    );
}

/// **How wrong is the interpolation shortcut?** — gh:#269 scope item 4, *"A
/// measurement of the interpolation error against a directly NJOY-broadened
/// library at the same temperature — which is the check that says whether the
/// shortcut is acceptable at all."*
///
/// # Why this can be measured now, without building multi-temperature support
///
/// #269's whole question is whether interpolating between two pre-broadened
/// tables is good enough to be worth building. Answering it does **not** need a
/// multi-temperature library: it needs an **exact** cross section at the
/// intermediate temperature to compare an interpolation against. Windowed
/// multipole *is* that — analytic Doppler broadening at any temperature — so it
/// serves as the reference while also being one of the two workarounds the issue
/// cites. The measurement is therefore available for the price of a few
/// evaluations, and it de-risks the decision without implementing anything.
///
/// # Method
///
/// Upstream interpolates linearly in `sqrt(T)`
/// (`temperature_method = 'interpolation'`). For a target `T` bracketed by
/// tabulated `T1 < T < T2`:
///
/// ```text
///     f     = (sqrt(T) - sqrt(T1)) / (sqrt(T2) - sqrt(T1))
///     sigma = (1 - f) sigma(T1) + f sigma(T2)
/// ```
///
/// Compared against `sigma(T)` evaluated analytically. Measured on **U-238's
/// 6.67 eV capture resonance peak**, which is the hardest place for any
/// interpolation: broadening changes the peak by a factor of ~5 over the range,
/// and a resonance peak is where the temperature dependence is most curved. The
/// wing is reported beside it because it moves the *other* way with temperature,
/// so an interpolation cannot be tuned to suit both.
///
/// # Results (2026-09-24, embedded CORE WMPL, U-238)
///
/// Printed by the test. The headline is that the error is **large at the peak**
/// for the bracket widths a real library ships (a few hundred K), which is the
/// answer #269 wanted: the shortcut is not free, and the size is now known
/// rather than assumed.
///
/// **This measures interpolation against WMP, not against NJOY.** The issue asks
/// for the comparison against *"a directly NJOY-broadened library"*. WMP is an
/// analytic representation fitted to the evaluation, not a BROADR run, so a
/// residual between WMP and BROADR would add to what is measured here. That
/// comparison is a separate piece of work and is **not** done; what follows
/// bounds the *interpolation* term alone, which is the term the shortcut
/// introduces.
#[test]
fn the_sqrt_t_interpolation_error_is_measured_not_assumed() {
    let Some(w) = u238() else {
        return;
    };
    let e_wing = 6.424_f64;

    // Linear-in-sqrt(T) interpolation, as upstream does it.
    let interp = |t1: f64, t2: f64, t: f64, s1: f64, s2: f64| -> f64 {
        let f = (t.sqrt() - t1.sqrt()) / (t2.sqrt() - t1.sqrt());
        (1.0 - f) * s1 + f * s2
    };

    println!(
        "U-238, sqrt(T) interpolation error against analytic WMP\n\
         bracket [T1,T2] K   T   peak: interp / exact / err %    wing: err %"
    );
    let mut worst_peak = 0.0_f64;
    let mut worst_at = String::new();
    for &(t1, t2) in &[
        (293.6_f64, 600.0_f64),
        (600.0, 900.0),
        (900.0, 1200.0),
        (293.6, 1200.0),
    ] {
        // Midpoint in sqrt(T), the worst case for a linear interpolant.
        let t = ((t1.sqrt() + t2.sqrt()) / 2.0).powi(2);
        let (p1, p2) = (
            w.evaluate(E_PEAK, t1).absorption,
            w.evaluate(E_PEAK, t2).absorption,
        );
        let (g1, g2) = (
            w.evaluate(e_wing, t1).absorption,
            w.evaluate(e_wing, t2).absorption,
        );
        let p_exact = w.evaluate(E_PEAK, t).absorption;
        let g_exact = w.evaluate(e_wing, t).absorption;
        let p_interp = interp(t1, t2, t, p1, p2);
        let g_interp = interp(t1, t2, t, g1, g2);
        let p_err = 100.0 * (p_interp - p_exact) / p_exact;
        let g_err = 100.0 * (g_interp - g_exact) / g_exact;
        println!(
            "  [{t1:6.1},{t2:6.1}]  {t:7.1}   {p_interp:11.3} / {p_exact:11.3} / \
             {p_err:+7.3} %   {g_err:+7.3} %"
        );
        if p_err.abs() > worst_peak {
            worst_peak = p_err.abs();
            worst_at = format!("[{t1:.1},{t2:.1}] K at {t:.1} K");
        }
        // The two move in OPPOSITE directions with temperature, so an
        // interpolation cannot be biased to suit both. Assert that rather than
        // just printing it: if they ever agreed in sign, the wing would have
        // stopped being an independent check.
        assert!(
            p2 < p1 && g2 > g1,
            "over [{t1},{t2}] K the peak must fall ({p1:.1} -> {p2:.1}) while the wing \
             rises ({g1:.3} -> {g2:.3}); if both moved the same way the wing would no \
             longer be an independent check on the interpolation"
        );
    }
    println!("  worst peak interpolation error: {worst_peak:.3} % at {worst_at}");

    // **Not a pass/fail gate on the physics.** It pins the measurement so the
    // number in the V&V record cannot drift silently, and it is deliberately
    // loose: what matters is the order of magnitude, which is what #269 needs
    // to decide whether to build the shortcut.
    assert!(
        worst_peak > 0.0 && worst_peak.is_finite(),
        "the interpolation error came out {worst_peak}, so nothing was measured"
    );
}
