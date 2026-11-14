//! Gate: the thermal S(alpha,beta) ACE table against NJOY2016, on Al-27.
//!
//! # Why this exists
//!
//! Until 2026-09-20 the nine `tsl-*.endf` tapes had **no comparison against
//! NJOY at all**. `tests/thermal_ace.rs` and `tests/thermal_ace_zrh.rs` assert
//! structure and sign only — `sigma_inel >= 0`, "not all zero", ascending grid,
//! block lengths — so the thermal **inelastic cross section had never been
//! checked against any reference value**, and neither had the coherent-elastic
//! cumulative `S`. `examples/thermal_ace_vs_njoy2016` measured them; this test
//! is what stops the result regressing.
//!
//! # Methodology
//!
//! Oracle: `reference-data/acer/al27_20k_thermal_njoy2016.csv`, extracted from
//! an NJOY2016 2016.79 table built by `reconr` -> `broadr` -> `thermr` ->
//! `acer` with `iopt = 2` and **`iwt = 1`** from `tsl-013_Al_027-ENDF8.0.endf`
//! paired with `n-013_Al_027-ENDF8.0.endf`.
//!
//! Two choices are load-bearing and each was got wrong first:
//!
//! - **`iwt = 1` is required.** `aceth.f90:674-676` sets `ifeng = 0` only for
//!   `iwt = 1`; the default `iwt = 0` produces the skewed `IFENG = 1` form,
//!   which this port does not write. An `IFENG = 1` reference would be a
//!   different representation, not a stricter oracle.
//! - **20 K, not 293.6 K.** That is this evaluation's own MF=7 base
//!   temperature, and `IncoherentInelastic` carries `S(alpha,beta)` at the base
//!   temperature only. A first run asked both sides for 293.6 K and produced a
//!   ~3000x "disagreement" that was purely the temperature mismatch.
//!
//! Ours is built on **NJOY's own incident-energy grid**, and Bragg edges are
//! matched **by energy**, never by index — ~~this port emits 568 edges where NJOY
//! keeps 309~~ **CORRECTED 2026-10-14 (GitHub #865):** it now keeps NJOY's 309,
//! with ACER's own selection; differencing by position would repeat the 1753x
//! artefact that shaped the continuous-energy comparator.
//!
//! This test builds **without** the incident nuclide's PENDF, so its ITIX is
//! not on NJOY's grid; the whole-table, word-for-word gate (with it) is
//! `tests/acer_thermal_word_parity.rs`.
//!
//! # Results (2026-09-20, NJOY2016 2016.79 `ac5adf5`)
//!
//! | quantity | measured | gate |
//! |---|---|---|
//! | NXS `IDPNI`/`NIL`/`NIEB`/`IDPNC`/`NCL`/`IFENG` | all equal | exact |
//! | inelastic `sigma` worst rel, 106 points | **2.238e-2** | 5e-2 |
//! | coherent-elastic cumulative `S` worst rel, 309 matched edges | **3.253e-6** | 1e-4 |
//! | NJOY Bragg edges found in ours to 1e-6 | **309 of 309** | all |
//!
//! The inelastic gate is deliberately looser than the elastic one because the
//! measured agreement is genuinely looser, not because a tighter one failed.
//!
//! # Re-measured 2026-10-14 (GitHub #865)
//!
//! | quantity | measured | gate |
//! |---|---|---|
//! | inelastic `sigma` worst rel, 106 points | **1.081e-3** (at 1.417e-8 MeV) | 5e-2 |
//! | coherent-elastic cumulative `S` worst rel, 309 matched edges | **4.381e-10** | 1e-4 |
//! | NJOY Bragg edges found in ours to 1e-6 | **309 of 309**, ours 309 | all |
//!
//! The 2.238e-2 above came from the port's own analytic inelastic integral and
//! a `sig` kernel that differed from upstream's; the remaining 1.1e-3 is the
//! ITIX interpolation on the incident PENDF grid, which this test does not
//! supply. The gates were not moved.
//!
//! # Results (2026-11-14, gh:#1366 §2 item 3)
//!
//! **The oracle is NJOY's `tol = 0.05` table.** Every ITIE/ITIX/ITCE/ITCX
//! value in `al27_20k_thermal_njoy2016.csv` equals
//! `al27_20k_thermal_tol0p05_njoy2016.ace.gz`'s at the CSV's 10 figures
//! (106/106, 309/309; against the tol 0.001 table 106 of 106 ITIX differ),
//! the table `acer_thermal_word_parity` holds identical in every word.
//!
//! | quantity | route | measured | gate |
//! |---|---|---|---|
//! | coherent `S`, 309 edges | no incident PENDF | 4.381e-10 unprinted; **0 differing as printed** | equality as printed; ~~1e-4~~ 5.05e-10 unprinted |
//! | inelastic `sigma`, 106 points | no incident PENDF | 1.081e-3 | ~~5e-2~~ **1.1e-3** |
//! | inelastic `sigma`, 106 points | on the incident PENDF, `tol = 0.05` | **0 differing as printed** | equality as printed |
//!
//! Causes, each checked: the coherent `S` residual is the CSV's own print (a
//! 10-figure value is within 5e-10 of the number it rounds); compared as the
//! CSV was made (the ACE file's 12 figures, then 10, see `csv_printed`)
//! nothing differs. Compared in one stage, straight to 10 figures, 1 of 309
//! differed, at a double-rounding tie (1.503608e-7 MeV); that was not in the
//! prediction. The inelastic residual is the route: without the
//! incident PENDF, ITIX is `calcem`'s value at each energy through `tpend`'s
//! text, where ACER takes `gety1` of THERMR's MF=3 on the PENDF grid
//! (`aceth.f90:355`). Supplying the PENDF removes all of it (third row), so the
//! cause predicts the size exactly. The no-PENDF route has no NJOY2016
//! counterpart (`thermr::on_pendf`'s module doc), so its gate stays at the
//! measured value. Predicted on gh:#1366 before the run, and held.

