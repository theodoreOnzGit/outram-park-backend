//! **`process` carries the 0 K elastic, and one RECONR serves many
//! temperatures** (GitHub #928, #934; epic #925).
//!
//! # Methodology
//!
//! Every check is an identity, so there is no tolerance anywhere.
//!
//! - **Default pinned** (the `tests/correct_physics_is_default.rs` pattern of
//!   `outram-mc-libs`): `process::cross_sections` with
//!   `CrossSectionSettings::default()` must return
//!   `NuclearCrossSections::elastic_0k` as `Some`. DBRC needs it, and the
//!   root `CLAUDE.md` makes physics the evaluation supplies default-on.
//! - **What it is:** at 293.6 K it must equal, bit for bit, the MT=2 of the
//!   0 K result (`cross_sections` at 0 K, which is RECONR written to the
//!   PENDF tape, the deck route 5's `0K/` ACE companion is built from). It
//!   must *differ* from the 293.6 K MT=2, or it would be the broadened
//!   section under another name.
//! - **Ablation:** `without_elastic_0k()` removes it and changes nothing else:
//!   the PENDF sections and the ACE table (Type 2, unrounded) are identical
//!   byte for byte.
//! - **Stages:** `cross_sections_at_temperatures` (one RECONR) at 0, 293.6 and
//!   600 K must equal `cross_sections` (one RECONR each) at each temperature:
//!   every PENDF energy and cross section, the 0 K elastic, PURR's tables
//!   (`to_f64s`) and the ACE table's Type-2 bytes.
//! - **Refusal:** a reconstruction made at one RECONR tolerance is refused by
//!   `cross_sections_at` with settings at another.
//!
//! Nuclides: ENDF/B-VIII.0 F-19 (MAT 925; resolved resonances, no
//! unresolved range) for the elastic checks, and Zn-70 (MAT 3043; has an
//! unresolved range, so PURR runs) for the stages, with PURR cut to
//! 15 bands (PURR refuses fewer) / 4 ladders / 200 samples so the test stays short. The cut changes
//! PURR's numbers, not whether the two routes agree.
//!
//! # Results (2026-10-17)
//!
//! All checks pass (identities; see the assertions). F-19's 0 K elastic has
//! the same number of points as the 0 K MT=2 and differs from the 293.6 K
//! MT=2. Tapes absent: every test skips.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::process::{self, CrossSectionSettings, NuclearCrossSections};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use njoy_outram_park_fork::units::Temperature;
use njoy_outram_park_fork::{MtReaction, NjoyError};
use uom::si::thermodynamic_temperature::kelvin;

fn kelvins(t: f64) -> Temperature {
    Temperature::new::<kelvin>(t)
}

fn tape(name: &str) -> Option<Tape> {
    let p = reference_endf_or_skip(name, "process-elastic-0k")?;
    Some(Tape::read_file(&p).expect("parse tape"))
}

fn bits(pairs: &[(f64, f64)]) -> Vec<(u64, u64)> {
    pairs
        .iter()
        .map(|&(e, s)| (e.to_bits(), s.to_bits()))
        .collect()
}

fn mt2(xs: &NuclearCrossSections) -> Vec<(f64, f64)> {
    xs.reaction(MtReaction::Mt2Elastic)
        .expect("MT=2")
        .pairs
        .clone()
}

/// Every PENDF section of `a` and `b`, compared bit for bit.
fn assert_same_pendf(a: &NuclearCrossSections, b: &NuclearCrossSections, what: &str) {
    assert_eq!(
        a.pendf.sections.len(),
        b.pendf.sections.len(),
        "{what}: section count"
    );
    for (x, y) in a.pendf.sections.iter().zip(&b.pendf.sections) {
        assert_eq!(x.mt, y.mt, "{what}: MT order");
        assert_eq!(x.qi.to_bits(), y.qi.to_bits(), "{what}: QI of {:?}", x.mt);
        assert_eq!(bits(&x.pairs), bits(&y.pairs), "{what}: MT {:?}", x.mt);
    }
}

fn ace_bytes(xs: &NuclearCrossSections) -> Vec<u8> {
    xs.to_ace().expect("ACER").to_type2_bytes().expect("Type 2")
}

