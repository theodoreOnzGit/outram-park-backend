//! V&V: what a **thinned tabulated-temperature grid** costs in accuracy —
//! graphite thermal scattering, ENDF/B-VIII.0.
//!
//! # What this measures
//!
//! `tsl-crystalline-graphite.endf` is 8 730 804 B, of which MF=7/MT=4 (the
//! incoherent-inelastic `S(α,β)`) is 8 695 236 B = **99.59 %** and MF=7/MT=2
//! (elastic) is 31 844 B = **0.36 %** — both measured from the tape on
//! 2026-08-13. The elastic channel nevertheless carries ~90 % of graphite's
//! thermal cross section (4.5514 b vs 0.4864 b at 0.0253 eV / 296 K). So the
//! bytes and the physics sit in different places, and the obvious low-fidelity
//! option — keep a few of the ten tabulated temperatures and interpolate — has
//! to be *measured*, not guessed.
//!
//! # Methodology
//!
//! The evaluation is its own oracle. For a candidate thinned grid, each
//! **withheld** tabulated temperature is reconstructed by interpolating from
//! the two kept temperatures that bracket it, through the *same*
//! `interp_s_temperature` kernel the production reader uses and under the
//! evaluation's own ENDF `LI` law, then compared against the row the
//! evaluation actually tabulates there. No model, no fit, no external
//! reference. Machinery: `thermr::temperature_thinning`; full report:
//! `examples/temperature_thinning_study.rs`.
//!
//! Four comparison levels, because they answer different questions:
//!
//! 1. `S(E)` at every Bragg edge (= the relative `σ_coh` error, since the edge
//!    energies are temperature-independent), whole table and restricted to
//!    E ≤ 0.0253 eV.
//! 2. `S(α,β)` over all 400 β × 150 α points, unfiltered and above a
//!    10⁻⁶ × S_max significance floor.
//! 3. `σ_inel(E)` — both tables run through the same production kernel at the
//!    same physical temperature.
//! 4. **`σ_total(0.0253 eV) = σ_coh + σ_inel`** with both channels thinned on
//!    the same grid. This is the decision number.
//!
//! Two extras: **leave-one-out** on the full grid (drop one interior
//! temperature, keep its immediate neighbours) characterises the *existing*
//! production interpolation; and a **log-space** run (`LI = 4`) against the
//! elastic channel's stated `LI = 2` tests the prior that Debye-Waller
//! suppression is roughly exponential in `T`.
//!
//! ## Data / provenance
//!
//! ENDF/B-VIII.0 thermal-scattering sublibrary (2018; open, per
//! `DATA_POLICY.md`), `tsl-crystalline-graphite.endf`, MAT 30, ZA 130, LEIP
//! Lab / A. I. Hawari, Y. Zhu, J. L. Wormald, NDS 148, 1 (2018). Tabulated at
//! 296, 400, 500, 600, 700, 800, 1000, 1200, 1600, 2000 K. Read from
//! `GRAPHITE_TSL_DIR` (env override) or the default path below; the tape is
//! **not** checked in and every test here **skips** (prints a note, passes)
//! when it is absent.
//!
//! # Measured results — 2026-08-13, ENDF/B-VIII.0, MAT 30
//!
//! Grid legend (all keep 296 K, which carries the shared `α` grid, and 2000 K,
//! the top of the range): **A** 296/600/1200/2000, **B** 296/800/2000,
//! **C** 296/500/1000/2000, **D** 296/400/600/1000/2000,
//! **E** 296/400/500/600/1000/2000. The five were fixed before any error was
//! measured.
//!
//! ## 1. Structure read off the tape
//!
//! Elastic `LI = 2` (lin-lin) on all nine intervals, 221 Bragg edges;
//! inelastic `LI = 4` (log-lin, `ln S` linear in `T`) on all nine, `LAT = 1`,
//! 400 β × 150 α = 60 000 cells per temperature, of which 9.8–11.1 % are
//! exactly zero (LEAPR-floored). Max tabulated `S` = 3.979191.
//!
//! ## 2. Coherent elastic — the error is concentrated where it does not matter
//!
//! Leave-one-out on the full grid, max relative `S(E)` error over all 221
//! edges vs. over the edges at/below 0.0253 eV:
//!
//! | Withheld | from | all edges (LI=2) | E ≤ 0.0253 eV (LI=2) | all edges (LI=4) | E ≤ 0.0253 eV (LI=4) |
//! |---|---|---|---|---|---|
//! | 400 K | 296/500 | 4.51 % | 0.0375 % | 1.99 % | 0.0409 % |
//! | 500 K | 400/600 | 3.13 % | 0.0614 % | 1.42 % | 0.0161 % |
//! | 600 K | 500/700 | 2.35 % | 0.0752 % | 1.08 % | 0.0060 % |
//! | 700 K | 600/800 | 1.82 % | 0.0832 % | 0.8397 % | 0.0049 % |
//! | 800 K | 700/1000 | 2.64 % | 0.1750 % | 1.26 % | 0.0225 % |
//! | 1000 K | 800/1200 | 4.03 % | 0.3658 % | 1.85 % | 0.0634 % |
//! | 1200 K | 1000/1600 | 5.06 % | 0.7140 % | 2.43 % | 0.1563 % |
//! | 1600 K | 1200/2000 | 7.05 % | 1.42 % | 3.17 % | 0.3456 % |
//!
//! The worst whole-table errors sit at Bragg edges of 0.37–1.28 eV (and up to
//! the table's 5 eV top), where `σ_coh = S/E` is two decades below thermal —
//! high-`Q` reflections carry the largest Debye-Waller exponents and therefore
//! the strongest curvature in `T`. Inside the thermal window the same
//! interpolation is 50–100× more accurate. Thinned grids behave the same way:
//! `σ_coh(0.0253 eV)` error is **≤ 0.74 % over 293–1000 K for grid A**,
//! ≤ 1.65 % for the 3-temperature grid B, ≤ 0.52 % for C, ≤ 0.35 % for D/E;
//! the worst case anywhere is 3.02 % (grid B at 1600 K). Elastic thinning is
//! effectively free — but it also saves nothing, being 0.36 % of the tape.
//!
//! ## 3. Log space *is* materially better for the elastic channel
//!
//! `LI = 4` (log-lin) beat the evaluation's stated `LI = 2` on **all 221
//! edges at every one of the eight leave-one-out points**, roughly halving the
//! error (4.51 → 1.99 %, 7.05 → 3.17 %). In the thermal window it wins from
//! 500 K up (0.0614 → 0.0161 % at 500 K; 1.42 → 0.3456 % at 1600 K) and is a
//! wash at 400 K (0.0375 → 0.0409 %, i.e. marginally worse). This confirms the
//! physical prior — Debye-Waller suppression is close to exponential in `T`,
//! so `ln S` is nearer linear than `S` is. **It is reported, not applied:**
//! ENDF/B-VIII.0 states `LI = 2` and the production path keeps it.
//!
//! ## 4. Incoherent inelastic — much larger errors, small weight
//!
//! `σ_inel` max relative error over E ∈ {0.001, 0.005, 0.0253, 0.1} eV:
//! grid A 8.8–14.3 %, B 14.9–27.1 %, C 8.6–13.5 %, D 5.1–7.5 %, E 6.2–7.5 %
//! across their withheld points in 293–1000 K. Leave-one-out on the *full*
//! grid still gives 4.6–18.5 % RMS over all six test energies, so most of this
//! is the difficulty of interpolating `S(α,β)` in temperature at all, not the
//! thinning.
//!
//! Reconstruction consistency was checked: holding the physical temperature at
//! 400 K and swapping only the table, `σ_inel(3.9 eV)` is 0.7228 b with the
//! 296 K table, 858.63 b with the 600 K table, 2.0459 b with the interpolated
//! table and 4.6367 b with the true 400 K table. The interpolated value lies
//! between the two endpoints at every test energy, so the machinery is
//! monotone and the large errors are a property of the data: at high incident
//! energy the integral samples the steep far tail of `S(α,β)`, where a modest
//! table error swings `σ` by orders of magnitude.
//!
//! ## 5. Combined `σ_total(0.0253 eV)` — the decision number
//!
//! Both channels thinned on the same grid, per withheld temperature:
//!
//! | Grid | withheld in 293–1000 K | worst σ_total error there | worst anywhere | MT=4 bytes | tape saved |
//! |---|---|---|---|---|---|
//! | A 296/600/1200/2000 | 400, 500, 700, 800, 1000 | **3.13 %** (1000 K) | 3.13 % | 3 952 684 | 54.3 % |
//! | B 296/800/2000 | 400, 500, 600, 700, 1000 | **4.27 %** (600 K) | 7.05 % (1600 K) | 3 162 208 | 63.4 % |
//! | C 296/500/1000/2000 | 400, 600, 700, 800 | **2.88 %** (800 K) | 4.66 % (1600 K) | 3 952 684 | 54.3 % |
//! | D 296/400/600/1000/2000 | 500, 700, 800 | **1.72 %** (800 K) | 4.66 % (1600 K) | 4 743 084 | 45.3 % |
//! | E 296/400/500/600/1000/2000 | 700, 800 | **1.72 %** (800 K) | 4.66 % (1600 K) | 5 533 484 | 36.2 % |
//!
//! Worked example (grid A, 400 K): coherent 4.3731 → 4.3771 b (0.0907 %),
//! inelastic 0.6960 → 0.6107 b (12.26 %), total 5.0691 → 4.9878 b (**1.60 %**).
//! The 12 % inelastic error is diluted to 1.6 % because inelastic is only
//! ~14 % of the total at that point.
//!
//! **Every candidate grid meets a 5 % criterion across 293–1000 K. None meets
//! 1 %** — the best (D and E) reach 1.72 %. Under a 2 % criterion, D (45.3 %
//! saved) and E (36.2 % saved) pass and A/B/C do not.
//!
//! ## 6. A pre-existing defect found on the way (NOT a thinning result)
//!
//! `σ_inel(E, T)` is monotone in `T` across the tabulated grid, so an
//! interpolated temperature whose `σ` falls outside its tabulated bracket is a
//! defect. The **production** path (`parse_mf7_at_temperature`, adjacent
//! tabulated temperatures, stated `LI = 4`) leaves the bracket at high energy:
//!
//! | T | 0.001–0.1 eV | 0.5 eV | 3.9 eV |
//! |---|---|---|---|
//! | 393.15 K | in bracket | 3.9488 ∈ [3.7915, 3.9978] ok | **4.4175 ∉ [4.6097, 4.6367]** |
//! | 523.15 K | in bracket | **4.1217 ∉ [4.1328, 4.2303]** | **4.4784 ∉ [4.6544, 4.6672]** |
//! | 900 K | in bracket | **4.3471 ∉ [4.3614, 4.4459]** | **4.4549 ∉ [4.6823, 4.6913]** |
//! | 1400 K | in bracket | **4.4635 ∉ [4.5057, 4.5866]** | **4.4279 ∉ [4.6927, 4.7006]** |
//!
//! ~4–5 % low at 3.9 eV, ≲ 1 % low at 0.5 eV, and correctly bracketed at and
//! below 0.1 eV. This affects every non-tabulated temperature request today,
//! independent of any thinning decision. It is **reported here, not fixed** —
//! the fix is a separate change with its own V&V.
//!
//! # Interpretation
//!
//! - **For HTR-10 (293 / 393 / 523 K) thinning is viable, with a caveat.**
//!   The measurable proxies in that band are the withheld 400 K and 500 K
//!   points. Grid A costs 1.60 % / 1.79 % of `σ_total(0.0253 eV)` there; grid C
//!   costs 0.96 % at 400 K and keeps 500 K; grid D keeps 400 K and costs
//!   0.73 % at 500 K. The caveat is that 293/393/523 K are themselves *not*
//!   tabulated, so they have no oracle — the withheld tabulated points are
//!   proxies, and the true error at 393 K on a thinned grid is bounded by, not
//!   equal to, the 400 K figure.
//! - **Keeping 296/400/500/600 K costs nothing at all in the HTR-10 band**,
//!   because the reconstruction there is then identical to the production
//!   path. Grid E does this and still removes 36.2 % of the tape by thinning
//!   only above 600 K, where HTR-10 does not operate.
//! - **Thin the inelastic, keep the elastic in full.** The elastic section is
//!   0.36 % of the tape and its thermal-window error is ≤ 0.35 % on the denser
//!   grids; thinning it buys nothing and is the channel carrying 90 % of the
//!   cross section.
//! - **The error is concentrated, not spread.** Elastic: at high-`Q` Bragg
//!   edges above ~0.4 eV. Inelastic: at high incident energy (3.9 eV worst at
//!   every grid and every withheld temperature) and, in the table, at large
//!   `α` and `β`. Both are outside the thermal window that drives HTR-10.
//!
//! # What was NOT measured
//!
//! - **No independent oracle.** Every figure is the evaluation against itself.
//!   Nothing here is compared with NJOY2016's own output, an ACE file, or a
//!   critical benchmark, so none of it validates the underlying THERMR port.
//! - **No k_eff impact.** A 1.7–3 % `σ_total` error has not been propagated
//!   through a transport calculation; the reactivity worth of these errors is
//!   unknown.
//! - **Only MAT 30** (crystalline graphite). The 10P and 30P reactor-graphite
//!   tapes have the same temperature grid and byte structure but were not run.
//! - **293.15 / 393.15 / 523.15 K have no ground truth** — see the caveat
//!   above.
//! - **Angular/emission distributions were not compared**, only integrated
//!   cross sections and the underlying tables.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::thermr::mf7::{parse_mf7, parse_mf7_at_temperature, CoherentElastic};
use njoy_outram_park_fork::thermr::temperature_thinning::{
    coherent_elastic_thinned_row, coherent_elastic_thinning_error,
    coherent_elastic_thinning_error_below, mf7_mt4_endf_bytes, SabTemperatureStack,
    ThinnedTemperatureGrid,
};

