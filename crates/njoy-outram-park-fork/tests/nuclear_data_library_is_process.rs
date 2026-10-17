//! **`NuclearDataLibrary` is `process`, bit for bit** (GitHub #933, epic
//! #925: "functional core, OOP shell").
//!
//! # Methodology
//!
//! The object's methods are thin calls into `process`; these tests pin that
//! the two styles give the same numbers, with no tolerance:
//!
//! - **Same result, default physics.** `from_file(F-19).reconstruct(0.001)
//!   .broaden(293.6 K)` against `process::cross_sections(&tape, 293.6 K,
//!   &CrossSectionSettings::default())`: every PENDF energy and cross
//!   section, the 0 K elastic, PURR's tables, and the ACE table's Type-2
//!   bytes (which carry HEATR's heating column). The object's default
//!   settings must equal `CrossSectionSettings::default()`, so the physics
//!   defaults (PURR, HEATR, the 0 K elastic) are the same on both paths.
//! - **Ablations match.** `without_purr()`, `without_heatr()` and
//!   `without_elastic_0k()` on the object against the same `without_*` on
//!   the settings, on Zn-70 (it has an unresolved range, so PURR matters),
//!   with PURR cut to 15 bands / 4 ladders / 200 samples to stay short.
//! - **A second `broaden` starts from 0 K (#617).** `broaden(600 K)` then
//!   `broaden(293.6 K)` must equal a single `broaden(293.6 K)`.
//! - **Several temperatures from one RECONR.** The object's
//!   `cross_sections_at_temperatures` against `process::cross_sections` at
//!   each.
//! - **MAT, ZA and AWR come from the tape.** F-19: MAT 925, ZA 9019, AWR from
//!   MF=1/MT=451. A tape with two materials is refused.
//! - **PENDF out.** `to_pendf_tape()` written with `Tape::write` and read back
//!   with `Tape::read` gives the processed sections bit for bit, and the
//!   temperature in MF=1/MT=451's `hdatio` record.
//! - **Refusals.** `broaden` and `to_ace` before `reconstruct` return
//!   `NjoyError::InvalidInput`, not a panic.
//!
//! # Results (2026-10-17)
//!
//! All pass (identities). Tapes absent: each test skips.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::interface::NuclearDataLibrary;
use njoy_outram_park_fork::process::{self, CrossSectionSettings, NuclearCrossSections};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
use njoy_outram_park_fork::{MtReaction, NjoyError};
use uom::si::area::barn;
use uom::si::energy::electronvolt;
use uom::si::thermodynamic_temperature::kelvin;

const F19: &str = "n-009_F_019-ENDF8.0.endf";
const ZN70: &str = "n-030_Zn_070-ENDF8.0.endf";

fn kelvins(t: f64) -> Temperature {
    Temperature::new::<kelvin>(t)
}

fn path(name: &str) -> Option<std::path::PathBuf> {
    reference_endf_or_skip(name, "nuclear-data-library")
}

fn bits(pairs: &[(f64, f64)]) -> Vec<(u64, u64)> {
    pairs
        .iter()
        .map(|&(e, s)| (e.to_bits(), s.to_bits()))
        .collect()
}

/// Everything a `NuclearCrossSections` holds, compared bit for bit.
fn assert_same(a: &NuclearCrossSections, b: &NuclearCrossSections, what: &str) {
    assert_eq!(a.mat, b.mat, "{what}: MAT");
    assert_eq!(a.settings, b.settings, "{what}: settings");
    assert_eq!(
        a.temperature.get::<kelvin>().to_bits(),
        b.temperature.get::<kelvin>().to_bits(),
        "{what}: temperature"
    );
    assert_eq!(
        a.pendf.sections.len(),
        b.pendf.sections.len(),
        "{what}: sections"
    );
    for (x, y) in a.pendf.sections.iter().zip(&b.pendf.sections) {
        assert_eq!((x.mt, x.qi.to_bits()), (y.mt, y.qi.to_bits()), "{what}");
        assert_eq!(bits(&x.pairs), bits(&y.pairs), "{what}: MT {:?}", x.mt);
    }
    assert_eq!(
        a.elastic_0k.as_deref().map(bits),
        b.elastic_0k.as_deref().map(bits),
        "{what}: 0 K elastic"
    );
    let urr = |x: &NuclearCrossSections| {
        x.urr
            .as_ref()
            .map(|u| u.to_f64s().iter().map(|v| v.to_bits()).collect::<Vec<_>>())
    };
    assert_eq!(urr(a), urr(b), "{what}: PURR tables");
    assert_eq!(
        a.heating.is_some(),
        b.heating.is_some(),
        "{what}: HEATR ran on both"
    );
    let ace =
        |x: &NuclearCrossSections| x.to_ace().expect("ACER").to_type2_bytes().expect("Type 2");
    assert_eq!(ace(a), ace(b), "{what}: ACE table");
}