use std::fs::File;

use njoy_outram_park_fork::{
    acer::{
        thermal::{jxs, nxs, ThermalAceOptions},
        AceTable,
    },
    endf::tape::Tape,
    thermr::mf7::parse_mf7,
};

const MAT: i32 = 53;
const T0_K: f64 = 20.0;
const EMEV: f64 = 1.0e6;

/// Worst relative difference permitted on the inelastic cross section **on
/// the route without the incident PENDF**. Measured 2.238e-2 on 2026-09-20,
/// 1.081e-3 on 2026-10-14 and 2026-11-14. ~~5e-2~~ **CORRECTED 2026-11-14
/// (gh:#1366):** 1.1e-3, the measured value rounded up in its second figure.
/// The cause is the route (see "Results (2026-11-14)" in the module doc); the
/// same table on the incident PENDF is gated exactly by
/// [`al27_thermal_table_on_the_incident_pendf_is_the_oracles`].
const INEL_TOL: f64 = 1.1e-3;
/// Bound on the **unprinted** coherent-elastic cumulative `S`. Measured
/// 3.253e-6 on 2026-09-20 and 4.381e-10 on 2026-10-14 and 2026-11-14.
/// ~~1e-4~~ **CORRECTED 2026-11-14 (gh:#1366):** 5.05e-10, what may separate
/// a value from its print in the oracle: half a unit in the tenth figure
/// (5e-10 relative at most) plus half a unit in the ACE file's twelfth
/// (5e-12), see [`csv_printed`]. The gate itself is equality as printed.
const BRAGG_TOL: f64 = 5.05e-10;

