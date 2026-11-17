//! Tests for the spectrum-tracking driver (gh:#1630). Synthetic spectrum
//! callbacks stand in for transport; predictions in each doc comment.

use super::*;
use crate::depletion::operator::deplete_coupled;

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

fn initial() -> Vec<(String, f64)> {
    let n_u = 0.02320_f64;
    vec![("U235".to_string(), 0.0425 * n_u), ("U238".to_string(), 0.9575 * n_u)]
}

fn settings() -> BurnupSettings {
    BurnupSettings { n_steps: 8, step_days: 30.0, ..BurnupSettings::default() }
}

/// A spectrum that hardens as U-235 burns: two groups split at 0.625 eV,
/// the fast weight growing with the burnt fraction (strength `k`).
fn hardening(k: f64) -> impl FnMut(&[(String, f64)]) -> Option<(Vec<f64>, Vec<f64>)> {
    let n0 = initial()[0].1;
    move |inv: &[(String, f64)]| {
        let n = inv.iter().find(|(x, _)| x == "U235").map_or(0.0, |(_, d)| *d);
        Some((vec![1e-5, 0.625, 2.0e7], vec![1.0, 0.2 + k * (1.0 - n / n0)]))
    }
}

fn eol(r: &BurnupResult, nuc: &str) -> f64 {
    r.eol().density(nuc)
}

/// Tolerance 0 adopts every changed spectrum: the history is
/// `deplete_coupled`'s bit for bit, with the same transport calls.
#[test]
fn zero_tolerance_is_deplete_coupled() {
    let chain = DepletionChain::simple();
    let t = deplete_tracking(&chain, &initial(), &settings(), 0.0, hardening(5.0));
    let c = deplete_coupled(&chain, &initial(), &settings(), hardening(5.0));
    for (a, b) in t.result.steps.iter().zip(&c.steps) {
        assert_eq!(a.densities, b.densities);
        assert_eq!(a.flux.to_bits(), b.flux.to_bits());
    }
    assert_eq!(t.transport_calls, settings().n_steps);
    assert_eq!(t.refreshes(), settings().n_steps - 1);
}

/// Composition-only control (#1630, verification 3): a fixed spectrum while
/// U-235 burns. Prediction: `c = 0` exactly at every step, no refresh,
/// though the inventory changes by percent.
#[test]
fn a_fixed_spectrum_never_fires_however_n_changes() {
    let chain = DepletionChain::simple();
    let fixed = |_: &[(String, f64)]| Some((vec![1e-5, 0.625, 2.0e7], vec![1.0, 0.2]));
    let t = deplete_tracking(&chain, &initial(), &settings(), 0.0, fixed);
    assert_eq!(t.refreshes(), 0);
    assert!(t.records.iter().all(|r| r.criterion.value == 0.0 && r.criterion.dominant.is_none()));
    let burnt = 1.0 - eol(&t.result, "U235") / initial()[0].1;
    assert!(burnt > 0.01, "the composition did change: {burnt}");
}

/// Infinite tolerance freezes the BOL spectrum: equal to `deplete_coupled`
/// whose callback answers only once.
#[test]
fn infinite_tolerance_freezes_the_first_spectrum() {
    let chain = DepletionChain::simple();
    let t = deplete_tracking(&chain, &initial(), &settings(), f64::INFINITY, hardening(5.0));
    let mut first = hardening(5.0);
    let mut called = false;
    let c = deplete_coupled(&chain, &initial(), &settings(), |inv| {
        if called {
            None
        } else {
            called = true;
            first(inv)
        }
    });
    assert_eq!(t.result.eol().densities, c.eol().densities);
    assert_eq!(t.refreshes(), 0);
    assert!(t.records.iter().all(|r| r.criterion.value > 0.0), "the spectrum did move");
}

/// Convergence (#1630, verifications 1 and 2): as the tolerance tightens,
/// the end-of-life Pu-239 approaches the refresh-every-step reference and
/// the refresh count rises. Prediction: the error is non-increasing over
/// the tolerances tried, and zero at tolerance 0.
///
/// Measured 2026-11-17 (`DepletionChain::simple` has no Pu-239, so U-235
/// is followed; 8 × 30 d, hardening strength 5): tolerance 1e-2 and 3e-3,
/// 0 refreshes, EOL U-235 error 5.245e-5; 1e-3, 2 refreshes, 1.263e-5;
/// 3e-4 and below, 7 refreshes (every step), 0. The criterion peaked at
/// 2.5e-3 with frozen data and at 3.6e-4 between refreshes.
#[test]
fn tightening_the_tolerance_converges_to_the_reference() {
    let chain = DepletionChain::simple();
    let nuc = chain.nuclide_names().into_iter().find(|n| n.starts_with("Pu239")).unwrap_or("U235").to_owned();
    let reference = eol(&deplete_coupled(&chain, &initial(), &settings(), hardening(5.0)), &nuc);
    let mut last_err = f64::INFINITY;
    let mut last_refreshes = 0;
    for tol in [1e-2, 3e-3, 1e-3, 3e-4, 1e-4, 3e-5, 1e-5, 0.0] {
        let t = deplete_tracking(&chain, &initial(), &settings(), tol, hardening(5.0));
        let err = ((eol(&t.result, &nuc) - reference) / reference).abs();
        #[cfg(feature = "std")]
        std::println!("tol {tol:e}: refreshes {}, max c {:.3e}, |{nuc} error| {err:.3e}", t.refreshes(), t.records.iter().map(|r| r.criterion.value).fold(0.0, f64::max));
        assert!(err <= last_err * (1.0 + 1e-12), "tol {tol}: {err} after {last_err}");
        assert!(t.refreshes() >= last_refreshes);
        last_err = err;
        last_refreshes = t.refreshes();
    }
    assert_eq!(last_err, 0.0);
}