#[test]
fn object_equals_free_function_with_the_same_physics_defaults() {
    let Some(p) = path(F19) else { return };
    let lib = NuclearDataLibrary::from_file(&p).expect("load");
    assert_eq!(
        lib.settings(),
        &CrossSectionSettings::default(),
        "same defaults as process"
    );
    let lib = lib
        .reconstruct(1.0e-3)
        .expect("RECONR")
        .broaden(kelvins(293.6))
        .expect("BROADR");
    let tape = Tape::read_file(&p).expect("tape");
    let free = process::cross_sections(&tape, kelvins(293.6), &CrossSectionSettings::default())
        .expect("process");
    let obj = lib.cross_sections().expect("processed");
    assert_same(obj, &free, "F-19 default");
    assert!(
        obj.elastic_0k.is_some() && obj.heating.is_some(),
        "DBRC data and HEATR on by default"
    );
    assert_eq!(
        lib.elastic_0k().map(<[_]>::len),
        free.elastic_0k.as_ref().map(Vec::len)
    );
    // The object's ACE is process::ace's.
    let ace = process::ace(&tape, kelvins(293.6), &CrossSectionSettings::default()).expect("ace");
    assert_eq!(
        lib.to_ace().expect("ACER").to_type2_bytes().unwrap(),
        ace.to_type2_bytes().unwrap()
    );
    // A query reads the processed PENDF.
    let e = 1.0e5;
    let el = free.reaction(MtReaction::Mt2Elastic).unwrap();
    let want = njoy_outram_park_fork::reconr::eval_lin_lin(&el.pairs, e);
    let got = lib
        .elastic_xs(NeutronEnergy::new::<electronvolt>(e))
        .get::<barn>();
    assert_eq!(got.to_bits(), want.to_bits());
}

#[test]
fn ablations_on_the_object_are_the_settings_ablations() {
    let Some(p) = path(ZN70) else { return };
    let tape = Tape::read_file(&p).expect("tape");
    let base = CrossSectionSettings::default().with_purr(15, 4, 200);
    let t = kelvins(293.6);
    let load = || {
        NuclearDataLibrary::from_file(&p)
            .expect("load")
            .with_settings(base.clone())
    };
    let cases: [(
        &str,
        fn(NuclearDataLibrary) -> NuclearDataLibrary,
        CrossSectionSettings,
    ); 4] = [
        ("defaults", |l| l, base.clone()),
        (
            "without_purr",
            NuclearDataLibrary::without_purr,
            base.clone().without_purr(),
        ),
        (
            "without_heatr",
            NuclearDataLibrary::without_heatr,
            base.clone().without_heatr(),
        ),
        (
            "without_elastic_0k",
            NuclearDataLibrary::without_elastic_0k,
            base.clone().without_elastic_0k(),
        ),
    ];
    for (name, ablate, settings) in cases {
        let lib = ablate(load())
            .reconstruct(1.0e-3)
            .unwrap()
            .broaden(t)
            .unwrap();
        let free = process::cross_sections(&tape, t, &settings).expect("process");
        assert_same(lib.cross_sections().unwrap(), &free, name);
    }
    let with_purr = load().reconstruct(1.0e-3).unwrap().broaden(t).unwrap();
    assert!(
        with_purr.urr().is_some(),
        "Zn-70 has an unresolved range: PURR on by default"
    );
}