/// A value as the oracle CSV holds it: printed by NJOY's Type-1 ACE writer
/// (`1pE20.11`, 12 figures), read back, and printed again as `%.9e`
/// (10 figures). The two stages are both needed: rounding straight to 10
/// figures differs at one Bragg edge, `E = 1.503608e-7` MeV, where this
/// port's `S` is `1.4070659915048e-7`, NJOY's file holds `1.40706599150E-07`,
/// and the CSV `1.407065991e-07` (the 12-figure value sits just below the
/// 10-figure tie as an f64). Found 2026-11-14, after the prediction on
/// gh:#1366 had said "0 differing as printed" with the one-stage print.
fn csv_printed(x: f64) -> String {
    let ace: f64 = format!("{x:.11e}").parse().expect("a printed f64 parses");
    format!("{ace:.9e}")
}

struct Oracle {
    inel: Vec<(f64, f64)>,
    bragg: Vec<(f64, f64)>,
}

fn oracle() -> Option<Oracle> {
    let p = njoy_outram_park_fork::reference_data::reference_data_dir("acer")
        .join("al27_20k_thermal_njoy2016.csv");
    let text = std::fs::read_to_string(p).ok()?;
    let mut inel = Vec::new();
    let mut bragg = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split(',').collect();
        let (x, y) = (f[2].parse().ok()?, f[3].parse().ok()?);
        match f[0] {
            "inel" => inel.push((x, y)),
            "bragg" => bragg.push((x, y)),
            _ => {}
        }
    }
    Some(Oracle { inel, bragg })
}

