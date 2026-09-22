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
//! matched **by energy**, never by index — this port emits 568 edges where NJOY
//! keeps 309, and differencing by position would repeat the 1753x artefact that
//! shaped the continuous-energy comparator.
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

/// Worst relative difference permitted on the inelastic cross section.
/// Measured 2.238e-2 on 2026-09-20.
const INEL_TOL: f64 = 5.0e-2;
/// Worst relative difference permitted on the coherent-elastic cumulative `S`.
/// Measured 3.253e-6 on 2026-09-20.
const BRAGG_TOL: f64 = 1.0e-4;

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
        if want != 0.0 {
            let d = ((got - want) / want).abs();
            if d > worst_s.0 {
                worst_s = (d, e);
            }
        }
    }
    assert_eq!(
        matched,
        oracle.bragg.len(),
        "every NJOY Bragg edge must be present in ours to 1e-6 relative; found \
         {matched} of {}. This port emits more edges than NJOY keeps (568 vs 309 \
         on 2026-09-20), which is a thinning difference — a MISSING edge is not.",
        oracle.bragg.len()
    );
    assert!(
        worst_s.0 <= BRAGG_TOL,
        "coherent-elastic cumulative S drifted from NJOY2016: worst {:.3e} at \
         {:.6e} MeV (recorded 3.253e-6 on 2026-09-20, gate {BRAGG_TOL:.0e})",
        worst_s.0,
        worst_s.1
    );
}
