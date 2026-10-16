// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **Cross-platform determinism harness, stage S0 of the HEM/KNP plan**
//! (epic gh:#896, issue gh:#903; design in
//! `crates/tampines/docs/hem-knp-stabilisation/02-petir-coverage-audit.md` §4).
//!
//! # What it measures
//!
//! A bit-level fingerprint of the crate's hot-path outputs. Each routine gets
//! its own FNV-1a-64 hash over the `f64::to_bits()` of its outputs, in a fixed
//! order. If two targets disagree, the sub-hash names the routine that
//! differs.
//!
//! - **Part A, IF97 property sweep** (milliseconds). The grid is 48 × 48: `p`
//!   is log-spaced from 611.657 Pa (the triple point) to 100 MPa, and `h` is linear from 0 to
//!   4200 kJ/kg. Points are kept only if they pass the crate's own `(p,h)`
//!   flash-domain test, which is mirrored here from the private
//!   `rho_h_flash_eqm::ph_is_within_validity_range`:
//!   `p_sat(273.15 K) <= p <= 100 MPa` and
//!   `h_tp_1(273.15 K, p) <= h <= h(1073.15 K, p)`. At each point the harness
//!   hashes `t_ph_eqm`, `v_ph_eqm`, `s_ph_eqm`, `x_ph_flash`,
//!   `kappa_t_ph_eqm`, `mu_ph_eqm` and `lambda_ph_eqm`, plus `p_rho_h_eqm` on
//!   the round-tripped density. `hem_sound` hashes a **mirror** of
//!   `hem_sound_speed_ph` (the KNP flux's sound speed). The function itself is
//!   `pub(crate)` inside a private module of `openfoam_algorithms`, so it
//!   cannot be called from here, and that tree is not edited by this stage.
//!   The mirror composes the same public routines in the same order:
//!   `ph_flash_region`, then Kieffer's region-4 speed with its
//!   finite-difference fallback, or `w_tp_{1,2,3,5}` at `t_ph_eqm`.
//!   ~~The function proper is reached by Part B's `edw_knp` run.~~
//!   **Updated 2026-10-16:** `hem_sound_speed_ph` was deleted with the KNP
//!   hybrid; the mirror now matches the public
//!   `ph_flash_eqm::w_ph_eqm_hem` (without its absent clamp), and the
//!   `edw_knp` run is gone. The mirror is kept unchanged so the `hem_sound`
//!   golden does not move. `lambda_ph` is
//!   hashed as NaN at the 17 grid points where `t_ph_eqm < 273.15 K`, because
//!   `lambda_ph_eqm` panics there (gh:#904; see `part_a`). There are also three
//!   probes:
//!   - `powi_const`: `f64::powi` with literal arguments, which the compiler
//!     may constant-fold;
//!   - `powi_runtime`: the same exponents through `std::hint::black_box`;
//!   - `std_libm`: raw `std` `exp`/`ln`/`powf`/`sin`/`cbrt`, as a canary for
//!     the platform libm.
//! - **Part B, a short Edwards slice** (seconds). It uses
//!   `tests/edwards_blowdown.rs`'s geometry, the Hendrie initial condition,
//!   the choked break boundary condition, 4/4 PISO and the pinned `Linear`
//!   energy scheme. The run is 24 cells, `dt = 30 us` and 200 steps,
//!   ~~three times~~ **once since 2026-10-16**:
//!   - `edw_pimple`: ~~`SolverMode::Pimple`~~ the array's only flux
//!     discretisation, PIMPLE.
//!
//!   **RETIRED 2026-10-16** with the KNP hybrid (gh:#914, decision D7; record
//!   in `verification_and_validation/determinism_golden_moves.md`). The two
//!   runs below and their 32 goldens were removed; the `edw_pimple` goldens
//!   did not move, which is the check that only the hybrid was removed:
//!   - ~~`edw_hybrid`~~: `SolverMode::HybridAllMach` at the default Mach-blend
//!     window. ~~This reaches the KNP flux and `hem_sound_speed_ph`.~~
//!     **Measured 2026-10-16: bit-identical to `edw_pimple` in all 16
//!     sub-hashes.** No face reaches the blend window within 200 steps, so the
//!     KNP dissipation never acts here. It is kept because it pins exactly
//!     that: the hybrid mode is a no-op in this regime.
//!   - ~~`edw_knp`~~: `HybridAllMach` with the window opened to `(0, 1e-3)`, so
//!     the KNP flux and `hem_sound_speed_ph` act on every moving face. It
//!     departs from `edw_pimple` in `U` from step 1, and in every field from
//!     step 10. This is a code-path exerciser, not a physics configuration.
//!
//!   Each run hashes `p`, `he`, `rho` and `U` after steps 1, 10, 50 and 200,
//!   so a divergence can be dated.
//!
//! # The goldens are PLATFORM-SPECIFIC (~~PRE-MIGRATION~~ post-S2 since 2026-10-16)
//!
//! `GOLDEN` holds the hashes measured on **x86_64-unknown-linux-gnu, release,
//! 2026-10-16**. ~~That is before stage S2 moves the IF97 `powi` sites onto
//! `petir`.~~ **UPDATED 2026-10-16:** they were regenerated after stage S2
//! moved the IF97 hot path onto `petir` (see "Golden history"). The test asserts them **only on x86_64 Linux**. On any other
//! target it prints its hashes and passes. ~~because cross-target equality is
//! **not yet expected**. The negative control below shows the native and wasm
//! hashes differ today.~~ After S2, native and wasm32-wasip1 agree on every
//! library sub-hash; only the deliberate raw-`std` canary `std_libm` differs,
//! so a cross-target gate must exempt it. Wiring that gate into CI (Windows
//! MSVC and macOS included) is still out of scope here (see below).
//!
//! **Golden history.** 2026-10-16: all Part B (`edw_*`) goldens were
//! regenerated after gh:#900 changed `TampinesSteamArray`'s defaults to
//! implicit energy convection, `PsiRefresh::OncePerOuterCorrector` and the K
//! terms. The step-1 `he` hashes are unchanged; every `p`/`rho`/`U` hash moves
//! from step 1. That is the expected physics change, not a determinism
//! regression. Part A (IF97 sweep) did not move. The record is in
//! `verification_and_validation/determinism_golden_moves.md`.
//!
//! 2026-10-16, **stage S2 done**: the IF97 hot path is now on petir
//! (`r_powi`, `r_atan`, `r_ln`, `r_exp`), gated by
//! `tests/no_platform_libm_on_hot_path.rs`. ~~The goldens are pre-migration.~~
//! They are now **post-S2**: `lambda_ph` (1 value, 1 ulp) and `p_rho_h` (134
//! values, ≤ 5e-13 relative) moved; nothing else did, Part B included. A
//! `petir_libm` positive-control probe was added beside `std_libm`. The
//! native-vs-wasm re-run after the migration is under "Negative control"
//! below.
//!
//! When a golden moves for a legitimate reason, regenerate it with
//! `DETERMINISM_PRINT=1` and record the move (old hash, new hash, cause) in a
//! `verification_and_validation/` note. **Never update a golden to make a
//! failure go away without naming the cause.**
//!
//! # Running it
//!
//! ```bash
//! cargo test --release -p tampines-steam-tables --test cross_platform_determinism -- --nocapture
//! DETERMINISM_PRINT=1 cargo test --release ... -- --nocapture   # print every sub-hash
//! DETERMINISM_DUMP=t_ph cargo test --release ... -- --nocapture # every value of one routine
//! cargo test --release --target wasm32-wasip1 -p tampines-steam-tables --test cross_platform_determinism
//! ```
//!
//! # Negative control (2026-10-16): the harness CAN fail
//!
//! **Prediction, written before the wasm run.** The `std_libm` probe differs
//! between native and wasm, because wasm32-wasip1's `std` maths come from
//! wasi-libc, not glibc. Every IF97 routine that already goes through petir
//! `r_exp`/`r_ln`/`r_powf` agrees. The `powi_*` probes and the
//! `powi`-heavy routines *possibly* differ, if the backend lowers `powi`
//! differently. Added before the run, for the other parts: the Edwards
//! slices differ if and only if a Part A routine they call differs.
//! Part C, coolprop, differs in every sweep and array run, because coolprop
//! calls raw `std` `exp`/`ln`/`powf` (audit §2a).
//!
//! **Result (2026-10-16, x86_64-linux glibc 2.39 vs wasm32-wasip1 under
//! Node 22 WASI, both release).** `NEGATIVE_CONTROL` below is the per-sub-hash
//! authority. **57 of 60 steam-tables sub-hashes agree; all 68 coolprop
//! (Part C) sub-hashes agree.**
//!
//! - **`std_libm` differs, as predicted, but only through `sin` and `cbrt`.**
//!   Split by function (500 samples each), `exp`, `ln` and `powf` agree on all
//!   500; `sin` differs on 7 and `cbrt` on 16.
//! - **`lambda_ph` and `p_rho_h` differ**, each in exactly one of 2164
//!   values, by 1 ulp (grid indices 1835 and 116). Neither went through petir:
//!   `lambda_ph_eqm` reaches std `atan` in the critical-enhancement term
//!   (`thermal_conductivity/mod.rs`, the "1 atan" site of S2), and
//!   `p_rho_h_eqm` brackets with std `ln`/`exp`
//!   (`rho_h_flash_eqm/mod.rs`). The `exp`/`ln` agreement above suggests the
//!   `atan` is the cause of the first; the second is not traced further.
//! - **The `powi` probes and every `powi`-heavy routine agree.** The
//!   "possibly differ" half of the prediction did not happen on this pair.
//!   Constant folding and run-time `__powidf2` gave the same bits here.
//!   Windows MSVC is where `powi` was seen to differ (`petir::real::powi`).
//! - **The Edwards slices agree in all 48 sub-hashes**, consistent with "differ
//!   iff a Part A routine they call differs".
//! - **Part C missed its prediction.** Coolprop was predicted to differ and
//!   does not. Its hot path is `exp`/`ln`/`powf`, which agree here. That is
//!   consistent with glibc (≥ 2.28) and musl (whose maths wasi-libc uses) both
//!   having adopted ARM optimized-routines' `exp`/`log`/`pow`. That lineage is
//!   recalled, not re-checked against either library's source.
//!
//! **What this means for the harness's power.** On linux-gnu vs wasm32-wasip1,
//! the harness **can** fail, and does so through trig, `cbrt` and `atan`.
//! It has **no discriminating power for `exp`/`ln`/`pow`** on this pair, so
//! Part C is currently a regression pin and not yet a cross-platform test.
//! The discriminating targets for that path are Windows MSVC and macOS. Both
//! are named in the CI plan below, and neither can be run in this container.
//!
//! **Re-run after S2 (2026-10-16, same pair, same runner).** Prediction,
//! written before the run: `lambda_ph` and `p_rho_h` now agree, `std_libm`
//! still differs (it is raw `std` in this file by design), so 59 of 60.
//! **Result: 60 of 61 agree** (59 of the original 60, plus the new
//! `petir_libm`, which agrees). `lambda_ph` and `p_rho_h` agree; the only
//! differing sub-hash is `std_libm`. `NEGATIVE_CONTROL` below is kept as the
//! pre-migration record.
//!
//! Running it: the wasm run was made from a git worktree nested inside the
//! main checkout. There Cargo merges both `.cargo/config.toml` files and
//! concatenates their `runner` arrays, so the runner failed ("ENOENT: open
//! 'bash'"). That is an artefact of the nested worktree; a normal checkout
//! does not hit it. The workaround: build with `--no-run`, then run the
//! printed `.wasm` with `node --no-warnings scripts/wasi-run.mjs <wasm>
//! --nocapture` from the crate directory.
//!
//! # CI wiring (described, not done here)
//!
//! Run this file on `ubuntu-latest`, `windows-latest`, `macos-latest` (Apple
//! Silicon, so real aarch64) and as a `wasm32-wasip1` job under the existing
//! Node runner. Each job would print its hashes and upload them as an
//! artifact, plus a comparison step that diffs them. Make that a hard gate
//! only once S2/S3 claim cross-target equality.

