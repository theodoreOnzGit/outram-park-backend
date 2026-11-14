//! GitHub #634: an ENDF `LTHR = 3` (coherent + incoherent elastic) tape lost
//! its incoherent part on the ENDF route (`build_elastic_channel` returned
//! the coherent part alone). Upstream processes both (`thermr.f90:432-434`).
//!
//! Methodology: NJOY2016 test 67's tape, D in 7LiD (ENDF/B-VIII.1 beta, MAT
//! 15, `LTHR = 3`), at the deck's 400 K. Built on the ENDF route, the elastic
//! channel must be `Mixed`, and its σ_el(E) is compared with the ACE route on
//! NJOY2016's own reference ACE of the same deck (`tests/67/referenceTape71`,
//! which this workspace's ACER already matches word for word,
//! `njoy-outram-park-fork/tests/deck_driver_tapes_vs_njoy2016.rs`). The
//! incoherent part is then checked to be what the ENDF route adds: σ_el minus
//! the coherent part equals the incoherent table's σ.
//!
//! Prediction written before the run (posted on #634): Mixed; ENDF vs ACE
//! σ_el within 1 % from 1e-3 to 1 eV (the ENDF route evaluates the
//! incoherent formula on its own log grid, the ACE route interpolates THERMR's
//! tabulation); before the fix the ENDF route would miss by the whole
//! incoherent part.
//!
//! Data: NJOY2016's test resources are not committed here (gitignored
//! `upstream_source/`); the test reads `$NJOY2016_ROOT` or
//! `crates/njoy-outram-park-fork/upstream_source/NJOY2016` and SKIPs when the
//! files are absent.
//!
//! Result (2026-11-14, release, 4-core shared container): the ENDF route
//! builds `Mixed`. Over 31 energies 1e-3..1 eV the coherent part agrees with
//! NJOY's ACE to **5.7e-6** and the incoherent part to **2.3e-3** once the
//! deck's `natom = 2` is applied. **The prediction on the total missed by a
//! factor 2 in the incoherent part**: NJOY's `iel` divides SB by the THERMR
//! card's `natom` (2 in deck 67), the ENDF route by MF=7/MT=4 B(6) (1 on this
//! tape). That is a convention set by a processing input, recorded on #634
//! for the maintainer, not fitted here. The 2.3e-3 is the ENDF route's own
//! log grid against THERMR's tabulation, largest near 1 eV.

use outram_mc_libs::material::thermal::{ThermalElastic, ThermalScattering};
use std::path::PathBuf;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT").map(PathBuf::from).unwrap_or_else(|_| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../njoy-outram-park-fork/upstream_source/NJOY2016")
    })
}

#[test]
fn lthr3_tape_keeps_both_elastic_parts_and_matches_the_ace_route() {
    let root = njoy_root();
    let tape = root.join("tests/resources/tsl-DinLiD-ENDF8.1-beta.endf");
    let ace = root.join("tests/67/referenceTape71");
    if !tape.exists() || !ace.exists() {
        eprintln!("SKIP: NJOY2016 test 67 data not found under {}", root.display());
        return;
    }
    let endf = ThermalScattering::from_endf_file(&tape, 15, 400.0, "D in 7LiD").expect("ENDF route");
    let raw = njoy_outram_park_fork::acer::read::read(&ace).expect("reference ACE");
    let acer = ThermalScattering::from_ace(&raw, "D in 7LiD").expect("ACE route");

    let ThermalElastic::Mixed(coh, inc) = endf.elastic() else {
        panic!("ENDF route built {}, not mixed", endf.elastic().kind_name());
    };
    let ThermalElastic::Mixed(ace_coh, ace_inc) = acer.elastic() else {
        panic!("ACE route built {}, not mixed", acer.elastic().kind_name());
    };

    // NJOY's `iel` divides SB by the THERMR card's `natom` (`c1 = sb/(2*natom)`,
    // thermr.f90 `iel`), which deck 67 sets to 2; the ENDF route here divides
    // by MF=7/MT=4 B(6) = 1 for this tape (as OpenMC's `from_endf` applies no
    // natom). Coherent elastic is not divided by `natom` on either side. So
    // the incoherent parts are compared through the deck's card value, an
    // input from outside this comparison, not a fitted factor.
    const DECK_67_NATOM: f64 = 2.0;
    let (mut worst_coh, mut worst_inc, mut worst_total): (f64, f64, f64) = (0.0, 0.0, 0.0);
    for k in 0..=30 {
        let e = 1.0e-3 * 1000f64.powf(k as f64 / 30.0);
        let b = endf.elastic_xs(e);
        // The incoherent part is what the fix adds.
        let parts = coh.cross_section(e) + inc.cross_section(e);
        assert_eq!(b.to_bits(), parts.to_bits());
        assert!(inc.cross_section(e) > 0.0);
        let (ca, cb) = (ace_coh.cross_section(e), coh.cross_section(e));
        if ca > 0.0 {
            worst_coh = worst_coh.max((cb - ca).abs() / ca);
        }
        let (ia, ib) = (ace_inc.cross_section(e), inc.cross_section(e) / DECK_67_NATOM);
        worst_inc = worst_inc.max((ib - ia).abs() / ia);
        let a = acer.elastic_xs(e);
        worst_total = worst_total.max((b - a).abs() / a);
        println!("E = {e:.4e} eV: coh ACE {ca:.6e} ENDF {cb:.6e} | inc ACE {ia:.6e} ENDF/natom {ib:.6e}");
    }
    println!(
        "worst rel. diff 1e-3..1 eV: coherent {worst_coh:.3e}, incoherent (ENDF/natom) {worst_inc:.3e}, \
         total sigma_el without the natom card {worst_total:.3e}"
    );
    assert!(worst_coh < 1.0e-2, "coherent parts differ by {worst_coh:.3e}");
    assert!(worst_inc < 1.0e-2, "incoherent parts differ by {worst_inc:.3e}");
}
