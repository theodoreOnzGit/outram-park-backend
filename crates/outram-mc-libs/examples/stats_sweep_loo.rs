//! **#499 V&V runner: leave-one-out validation of a k-vs-height surrogate on
//! the committed HTR-10 sweep.** No transport is run — it reads
//! `crates/nee_soon/verification_and_validation/htr10_seker_2026_10_07_10k/results_table.csv`
//! (22 full runs: 11 loading heights × ENDF/B-VIII.0 and VII.0, 10 000
//! histories × [5 + 135], on the bounded delta-tracking majorant of gh:#589).
//! (~~`htr10_seker_2026_10_01_10k/results_table.csv`~~ **CORRECTED
//! 2026-10-07** (gh:#782): that record was measured on a majorant under-bound
//! 14× at 661 eV and is superseded by the 2026-10-07 one, same statistics and
//! columns.)
//!
//! # Methodology (fixed in advance, 2026-10-03)
//!
//! - One surrogate per library (the library is categorical, not a polynomial
//!   variable). Input: `built_height_cm`; output: `k`; per-run `σ`: `sigma`.
//! - **Degree 2 is the gated model**, chosen before seeing any result as the
//!   lowest order that can carry the curvature of a saturating `k(H)`.
//!   Degrees 1 and 3 are printed as a degree study, labelled as such; they
//!   are not alternative gates.
//! - Gate (#499): leave-one-out error within the per-run MC `σ`. Printed in
//!   both readings `SweepSurrogate::loo_gate` provides — literal
//!   (`loo_rmse ≤ rms σ`, expected to fail even for a perfect surrogate,
//!   because each held-out residual carries that run's own noise) and
//!   noise-floor-aware. Which reading the gate means is the maintainer's call.
//! - Also printed: jackknife+ intervals (`α = 0.1`, coverage ≥ 80 %) at the
//!   mid-points between runs, labelled as surrogate values — never as
//!   transport results — and three proposed next heights.
//!
//! # Results
//!
//! ~~**NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**~~
//! **CORRECTED 2026-10-07** (gh:#782): run once, on the 2026-10-07 record
//! (worktree of `develop` at `065bd710`, release build). No result was ever
//! recorded on the superseded 2026-10-01 CSV.
//!
//! | library | degree | LOO RMSE [pcm] | rms σ [pcm] | literal | χ²/pt | floor | χ²/pt ÷ floor | floor-aware |
//! |---|---|---|---|---|---|---|---|---|
//! | VIII.0 | 1 (study) | 1418 | 107 | FAIL | 171.90 | 1.22 | 141 | FAIL |
//! | VIII.0 | **2 (gated)** | **352** | 107 | **FAIL** | 10.94 | 1.38 | 7.9 | **FAIL** |
//! | VIII.0 | 3 (study) | 168 | 107 | FAIL | 2.50 | 1.57 | 1.6 | PASS |
//! | VII.0 | 1 (study) | 1567 | 106 | FAIL | 228.10 | 1.22 | 187 | FAIL |
//! | VII.0 | **2 (gated)** | **330** | 106 | **FAIL** | 10.39 | 1.38 | 7.5 | **FAIL** |
//! | VII.0 | 3 (study) | 148 | 106 | FAIL | 2.16 | 1.57 | 1.4 | PASS |
//!
//! **The gated degree-2 surrogate fails #499's gate on both libraries, in
//! both readings:** its LOO error is 3.1–3.3× the per-run σ, and χ²/point is
//! about 7.5–7.9× the noise floor, far outside the 95 % band. The miss is not
//! the literal reading's built-in excess (that is ≈ 1.15 σ). A quadratic in
//! height does not carry k(H) to MC precision on these runs. Degree 3 passes
//! the floor-aware reading on both, but it is a degree study, not an
//! alternative gate: switching the gated degree after seeing this would be
//! choosing the instrument after the result. Jackknife+ intervals and
//! proposals are printed by the runner; they are surrogate values, not
//! transport results, and are not recorded here.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example stats_sweep_loo
//! ```

