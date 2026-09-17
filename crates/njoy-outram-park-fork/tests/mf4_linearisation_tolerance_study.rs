//! **Pricing `ANGLE_TOL`: how much `<mu>` error does the MF=4 lineariser leave,
//! and is it worth more grid points?**
//!
//! # Why this study exists
//!
//! `tests/mf4_mean_cosine_vs_legendre_a1.rs` measures `|mean_cosine - a1|` and
//! finds a residual that is **not** the quadrature (that was fixed 2026-09-16)
//! and **not** the positivity clamp (zero rows clamp). It is
//! `legendre_cosine_law`'s adaptive bisection, which stops when lin-lin
//! interpolation reproduces `f` within `ANGLE_TOL = 5e-3` **relative to the local
//! `f`**. A tolerance stated on `f` leaves a residual in a *moment* of `f`, and
//! that residual was flagged rather than fixed, on the grounds that changing a
//! lineariser tolerance moves every angular table in the workspace and deserves
//! its own measurement.
//!
//! This is that measurement. It reimplements the bisection locally so the
//! criterion itself can be varied — the library keeps exactly one lineariser.
//!
//! # A hypothesis that measurement killed
//!
//! The obvious diagnosis was that a **relative-to-local-`f`** criterion is wrong
//! for a moment: where `f` is largest (the forward peak, which dominates
//! `<mu>`) it permits the largest *absolute* error. Scaling to the peak instead
//! should therefore do better.
//!
//! **It is far worse** — 1.46e-1 against 6.21e-3. Scaling to the peak loosens
//! the tolerance in the *tails*, and the tails act on `<mu>` through a long
//! lever arm in `mu`. The plausible mechanism was the wrong one, and the only
//! reason that is known is that it was tried rather than argued.
//!
//! # Results (2026-09-16, 1857 anisotropic Legendre rows, U-235 + U-238,
//! MT=2 and MT=51..60)
//!
//! | criterion | tol | worst | worst <6 MeV | **Watt-weighted** | mean pts/row |
//! |---|---|---|---|---|---|
//! | rel. local (current) | 5e-3 | 6.21e-3 | 1.20e-3 | **1.44e-4** | 43.8 |
//! | rel. local | 1e-3 | 3.95e-4 | 2.18e-4 | **2.90e-5** | 95.7 |
//! | rel. local, depth 24 | 5e-3 | 6.21e-3 | 1.20e-3 | 1.44e-4 | 43.8 |
//! | rel. peak | 5e-3 | 1.46e-1 | 1.14e-2 | 3.07e-4 | 21.5 |
//! | rel. peak | 1e-3 | 1.47e-1 | 4.44e-3 | 8.11e-5 | 44.9 |
//! | rel. peak | 5e-4 | 2.32e-2 | 2.09e-3 | 3.82e-5 | 63.1 |
//!
//! `MAX_DEPTH` is **not** binding: raising it 20 -> 24 changes nothing.
//!
//! # The decision, and the number behind it
//!
//! **`ANGLE_TOL` stays at 5e-3.** The Watt-weighted error — each row weighted by
//! a fission spectrum at its own incident energy, i.e. what a neutron in a
//! reactor actually meets, rather than the worst row anywhere — is **1.44e-4**.
//! `<mu>` enters transport as `Sigma_tr = Sigma_t (1 - <mu>)`, so at
//! `<mu> ~ 0.26` that is a **0.019 %** error in `Sigma_tr`. On Godiva, 55.8 %
//! leakage, that is worth well under 1 pcm — below anything the 256-seed
//! ensemble (sem 11 pcm) could resolve.
//!
//! Tightening to 1e-3 would buy 1.44e-4 -> 2.90e-5 for **2.2x the grid points**
//! in every angular table in the workspace. That is paying real memory and a
//! perturbation of every angular sample for a sub-pcm effect.
//!
//! **The caveat that goes with the decision.** The 6.21e-3 worst case is real
//! and sits at **13-28 MeV** on discrete inelastic levels. A fission spectrum
//! barely reaches there, which is the whole reason the weighted number is 43x
//! smaller. **An application that lives at those energies — fusion neutronics,
//! deep-penetration shielding above 10 MeV — should tighten `ANGLE_TOL` and pay
//! the table size.** The default is chosen for this workspace's reactor cases,
//! not universally.
//!
//! # What this asserts
//!
//! The Watt-weighted error at the shipped setting, so the decision above cannot
//! silently stop being true, and that the rel-peak criterion really is worse (if
//! it ever became better, the reasoning here would need revisiting).