const DEFAULT_DIR: &str = "/home/teddy0/Documents/research/ENDF-B-VIII.0/thermal_scatt";
const FILE: &str = "tsl-crystalline-graphite.endf";
const MAT: i32 = 30;
const E_THERMAL: f64 = 0.0253;

/// The ten tabulated temperatures \[K\] of the ENDF/B-VIII.0 graphites.
const TABULATED_K: [f64; 10] = [
    296.0, 400.0, 500.0, 600.0, 700.0, 800.0, 1000.0, 1200.0, 1600.0, 2000.0,
];

/// Grid A of the study — the brief's example thinning.
const GRID_A: [f64; 4] = [296.0, 600.0, 1200.0, 2000.0];
/// Grid D — bottom-dense, the densest candidate that still drops four points.
const GRID_D: [f64; 5] = [296.0, 400.0, 600.0, 1000.0, 2000.0];

/// Resolve the tape path (env override → default), or `None` + a skip note.
fn tape_path() -> Option<std::path::PathBuf> {
    let dir = std::env::var("GRAPHITE_TSL_DIR").unwrap_or_else(|_| DEFAULT_DIR.to_string());
    let p = std::path::Path::new(&dir).join(FILE);
    if p.exists() {
        Some(p)
    } else {
        eprintln!(
            "SKIP thermal_temperature_thinning: {FILE} not found under {dir} \
             (set GRAPHITE_TSL_DIR)"
        );
        None
    }
}

