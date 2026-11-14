// SPDX-License-Identifier: AGPL-3.0-only

//! GitHub #205: the CLS / SCLS point-query state moved out of a global
//! `Mutex` into per-run state the driver owns. On the single-thread backend
//! the query order is unchanged, so every generation's `k` must be
//! **bit-identical** to what the Mutex version produced.
//!
//! Methodology: the FHR reference pebble on LOW embedded data (the model of
//! `dh_universe::tests::power_iteration_steps_keff_bit_for_bit`), 150
//! histories x [2 inactive + 3 active], default seed, one fresh universe per
//! run, treatments CLS and SCLS. `PINNED` holds the per-generation `k` bits
//! printed by the commit BEFORE the change (`5fdb7fd3b`), on the host that
//! printed them (x86-64 Linux, release).
//!
//! Second part: the multi-threaded backend, where the old lock consumed the
//! sampler stream in thread-arrival order (non-reproducible CLS, the live
//! defect of #205). Two runs on different thread counts must now agree bit
//! for bit.

use outram_mc_libs::dh_universe::{DhTreatment, DhUniverse, PebbleParams};
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::compute::{ComputeType, ThreadCount};
use outram_mc_libs::physics::keff::KeffSettings;

fn setup() -> (Vec<Nuclide>, Vec<Material>) {
    let nuclides: Vec<Nuclide> = ["U235", "U238", "O16", "C0", "Si28"]
        .iter()
        .map(|n| Nuclide::from_core(n).unwrap())
        .collect();
    let mat = |id: i32, comps: &[(usize, f64)]| Material {
        id,
        name: format!("m{id}"),
        temperature: 293.6,
        components: comps
            .iter()
            .map(|&(nuclide_idx, atom_density)| NuclideComponent { nuclide_idx, atom_density })
            .collect(),
    };
    let materials = vec![
        mat(0, &[(0, 4.40e-3), (1, 1.77e-2), (2, 2.27e-2), (3, 9.10e-3)]),
        mat(1, &[(3, 5.02e-2)]),
        mat(2, &[(3, 9.53e-2)]),
        mat(3, &[(4, 4.79e-2), (3, 4.79e-2)]),
        mat(4, &[(3, 9.53e-2)]),
        mat(5, &[(3, 8.53e-2)]),
        mat(6, &[(3, 8.78e-2)]),
    ];
    (nuclides, materials)
}

fn settings(compute: ComputeType) -> KeffSettings {
    KeffSettings { n_particles: 150, n_inactive: 2, n_active: 3, compute, ..KeffSettings::default() }
}

fn k_bits(t: DhTreatment, compute: ComputeType) -> Vec<u64> {
    let (nuclides, materials) = setup();
    let u = DhUniverse::pebble(PebbleParams::fhr_reference().with_materials(materials), t).unwrap();
    u.keff(&nuclides, &settings(compute)).k_by_generation.iter().map(|k| k.to_bits()).collect()
}

/// Printed by commit `5fdb7fd3b` (the Mutex version); see the module doc.
const PINNED_CLS: &[u64] = &[
    4608484763905907234,
    4608224209937744055,
    4608091186029888642,
    4607319171005153610,
    4607742448590865707,
];
const PINNED_SCLS: &[u64] = &[
    4607954734115426933,
    4607990919318881655,
    4608201754557579738,
    4608141470626228026,
    4607883598742223999,
];

#[test]
fn single_thread_cls_and_scls_are_bit_identical_to_the_mutex_version() {
    let cls = k_bits(DhTreatment::ChordLength, ComputeType::CpuSingleThread);
    let scls = k_bits(DhTreatment::Scls, ComputeType::CpuSingleThread);
    assert_eq!(cls, PINNED_CLS, "CLS k moved");
    assert_eq!(scls, PINNED_SCLS, "SCLS k moved");
}

/// The live defect of #205: multi-threaded CLS consumed one sampler stream in
/// lock-arrival order, so two runs differed (measured on 2026-09-14: +3028 pcm
/// between two identical runs). With per-history state seeded from
/// (generation, history) the thread count cannot matter.
#[test]
fn multi_thread_cls_is_reproducible_across_thread_counts() {
    let one = k_bits(DhTreatment::ChordLength, ComputeType::CpuMultiThread(ThreadCount::Fixed(1)));
    let three = k_bits(DhTreatment::ChordLength, ComputeType::CpuMultiThread(ThreadCount::Fixed(3)));
    let again = k_bits(DhTreatment::ChordLength, ComputeType::CpuMultiThread(ThreadCount::Fixed(3)));
    assert_eq!(one, three, "CLS k depends on the thread count");
    assert_eq!(three, again, "CLS k differs between identical runs");
}

/// Same for SCLS: its retained inclusions are per-history state in the
/// parallel drivers.
#[test]
fn multi_thread_scls_is_reproducible_across_thread_counts() {
    let one = k_bits(DhTreatment::Scls, ComputeType::CpuMultiThread(ThreadCount::Fixed(1)));
    let three = k_bits(DhTreatment::Scls, ComputeType::CpuMultiThread(ThreadCount::Fixed(3)));
    assert_eq!(one, three, "SCLS k depends on the thread count");
}
