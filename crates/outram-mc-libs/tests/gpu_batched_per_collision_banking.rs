//! gh:#1513: the GPU batched-flight driver ([`run_keff_gpu_batched`], host-side
//! collision `collide_batched` in `physics/keff.rs`) banks fission sites PER
//! COLLISION by default, as OpenMC `sample_neutron_reaction` ->
//! `create_fission_sites` (`src/physics.cpp:119-123, 172-189`, `8f79d9d0`) and
//! the CPU drivers do. The legacy bank-at-fission order stays reachable under
//! `OUTRAM_MC_LEGACY_FISSION_BANKING=1` (or the master
//! `OUTRAM_MC_LEGACY_REACTION_ORDER=1`) and reproduces the pre-change numbers.
//!
//! The change itself landed in `e83cea6a2` (2026-11-11). Its unit test
//! (`collide_batched_banks_fission_sites_at_every_collision`) pins the kernel;
//! this file pins the driver's k under both orders.
//!
//! # Methodology
//! Godiva (bare HEU sphere, R = 8.7407 cm, LOW-tier `from_core` U-234/235/238 at
//! the ICSBEP atom densities), `n_particles = 2000`, 10 inactive + 50 active
//! generations, seed 12345, through [`run_keff_gpu_batched`] (flight in WGSL on
//! the adapter `gpu::probe` returns; collision on the CPU in f64). Two arms in
//! one process, switched with `scatter::set_fission_banking_override` (one
//! `#[test]` so nothing else in the process sees the override):
//! - **default**: no override, i.e. the environment switch (per collision unless
//!   the legacy variable is set);
//! - **legacy**: `FissionBanking::AtFission`.
//!
//! **Old numbers.** `OLD_*` below were produced by THIS test run on develop
//! `87473ff8a` with `e83cea6a2` reverted (`git revert --no-commit e83cea6a2`, the
//! only change), where `collide_batched` always banks at fission. **New numbers**
//! (`NEW_*`) by the default arm on develop `87473ff8a` itself. Both were measured
//! again after rebasing onto develop `e1aa4acf9` (PETIR specfunc round), with
//! the same revert there: every word unchanged.
//!
//! # Gates (written before any run)
//! 1. Without the legacy variable, `fission_banking_from_env()` is
//!    `PerCollision` (the default is pinned, not opt-in).
//! 2. On Mesa llvmpipe (software Vulkan; the f32 flight is adapter-specific, so
//!    the bit pins are checked only there and printed elsewhere): the legacy arm
//!    reproduces `OLD_*` bit for bit (`k_mean`, `k_std`, a fold of every
//!    `k_gen`); the default arm gives `NEW_*` bit for bit, or `OLD_*` when the
//!    legacy variable is set (the environment switch reproduces the old run).
//! 3. Every adapter: |k_default - k_legacy| <= 3 sigma, sigma the two standard
//!    errors in quadrature (both orders are unbiased; over 3 sigma is a finding,
//!    not a re-pin).
//!
//! Prediction (posted on #1513 before the run): gate 2 legacy bit-identical to
//! the reverted code; default - legacy zero within statistics, sign not
//! predicted.
//!
//! # Results
//! 2026-11-13, rustc 1.99.1 release, Mesa llvmpipe (LLVM 20.1.2, 256 bits),
//! i.e. software Vulkan on an Intel Xeon @ 2.80 GHz, 4 logical cores shared with
//! other builds, 15 GiB RAM, Linux 6.18.44. Each arm reproduced run to run.
//! All gates PASSED; none was adjusted.
//!
//! | arm | k_mean +/- k_std (50 active gens) |
//! |---|---|
//! | old code (`e83cea6a2` reverted), both arms | 1.00974 +/- 0.00445 |
//! | legacy arm, current code | 1.00974 +/- 0.00445, bit for bit = old (all three words) |
//! | default (per collision), current code | 1.00987 +/- 0.00374 |
//!
//! Default - legacy: **+12 pcm, +0.02 sigma** (prediction: zero within
//! statistics; held). The per-run standard error is 16 % lower per collision,
//! the direction the CPU drivers showed (Jemima seed-to-seed sd 187 -> 144 pcm);
//! one seed does not resolve it. With `OUTRAM_MC_LEGACY_FISSION_BANKING=1` in
//! the environment and no override, the default arm reproduces OLD bit for bit.
//! With the master `OUTRAM_MC_LEGACY_REACTION_ORDER=1` both arms give
//! 1.01210 +/- 0.00418, identical to each other (every legacy order at once, so
//! not comparable with OLD).

use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{KeffResult, KeffSettings};
use outram_mc_libs::physics::scatter::{
    fission_banking_from_env, set_fission_banking_override, FissionBanking,
};

const RADIUS: f64 = 8.7407;

