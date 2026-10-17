// SPDX-License-Identifier: GPL-3.0

//! **Bit-identity gate for replacing `CoreInventory` with outram-mc's
//! `RadionuclideInventory`** (GitHub issue #951, 2026-10-17).
//!
//! # Methodology
//!
//! The change swaps the TYPE the inventory travels in; it must not change one
//! bit of any number this crate produces. Each case below runs a fixed
//! calculation through the ordinary public API and folds every output `f64`
//! (`to_bits`) and every output name into a 64-bit FNV-1a fingerprint, which
//! is pinned. The pins were printed by THIS FILE on the commit before the
//! change (`SEMB_FP_PRINT=1 cargo test --release -p sembawang --test
//! inventory_handoff_fingerprints -- --nocapture`); then the type was changed
//! and only the lines that BUILD the inventory were edited (marked
//! `// inventory construction`). A fingerprint is a hash, so a mismatch says
//! only that something moved; rerun with `SEMB_FP_PRINT=1` to see which case.
//!
//! What is folded, per `AccidentRelease`: every window's start and end, every
//! released nuclide's label, decay constant and per-window activity, the
//! venting indices and fractions, the `screened_out` names, the
//! `cumulative_final` pairs, and the caveat flags. For
//! `htr10_air_ingress_bound`: every (nuclide, Bq) pair.
//!
//! The cases cover each way an inventory enters the release chain: the
//! shakedown unit inventory on a 2 x 3 mesh with real normal-operation pools
//! (`unit_2x3_pools`) and with the empty-pools ablation
//! (`unit_2x3_no_pools`); a ring-resolved, non-uniform inventory
//! (`rings_3x2_nonuniform`, distinct activity per ring and nuclide, so a ring
//! mix-up moves the hash); a full-flow-through vent (`unit_1x2_flow`); and the
//! published Liu & Cao Table 1 inventory through the library's own
//! `htr10_air_ingress_bound` (`htr10_air_ingress_bound_140h`), which builds its
//! inventory inside `lwr_comparison`.
//!
//! # Results
//!
//! Pre-change pins printed 2026-10-17 on `21a93d9460` (x86_64 Linux, glibc);
//! see the table under `PINS` for the post-change outcome.

use sembawang::accident::release::{
    accident_release, accident_release_with_venting, AccidentRelease, PlantParameters, Venting,
};
use sembawang::htr10::Htr10Geometry;
use sembawang::inventory::{CoreInventory, NuclideInventory};
use sembawang::lwr_comparison::htr10_air_ingress_bound;
use sembawang::scenario::TemperatureTransient;
use sembawang::units;
use uom::si::f64::{ThermodynamicTemperature, Time};
use uom::si::frequency::hertz;
use uom::si::radioactivity::becquerel;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::{hour, second};

const NUCLIDES: [&str; 5] = ["Kr-85", "Kr-88", "I-131", "Te-132", "Cs-137"];

/// FNV-1a over the bit patterns of `xs`.
fn fnv(state: &mut u64, xs: &[f64]) {
    for x in xs {
        fnv_bytes(state, &x.to_bits().to_le_bytes());
    }
}

