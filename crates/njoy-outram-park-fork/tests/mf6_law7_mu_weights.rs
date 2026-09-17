//! **MF=6 LAW=7: the per-cosine tables carry an angular distribution, and the
//! parser used to throw it away.**
//!
//! # The defect this pins
//!
//! `LAW=7` stores, at each incident energy, a lab-cosine grid and — per cosine —
//! a tabulated outgoing-energy spectrum `f(mu, E')`. The *relative* size of
//! those per-cosine tables **is** the angular distribution `f(mu)`; there is no
//! separate angular record.
//!
//! Until 2026-09-16 `parse_law7_lab_angle_energy_body` pushed each table through
//! `normalize_pdf_cdf`, which scales every table to unit area and discarded the
//! integral it divided out. Every cosine therefore came back looking equally
//! likely, so any consumer built on the parsed structure would have sampled `mu`
//! **uniformly** — the entire angular distribution silently flattened. Same
//! class as `op-og56` (MF=6 `LANG` coefficients dropped at parse time).
//!
//! `Law7MuTable::weight` now retains that integral. This test asserts it is
//! **non-uniform**, which is the falsifiable statement: if the weights were all
//! equal there would have been nothing to lose and the fix would be pointless.
//!
//! # Methodology
//!
//! Scan every tape in `reference-data/endf/` for an MF=6 section carrying a
//! `ZAP=1, LAW=7` neutron subsection, parse it, and for each incident energy
//! measure the spread of the per-cosine weights,
//!
//! ```text
//! spread = (max(w) - min(w)) / mean(w)
//! ```
//!
//! Pass criterion: at least one tape carries LAW=7 (otherwise this test is
//! vacuous and says so), and on it the worst-case spread over incident energies
//! exceeds 0.10 — i.e. the weights differ by more than 10 % of their mean, far
//! outside anything a rounding artefact could produce.
//!
//! # Results
//!
//! Measured 2026-09-16 on ENDF/B-VIII.0: **Be-9 MT=16** is the only LAW=7
//! neutron subsection in `reference-data/endf/` — 24 incident energies, worst
//! per-cosine weight spread **`(max-min)/mean = 3.67`**. The most-favoured
//! cosine is several times more likely than the least, so the normalisation the
//! parser used to discard was carrying a strongly anisotropic distribution, not
//! a constant.
//!
//! # A second, larger defect this test found
//!
//! Writing it surfaced that the LAW=7 body was reading the **wrong record type**
//! and had never parsed anything at all. `parse_law7_lab_angle_energy_body`
//! followed `acefc.f90`'s `acelf6`, which reads the per-incident-energy record
//! as a `TAB1` with `INTMU = L1`, `NMU = L2`. That is right for ACER, which runs
//! on NJOY's *own intermediate* File 6 (`skip6a`'s header comment says so
//! explicitly), and wrong for a genuine ENDF-6 tape, where the record is a
//! `TAB2` carrying `NMU` in `N2`. On Be-9 it read `L1 = L2 = 0`, built **zero
//! cosine tables**, and consumed the real data as the TAB1's own pairs. The
//! reader now follows `groupr.f90`'s `getmf6`, which is NJOY's reader for real
//! evaluation tapes. The `law7_mu_grid_is_well_formed` case below is the
//! regression gate for that.
//!
//! # The strongest evidence here: the evaluation's own normalisation
//!
//! ENDF-102 normalises LAW=7 so that the double integral of `f(mu, E')` over
//! both variables is 1. Each retained weight is the inner integral, so
//! integrating the weights across the cosine grid must give **1** — a condition
//! written by the evaluator, not by this port. Measured on Be-9 MT=16 at all 24
//! incident energies: **1.000000 to 1.000001**, i.e. within `1e-6`. Two things
//! follow, neither of which the weight-spread assertion alone could establish:
//! the TAB2 records are being read at the right offsets, and `weight` really is
//! `f(mu)` rather than some proxy proportional to it. The gate is set at `1e-3`
//! to leave room for a coarser evaluation without ceasing to be a real check.
//!
//! # What this does NOT claim
//!
//! **LAW=7 sampling is still unported.** `ContinuumEmission::from_endf_mf6`
//! continues to fall back for it (`secondary.rs`, "LAW=7 and anything else stay
//! unported"), so no transport path consumes these tables yet. This test fixes
//! the *parse* so that whoever ports the sampler is not building on flattened
//! data — it is not a claim that Be-9 (n,2n) is modelled correctly today.

use njoy_outram_park_fork::acer::energy::parse_mf6_law7_lab_angle_energy;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::{reference_data_dir, reference_endf_or_skip};

/// `(max - min) / mean` over a slice; `0.0` for fewer than two entries.
fn spread(w: &[f64]) -> f64 {
    if w.len() < 2 {
        return 0.0;
    }
    let mean = w.iter().sum::<f64>() / w.len() as f64;
    if mean <= 0.0 {
        return 0.0;
    }
    let max = w.iter().cloned().fold(f64::MIN, f64::max);
    let min = w.iter().cloned().fold(f64::MAX, f64::min);
    (max - min) / mean
}