/// Removal pair (#1630, verification 5, on the chain this crate holds):
/// xenon sparged at 1/(10 min) as in an MSR off-gas system. Signs known
/// before running: Xe-135 and its decay product Cs-135 fall, and with less
/// Xe-135 absorbing, `k_inf` at end of life rises.
///
/// Measured 2026-11-17 (8 × 30 d, tolerance 1e-3): Xe-135 1.81e-10 vs
/// 5.43e-9 (a factor 30 lower), Cs-135 8.16e-8 vs 2.57e-6, `k_inf`
/// 1.855095 vs 1.827911 (+0.0272). All three signs as predicted.
#[test]
fn xenon_removal_has_the_known_sign() {
    use crate::depletion::external_rates::RateUnit;
    let chain = DepletionChain::simple();
    let names: Vec<String> = chain.nuclide_names().iter().map(|n| (*n).to_owned()).collect();
    let mut tr = TransferRates::new(vec!["fuel".into()], names, settings().n_steps);
    tr.set_transfer_rate("fuel", &["Xe"], 6.0, RateUnit::PerHour, None, None).unwrap();
    let p = deplete_tracking_pair(&chain, &initial(), &settings(), 1e-3, &tr, hardening(5.0), hardening(5.0)).unwrap();
    let (w, wo) = (p.with_removal.result.eol(), p.without_removal.result.eol());
    for n in ["Xe135", "Cs135"] {
        #[cfg(feature = "std")]
        std::println!("{n}: with removal {:.4e}, without {:.4e}", w.density(n), wo.density(n));
        assert!(w.density(n) < wo.density(n), "{n}");
    }
    #[cfg(feature = "std")]
    std::println!("k_inf EOL: with {:.6}, without {:.6}", w.k_inf, wo.k_inf);
    assert!(w.k_inf > wo.k_inf);
    assert_eq!(p.without_removal.result.eol().densities, deplete_tracking(&chain, &initial(), &settings(), 1e-3, hardening(5.0)).result.eol().densities);
}

/// Chain regeneration (#1630). Built from OpenMC's `chain_simple.xml` so
/// both chains share one nuclide order. Predictions: (1) a regenerator that
/// returns the initial chain changes nothing, bit for bit; (2) one that
/// sends I-135 capture to Xe-135 instead of Xe-136 from the first refresh
/// on raises EOL Xe-135 and lowers Xe-136; (3) a chain with another
/// nuclide list is refused.
///
/// Measured 2026-11-17 (tolerance 1e-3, 8 × 30 d): EOL Xe-135 5.427016e-9
/// against 5.426740e-9 unregenerated (+5.1e-5 relative), Xe-136 8.547135e-6
/// against 8.547210e-6 (−8.8e-6). Signs as predicted; small, as I-135's
/// capture is a small share of its loss (it mostly decays).
#[cfg(feature = "std")]
#[test]
fn regenerated_chains_take_effect_at_refreshes() {
    use njoy_outram_park_fork::hdf5::depletion_chain_xml::DepletionChainXml;
    let text = include_str!("../../../../njoy-outram-park-fork/verification_and_validation/depletion_chain_xml/openmc_inputs/chain_simple.xml");
    let xml = DepletionChainXml::parse(text).unwrap();
    let base = DepletionChain::from_chain_xml(&xml, 0.0253).unwrap();
    let mut alt = xml.clone();
    for n in alt.nuclides.iter_mut().filter(|n| n.name == "I135") {
        for r in n.reactions.iter_mut().filter(|r| r.kind == "(n,gamma)") {
            r.target = Some("Xe135".into());
        }
    }
    let alt = DepletionChain::from_chain_xml(&alt, 0.0253).unwrap();
    let plain = deplete_tracking(&base, &initial(), &settings(), 1e-3, hardening(5.0));

    let same = deplete_tracking_regenerating(&base, &initial(), &settings(), 1e-3, hardening(5.0), |_, _| Ok(Some(base.clone()))).unwrap();
    assert_eq!(same.result.eol().densities, plain.result.eol().densities);
    assert_eq!(same.chains.len(), 1 + same.refreshes());

    let changed = deplete_tracking_regenerating(&base, &initial(), &settings(), 1e-3, hardening(5.0), |_, _| Ok(Some(alt.clone()))).unwrap();
    assert!(changed.refreshes() > 0);
    let (c, p) = (changed.result.eol(), plain.result.eol());
    std::println!("Xe135 {:.6e} vs {:.6e}; Xe136 {:.6e} vs {:.6e}", c.density("Xe135"), p.density("Xe135"), c.density("Xe136"), p.density("Xe136"));
    assert!(c.density("Xe135") > p.density("Xe135"));
    assert!(c.density("Xe136") < p.density("Xe136"));

    let other = DepletionChain::simple();
    let bad = deplete_tracking_regenerating(&base, &initial(), &settings(), 1e-3, hardening(5.0), |_, _| Ok(Some(other.clone())));
    assert!(bad.is_err() || other.nuclide_names() == base.nuclide_names());
}