use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;

fn legendre_pdf(coeffs: &[f64], mu: f64) -> f64 {
    let mut p_prev = 1.0f64;
    let mut p_cur = mu;
    let mut acc = 0.5;
    for (i, &a) in coeffs.iter().enumerate() {
        let l = (i + 1) as f64;
        acc += (2.0 * l + 1.0) / 2.0 * a * p_cur;
        let p_next = ((2.0 * l + 1.0) * mu * p_cur - l * p_prev) / (l + 1.0);
        p_prev = p_cur;
        p_cur = p_next;
    }
    acc
}

#[derive(Clone, Copy, Debug)]
enum Crit {
    /// Current: |f_lin - f_m| > tol * max(|f_m|, 1e-10)
    RelLocal,
    /// |f_lin - f_m| > tol * peak, with peak = max |f| over the seed grid
    RelPeak,
}

fn refine(
    coeffs: &[f64],
    a: f64,
    fa: f64,
    b: f64,
    fb: f64,
    depth: u32,
    out: &mut Vec<(f64, f64)>,
    tol: f64,
    crit: Crit,
    peak: f64,
    max_depth: u32,
) {
    if depth >= max_depth {
        return;
    }
    let m = 0.5 * (a + b);
    if m <= a || m >= b {
        return;
    }
    let fm = legendre_pdf(coeffs, m);
    let f_lin = 0.5 * (fa + fb);
    let scale = match crit {
        Crit::RelLocal => fm.abs().max(1.0e-10),
        Crit::RelPeak => peak.max(1.0e-10),
    };
    if (f_lin - fm).abs() > tol * scale {
        refine(
            coeffs,
            a,
            fa,
            m,
            fm,
            depth + 1,
            out,
            tol,
            crit,
            peak,
            max_depth,
        );
        out.push((m, fm));
        refine(
            coeffs,
            m,
            fm,
            b,
            fb,
            depth + 1,
            out,
            tol,
            crit,
            peak,
            max_depth,
        );
    }
}

fn tabulate(coeffs: &[f64], tol: f64, crit: Crit, max_depth: u32) -> (Vec<f64>, Vec<f64>) {
    const SEED: usize = 9;
    let seed: Vec<f64> = (0..SEED)
        .map(|i| -1.0 + 2.0 * i as f64 / (SEED - 1) as f64)
        .collect();
    let peak = (0..257)
        .map(|i| legendre_pdf(coeffs, -1.0 + 2.0 * i as f64 / 256.0).abs())
        .fold(0.0, f64::max);
    let mut out: Vec<(f64, f64)> = Vec::new();
    out.push((seed[0], legendre_pdf(coeffs, seed[0])));
    for w in seed.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (fa, fb) = (legendre_pdf(coeffs, a), legendre_pdf(coeffs, b));
        refine(
            coeffs, a, fa, b, fb, 0, &mut out, tol, crit, peak, max_depth,
        );
        out.push((b, fb));
    }
    let x: Vec<f64> = out.iter().map(|&(m, _)| m).collect();
    let mut y: Vec<f64> = out.iter().map(|&(_, f)| f.max(0.0)).collect();
    let area: f64 = (1..x.len())
        .map(|i| 0.5 * (y[i] + y[i - 1]) * (x[i] - x[i - 1]))
        .sum();
    if area > 0.0 {
        for v in &mut y {
            *v /= area;
        }
    }
    (x, y)
}

fn mubar_closed(x: &[f64], f: &[f64]) -> f64 {
    (1..x.len())
        .map(|i| {
            let (a, b) = (x[i - 1], x[i]);
            let (fa, fb) = (f[i - 1], f[i]);
            (b - a) * (a * (2.0 * fa + fb) + b * (fa + 2.0 * fb)) / 6.0
        })
        .sum()
}