#[test]
fn elastic_0k_is_on_by_default_and_is_the_0k_pendf_elastic() {
    let Some(f19) = tape("n-009_F_019-ENDF8.0.endf") else {
        return;
    };
    let s = CrossSectionSettings::default().without_heatr();
    let hot = process::cross_sections(&f19, kelvins(293.6), &CrossSectionSettings::default())
        .expect("293.6 K");
    let el0 = hot
        .elastic_0k
        .as_ref()
        .expect("the 0 K elastic is on by default");
    let cold = process::cross_sections(&f19, kelvins(0.0), &s).expect("0 K");
    assert_eq!(
        bits(el0),
        bits(&mt2(&cold)),
        "the 0 K elastic is the 0 K PENDF's MT=2"
    );
    assert_eq!(
        bits(cold.elastic_0k.as_ref().expect("at 0 K too")),
        bits(&mt2(&cold)),
        "at 0 K the 0 K elastic is the PENDF's own MT=2"
    );
    assert_ne!(
        bits(el0),
        bits(&mt2(&hot)),
        "the 0 K elastic must not be the broadened one"
    );
    eprintln!(
        "F-19: 0 K elastic {} points, 293.6 K MT=2 {} points",
        el0.len(),
        mt2(&hot).len()
    );
}

#[test]
fn without_elastic_0k_drops_it_and_changes_nothing_else() {
    let Some(f19) = tape("n-009_F_019-ENDF8.0.endf") else {
        return;
    };
    let on = CrossSectionSettings::default().without_heatr();
    let off = on.clone().without_elastic_0k();
    let a = process::cross_sections(&f19, kelvins(293.6), &on).expect("with");
    let b = process::cross_sections(&f19, kelvins(293.6), &off).expect("without");
    assert!(a.elastic_0k.is_some() && b.elastic_0k.is_none());
    assert_same_pendf(&a, &b, "elastic_0k ablation");
    assert_eq!(
        ace_bytes(&a),
        ace_bytes(&b),
        "the ACE table does not read the 0 K elastic"
    );
}

#[test]
fn one_reconr_for_many_temperatures_is_bit_for_bit() {
    let Some(zn70) = tape("n-030_Zn_070-ENDF8.0.endf") else {
        return;
    };
    let s = CrossSectionSettings::default()
        .with_purr(15, 4, 200)
        .without_heatr();
    let temps = [kelvins(0.0), kelvins(293.6), kelvins(600.0)];
    let many = process::cross_sections_at_temperatures(&zn70, &temps, &s).expect("one RECONR");
    assert_eq!(many.len(), temps.len());
    for (t, m) in temps.iter().zip(&many) {
        let one = process::cross_sections(&zn70, *t, &s).expect("cross_sections");
        let what = format!("Zn-70 at {} K", t.get::<kelvin>());
        assert_eq!(
            m.temperature.get::<kelvin>().to_bits(),
            t.get::<kelvin>().to_bits()
        );
        assert_same_pendf(m, &one, &what);
        assert_eq!(
            m.elastic_0k.as_deref().map(bits),
            one.elastic_0k.as_deref().map(bits),
            "{what}: 0 K elastic"
        );
        let urr = |x: &NuclearCrossSections| {
            x.urr
                .as_ref()
                .map(|u| u.to_f64s().iter().map(|v| v.to_bits()).collect::<Vec<_>>())
        };
        assert_eq!(urr(m), urr(&one), "{what}: PURR tables");
        assert_eq!(ace_bytes(m), ace_bytes(&one), "{what}: ACE");
    }
    assert!(many[0].urr.is_none(), "no PURR at 0 K");
    assert!(
        many[1].urr.is_some(),
        "Zn-70 has an unresolved range, so PURR runs at 293.6 K"
    );
    // The 0 K elastic does not depend on the temperature asked for.
    let e0 = many[0].elastic_0k.as_deref().map(bits);
    assert!(many.iter().all(|m| m.elastic_0k.as_deref().map(bits) == e0));
}

#[test]
fn a_reconstruction_at_another_tolerance_is_refused() {
    let Some(f19) = tape("n-009_F_019-ENDF8.0.endf") else {
        return;
    };
    let s = CrossSectionSettings::default().without_heatr();
    let recon = process::reconstruct(&f19, &s.clone().with_reconr_tolerance(0.01)).expect("RECONR");
    assert_eq!(recon.mat, 925);
    assert!((recon.za() - 9019.0).abs() < 0.5 && recon.awr() > 18.0);
    match process::cross_sections_at(&recon, kelvins(293.6), &s) {
        Err(NjoyError::InvalidInput(m)) => assert!(m.contains("tolerance"), "{m}"),
        other => panic!("expected a refusal, got {:?}", other.map(|x| x.mat)),
    }
    assert!(matches!(
        process::cross_sections_at_temperatures(&f19, &[kelvins(-1.0)], &s),
        Err(NjoyError::InvalidInput(_))
    ));
}
