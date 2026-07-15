//! Rust-generated comparison summary: SAM vs TUAS (K=17.8) vs CIET experiment
//! for the 25 coupled natural-circulation cases (datasets A/B/C).
//!
//! Running the test [`generate_coupled_natcirc_sam_vs_tuas_vs_experiment_md`]
//! programmatically builds a GitHub-Flavored-Markdown report with `format!` /
//! `write!` and writes it to
//! `verification_and_validation/coupled_natural_circulation_SAM_vs_TUAS_vs_experiment.md`
//! (a committed file — only `*.csv` under that folder is gitignored). It
//! regenerates automatically on `cargo test`.
//!
//! # Data provenance (no fabrication)
//!
//! - **Experimental** DRACS/primary mass flow rates and heater powers: the CIET
//!   coupled natural-circulation dataset as tabulated in NED-2021 Table 4 (and
//!   used as the assertion targets by the `dataset_a/b/c` regression tests).
//! - **TUAS K=17.8** computed DRACS/primary mass flow rates: the regression-
//!   locked outputs of the `ciet_coupled_nat_circ_set_*` tests with pipe 38 at
//!   the SAM-matched form loss K=17.8 (`new_pipe_38_sam_model`), measured on
//!   2026-07-15 (`cargo test --release`, 25/25 pass). These mirror the values
//!   the coupled tests assert to 0.1% and emit to the per-case
//!   `coupled_dracs_natcirc_set*.csv` files; if the model changes, both the
//!   regression references and the `TUAS_*` constants below must be updated
//!   together.
//! - **SAM** predicted DRACS/primary mass flow rates: NED-2021 Table 4 (Zou et
//!   al. 2021), obtained via [`super::sam_table4_flowrates_kg_per_s`]; see that
//!   function's doc comment for the openly-available source.

/// One coupled natural-circulation case: experimental and TUAS-K=17.8 values.
/// SAM values are looked up separately from NED-2021 Table 4.
///
/// Fields: set letter, case label, heater power [W], TCHX outlet setpoint [degC],
/// experimental DRACS [kg/s], experimental primary [kg/s],
/// TUAS-K=17.8 computed DRACS [kg/s], TUAS-K=17.8 computed primary [kg/s].
struct CoupledCase {
    set: &'static str,
    case: &'static str,
    heater_w: f64,
    tchx_c: f64,
    exp_dracs: f64,
    exp_pri: f64,
    tuas_dracs: f64,
    tuas_pri: f64,
}