fn load_tape() -> Option<Tape> {
    Some(Tape::read(std::fs::File::open(tape_path()?).unwrap()).unwrap())
}

fn load_coherent() -> Option<CoherentElastic> {
    Some(
        parse_mf7(&load_tape()?, MAT)
            .unwrap()
            .coherent_elastic
            .expect("graphite has coherent elastic"),
    )
}

/// Assert `got` is within `tol` (relative) of the value measured on 2026-08-13.
#[track_caller]
fn near(got: f64, measured: f64, tol: f64, what: &str) {
    assert!(
        (got - measured).abs() <= tol * measured.abs(),
        "{what}: got {got:.6}, measured 2026-08-13 {measured:.6} (tol {:.1} %)",
        100.0 * tol
    );
}

/// (1) Structure read off the tape: ten temperatures, elastic `LI = 2`,
/// inelastic `LI = 4`, `LAT = 1`, 221 Bragg edges, 400 β × 150 α.
#[test]
fn evaluation_structure_is_as_documented() {
    let Some(tape) = load_tape() else { return };
    let ce = parse_mf7(&tape, MAT).unwrap().coherent_elastic.unwrap();
    assert_eq!(ce.temperatures_k, TABULATED_K.to_vec());
    assert_eq!(ce.bragg_energies_ev.len(), 221, "Bragg edges");
    assert!(
        ce.temp_interp.iter().all(|&li| li == 2),
        "elastic LI codes are lin-lin: {:?}",
        ce.temp_interp
    );

    let ii = parse_mf7(&tape, MAT).unwrap().incoherent_inelastic.unwrap();
    assert_eq!(ii.lat, 1, "LAT = 1 (α, β scaled to 0.0253 eV)");
    assert_eq!(ii.beta.len(), 400, "β grid");
    assert_eq!(ii.s_tables[0].alpha.len(), 150, "α grid");
    assert!(
        ii.temp_interp.iter().all(|&li| li == 4),
        "inelastic LI codes are log-lin: {:?}",
        ii.temp_interp
    );
    assert_eq!(ii.temp_interp.len(), 9, "one LI per interval");
}

