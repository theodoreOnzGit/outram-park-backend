//! **Which stage loses the U-238 20 keV discontinuity: RECONR or BROADR?**
//!
//! Neither, once BROADR is bounded at `thnmax` the way NJOY bounds it. This
//! program is the measurement that established that (`op-sdbk`, 2026-09-10),
//! and it also answers the fast-fission hypothesis raised alongside it.
//!
//! # Why it exists: a port defect that is a missing LIMIT, not a wrong formula
//!
//! NJOY bounds BROADR at `thnmax` and never runs SIGMA1 across the
//! resolved/unresolved boundary. This port broadened the whole grid
//! unconditionally. That is the archetypal port defect — a missing *guard*
//! rather than a wrong equation — and it is why this repository's `CLAUDE.md`
//! carries "read upstream first" as a hard rule: formulas get reviewed
//! line-by-line during translation, control flow does not.
//!
//! Each window is dumped after RECONR, after the **unbounded** kernel
//! (`doppler_broaden`, the pre-fix behaviour), and after the upstream-faithful
//! **bounded** kernel (`broaden_result`), against the tape's own MF=3 values
//! interpolated lin-lin. The tape is its own oracle: no second code is involved.
//!
//! # V&V results (2026-09-11, U-238 ENDF/B-VIII.0, 600 K)
//!
//! `resonance_upper_limit = 20000 eV`, `thnmax = 2.000000e4 eV`.
//!
//! **At the seam**, just above 20 keV, against the tape's own MF=3:
//!
//! ```text
//!   E [eV]        tape      RECONR 0K   unbounded 600K   bounded 600K
//!   2.000001e4   0.529870    0.529870      0.282259        0.529870
//!   2.050e4      0.522620    0.522620      0.305960        0.522620
//!   2.100e4      0.515360    0.515360      0.329660        0.515360
//!   2.200e4      0.500860    0.500860      0.377050        0.500860
//!   2.300e4      0.486350    0.486350      0.424450        0.486350
//! ```
//!
//! The unbounded kernel destroys **46.7 %** of the unresolved capture cross
//! section immediately above the boundary — it is smearing resolved-resonance
//! structure from below the seam across a discontinuity that is real — and
//! recovers only slowly, still 13 % low at 23 keV. The bounded kernel reproduces
//! the tape to **+0.000 %** at every point above the seam.
//!
//! **MT=18 fission**: the unbounded kernel's worst deviation above 20 keV is
//! **19.6 %**, also at the seam (E = 2.000001e4).
//!
//! **MT=16 (n,2n) at its 6.179 MeV threshold** — the fast-fission hypothesis:
//!
//! ```text
//!   E [eV]       tape        unbounded 600K   bounded 600K
//!   6.179071e6   0            1.008916e-6      0
//!   6.300e6      5.877499e-3  5.881614e-3      5.877499e-3   (+0.070 % / +0.000 %)
//!   6.500e6      7.819100e-2  7.819909e-2      7.819100e-2   (+0.010 % / +0.000 %)
//! ```
//!
//! So the answer to that hypothesis is **no, not in magnitude**: at MeV energies
//! the Doppler width is negligible against the structure scale, and the
//! unbounded kernel moves the curve by 0.07 % at worst. It *is* structurally
//! wrong — it leaks 1.0e-6 b **below a threshold**, where the cross section must
//! be exactly zero — but that is not a reactivity effect. The damage is confined
//! to the 20 keV seam.
//!
//! # The counter-example is kept executable
//!
//! The gate at the bottom asserts that the unbounded kernel **still fails**.
//! Deleting it would leave the fix with nothing demonstrating what it fixed.
use njoy_outram_park_fork::broadr::{broaden_result, broadening_limit, doppler_broaden};
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{eval_lin_lin, reconr, ReconrConfig, ReconrSection};

fn find(secs: &[ReconrSection], mt: i32) -> &ReconrSection {
    secs.iter()
        .find(|s| i32::from(s.mt) == mt)
        .unwrap_or_else(|| panic!("no MT={mt}"))
}

fn dump(tag: &str, pairs: &[(f64, f64)], lo: f64, hi: f64, raw: &[(f64, f64)], max: usize) {
    println!("\n-- {tag} -- points in [{lo:.4e}, {hi:.4e}]  (rel. to tape MF=3 lin-lin):");
    let mut n = 0;
    for &(e, s) in pairs {
        if (lo..=hi).contains(&e) {
            let r = eval_lin_lin(raw, e);
            let rel = if r != 0.0 {
                (s - r) / r * 100.0
            } else {
                f64::NAN
            };
            println!("   E = {e:>14.8e}   sigma = {s:>12.6e} b   tape {r:>12.6e}   {rel:>+8.3}%");
            n += 1;
            if n >= max {
                println!("   ...");
                break;
            }
        }
    }
    if n == 0 {
        println!("   (none)");
    }
}