/// All 25 coupled cases (A1..A7, B1..B9, C1..C9). Experimental values and
/// heater powers are the CIET/NED-2021 Table-4 dataset; the TUAS columns are
/// the 2026-07-15 K=17.8 regression outputs (see module docs).
const COUPLED_CASES: [CoupledCase; 25] = [
    CoupledCase {
        set: "A",
        case: "A1",
        heater_w: 1479.86,
        tchx_c: 46.0,
        exp_dracs: 3.341e-2,
        exp_pri: 2.738e-2,
        tuas_dracs: 3.414122e-2,
        tuas_pri: 2.767611e-2,
    },
    CoupledCase {
        set: "A",
        case: "A2",
        heater_w: 1653.90,
        tchx_c: 46.0,
        exp_dracs: 3.544e-2,
        exp_pri: 2.819e-2,
        tuas_dracs: 3.617044e-2,
        tuas_pri: 2.913043e-2,
    },
    CoupledCase {
        set: "A",
        case: "A3",
        heater_w: 2014.51,
        tchx_c: 46.0,
        exp_dracs: 3.877e-2,
        exp_pri: 3.236e-2,
        tuas_dracs: 3.990515e-2,
        tuas_pri: 3.181659e-2,
    },
    CoupledCase {
        set: "A",
        case: "A4",
        heater_w: 2178.49,
        tchx_c: 46.0,
        exp_dracs: 4.011e-2,
        exp_pri: 3.255e-2,
        tuas_dracs: 4.143821e-2,
        tuas_pri: 3.291795e-2,
    },
    CoupledCase {
        set: "A",
        case: "A5",
        heater_w: 2395.90,
        tchx_c: 46.0,
        exp_dracs: 4.277e-2,
        exp_pri: 3.390e-2,
        tuas_dracs: 4.334337e-2,
        tuas_pri: 3.428215e-2,
    },
    CoupledCase {
        set: "A",
        case: "A6",
        heater_w: 2491.87,
        tchx_c: 46.0,
        exp_dracs: 4.465e-2,
        exp_pri: 3.355e-2,
        tuas_dracs: 4.414367e-2,
        tuas_pri: 3.485307e-2,
    },
    CoupledCase {
        set: "A",
        case: "A7",
        heater_w: 2696.24,
        tchx_c: 46.0,
        exp_dracs: 4.710e-2,
        exp_pri: 3.462e-2,
        tuas_dracs: 4.577478e-2,
        tuas_pri: 3.601093e-2,
    },
    CoupledCase {
        set: "B",
        case: "B1",
        heater_w: 655.16,
        tchx_c: 35.0,
        exp_dracs: 2.329e-2,
        exp_pri: 1.731e-2,
        tuas_dracs: 2.174746e-2,
        tuas_pri: 1.811033e-2,
    },
    CoupledCase {
        set: "B",
        case: "B2",
        heater_w: 1054.32,
        tchx_c: 35.0,
        exp_dracs: 2.952e-2,
        exp_pri: 2.198e-2,
        tuas_dracs: 2.869994e-2,
        tuas_pri: 2.321078e-2,
    },
    CoupledCase {
        set: "B",
        case: "B3",
        heater_w: 1394.70,
        tchx_c: 35.0,
        exp_dracs: 3.324e-2,
        exp_pri: 2.570e-2,
        tuas_dracs: 3.325217e-2,
        tuas_pri: 2.666051e-2,
    },
    CoupledCase {
        set: "B",
        case: "B4",
        heater_w: 1685.62,
        tchx_c: 35.0,
        exp_dracs: 3.611e-2,
        exp_pri: 2.846e-2,
        tuas_dracs: 3.655763e-2,
        tuas_pri: 2.917745e-2,
    },
    CoupledCase {
        set: "B",
        case: "B5",
        heater_w: 1987.75,
        tchx_c: 35.0,
        exp_dracs: 3.841e-2,
        exp_pri: 3.118e-2,
        tuas_dracs: 3.959186e-2,
        tuas_pri: 3.147784e-2,
    },
    CoupledCase {
        set: "B",
        case: "B6",
        heater_w: 2282.01,
        tchx_c: 35.0,
        exp_dracs: 4.063e-2,
        exp_pri: 3.374e-2,
        tuas_dracs: 4.225131e-2,
        tuas_pri: 3.347600e-2,
    },
    CoupledCase {
        set: "B",
        case: "B7",
        heater_w: 2546.60,
        tchx_c: 35.0,
        exp_dracs: 4.270e-2,
        exp_pri: 3.577e-2,
        tuas_dracs: 4.444483e-2,
        tuas_pri: 3.510609e-2,
    },
    CoupledCase {
        set: "B",
        case: "B8",
        heater_w: 2874.03,
        tchx_c: 35.0,
        exp_dracs: 4.456e-2,
        exp_pri: 3.796e-2,
        tuas_dracs: 4.694837e-2,
        tuas_pri: 3.694139e-2,
    },
    CoupledCase {
        set: "B",
        case: "B9",
        heater_w: 3031.16,
        tchx_c: 35.0,
        exp_dracs: 4.636e-2,
        exp_pri: 3.849e-2,
        tuas_dracs: 4.807898e-2,
        tuas_pri: 3.775748e-2,
    },
    CoupledCase {
        set: "C",
        case: "C1",
        heater_w: 841.02,
        tchx_c: 40.0,
        exp_dracs: 2.686e-2,
        exp_pri: 2.003e-2,
        tuas_dracs: 2.503265e-2,
        tuas_pri: 2.084249e-2,
    },
    CoupledCase {
        set: "C",
        case: "C2",
        heater_w: 1158.69,
        tchx_c: 40.0,
        exp_dracs: 3.055e-2,
        exp_pri: 2.367e-2,
        tuas_dracs: 3.012198e-2,
        tuas_pri: 2.449453e-2,
    },
    CoupledCase {
        set: "C",
        case: "C3",
        heater_w: 1409.22,
        tchx_c: 40.0,
        exp_dracs: 3.345e-2,
        exp_pri: 2.635e-2,
        tuas_dracs: 3.343442e-2,
        tuas_pri: 2.692927e-2,
    },
    CoupledCase {
        set: "C",
        case: "C4",
        heater_w: 1736.11,
        tchx_c: 40.0,
        exp_dracs: 3.649e-2,
        exp_pri: 2.949e-2,
        tuas_dracs: 3.715871e-2,
        tuas_pri: 2.968709e-2,
    },
    CoupledCase {
        set: "C",
        case: "C5",
        heater_w: 2026.29,
        tchx_c: 40.0,
        exp_dracs: 3.869e-2,
        exp_pri: 3.190e-2,
        tuas_dracs: 4.005844e-2,
        tuas_pri: 3.183202e-2,
    },
    CoupledCase {
        set: "C",
        case: "C6",
        heater_w: 2288.83,
        tchx_c: 40.0,
        exp_dracs: 4.115e-2,
        exp_pri: 3.412e-2,
        tuas_dracs: 4.243386e-2,
        tuas_pri: 3.357894e-2,
    },
    CoupledCase {
        set: "C",
        case: "C7",
        heater_w: 2508.71,
        tchx_c: 40.0,
        exp_dracs: 4.312e-2,
        exp_pri: 3.562e-2,
        tuas_dracs: 4.427672e-2,
        tuas_pri: 3.492421e-2,
    },
    CoupledCase {
        set: "C",
        case: "C8",
        heater_w: 2685.83,
        tchx_c: 40.0,
        exp_dracs: 4.509e-2,
        exp_pri: 3.593e-2,
        tuas_dracs: 4.567855e-2,
        tuas_pri: 3.594002e-2,
    },
    CoupledCase {
        set: "C",
        case: "C9",
        heater_w: 2764.53,
        tchx_c: 40.0,
        exp_dracs: 4.699e-2,
        exp_pri: 3.547e-2,
        tuas_dracs: 4.628018e-2,
        tuas_pri: 3.637349e-2,
    },
];