/// (2) Leave-one-out on the full grid — the accuracy of the *production*
/// interpolation for the elastic channel — and the concentration of the error
/// outside the thermal window. Measured 2026-08-13; see the module table.
#[test]
fn coherent_elastic_error_is_concentrated_above_the_thermal_window() {
    let Some(ce) = load_coherent() else { return };
    // (withheld index, max % over all edges, max % over E ≤ 0.0253 eV)
    let measured = [
        (1usize, 4.51, 0.0375),
        (2, 3.13, 0.0614),
        (3, 2.35, 0.0752),
        (4, 1.82, 0.0832),
        (5, 2.64, 0.1750),
        (6, 4.03, 0.3658),
        (7, 5.06, 0.7140),
        (8, 7.05, 1.42),
    ];
    for (j, all_pct, thermal_pct) in measured {
        let grid = ThinnedTemperatureGrid::leave_one_out(TABULATED_K.len(), j).unwrap();
        let all = coherent_elastic_thinning_error(&ce, &grid, j, None).unwrap();
        let thermal =
            coherent_elastic_thinning_error_below(&ce, &grid, j, None, E_THERMAL).unwrap();
        eprintln!(
            "{:>5} K: all edges {:.3} %, E <= 0.0253 eV {:.4} %",
            TABULATED_K[j],
            100.0 * all.max_rel,
            100.0 * thermal.max_rel
        );
        near(
            100.0 * all.max_rel,
            all_pct,
            0.02,
            &format!("{} K, all edges", TABULATED_K[j]),
        );
        near(
            100.0 * thermal.max_rel,
            thermal_pct,
            0.02,
            &format!("{} K, thermal window", TABULATED_K[j]),
        );
        assert!(
            thermal.max_rel < all.max_rel,
            "{} K: the thermal window must be the easier one",
            TABULATED_K[j]
        );
    }
}

