//! Tests for the `Chain.from_endf` port (gh:#1621). Predictions are in each
//! doc comment; measured values are printed (`--nocapture`) and recorded.

use super::*;
use std::path::PathBuf;

fn data_dir() -> Option<PathBuf> {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/ace/endf/endf-b-viii.0");
    if d.join("decay").is_dir() {
        Some(d)
    } else {
        std::println!("SKIP: reference-data/ace (ENDF/B-VIII.0 sublibraries) not checked out");
        None
    }
}

fn tape(p: &std::path::Path) -> Tape {
    Tape::from_bytes(&std::fs::read(p).unwrap()).unwrap()
}

fn tapes_in(dir: &std::path::Path, prefix: &str) -> Vec<Tape> {
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(prefix) && p.extension().is_some_and(|x| x == "endf"))
        .collect();
    names.sort();
    names.iter().map(|p| tape(p)).collect()
}

/// Python `str(float).strip('0').replace('.', '')`, digit by digit.
#[test]
fn decay_mode_codes_read_like_python() {
    assert_eq!(get_decay_modes(1.0).unwrap(), vec!["beta-"]);
    assert_eq!(get_decay_modes(1.5).unwrap(), vec!["beta-", "n"]);
    assert_eq!(get_decay_modes(2.4).unwrap(), vec!["ec/beta+", "alpha"]);
    assert_eq!(get_decay_modes(10.0).unwrap(), vec!["unknown"]);
    assert!(get_decay_modes(0.0).unwrap().is_empty(), "upstream strips gamma's '0.0' to nothing");
    assert_eq!(get_decay_modes(3.0).unwrap(), vec!["IT"]);
}

/// Cs-137 (ENDF/B-VIII.0): β⁻ to Ba-137m (94.7 %) and to Ba-137 (5.3 %),
/// half-life 30.08 a. Prediction: two modes with those daughters, ratios
/// summing to 1, half-life 9.49e8 s within 0.1 %.
#[test]
fn cs137_decay_record() {
    let Some(d) = data_dir() else { return };
    let r = read_decay(&tape(&d.join("decay/dec-055_Cs_137.endf"))).unwrap();
    std::println!("{r:?}");
    assert_eq!(r.name, "Cs137");
    assert!(!r.stable);
    assert!((r.half_life / 9.492e8 - 1.0).abs() < 1e-3, "{}", r.half_life);
    let ds: Vec<(Option<&str>, f64)> = r.modes.iter().map(|m| (m.daughter.as_deref(), m.branching_ratio)).collect();
    assert!(ds.iter().any(|(d, b)| *d == Some("Ba137_m1") && (*b - 0.947).abs() < 2e-3), "{ds:?}");
    assert!(ds.iter().any(|(d, _)| *d == Some("Ba137")), "{ds:?}");
}

/// The whole decay and NFY sublibraries with the five thorium-cycle and
/// uranium tapes. Predictions: U-235 carries fission (Q from MT=18, about
/// 193 MeV) and (n,γ) → U236; Th-232 (n,γ) → Th233; Pa-233 (n,γ) → Pa234;
/// U-233 has its own yields at the NFY energies (0.0253 eV, 500 keV,
/// 14 MeV); the chain exports and parses back unchanged.
#[test]
fn chain_from_the_full_sublibraries() {
    let Some(d) = data_dir() else { return };
    let decay = tapes_in(&d.join("decay"), "dec-");
    let nfy = tapes_in(&d.join("nfy"), "nfy-");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let mut neutron = vec![
        tape(&d.join("n-090_Th_232-ENDF8.0.endf")),
        tape(&d.join("n-091_Pa_233-ENDF8.0.endf")),
        tape(&d.join("n-092_U_233-ENDF8.0.endf")),
    ];
    for f in ["n-092_U_235-ENDF8.0.endf", "n-092_U_238.endf"] {
        if root.join(f).is_file() {
            neutron.push(tape(&root.join(f)));
        }
    }
    let (chain, report) = from_endf(&decay, &nfy, &neutron, None).unwrap();
    std::println!(
        "{} nuclides from {} decay, {} nfy, {} neutron tapes; renormalised {}, missing daughters {}, borrowed yields {}, replaced FP sets {}",
        chain.nuclides.len(),
        decay.len(),
        nfy.len(),
        neutron.len(),
        report.renormalised.len(),
        report.missing_daughter.len(),
        report.missing_fpy.len(),
        report.missing_fp.len()
    );
    let target = |n: &str, kind: &str| chain.get(n).and_then(|x| x.reactions.iter().find(|r| r.kind == kind)).and_then(|r| r.target.clone());
    assert_eq!(target("Th232", "(n,gamma)").as_deref(), Some("Th233"));
    assert_eq!(target("Pa233", "(n,gamma)").as_deref(), Some("Pa234"));
    let u233 = chain.get("U233").unwrap();
    let f = u233.reactions.iter().find(|r| r.kind == "fission").unwrap();
    std::println!("U233 fission Q {} eV, yield energies {:?}", f.q_ev, u233.fission_yields.iter().map(|y| y.energy_ev).collect::<Vec<_>>());
    assert!(f.q_ev > 1.8e8 && f.q_ev < 2.1e8);
    assert!(u233.fission_yield_parent.is_none() && !u233.fission_yields.is_empty());
    if chain.get("U235").is_some_and(|u| u.reactions.iter().any(|r| r.kind == "fission")) {
        assert_eq!(target("U235", "(n,gamma)").as_deref(), Some("U236"));
    }
    let xml = super::super::depletion_chain_ops::export_to_xml(&chain);
    let back = DepletionChainXml::parse(&xml).unwrap();
    if let Some((a, b)) = back.nuclides.iter().zip(&chain.nuclides).find(|(a, b)| a != b) {
        panic!("first difference:\n back {a:?}\n ours {b:?}");
    }
    assert_eq!(back, chain);
}