#[test]
fn law7_per_cosine_weights_are_not_uniform() {
    let dir = reference_data_dir("endf");
    if !dir.is_dir() {
        eprintln!("SKIP: reference-data/endf/ not present");
        return;
    }

    let mut tapes: Vec<_> = std::fs::read_dir(&dir)
        .expect("reference-data/endf/ is readable")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "endf"))
        .collect();
    tapes.sort();

    // (tape label, MT, worst spread, number of incident energies)
    let mut found: Vec<(String, i32, f64, usize)> = Vec::new();

    for path in &tapes {
        let Ok(tape) = Tape::read_file(path) else {
            continue;
        };
        let label = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        {
            for sec in tape.sections() {
                if sec.key.mf != 6 {
                    continue;
                }
                let Ok(law7) = parse_mf6_law7_lab_angle_energy(sec) else {
                    continue; // not a LAW=7 neutron subsection
                };
                let mut worst = 0.0f64;
                for inc in &law7.incident {
                    let w: Vec<f64> = inc.tables.iter().map(|t| t.weight).collect();
                    worst = worst.max(spread(&w));
                }
                found.push((label.clone(), sec.key.mt, worst, law7.incident.len()));
            }
        }
    }

    println!("MF=6 LAW=7 (lab angle-energy) neutron subsections in reference-data/endf/:");
    for (label, mt, worst, ne) in &found {
        println!(
            "   {label} MT={mt}: {ne} incident energies, worst per-cosine weight spread \
             (max-min)/mean = {worst:.4}",
        );
    }

    assert!(
        !found.is_empty(),
        "no MF=6 LAW=7 neutron subsection found in reference-data/endf/ — this test is \
         vacuous. The interpolation survey recorded Be-9 MT=16 as LAW=7 on 2026-09-16; if \
         that tape is gone the survey note in mf6.rs is stale."
    );

    let best = found
        .iter()
        .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap())
        .unwrap();
    assert!(
        best.2 > 0.10,
        "every LAW=7 per-cosine weight came back within 10 % of the mean (worst spread \
         {:.4} on {} MT={}). Either the weights are not being retained (the pre-2026-09-16 \
         defect, where normalize_pdf_cdf's divisor was discarded and every cosine looked \
         equally likely), or this evaluation genuinely is near-isotropic and this test needs \
         a different one.",
        best.2,
        best.0,
        best.1
    );
}

/// The cosine grid the parser builds is well formed: one table per cosine,
/// ascending in `[-1, 1]`, and — the point of this case — **non-empty**.
///
/// # Why non-empty is the interesting assertion
///
/// The pre-2026-09-16 reader took `NMU` from the record's `L2`, which is `0` on
/// a real ENDF-6 tape (`NMU` lives in `N2` there). Every incident energy
/// therefore came back with an empty cosine grid, and every downstream `for mu
/// in …` loop did nothing. Nothing consumed LAW=7 at the time, so it failed
/// silently. This asserts it cannot go back to doing that.
#[test]
fn law7_mu_grid_is_well_formed() {
    let Some(p) = reference_endf_or_skip("n-004_Be_009-ENDF8.0.endf", "Be-9 (MF=6 LAW=7)") else {
        return;
    };
    let tape = Tape::read_file(&p).expect("Be-9 tape parses");
    let mat = tape.materials()[0];
    let sec = tape.section(mat, 6, 16).expect("Be-9 has MF=6 MT=16");
    let law7 = parse_mf6_law7_lab_angle_energy(&sec).expect("Be-9 MT=16 is ZAP=1 LAW=7");

    assert!(
        !law7.incident.is_empty(),
        "LAW=7 parsed with no incident energies at all"
    );
    for inc in &law7.incident {
        assert_eq!(
            inc.mu.len(),
            inc.tables.len(),
            "cosine grid and table list disagree at E_in = {} MeV",
            inc.e_in_mev
        );
        assert!(
            inc.mu.len() >= 2,
            "E_in = {} MeV carries {} cosine(s). An empty or single-point grid is what the \
             TAB1/TAB2 record-type confusion produced (NMU read from L2 = 0 instead of N2).",
            inc.e_in_mev,
            inc.mu.len()
        );
        assert!(
            inc.mu.windows(2).all(|w| w[1] > w[0]),
            "cosine grid not strictly ascending at E_in = {} MeV: {:?}",
            inc.e_in_mev,
            inc.mu
        );
        assert!(
            inc.mu[0] >= -1.0 && *inc.mu.last().unwrap() <= 1.0,
            "cosine grid leaves [-1, 1] at E_in = {} MeV: {:?}",
            inc.e_in_mev,
            inc.mu
        );
        for t in &inc.tables {
            assert!(
                t.weight >= 0.0 && t.weight.is_finite(),
                "negative or non-finite per-cosine weight {} at E_in = {} MeV",
                t.weight,
                inc.e_in_mev
            );
        }
        // ENDF-102 normalises LAW=7 so that the double integral over mu and
        // E' is 1. Each retained weight is the inner integral, i.e. f(mu), so
        // integrating the weights over the cosine grid must give 1. This is the
        // check that the retained number really IS the angular density and not
        // some scaled proxy -- it is the evaluation's own condition, not ours.
        let w: Vec<f64> = inc.tables.iter().map(|t| t.weight).collect();
        let norm: f64 = inc
            .mu
            .windows(2)
            .zip(w.windows(2))
            .map(|(m, f)| 0.5 * (f[0] + f[1]) * (m[1] - m[0]))
            .sum();
        println!(
            "   E_in = {:10.5} MeV: {:3} cosines, integral f(mu) dmu = {:.6}",
            inc.e_in_mev,
            inc.mu.len(),
            norm
        );
        assert!(
            (norm - 1.0).abs() < 1.0e-3,
            "LAW=7 angular weights at E_in = {} MeV integrate to {norm:.6}, not 1. ENDF-102 \
             normalises this law so that the double integral over mu and E' is unity, and each \
             weight is the inner integral f(mu). A value far from 1 means the retained weight \
             is not f(mu).",
            inc.e_in_mev
        );
    }
}
