//! Tests for the `Chain` operations port (gh:#1621). Predictions are in each
//! doc comment; results recorded 2026-11-17.

use super::*;

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

const NI: &str = include_str!("../../../verification_and_validation/depletion_chain_xml/openmc_inputs/chain_ni.xml");
const SIMPLE: &str = include_str!("../../../verification_and_validation/depletion_chain_xml/openmc_inputs/chain_simple.xml");
const SIMPLE_DECAY: &str =
    include_str!("../../../verification_and_validation/depletion_chain_xml/openmc_inputs/chain_simple_decay.xml");

/// `chain_ni.xml` is OpenMC's `tests/chain_ni.xml`, written by
/// `Chain.export_to_xml` (its floats are Python `str`). Prediction: our
/// export of the parsed chain is the same file, byte for byte.
#[test]
fn export_reproduces_an_openmc_written_chain_byte_for_byte() {
    let chain = DepletionChainXml::parse(NI).unwrap();
    let ours = export_to_xml(&chain);
    if ours != NI {
        let first = ours.lines().zip(NI.lines()).position(|(a, b)| a != b);
        panic!("first differing line {first:?}:\n ours {:?}\n file {:?}", first.map(|i| ours.lines().nth(i)), first.map(|i| NI.lines().nth(i)));
    }
}

/// The hand-written chains: export then parse gives the same chain.
#[test]
fn export_round_trips_every_committed_chain() {
    for text in [NI, SIMPLE, SIMPLE_DECAY] {
        let chain = DepletionChainXml::parse(text).unwrap();
        let back = DepletionChainXml::parse(&export_to_xml(&chain)).unwrap();
        assert_eq!(back, chain);
    }
}

#[test]
fn floats_print_as_python_str() {
    let cases = [
        (2224631.0, "2224631.0"),
        (0.1, "0.1"),
        (1e-5, "1e-05"),
        (1e-4, "0.0001"),
        (1.5e16, "1.5e+16"),
        (1e15, "1000000000000000.0"),
        (-7646000.0, "-7646000.0"),
        (4.281895544855688e-11, "4.281895544855688e-11"),
        (0.0, "0.0"),
        (-0.0, "-0.0"),
        (123.456, "123.456"),
        (f64::INFINITY, "inf"),
    ];
    for (x, s) in cases {
        assert_eq!(python_float_repr(x), s, "{x:e}");
    }
}

#[test]
fn zam_reads_gnds_names() {
    assert_eq!(zam("Am242_m1").unwrap(), (95, 242, 1));
    assert_eq!(zam("H1").unwrap(), (1, 1, 0));
    assert!(zam("Qq3").is_err());
    assert!(zam("he4").is_err(), "case matters in GNDS");
}

/// Upstream's own docstring example on its own chain: `reduce(["I135"])`
/// gives I135, Xe135, Cs135 and Xe136, and level 0 gives the start alone.
#[test]
fn reduce_follows_upstream_docstring_example() {
    let chain = DepletionChainXml::parse(SIMPLE).unwrap();
    let r = reduce(&chain, &["I135"], None).unwrap();
    assert_eq!(r.names(), vec!["I135", "Xe135", "Xe136", "Cs135"], "zam order");
    let r0 = reduce(&chain, &["I135"], Some(0)).unwrap();
    assert_eq!(r0.names(), vec!["I135"]);
    // Rates are kept with no target.
    let i = r0.get("I135").unwrap();
    assert_eq!(i.decays.len(), 1);
    assert_eq!(i.decays[0].target, None);
    assert_eq!(i.reactions[0].target, None);
    assert!(reduce(&chain, &["Pu239"], None).is_err());
}