#[test]
fn angle_tol_is_priced_and_the_shipped_setting_is_justified() {
    let mut rows: Vec<(Vec<f64>, f64, f64)> = Vec::new(); // (coeffs, a1, e_ev)
    for f in ["n-092_U_235-ENDF8.0.endf", "n-092_U_238-ENDF8.0.endf"] {
        let Some(p) = reference_endf_or_skip(f, "sweep") else {
            return;
        };
        let tape = Tape::read_file(&p).unwrap();
        let mat = tape.materials()[0];
        for mt in std::iter::once(2).chain(51..=60) {
            let Some(sec) = tape.section(mat, 4, mt) else {
                continue;
            };
            let mut cur = SectionCursor::new(&sec.rows);
            let head = cur.read_cont().unwrap();
            if head.l2 != 1 && head.l2 != 3 {
                continue;
            }
            let _ = cur.read_cont().unwrap();
            let tab2 = cur.read_tab2().unwrap();
            for _ in 0..tab2.head.n2 {
                let list = cur.read_list().unwrap();
                if let Some(&a1) = list.data.first() {
                    if list.data.iter().any(|&a| a != 0.0) {
                        rows.push((list.data.clone(), a1, list.head.c2));
                    }
                }
            }
        }
    }
    println!("{} anisotropic Legendre rows", rows.len());
    println!(
        "{:>10} {:>10} {:>8} {:>12} {:>12} {:>12} {:>10}",
        "criterion", "tol", "depth", "worst d<mu>", "worst<6MeV", "Watt-wtd", "mean pts"
    );
    let mut shipped_watt = -1.0f64;
    let mut relpeak_watt = -1.0f64;
    for (crit, tol, depth) in [
        (Crit::RelLocal, 5.0e-3, 20u32),
        (Crit::RelLocal, 1.0e-3, 20),
        (Crit::RelLocal, 5.0e-3, 24),
        (Crit::RelPeak, 5.0e-3, 20),
        (Crit::RelPeak, 1.0e-3, 20),
        (Crit::RelPeak, 5.0e-4, 20),
    ] {
        let mut worst = 0.0f64;
        let mut worst6 = 0.0f64;
        let mut pts = 0usize;
        // Watt-spectrum weight at the row's incident energy: what a fission
        // neutron actually sees, as opposed to the worst row anywhere.
        let watt = |e: f64| -> f64 {
            let a = 0.988e6;
            let b = 2.249e-6;
            (-e / a).exp() * (b * e).max(0.0).sqrt().sinh()
        };
        let (mut wsum, mut wacc) = (0.0f64, 0.0f64);
        for (c, a1, e) in &rows {
            let (x, y) = tabulate(c, tol, crit, depth);
            let d = (mubar_closed(&x, &y) - a1).abs();
            worst = worst.max(d);
            if *e <= 6.0e6 {
                worst6 = worst6.max(d);
            }
            let w = watt(*e);
            wsum += w;
            wacc += w * d;
            pts += x.len();
        }
        let wmean = if wsum > 0.0 { wacc / wsum } else { 0.0 };
        println!(
            "{:>10?} {:>10.0e} {:>8} {:>12.3e} {:>12.3e} {:>12.3e} {:>10.1}",
            crit,
            tol,
            depth,
            worst,
            worst6,
            wmean,
            pts as f64 / rows.len() as f64
        );
        if matches!(crit, Crit::RelLocal) && tol == 5.0e-3 && depth == 20 {
            shipped_watt = wmean;
        }
        if matches!(crit, Crit::RelPeak) && tol == 5.0e-3 {
            relpeak_watt = wmean;
        }
    }

    assert!(
        rows.len() > 1000,
        "only {} rows; the study is not exercised",
        rows.len()
    );
    // The shipped setting's fission-spectrum-weighted error. Measured 1.441e-4;
    // gated at 3e-4, which is still a 0.04 % error in Sigma_tr and therefore
    // still sub-pcm, so a modest drift stays acceptable while a real regression
    // in the lineariser does not.
    assert!(
        shipped_watt > 0.0 && shipped_watt < 3.0e-4,
        "Watt-weighted |<mu> - a1| at the shipped ANGLE_TOL is {shipped_watt:.3e}, outside the \
         3e-4 this study justified the setting with. Either the lineariser changed or the \
         reference set did; re-run the table above and revisit the decision rather than moving \
         this bound."
    );
    // The killed hypothesis, kept as an assertion so it stays killed.
    assert!(
        relpeak_watt > shipped_watt,
        "scaling the bisection tolerance to the distribution's peak now beats scaling it to the \
         local f ({relpeak_watt:.3e} vs {shipped_watt:.3e}). That reverses this study's finding \
         -- the peak-scaled criterion loosens the tolerance in the tails, which act on <mu> \
         through a long lever arm -- so the reasoning in the module docs needs redoing."
    );
}
