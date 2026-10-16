// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **Cross-platform determinism harness, Part C (coolprop twin), stage S0 of
//! the HEM/KNP plan** (epic gh:#896, issue gh:#903; design in
//! `crates/tampines/docs/hem-knp-stabilisation/02-petir-coverage-audit.md` §4,
//! item 4). Parts A and B are in
//! `crates/tampines-steam-tables/tests/cross_platform_determinism.rs`, which
//! documents the method in full. This file applies the same method to
//! `outram-park-fork-coolprop`.
//!
//! # What it measures
//!
//! Each routine gets its own FNV-1a-64 hash over the `f64::to_bits()` of its
//! outputs, in a fixed order, so a mismatch names the routine.
//!
//! - **`state_pt` / `state_ph` sweeps for Water (IAPWS-95) and Nitrogen.** The
//!   grid is 24 × 24: `T` is linear over a single-phase-friendly range for
//!   each fluid, and `p` is log-spaced. At each point the harness hashes
//!   `state_pt`'s density, enthalpy, entropy, `c_p`, `c_v` and speed of sound.
//!   It then runs `state_ph` at the enthalpy it just got and hashes the
//!   recovered temperature and density, followed by `viscosity` and
//!   `conductivity`. A flash error is hashed as a fixed NaN, so the *set* of
//!   failing points is part of the fingerprint.
//! - **A 200-step `OPCPFluidArray` (`rhoPimpleFoam`) run.** It reuses the setup
//!   of the crate's `inlet_outlet_bcs_drive_gas_flow_and_outlet_pressure_settles`
//!   test: Nitrogen, 10 cells, 1 m, `dt = 2e-4 s`, PISO-2, an inlet velocity
//!   of 0.5 m/s and an outlet held at 1 bar. It runs once in
//!   `SolverMode::Pimple`, once in `SolverMode::HybridAllMach` at the default
//!   Mach-blend window, and once with the Mach window and density taper forced
//!   open (`n2arr_knp`).
//!   Each run hashes `p`, `he`, `rho` and `U` after steps 1, 10, 50 and 200.
//!   The default-window hybrid run is bit-identical to Pimple (Ma ≈ 1.4e-3),
//!   so only `n2arr_knp` exercises the KNP flux. The sweeps hash
//!   `state_pt` failures as NaN; there are 2 for Water and 0 for Nitrogen on
//!   this grid.
//!
//! # The goldens are PRE-MIGRATION and PLATFORM-SPECIFIC
//!
//! As for Parts A and B: `GOLDEN` holds **x86_64-unknown-linux-gnu, release,
//! 2026-10-16** hashes, before any coolprop call site moves to petir. They are
//! asserted only on x86_64 Linux. Every other target prints its hashes and
//! passes. Regenerate with `DETERMINISM_PRINT=1`, and record every legitimate
//! move (old, new, cause) in a `verification_and_validation/` note.
//! `DETERMINISM_DUMP=<sub-hash name>` prints every value of one routine.
//!
//! ```bash
//! cargo test --release -p outram-park-fork-coolprop --test cross_platform_determinism -- --nocapture
//! cargo test --release --target wasm32-wasip1 -p outram-park-fork-coolprop --test cross_platform_determinism --no-run
//! ```
//!
//! # Negative control
//!
//! The audit's prediction for Part C is that the sweeps **differ** between
//! linux-gnu and wasm, because coolprop calls raw `std` `exp`/`ln`/`powf`
//! (audit §2a). That prediction was written down, before the run, in the
//! steam-tables file.
//!
//! **Result (2026-10-16): the prediction MISSED. All 68 sub-hashes are
//! bit-identical** between x86_64-linux (glibc 2.39) and wasm32-wasip1
//! (Node 22 WASI). The steam-tables `std_libm` probe shows why: `exp`, `ln`
//! and `powf` agree on all 500 samples on this pair, and coolprop's hot path
//! is made of exactly those. Only `sin`, `cbrt` and `atan` differ. So on
//! linux-gnu vs wasm this file is a regression pin, **not yet a cross-platform
//! test**: it has no discriminating power here. Windows MSVC and macOS are the
//! discriminating targets, and neither could be run in this container. The
//! 1-ulp mutation test below shows that the hash itself does move.
//!
//! The wasm run was made from a nested git worktree, where Cargo concatenates
//! the two `.cargo/config.toml` `runner` arrays and the runner fails. It was
//! therefore built with `--no-run` and run with
//! `node --no-warnings scripts/wasi-run.mjs <wasm> --nocapture` from the crate
//! directory.