use std::sync::OnceLock;

use petir::mathf::RealMath;

use tampines_steam_tables::interfaces::functional_programming::ph_flash_eqm::{
    kappa_t_ph_eqm, lambda_ph_eqm, ph_flash_region, s_ph_eqm, t_ph_eqm, v_ph_eqm, x_ph_flash,
};
use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::{
    h_tp_eqm_single_phase, FwdEqnRegion,
};
use tampines_steam_tables::region_2_vapour::w_tp_2;
use tampines_steam_tables::region_3_single_phase_plus_supercritical_steam::w_tp_3;
use tampines_steam_tables::region_4_vap_liq_equilibrium::{
    w_ps_eqm_region4_finite_diff_vol, w_ps_eqm_region4_kieffer,
};
use tampines_steam_tables::region_5_steam_at_800_plus_degc::w_tp_5;
use tampines_steam_tables::interfaces::functional_programming::rho_h_flash_eqm::{
    p_rho_h_eqm, rho_h_is_within_validity_range,
};
use tampines_steam_tables::dynamic_viscosity::mu_ph_eqm;
use tampines_steam_tables::region_1_subcooled_liquid::{h_tp_1, w_tp_1};
use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_pressure_4;
use tampines_steam_tables::steam_turbine_equations::converging_diverging_nozzles::choked_flow::get_critical_pressure_and_mass_flux_multiphase_ph;
use tampines_steam_tables::{EnergyConvectionScheme, TampinesSteamArray};

