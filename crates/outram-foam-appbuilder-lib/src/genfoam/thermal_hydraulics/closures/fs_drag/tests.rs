// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam), commit 652b3da, EPFL,
// GPL-3.0. See the module header for provenance.
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0.

//! # Verification & validation — fluid-structure wall-friction factors
//!
//! **Scope.** These tests verify the [`FsWallFriction`] port against (a) a
//! closed-form analytical limit, (b) an independent published correlation, and
//! (c) exact algebraic reproduction of each upstream `value()` branch.
//!
//! ## Methodology
//!
//! 1. **Churchill laminar limit (analytical).** The Churchill (1977) correlation
//!    is constructed so that as `Re -> 0` the `(8/Re)^12` term dominates and
//!    `f -> 64/Re`, i.e. the exact Hagen-Poiseuille circular-pipe Darcy friction
//!    factor. Reference: `f · Re = 64` (analytical, exact). Pass criterion:
//!    relative error `< 1e-6` for `Re in {50, 100, 500, 1000}` and `< 2e-3` at
//!    `Re = 2000` (transition onset, where the turbulent term begins to
//!    contribute and the pure-laminar limit no longer holds tightly).
//! 2. **Churchill turbulent smooth-pipe cross-validation.** For a hydraulically
//!    smooth wall (`epsilon = 0`) at `Re = 1e5`, the Churchill factor is compared
//!    against the implicit Colebrook-White smooth-pipe solution
//!    `1/sqrt(f) = -2 log10(2.51/(Re sqrt(f)))`, solved here by fixed-point
//!    iteration. Reference: Colebrook-White (1937), the Moody-chart basis. Pass
//!    criterion: relative agreement `< 2%` (Churchill is a documented ~1-2%
//!    fit to Colebrook-White across the turbulent regime).
//! 3. **Port-correctness (verification).** Each remaining correlation is checked
//!    to reproduce its upstream algebra exactly, including the published Engel
//!    laminar bundle constant `f = 99/Re` and the `Re = 400` blend continuity.
//!
//! ## Results (measured 2026-07-15, `cargo test --release`, data: source formulae)
//!
//! - **Laminar limit:** `f · Re = 64` reproduced to `< 1e-10` relative at
//!   `Re in {50,100,500,1000}`; at `Re = 2000` measured `f · Re = 64.087`
//!   (0.135% high, the physical transition-onset turbulent contribution) — PASS.
//!   Confirms the correlation collapses onto Hagen-Poiseuille in the laminar
//!   regime and departs smoothly at transition.
//! - **Turbulent cross-check:** at `Re = 1e5`, `epsilon = 0`,
//!   Churchill `f = 0.017875` vs Colebrook-White `f = 0.017990`; relative
//!   difference `0.64%` (well inside the 2% band) — PASS. Confirms the turbulent
//!   branch matches the accepted smooth-pipe reference.
//! - **Port correctness:** Engel `f(100) = 0.990000` (= 99/100), all algebraic
//!   variants and the bundle-geometry constructors reproduce their formulae to
//!   machine precision — PASS.
//!
//! Interpretation: the fluid-structure friction closure is implemented correctly
//! (verification) and reproduces the physical laminar limit and the accepted
//! turbulent smooth-pipe correlation (validation) for the Churchill model, which
//! is the general-purpose all-regime choice. The bundle-specific correlations are
//! verified as faithful ports; their validation against rod-bundle pressure-drop
//! data is deferred to when the porous momentum solver (bead op-p6p.7.11) exists.

use super::FsWallFriction;
use crate::genfoam::thermal_hydraulics::units::reynolds_number;
use approx::assert_relative_eq;

/// Bare-`f64` friction factor at a given Reynolds number, going through the full
/// `uom` boundary (so the test also exercises the typed API).
fn f_of(model: &FsWallFriction, re: f64) -> f64 {
    model
        .friction_factor(reynolds_number(re))
        .get::<uom::si::ratio::ratio>()
}

/// V&V 1 — Churchill collapses onto the analytical laminar limit `f·Re = 64`.
#[test]
fn churchill_recovers_laminar_hagen_poiseuille() {
    let model = FsWallFriction::Churchill {
        surface_roughness: 0.0,
    };
    for &re in &[50.0, 100.0, 500.0, 1000.0] {
        let f_re = f_of(&model, re) * re;
        assert_relative_eq!(f_re, 64.0, max_relative = 1e-6);
    }
    // At Re = 2000 (transition onset) the turbulent term contributes ~0.14%,
    // so the pure-laminar limit no longer holds tightly — this is physics, not
    // a port error. Measured f·Re = 64.087 (0.135% above 64).
    let f_re_2000 = f_of(&model, 2000.0) * 2000.0;
    assert_relative_eq!(f_re_2000, 64.0, max_relative = 2e-3);
}

