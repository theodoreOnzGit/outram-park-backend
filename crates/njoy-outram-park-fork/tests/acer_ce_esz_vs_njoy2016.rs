//! **V&V — this port's continuous-energy ACE against NJOY2016's own.**
//!
//! NJOY2016 is the specification. This test fails if the port's RECONR + ACER
//! path stops reproducing it, and it is the gate that makes that a *regression*
//! rather than something noticed by hand months later.
//!
//! # The oracle, and why it is a slice
//!
//! NJOY2016's 0 K U-235 ACE table is **316 MB** and cannot live in this
//! repository. The rows that matter can. `reference-data/acer/u235_0k_esz_njoy2016.csv`
//! holds NJOY's ESZ values at the **2463 grid energies this port independently
//! chose as well** — the interpolation-free subset — so the comparison needs no
//! 316 MB dependency, no gzip, and no interpolation.
//!
//! Same shape as `tests/acer_acesix_vs_njoy2016_ace.rs`, which likewise keeps a
//! representative slice of its own comparison rather than the whole file.
//!
//! Regenerate it with:
//!
//! ```bash
//! # 293.6 K production set and the matched 0 K oracle:
//! #   outram-mc-libs/verification_and_validation/openmc_godiva_cross_code/make_ace_0k.sh
//! cargo run --release -p njoy-outram-park-fork --example ace_vs_njoy2016 -- \
//!     <njoy_0k_ace> --dump-oracle reference-data/acer/u235_0k_esz_njoy2016.csv
//! ```
//!
//! # The comparison is MATCHED, and that is the whole design
//!
//! This port's ACER path runs **RECONR only, at 0 K**. The production deck runs
//! **RECONR → BROADR(293.6 K) → PURR → ACER**. Comparing those would confound
//! Doppler broadening, unresolved probability tables and any genuine ACER
//! difference at once, and a disagreement could be attributed to none of them.
//! The oracle is therefore a **0 K, RECONR→ACER-only** NJOY run. Broadening and
//! probability tables are gated separately, by
//! `tests/broadr_light_nuclide_pendf_golden.rs` and
//! `tests/purr_u238_ptables_vs_njoy.rs`.
//!
//! An earlier draft of the comparison diffed `xss[i]` against `xss[i]` across
//! the two grids, which are **different lengths** (ours 237 049 points, NJOY's
//! 233 515) — so index `i` was a different *energy* in each file. It reported
//! absorption differing by a factor of 1753, entirely an artefact. This test
//! matches on **energy**, by exact `f64` equality, which is why the oracle
//! carries 17 significant digits.
//!
//! # Results (2026-09-20, ENDF/B-VIII.0 U-235, NJOY2016 2016.79 `ac5adf5`)
//!
//! | quantity | worst relative difference |
//! |---|---|
//! | total | 4.697e-7 |
//! | absorption | 1.527e-6 |
//! | elastic | 8.058e-7 |
//!
//! **Runtime: 88.5 s for both tests** (RECONR on the 36 MB U-235 tape,
//! measured 2026-09-20 on 4 cores). That is under the workspace's 5-minute
//! threshold, so these are deliberately NOT behind `long-tests` — an earlier
//! draft gated them on an assumed ~3 min that was never measured.
//!
//! ~6–7 significant figures. **Gated at 1e-5**, a 6.5x margin over the worst
//! measured value — tight enough to catch a real regression, loose enough not
//! to fail on an unrelated grid refinement. The measured numbers are recorded
//! here so the margin is visible rather than implied; **do not widen the gate
//! to absorb a failure.**

use njoy_outram_park_fork::{
    acer::{angular::parse_elastic_angular, energy::build_emissions, jxs, nxs, AceTable},
    endf::tape::Tape,
    heatr::{build_emission_spectra, Kerma},
    nuclear_data::secondary::{FissionSpectrum, NuBar},
    photon::PhotonProduction,
    reconr::{reconr, ReconrConfig},
    reference_data::reference_file_or_skip,
};