fn fnv_bytes(state: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *state ^= u64::from(*b);
        *state = state.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn fold_release(out: &AccidentRelease) -> u64 {
    let mut h = FNV_OFFSET;
    for w in &out.source_term.windows {
        fnv(&mut h, &[w.start.get::<second>(), w.end.get::<second>()]);
    }
    for n in &out.source_term.nuclides {
        fnv_bytes(&mut h, n.label.as_bytes());
        fnv(&mut h, &[n.decay_constant.get::<hertz>()]);
        let a: Vec<f64> = n.released.iter().map(|r| r.get::<becquerel>()).collect();
        fnv(&mut h, &a);
    }
    for i in out.venting.indices() {
        fnv_bytes(&mut h, &i.to_le_bytes());
    }
    fnv(&mut h, out.venting.fractions());
    for s in &out.screened_out {
        fnv_bytes(&mut h, s.as_bytes());
    }
    for (name, bq) in &out.cumulative_final {
        fnv_bytes(&mut h, name.as_bytes());
        fnv(&mut h, &[*bq]);
    }
    let c = &out.caveats;
    fnv_bytes(&mut h, format!("{c:?}").as_bytes());
    h
}

fn c(x: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(x)
}

fn ramp(n_radial: usize, n_axial: usize) -> TemperatureTransient {
    TemperatureTransient::from_ramp(
        c(600.0),
        c(1600.0),
        Time::new::<second>(1.0e4),
        Time::new::<second>(1.0e5),
        51,
        n_radial,
        n_axial,
    )
    .unwrap()
}

fn plant() -> PlantParameters {
    PlantParameters::np_mhtgr_reference(1.0e-4, 1.0e-4, 0.0)
}

fn htr10_geometry() -> Htr10Geometry {
    let particle = tampines::pebble_bed::triso::TrisoParticle::htr10();
    let pebble = tampines::pebble_bed::pebble::Pebble::htr10();
    Htr10Geometry {
        kernel_radius: particle.kernel_radius,
        sic_thickness: particle.silicon_carbide_outer_radius - particle.inner_pyc_outer_radius,
        graphite_thickness: pebble.outer_radius - pebble.fuelled_zone_radius,
    }
}

fn cases() -> Vec<(&'static str, u64)> {
    let mut out = Vec::new();

    // inventory construction
    let unit_2x3 = CoreInventory::unit(&NUCLIDES, 2, 3);
    let r = accident_release(&unit_2x3, &ramp(2, 3), &plant()).unwrap();
    out.push(("unit_2x3_pools", fold_release(&r)));

    let r = accident_release(
        &unit_2x3,
        &ramp(2, 3),
        &plant().without_normal_operation_pools(),
    )
    .unwrap();
    out.push(("unit_2x3_no_pools", fold_release(&r)));

    // inventory construction
    let rings = CoreInventory::new(
        NUCLIDES
            .iter()
            .enumerate()
            .map(|(i, n)| NuclideInventory {
                name: (*n).to_string(),
                per_ring: (0..3)
                    .map(|r| units::from_curies(1.0e3 * (1.0 + i as f64) * (1.0 + 0.37 * r as f64)))
                    .collect(),
            })
            .collect(),
        3,
        2,
    );
    let r = accident_release(&rings, &ramp(3, 2), &plant()).unwrap();
    out.push(("rings_3x2_nonuniform", fold_release(&r)));

    // inventory construction
    let unit_1x2 = CoreInventory::unit(&NUCLIDES, 1, 2);
    let r = accident_release_with_venting(&unit_1x2, &ramp(1, 2), &plant(), &Venting::FullFlowThrough)
        .unwrap();
    out.push(("unit_1x2_flow", fold_release(&r)));

    let rel = htr10_air_ingress_bound(htr10_geometry(), Time::new::<hour>(140.0)).unwrap();
    let mut h = FNV_OFFSET;
    for (name, bq) in &rel {
        fnv_bytes(&mut h, name.as_bytes());
        fnv(&mut h, &[*bq]);
    }
    out.push(("htr10_air_ingress_bound_140h", h));

    out
}

/// Pinned on the pre-change commit.
///
/// | case | pre-change | post-change |
/// |---|---|---|
/// | `unit_2x3_pools` | `0xcf21dcf2a24fa6db` | pending |
/// | `unit_2x3_no_pools` | `0x33eae7072bbb9ca6` | pending |
/// | `rings_3x2_nonuniform` | `0x85d901d637b3e2d7` | pending |
/// | `unit_1x2_flow` | `0x5c5b31f88b4ba69f` | pending |
/// | `htr10_air_ingress_bound_140h` | `0x0e5be9eeeaccdfe1` | pending |
const PINS: &[(&str, u64)] = &[
    ("unit_2x3_pools", 0xcf21_dcf2_a24f_a6db),
    ("unit_2x3_no_pools", 0x33ea_e707_2bbb_9ca6),
    ("rings_3x2_nonuniform", 0x85d9_01d6_37b3_e2d7),
    ("unit_1x2_flow", 0x5c5b_31f8_8b4b_a69f),
    ("htr10_air_ingress_bound_140h", 0x0e5b_e9ee_eacc_dfe1),
];

#[test]
fn every_inventory_handoff_case_is_bit_identical() {
    let got = cases();
    if std::env::var_os("SEMB_FP_PRINT").is_some() {
        for (name, h) in &got {
            println!("(\"{name}\", 0x{h:016x}),");
        }
    }
    assert_eq!(got.len(), PINS.len(), "one pin per case");
    for ((name, h), (pin_name, pin)) in got.iter().zip(PINS) {
        assert_eq!(name, pin_name);
        assert_eq!(*h, *pin, "{name}: fingerprint moved (0x{h:016x} vs pinned 0x{pin:016x})");
    }
}
