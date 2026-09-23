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

// ── The secondary-distribution blocks (AND, DLW, NU) ────────────────────────

use njoy_outram_park_fork::acer::ce_laws::{
    decode_angular, decode_energy_law, decode_nu, n_neutron_reactions, AceEnergyLaw,
};
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::nuclear_data::secondary::NuBar;
use njoy_outram_park_fork::reference_data::reference_endf;

fn raw(nuclide: &str) -> Option<njoy_outram_park_fork::acer::read::RawAceTable> {
    let rel = format!("reference-njoy/endf-b-viii.0/293.6K/{nuclide}.ace.gz");
    let p = ace_reference_file_or_skip(&rel, &format!("ace-laws/{nuclide}"))?;
    Some(read::read(&p).unwrap_or_else(|e| panic!("read {nuclide}: {e}")))
}

fn lin_interp(nb: &NuBar, e: f64) -> f64 {
    let i = nb.energy.partition_point(|&x| x < e).clamp(1, nb.energy.len() - 1);
    let (e0, e1) = (nb.energy[i - 1], nb.energy[i]);
    let (v0, v1) = (nb.nu_total[i - 1], nb.nu_total[i]);
    if e1 > e0 {
        v0 + (v1 - v0) * (e - e0) / (e1 - e0)
    } else {
        v0
    }
}

/// **Every DLW law in the reference library decodes**, and every outgoing
/// energy distribution is a proper distribution.
///
/// # Results (2026-09-23)
///
/// U-235 44/44 laws, U-238 44/44, U-234 48/48, zero failures. Every tabulated
/// law's cdf ends at 1.00000. Thresholds land where the physics puts them:
/// U-235 MT=16 `(n,2n)` opens at 5.321 MeV and MT=17 `(n,3n)` at 12.19 MeV.
#[test]
fn every_secondary_law_in_the_reference_library_decodes() {
    for name in ["U234", "U235", "U238"] {
        let Some(t) = raw(name) else { return };
        let d = decode_ce(&t).unwrap();
        let nr = n_neutron_reactions(&t);
        assert!(nr > 0, "{name}: no reactions with secondary neutrons");
        let mut n_law3 = 0;
        let mut n_tab = 0;
        for i in 0..nr {
            let law = decode_energy_law(&t, i)
                .unwrap_or_else(|e| panic!("{name} reaction {i} (MT={}): {e}", d.reactions[i].mt));
            match law {
                AceEnergyLaw::TwoBodyLevel { ldat2, .. } => {
                    n_law3 += 1;
                    // LDAT2 is (A/(A+1))^2, so it is a fraction just under 1
                    // for a heavy nuclide. A value outside (0, 1] means the
                    // two LAW=3 words were read in the wrong order.
                    assert!(
                        ldat2 > 0.0 && ldat2 <= 1.0,
                        "{name} MT={}: LAW=3 LDAT2 = {ldat2}, expected (A/(A+1))^2 in (0,1]",
                        d.reactions[i].mt
                    );
                }
                AceEnergyLaw::Tabulated { law, rows, .. } => {
                    n_tab += 1;
                    assert!(!rows.is_empty(), "{name} MT={}: empty law", d.reactions[i].mt);
                    for r in &rows {
                        let c = r.eout.cdf.last().copied().unwrap_or(0.0);
                        assert!(
                            (c - 1.0).abs() < 1.0e-6,
                            "{name} MT={} LAW={law} at E_in={:.4e}: cdf ends at {c}, not 1",
                            d.reactions[i].mt,
                            r.e_in
                        );
                        assert!(
                            r.eout.e_out.windows(2).all(|w| w[1] >= w[0]),
                            "{name} MT={}: outgoing energies not ascending",
                            d.reactions[i].mt
                        );
                        assert!(
                            r.eout.pdf.iter().all(|p| *p >= 0.0),
                            "{name} MT={}: negative outgoing-energy pdf",
                            d.reactions[i].mt
                        );
                    }
                    if law == 44 {
                        assert!(rows.iter().all(|r| r.kalbach.is_some()), "LAW=44 without r/a");
                    }
                    if law == 61 {
                        assert!(rows.iter().all(|r| r.cosines.is_some()), "LAW=61 without cosines");
                    }
                }
            }
        }
        println!("{name}: {nr} laws decoded ({n_law3} LAW=3, {n_tab} tabulated)");
    }
}