const MAT: i32 = 9228;
/// Gate. Worst measured 2026-09-20 was 1.527e-6 (absorption); see module docs.
const TOL: f64 = 1.0e-5;

/// NJOY2016's own MTR block for this evaluation, ascending.
///
/// Read directly out of `reference-data/ace/reference-njoy/endf-b-viii.0/0K/
/// U235.ace.gz` (NJOY2016 2016.79 `ac5adf5`) on 2026-09-20 — the same file the
/// ESZ oracle above was dumped from. Inlined rather than parsed at test time
/// because that table is 316 MB uncompressed and this is 84 integers.
///
/// MT=649 is the (n,p) continuum level and MT=800-835 the (n,α) discrete
/// levels; those 37 were the gap this port carried until 2026-09-20.
const NJOY_MTR_U235_0K: [i32; 84] = [
    5, 16, 17, 18, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70,
    71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 91, 102, 103, 107,
    649, 800, 801, 802, 803, 804, 805, 806, 807, 808, 809, 810, 811, 812, 813, 814, 815, 816, 817,
    818, 819, 820, 821, 822, 823, 824, 825, 826, 827, 828, 829, 830, 831, 832, 833, 834, 835,
];

/// One oracle row: NJOY2016's ESZ at an energy this port also has on its grid.
struct Row {
    e: f64,
    total: f64,
    absorption: f64,
    elastic: f64,
}

fn oracle() -> Vec<Row> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../reference-data/acer/u235_0k_esz_njoy2016.csv"
    );
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read oracle {path}: {e}"));
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("energy_mev"))
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Vec<f64> = l
                .split(',')
                .map(|t| t.parse().expect("oracle number"))
                .collect();
            assert_eq!(v.len(), 4, "oracle row {l:?}");
            Row {
                e: v[0],
                total: v[1],
                absorption: v[2],
                elastic: v[3],
            }
        })
        .collect()
}

fn build_ours() -> AceTable {
    let path = reference_file_or_skip(
        "endf",
        "n-092_U_235-ENDF8.0.endf",
        "U-235 evaluation (ACER vs NJOY2016)",
    )
    .expect("tape present");
    let tape = Tape::read(std::fs::File::open(&path).expect("open")).expect("parse ENDF");
    let cfg = ReconrConfig {
        mat: MAT,
        tolerance: 0.001,
        temperature: 0.0,
    };
    let result = reconr(&tape, &cfg).expect("RECONR");
    let angular = tape
        .section(MAT, 4, 2)
        .map(|s| parse_elastic_angular(s).expect("parse MF=4"));
    let partials: Vec<(i32, f64)> = result
        .sections
        .iter()
        .map(|s| (i32::from(s.mt), s.qi))
        .collect();
    let emissions = build_emissions(&tape, MAT, result.material.awr, &partials);
    let nu = NuBar::from_endf(&tape, MAT)
        .expect("MF=1")
        .unwrap_or_default();
    let chi = FissionSpectrum::from_endf_mf5(&tape, MAT)
        .expect("MF=5")
        .unwrap_or_default();
    let emission = build_emission_spectra(&tape, MAT);
    let photons = PhotonProduction::from_endf(&tape, MAT, &result);
    let kerma =
        Kerma::from_reconr(&result, &nu, &chi, &emission).with_energy_balance(&photons, &result);
    // The ACE NU block (fission nu-bar); None for a non-fissile nuclide.
    let nu_block = njoy_outram_park_fork::acer::nu::build(&tape, MAT).expect("NU block");
    AceTable::from_reconr_full(
        &result,
        0.0,
        0,
        angular.as_ref(),
        &emissions,
        Some(&kerma),
        nu_block.as_deref(),
        njoy_outram_park_fork::acer::has_mt19_distributions(&tape, MAT),
        njoy_outram_park_fork::acer::photon_blocks::build(&tape, MAT).as_deref(),
    )
}

