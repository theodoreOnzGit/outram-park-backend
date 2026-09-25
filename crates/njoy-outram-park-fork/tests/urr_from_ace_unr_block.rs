// SPDX-License-Identifier: GPL-3.0

//! **The ACE UNR block decodes into usable probability tables** — GitHub #307.
//!
//! `outram_mc_libs::Nuclide::from_ace` set `urr: None`, so the ACE route
//! carried no unresolved-resonance self-shielding while the ENDF route applied
//! it by default. Two routes through one workspace with different physics is
//! the shape the root `CLAUDE.md`'s "correct physics is the DEFAULT SETTING"
//! rule exists to stop, and it was worse than a flag here: structurally
//! absent, so the ablation machinery could not express it either.
//!
//! `UrrProbabilityTables::from_ace` closes it, ported from
//! `openmc/data/urr.py::ProbabilityTables.from_ace` at OpenMC `afa7a14`.
//!
//! # What makes this a real check rather than a shape check
//!
//! The tables are read from the **NJOY2016 reference library**
//! (`reference-data/ace/reference-njoy/endf-b-viii.0/293.6K`, provenance in the
//! submodule's `MANIFEST.tsv`), and the same quantity is available a second,
//! wholly independent way: this crate's own **PURR** port *generates*
//! probability tables from the ENDF evaluation by sampling resonance ladders.
//!
//! So there are two routes to one physical quantity — deserialise NJOY's, or
//! generate our own — and they are compared. That is stronger than either
//! alone: a decode error in the ACE reader and a bug in PURR would have to
//! agree to pass.
//!
//! **They are not expected to agree closely**, and that is the point of the
//! bound used: PURR's tables come from a *random* ladder sample with its own
//! seed and bin count, so band-by-band equality is not even meaningful. What
//! must agree is the physics the bands encode: the energy range, the LSSF
//! convention, and the probability-weighted mean self-shielding factor.
//!
//! # Results, 2026-09-25
//!
//! Printed by the tests.

use njoy_outram_park_fork::acer::read;
use njoy_outram_park_fork::purr::{UrrProbabilityTables, UrrSample};
use njoy_outram_park_fork::reference_data::ace_reference_file_or_skip;

const TEMP_K: f64 = 293.6;

fn ace_urr(nuclide: &str) -> Option<Option<UrrProbabilityTables>> {
    let rel = format!("reference-njoy/endf-b-viii.0/293.6K/{nuclide}.ace.gz");
    let p = ace_reference_file_or_skip(&rel, &format!("urr-from-ace/{nuclide}"))?;
    let raw = read::read(&p).unwrap_or_else(|e| panic!("read {nuclide}: {e}"));
    Some(
        UrrProbabilityTables::from_ace(&raw, TEMP_K)
            .unwrap_or_else(|e| panic!("UNR decode {nuclide}: {e}")),
    )
}

/// The probability-weighted mean of each channel's factor at `e`, over the
/// bands.
///
/// **This is a weak check on its own and the tests say so.** Self-shielding
/// factors are normalised so that their probability-weighted mean is **1** —
/// that is what makes the mean cross section equal the infinitely-dilute one.
/// So comparing means between two routes compares 1.0 with 1.0 and would pass
/// against almost any correctly-normalised table, including a wrong one. Use
/// [`spread_factors`] for the quantity that actually carries the physics.
fn mean_factors(t: &UrrProbabilityTables, e: f64) -> Option<[f64; 4]> {
    // Average over many quantiles rather than reaching into private fields:
    // `sample` is the only public view of the bands, and averaging its result
    // over a uniform grid of xi IS the probability-weighted mean by
    // construction, since the bands are selected by cumulative probability.
    let n = 2000;
    let mut acc = [0.0_f64; 4];
    let mut hit = 0usize;
    for i in 0..n {
        let xi = (i as f64 + 0.5) / n as f64;
        let s = t.sample(e, xi)?;
        let v = match s {
            UrrSample::SelfShieldingFactors(v) | UrrSample::CrossSections(v) => v,
        };
        for k in 0..4 {
            acc[k] += v[k];
        }
        hit += 1;
    }
    (hit > 0).then(|| {
        let f = hit as f64;
        [acc[0] / f, acc[1] / f, acc[2] / f, acc[3] / f]
    })
}