/// The elastic AND block decodes, in the centre-of-mass frame ACE stores it in.
#[test]
fn the_elastic_angular_block_decodes() {
    for name in ["U234", "U235", "U238"] {
        let Some(t) = raw(name) else { return };
        let a = decode_angular(&t, 0, 2)
            .unwrap_or_else(|e| panic!("{name} elastic AND: {e}"))
            .unwrap_or_else(|| panic!("{name}: elastic is not isotropic in any evaluation"));
        assert_eq!(a.lct, 2, "{name}: elastic angular must be centre-of-mass");
        assert!(a.energies.len() > 10, "{name}: {} incident energies", a.energies.len());
        assert!(
            a.energies.windows(2).all(|w| w[1].e_mev >= w[0].e_mev),
            "{name}: AND incident energies not ascending"
        );
        for ea in &a.energies {
            if ea.cosines.is_empty() {
                continue; // isotropic at this energy
            }
            assert!(
                ea.cosines.first().unwrap() >= &-1.000_001
                    && ea.cosines.last().unwrap() <= &1.000_001,
                "{name}: cosine grid leaves [-1, 1] at {:.3e} MeV",
                ea.e_mev
            );
            let c = ea.cdf.last().copied().unwrap_or(0.0);
            assert!(
                (c - 1.0).abs() < 1.0e-6,
                "{name}: angular cdf ends at {c} at {:.3e} MeV",
                ea.e_mev
            );
        }
        println!("{name}: elastic AND, {} incident energies, lct={}", a.energies.len(), a.lct);
    }
}

/// **The ACE nu-bar is the ENDF nu-bar.** The cross-check that says which block
/// was read.
///
/// # Why magnitude alone could not settle this
///
/// The decoded U-235 value at thermal is 2.4299, which sits between the
/// accepted prompt (~2.425) and total (~2.437) nu-bar. Eyeballing it cannot
/// tell which block the decoder landed on, and the difference is the delayed
/// fraction -- about 0.7 % on `k`, in the direction that flatters a critical
/// benchmark. Comparing against the evaluation settles it.
///
/// # Results (2026-09-23)
///
/// ACE equals ENDF **exactly**, to all five printed digits, for U-234, U-235
/// and U-238 at 0.0253 eV, 1 keV, 1 MeV, 2 MeV and 14 MeV. That is expected
/// rather than lucky: ACER copies MF=1/452 through without touching it, so
/// anything other than exact equality would mean a decode error. The test
/// therefore gates hard, at 1e-9 relative.
#[test]
fn ace_nubar_equals_the_endf_evaluation() {
    for (name, endf) in [
        ("U234", "n-092_U_234-ENDF8.0.endf"),
        ("U235", "n-092_U_235-ENDF8.0.endf"),
        ("U238", "n-092_U_238.endf"),
    ] {
        let Some(t) = raw(name) else { return };
        let ace = decode_nu(&t)
            .unwrap_or_else(|e| panic!("{name} NU: {e}"))
            .unwrap_or_else(|| panic!("{name} is fissionable and must have a NU block"));
        let Some(ep) = reference_endf(endf) else {
            println!("[ace-nubar/{name}] SKIP: no ENDF tape {endf}");
            continue;
        };
        let tape = Tape::read_file(&ep).unwrap();
        let mat = tape.materials()[0];
        let Some(reference) = NuBar::from_endf(&tape, mat).unwrap() else {
            panic!("{name}: ENDF tape has no MF=1/452")
        };
        for e in [2.53e-2_f64, 1.0e3, 1.0e6, 2.0e6, 1.4e7] {
            let a = lin_interp(&ace, e);
            let r = lin_interp(&reference, e);
            assert!(
                (a - r).abs() <= 1.0e-9 * r.abs(),
                "{name} at {e:.3e} eV: ACE nu = {a:.8}, ENDF nu = {r:.8}, diff {:+.3e}. \
                 ACER copies MF=1/452 verbatim, so any difference is a decode error -- \
                 most likely prompt read where total was meant.",
                a - r
            );
        }
        // And it must be physical, not merely self-consistent.
        let thermal = lin_interp(&ace, 2.53e-2);
        assert!(
            (2.2..2.6).contains(&thermal),
            "{name}: nu-bar {thermal} at thermal is not a fission yield"
        );
        println!("{name}: ACE nu-bar == ENDF nu-bar exactly; nu(0.0253 eV) = {thermal:.5}");
    }
}
