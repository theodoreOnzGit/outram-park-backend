//! **`waste` against OpenMC's `waste.py`, code to code** (gh:#1391).
//!
//! # Methodology
//!
//! `verification_and_validation/waste_1391/openmc_inputs/waste_reference.py`
//! builds 45 activated-material cases (three recipes, scales 1e-6..1e8 of the
//! activation products) in OpenMC `8f79d9d02483c90e4134f03a9df4b75ce6b062bf`
//! and records, for `metal` false and true: `Material.waste_classification`,
//! `Material.waste_disposal_rating` for all five named limit sets, and the
//! per-nuclide ratios; with the inputs OpenMC computed them from
//! (`get_activity(units='Ci/m3', by_nuclide=True)`, `get_mass_density()`).
//!
//! 1. **The port of `waste.py`**: [`outram_mc_libs::waste`] is fed OpenMC's own
//!    activities and mass density and the ENDF/B-VIII.0 half-lives
//!    (`half_life.json`, committed beside the script), and must return the same
//!    class, the same per-nuclide ratios and the same ratio sums. **Pass
//!    criterion: bit identity** (the arithmetic is the same operations in the
//!    same order).
//! 2. **The inventory adapter**: the same cases rebuilt as a
//!    [`RadionuclideInventory`] (activity `lambda * atoms` in Bq, `lambda =
//!    ln 2 / T_half`) and converted by `WasteInventory::from_inventory`; the
//!    Ci/m^3 must agree with OpenMC's `get_activity` to 1e-12 relative. They are
//!    not the same operation order (OpenMC multiplies `lambda * 1e24 * n` by
//!    `1e6 / 3.7e10`; the inventory divides a stored Bq by the volume), so
//!    round-off is expected; the worst is printed.
//!
//! # Results
//!
//! See `verification_and_validation/waste_1391/README.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use outram_mc_libs::inventory::{Amount, InventoryPoint, InventorySource, RadionuclideInventory, Regions};
use outram_mc_libs::model::Isotope;
use outram_mc_libs::waste::{
    waste_classification, waste_disposal_rating, waste_disposal_rating_by_nuclide, HalfLifeTable, WasteClass,
    WasteInventory, WasteLimits,
};
use uom::si::amount_of_substance::particle;
use uom::si::f64::{AmountOfSubstance, MassDensity, Radioactivity, Volume};
use uom::si::mass_density::gram_per_cubic_centimeter;
use uom::si::radioactivity::becquerel;
use uom::si::volume::cubic_centimeter;

fn vv() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/waste_1391")
}

fn load() -> HashMap<String, Vec<String>> {
    let text = std::fs::read_to_string(vv().join("openmc_outputs/waste.txt")).unwrap();
    let mut m = HashMap::new();
    for line in text.lines() {
        let mut it = line.split(' ');
        let tag = it.next().unwrap().to_string();
        m.insert(tag, it.map(str::to_string).collect::<Vec<_>>());
    }
    m
}

fn floats(r: &HashMap<String, Vec<String>>, tag: &str) -> Vec<f64> {
    r[tag].iter().map(|s| s.parse().unwrap()).collect()
}

fn labels() -> Vec<String> {
    let mut out = Vec::new();
    for recipe in ["steel", "fp_tru", "inert"] {
        for k in -6..=8 {
            out.push(format!("{recipe}_k{k}"));
        }
    }
    out
}

fn limit_of(name: &str) -> WasteLimits {
    match name {
        "Fetter" => WasteLimits::Fetter,
        "NRC_long" => WasteLimits::NrcLong,
        "NRC_short_A" => WasteLimits::NrcShortA,
        "NRC_short_B" => WasteLimits::NrcShortB,
        "NRC_short_C" => WasteLimits::NrcShortC,
        other => panic!("{other}"),
    }
}

fn class_of(s: &str) -> WasteClass {
    match s {
        "Class_A" => WasteClass::ClassA,
        "Class_B" => WasteClass::ClassB,
        "Class_C" => WasteClass::ClassC,
        "GTCC" => WasteClass::Gtcc,
        other => panic!("{other}"),
    }
}

fn half_lives() -> HalfLifeTable {
    HalfLifeTable::from_openmc_json(&std::fs::read_to_string(vv().join("openmc_inputs/half_life.json")).unwrap()).unwrap()
}