#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "reference-data tier (NJOY2016 oracles); runs by default, skipped under --no-default-features"
)]
fn al27_thermal_table_matches_njoy2016() {
    // A skip must HONOUR the require flag, or this test joins the 19 that were
    // found on 2026-09-11 "passing in 0.00 s having asserted nothing". Proven
    // necessary the same day this test was written: without these two asserts it
    // reported `ok` in 0.00 s under OUTRAM_PARK_REQUIRE_REFERENCE_DATA=1, while
    // gaminr_vs_njoy2016 beside it correctly failed.
    let Some(oracle) = oracle() else {
        assert!(
            !njoy_outram_park_fork::reference_data::reference_data_required(),
            "[al27-thermal] reference-data/acer/al27_20k_thermal_njoy2016.csv absent              and OUTRAM_PARK_REQUIRE_REFERENCE_DATA is set"
        );
        println!("[al27-thermal] SKIP — oracle CSV not present");
        return;
    };
    let Some(tsl) =
        njoy_outram_park_fork::reference_data::reference_endf("tsl-013_Al_027-ENDF8.0.endf")
    else {
        assert!(
            !njoy_outram_park_fork::reference_data::reference_data_required(),
            "[al27-thermal] tsl-013_Al_027-ENDF8.0.endf absent and              OUTRAM_PARK_REQUIRE_REFERENCE_DATA is set"
        );
        println!("[al27-thermal] SKIP — tsl tape not present");
        return;
    };
    let tape = Tape::read(File::open(tsl).expect("open tsl")).expect("parse tsl");
    let mf7 = parse_mf7(&tape, MAT).expect("parse MF=7");

    // The oracle's own grid, in eV. Building on NJOY's grid is the whole point:
    // this port's thermal writer takes its grid from the caller, so using ours
    // would measure the grid choice rather than the physics.
    let grid_ev: Vec<f64> = oracle.inel.iter().map(|&(e, _)| e * EMEV).collect();
    let emax_ev = *grid_ev.last().expect("non-empty grid");

    let ace = AceTable::thermal_from_mf7(
        &mf7,
        T0_K,
        "al27",
        0,
        &grid_ev,
        ThermalAceOptions {
            n_outgoing: 16,
            n_cosines: 16, // NIL = 15
            natom: 1.0,
            emax_ev,
            ..Default::default()
        },
    )
    .expect("thermal ACE builds");

    // ---- NXS, exactly ----------------------------------------------------
    assert_eq!(ace.nxs[nxs::IDPNI], 3, "IDPNI");
    assert_eq!(ace.nxs[nxs::NIL], 15, "NIL");
    assert_eq!(ace.nxs[nxs::NIEB], 16, "NIEB");
    assert_eq!(ace.nxs[nxs::IDPNC], 4, "IDPNC — Al-27 is coherent elastic");
    assert_eq!(ace.nxs[nxs::NCL], -1, "NCL");
    assert_eq!(
        ace.nxs[nxs::IFENG],
        0,
        "IFENG — the oracle was built with iwt=1 so that this is 0; an IFENG=1 \
         reference would be a different representation, not a stricter test"
    );

    // ---- inelastic cross section, on NJOY's own grid ----------------------
    let itix = (ace.jxs[jxs::ITIX] - 1) as usize;
    let mut worst = (0.0f64, 0.0f64);
    for (k, &(e, want)) in oracle.inel.iter().enumerate() {
        let got = ace.xss[itix + k];
        if want != 0.0 {
            let d = ((got - want) / want).abs();
            if d > worst.0 {
                worst = (d, e);
            }
        }
    }
    assert!(
        worst.0 <= INEL_TOL,
        "thermal inelastic cross section drifted from NJOY2016: worst {:.3e} at \
         {:.6e} MeV (recorded 2.238e-2 on 2026-09-20, gate {INEL_TOL:.0e})",
        worst.0,
        worst.1
    );

    // ---- coherent elastic, matched BY ENERGY ------------------------------
    let itce = (ace.jxs[jxs::ITCE] - 1) as usize;
    let itcx = (ace.jxs[jxs::ITCX] - 1) as usize;
    let nee_ours = ace.xss[itce] as usize;
    let ours_e: Vec<f64> = (0..nee_ours).map(|k| ace.xss[itce + 1 + k]).collect();

    let mut matched = 0usize;
    let mut s_differing = 0usize;
    let mut worst_s = (0.0f64, 0.0f64);
    for &(e, want) in &oracle.bragg {
        let Some((j, _)) = ours_e
            .iter()
            .enumerate()
            .map(|(j, &oe)| (j, ((oe - e) / e).abs()))
            .filter(|&(_, d)| d <= 1.0e-6)
            .min_by(|a, b| a.1.partial_cmp(&b.1).expect("finite"))
        else {
            continue;
        };
        matched += 1;
        let got = ace.xss[itcx + j];
        if csv_printed(got) != csv_printed(want) {
            s_differing += 1;
            println!("[al27-thermal] S differs as printed at E={e:e} MeV: ours {got:.17e}, oracle {want:.17e}");
        }
        if want != 0.0 {
            let d = ((got - want) / want).abs();
            if d > worst_s.0 {
                worst_s = (d, e);
            }
        }
    }
    println!(
        "[al27-thermal] inelastic worst {:.3e} at {:.6e} MeV; {matched} of {} NJOY edges found, ours {nee_ours}; cumulative S worst {:.3e}, {s_differing} differing as the CSV prints",
        worst.0,
        worst.1,
        oracle.bragg.len(),
        worst_s.0
    );
    assert_eq!(
        matched,
        oracle.bragg.len(),
        "every NJOY Bragg edge must be present in ours to 1e-6 relative; found \
         {matched} of {}. This port emitted more edges than NJOY keeps (568 vs 309 \
         on 2026-09-20) until ACER's selection was ported (309 on 2026-10-14); \
         a MISSING edge was never acceptable.",
        oracle.bragg.len()
    );
    assert_eq!(
        s_differing, 0,
        "coherent-elastic cumulative S: {s_differing} of {matched} values differ from \
         NJOY2016's as the oracle prints them (10 figures); worst unprinted {:.3e} at \
         {:.6e} MeV. Identical on 2026-11-14 (gh:#1366).",
        worst_s.0,
        worst_s.1
    );
    assert!(
        worst_s.0 <= BRAGG_TOL,
        "coherent-elastic cumulative S drifted from NJOY2016: worst {:.3e} at \
         {:.6e} MeV (recorded 3.253e-6 on 2026-09-20, 4.381e-10 on 2026-10-14, \
         gate {BRAGG_TOL:.0e})",
        worst_s.0,
        worst_s.1
    );
}