/// (3) Log-space (`LI = 4`) beats the evaluation's stated `LI = 2` on the whole
/// elastic table at every leave-one-out point, roughly halving the error —
/// the Debye-Waller-is-exponential-in-T prior, confirmed. Reported only; the
/// production path keeps `LI = 2`.
#[test]
fn log_space_halves_the_coherent_elastic_error() {
    let Some(ce) = load_coherent() else { return };
    for j in 1..TABULATED_K.len() - 1 {
        let grid = ThinnedTemperatureGrid::leave_one_out(TABULATED_K.len(), j).unwrap();
        let lin = coherent_elastic_thinning_error(&ce, &grid, j, None).unwrap();
        let log = coherent_elastic_thinning_error(&ce, &grid, j, Some(4)).unwrap();
        eprintln!(
            "{:>5} K: LI=2 {:.3} %  LI=4 {:.3} %  ratio {:.2}",
            TABULATED_K[j],
            100.0 * lin.max_rel,
            100.0 * log.max_rel,
            lin.max_rel / log.max_rel
        );
        assert!(
            log.max_rel < lin.max_rel,
            "{} K: log-space must win on the whole table",
            TABULATED_K[j]
        );
        assert!(
            (1.8..2.6).contains(&(lin.max_rel / log.max_rel)),
            "{} K: measured 2026-08-13 as roughly a halving, got {:.2}x",
            TABULATED_K[j],
            lin.max_rel / log.max_rel
        );
    }
    // The two end points of the measured table.
    let g1 = ThinnedTemperatureGrid::leave_one_out(TABULATED_K.len(), 1).unwrap();
    near(
        100.0
            * coherent_elastic_thinning_error(&ce, &g1, 1, Some(4))
                .unwrap()
                .max_rel,
        1.99,
        0.02,
        "400 K log-space",
    );
    let g8 = ThinnedTemperatureGrid::leave_one_out(TABULATED_K.len(), 8).unwrap();
    near(
        100.0
            * coherent_elastic_thinning_error(&ce, &g8, 8, Some(4))
                .unwrap()
                .max_rel,
        3.17,
        0.02,
        "1600 K log-space",
    );
}