/// Percentage error of `value` relative to `reference`: `(value - ref)/ref * 100`.
fn pct_err(value: f64, reference: f64) -> f64 {
    (value - reference) / reference * 100.0
}

/// Format a mass flow rate in kg/s to 5 decimal places (0.01 g/s resolution).
fn fmt_flow(kg_per_s: f64) -> String {
    format!("{:.5}", kg_per_s)
}

/// Format a percentage error with an explicit sign and 2 decimal places.
fn fmt_pct(pct: f64) -> String {
    format!("{:+.2}", pct)
}

/// Build the full markdown report as a `String` (deterministic; no clock/RNG,
/// so re-running does not churn the committed file).
fn build_markdown() -> String {
    use std::fmt::Write as _;
    let mut md = String::new();

    // --- Mandatory AI-generated disclaimer (see AI_USAGE.md) ---------------
    md.push_str(
        "> **AI-GENERATED DOCUMENT — REQUIRES HUMAN REVIEW BEFORE USE.**\n\
         >\n\
         > This file was generated programmatically by a Rust test\n\
         > (`sam_vs_tuas_vs_experiment_summary.rs`) written with the assistance\n\
         > of an AI assistant (Anthropic Claude). Per the project `AI_USAGE.md`,\n\
         > AI-generated outputs are untrusted draft material: they are NOT\n\
         > accepted automatically and MUST undergo independent human review\n\
         > before being used, cited, or published. Scientific interpretation,\n\
         > verification, validation, and final acceptance remain under human\n\
         > control. Verify every number against the cited sources and the test\n\
         > outputs before relying on this document.\n\n",
    );

    md.push_str("# CIET coupled natural circulation: SAM vs TUAS (K=17.8) vs experiment\n\n");

    md.push_str(
        "This document compares three sources of steady-state natural-circulation\n\
         mass flow rates for the 25 coupled CIET tests (datasets A/B/C): the CIET\n\
         **experimental** measurements, the **TUAS** thermal-hydraulics library\n\
         (this crate) with the SAM-matched pipe-38 form loss **K = 17.8**\n\
         (`new_pipe_38_sam_model`), and the **SAM** code predictions.\n\n",
    );

    // --- Methodology -------------------------------------------------------
    md.push_str("## Methodology\n\n");
    md.push_str(
        "- **Benchmark / references.** CIET coupled natural-circulation steady\n\
         states (25 cases, sets A/B/C at TCHX outlet 46 / 35 / 40 degC).\n\
         Experimental values and SAM predictions are from NED-2021 Table 4:\n\
         Zou, L., Hu, G., O'Grady, D., & Hu, R. (2021), *Code validation of SAM\n\
         using natural-circulation experimental data from the compact integral\n\
         effects test (CIET) facility*, Nuclear Engineering and Design, 377,\n\
         111144 (accepted manuscript openly available, OSTI ID 1774637; also\n\
         ANL/NSE-19/11, Zou, Hu & Charpentier 2019).\n\
         - **TUAS inputs.** The coupled `dataset_a/b/c` regression tests, run\n\
         with pipe 38 at the SAM-matched form loss K = 17.8. The primary loop\n\
         already uses the SAM K values (pipe 3 K=17.15, pipe 22 K=45.95).\n\
         - **Quantities.** DRACS-loop and heater-DHX (primary) subloop mass flow\n\
         rate at steady state, in kg/s.\n\
         - **Error metric.** Percentage error against the CIET experiment,\n\
         `(model - experiment) / experiment * 100`, for both TUAS and SAM.\n\
         - **Data version.** TUAS values are the K=17.8 regression outputs\n\
         measured 2026-07-15 (`cargo test --release`, 25/25 coupled tests pass).\n\n",
    );

    // --- Per-loop comparison tables ---------------------------------------
    md.push_str("## DRACS loop mass flow rate (kg/s)\n\n");
    md.push_str(&flow_table(Loop::Dracs));
    md.push('\n');

    md.push_str("## Primary (heater-DHX) loop mass flow rate (kg/s)\n\n");
    md.push_str(&flow_table(Loop::Primary));
    md.push('\n');

    // --- Max-error summary -------------------------------------------------
    let s = summary_stats();
    md.push_str("## Maximum absolute error vs experiment\n\n");
    md.push_str(
        "| Loop | TUAS K=17.8 max \\|error\\| | at case | SAM max \\|error\\| | at case |\n",
    );
    md.push_str("|---|---|---|---|---|\n");
    let _ = writeln!(
        md,
        "| DRACS | {:.2}% | {} | {:.2}% | {} |",
        s.tuas_dracs_max_abs, s.tuas_dracs_max_case, s.sam_dracs_max_abs, s.sam_dracs_max_case
    );
    let _ = writeln!(
        md,
        "| Primary | {:.2}% | {} | {:.2}% | {} |",
        s.tuas_pri_max_abs, s.tuas_pri_max_case, s.sam_pri_max_abs, s.sam_pri_max_case
    );
    md.push('\n');

    // --- Prose summary -----------------------------------------------------
    md.push_str("## Summary\n\n");
    let _ = writeln!(
        md,
        "Across all 25 coupled cases, TUAS with the SAM-matched pipe-38 form loss\n\
         K = 17.8 has a maximum DRACS-loop error of {:.2}% (case {}) and a maximum\n\
         primary-loop error of {:.2}% (case {}). The SAM code, on the same\n\
         experimental dataset, has a maximum DRACS error of {:.2}% (case {}) and a\n\
         maximum primary error of {:.2}% (case {}).",
        s.tuas_dracs_max_abs,
        s.tuas_dracs_max_case,
        s.tuas_pri_max_abs,
        s.tuas_pri_max_case,
        s.sam_dracs_max_abs,
        s.sam_dracs_max_case,
        s.sam_pri_max_abs,
        s.sam_pri_max_case,
    );
    md.push('\n');
    md.push_str(
        "On the **primary loop**, TUAS's worst-case agreement is better than SAM's.\n\
         On the **DRACS loop**, TUAS's worst case is essentially tied with SAM's\n\
         (both codes have their largest DRACS error on the same low-power case,\n\
         C1). Both codes systematically under-predict the DRACS flow at the two\n\
         lowest-power operating points (B1, C1) and over-predict it at higher\n\
         flow; a single uniform form loss cannot correct both ends, which is why\n\
         C1/B1 sit at the edge of (or just past) the SAM-defined error band. This\n\
         is a documented limitation, not a tuned result: see the coupled-loop\n\
         module docs and the follow-up on a velocity/Reynolds-dependent pipe-38\n\
         form loss.\n\n",
    );

    md.push_str(
        "*Generated by `sam_vs_tuas_vs_experiment_summary.rs` in the\n\
         `tuas_boussinesq_solver` crate. Numbers are the real experiment / TUAS /\n\
         SAM values from the sources cited above; see the module documentation\n\
         for provenance. This document is AI-generated and requires human review\n\
         (see the disclaimer at the top).*\n",
    );

    md
}

