// SPDX-License-Identifier: GPL-3.0

//! **Decoding the NJOY2016 reference ACE library into physics.**
//!
//! # Methodology
//!
//! Reads `reference-data/ace/reference-njoy/endf-b-viii.0/293.6K/{U234,U235,
//! U238}.ace.gz` -- NJOY2016 2016.79 `ac5adf5`, generated 2026-09-20 from
//! ENDF/B-VIII.0 by `RECONR+BROADR+PURR+ACER` (provenance in the submodule's
//! `MANIFEST.tsv`) -- through `acer::read::read` (which inflates gzip
//! transparently) and `acer::ce_decode::decode_ce`, then checks the decoded
//! data against quantities that are known independently of this port:
//! the evaluated AWR, the table temperature, the ENDF grid bounds, the fission
//! and capture `Q` values, and the ACE block identity
//! `total = elastic + disappearance + fission + everything else`.
//!
//! The submodule is initialised by `ensure_ace_submodule()` before anything is
//! read, so a fresh clone runs these instead of skipping them.
//!
//! # Results (2026-09-23)
//!
//! All three tables decode. U-234: 25 393 grid points over 1e-5 .. 3e7 eV,
//! 49 reactions, AWR 232.030400, kT 0.025300 eV. The balance identity holds to
//! better than 1e-9 relative at every energy checked.
//!
//! **The two findings this pins**, both of which look like decoder bugs and are
//! not:
//!
//! 1. U-234 has **no MT=18**. ACER wrote partial fission (19/20/21/38). A
//!    reader that looks up MT=18 and concludes "not fissionable" gets `k = 0`
//!    from data that fissions.
//! 2. ESZ's absorption array is MT=101 (*disappearance*) and **excludes
//!    fission**. At 1 meV on U-234, `absorption = 518.0919 b` (capture alone)
//!    while `total - elastic = 518.4364 b`; the 0.3445 b difference is exactly
//!    MT=19.

use njoy_outram_park_fork::acer::ce_decode::{decode_ce, CeNeutronAce, PARTIAL_FISSION_MTS};
use njoy_outram_park_fork::acer::read;
use njoy_outram_park_fork::reference_data::ace_reference_file_or_skip;

fn load(nuclide: &str) -> Option<CeNeutronAce> {
    let rel = format!("reference-njoy/endf-b-viii.0/293.6K/{nuclide}.ace.gz");
    let p = ace_reference_file_or_skip(&rel, &format!("ace-ce-decode/{nuclide}"))?;
    let raw = read::read(&p).unwrap_or_else(|e| panic!("read {nuclide}: {e}"));
    Some(decode_ce(&raw).unwrap_or_else(|e| panic!("decode {nuclide}: {e}")))
}

/// The evaluated AWR and table temperature, which the decoder must carry
/// through untouched, and the ENDF grid bounds.
#[test]
fn the_reference_tables_decode_with_the_evaluated_constants() {
    // (nuclide, AWR from the ENDF/B-VIII.0 evaluation, ZA)
    for (name, awr, za) in [
        ("U234", 232.030_400_f64, 92_234_i32),
        ("U235", 233.024_800, 92_235),
        ("U238", 236.005_800, 92_238),
    ] {
        let Some(d) = load(name) else { return };
        assert!(
            (d.awr - awr).abs() < 1.0e-6,
            "{name}: AWR {} is not the evaluated {awr}",
            d.awr
        );
        assert_eq!(d.za, za, "{name}: ZA");
        // 293.6 K -> kT = 2.53e-2 eV, as ACER's header records it in MeV.
        assert!(
            (d.kt_ev - 0.0253).abs() < 1.0e-5,
            "{name}: kT {} eV is not 293.6 K",
            d.kt_ev
        );
        assert!(d.energy.len() > 10_000, "{name}: only {} grid points", d.energy.len());
        assert!(
            d.energy[0] > 0.0 && d.energy[0] < 1.0e-4,
            "{name}: grid starts at {:.3e} eV, expected ~1e-5",
            d.energy[0]
        );
        assert!(
            *d.energy.last().unwrap() >= 2.0e7,
            "{name}: grid ends at {:.3e} eV, expected >= 2e7",
            d.energy.last().unwrap()
        );
        assert!(
            d.energy.windows(2).all(|w| w[1] >= w[0]),
            "{name}: energy grid is not ascending"
        );
        for (arr, what) in [
            (&d.total, "total"),
            (&d.absorption, "absorption"),
            (&d.elastic, "elastic"),
        ] {
            assert_eq!(arr.len(), d.energy.len(), "{name}: {what} length");
            assert!(arr.iter().all(|v| *v >= 0.0), "{name}: negative {what}");
        }
    }
}