/// The probability-weighted **standard deviation** of each channel's factor at
/// `e`, over the bands.
///
/// This is the quantity that produces self-shielding. A table whose factors are
/// all exactly 1 has mean 1 and spread **0**, and shields nothing; the spread is
/// what makes the flux-weighted cross section differ from the infinitely-dilute
/// one. So it is the discriminating comparison between two routes, where the
/// mean is not.
fn spread_factors(t: &UrrProbabilityTables, e: f64) -> Option<[f64; 4]> {
    let m = mean_factors(t, e)?;
    let n = 2000;
    let mut acc = [0.0_f64; 4];
    for i in 0..n {
        let xi = (i as f64 + 0.5) / n as f64;
        let v = match t.sample(e, xi)? {
            UrrSample::SelfShieldingFactors(v) | UrrSample::CrossSections(v) => v,
        };
        for k in 0..4 {
            acc[k] += (v[k] - m[k]) * (v[k] - m[k]);
        }
    }
    let f = n as f64;
    Some([
        (acc[0] / f).sqrt(),
        (acc[1] / f).sqrt(),
        (acc[2] / f).sqrt(),
        (acc[3] / f).sqrt(),
    ])
}

/// **THE #307 GATE: U-238's UNR block decodes, and says what it should.**
///
/// U-238 is the case that matters — its unresolved range is the one that
/// self-shields in a thermal lattice.
#[test]
fn u238_unr_block_decodes_into_sane_tables() {
    let Some(maybe) = ace_urr("U238") else { return };
    let t = maybe.expect(
        "U-238's evaluation HAS an unresolved range, so JXS(23) must be non-zero \
         and the block must decode. `None` here would mean either the locator is \
         being read at the wrong index or the reference table was built without \
         PURR.",
    );

    println!(
        "U238 ACE UNR: lssf={} range {:.4e}..{:.4e} eV, {} energies, T={} K",
        t.lssf,
        t.e_low,
        t.e_high,
        t.len(),
        t.temperature_k
    );

    // ENDF/B-VIII.0 puts U-238's unresolved range at 20 keV .. 149.03 keV.
    assert!(
        t.e_low > 1.0e4 && t.e_low < 3.0e4,
        "U-238's unresolved range starts near 20 keV, got {:.4e}",
        t.e_low
    );
    assert!(
        t.e_high > 1.0e5 && t.e_high < 2.0e5,
        "U-238's unresolved range ends near 149 keV, got {:.4e}",
        t.e_high
    );
    assert!(t.len() >= 2, "a usable table needs at least two energies");
    assert!(
        t.covers(0.5 * (t.e_low + t.e_high)),
        "the table must cover its own mid-range"
    );
    assert!(
        !t.covers(1.0),
        "1 eV is far below the unresolved range and must not be covered -- a \
         table that claims to cover thermal would self-shield the resolved \
         resonances it has no business touching"
    );

    // **The LSSF convention.** IFF=1 means the values MULTIPLY the smooth cross
    // section. Getting this backwards multiplies barns by barns, which no shape
    // check would catch, so it is asserted by magnitude: self-shielding factors
    // are O(1), cross sections in this range are O(10) barns and up.
    let mid = 0.5 * (t.e_low + t.e_high);
    let m = mean_factors(&t, mid).expect("the mid-range must sample");
    println!(
        "  mean over bands at {mid:.4e} eV: total={:.5} elastic={:.5} \
         fission={:.5} capture={:.5}",
        m[0], m[1], m[2], m[3]
    );
    match t.sample(mid, 0.5).unwrap() {
        UrrSample::SelfShieldingFactors(_) => {
            assert!(
                (0.1..10.0).contains(&m[0]) && (0.1..10.0).contains(&m[1]),
                "LSSF=1 means dimensionless factors near 1; total={} elastic={} \
                 look like barns, so IFF has been mapped the wrong way round",
                m[0],
                m[1]
            );
        }
        UrrSample::CrossSections(_) => {
            assert!(
                m[0] > 1.0,
                "LSSF=0 means barns; a total of {} is too small to be a cross \
                 section in the unresolved range",
                m[0]
            );
        }
    }
    // Every channel must be non-negative whichever convention applies.
    for (k, label) in ["total", "elastic", "fission", "capture"].iter().enumerate() {
        assert!(
            m[k] >= 0.0,
            "{label} mean is negative ({}), which is unphysical either as a \
             factor or as a cross section",
            m[k]
        );
    }

    // **THE CHECK THAT ACTUALLY BITES: the bands must SPREAD.**
    //
    // The mean above is ~1 by normalisation, so it would pass for a table whose
    // every factor is exactly 1 -- a table that shields nothing. The spread is
    // what makes the flux-weighted cross section differ from the infinitely
    // dilute one, i.e. it is the self-shielding. A decoder that read the
    // cumulative-probability column into the value slots, or read one band
    // repeatedly, would give mean 1 and spread 0 and pass every other assertion
    // here.
    let sd = spread_factors(&t, mid).expect("the mid-range must sample");
    println!(
        "  band spread at {mid:.4e} eV: total={:.5} elastic={:.5} fission={:.5} \
         capture={:.5}",
        sd[0], sd[1], sd[2], sd[3]
    );
    assert!(
        sd[0] > 1.0e-3,
        "the total-channel factors have spread {:.3e} across bands, i.e. they are \
         effectively all equal -- such a table self-shields NOTHING, so either \
         the value columns were not read or one band is being returned for every \
         quantile",
        sd[0]
    );
    assert!(
        sd[3] > 1.0e-3,
        "the capture-channel factors have spread {:.3e}; capture is the channel \
         self-shielding matters most for in U-238, so a flat one is not credible",
        sd[3]
    );
}