/// Pre-`e83cea6a2` run (bank at fission), llvmpipe: `(k_mean, k_std, k_gen fold)` bits.
const OLD: (u64, u64, u64) = (0x3ff0_27e8_6997_8cb6, 0x3f72_3b47_d73f_9ef1, 0x18d9_4163_fe3f_0eec);
/// Per-collision default on develop `87473ff8a` (and `e1aa4acf9`), llvmpipe.
const NEW: (u64, u64, u64) = (0x3ff0_286a_f67e_6426, 0x3f6e_9d25_32aa_4b55, 0x23a8_ef27_f6ac_2e0d);

fn godiva() -> (Material, Vec<Nuclide>) {
    let nuclides = vec![
        Nuclide::from_core("U234").unwrap(),
        Nuclide::from_core("U235").unwrap(),
        Nuclide::from_core("U238").unwrap(),
    ];
    let material = Material {
        id: 1,
        name: "Godiva".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
        ],
    };
    (material, nuclides)
}

fn settings() -> KeffSettings {
    KeffSettings { n_particles: 2000, n_inactive: 10, n_active: 50, seed: 12345, ..KeffSettings::default() }
}

/// The same reading of a legacy variable as `fission_banking_from_env`.
fn env_set(k: &str) -> bool {
    std::env::var(k).map(|v| !v.is_empty() && v != "0").unwrap_or(false)
}

/// `(k_mean, k_std, fold of every k_gen)` as bits; the fold is order-sensitive.
fn fingerprint(r: &KeffResult) -> (u64, u64, u64) {
    let fold = r
        .k_by_generation
        .iter()
        .fold(0xcbf2_9ce4_8422_2325u64, |h, k| (h ^ k.to_bits()).wrapping_mul(0x0100_0000_01b3));
    (r.k_mean.to_bits(), r.k_std.to_bits(), fold)
}

#[cfg(not(target_os = "android"))]
#[test]
fn batched_driver_banks_per_collision_by_default_and_legacy_reproduces_old_run() {
    use outram_mc_libs::physics::keff::run_keff_gpu_batched;

    // Gate 1.
    // The master switch also restores the legacy reaction order, scatter
    // kernels and fission-neutron order, so it cannot reproduce OLD (run with
    // today's other defaults); under it only gates 1 and 3 and the
    // "default arm = legacy arm" identity are checked.
    let master = env_set("OUTRAM_MC_LEGACY_REACTION_ORDER");
    let legacy = master || env_set("OUTRAM_MC_LEGACY_FISSION_BANKING");
    let expected_mode = if legacy { FissionBanking::AtFission } else { FissionBanking::PerCollision };
    assert_eq!(fission_banking_from_env(), expected_mode, "default banking mode");

    let Some(ctx) = outram_mc_libs::gpu::probe() else {
        eprintln!("SKIP batched_driver_banks_per_collision: no GPU adapter");
        return;
    };
    let adapter = ctx.info.name.clone();
    println!("adapter: {adapter}; legacy variable set: {legacy}");
    let (material, nuclides) = godiva();
    let s = settings();

    set_fission_banking_override(None);
    let default = run_keff_gpu_batched(&ctx, RADIUS, &material, &nuclides, &s);
    set_fission_banking_override(Some(FissionBanking::AtFission));
    let old_order = run_keff_gpu_batched(&ctx, RADIUS, &material, &nuclides, &s);
    set_fission_banking_override(None);

    let (fd, fl) = (fingerprint(&default), fingerprint(&old_order));
    println!(
        "default: k = {:.5} +/- {:.5}  bits ({:#018x}, {:#018x}, {:#018x})",
        default.k_mean, default.k_std, fd.0, fd.1, fd.2
    );
    println!(
        "legacy:  k = {:.5} +/- {:.5}  bits ({:#018x}, {:#018x}, {:#018x})",
        old_order.k_mean, old_order.k_std, fl.0, fl.1, fl.2
    );

    // Gate 3.
    let dk = default.k_mean - old_order.k_mean;
    let sigma = (default.k_std.powi(2) + old_order.k_std.powi(2)).sqrt();
    println!("default - legacy: {:+.0} pcm, {:+.2} sigma", dk * 1e5, dk / sigma);
    assert!(dk.abs() <= 3.0 * sigma, "default - legacy = {:+.2} sigma: a finding", dk / sigma);

    // Under a legacy variable the environment switch alone selects the old
    // order: the default arm IS the legacy arm, on any adapter.
    if legacy {
        assert_eq!(fd, fl, "legacy variable set: default arm must equal the legacy arm");
    }

    // Gate 2 (llvmpipe only).
    if master {
        eprintln!("master legacy switch set: bit pins (today's other defaults) not applicable");
    } else if adapter.contains("llvmpipe") {
        assert_eq!(fl, OLD, "legacy arm must reproduce the pre-e83cea6a2 run bit for bit");
        assert_eq!(fd, if legacy { OLD } else { NEW }, "default arm pin");
    } else {
        eprintln!("bit pins recorded on llvmpipe only; adapter {adapter}: statistical gate only");
    }
}