#[test]
fn waste_port_is_bit_identical_to_openmc() {
    let r = load();
    let hl = half_lives();
    assert!(hl.len() > 3000, "half_life.json has {} entries", hl.len());
    let mut n_ratio = 0;
    let mut n_class = 0;
    let mut classes: Vec<WasteClass> = Vec::new();
    for label in labels() {
        let names = &r[&format!("{label}.names")];
        let act = floats(&r, &format!("{label}.activity"));
        let rho = floats(&r, &format!("{label}.massdensity"))[0];
        let inv = WasteInventory::new(names.iter().cloned().zip(act).collect(), rho).unwrap();
        for metal in [false, true] {
            let want = class_of(&r[&format!("{label}.class.{}", cap(metal))][0]);
            assert_eq!(waste_classification(&inv, metal, &hl), want, "{label} metal={metal}");
            n_class += 1;
            classes.push(want);
            for lim in ["Fetter", "NRC_long", "NRC_short_A", "NRC_short_B", "NRC_short_C"] {
                let limits = limit_of(lim);
                let tag = format!("{label}.rating.{lim}.{}", cap(metal));
                let sum = waste_disposal_rating(&inv, &limits, metal, &hl);
                let want = floats(&r, &tag)[0];
                assert_eq!(sum.to_bits(), want.to_bits(), "{tag}: {sum:e} vs {want:e}");
                let by = waste_disposal_rating_by_nuclide(&inv, &limits, metal, &hl);
                let want_by = &r[&format!("{label}.bynuc.{lim}.{}", cap(metal))];
                let want_by: Vec<(&str, f64)> = want_by
                    .iter()
                    .filter(|s| *s != "-")
                    .map(|s| {
                        let (n, v) = s.split_once('=').unwrap();
                        (n, v.parse().unwrap())
                    })
                    .collect();
                assert_eq!(by.len(), want_by.len(), "{tag}: nuclides");
                for ((n, v), (wn, wv)) in by.iter().zip(&want_by) {
                    assert_eq!(n, wn, "{tag}");
                    assert_eq!(v.to_bits(), wv.to_bits(), "{tag} {n}: {v:e} vs {wv:e}");
                    n_ratio += 1;
                }
            }
        }
    }
    println!("waste port: {n_class} classifications and {n_ratio} per-nuclide ratios bit-identical to OpenMC");
    // the ladder reaches every class, so every branch of the rules is compared
    for c in [WasteClass::ClassA, WasteClass::ClassB, WasteClass::ClassC, WasteClass::Gtcc] {
        assert!(classes.contains(&c), "{c:?} never reached");
    }
}

fn cap(b: bool) -> &'static str {
    if b { "True" } else { "False" }
}

#[test]
fn inventory_adapter_matches_get_activity() {
    let r = load();
    let hl = half_lives();
    let v_cm3 = 1.0e6; // 1 m3
    let mut worst = 0.0_f64;
    let mut exact = 0;
    let mut total = 0;
    for label in labels() {
        let names = &r[&format!("{label}.names")];
        let atoms_per_bcm = floats(&r, &format!("{label}.atoms"));
        let want = floats(&r, &format!("{label}.activity"));
        let rho = floats(&r, &format!("{label}.massdensity"))[0];
        let mut entries: Vec<(Isotope, Vec<Amount>)> = Vec::new();
        for (name, n) in names.iter().zip(&atoms_per_bcm) {
            let lambda = hl.half_life(name).map_or(0.0, |t| std::f64::consts::LN_2 / t);
            let atoms = n * 1e24 * v_cm3;
            entries.push((
                Isotope::from_name(name).unwrap(),
                vec![Amount::from_atoms_and_activity(
                    AmountOfSubstance::new::<particle>(atoms),
                    Radioactivity::new::<becquerel>(lambda * atoms),
                )],
            ));
        }
        let inv = RadionuclideInventory::new(
            Regions::WholeCore,
            entries,
            InventoryPoint::unspecified(),
            InventorySource::Prescribed { reference: "waste_vs_openmc_1391".into() },
        )
        .unwrap();
        let w = WasteInventory::from_inventory(
            &inv,
            0,
            Volume::new::<cubic_centimeter>(v_cm3),
            MassDensity::new::<gram_per_cubic_centimeter>(rho),
        )
        .unwrap();
        for ((n, got), (wn, theirs)) in w.activities_ci_per_m3().iter().zip(names.iter().zip(&want)) {
            assert_eq!(n, wn);
            total += 1;
            if got.to_bits() == theirs.to_bits() {
                exact += 1;
            } else {
                let rel = if *theirs == 0.0 { got.abs() } else { ((got - theirs) / theirs).abs() };
                worst = worst.max(rel);
                assert!(rel <= 1e-12, "{label} {n}: {got:e} vs {theirs:e}");
            }
        }
    }
    println!("inventory adapter: {exact}/{total} Ci/m3 values bit-identical, worst relative difference {worst:e}");
}