/// U-235 decodes too, and its range differs from U-238's — so the decoder is
/// reading each table's own header rather than something shared.
#[test]
fn u235_unr_block_decodes_and_differs_from_u238() {
    let (Some(a), Some(b)) = (ace_urr("U235"), ace_urr("U238")) else {
        return;
    };
    let (Some(u235), Some(u238)) = (a, b) else {
        println!("SKIP: one of the reference tables carries no UNR block");
        return;
    };
    println!(
        "U235 range {:.4e}..{:.4e} eV ({} energies); U238 {:.4e}..{:.4e} ({} energies)",
        u235.e_low,
        u235.e_high,
        u235.len(),
        u238.e_low,
        u238.e_high,
        u238.len()
    );
    assert!(
        (u235.e_low - u238.e_low).abs() > 1.0 || (u235.e_high - u238.e_high).abs() > 1.0,
        "U-235 and U-238 have different unresolved ranges; identical bounds mean \
         the decoder is not reading per-table headers"
    );
}

/// **A zero locator is `None`, not an error and not a fabricated table.**
///
/// This is the distinction the old `urr: None` destroyed: "this path does not
/// read the block" and "the evaluation has none" must not look the same. A
/// light nuclide such as H-1 has no unresolved range and so carries
/// `JXS(23) == 0`.
///
/// It is tested by taking a table that **does** have a block and zeroing the
/// locator, rather than by reaching for a light reference table: the committed
/// reference library is uranium only, so a light-nuclide version of this test
/// would skip and a skip must not read as a pass.
#[test]
fn a_zero_locator_yields_none_rather_than_an_error() {
    let rel = "reference-njoy/endf-b-viii.0/293.6K/U238.ace.gz";
    let Some(p) = ace_reference_file_or_skip(rel, "urr-zero-locator") else {
        return;
    };
    let mut raw = read::read(&p).expect("read U238");

    // Sanity: it has one to begin with, or this test proves nothing.
    assert!(
        raw.jxs[njoy_outram_park_fork::acer::jxs::LUNR] > 0,
        "U-238 must carry a UNR block for this test to mean anything"
    );
    assert!(UrrProbabilityTables::from_ace(&raw, TEMP_K)
        .expect("decodes")
        .is_some());

    // Now the case a light nuclide presents.
    raw.jxs[njoy_outram_park_fork::acer::jxs::LUNR] = 0;
    let got = UrrProbabilityTables::from_ace(&raw, TEMP_K)
        .expect("a zero locator is not an error -- most nuclides have no unresolved range");
    assert!(
        got.is_none(),
        "a zero JXS(23) must give None; a table here means the decoder is reading \
         words from wherever the locator happens to point"
    );
    println!("zero locator -> None, as a light nuclide would give");
}