/// V&V 2 — Churchill smooth-pipe turbulent branch vs Colebrook-White.
#[test]
fn churchill_matches_colebrook_white_smooth_pipe() {
    let re = 1.0e5;
    let churchill = f_of(
        &FsWallFriction::Churchill {
            surface_roughness: 0.0,
        },
        re,
    );

    // Implicit Colebrook-White smooth-pipe (epsilon = 0), fixed-point iteration.
    let mut f = 0.02_f64;
    for _ in 0..100 {
        let inv_sqrt = -2.0 * (2.51 / (re * f.sqrt())).log10();
        f = 1.0 / (inv_sqrt * inv_sqrt);
    }

    assert_relative_eq!(churchill, f, max_relative = 0.02);
}

/// V&V 3a — Engel bundle laminar branch is the published `f = 99/Re`.
#[test]
fn engel_laminar_constant_is_99() {
    let engel = FsWallFriction::Engel;
    assert_relative_eq!(f_of(&engel, 100.0), 99.0 / 100.0, max_relative = 1e-12);
    assert_relative_eq!(f_of(&engel, 300.0), 99.0 / 300.0, max_relative = 1e-12);
    // modifiedEngel uses 110/Re in the laminar branch.
    let m = FsWallFriction::ModifiedEngel;
    assert_relative_eq!(f_of(&m, 100.0), 110.0 / 100.0, max_relative = 1e-12);
}

/// V&V 3b — the blended bundle correlations are continuous at `Re = 400`
/// (blend weight `psi -> 0`, so `f -> fl`, the laminar branch).
#[test]
fn bundle_blend_is_continuous_at_re_400() {
    let engel = FsWallFriction::Engel;
    // Just above 400 the blend returns essentially the laminar value 99/Re.
    let just_above = f_of(&engel, 400.0 + 1e-6);
    assert_relative_eq!(just_above, 99.0 / (400.0 + 1e-6), max_relative = 1e-3);
    // At/below 400 it is exactly the laminar branch.
    assert_relative_eq!(f_of(&engel, 400.0), 99.0 / 400.0, max_relative = 1e-12);
}

/// V&V 3c — the explicit algebraic fits reproduce their formulae exactly.
#[test]
fn algebraic_fits_reproduce_formulae() {
    // Colebrook power-law fit: f = (coeff·log10(Re) + const)^exp.
    let cole = FsWallFriction::Colebrook {
        coeff: -1.8,
        constant: 1.64,
        exp: -2.0,
    };
    let re = 5.0e4_f64;
    let expected = (-1.8 * re.log10() + 1.64).powf(-2.0);
    assert_relative_eq!(f_of(&cole, re), expected, max_relative = 1e-12);

    // ReynoldsPower: f = coeff·Re^exp + const (e.g. Blasius 0.316·Re^-0.25).
    let blasius = FsWallFriction::ReynoldsPower {
        coeff: 0.316,
        exp: -0.25,
        constant: 0.0,
    };
    assert_relative_eq!(
        f_of(&blasius, 1.0e4),
        0.316 * (1.0e4_f64).powf(-0.25),
        max_relative = 1e-12
    );
}

/// V&V 3d — the wire-wrap geometry constructors reproduce GeN-Foam's coefficient
/// derivation. Checked against an independent hand computation for a concrete
/// bundle geometry (Dp = 8 mm, Dw = 1.4 mm, H = 200 mm).
#[test]
fn baxi_dalle_donne_geometry_coefficients() {
    let dp = 8.0e-3;
    let dw = 1.4e-3;
    let h = 0.2;
    let model = FsWallFriction::baxi_dalle_donne_from_geometry(dp, dw, h);

    let pt = dp + 1.0444 * dw;
    let ratio_pd = pt / dp;
    let a = (80.0 / (100.0 * h).sqrt()) * ratio_pd.powf(1.5);
    let b = 1.034 / ratio_pd.powf(0.124);
    let c = 29.7 * ratio_pd.powf(6.9) / (h / (dp + dw)).powf(2.239);

    match model {
        FsWallFriction::BaxiDalleDonne {
            a: ma,
            b: mb,
            c: mc,
        } => {
            assert_relative_eq!(ma, a, max_relative = 1e-12);
            assert_relative_eq!(mb, b, max_relative = 1e-12);
            assert_relative_eq!(mc, c, max_relative = 1e-12);
        }
        _ => panic!("constructor returned wrong variant"),
    }

    // No & Kazimi differs only in the laminar coefficient (32 vs 80).
    let nk = FsWallFriction::no_kazimi_from_geometry(dp, dw, h);
    if let FsWallFriction::NoKazimi { a: ma, .. } = nk {
        assert_relative_eq!(
            ma,
            (32.0 / (100.0 * h).sqrt()) * ratio_pd.powf(1.5),
            max_relative = 1e-12
        );
    } else {
        panic!("constructor returned wrong variant");
    }
}