/// (4) Thinned-grid `σ_coh(0.0253 eV)`: grid A stays under 0.74 % across
/// 293–1000 K, with 1.42 % at its worst (1600 K). Measured 2026-08-13.
#[test]
fn coherent_elastic_thinning_is_cheap_at_thermal_energies() {
    let Some(ce) = load_coherent() else { return };
    let grid = ThinnedTemperatureGrid::from_kept_temperatures(&TABULATED_K, &GRID_A).unwrap();
    // (withheld index, measured % error in σ_coh(0.0253 eV))
    let measured = [
        (1usize, 0.0907),
        (2, 0.1086),
        (4, 0.4152),
        (5, 0.6908),
        (6, 0.7393),
        (8, 1.42),
    ];
    for (j, pct) in measured {
        let t = TABULATED_K[j];
        let want = ce.cross_section(E_THERMAL, t).unwrap();
        let got = coherent_elastic_thinned_row(&ce, &grid, j, None)
            .unwrap()
            .cross_section(E_THERMAL, t)
            .unwrap();
        let err = 100.0 * (got - want).abs() / want;
        eprintln!("{t:>5} K: sigma_coh {want:.4} -> {got:.4} b ({err:.4} %)");
        near(err, pct, 0.03, &format!("{t} K sigma_coh error"));
        if t <= 1000.0 {
            assert!(
                err < 0.75,
                "{t} K: grid A must stay under 0.75 % below 1000 K"
            );
        }
    }
}

/// (5) The inelastic channel: much larger errors than the elastic one, and
/// they are concentrated at high incident energy. Grid A at 400 K —
/// `σ_inel(0.0253 eV)` 0.6960 → 0.6107 b (12.26 %), `σ_inel(3.9 eV)`
/// 4.6367 → 2.0459 b (55.88 %). Measured 2026-08-13.
#[test]
fn inelastic_thinning_error_is_large_and_worst_at_high_energy() {
    let Some(tape) = load_tape() else { return };
    let stack = SabTemperatureStack::from_tape(&tape, MAT).unwrap();
    assert_eq!(stack.temperatures_k, TABULATED_K.to_vec());
    near(stack.max_s(), 3.979191, 1e-5, "max tabulated S");

    let grid = ThinnedTemperatureGrid::from_kept_temperatures(&TABULATED_K, &GRID_A).unwrap();
    let approx = stack.thinned_kernel(&grid, 1, None).unwrap();
    let natom = 1.0; // graphite B(6) = 1

    for (e, want, got_measured) in [(E_THERMAL, 0.69597, 0.6107), (3.9, 4.63671, 2.0459)] {
        let reference = stack.kernels[1].cross_section(e, 400.0, natom);
        let got = approx.cross_section(e, 400.0, natom);
        eprintln!("E = {e} eV, 400 K: sigma_inel {reference:.5} -> {got:.5} b");
        near(
            reference,
            want,
            1e-3,
            &format!("tabulated sigma_inel({e} eV)"),
        );
        near(
            got,
            got_measured,
            0.02,
            &format!("thinned sigma_inel({e} eV)"),
        );
    }

    // The reconstruction is monotone: the interpolated table's σ lies between
    // the two bracketing tables' σ evaluated at the same physical temperature.
    for e in [E_THERMAL, 0.1, 3.9] {
        let lo = stack.kernels[0].cross_section(e, 400.0, natom);
        let hi = stack.kernels[3].cross_section(e, 400.0, natom);
        let mid = approx.cross_section(e, 400.0, natom);
        let (min, max) = if lo < hi { (lo, hi) } else { (hi, lo) };
        assert!(
            mid >= min - 1e-9 && mid <= max + 1e-9,
            "E = {e} eV: interpolated {mid} must lie in [{min}, {max}]"
        );
    }

    // The S(α,β) table error above the significance floor, grid A at 400 K.
    let floor = 1.0e-6 * stack.max_s();
    let sab = stack.thinning_error(&grid, 1, None, floor).unwrap();
    eprintln!(
        "S(a,b) >= 1e-6 S_max: max {:.2} % rms {:.2} % over {} points ({} skipped)",
        100.0 * sab.max_rel,
        100.0 * sab.rms_rel,
        sab.n_compared,
        sab.n_skipped
    );
    near(100.0 * sab.max_rel, 68.45, 0.02, "S(a,b) max at 400 K");
    near(100.0 * sab.rms_rel, 26.12, 0.02, "S(a,b) rms at 400 K");
}

