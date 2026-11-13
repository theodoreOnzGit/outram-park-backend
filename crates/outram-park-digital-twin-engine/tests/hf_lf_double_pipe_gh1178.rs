//! gh:#1178, rung 1 of the HF → LF epic gh:#1177: the double-pipe CFD's
//! closures into the TUAS 1-D exchanger (`multifidelity::double_pipe`).
//!
//! **Methodology** (written before the runs, with the predictions, in
//! `verification_and_validation/hf_lf_double_pipe_gh1178.md`): the CFD
//! (`outram_foam_appbuilder_lib::cases::double_pipe`, 20k demo
//! cross-section, Δz ≈ 2.67 mm) over a 36-point full-factorial training grid
//! and 8 held-out points in (Re_hot, Re_cold, k_wall, L); the TUAS 1-D
//! exchanger at the same points (a) uncalibrated, with its own textbook
//! correlations, and (b) with the CFD's mean Nusselt numbers and apparent
//! friction, interpolated log-multilinearly at the held-out points.
//!
//! **Results:** see the V&V record's "Results" (2026-11-03) and the
//! doc comments of the tests below.
use outram_park_digital_twin_engine::multifidelity::double_pipe::cfd::{self, CfdMesh};
use outram_park_digital_twin_engine::multifidelity::double_pipe::dataset::{self, record_row};
use outram_park_digital_twin_engine::multifidelity::double_pipe::design::{design_grid, DesignPoint, Split};
use outram_park_digital_twin_engine::multifidelity::double_pipe::study;
use outram_park_digital_twin_engine::multifidelity::double_pipe::DoublePipeInput;

/// The study's plumbing on the coarse test grid (not a dataset grid): two
/// points on two threads, energy closing, the CSVs round-tripping through
/// the parser. A harness check, not V&V.
#[test]
fn coarse_grid_plumbing() {
    let pts = vec![
        DesignPoint { id: "A".into(), split: Split::Training, input: DoublePipeInput::si(30.0, 15.0, 10.0, 0.1) },
        DesignPoint { id: "B".into(), split: Split::HeldOut, input: DoublePipeInput::si(10.0, 45.0, 100.0, 0.1) },
    ];
    let recs = study::run_cfd_design(&pts, CfdMesh::coarse_test(), 2, |s| println!("{s}")).unwrap();
    for (id, _, r) in &recs {
        let q = r.output.q.value;
        assert!((r.q_cold.value / q - 1.0).abs() < 1e-6, "{id}: energy");
        assert!(r.output.nu_hot.value > 2.0 && r.output.nu_cold.value > 2.0, "{id}: {:?}", r.output);
        assert!(r.output.dp_hot.value > 0.0 && r.output.dp_cold.value > 0.0);
        assert_eq!(r.profiles.z.len(), r.layers);
    }
    let csv = dataset::write_records(&recs);
    let rows = dataset::parse_records(&csv).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].split, Split::HeldOut);
    assert_eq!(rows[0].output, recs[0].2.output, "shortest round-trip floats");
    let prof = dataset::write_profiles(&recs);
    assert_eq!(prof.lines().filter(|l| !l.starts_with('#')).count(), 1 + recs[0].2.layers + recs[1].2.layers);
}

/// The committed dataset is reproducible: T33 (Re_hot 90, Re_cold 45,
/// k_wall 1, L 0.1 m; the cheapest point, ≈ 2 s) re-run on the dataset's
/// mesh gives the committed row to **1e-3 relative in every number** (text
/// fields exactly), apart from the wall time.
///
/// ~~Result (2026-11-03): identical, character for character.~~ **CHANGED
/// 2026-11-06 (maintainer: "no bit identity required"):** the gh:#1202
/// PimpleFoam fixes (upstream interpolation weights, warm-started momentum
/// predictor) moved the CFD in the 4th-5th figure (Nu 0.6805446 ->
/// 0.6805771, 4.8e-5 relative), so the row no longer regenerates character
/// for character (gh:#1300). 1e-3 is about ten times below the smallest
/// effect this study resolves (the CFD's own 0.8 % discretisation error,
/// P7) and far below its 1-70 % comparisons, so a row inside it still
/// supports the recorded conclusions.
#[test]
fn the_committed_dataset_regenerates_within_1e_3() {
    let p = design_grid().into_iter().find(|p| p.id == "T33").unwrap();
    let rec = cfd::run(&p.input, CfdMesh::default_grid()).unwrap();
    let fresh = record_row(&p.id, p.split, &rec);
    let committed = dataset::COMMITTED_CSV.lines().find(|l| l.starts_with("T33,")).unwrap();
    // Every field but the last (the wall time).
    let fields = |l: &str| l.rsplit_once(',').unwrap().0.split(',').map(str::to_string).collect::<Vec<_>>();
    let (a, b) = (fields(&fresh), fields(committed));
    assert_eq!(a.len(), b.len(), "{fresh}\n{committed}");
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        match (x.parse::<f64>(), y.parse::<f64>()) {
            (Ok(u), Ok(v)) => {
                let scale = u.abs().max(v.abs());
                assert!(scale == 0.0 || (u - v).abs() <= 1e-3 * scale, "field {i}: {u} vs committed {v}");
            }
            _ => assert_eq!(x, y, "field {i}"),
        }
    }
}