/// The ESZ cross sections reproduce NJOY2016's at every energy both grids hold.
#[test]
fn esz_cross_sections_match_njoy2016_at_shared_grid_energies() {
    let rows = oracle();
    assert!(
        rows.len() > 2000,
        "oracle has only {} rows; it should carry ~2463. A truncated oracle \
         would let this test pass on almost nothing.",
        rows.len()
    );

    let ours = build_ours();
    let nes = ours.nxs[nxs::NES] as usize;
    let base = (ours.jxs[jxs::ESZ] - 1) as usize;
    let e = &ours.xss[base..base + nes];

    let mut worst = [(0.0f64, 0.0f64); 3];
    let mut matched = 0usize;
    for r in &rows {
        // Exact equality: both sides parsed the same 12-digit ACE decimal, and
        // the oracle is written with 17 significant digits so it round-trips.
        // A binary search that misses means the grid moved, which is itself a
        // finding -- so it is asserted rather than skipped.
        let i = e
            .binary_search_by(|p| p.partial_cmp(&r.e).unwrap())
            .unwrap_or_else(|_| {
                panic!(
                    "energy {:.17e} MeV is in NJOY's grid and in the oracle, but no longer \
                     in this port's. The reconstruction grid moved; regenerate the oracle \
                     only after establishing WHY.",
                    r.e
                )
            });
        for (k, (want, got)) in [
            (r.total, ours.xss[base + nes + i]),
            (r.absorption, ours.xss[base + 2 * nes + i]),
            (r.elastic, ours.xss[base + 3 * nes + i]),
        ]
        .into_iter()
        .enumerate()
        {
            if want == 0.0 {
                continue;
            }
            let d = ((got - want) / want).abs();
            if d > worst[k].0 {
                worst[k] = (d, r.e);
            }
        }
        matched += 1;
    }

    let names = ["total", "absorption", "elastic"];
    for (k, (d, at)) in worst.iter().enumerate() {
        println!(
            "acer vs NJOY2016: {:<11} worst rel {d:.3e} at {at:.6e} MeV over {matched} energies",
            names[k]
        );
        assert!(
            *d < TOL,
            "{} differs from NJOY2016 by {d:.3e} at {at:.6e} MeV, over the {TOL:.0e} gate. \
             NJOY is the specification -- this is a defect in the port until shown otherwise. \
             DO NOT widen the gate: the worst value measured when this was written was \
             1.527e-6, so a failure here is a real change, not drift.",
            names[k]
        );
    }
}