use outram_park_fork_coolprop::openfoam_algorithms::rhoPimpleFoam::SolverMode;
use outram_park_fork_coolprop::{conductivity, state_ph, state_pt, viscosity, Fluid, OPCPFluidArray};

use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    Area, AvailableEnergy, Length, MassDensity, Pressure, Ratio, ThermodynamicTemperature, Time, Velocity,
};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::ratio::ratio;
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// FNV-1a 64-bit over the little-endian bytes of each value's bit pattern.
struct Fnv {
    h: u64,
    n: usize,
    dump: bool,
}

impl Fnv {
    fn new(name: &str) -> Self {
        let dump = std::env::var("DETERMINISM_DUMP").map(|v| v == name).unwrap_or(false);
        Fnv { h: 0xcbf2_9ce4_8422_2325, n: 0, dump }
    }
    fn push(&mut self, x: f64) {
        if self.dump {
            println!("  [{:6}] {:#018x} {:e}", self.n, x.to_bits(), x);
        }
        for b in x.to_bits().to_le_bytes() {
            self.h ^= u64::from(b);
            self.h = self.h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        self.n += 1;
    }
}

/// Native golden hashes, `(sub-hash name, value count, hash)`.
/// x86_64-unknown-linux-gnu, release, 2026-10-16, PRE-migration.
const GOLDEN: &[(&str, usize, u64)] = &[
    // GOLDEN-TABLE-START
    ("water_rho", 576, 0xd6e7bdd49b514bfa),
    ("water_h", 576, 0x116b029de0d713fd),
    ("water_s", 576, 0x29cbd4815016e24f),
    ("water_cp", 576, 0xdee28b77521dd8d5),
    ("water_cv", 576, 0xfb82eadb6e71ac38),
    ("water_w", 576, 0x0077a62492027d9e),
    ("water_ph_T", 576, 0x750f30d3065e6b85),
    ("water_ph_rho", 576, 0x55a7d705c887ce88),
    ("water_mu", 576, 0x821605191e8f6344),
    ("water_lambda", 576, 0x1d03bb4cc196ac86),
    ("n2_rho", 576, 0xcadfff609240c746),
    ("n2_h", 576, 0xbfb6f6ff6e930f05),
    ("n2_s", 576, 0x8711f1af5c0733d1),
    ("n2_cp", 576, 0x096b5980e6508b43),
    ("n2_cv", 576, 0x539aa2ef45c5bd15),
    ("n2_w", 576, 0x86b82aac43ec6efb),
    ("n2_ph_T", 576, 0xadd27a211d530ff5),
    ("n2_ph_rho", 576, 0x605496e9059d6d4e),
    ("n2_mu", 576, 0x9d96a0d570808b0c),
    ("n2_lambda", 576, 0xb78d38d10a8db01c),
    ("n2arr_pimple_p_1", 10, 0xc593ad5453fadb2d),
    ("n2arr_pimple_he_1", 10, 0xe02fbb6bcb14e9b9),
    ("n2arr_pimple_rho_1", 10, 0x1e7a398d49c67b58),
    ("n2arr_pimple_U_1", 30, 0xcbd400ad968b13d8),
    ("n2arr_pimple_p_10", 10, 0x60cac2e4deec8a3b),
    ("n2arr_pimple_he_10", 10, 0xc3825424b8e38e5d),
    ("n2arr_pimple_rho_10", 10, 0xb2842afa7d9c8ab2),
    ("n2arr_pimple_U_10", 30, 0x53e674be6debf8f2),
    ("n2arr_pimple_p_50", 10, 0xe9fee336758b806d),
    ("n2arr_pimple_he_50", 10, 0x7c9c31f29a443528),
    ("n2arr_pimple_rho_50", 10, 0x8758c468709d5cb5),
    ("n2arr_pimple_U_50", 30, 0x34895150d19f50b4),
    ("n2arr_pimple_p_200", 10, 0x07ef4d9edb494bfb),
    ("n2arr_pimple_he_200", 10, 0x3fea5c7f55a1d2e8),
    ("n2arr_pimple_rho_200", 10, 0x526666a45dd9bc30),
    ("n2arr_pimple_U_200", 30, 0xa301458654117bb8),
    ("n2arr_hybrid_p_1", 10, 0xc593ad5453fadb2d),
    ("n2arr_hybrid_he_1", 10, 0xe02fbb6bcb14e9b9),
    ("n2arr_hybrid_rho_1", 10, 0x1e7a398d49c67b58),
    ("n2arr_hybrid_U_1", 30, 0xcbd400ad968b13d8),
    ("n2arr_hybrid_p_10", 10, 0x60cac2e4deec8a3b),
    ("n2arr_hybrid_he_10", 10, 0xc3825424b8e38e5d),
    ("n2arr_hybrid_rho_10", 10, 0xb2842afa7d9c8ab2),
    ("n2arr_hybrid_U_10", 30, 0x53e674be6debf8f2),
    ("n2arr_hybrid_p_50", 10, 0xe9fee336758b806d),
    ("n2arr_hybrid_he_50", 10, 0x7c9c31f29a443528),
    ("n2arr_hybrid_rho_50", 10, 0x8758c468709d5cb5),
    ("n2arr_hybrid_U_50", 30, 0x34895150d19f50b4),
    ("n2arr_hybrid_p_200", 10, 0x07ef4d9edb494bfb),
    ("n2arr_hybrid_he_200", 10, 0x3fea5c7f55a1d2e8),
    ("n2arr_hybrid_rho_200", 10, 0x526666a45dd9bc30),
    ("n2arr_hybrid_U_200", 30, 0xa301458654117bb8),
    ("n2arr_knp_p_1", 10, 0xc593ad5453fadb2d),
    ("n2arr_knp_he_1", 10, 0xedc6ffd9fe6f8041),
    ("n2arr_knp_rho_1", 10, 0x1e7a398d49c67b58),
    ("n2arr_knp_U_1", 30, 0xcbd400ad968b13d8),
    ("n2arr_knp_p_10", 10, 0x727d3ca54a16c5a1),
    ("n2arr_knp_he_10", 10, 0x40e9786afc99e94a),
    ("n2arr_knp_rho_10", 10, 0xc177b4812941dfda),
    ("n2arr_knp_U_10", 30, 0xbe4fd350682ea62f),
    ("n2arr_knp_p_50", 10, 0x8dfaec51ec7e289a),
    ("n2arr_knp_he_50", 10, 0x52ef9011b06545b7),
    ("n2arr_knp_rho_50", 10, 0x3632048f2eda4bcb),
    ("n2arr_knp_U_50", 30, 0xb2529da96d7527fe),
    ("n2arr_knp_p_200", 10, 0x259b78cbd489c6f0),
    ("n2arr_knp_he_200", 10, 0xf9ac1f94278ac285),
    ("n2arr_knp_rho_200", 10, 0x1f4c58815bb8b015),
    ("n2arr_knp_U_200", 30, 0xb3588abfa95d04f8),
    // GOLDEN-TABLE-END
];

fn report(results: &[(String, usize, u64)]) {
    let print = std::env::var("DETERMINISM_PRINT").is_ok();
    let native = cfg!(all(target_arch = "x86_64", target_os = "linux"));
    for (name, n, h) in results {
        if print || !native {
            println!("    (\"{name}\", {n}, {h:#018x}),");
        }
    }
    if !native {
        println!("(not x86_64-linux: hashes printed, not asserted -- see module docs)");
        return;
    }
    let mut mismatches = Vec::new();
    for (name, n, h) in results {
        match GOLDEN.iter().find(|(g, _, _)| g == name) {
            Some((_, gn, gh)) if gn == n && gh == h => {}
            Some((_, gn, gh)) => {
                mismatches.push(format!("{name}: golden ({gn}, {gh:#018x}) vs now ({n}, {h:#018x})"))
            }
            None => mismatches.push(format!("{name}: no golden recorded")),
        }
    }
    assert!(
        mismatches.is_empty(),
        "determinism goldens moved on x86_64-linux:\n  {}\nIf the move is intended, \
         regenerate with DETERMINISM_PRINT=1 and record the cause.",
        mismatches.join("\n  ")
    );
}

// ─────────────────────────────────────────────────────────────────────────────
//  Property sweeps
// ─────────────────────────────────────────────────────────────────────────────

const NT: usize = 24;
const NPR: usize = 24;

/// `(t_lo, t_hi, p_lo, p_hi)` per fluid, in K and Pa.
fn ranges(fluid: Fluid) -> (f64, f64, f64, f64) {
    match fluid {
        Fluid::Water => (280.0, 1000.0, 1.0e4, 5.0e7),
        _ => (80.0, 1000.0, 1.0e4, 5.0e7),
    }
}

fn fluid_sweep(fluid: Fluid, tag: &str) -> Vec<(String, usize, u64)> {
    let names = ["rho", "h", "s", "cp", "cv", "w", "ph_T", "ph_rho", "mu", "lambda"];
    let mut hs: Vec<Fnv> = names.iter().map(|n| Fnv::new(&format!("{tag}_{n}"))).collect();
    let (t_lo, t_hi, p_lo, p_hi) = ranges(fluid);
    let (lp0, lp1) = (p_lo.ln(), p_hi.ln());
    for i in 0..NT {
        let t = t_lo + (t_hi - t_lo) * i as f64 / (NT - 1) as f64;
        for j in 0..NPR {
            let p = (lp0 + (lp1 - lp0) * j as f64 / (NPR - 1) as f64).exp();
            match state_pt(fluid, t, p) {
                Ok(s) => {
                    for (k, v) in [s.density, s.enthalpy, s.entropy, s.cp, s.cv, s.speed_of_sound]
                        .into_iter()
                        .enumerate()
                    {
                        hs[k].push(v);
                    }
                    match state_ph(fluid, p, s.enthalpy) {
                        Ok(b) => {
                            hs[6].push(b.temperature);
                            hs[7].push(b.density);
                        }
                        Err(_) => {
                            hs[6].push(f64::NAN);
                            hs[7].push(f64::NAN);
                        }
                    }
                    hs[8].push(viscosity(fluid, t, s.density).unwrap_or(f64::NAN));
                    hs[9].push(conductivity(fluid, t, s.density).unwrap_or(f64::NAN));
                }
                Err(_) => {
                    for h in hs.iter_mut() {
                        h.push(f64::NAN);
                    }
                }
            }
        }
    }
    names.iter().zip(hs).map(|(n, h)| (format!("{tag}_{n}"), h.n, h.h)).collect()
}

#[test]
fn part_c_water_and_nitrogen_property_sweeps() {
    let mut r = fluid_sweep(Fluid::Water, "water");
    r.extend(fluid_sweep(Fluid::Nitrogen, "n2"));
    report(&r);
}

// ─────────────────────────────────────────────────────────────────────────────
//  200-step rhoPimpleFoam array
// ─────────────────────────────────────────────────────────────────────────────

fn array_slice(mode: SolverMode, blend: Option<(f64, f64)>, tag: &str) -> Vec<(String, usize, u64)> {
    let n = 10;
    let mut arr = OPCPFluidArray::new(
        Fluid::Nitrogen,
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(1.0e-4),
        n as i64,
        Time::new::<second>(2.0e-4),
    )
    .expect("valid 1-D geometry");
    arr.set_piso_algorithm(2);
    arr.set_solver_mode(mode);
    if let Some((lo, hi)) = blend {
        arr.set_mach_blend_window(Ratio::new::<ratio>(lo), Ratio::new::<ratio>(hi));
        // The default density taper (50/100 kg/m^3, steam-calibrated) zeroes
        // the hybrid in a ~1 kg/m^3 gas; open it too, or nothing changes.
        arr.set_rho_taper_window(
            MassDensity::new::<kilogram_per_cubic_meter>(0.0),
            MassDensity::new::<kilogram_per_cubic_meter>(1.0e-3),
        );
    }
    let p_out = Pressure::new::<pascal>(1.0e5);
    for c in 0..n {
        arr.p.internal[c] = p_out.get::<pascal>();
    }
    arr.set_temperature_vector(vec![ThermodynamicTemperature::new::<kelvin>(300.0); n]).unwrap();
    let inlet = state_pt(Fluid::Nitrogen, 300.0, 1.0e5).expect("N2 at 300 K, 1 bar is single phase");
    arr.set_inlet_velocity(Velocity::new::<meter_per_second>(0.5));
    arr.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(inlet.enthalpy));
    arr.set_outlet_pressure(p_out);