/// **A corrupt block is refused rather than half-read.**
///
/// A declared extent past the end of `XSS` is the failure that would otherwise
/// read adjacent blocks as probabilities — numbers that parse and are wrong.
#[test]
fn a_block_running_past_xss_is_refused() {
    let rel = "reference-njoy/endf-b-viii.0/293.6K/U238.ace.gz";
    let Some(p) = ace_reference_file_or_skip(rel, "urr-corrupt") else {
        return;
    };
    let mut raw = read::read(&p).expect("read U238");
    let loc = raw.jxs[njoy_outram_park_fork::acer::jxs::LUNR] as usize;

    // Claim far more energies than the file can hold.
    raw.xss[loc - 1] = 1.0e9;
    let err = UrrProbabilityTables::from_ace(&raw, TEMP_K)
        .expect_err("a block claiming a billion energies must be refused");
    let m = format!("{err}");
    assert!(m.contains("UNR"), "the error must name the block: {m}");
    assert!(m.contains("XSS"), "and say what it ran past: {m}");
    println!("corrupt extent refused: {m}");

    // And a zero band count, which would divide the table into nothing.
    let mut raw2 = read::read(&p).expect("read U238");
    raw2.xss[loc] = 0.0;
    let err = UrrProbabilityTables::from_ace(&raw2, TEMP_K)
        .expect_err("zero bands must be refused, not silently treated as absent");
    assert!(format!("{err}").contains("bands"), "{err}");
}

