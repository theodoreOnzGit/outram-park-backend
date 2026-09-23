// SPDX-License-Identifier: GPL-3.0

//! **The parsed depletion chain against the hand transcription of the same
//! file** — GitHub #270's "Depletion chain XML | read" row.
//!
//! # Methodology
//!
//! `DepletionChain::simple()` is a transcription, written out by hand in Rust,
//! of OpenMC's `examples/pincell_depletion/chain_simple.xml`. Until 2026-09-23
//! that transcription was the **only** source of neutron-reaction targets in
//! this crate, because the decay-data crates do not expose them — the chain
//! module's own "Known limitations" note says so. This test reads the XML file
//! through the new codec
//! (`njoy_outram_park_fork::hdf5::depletion_chain_xml::DepletionChainXml`) and
//! asserts the result is **exactly equal** to the transcription: same nuclides
//! in the same order, same half-lives, same decay branches and branching
//! ratios, same reaction channels and targets, same fission yields, same Q
//! values, `f64` for `f64` with no tolerance.
//!
//! Exact equality is the right instrument here, and it is available because both
//! sides ultimately come from the same decimal literals: Rust's `f64` parser is
//! correctly rounded, so `"2.36520E+04"` in the file and `2.36520e4` in the
//! source must produce the same bits or one of the two is wrong.
//!
//! The comparison is worth something in **both** directions. A disagreement
//! would mean either the reader misreads the format or the transcription was
//! never faithful, and either is a defect worth finding. Agreement means the
//! reader is exercised against 9 nuclides of known-correct expected output, and
//! the transcription is no longer an unchecked hand copy.
//!
//! # Results (2026-09-23)
//!
//! * `openmc_chain_simple.xml` (committed copy, OpenMC `afa7a14`, MIT) parses
//!   to a chain **exactly equal** to `DepletionChain::simple()` — 9 nuclides,
//!   2 decay branches, 7 reaction channels, 18 fission yields.
//! * The committed copy is **byte-identical** to the checkout's file.
//! * No unmodelled channels: every reaction in this chain is `(n,gamma)` or
//!   `fission` at unit branching, so nothing is dropped.
//! * The burnup matrix built from the parsed chain is **identical entry for
//!   entry** to the one built from the transcription, at the same reaction
//!   rates — which is the property that actually matters, since the matrix is
//!   what the solver sees.
//! * `chain_ni.xml` (21 nuclides, when a checkout is present) carries four
//!   reaction types — `(n,2n)`, `(n,a)`, `(n,gamma)`, `(n,p)` — of which this
//!   crate models two, and reports **18 unmodelled channels**, all `(n,a)` or
//!   `(n,p)`. That is a recorded limitation of the three-channel one-group
//!   operator, not a reader defect; the test below pins the count so it cannot
//!   grow silently.
//!
//! # Provenance
//!
//! Reference file: `tests/reference_data/openmc_chain_simple.xml`; see that
//! folder's `README.md` for its source, commit, licence and (absence of)
//! processing.

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::hdf5::depletion_chain_xml::DepletionChainXml;
use outram_mc_libs::depletion::{
    DepletionChain, MicroRate, ReactionRates, UnmodelledReason,
};

/// Thermal, 0.0253 eV — the single energy `chain_simple.xml` tabulates.
const THERMAL_EV: f64 = 2.53000e-2;

fn reference_chain_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/reference_data/openmc_chain_simple.xml")
}