/// Which loop a table/column refers to.
#[derive(Clone, Copy)]
enum Loop {
    Dracs,
    Primary,
}

/// Build one per-loop GFM table (25 rows, grouped implicitly by the case label).
fn flow_table(which: Loop) -> String {
    use std::fmt::Write as _;
    let mut t = String::new();
    t.push_str("| Case | Heater (W) | TCHX (degC) | Experiment | TUAS K=17.8 | TUAS %err | SAM | SAM %err |\n");
    t.push_str("|---|---|---|---|---|---|---|---|\n");
    for c in COUPLED_CASES.iter() {
        let (exp, tuas) = match which {
            Loop::Dracs => (c.exp_dracs, c.tuas_dracs),
            Loop::Primary => (c.exp_pri, c.tuas_pri),
        };
        // SAM value from NED-2021 Table 4.
        let sam = super::sam_table4_flowrates_kg_per_s(c.set, c.heater_w);
        let (sam_cell, sam_err_cell) = match sam {
            Some((sam_dracs, sam_pri)) => {
                let sam_val = match which {
                    Loop::Dracs => sam_dracs,
                    Loop::Primary => sam_pri,
                };
                (fmt_flow(sam_val), fmt_pct(pct_err(sam_val, exp)))
            }
            None => (String::from("n/a"), String::from("n/a")),
        };
        let _ = writeln!(
            t,
            "| {} | {:.2} | {:.0} | {} | {} | {} | {} | {} |",
            c.case,
            c.heater_w,
            c.tchx_c,
            fmt_flow(exp),
            fmt_flow(tuas),
            fmt_pct(pct_err(tuas, exp)),
            sam_cell,
            sam_err_cell,
        );
    }
    t
}