/// **Two independent routes to the same physics: NJOY's tables vs ours.**
///
/// The ACE side deserialises the tables NJOY2016's PURR wrote; the ENDF side
/// runs **this crate's** PURR port on the same evaluation to generate its own.
/// A decode error here and a PURR bug there would have to agree to pass.
///
/// The comparison is on the **energy range and the LSSF convention**, not on
/// band values: PURR samples random ladders, so band-by-band equality is not
/// meaningful and demanding it would be a fake gate. The mean factor is printed
/// for both so a reader can see how close they land without a threshold being
/// asserted on a quantity whose scatter has not been characterised.
#[test]
fn ace_tables_and_our_own_purr_agree_on_range_and_convention() {
    use njoy_outram_park_fork::endf::tape::Tape;
    use njoy_outram_park_fork::reference_data::reference_endf;

    let Some(maybe) = ace_urr("U238") else { return };
    let Some(from_ace) = maybe else {
        println!("SKIP: the reference U238 table carries no UNR block");
        return;
    };
    let Some(path) = reference_endf("n-092_U_238.endf") else {
        println!("SKIP: U-238 ENDF tape absent, cannot run our own PURR");
        return;
    };
    let tape = Tape::read_file(&path).expect("parse U-238");
    let mat = *tape.materials().first().expect("a material");
    let ours = UrrProbabilityTables::from_endf(&tape, mat, TEMP_K, 20, 64, 1)
        .expect("our PURR must run")
        .expect("U-238 has an unresolved range, so our PURR must produce tables");

    println!(
        "  NJOY's (via ACE): lssf={} range {:.5e}..{:.5e} eV, {} energies",
        from_ace.lssf,
        from_ace.e_low,
        from_ace.e_high,
        from_ace.len()
    );
    println!(
        "  ours (via PURR) : lssf={} range {:.5e}..{:.5e} eV, {} energies",
        ours.lssf,
        ours.e_low,
        ours.e_high,
        ours.len()
    );

    assert_eq!(
        from_ace.lssf, ours.lssf,
        "the two routes disagree on LSSF ({} vs {}). One of them is treating \
         self-shielding factors as cross sections, which multiplies barns by \
         barns -- and ACE's IFF is what maps onto LSSF, so this is exactly the \
         mapping to suspect",
        from_ace.lssf, ours.lssf
    );

    // The unresolved range is a property of the EVALUATION, not of how the
    // tables were made, so these must agree to well within a percent.
    let rel = |a: f64, b: f64| (a - b).abs() / b.max(1.0);
    assert!(
        rel(from_ace.e_low, ours.e_low) < 0.01,
        "lower bound differs: {:.6e} (ACE) vs {:.6e} (ours) -- the unresolved \
         range comes from the evaluation and cannot depend on the route",
        from_ace.e_low,
        ours.e_low
    );
    assert!(
        rel(from_ace.e_high, ours.e_high) < 0.01,
        "upper bound differs: {:.6e} (ACE) vs {:.6e} (ours)",
        from_ace.e_high,
        ours.e_high
    );

    let mid = 0.5 * (from_ace.e_low + from_ace.e_high);

    // The MEAN is printed only, and is near-vacuous: both are pinned to 1 by
    // normalisation (see `mean_factors`).
    if let (Some(a), Some(b)) = (mean_factors(&from_ace, mid), mean_factors(&ours, mid)) {
        println!(
            "  mean factors at {mid:.4e} eV (pinned to ~1 by normalisation) -- \
             NJOY's [{:.5}, {:.5}, {:.5}, {:.5}] vs ours [{:.5}, {:.5}, {:.5}, {:.5}]",
            a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]
        );
    }

    // The SPREAD is the physics, and both routes must find self-shielding of a
    // comparable size. Asserted loosely and deliberately: PURR samples random
    // ladders with its own seed and bin count, so the spreads cannot match
    // closely and demanding that they should would be a gate that fails for the
    // wrong reason. What is NOT acceptable is one route finding structure and
    // the other finding none -- an order of magnitude is the honest bound.
    let (Some(sa), Some(sb)) = (spread_factors(&from_ace, mid), spread_factors(&ours, mid))
    else {
        return;
    };
    println!(
        "  band spread at {mid:.4e} eV -- NJOY's [{:.5}, {:.5}, {:.5}, {:.5}] \
         vs ours [{:.5}, {:.5}, {:.5}, {:.5}]",
        sa[0], sa[1], sa[2], sa[3], sb[0], sb[1], sb[2], sb[3]
    );
    for (k, label) in ["total", "elastic", "fission", "capture"].iter().enumerate() {
        if sa[k] < 1.0e-6 && sb[k] < 1.0e-6 {
            continue; // a genuinely flat channel on both sides says nothing
        }
        let (lo, hi) = (sa[k].min(sb[k]), sa[k].max(sb[k]));
        assert!(
            lo > 1.0e-6 && hi / lo < 10.0,
            "the {label} channel's band spread differs by more than 10x between \
             routes: {:.4e} (NJOY via ACE) vs {:.4e} (ours via PURR). One route \
             is finding self-shielding structure the other is not, which no \
             amount of ladder-sampling scatter explains",
            sa[k],
            sb[k]
        );
    }
}