/// **The headline check.** Reading the file reproduces the hand transcription
/// exactly.
#[test]
fn the_parsed_chain_equals_the_hand_transcription() {
    let parsed = DepletionChain::from_chain_xml_file(&reference_chain_path(), THERMAL_EV)
        .expect("the committed reference chain must parse");
    let transcribed = DepletionChain::simple();

    // Report the shape before asserting equality, so a failure says *what*
    // differs rather than dumping two nine-element Debug blobs.
    assert_eq!(parsed.nuclide_names(), transcribed.nuclide_names());
    for name in transcribed.nuclide_names() {
        let a = parsed.nuclide(name).unwrap();
        let b = transcribed.nuclide(name).unwrap();
        assert_eq!(a.half_life_seconds, b.half_life_seconds, "{name} half-life");
        assert_eq!(a.decays, b.decays, "{name} decay branches");
        assert_eq!(a.reactions, b.reactions, "{name} reaction channels");
        assert_eq!(a.fission_yields, b.fission_yields, "{name} fission yields");
        assert_eq!(a.fission_q_ev, b.fission_q_ev, "{name} fission Q");
    }
    assert_eq!(parsed, transcribed, "the whole chain must be equal");

    // Nothing in this chain is outside the three modelled channels.
    assert!(
        parsed.unmodelled_channels().is_empty(),
        "unexpected unmodelled channels: {:?}",
        parsed.unmodelled_channels()
    );

    // And the counts the doc comment above records.
    let decays: usize = parsed.nuclides().iter().map(|n| n.decays.len()).sum();
    let reactions: usize = parsed.nuclides().iter().map(|n| n.reactions.len()).sum();
    let yields: usize = parsed
        .nuclides()
        .iter()
        .map(|n| n.fission_yields.len())
        .sum();
    assert_eq!((parsed.len(), decays, reactions, yields), (9, 2, 7, 18));
}

/// Equality of the *records* is only interesting if it survives into the
/// **matrix**, which is what the CRAM solver actually integrates.
#[test]
fn both_chains_build_the_same_burnup_matrix() {
    let parsed = DepletionChain::from_chain_xml_file(&reference_chain_path(), THERMAL_EV).unwrap();
    let transcribed = DepletionChain::simple();

    // Non-zero rates on every channel present, so no term can agree by being
    // multiplied out to zero.
    let mut rates = ReactionRates::default();
    for (name, gamma, fission) in [
        ("I135", 1.1e-6, 0.0),
        ("Xe135", 2.2e-4, 0.0),
        ("Gd157", 3.3e-6, 0.0),
        ("Gd156", 4.4e-7, 0.0),
        ("U234", 0.0, 5.5e-9),
        ("U235", 0.0, 6.6e-7),
        ("U238", 0.0, 7.7e-10),
    ] {
        rates.set(
            name,
            MicroRate {
                gamma,
                fission,
                n2n: 0.0,
            },
        );
    }

    let a = parsed.build_matrix(&rates);
    let b = transcribed.build_matrix(&rates);
    let n = parsed.len();
    for i in 0..n {
        for j in 0..n {
            assert_eq!(
                a.get(i, j),
                b.get(i, j),
                "matrix entry ({i},{j}) differs: {} vs {}",
                a.get(i, j),
                b.get(i, j)
            );
        }
    }
    // A sanity floor: the matrix must not be all zeros, or the loop above
    // proves nothing.
    let nonzero = (0..n)
        .flat_map(|i| (0..n).map(move |j| (i, j)))
        .filter(|&(i, j)| a.get(i, j) != 0.0)
        .count();
    assert!(nonzero >= 20, "only {nonzero} non-zero entries");
}

/// The committed reference copy must not drift from the file it came from.
/// **Skips** when no OpenMC checkout is present rather than failing — the
/// committed copy is the one the test above uses, and it is present always.
#[test]
fn the_committed_copy_still_matches_the_openmc_checkout() {
    let upstream = Path::new("/opt/src/openmc/examples/pincell_depletion/chain_simple.xml");
    if !upstream.is_file() {
        println!("SKIP: no OpenMC checkout at {}", upstream.display());
        return;
    }
    let ours = std::fs::read(reference_chain_path()).unwrap();
    let theirs = std::fs::read(upstream).unwrap();
    assert_eq!(
        ours, theirs,
        "tests/reference_data/openmc_chain_simple.xml has drifted from the checkout's copy; \
         re-copy it and re-check the README's commit hash"
    );
}