/// (6) The decision number: `σ_total(0.0253 eV) = σ_coh + σ_inel` with both
/// channels thinned on the same grid. Grid A costs 1.60–3.13 % across
/// 293–1000 K; grid D costs 0.73–1.72 %. Neither reaches 1 %; both are well
/// inside 5 %. Measured 2026-08-13.
#[test]
fn combined_thermal_cross_section_error_decides_the_trade() {
    let Some(tape) = load_tape() else { return };
    let ce = parse_mf7(&tape, MAT).unwrap().coherent_elastic.unwrap();
    let stack = SabTemperatureStack::from_tape(&tape, MAT).unwrap();
    let natom = 1.0;

    let total_error = |keep: &[f64], j: usize| -> f64 {
        let grid = ThinnedTemperatureGrid::from_kept_temperatures(&TABULATED_K, keep).unwrap();
        let t = TABULATED_K[j];
        let coh_ref = ce.cross_section(E_THERMAL, t).unwrap();
        let coh_got = coherent_elastic_thinned_row(&ce, &grid, j, None)
            .unwrap()
            .cross_section(E_THERMAL, t)
            .unwrap();
        let inel_ref = stack.kernels[j].cross_section(E_THERMAL, t, natom);
        let inel_got = stack
            .thinned_kernel(&grid, j, None)
            .unwrap()
            .cross_section(E_THERMAL, t, natom);
        100.0 * ((coh_got + inel_got) - (coh_ref + inel_ref)).abs() / (coh_ref + inel_ref)
    };

    // Grid A, every withheld point in 293–1000 K. (index, measured %)
    for (j, pct) in [(1usize, 1.60), (2, 1.79), (4, 1.71), (5, 2.85), (6, 3.13)] {
        let err = total_error(&GRID_A, j);
        eprintln!(
            "grid A, {:>5} K: sigma_total error {err:.2} %",
            TABULATED_K[j]
        );
        near(
            err,
            pct,
            0.03,
            &format!("grid A total error at {} K", TABULATED_K[j]),
        );
        assert!(err < 5.0, "grid A must meet a 5 % criterion below 1000 K");
        assert!(
            err > 1.0,
            "grid A does NOT meet a 1 % criterion — this assert records that finding"
        );
    }

    // Grid D, every withheld point in 293–1000 K.
    for (j, pct) in [(2usize, 0.7276), (4, 1.22), (5, 1.72)] {
        let err = total_error(&GRID_D, j);
        eprintln!(
            "grid D, {:>5} K: sigma_total error {err:.2} %",
            TABULATED_K[j]
        );
        near(
            err,
            pct,
            0.03,
            &format!("grid D total error at {} K", TABULATED_K[j]),
        );
        assert!(err < 2.0, "grid D meets a 2 % criterion below 1000 K");
    }

    // Worked example from the module docs.
    let t = 400.0;
    let coh_ref = ce.cross_section(E_THERMAL, t).unwrap();
    let inel_ref = stack.kernels[1].cross_section(E_THERMAL, t, natom);
    near(coh_ref, 4.3731, 1e-3, "sigma_coh(0.0253 eV, 400 K)");
    near(inel_ref, 0.69597, 1e-3, "sigma_inel(0.0253 eV, 400 K)");
    near(
        coh_ref + inel_ref,
        5.0691,
        1e-3,
        "sigma_total(0.0253 eV, 400 K)",
    );
    assert!(
        inel_ref / (coh_ref + inel_ref) < 0.15,
        "inelastic is a small share of the thermal total — the reason a 12 % \
         inelastic error becomes a 1.6 % total error"
    );
}