/// One level from U-235 reaches its fission products but not what they
/// make; fission keeps only yields to retained products, sorted by name.
#[test]
fn reduce_restricts_fission_yields() {
    let chain = DepletionChainXml::parse(SIMPLE).unwrap();
    let full = reduce(&chain, &["U235"], None).unwrap();
    let r = reduce(&chain, &["U235", "I135"], Some(0)).unwrap();
    let u = r.get("U235").unwrap();
    assert!(u.reactions.iter().any(|x| x.kind == "fission"));
    let products: Vec<&str> = u.fission_yields[0].yields.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(products, vec!["I135"]);
    let alone = reduce(&chain, &["U235"], Some(0)).unwrap();
    let u = alone.get("U235").unwrap();
    assert!(u.fission_yields.is_empty() && !u.reactions.iter().any(|x| x.kind == "fission"), "no product left: fission dropped");
    assert!(full.nuclides.len() >= 7);
    // A reduced chain still writes and reads.
    assert_eq!(DepletionChainXml::parse(&export_to_xml(&full)).unwrap(), full);
}

fn am_chain() -> DepletionChainXml {
    let nuc = |name: &str, reactions: Vec<ChainReaction>| ChainNuclide {
        name: name.into(),
        half_life_seconds: None,
        decay_energy_ev: None,
        decays: vec![],
        reactions,
        fission_yields: vec![],
        fission_yield_parent: None,
        sources: vec![],
    };
    let ng = |t: &str, br: f64| ChainReaction { kind: "(n,gamma)".into(), target: Some(t.into()), q_ev: 5.5e6, branching_ratio: br };
    DepletionChainXml::from_nuclides(vec![
        nuc("Am241", vec![ng("Am242", 1.0)]),
        nuc("Am242", vec![]),
        nuc("Am242_m1", vec![]),
        nuc("Am243", vec![ng("Am244", 1.0)]),
        nuc("Am244", vec![]),
    ])
    .unwrap()
}

/// All-metastable ratios get the ground remainder: Am241 to Am242_m1 at
/// 0.1 leaves 0.9 to Am242, with the old Q.
#[test]
fn set_branch_ratios_fills_the_ground_remainder() {
    let mut c = am_chain();
    let w = set_branch_ratios(&mut c, &[("Am241", &[("Am242_m1", 0.1)])], "(n,gamma)", true, 1e-5).unwrap();
    assert!(w.is_empty());
    let r = &c.get("Am241").unwrap().reactions;
    assert_eq!(r.len(), 2);
    assert_eq!((r[0].target.as_deref(), r[0].branching_ratio), (Some("Am242_m1"), 0.1));
    assert_eq!((r[1].target.as_deref(), r[1].branching_ratio), (Some("Am242"), 0.9));
    assert_eq!(r[1].q_ev, 5.5e6);
    let got = get_branch_ratios(&c, "(n,gamma)");
    assert_eq!(got, vec![("Am241".to_owned(), vec![(Some("Am242_m1".to_owned()), 0.1), (Some("Am242".to_owned()), 0.9)])]);
}

#[test]
fn set_branch_ratios_validates_like_upstream() {
    let mut c = am_chain();
    assert!(set_branch_ratios(&mut c, &[("Am241", &[("Am242", 0.6), ("Am242_m1", 0.6)])], "(n,gamma)", true, 1e-5).is_err());
    assert!(set_branch_ratios(&mut c, &[("Cm244", &[("Am242", 1.0)])], "(n,gamma)", true, 1e-5).is_err());
    assert!(set_branch_ratios(&mut c, &[("Am242", &[("Am242_m1", 1.0)])], "(n,gamma)", true, 1e-5).is_err());
    assert!(set_branch_ratios(&mut c, &[("Am241", &[("Am242", 0.5)])], "(n,gamma)", true, 1e-5).is_err(), "ground given, sum < 1");
    // Lenient: the bad parent is skipped with a warning, the good one set.
    let w = set_branch_ratios(&mut c, &[("Cm244", &[("Am242", 1.0)]), ("Am241", &[("Am242_m1", 0.2), ("Am242", 0.8)])], "(n,gamma)", false, 1e-5).unwrap();
    assert_eq!(w.len(), 1);
    assert_eq!(c.get("Am241").unwrap().reactions.len(), 2);
    assert_eq!(secondaries_of("(n,a)"), &["He4"]);
    assert_eq!(REACTION_SECONDARIES.len(), 84);
}