/// Columns of `one_d_results.csv` by header name.
fn one_d_table() -> Vec<std::collections::HashMap<String, String>> {
    let mut lines = dataset::COMMITTED_ONE_D_CSV.lines().filter(|l| !l.starts_with('#'));
    let head: Vec<String> = lines.next().unwrap().split(',').map(str::to_string).collect();
    lines.map(|l| head.iter().cloned().zip(l.split(',').map(str::to_string)).collect()).collect()
}

fn num(r: &std::collections::HashMap<String, String>, k: &str) -> f64 {
    r[k].parse().unwrap()
}

/// **The V&V gates, on the committed record (all 44 points).** Only the
/// predictions that were met are gated (V&V record, "Against the
/// predictions"); a missed prediction is reported there, not re-gated at a
/// moved threshold.
///
/// - P1: the CFD's annulus Nu is above TUAS's pipe correlation on D_h at
///   every point (measured +25.7 % to +58.1 %; the predicted *range*,
///   +15 to +45 %, was missed at the top).
/// - P3 / P4 (signs): the uncalibrated 1-D UA and Q are below the CFD's at
///   every point (UA −1.1 % to −21.4 %, Q −0.9 % to −6.7 %).
/// - P5 / P6 (signs): the CFD's Δp is above TUAS's on both sides at every
///   point (tube +3.8 to +23.5 %, annulus +43.7 to +47.2 %).
/// - P8 (sign): the 1-D ε is below ε-NTU at its own UA at every point.
/// - P10 / P11 / P12 (held-out): interpolated Nu within 5 % of the point's
///   own (measured ≤ 4.93 % tube, ≤ 3.58 % annulus); 1-D Q with the
///   closures within 3 % (≤ 2.54 %) and closer than uncalibrated at every
///   held-out point; Δp within 3 % (≤ 1.66 %).
#[test]
fn predictions_that_held_still_hold_on_the_committed_record() {
    let t = one_d_table();
    assert_eq!(t.len(), 44);
    let cfd: std::collections::HashMap<String, dataset::DatasetRow> = dataset::load_committed().into_iter().map(|r| (r.id.clone(), r)).collect();
    for r in &t {
        let id = &r["id"];
        let y = &cfd[id].output;
        assert!(num(r, "nu_cold_cfd [-]") > num(r, "nu_cold_a [-]"), "P1 {id}");
        assert!(num(r, "ua_a [W/K]") < num(r, "ua_cfd [W/K]"), "P3 {id}");
        assert!(num(r, "q_a [W]") < num(r, "q_cfd [W]"), "P4 {id}");
        assert!(y.dp_hot.value > num(r, "dp_hot_a [Pa]") && y.dp_cold.value > num(r, "dp_cold_a [Pa]"), "P5/P6 {id}");
        assert!(num(r, "eps_a [-]") < num(r, "eps_ntu_a [-]"), "P8 {id}");
        if r["split"] == "held_out" {
            let rel = |a: &str, b: &str| (num(r, a) / num(r, b) - 1.0).abs();
            assert!(rel("nu_hot_b [-]", "nu_hot_cfd [-]") < 0.05 && rel("nu_cold_b [-]", "nu_cold_cfd [-]") < 0.05, "P10 {id}");
            assert!(rel("q_b [W]", "q_cfd [W]") < 0.03, "P11 {id}");
            assert!(rel("q_b [W]", "q_cfd [W]") < rel("q_a [W]", "q_cfd [W]"), "P11 improvement {id}");
            assert!((num(r, "dp_hot_b [Pa]") / y.dp_hot.value - 1.0).abs() < 0.03 && (num(r, "dp_cold_b [Pa]") / y.dp_cold.value - 1.0).abs() < 0.03, "P12 {id}");
        }
    }
}

/// The 1-D side regenerates the committed record at the held-out points
/// (the closure table built from all 36 training rows; 16 TUAS runs on 3
/// threads), to round-off (≤ 1e-12 relative), and the CSV/Markdown writers
/// run on the result.
#[test]
fn the_held_out_one_d_runs_regenerate_the_record() {
    let rows = dataset::load_committed();
    let ids: Vec<&str> = rows.iter().filter(|r| r.split == Split::HeldOut).map(|r| r.id.as_str()).collect();
    let c = study::compare_only(&rows, &ids, 3).unwrap();
    assert_eq!(c.len(), 8);
    let t = one_d_table();
    for k in &c {
        let r = t.iter().find(|r| r["id"] == k.row.id).unwrap();
        for (fresh, col) in [(k.uncalibrated.q.value, "q_a [W]"), (k.uncalibrated.ua.value, "ua_a [W/K]"), (k.with_closures.q.value, "q_b [W]"), (k.with_closures.ua.value, "ua_b [W/K]"), (k.with_closures.dp_cold.value, "dp_cold_b [Pa]")] {
            assert!((fresh / num(r, col) - 1.0).abs() < 1e-12, "{} {col}: {fresh} vs {}", k.row.id, r[col]);
        }
    }
    let csv = study::comparison_csv(&c);
    assert_eq!(csv.lines().filter(|l| !l.starts_with('#')).count(), 9);
    let md = study::markdown_summary(&c);
    assert_eq!(md.lines().count(), 10);
    assert!((study::rel(2.0, 1.0) - 1.0).abs() < 1e-15);
}