    let mut out = Vec::new();
    for step in 1..=200 {
        arr.step();
        if [1, 10, 50, 200].contains(&step) {
            let mut hp = Fnv::new(&format!("{tag}_p_{step}"));
            let mut hh = Fnv::new(&format!("{tag}_he_{step}"));
            let mut hr = Fnv::new(&format!("{tag}_rho_{step}"));
            let mut hu = Fnv::new(&format!("{tag}_U_{step}"));
            for c in 0..n {
                hp.push(arr.p.internal[c]);
                hh.push(arr.he.internal[c]);
                hr.push(arr.rho.internal[c]);
                let u = arr.u.internal[c];
                hu.push(u.x);
                hu.push(u.y);
                hu.push(u.z);
            }
            for (k, h) in [("p", hp), ("he", hh), ("rho", hr), ("U", hu)] {
                out.push((format!("{tag}_{k}_{step}"), h.n, h.h));
            }
        }
    }
    out
}

#[test]
fn part_c_rho_pimple_200_steps_pimple() {
    report(&array_slice(SolverMode::Pimple, None, "n2arr_pimple"));
}

#[test]
fn part_c_rho_pimple_200_steps_hybrid_knp() {
    report(&array_slice(SolverMode::HybridAllMach, None, "n2arr_hybrid"));
}

