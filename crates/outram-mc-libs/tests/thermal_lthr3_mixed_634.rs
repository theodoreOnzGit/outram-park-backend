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

#![cfg(feature = "std")] // needs files, threads or reports (gh:#1605)

use outram_mc_libs::material::thermal::{EndfElasticOptions, ThermalElastic, ThermalScattering};
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
    // Deck 67's THERMR card has natom = 2; passed through the new input
    // (gh:#634 decision 2026-11-17), not applied by hand.
    const DECK_67_NATOM: f64 = 2.0;
    let endf = ThermalScattering::from_endf_file_with_options(
        &tape,
        15,
        400.0,
        "D in 7LiD",
        EndfElasticOptions::default().with_natom(DECK_67_NATOM),
    )
    .expect("ENDF route with natom");
    let endf_default =
        ThermalScattering::from_endf_file(&tape, 15, 400.0, "D in 7LiD").expect("ENDF route, default");
    let raw = njoy_outram_park_fork::acer::read::read(&ace).expect("reference ACE");
    let acer = ThermalScattering::from_ace(&raw, "D in 7LiD").expect("ACE route");

    let ThermalElastic::Mixed(coh, inc) = endf.elastic() else {
        panic!("ENDF route built {}, not mixed", endf.elastic().kind_name());
    };
    let ThermalElastic::Mixed(ace_coh, ace_inc) = acer.elastic() else {
        panic!("ACE route built {}, not mixed", acer.elastic().kind_name());
    };

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
        let (ia, ib) = (ace_inc.cross_section(e), inc.cross_section(e));
        worst_inc = worst_inc.max((ib - ia).abs() / ia);
        let a = acer.elastic_xs(e);
        worst_total = worst_total.max((b - a).abs() / a);
        println!("E = {e:.4e} eV: coh ACE {ca:.6e} ENDF {cb:.6e} | inc ACE {ia:.6e} ENDF(natom=2) {ib:.6e}");
        // The default route (no natom) divides by the tape's B(6) = 1 here, so its
        // incoherent part is exactly natom times ours; the coherent part is
        // identical (never divided by natom).
        let ThermalElastic::Mixed(dcoh, dinc) = endf_default.elastic() else { unreachable!() };
        assert_eq!(dcoh.cross_section(e).to_bits(), cb.to_bits());
        assert_eq!(dinc.cross_section(e), ib * DECK_67_NATOM);
    }
    println!(
        "worst rel. diff 1e-3..1 eV: coherent {worst_coh:.3e}, incoherent (ENDF, natom = 2) {worst_inc:.3e}, \
         total sigma_el {worst_total:.3e}"
    );
    assert!(worst_coh < 1.0e-2, "coherent parts differ by {worst_coh:.3e}");
    assert!(worst_inc < 1.0e-2, "incoherent parts differ by {worst_inc:.3e}");
    assert!(worst_total < 1.0e-2, "total sigma_el differs by {worst_total:.3e}");
}

/// **The default is unchanged (gh:#634, decision 2026-11-17).** Without a
/// `natom` the incoherent elastic part is divided by the tape's MT=4 B(6), as
/// before and as OpenMC's `from_endf`. Pinned on a tape that is held in the
/// repository (H in ZrH, incoherent-only, MAT 7): `with_natom(B(6))` is
/// bit-identical to the default (`from_endf_file`)
/// (factor `B(6)/B(6)` = 1); `with_natom(2 B(6))` halves sigma exactly; the
/// coherent-free elastic of other tapes is untouched; a bad `natom` is refused.
/// Predicted before the run: all four hold exactly (the rescale is a single
/// multiplication by an exact power of two or one).
#[test]
fn default_keeps_the_tape_b6_and_natom_rescales_the_incoherent_part_only() {
    use njoy_outram_park_fork::thermr::scattering::IncoherentElasticScattering;
    use njoy_outram_park_fork::units::Temperature;
    use uom::si::thermodynamic_temperature::kelvin;

    let Some(path) = njoy_outram_park_fork::reference_data::reference_endf_or_skip(
        "tsl-HinZrH-ENDF8.0.endf",
        "#634 natom default pin",
    ) else {
        return;
    };
    let base = ThermalScattering::from_endf_file(&path, 7, 296.0, "H in ZrH").unwrap();
    let tape = njoy_outram_park_fork::endf::tape::Tape::read_file(&path).unwrap();
    let b6 = IncoherentElasticScattering::from_tape(&tape, 7, Temperature::new::<kelvin>(296.0))
        .unwrap()
        .principal_atom_count();
    let same_b6 = ThermalScattering::from_endf_file_with_options(
        &path,
        7,
        296.0,
        "H in ZrH",
        EndfElasticOptions::default().with_natom(b6),
    )
    .unwrap();
    let double = ThermalScattering::from_endf_file_with_options(
        &path,
        7,
        296.0,
        "H in ZrH",
        EndfElasticOptions::default().with_natom(2.0 * b6),
    )
    .unwrap();
    println!("ZrH B(6) = {b6}");
    for e in [1.0e-3, 0.0253, 0.3] {
        let d = base.elastic_xs(e);
        assert!(d > 0.0);
        assert_eq!(same_b6.elastic_xs(e).to_bits(), d.to_bits());
        assert_eq!(double.elastic_xs(e).to_bits(), (d * 0.5).to_bits());
        // Inelastic is not touched by the elastic option.
        assert_eq!(double.inelastic_xs(e).to_bits(), base.inelastic_xs(e).to_bits());
    }
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let r = ThermalScattering::from_endf_file_with_options(
            &path,
            7,
            296.0,
            "H in ZrH",
            EndfElasticOptions::default().with_natom(bad),
        );
        assert!(r.is_err(), "natom = {bad} must be refused");
    }
}