/// **The same table on the incident PENDF reproduces the oracle exactly**
/// (gh:#1366 §2 item 3): the check that the 1.081e-3 inelastic residual of
/// [`al27_thermal_table_matches_njoy2016`] is entirely the missing PENDF grid.
///
/// Ours: RECONR (0.001) + BROADR (20 K) of `n-013_Al_027-ENDF8.0.endf`, then
/// `thermal_from_mf7_on_pendf` at `tol = 0.05` (the oracle's deck, see the
/// module doc) on the oracle's own incident grid. Every ITIX value and every
/// matched Bragg edge's `S` is compared as the CSV prints it (`%.9e`).
/// No tolerance. Result 2026-11-14: 0 of 106 and 0 of 309 differ.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "reference-data tier (NJOY2016 oracles); runs by default, skipped under --no-default-features"
)]
fn al27_thermal_table_on_the_incident_pendf_is_the_oracles() {
    use njoy_outram_park_fork::acer::thermal::IncidentPendf;
    use njoy_outram_park_fork::broadr::broaden_result;
    use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
    use njoy_outram_park_fork::reference_data::{reference_data_required, reference_endf};
    let (Some(oracle), Some(tsl), Some(inc)) = (
        oracle(),
        reference_endf("tsl-013_Al_027-ENDF8.0.endf"),
        reference_endf("n-013_Al_027-ENDF8.0.endf"),
    ) else {
        assert!(!reference_data_required(), "[al27-thermal-pendf] reference data absent");
        return;
    };
    let inc_tape = Tape::read_file(&inc).expect("incident tape");
    let r0 = reconr(&inc_tape, &ReconrConfig { mat: 1325, tolerance: 1.0e-3, temperature: 0.0 }).expect("RECONR");
    let r = broaden_result(&r0, T0_K);
    let elastic = r.sections.iter().find(|s| s.mt.number() == 2).expect("MT=2").pairs.clone();
    let incident = IncidentPendf { mat: 1325, awr: r.material.awr, moderator_za: vec![13027], elastic };
    let mf7 = parse_mf7(&Tape::read_file(&tsl).expect("tsl"), MAT).expect("MF=7");
    let grid_ev: Vec<f64> = oracle.inel.iter().map(|&(e, _)| e * EMEV).collect();
    let opts = ThermalAceOptions { n_outgoing: 16, n_cosines: 16, natom: 1.0, emax_ev: 4.0, ..Default::default() };
    let ace = AceTable::thermal_from_mf7_on_pendf(&mf7, T0_K, "al27", 0, &grid_ev, opts, 0.05, &incident)
        .expect("thermal ACE on the PENDF");
    let itix = (ace.jxs[jxs::ITIX] - 1) as usize;
    let inel_diff = oracle
        .inel
        .iter()
        .enumerate()
        .filter(|&(k, &(_, want))| csv_printed(ace.xss[itix + k]) != csv_printed(want))
        .count();
    let itce = (ace.jxs[jxs::ITCE] - 1) as usize;
    let itcx = (ace.jxs[jxs::ITCX] - 1) as usize;
    let ne = ace.xss[itce] as usize;
    assert_eq!(ne, oracle.bragg.len(), "Bragg edge count");
    let bragg_diff = oracle
        .bragg
        .iter()
        .enumerate()
        .filter(|&(k, &(e, s))| {
            csv_printed(ace.xss[itce + 1 + k]) != csv_printed(e) || csv_printed(ace.xss[itcx + k]) != csv_printed(s)
        })
        .count();
    println!(
        "[al27-thermal-pendf] ITIX {inel_diff} of {} differ, Bragg (E, S) {bragg_diff} of {ne} differ, as printed",
        oracle.inel.len()
    );
    assert_eq!(inel_diff, 0, "ITIX on the incident PENDF must be the oracle's as printed");
    assert_eq!(bragg_diff, 0, "the Bragg table must be the oracle's as printed");
}