/// At the default Mach-blend window `n2arr_hybrid` is **bit-identical to
/// `n2arr_pimple`** in all 16 sub-hashes (measured 2026-10-16). At 0.5 m/s in
/// nitrogen, Ma is about 1.4e-3, far below the window, so the KNP dissipation
/// never acts. The default density taper (50/100 kg/m^3) would zero it even
/// at sonic speed in a ~1 kg/m^3 gas. This run opens the Mach window to
/// `(0, 1e-3)` and the taper to `(0, 1e-3)` kg/m^3, so that the KNP path is
/// actually exercised. It is a code-path exerciser, not a physics
/// configuration.
#[test]
fn part_c_rho_pimple_200_steps_knp_forced() {
    report(&array_slice(SolverMode::HybridAllMach, Some((0.0, 1.0e-3)), "n2arr_knp"));
}

/// Mutation half of the negative control: a 1-ulp input change moves the hash.
#[test]
fn one_ulp_input_perturbation_moves_the_hash() {
    let a = state_pt(Fluid::Water, 500.0, 1.0e7).unwrap().density;
    let b = state_pt(Fluid::Water, f64::from_bits(500.0_f64.to_bits() + 1), 1.0e7).unwrap().density;
    assert_ne!(a.to_bits(), b.to_bits(), "1-ulp T did not move rho");
    let (mut ha, mut hb) = (Fnv::new(""), Fnv::new(""));
    ha.push(a);
    hb.push(b);
    assert_ne!(ha.h, hb.h);
}