fn main() {
    let path = std::path::Path::new("reference-data/endf/n-092_U_238.endf");
    let tape = Tape::read_file(path).unwrap();
    let mat = tape.materials()[0];
    let raw = |mt: i32| -> Vec<(f64, f64)> {
        let sec = tape.section(mat, 3, mt).unwrap();
        let mut cur = SectionCursor::new(&sec.rows);
        cur.read_cont().unwrap();
        cur.read_tab1().unwrap().pairs
    };
    let raw102 = raw(102);
    let raw18 = raw(18);
    let raw16 = raw(16);

    let t0 = std::time::Instant::now();
    let r = reconr(
        &tape,
        &ReconrConfig {
            mat,
            tolerance: 1e-3,
            temperature: 0.0,
        },
    )
    .unwrap();
    println!(
        "RECONR: {} s; resonance_upper_limit = {:?}; thnmax = {:.6e} eV",
        t0.elapsed().as_secs_f64(),
        r.resonance_upper_limit,
        broadening_limit(&r)
    );

    let t0 = std::time::Instant::now();
    let unb = doppler_broaden(&r.sections, r.material.awr, 600.0);
    println!("unbounded BROADR 600 K: {} s", t0.elapsed().as_secs_f64());
    let t0 = std::time::Instant::now();
    let bnd = broaden_result(&r, 600.0).sections;
    println!("bounded   BROADR 600 K: {} s", t0.elapsed().as_secs_f64());

    println!("\n=============== MT=102 at the resolved/unresolved seam ===============");
    dump(
        "RECONR 0 K",
        &find(&r.sections, 102).pairs,
        1.9999e4,
        2.45e4,
        &raw102,
        14,
    );
    dump(
        "UNBOUNDED BROADR 600 K (pre-fix behaviour)",
        &find(&unb, 102).pairs,
        1.9999e4,
        2.45e4,
        &raw102,
        14,
    );
    dump(
        "BOUNDED BROADR 600 K (thnmax)",
        &find(&bnd, 102).pairs,
        1.9999e4,
        2.45e4,
        &raw102,
        14,
    );
    for e in [2.0e4, 2.05e4, 2.1e4, 2.2e4, 2.3e4] {
        println!(
            "   interp at {e:.3e}: tape {:.5}  reconr {:.5}  unbounded {:.5}  bounded {:.5}",
            eval_lin_lin(&raw102, e),
            eval_lin_lin(&find(&r.sections, 102).pairs, e),
            eval_lin_lin(&find(&unb, 102).pairs, e),
            eval_lin_lin(&find(&bnd, 102).pairs, e)
        );
    }

    println!("\n=============== MT=18 fission rise, 0.8-2.0 MeV ===============");
    dump(
        "RECONR 0 K",
        &find(&r.sections, 18).pairs,
        8.0e5,
        2.0e6,
        &raw18,
        40,
    );
    dump(
        "UNBOUNDED BROADR 600 K (pre-fix behaviour)",
        &find(&unb, 18).pairs,
        8.0e5,
        2.0e6,
        &raw18,
        40,
    );
    dump(
        "BOUNDED BROADR 600 K (thnmax)",
        &find(&bnd, 18).pairs,
        8.0e5,
        2.0e6,
        &raw18,
        40,
    );
    // Worst relative deviation of the unbounded kernel over the whole MT=18 grid above 20 keV
    let mut worst = (0.0f64, 0.0f64, 0.0f64);
    for &(e, s) in &find(&unb, 18).pairs {
        if e > 2.0e4 {
            let t = eval_lin_lin(&raw18, e);
            if t > 0.0 {
                let rel = ((s - t) / t).abs();
                if rel > worst.0 {
                    worst = (rel, e, t);
                }
            }
        }
    }
    println!(
        "\n   unbounded MT=18 worst |rel dev| above 20 keV: {:.3e} at E={:.6e} (tape {:.6e})",
        worst.0, worst.1, worst.2
    );

    println!("\n=============== MT=16 (n,2n) threshold 6.179 MeV ===============");
    dump(
        "RECONR 0 K",
        &find(&r.sections, 16).pairs,
        6.1e6,
        6.6e6,
        &raw16,
        12,
    );
    dump(
        "UNBOUNDED BROADR 600 K (pre-fix behaviour)",
        &find(&unb, 16).pairs,
        6.1e6,
        6.6e6,
        &raw16,
        12,
    );
    dump(
        "BOUNDED BROADR 600 K (thnmax)",
        &find(&bnd, 16).pairs,
        6.1e6,
        6.6e6,
        &raw16,
        12,
    );

    vv_gate(&r, &unb, &bnd, &raw102, &raw18, &raw16, worst.0);
}