#[cfg(target_os = "android")]
fn main() {
    println!("stats_sweep_loo reads a workspace CSV; desktop only.");
}

#[cfg(not(target_os = "android"))]
fn main() {
    use outram_mc_libs::stats::sweep::{SweepRun, SweepSurrogate};
    use std::path::Path;

    const GATED_DEGREE: usize = 2;
    let csv = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../nee_soon/verification_and_validation/htr10_seker_2026_10_07_10k/results_table.csv");
    let text = match std::fs::read_to_string(&csv) {
        Ok(t) => t,
        Err(e) => {
            println!("cannot read {}: {e}", csv.display());
            return;
        }
    };
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or("").split(',').collect();
    let col = |name: &str| {
        header
            .iter()
            .position(|h| *h == name)
            .unwrap_or_else(|| panic!("column {name} missing from {}", csv.display()))
    };
    let (c_lib, c_h, c_k, c_s) = (col("library"), col("built_height_cm"), col("k"), col("sigma"));
    let mut by_lib: Vec<(String, Vec<SweepRun>)> = Vec::new();
    for line in lines.filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split(',').collect();
        let parse = |i: usize| f[i].trim().parse::<f64>().expect("numeric field");
        let run = SweepRun {
            x: vec![parse(c_h)],
            value: parse(c_k),
            sigma: parse(c_s),
        };
        match by_lib.iter_mut().find(|(l, _)| l == f[c_lib]) {
            Some((_, v)) => v.push(run),
            None => by_lib.push((f[c_lib].to_string(), vec![run])),
        }
    }

    println!("#499 leave-one-out validation, {}", csv.display());
    for (lib, runs) in &by_lib {
        println!("\nlibrary {lib}: {} full runs", runs.len());
        for degree in [1, GATED_DEGREE, 3] {
            let tag = if degree == GATED_DEGREE { "GATED" } else { "degree study" };
            let s = match SweepSurrogate::fit(runs.clone(), degree, 0.0) {
                Ok(s) => s,
                Err(e) => {
                    println!("  degree {degree}: fit failed: {e}");
                    continue;
                }
            };
            let g = s.loo_gate();
            println!(
                "  degree {degree} [{tag}]: LOO rmse {:.0} pcm vs rms sigma {:.0} pcm -> literal {}; \
                 chi2/pt {:.2} vs noise floor {:.2} -> floor-aware {}",
                g.loo_rmse * 1e5,
                g.rms_sigma * 1e5,
                if g.literal_pass { "PASS" } else { "FAIL" },
                g.chi2_per_point,
                g.noise_floor,
                if g.floor_aware_pass { "PASS" } else { "FAIL" }
            );
            if degree != GATED_DEGREE {
                continue;
            }
            let mut hs: Vec<f64> = runs.iter().map(|r| r.x[0]).collect();
            hs.sort_by(f64::total_cmp);
            for w in hs.windows(2) {
                let mid = 0.5 * (w[0] + w[1]);
                if let Ok(p) = s.predict(&[mid], 0.1) {
                    println!(
                        "    SURROGATE (not a transport result) H {mid:.1} cm: k {:.5}, \
                         jackknife+ 80 % [{:.5}, {:.5}]",
                        p.value, p.interval.0, p.interval.1
                    );
                }
            }
            let cands: Vec<Vec<f64>> = (0..=200)
                .map(|i| vec![hs[0] + (hs[hs.len() - 1] - hs[0]) * i as f64 / 200.0])
                .collect();
            let picks = s.propose_next_runs(&cands, 3);
            let ph: Vec<String> = picks.iter().map(|&i| format!("{:.1}", cands[i][0])).collect();
            println!("    proposed next full runs at H = {} cm", ph.join(", "));
        }
    }
}