/// (7) **Pre-existing defect, not a thinning result.** `σ_inel(E, T)` is
/// monotone in `T` across the tabulated grid, yet the *production*
/// interpolation at a non-tabulated temperature leaves the tabulated bracket
/// above ~0.5 eV — 4.4175 b at 393.15 K / 3.9 eV against a bracket of
/// [4.6097, 4.6367]. Correctly bracketed at and below 0.1 eV. Measured
/// 2026-08-13; reported, not fixed.
#[test]
fn production_interpolation_leaves_the_bracket_above_half_an_ev() {
    let Some(tape) = load_tape() else { return };
    let stack = SabTemperatureStack::from_tape(&tape, MAT).unwrap();
    let natom = 1.0;

    let interp = parse_mf7_at_temperature(&tape, MAT, Some(393.15))
        .unwrap()
        .incoherent_inelastic
        .unwrap();

    // In bracket at and below 0.1 eV.
    for e in [1.0e-3, 5.0e-3, E_THERMAL, 0.1] {
        let lo = stack.kernels[0].cross_section(e, 296.0, natom);
        let hi = stack.kernels[1].cross_section(e, 400.0, natom);
        let x = interp.cross_section(e, 393.15, natom);
        assert!(
            x >= lo - 1e-9 && x <= hi + 1e-9,
            "E = {e} eV: {x} should be inside [{lo}, {hi}]"
        );
    }

    // Out of bracket at 3.9 eV — the defect.
    let lo = stack.kernels[0].cross_section(3.9, 296.0, natom);
    let hi = stack.kernels[1].cross_section(3.9, 400.0, natom);
    let x = interp.cross_section(3.9, 393.15, natom);
    eprintln!("393.15 K, 3.9 eV: {x:.4} b vs bracket [{lo:.4}, {hi:.4}]");
    near(lo, 4.60966, 1e-3, "sigma_inel(3.9 eV, 296 K)");
    near(hi, 4.63671, 1e-3, "sigma_inel(3.9 eV, 400 K)");
    near(
        x,
        4.4175,
        0.01,
        "sigma_inel(3.9 eV, 393.15 K), production path",
    );
    assert!(
        x < lo,
        "the defect: the interpolated value falls BELOW its bracket"
    );
    assert!(
        (lo - x) / lo > 0.03,
        "the shortfall was 4.2 % on 2026-08-13, not a rounding artefact"
    );
}

/// (8) Byte accounting. The record model reproduces the real MT=4 section
/// exactly (8 695 236 B = 99.59 % of the 8 730 804 B tape, measured
/// 2026-08-13), so the per-grid savings are arithmetic, not estimates.
#[test]
fn byte_savings_per_grid() {
    let full = mf7_mt4_endf_bytes(10, 400, 150);
    assert_eq!(full, 8_695_236, "full 10-temperature MT=4 section");
    assert_eq!(mf7_mt4_endf_bytes(3, 400, 150), 3_162_208, "3 temperatures");
    assert_eq!(mf7_mt4_endf_bytes(4, 400, 150), 3_952_684, "4 temperatures");
    assert_eq!(mf7_mt4_endf_bytes(5, 400, 150), 4_743_084, "5 temperatures");
    assert_eq!(mf7_mt4_endf_bytes(6, 400, 150), 5_533_484, "6 temperatures");

    // Against the real tape when it is present.
    let Some(p) = tape_path() else { return };
    let on_disk = std::fs::metadata(&p).unwrap().len();
    assert_eq!(on_disk, 8_730_804, "tsl-crystalline-graphite.endf size");
    let share = full as f64 / on_disk as f64;
    assert!(
        (0.9955..0.9965).contains(&share),
        "MT=4 is 99.59 % of the tape, got {:.4} %",
        100.0 * share
    );
}