/// **The limitation, stated as a test rather than only in prose.** A real chain
/// carries reaction types this crate's three-channel operator has no rate field
/// for. They must be *reported*, not silently absent — a caller who cannot see
/// what was dropped cannot tell a complete chain from a gutted one.
///
/// Skips without an OpenMC checkout; `chain_ni.xml` is a test fixture that is
/// not worth committing here, since the behaviour is also covered by the
/// synthetic case below.
#[test]
fn unmodelled_reaction_types_are_reported_not_dropped_silently() {
    // Synthetic first, so this assertion runs everywhere.
    let xml = DepletionChainXml::parse(
        r#"<depletion_chain>
             <nuclide name="Fe57" reactions="3">
               <reaction type="(n,gamma)" Q="1.0e7" target="Fe58"/>
               <reaction type="(n,p)" Q="-1.9e6" target="Mn57"/>
               <reaction type="(n,a)" Q="2.4e6"/>
             </nuclide>
             <nuclide name="Fe58" reactions="0"/>
             <nuclide name="Mn57" reactions="0"/>
           </depletion_chain>"#,
    )
    .unwrap();
    let chain = DepletionChain::from_chain_xml(&xml, THERMAL_EV).unwrap();
    let dropped = chain.unmodelled_channels();
    assert_eq!(dropped.len(), 2, "{dropped:?}");
    assert!(dropped.iter().all(|d| d.nuclide == "Fe57"));
    assert_eq!(
        dropped
            .iter()
            .map(|d| d.reaction.as_str())
            .collect::<Vec<_>>(),
        ["(n,p)", "(n,a)"]
    );
    assert!(dropped
        .iter()
        .all(|d| d.reason == UnmodelledReason::ReactionTypeNotModelled));
    // The channel that IS modelled is still there.
    assert_eq!(chain.nuclide("Fe57").unwrap().reactions.len(), 1);

    // A branched channel is reported too: this crate's NeutronReaction has no
    // branching field, and applying the full rate to one product would create
    // atoms the file did not specify.
    let xml = DepletionChainXml::parse(
        r#"<depletion_chain>
             <nuclide name="Ag109" reactions="2">
               <reaction type="(n,gamma)" Q="1.0" target="Ag110" branching_ratio="0.955"/>
               <reaction type="(n,gamma)" Q="1.0" target="Ag110_m1" branching_ratio="0.045"/>
             </nuclide>
             <nuclide name="Ag110" reactions="0"/>
             <nuclide name="Ag110_m1" reactions="0"/>
           </depletion_chain>"#,
    )
    .unwrap();
    let chain = DepletionChain::from_chain_xml(&xml, THERMAL_EV).unwrap();
    assert_eq!(chain.unmodelled_channels().len(), 2);
    assert_eq!(
        chain.unmodelled_channels()[0].reason,
        UnmodelledReason::BranchedChannel {
            branching_ratio: 0.955
        }
    );
    assert!(
        chain.nuclide("Ag109").unwrap().reactions.is_empty(),
        "a branched channel must be left out entirely, not applied at full rate"
    );

    // Then the real file, if it is there — the numbers the doc comment records.
    let ni = Path::new("/opt/src/openmc/tests/chain_ni.xml");
    if !ni.is_file() {
        println!("SKIP chain_ni.xml: no OpenMC checkout");
        return;
    }
    let xml = DepletionChainXml::read_file(ni).unwrap();
    let chain = DepletionChain::from_chain_xml(&xml, THERMAL_EV).unwrap();
    println!(
        "chain_ni.xml: {} nuclides, {} unmodelled channels, kinds in file {:?}",
        chain.len(),
        chain.unmodelled_channels().len(),
        xml.reaction_kinds()
    );
    assert_eq!(chain.len(), 21);
    // Pinned so the number cannot drift unnoticed: 18 channels, every one of
    // them (n,a) or (n,p). (n,2n) IS modelled, so it must NOT appear here.
    assert_eq!(chain.unmodelled_channels().len(), 18);
    for d in chain.unmodelled_channels() {
        assert!(
            d.reaction == "(n,a)" || d.reaction == "(n,p)",
            "unexpected unmodelled channel {d:?}"
        );
    }
}

/// Asking for an energy the file does not tabulate is an **error**, not a
/// nearest-neighbour substitution. Four wrong reference values in this
/// workspace's V&V history came from silently taking a neighbouring point.
#[test]
fn a_fission_energy_the_file_does_not_carry_is_refused() {
    let err = DepletionChain::from_chain_xml_file(&reference_chain_path(), 5.0e5).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("no fission yields at 500000"), "{msg}");
    assert!(msg.contains("does not interpolate"), "{msg}");
}
