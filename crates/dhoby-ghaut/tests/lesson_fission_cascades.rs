//! Reaches every function of the "A fragment's story" chapter's generator
//! (`examples/lesson_fission_cascades/{physics,boonlay}.rs`) and checks each
//! against something it can fail on: an exact identity, a hand calculation
//! from the stated formula, or boon-lay's own data. The generated page data
//! are checked separately by
//! `docs/site/science/fission-products/check-fission.js`.
//!
//! Tests needing the U-235 tape or the fission-yields-data source directory
//! skip, with a note, when it is absent.
//!
//! Results 2026-10-20: all 6 pass (`cargo test --release
//! --ignore-rust-version -p dhoby-ghaut --test lesson_fission_cascades`):
//! the U-235 MT=458 line reads EFR 1.6913e8 eV and ENP 4.827645e6 eV; Sr-95's
//! main path ends on stable Mo-95; Pu-239's alpha lands on U-235; the
//! clamps found from `diffusion_coefficient`'s output are Cs kernel 700 C,
//! Sr graphite 800 C, Ag graphite 480 C (the grid point below 490 C).
#![cfg(not(any(target_arch = "wasm32", target_os = "android")))]

#[path = "../examples/lesson_fission_cascades/boonlay.rs"]
mod boonlay;
#[path = "../examples/lesson_fission_cascades/physics.rs"]
mod physics;

use boon_lay::prelude::decay_library::DecayLibrary;
use boon_lay::Nuclide;
use boonlay::*;
use physics::*;

#[test]
fn rounding_and_q_intervals() {
    // 3 significant figures: -75100 keeps hundreds, 8070 keeps tens.
    assert_eq!(rounding_half_unit_kev(-75100.0), 50.0);
    assert_eq!(rounding_half_unit_kev(8070.0), 5.0);
    assert_eq!(rounding_half_unit_kev(0.0), 0.0);
    let q = q_from_excesses(&[(1.0, -75100.0), (-1.0, -78300.0)]);
    assert_eq!(q, (3200.0, 3100.0, 3300.0));
}

#[test]
fn kinematics_hand_cases() {
    // An electron with T = m_e c^2 has pc = sqrt(3) m_e c^2.
    assert!((pc_kev(ME_KEV, ME_KEV) - 3f64.sqrt() * ME_KEV).abs() < 1e-9);
    // p^2/2M.
    assert_eq!(recoil_from_pc_kev(2.0, 4.0), 0.5);
    assert_eq!(gamma_recoil_kev(2.0, 4.0), 0.5);
    // Beta recoil: Q = 0 gives nothing; it grows with Q.
    assert_eq!(beta_recoil_max_kev(0.0, 1e8), 0.0);
    assert!(beta_recoil_max_kev(2000.0, 1e8) > beta_recoil_max_kev(1000.0, 1e8));
    // Equal masses share equally.
    assert_eq!(two_body_recoil_kev(10.0, 1.0, 1.0), 5.0);
    // Momentum split: E_L/E_H = A_H/A_L, sum EFR.
    let (l, h) = fragment_energies_ev(234.0, 95.0, 139.0);
    assert!((l + h - 234.0).abs() < 1e-12 && (l / h - 139.0 / 95.0).abs() < 1e-12);
    // Speed: T = (gamma - 1) m, so T = m gives beta = sqrt(3)/2.
    assert!((speed_m_s(1.0, 1.0) / C_M_S - 3f64.sqrt() / 2.0).abs() < 1e-12);
    // A kick: angle dp/p, energy v dp.
    let (a, de) = in_flight_kick(1.0, 1.0, 0.01);
    assert!((a - 0.01 / 3f64.sqrt()).abs() < 1e-15);
    assert!((de - 0.01 * 3f64.sqrt() / 2.0).abs() < 1e-15);
    assert_eq!(atomic_mass_kev(2, 1.0), 2.0 * U_KEV + 1.0);
}

#[test]
fn endf_text_readers() {
    assert_eq!(endf_float(" 1.691300+8"), Some(1.6913e8));
    assert_eq!(endf_float("-2.500000-3"), Some(-2.5e-3));
    assert_eq!(endf_float(" 2.414000+0"), Some(2.414));
    assert_eq!(endf_float("           "), Some(0.0));
    assert_eq!(endf_float("abc"), None);
    assert_eq!(pretty_name("Sr95"), "Sr-95");
    assert_eq!(pretty_name("Ag110m"), "Ag-110m");
    assert_eq!(mode_label("BetaMinus"), "beta-");
    assert_eq!(mode_label("BetaMinusAndNeutron"), "beta-,n");
    assert_eq!(mode_label("Alpha"), "alpha");
    assert_eq!(mode_label("Whatever"), "other");
    let tape = concat!(env!("CARGO_MANIFEST_DIR"), "/../../reference-data/endf/n-092_U_235-ENDF8.0.endf");
    let Ok(text) = std::fs::read_to_string(tape) else {
        eprintln!("lesson_fission_cascades: U-235 tape absent, MT=458 reader not checked");
        return;
    };
    // The tape's own line (boon-lay's rustbca_cascades test pins the same one).
    assert_eq!(mt458_efr_enp(&text, 9228), Some((1.6913e8, 4.827645e6)));
    assert_eq!(mt458_efr_enp(&text, 1), None);
}