/// U-238's resolved/unresolved boundary in ENDF/B-VIII.0, eV. BROADR must stop
/// here — this is `thnmax`.
const SEAM_EV: f64 = 2.0e4;

/// U-238's (n,2n) threshold, eV. The cross section is exactly zero below it.
const N2N_THRESHOLD_EV: f64 = 6.179_071e6;

/// V&V gate: the bounded kernel reproduces the tape across the seam, and the
/// unbounded one still does not.
///
/// # The oracle
///
/// The evaluation's own MF=3, read straight off the same tape RECONR consumed.
/// Not another code — the file this crate is processing *from*, so agreement is
/// a statement about the processing and nothing else.
///
/// # What each claim is for
///
/// 1. **`thnmax` sits at the resolved-resonance upper limit.** Structural, and
///    the root of everything else: if the limit moves, the rest of this gate is
///    measuring a different configuration.
/// 2. **Bounded BROADR reproduces MF=3 above the seam**, to 0.01 %. The fix.
/// 3. **Unbounded BROADR does NOT** — at least 20 % low just above the seam.
///    The counter-example, asserted so it stays reproducible. If this ever
///    passes, either `doppler_broaden` has silently acquired the bound (in which
///    case the two entry points are no longer distinct and this program has
///    nothing left to compare) or the defect has been re-introduced elsewhere.
/// 4. **The (n,2n) threshold is exact under the bounded kernel and leaks under
///    the unbounded one.** A threshold is a hard kinematic bound; a nonzero
///    cross section below it is wrong regardless of magnitude, and no percentage
///    envelope would catch 1e-6 b against an oracle of exactly zero.
///
/// Results, tolerances and the interpretation are in this file's module docs.
fn vv_gate(
    r: &njoy_outram_park_fork::reconr::ReconrResult,
    unb: &[ReconrSection],
    bnd: &[ReconrSection],
    raw102: &[(f64, f64)],
    raw18: &[(f64, f64)],
    raw16: &[(f64, f64)],
    unbounded_mt18_worst: f64,
) {
    use njoy_outram_park_fork::vv::assert_relative;

    println!("\n=== V&V gate: BROADR bounded at thnmax vs the tape's own MF=3 ===");

    // 1. thnmax is the resolved-resonance upper limit.
    let thnmax = broadening_limit(r);
    assert_eq!(
        r.resonance_upper_limit,
        Some(SEAM_EV),
        "U-238's resolved-resonance upper limit is no longer {SEAM_EV:e} eV. \
         Everything below is measured relative to that boundary."
    );
    assert!(
        (thnmax - SEAM_EV).abs() < 1.0,
        "thnmax = {thnmax:e} eV, not the resolved-resonance upper limit \
         {SEAM_EV:e} eV. NJOY bounds BROADR there and never runs SIGMA1 across \
         the resolved/unresolved boundary; a thnmax anywhere else means this \
         port has stopped matching upstream's control flow, which is exactly the \
         defect this program was written to find."
    );
    println!("  [PASS] thnmax = {thnmax:e} eV, at the resolved-resonance upper limit");

    // 2 and 3. Above the seam: bounded must match, unbounded must not.
    let probe = [2.000_001e4_f64, 2.05e4, 2.10e4, 2.20e4, 2.30e4];
    let mut worst_bounded = 0.0_f64;
    let mut worst_unbounded_shortfall = 0.0_f64;
    for &e in &probe {
        let tape = eval_lin_lin(raw102, e);
        let b = eval_lin_lin(&find(bnd, 102).pairs, e);
        let u = eval_lin_lin(&find(unb, 102).pairs, e);
        assert!(tape > 0.0, "tape MF=3/MT=102 is {tape} at {e:e} eV");
        worst_bounded = worst_bounded.max(((b - tape) / tape).abs());
        worst_unbounded_shortfall = worst_unbounded_shortfall.max((tape - u) / tape);
        println!(
            "    {e:>12.6e}  tape {tape:.6}  bounded {b:.6} ({:+.3} %)  \
             unbounded {u:.6} ({:+.2} %)",
            100.0 * (b / tape - 1.0),
            100.0 * (u / tape - 1.0),
        );
    }
    assert!(
        worst_bounded < 1.0e-4,
        "bounded BROADR deviates from the tape's own MF=3 by up to {:.3} % above \
         the {SEAM_EV:e} eV seam. It must reproduce it to round-off: above thnmax \
         BROADR does nothing at all, so the values there ARE the tape's, \
         unchanged. A departure means the bound has moved or is not being \
         applied.",
        100.0 * worst_bounded,
    );
    println!(
        "  [PASS] bounded BROADR reproduces MF=3 above the seam (worst {:.4} %)",
        100.0 * worst_bounded
    );

    assert!(
        worst_unbounded_shortfall > 0.20,
        "the UNBOUNDED kernel is now only {:.1} % below the tape above the seam. \
         It should be ~46.7 % low at {:.6e} eV, because broadening across the \
         resolved/unresolved discontinuity smears resolved-resonance structure \
         into the unresolved range.\n\
         This assertion is a COUNTER-EXAMPLE, kept executable on purpose: it is \
         what demonstrates that bounding BROADR at thnmax fixes something. If \
         `doppler_broaden` has quietly acquired the bound, the two entry points \
         are no longer distinct and this program has nothing left to compare.",
        100.0 * worst_unbounded_shortfall,
        probe[0],
    );
    println!(
        "  [PASS] unbounded BROADR still loses {:.1} % at the seam — the \
         counter-example is reproducible",
        100.0 * worst_unbounded_shortfall
    );

    // MT=18's own worst, measured over the whole grid above the seam by main().
    assert!(
        unbounded_mt18_worst > 0.10,
        "the unbounded kernel's worst MT=18 deviation above {SEAM_EV:e} eV is now \
         only {:.1} %; it was 19.6 %, at the seam. Same counter-example, fission \
         channel.",
        100.0 * unbounded_mt18_worst,
    );
    println!(
        "  [PASS] unbounded MT=18 worst deviation above the seam is {:.1} %",
        100.0 * unbounded_mt18_worst
    );

    // 4. The (n,2n) threshold. A hard kinematic bound, so exact zero or nothing.
    let b_thresh = eval_lin_lin(&find(bnd, 16).pairs, N2N_THRESHOLD_EV);
    let u_thresh = eval_lin_lin(&find(unb, 16).pairs, N2N_THRESHOLD_EV);
    assert_eq!(
        b_thresh, 0.0,
        "bounded BROADR gives sigma(n,2n) = {b_thresh:e} b at the \
         {N2N_THRESHOLD_EV:e} eV threshold. A reaction threshold is a hard \
         kinematic bound: the cross section is exactly zero below it, and no \
         percentage envelope would catch a leak this small against an oracle of \
         exactly zero."
    );
    assert!(
        u_thresh > 0.0,
        "unbounded BROADR no longer leaks below the (n,2n) threshold. Recorded: \
         1.008916e-6 b at {N2N_THRESHOLD_EV:e} eV. Counter-example again — the \
         leak is negligible in magnitude (the curve moves 0.07 % at 6.3 MeV) but \
         it is the structural signature of broadening something that must not be \
         broadened."
    );
    // And the magnitude claim that answers the fast-fission hypothesis.
    let tape_63 = eval_lin_lin(raw16, 6.3e6);
    assert_relative(
        "bounded BROADR leaves the (n,2n) curve at 6.3 MeV untouched",
        eval_lin_lin(&find(bnd, 16).pairs, 6.3e6),
        tape_63,
        1.0e-6,
    );
    assert!(
        (eval_lin_lin(&find(unb, 16).pairs, 6.3e6) / tape_63 - 1.0).abs() < 0.01,
        "the unbounded kernel now moves the (n,2n) curve at 6.3 MeV by more than \
         1 %. Recorded +0.070 %, which is what makes the fast-fission reading of \
         this defect a NON-effect in magnitude — the damage is at the 20 keV \
         seam, not at MeV thresholds. A larger number here would change that \
         conclusion."
    );
    println!(
        "  [PASS] (n,2n) threshold: bounded is exactly zero below it, unbounded \
         leaks {u_thresh:.3e} b — structurally wrong, negligible in magnitude"
    );
}