/// The blocks this port does **not** write are pinned, so neither their absence
/// nor their arrival goes unnoticed.
///
/// A missing block and a matching block are indistinguishable in a naive diff,
/// and `JXS(2) = 0` reads as "agrees" to anything that only compares numbers.
/// Pinning it means the day someone implements ν̄ output, this test says so
/// instead of silently continuing to pass.
///
/// **It worked.** The reaction-count pin fired on 2026-09-20 when MT=649 and
/// MT=800-835 were added, and refused to be updated without establishing what
/// had changed. That assertion is now a set comparison against NJOY's own
/// MTR block rather than a bare count.
#[test]
fn the_unwritten_ace_blocks_are_still_the_ones_we_think() {
    let ours = build_ours();

    // CLOSED 2026-09-20. This pin fired as designed when the NU block landed.
    // It is now the opposite assertion: the block must be present AND must
    // reproduce NJOY2016's own, value for value.
    assert_ne!(ours.jxs[jxs::NU], 0, "the NU block disappeared again");

    let nu_oracle: Vec<f64> = {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../reference-data/acer/u235_0k_nu_njoy2016.csv"
        );
        let text =
            std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read NU oracle {path}: {e}"));
        text.lines()
            .filter(|l| !l.starts_with('#') && *l != "xss" && !l.trim().is_empty())
            .map(|l| l.trim().parse::<f64>().expect("NU oracle value"))
            .collect()
    };
    let nu_start = (ours.jxs[jxs::NU] - 1) as usize;
    let ours_nu = &ours.xss[nu_start..nu_start + nu_oracle.len()];
    assert_eq!(
        ours_nu.len(),
        nu_oracle.len(),
        "NU block length moved off NJOY2016's 347"
    );
    let mismatches: Vec<(usize, f64, f64)> = ours_nu
        .iter()
        .zip(&nu_oracle)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, (a, b))| (i, *a, *b))
        .collect();
    assert!(
        mismatches.is_empty(),
        "the NU block was BIT-IDENTICAL to NJOY2016's on 2026-09-20 and no longer is. \
         {} of {} values differ; first three: {:?}. Exact equality is asserted \
         deliberately -- this block is copied from the evaluation with one unit \
         change, so anything other than bit-identical means a real change in how \
         it is read or written, not rounding.",
        mismatches.len(),
        nu_oracle.len(),
        &mismatches[..mismatches.len().min(3)]
    );
    println!(
        "acer vs NJOY2016: NU block {} values, BIT-IDENTICAL",
        nu_oracle.len()
    );
    // CLOSED 2026-09-20 — the last of the three pinned gaps. This asserted
    // NXS(6)==0 and fired when the photon blocks landed; it is now a positive
    // check of the whole MTRP block against NJOY's own, in NJOY's own ORDER.
    //
    // Order is asserted, not just the set, because LSIGP and LDLWP are
    // positional: a table with the right MTs in the wrong order pairs every
    // entry with the wrong cross section and the wrong distribution. That
    // exact bug occurred while this was being written.
    let mtrp_oracle: Vec<i32> = {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../reference-data/acer/u235_0k_mtrp_njoy2016.csv"
        );
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read MTRP oracle {path}: {e}"));
        text.lines()
            .filter(|l| !l.starts_with('#') && *l != "mtrp" && !l.trim().is_empty())
            .map(|l| l.trim().parse::<i32>().expect("MTRP oracle value"))
            .collect()
    };
    let ntrp = ours.nxs[nxs::NTRP] as usize;
    assert_eq!(
        ntrp,
        mtrp_oracle.len(),
        "NXS(6) moved off NJOY2016's {} photon-production entries",
        mtrp_oracle.len()
    );
    let pb = ours.jxs[jxs::MTRP];
    assert!(pb > 0, "NXS(6) is {ntrp} but JXS(13) is 0");
    let ours_mtrp: Vec<i32> = (0..ntrp)
        .map(|i| ours.xss[(pb - 1) as usize + i].round() as i32)
        .collect();
    assert_eq!(
        ours_mtrp,
        mtrp_oracle,
        "the MTRP block no longer matches NJOY2016's, in content or in order. \
         It matched exactly on 2026-09-20 at {} entries.",
        mtrp_oracle.len()
    );

    // CLOSED 2026-09-20. This was pinned at 47 against NJOY's 84; the gap was
    // MT=649 and MT=800-835, the discrete charged-particle levels, which
    // `role_of`'s `m >= 251` catch-all had been sweeping up as "derived".
    // Upstream stores them in a second pass over MF=3 and keeps them out of
    // the ESZ sums when the lumped MT=103/107 is present (`acefc.f90` ~5536
    // and ~5652). Both halves are implemented; this now asserts the set, not
    // just the count, because a count can match while the members differ.
    let ntr = ours.nxs[nxs::NTR] as usize;
    let m = ours.jxs[jxs::MTR];
    assert!(m > 0, "MTR block is absent");
    let base = (m - 1) as usize;
    let mut ours_mts: Vec<i32> = (0..ntr)
        .map(|i| ours.xss[base + i].round() as i32)
        .collect();
    ours_mts.sort_unstable();
    let njoy_mts: Vec<i32> = NJOY_MTR_U235_0K.to_vec();
    assert_eq!(
        ours_mts, njoy_mts,
        "the MTR reaction set no longer matches NJOY2016's exactly. It matched \
         on 2026-09-20 at 84 MTs. Compare the two lists before touching this -- \
         a count that still reads 84 while the members differ is the failure \
         this assertion exists to catch."
    );
    assert_eq!(ntr, 84, "reaction count moved off NJOY2016's 84");
    println!(
        "acer vs NJOY2016: NTRP {} matching NJOY's MTRP exactly, {ntr} reactions \
         matching NJOY's MTR set exactly",
        ntrp
    );
}