#[test]
fn a_second_broaden_starts_again_from_0k() {
    let Some(p) = path(F19) else { return };
    let r = NuclearDataLibrary::from_file(&p)
        .unwrap()
        .without_heatr()
        .reconstruct(1.0e-3)
        .unwrap();
    let once = r.clone().broaden(kelvins(293.6)).unwrap();
    let twice = r
        .clone()
        .broaden(kelvins(600.0))
        .unwrap()
        .broaden(kelvins(293.6))
        .unwrap();
    assert_same(
        once.cross_sections().unwrap(),
        twice.cross_sections().unwrap(),
        "#617",
    );
    assert_eq!(twice.temperature().get::<kelvin>(), 293.6);
    // And several temperatures from the one RECONR held.
    let temps = [kelvins(0.0), kelvins(293.6), kelvins(900.0)];
    let many = r.cross_sections_at_temperatures(&temps).unwrap();
    let tape = Tape::read_file(&p).unwrap();
    for (t, m) in temps.iter().zip(&many) {
        let free = process::cross_sections(&tape, *t, r.settings()).unwrap();
        assert_same(m, &free, &format!("F-19 at {} K", t.get::<kelvin>()));
    }
    // Without a held reconstruction, the object calls the one-RECONR function.
    let fresh = NuclearDataLibrary::from_file(&p).unwrap().without_heatr();
    assert_same(
        &fresh.cross_sections_at_temperatures(&temps[1..2]).unwrap()[0],
        &many[1],
        "fresh",
    );
}

#[test]
fn mat_za_and_awr_come_from_the_tape_and_two_materials_are_refused() {
    let Some(p) = path(F19) else { return };
    let lib = NuclearDataLibrary::from_bytes(&std::fs::read(&p).unwrap()).expect("from bytes");
    assert_eq!(lib.mat(), 925);
    assert_eq!(lib.za(), 9019.0);
    let tape = Tape::read_file(&p).unwrap();
    assert_eq!(
        lib.awr().to_bits(),
        tape.section(925, 1, 451).unwrap().rows[0][1].to_bits()
    );
    let Some(p2) = path("n-008_O_016-ENDF8.0.endf") else {
        return;
    };
    let mut secs = tape.sections().to_vec();
    secs.extend(Tape::read_file(&p2).unwrap().sections().iter().cloned());
    let two = Tape::from_sections(String::new(), secs);
    assert!(matches!(
        NuclearDataLibrary::from_tape(two),
        Err(NjoyError::InvalidInput(_))
    ));
}

#[test]
fn the_pendf_tape_reads_back_bit_for_bit() {
    let Some(p) = path(F19) else { return };
    let lib = NuclearDataLibrary::from_file(&p)
        .unwrap()
        .without_heatr()
        .reconstruct(1.0e-3)
        .unwrap()
        .broaden(kelvins(293.6))
        .unwrap();
    let mut text = Vec::new();
    lib.to_pendf_tape()
        .unwrap()
        .write(&mut text)
        .expect("MODER write");
    let back = Tape::read(text.as_slice()).expect("read back");
    let xs = lib.cross_sections().unwrap();
    assert_eq!(
        back.section(925, 1, 451).unwrap().rows.last().unwrap()[0],
        293.6,
        "hdatio temperature"
    );
    for s in &xs.pendf.sections {
        let rows = &back
            .section(925, 3, s.mt.number())
            .expect("MF=3 section")
            .rows;
        let np = rows[1][5] as usize;
        let pairs: Vec<(f64, f64)> = rows[3..]
            .iter()
            .flat_map(|r| [(r[0], r[1]), (r[2], r[3]), (r[4], r[5])])
            .take(np)
            .collect();
        assert_eq!(bits(&pairs), bits(&s.pairs), "MT {:?}", s.mt);
    }
    let dir = std::env::temp_dir().join(format!("ndl-pendf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    lib.write_pendf(dir.join("f19.pendf")).expect("write_pendf");
    assert_eq!(std::fs::read(dir.join("f19.pendf")).unwrap(), text);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn steps_out_of_order_are_refused_not_panicked_on() {
    let Some(p) = path(F19) else { return };
    let lib = NuclearDataLibrary::from_file(&p).unwrap();
    assert!(matches!(lib.to_ace(), Err(NjoyError::InvalidInput(_))));
    assert!(matches!(
        lib.to_continuous_energy_data(),
        Err(NjoyError::InvalidInput(_))
    ));
    assert!(matches!(
        lib.clone().broaden(kelvins(293.6)),
        Err(NjoyError::InvalidInput(_))
    ));
    assert_eq!(
        lib.total_xs(NeutronEnergy::new::<electronvolt>(1.0))
            .get::<barn>(),
        0.0
    );
}