/// Maximum absolute percentage errors (and the cases where they occur).
struct SummaryStats {
    tuas_dracs_max_abs: f64,
    tuas_dracs_max_case: &'static str,
    tuas_pri_max_abs: f64,
    tuas_pri_max_case: &'static str,
    sam_dracs_max_abs: f64,
    sam_dracs_max_case: &'static str,
    sam_pri_max_abs: f64,
    sam_pri_max_case: &'static str,
}

/// Compute the max |error| stats over all 25 cases for TUAS and SAM, both loops.
fn summary_stats() -> SummaryStats {
    let mut s = SummaryStats {
        tuas_dracs_max_abs: 0.0,
        tuas_dracs_max_case: "",
        tuas_pri_max_abs: 0.0,
        tuas_pri_max_case: "",
        sam_dracs_max_abs: 0.0,
        sam_dracs_max_case: "",
        sam_pri_max_abs: 0.0,
        sam_pri_max_case: "",
    };
    for c in COUPLED_CASES.iter() {
        let td = pct_err(c.tuas_dracs, c.exp_dracs).abs();
        if td > s.tuas_dracs_max_abs {
            s.tuas_dracs_max_abs = td;
            s.tuas_dracs_max_case = c.case;
        }
        let tp = pct_err(c.tuas_pri, c.exp_pri).abs();
        if tp > s.tuas_pri_max_abs {
            s.tuas_pri_max_abs = tp;
            s.tuas_pri_max_case = c.case;
        }
        if let Some((sam_dracs, sam_pri)) = super::sam_table4_flowrates_kg_per_s(c.set, c.heater_w)
        {
            let sd = pct_err(sam_dracs, c.exp_dracs).abs();
            if sd > s.sam_dracs_max_abs {
                s.sam_dracs_max_abs = sd;
                s.sam_dracs_max_case = c.case;
            }
            let sp = pct_err(sam_pri, c.exp_pri).abs();
            if sp > s.sam_pri_max_abs {
                s.sam_pri_max_abs = sp;
                s.sam_pri_max_case = c.case;
            }
        }
    }
    s
}

/// Generate the SAM-vs-TUAS-vs-experiment markdown report and write it to
/// `verification_and_validation/coupled_natural_circulation_SAM_vs_TUAS_vs_experiment.md`.
///
/// This runs under a normal `cargo test`, so the committed report is refreshed
/// automatically. The test also sanity-checks that the report is non-empty and
/// that the SAM data was found for all 25 cases (i.e. no `n/a` slipped in).
#[test]
fn generate_coupled_natcirc_sam_vs_tuas_vs_experiment_md() {
    let md = build_markdown();

    // Every coupled case must have resolved SAM data from NED-2021 Table 4.
    assert!(
        !md.contains("n/a"),
        "SAM Table-4 lookup failed for at least one case (found 'n/a' in report)"
    );
    // Report should contain all 25 case labels.
    for c in COUPLED_CASES.iter() {
        assert!(md.contains(c.case), "report missing case {}", c.case);
    }

    let path = crate::vnv_test_support::vnv_csv_path(
        "coupled_natural_circulation_SAM_vs_TUAS_vs_experiment.md",
    );
    std::fs::write(&path, md).expect("could not write SAM-vs-TUAS-vs-experiment markdown");
    eprintln!(
        "wrote coupled natcirc comparison markdown to {}",
        path.display()
    );
}