#[test]
fn nudat_masses_read() {
    let csv = "name,z,n,massExcess(keV)\nNeutron,0,1,8070\nX,1,1,5\nXm,1,1,9\nbad\n";
    let m = mass_excess_table(csv);
    assert_eq!(m.get(&(0, 1)), Some(&8070.0));
    assert_eq!(m.get(&(1, 2)), Some(&5.0), "the ground state is the lower row");
    let Ok(src) = fission_yields_data_src() else {
        eprintln!("lesson_fission_cascades: fission-yields-data source absent, NuDat table not checked");
        return;
    };
    let m = mass_excess_table(&std::fs::read_to_string(src.join("nndc_nudat_z_n_and_mass_excess.csv")).unwrap());
    // C-12 defines the unit: its excess is zero.
    assert_eq!(m.get(&(6, 12)), Some(&0.0));
}

#[test]
fn decay_chains_from_boon_lay() {
    let mut lib = DecayLibrary::new();
    let path = main_path(&lib, Nuclide::Sr95, 12);
    assert!(!path.is_empty());
    // Each step's daughter is the next step's parent; A is kept by beta-.
    for w in path.windows(2) {
        assert_eq!(w[0].daughter, w[1].parent);
    }
    for s in &path {
        assert!(s.half_life_s > 0.0 && s.branching > 0.0 && s.branching <= 1.0);
        if s.mode == "BetaMinus" {
            assert_eq!((s.daughter_z, s.daughter_a), (s.z + 1, s.a));
        }
    }
    // The chain ends on a nuclide the library has as stable.
    let last = path.last().unwrap();
    let end = Nuclide::Mo95;
    assert_eq!(last.daughter, format!("{end:?}"));
    assert_eq!(half_life_s(&lib, end), None);
    assert!(half_life_s(&lib, Nuclide::Sr95).is_some());
    assert!(main_path(&lib, end, 5).is_empty());
    // Alpha branch of Pu-239 lands on U-235.
    let (br, d) = branch_of(&lib, Nuclide::Pu239, "Alpha").expect("Pu-239 alpha");
    assert!(br > 0.0);
    assert_eq!(d, Nuclide::U235);
    assert!(branch_of(&lib, Nuclide::Pu239, "BetaMinusAndNeutron").is_none());
    // Histories: reproducible with the seed, times increasing, the last stable.
    let h1 = sample_histories(&mut lib, Nuclide::Sr95, 3, 7);
    let h2 = sample_histories(&mut lib, Nuclide::Sr95, 3, 7);
    assert_eq!(h1, h2);
    for h in &h1 {
        assert_eq!(h[0].0, "Sr95");
        for w in h.windows(2) {
            assert!(w[1].1 >= w[0].1);
        }
        assert!(h.last().unwrap().1.is_infinite());
    }
}

#[test]
fn diffusion_series_from_boon_lay() {
    assert_eq!(flat_up_to(&[1.0, 2.0, 3.0, 4.0], &[5.0, 5.0, 5.0, 6.0]), Some(3.0));
    assert_eq!(flat_up_to(&[1.0, 2.0], &[5.0, 6.0]), None);
    let t: Vec<f64> = (0..=80).map(|i| 400.0 + 20.0 * i as f64).collect();
    let s = diffusion_series(&t);
    assert_eq!(s.len(), 11);
    let get = |l: &str| s.iter().find(|x| x.label == l).unwrap();
    // The module doc's clamps, found from the function's output alone: Cs
    // kernel >= 700 C, Sr graphite >= 800 C, Ag graphite >= 490 C (the grid
    // point at or below it), Xe and the SiC term never flat.
    assert_eq!(get("Cs in kernel").flat_up_to_c, Some(700.0));
    assert_eq!(get("Sr in graphite").flat_up_to_c, Some(800.0));
    assert_eq!(get("Ag in graphite").flat_up_to_c, Some(480.0));
    assert_eq!(get("Xe in kernel").flat_up_to_c, None);
    assert_eq!(get("Ag in SiC").flat_up_to_c, None);
    // Arrhenius: D rises with T where not clamped.
    let x = get("Ag in SiC");
    assert!(x.d.windows(2).all(|w| w[1] > w[0]));
}