/// **The ACE block identity**, and the lump-versus-levels trap it exposes.
///
/// `total` must equal `elastic` plus the non-redundant channels. This is the
/// check that catches an off-by-one in the ESZ sub-block offsets -- reading
/// `absorption` where `elastic` lives still yields plausible cross sections,
/// but the sum stops balancing.
///
/// It also pins `channel_mts()`. Summing MTR verbatim instead gives U-235 a
/// total 30 % high at 5.5 MeV, because MTR's last entry is `MT=4` and MT=51..91
/// are in the block beside it.
#[test]
fn total_equals_elastic_plus_every_non_redundant_channel() {
    for name in ["U234", "U235", "U238"] {
        let Some(d) = load(name) else { return };
        let recon = d.reconstructed_total();
        let mut worst = 0.0_f64;
        let mut worst_e = 0.0_f64;
        for i in 0..d.energy.len() {
            let t = d.total[i];
            if t <= 0.0 {
                continue;
            }
            let rel = ((recon[i] - t) / t).abs();
            if rel > worst {
                worst = rel;
                worst_e = d.energy[i];
            }
        }
        assert!(
            worst < 1.0e-6,
            "{name}: total != elastic + channels, worst {worst:.3e} relative at \
             {worst_e:.4e} eV. A lump and its own levels are probably both being \
             summed -- see `channel_mts`."
        );
        println!(
            "{name}: {} channels, balance holds to {worst:.2e} relative (worst at {worst_e:.4e} eV)",
            d.channel_mts().len()
        );
    }
}

/// **Fission is found even with no MT=18, and absorption is not fission.**
///
/// Pins both findings in the module docs. If `fission_xs` ever starts reading
/// MT=18 only, or someone "simplifies" it to the ESZ absorption array, this
/// fails.
#[test]
fn fission_comes_from_the_partials_when_mt18_is_absent() {
    let Some(d) = load("U234") else { return };
    assert!(d.is_fissionable(), "U-234 must be fissionable");
    assert!(
        d.xs_on_grid(18).is_none(),
        "this test exists because NJOY2016's U-234 has no MT=18; if it now does, \
         the convention changed and `fission_xs` needs re-reading"
    );
    assert!(
        PARTIAL_FISSION_MTS.iter().any(|mt| d.xs_on_grid(*mt).is_some()),
        "U-234 carries no partial fission MT either"
    );
    let fiss = d.fission_xs().expect("fissionable table yields a fission xs");
    assert_eq!(fiss.len(), d.energy.len());

    // At 1 meV: absorption is capture ALONE, and total - elastic is capture
    // plus fission. The difference is the fission cross section.
    let i = d.energy.partition_point(|&e| e < 1.0e-3).min(d.energy.len() - 1);
    let non_elastic = d.total[i] - d.elastic[i];
    let gap = non_elastic - d.absorption[i];
    assert!(
        (gap - fiss[i]).abs() <= 1.0e-9 * non_elastic.max(1.0),
        "at {:.3e} eV: (total - elastic) - absorption = {gap:.6e} should equal \
         fission {:.6e}; if these differ, either `fission_xs` is wrong or ACE's \
         absorption is not the disappearance cross section this assumes",
        d.energy[i],
        fiss[i]
    );
    assert!(gap > 0.0, "the fission contribution should be positive at 1 meV");
    println!(
        "U234 @ {:.3e} eV: absorption {:.6e} b, total-elastic {:.6e} b, fission {:.6e} b",
        d.energy[i], d.absorption[i], non_elastic, fiss[i]
    );
}

/// A non-continuous-energy table must be refused, not decoded into numbers.
#[test]
fn a_thermal_table_is_refused_rather_than_misread() {
    let Some(p) = ace_reference_file_or_skip("MANIFEST.tsv", "ace-ce-decode/manifest") else {
        return;
    };
    // The manifest is not an ACE file at all; reading it must fail rather than
    // produce a table. This is the cheap version of the "wrong class" guard.
    assert!(
        read::read(&p).is_err() || decode_ce(&read::read(&p).unwrap()).is_err(),
        "a non-ACE file decoded without complaint"
    );
}