use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{
    Area, AvailableEnergy, Length, MassDensity, Pressure, Ratio, ThermodynamicTemperature, Time,
    Velocity,
};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// ~~Mirror of `openfoam_algorithms::rhoPimpleFoam::central_upwind::hem_sound_speed_ph`
/// (`pub(crate)`, unreachable from an integration test)~~ **Updated
/// 2026-10-16:** that function was deleted with the KNP hybrid. This is now a
/// mirror of the public `ph_flash_eqm::w_ph_eqm_hem`, kept as written so the
/// `hem_sound` golden does not move. No `c_min` clamp, so a non-finite raw
/// value shows in the hash rather than being masked. Same routines, same
/// order.
fn hem_sound_speed_mirror(p: Pressure, h: AvailableEnergy) -> f64 {
    match ph_flash_region(p, h) {
        FwdEqnRegion::Region4 => {
            let s = s_ph_eqm(p, h);
            let c = w_ps_eqm_region4_kieffer(p, s).get::<meter_per_second>();
            if c.is_finite() && c > 0.0 {
                c
            } else {
                w_ps_eqm_region4_finite_diff_vol(p, s).get::<meter_per_second>()
            }
        }
        FwdEqnRegion::Region1 => w_tp_1(t_ph_eqm(p, h), p).get::<meter_per_second>(),
        FwdEqnRegion::Region2 => w_tp_2(t_ph_eqm(p, h), p).get::<meter_per_second>(),
        FwdEqnRegion::Region3 => w_tp_3(t_ph_eqm(p, h), p).get::<meter_per_second>(),
        FwdEqnRegion::Region5 => w_tp_5(t_ph_eqm(p, h), p).get::<meter_per_second>(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Hashing
// ─────────────────────────────────────────────────────────────────────────────

/// FNV-1a 64-bit over the little-endian bytes of each value's bit pattern.
/// The byte order is fixed explicitly so the hash is target-independent.
#[derive(Clone)]
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
/// x86_64-unknown-linux-gnu, release, 2026-10-16, ~~PRE-migration~~ post-S2
/// (`lambda_ph`, `p_rho_h` regenerated, `petir_libm` added; see the module docs).
const GOLDEN: &[(&str, usize, u64)] = &[
    // GOLDEN-TABLE-START
    ("t_ph", 2164, 0xfe7442848ca7f258),
    ("v_ph", 2164, 0x124ef7e0b20ca901),
    ("s_ph", 2164, 0x662fc8dd7d47cd0a),
    ("x_ph", 2164, 0x7525347c80e73231),
    ("kappa_t_ph", 2164, 0x30e98ac17f829393),
    ("mu_ph", 2164, 0xf94a3d0371d786d7),
    ("lambda_ph", 2164, 0xf2fb2a32e2b829c9),
    ("hem_sound", 2164, 0x2703fa131e5f029b),
    ("p_rho_h", 2164, 0x8a482250694e65de),
    ("powi_const", 7, 0x29117476631d81ac),
    ("powi_runtime", 2007, 0x2eecdc58aa489499),
    ("std_libm", 2500, 0xc001414e982af1cb),
    ("petir_libm", 2500, 0x6dfb4db67eae5e27),
    ("edw_pimple_p_1", 24, 0x5c438839a9588fe5),
    ("edw_pimple_he_1", 24, 0x99e80f2836e95484),
    ("edw_pimple_rho_1", 24, 0xbf05587125d7c7a4),
    ("edw_pimple_U_1", 72, 0x60673c681d1e7655),
    ("edw_pimple_p_10", 24, 0x2423d833a3f87c70),
    ("edw_pimple_he_10", 24, 0x2745388edcf1211a),
    ("edw_pimple_rho_10", 24, 0x5f784bec19fe3451),
    ("edw_pimple_U_10", 72, 0xb409cf81ca9ef7ee),
    ("edw_pimple_p_50", 24, 0x77a391f98be19cae),
    ("edw_pimple_he_50", 24, 0x55a763f07d6ce4b7),
    ("edw_pimple_rho_50", 24, 0x928380f00731e1bf),
    ("edw_pimple_U_50", 72, 0x341470de4b9f6b88),
    ("edw_pimple_p_200", 24, 0x3d1ebba98509e7e5),
    ("edw_pimple_he_200", 24, 0x2efa2c88b9d4c55c),
    ("edw_pimple_rho_200", 24, 0x4f9fe4a58a074f24),
    ("edw_pimple_U_200", 72, 0xddc1daac10ff6f72),
    // GOLDEN-TABLE-END
];

/// Negative control: which sub-hashes differed between x86_64-linux (glibc
/// 2.39) and wasm32-wasip1 (Node 22 WASI), both built 2026-10-16 from this
/// tree.
/// `true` = differed. Documentation only; nothing asserts it. Kept as the
/// 2026-10-16 record: its `edw_hybrid_*` and `edw_knp_*` rows name runs that
/// were removed later that day with the KNP hybrid.
#[allow(dead_code)]
const NEGATIVE_CONTROL: &[(&str, bool)] = &[
    // NEGATIVE-CONTROL-START
    ("t_ph", false),
    ("v_ph", false),
    ("s_ph", false),
    ("x_ph", false),
    ("kappa_t_ph", false),
    ("mu_ph", false),
    ("lambda_ph", true),
    ("hem_sound", false),
    ("p_rho_h", true),
    ("powi_const", false),
    ("powi_runtime", false),
    ("std_libm", true),
    ("edw_pimple_p_1", false),
    ("edw_pimple_he_1", false),
    ("edw_pimple_rho_1", false),
    ("edw_pimple_U_1", false),
    ("edw_pimple_p_10", false),
    ("edw_pimple_he_10", false),
    ("edw_pimple_rho_10", false),
    ("edw_pimple_U_10", false),
    ("edw_pimple_p_50", false),
    ("edw_pimple_he_50", false),
    ("edw_pimple_rho_50", false),
    ("edw_pimple_U_50", false),
    ("edw_pimple_p_200", false),
    ("edw_pimple_he_200", false),
    ("edw_pimple_rho_200", false),
    ("edw_pimple_U_200", false),
    ("edw_hybrid_p_1", false),
    ("edw_hybrid_he_1", false),
    ("edw_hybrid_rho_1", false),
    ("edw_hybrid_U_1", false),
    ("edw_hybrid_p_10", false),
    ("edw_hybrid_he_10", false),
    ("edw_hybrid_rho_10", false),
    ("edw_hybrid_U_10", false),
    ("edw_hybrid_p_50", false),
    ("edw_hybrid_he_50", false),
    ("edw_hybrid_rho_50", false),
    ("edw_hybrid_U_50", false),
    ("edw_hybrid_p_200", false),
    ("edw_hybrid_he_200", false),
    ("edw_hybrid_rho_200", false),
    ("edw_hybrid_U_200", false),
    ("edw_knp_p_1", false),
    ("edw_knp_he_1", false),
    ("edw_knp_rho_1", false),
    ("edw_knp_U_1", false),
    ("edw_knp_p_10", false),
    ("edw_knp_he_10", false),
    ("edw_knp_rho_10", false),
    ("edw_knp_U_10", false),
    ("edw_knp_p_50", false),
    ("edw_knp_he_50", false),
    ("edw_knp_rho_50", false),
    ("edw_knp_U_50", false),
    ("edw_knp_p_200", false),
    ("edw_knp_he_200", false),
    ("edw_knp_rho_200", false),
    ("edw_knp_U_200", false),
    // NEGATIVE-CONTROL-END
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
//  Part A: IF97 property sweep
// ─────────────────────────────────────────────────────────────────────────────

const NP: usize = 48;

/// Lowest isobar of the grid: the triple-point pressure, as the audit asks.
const P_GRID_LO_PA: f64 = 611.657;
const NH: usize = 48;

/// The `(p,h)` grid, filtered by the crate's own flash-domain test (mirrored).
fn grid() -> &'static Vec<(f64, f64)> {
    static G: OnceLock<Vec<(f64, f64)>> = OnceLock::new();
    G.get_or_init(|| {
        let p_lo = sat_pressure_4(ThermodynamicTemperature::new::<kelvin>(273.15)).get::<pascal>();
        let (lp0, lp1) = (P_GRID_LO_PA.ln(), 100.0e6_f64.ln());
        let mut pts = Vec::new();
        for i in 0..NP {
            let p_pa = (lp0 + (lp1 - lp0) * i as f64 / (NP - 1) as f64).exp();
            if p_pa < p_lo || p_pa > 100.0e6 {
                continue;
            }
            let p = Pressure::new::<pascal>(p_pa);
            let h_lo = h_tp_1(ThermodynamicTemperature::new::<kelvin>(273.15), p).get::<joule_per_kilogram>();
            let h_hi = h_tp_eqm_single_phase(ThermodynamicTemperature::new::<kelvin>(1073.15), p)
                .get::<joule_per_kilogram>();
            for j in 0..NH {
                let h = 4200.0e3 * j as f64 / (NH - 1) as f64;
                if h >= h_lo && h <= h_hi {
                    pts.push((p_pa, h));
                }
            }
        }
        pts
    })
}

fn sweep(name: &str, f: impl Fn(Pressure, AvailableEnergy) -> f64) -> (String, usize, u64) {
    let mut h = Fnv::new(name);
    for &(p, e) in grid() {
        h.push(f(Pressure::new::<pascal>(p), AvailableEnergy::new::<joule_per_kilogram>(e)));
    }
    (name.to_string(), h.n, h.h)
}

fn part_a() -> Vec<(String, usize, u64)> {
    let mut out = vec![
        sweep("t_ph", |p, h| t_ph_eqm(p, h).get::<kelvin>()),
        sweep("v_ph", |p, h| v_ph_eqm(p, h).get::<cubic_meter_per_kilogram>()),
        sweep("s_ph", |p, h| s_ph_eqm(p, h).get::<joule_per_kilogram_kelvin>()),
        sweep("x_ph", x_ph_flash),
        sweep("kappa_t_ph", |p, h| kappa_t_ph_eqm(p, h).value), // SI base: 1/Pa
        sweep("mu_ph", |p, h| mu_ph_eqm(p, h).get::<pascal_second>()),
        // `lambda_ph_eqm` PANICS ("t,p flashing at eqm out of bounds!", via
        // `lambda_2_crit_enhancement_term_tp_two_phase_estimate` ->
        // `v_tp_eqm_two_phase`) at points inside the crate's own (p,h)
        // validity range, wherever the two-phase flash puts T a hair below
        // 273.15 K. Measured 2026-10-16: (T, p) = (273.1384 K, 611.657 Pa) and
        // (273.1383 K, 1000 Pa). That is a library defect, filed as gh:#904.
        // A panic cannot be caught on wasm32-wasip1 (panic = abort), so such
        // points are hashed as a fixed NaN. The rule is stated in advance, not
        // fitted: lambda is skipped wherever t_ph_eqm < 273.15 K.
        sweep("lambda_ph", |p, h| {
            if t_ph_eqm(p, h).get::<kelvin>() < 273.15 {
                f64::NAN
            } else {
                lambda_ph_eqm(p, h).get::<watt_per_meter_kelvin>()
            }
        }),
        sweep("hem_sound", hem_sound_speed_mirror),
        sweep("p_rho_h", |p, h| {
            let rho = MassDensity::new::<kilogram_per_cubic_meter>(
                1.0 / v_ph_eqm(p, h).get::<cubic_meter_per_kilogram>(),
            );
            if rho_h_is_within_validity_range(rho, h) {
                p_rho_h_eqm(rho, h).get::<pascal>()
            } else {
                f64::NAN // out of the (rho,h) domain: hashed as a fixed NaN
            }
        }),
    ];

    // Constant-argument powi: candidates for compile-time folding.
    let mut c = Fnv::new("powi_const");
    for x in [
        10f64.powi(-23),
        10f64.powi(-7),
        10f64.powi(23),
        1.1f64.powi(31),
        0.7f64.powi(-19),
        3.0f64.powi(13),
        1.000_001f64.powi(1001),
    ] {
        c.push(x);
    }
    out.push(("powi_const".into(), c.n, c.h));

    // The same, through black_box: forced to run-time lowering.
    let mut r = Fnv::new("powi_runtime");
    for (b, e) in [(10.0, -23), (10.0, -7), (10.0, 23), (1.1, 31), (0.7, -19), (3.0, 13), (1.000_001, 1001)] {
        r.push(std::hint::black_box(b as f64).powi(std::hint::black_box(e)));
    }
    // ...and over a runtime sweep, the IF97 pattern `pi.powi(ii)`.
    for k in 1..=200 {
        let x = std::hint::black_box(0.05 * k as f64);
        for e in [-41, -9, -2, 1, 2, 3, 7, 12, 24, 32] {
            r.push(x.powi(std::hint::black_box(e)));
        }
    }
    out.push(("powi_runtime".into(), r.n, r.h));

    // Platform-libm canary: what an unmigrated call site gets.
    let mut s = Fnv::new("std_libm");
    for k in 1..=500 {
        let x = std::hint::black_box(0.0371 * k as f64);
        s.push(x.exp());
        s.push(x.ln());
        s.push(x.powf(1.37));
        s.push(x.sin());
        s.push(x.cbrt());
    }
    out.push(("std_libm".into(), s.n, s.h));

    // Positive control (added 2026-10-16, S2): the same samples through petir
    // `RealMath`, the route S2 moved the library onto. `std_libm` above stays
    // raw on purpose: it is the canary that shows the platform still differs.
    let mut q = Fnv::new("petir_libm");
    for k in 1..=500 {
        let x = std::hint::black_box(0.0371 * k as f64);
        q.push(x.r_exp());
        q.push(x.r_ln());
        q.push(x.r_powf(1.37));
        q.push(x.r_sin());
        q.push(x.r_cbrt());
    }
    out.push(("petir_libm".into(), q.n, q.h));
    out
}

#[test]
fn part_a_if97_property_sweep() {
    let results = part_a();
    assert!(grid().len() > 1000, "grid too sparse: {}", grid().len());
    report(&results);
}

// ─────────────────────────────────────────────────────────────────────────────
//  Part B: 200-step Edwards slice
// ─────────────────────────────────────────────────────────────────────────────

// Edwards–O'Brien geometry and IC, from tests/edwards_blowdown.rs (B-T-3271).
const PIPE_LENGTH_M: f64 = 4.096;
const PIPE_ID_M: f64 = 0.073;
const PIPE_AREA_M2: f64 = std::f64::consts::PI * PIPE_ID_M * PIPE_ID_M / 4.0;
const BREAK_AREA_FRACTION: f64 = 0.87;
const DISC_OPEN_TIME_S: f64 = 1.0e-3;
const P_INIT_PA: f64 = 7.0e6;
const P_AMBIENT_PA: f64 = 1.0e5;
const FT_TO_M: f64 = 0.3048;
const NODE_CENTRE_FT: [f64; 24] = [
    0.260, 0.780, 1.300, 1.820, 2.385, 3.000, 3.615, 4.215, 4.820, 5.425, 6.025, 6.640, 7.255,
    7.820, 8.340, 8.940, 9.630, 10.320, 10.920, 11.440, 11.905, 12.370, 12.890, 13.295,
];
const NODE_T_DEGF: [f64; 24] = [
    447.5, 448.4, 449.4, 450.3, 451.4, 452.5, 451.7, 450.8, 450.0, 449.2, 448.3, 447.5, 448.0,
    448.5, 448.9, 449.4, 450.0, 446.5, 443.4, 440.8, 438.4, 436.0, 437.0, 437.8,
];

fn degf_to_k(f: f64) -> f64 {
    (f - 32.0) * 5.0 / 9.0 + 273.15
}

/// As `tests/edwards_blowdown.rs::lerp`.
fn lerp(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[xs.len() - 1] {
        return ys[ys.len() - 1];
    }
    for i in 0..xs.len() - 1 {
        if x >= xs[i] && x <= xs[i + 1] {
            let w = (x - xs[i]) / (xs[i + 1] - xs[i]);
            return ys[i] * (1.0 - w) + ys[i + 1] * w;
        }
    }
    ys[ys.len() - 1]
}

/// As `tests/edwards_blowdown.rs::choked_break`, returning only the face velocity.
fn choked_face_velocity(p0_pa: f64, h0_jkg: f64, rho0: f64, t: f64) -> f64 {
    if p0_pa <= P_AMBIENT_PA * 1.001 {
        return 0.0;
    }
    let (_p_throat, g) = get_critical_pressure_and_mass_flux_multiphase_ph(
        Pressure::new::<pascal>(p0_pa),
        AvailableEnergy::new::<joule_per_kilogram>(h0_jkg),
    );
    let g = g.get::<kilogram_per_square_meter_second>().max(0.0);
    let a_break = (t / DISC_OPEN_TIME_S).clamp(0.0, 1.0) * BREAK_AREA_FRACTION * PIPE_AREA_M2;
    g * a_break / (rho0.max(1e-4) * PIPE_AREA_M2)
}

fn edwards_slice(tag: &str) -> Vec<(String, usize, u64)> {
    let dt_s = 30.0e-6;
    let mut array = TampinesSteamArray::new(
        Length::new::<meter>(PIPE_LENGTH_M),
        Area::new::<square_meter>(PIPE_AREA_M2),
        24,
        Time::new::<second>(dt_s),
    )
    .expect("valid Edwards pipe geometry");
    array.set_he_convection_scheme(EnergyConvectionScheme::Linear);
    array.set_pimple_algorithm(4, 4, Ratio::new::<ratio>(1.0), Ratio::new::<ratio>(1.0));
    let n = array.mesh.n_cells;
    for c in 0..n {
        array.p.internal[c] = P_INIT_PA;
    }
    let centres_m: Vec<f64> = NODE_CENTRE_FT.iter().map(|&ft| ft * FT_TO_M).collect();
    let temps_k: Vec<f64> = NODE_T_DEGF.iter().map(|&f| degf_to_k(f)).collect();
    let temps: Vec<ThermodynamicTemperature> = (0..n)
        .map(|c| ThermodynamicTemperature::new::<kelvin>(lerp(&centres_m, &temps_k, array.mesh.cell_centres[c].x)))
        .collect();
    array.set_temperature_vector(temps).unwrap();
    array.set_inlet_velocity(Velocity::new::<meter_per_second>(0.0));

    let mut out = Vec::new();
    let mut time_s = 0.0_f64;
    for step in 1..=200 {
        let c = n - 1;
        let u_face = choked_face_velocity(array.p.internal[c], array.he.internal[c], array.rho.internal[c], time_s);
        array.set_outlet_velocity(Velocity::new::<meter_per_second>(u_face));
        array
            .step()
            .expect("200-step Edwards slice must step without a solver error");
        time_s += dt_s;
        if [1, 10, 50, 200].contains(&step) {
            let mut hp = Fnv::new(&format!("{tag}_p_{step}"));
            let mut hh = Fnv::new(&format!("{tag}_he_{step}"));
            let mut hr = Fnv::new(&format!("{tag}_rho_{step}"));
            let mut hu = Fnv::new(&format!("{tag}_U_{step}"));
            for c in 0..n {
                hp.push(array.p.internal[c]);
                hh.push(array.he.internal[c]);
                hr.push(array.rho.internal[c]);
                let u = array.u.internal[c];
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
fn part_b_edwards_200_steps_pimple() {
    report(&edwards_slice("edw_pimple"));
}

// ~~`part_b_edwards_200_steps_hybrid_knp` and `part_b_edwards_200_steps_knp_forced`~~
// removed 2026-10-16 with the KNP hybrid (module docs).

/// The hash moves when one input moves by one ulp: the mutation half of the
/// negative control (audit §4.7).
#[test]
fn one_ulp_input_perturbation_moves_the_hash() {
    // Superheated vapour, where v ~ 1/p, so a 1-ulp change in p must reach v.
    // (A two-phase point would not do: there T = T_sat(p), which a 1-ulp p
    // change can leave untouched -- measured at (7 MPa, 1.2 MJ/kg).)
    let (p, h) = (1.0e5, 3.0e6);
    let v = |p: f64| {
        v_ph_eqm(Pressure::new::<pascal>(p), AvailableEnergy::new::<joule_per_kilogram>(h))
            .get::<cubic_meter_per_kilogram>()
    };
    let (base, bumped) = (v(p), v(f64::from_bits(p.to_bits() + 1)));
    assert_ne!(base.to_bits(), bumped.to_bits(), "1-ulp p did not move v");
    let (mut a, mut b) = (Fnv::new(""), Fnv::new(""));
    a.push(base);
    b.push(bumped);
    assert_ne!(a.h, b.h);
}
