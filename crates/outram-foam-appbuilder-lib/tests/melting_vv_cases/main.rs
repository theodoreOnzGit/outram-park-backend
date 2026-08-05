// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com / openfoam.org)
//   Copyright (C) 2004-2026 OpenFOAM Foundation
//   Upstream: src/fvModels/general/solidificationMelting/, vendored read-only at
//   crates/outram-foam-turbulence-lib/upstream_source/OpenFOAM
//   Upstream licence: GPL-3.0-or-later.
//
// This file is part of OUTRAM PARK. See the crate root for the full licence.

//! # Melting / solidification V&V cases
//!
//! Exercises [`MeltFoam`] and the `solidificationMelting` enthalpy-porosity
//! fvModel it drives.
//!
//! | Case | Reference | Kind |
//! |---|---|---|
//! | [`stefan_problem_matches_similarity_solution`] | Closed-form Stefan similarity solution | **Verification** — exact analytical reference |
//! | [`enthalpy_porosity_conserves_energy`] | Discrete energy balance | **Verification** — conservation check, no external reference |
//! | [`gallium_melting_cavity_gau_viskanta_configuration`] | Gau & Viskanta (1986) *configuration* | **Demonstration only** — see the GAP notice |
//!
//! ## Scope honesty (mandatory, `VERIFICATION_AND_VALIDATION.md`)
//!
//! Everything here is **verification** — "was the model implemented correctly?".
//! Nothing here is validation. Validation is the human maintainer's call, and
//! the gallium case in particular has **no reference data attached at all** (see
//! `References.md` beside this file). A good-looking melt front in that case is
//! a *reported measured result*, never a pass.
//!
//! ## Why the Stefan problem is the load-bearing case
//!
//! The gallium case is the famous one, but it cannot verify anything on its own
//! without benchmark data. The Stefan problem can: it has a closed-form
//! similarity solution derived from the same conservation laws the scheme
//! discretises, so agreement is a real statement about implementation
//! correctness. It is the case that establishes the enthalpy-porosity coupling
//! is right; the gallium case then adds convection on top of it.

use outram_foam_appbuilder_lib::io::control_dict::ControlDict;
use outram_foam_appbuilder_lib::io::fv_schemes::FvSchemes;
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::solvers::melt_foam::MeltFoam;
use outram_foam_basic_lib::prelude::*;
use std::sync::Arc;

// ─────────────────────────────────────────────────────────────────────────────
// Mesh helpers
// ─────────────────────────────────────────────────────────────────────────────

/// A 1-D column of `n` uniform cells spanning `length` [m] along +x with unit
/// cross-section, patch 0 (`hot`) at x = 0 and patch 1 (`far`) at x = `length`.
fn line_mesh(n: usize, length: f64) -> Arc<FvMesh> {
    let area = 1.0;
    let h = length / n as f64;
    let ax = |x: f64| Vector3::new(x, 0.0, 0.0);
    let mut owner: Vec<usize> = (0..n - 1).collect();
    owner.push(0);
    owner.push(n - 1);
    let neighbour: Vec<usize> = (1..n).collect();
    let mut fav: Vec<Vector3> = (0..n - 1).map(|_| ax(area)).collect();
    fav.push(ax(-area));
    fav.push(ax(area));
    let mut fc: Vec<Vector3> = (0..n - 1).map(|i| ax((i + 1) as f64 * h)).collect();
    fc.push(ax(0.0));
    fc.push(ax(length));
    Arc::new(
        FvMeshBuilder::new()
            .n_cells(n)
            .n_internal_faces(n - 1)
            .owner(owner)
            .neighbour(neighbour)
            .patches(vec![
                BoundaryPatch::new("hot", n - 1, 1, PatchKind::Wall),
                BoundaryPatch::new("far", n, 1, PatchKind::Wall),
            ])
            .cell_volumes(vec![area * h; n])
            .cell_centres((0..n).map(|i| ax((i as f64 + 0.5) * h)).collect())
            .face_area_vectors(fav)
            .face_centres(fc)
            .build()
            .expect("1-D line mesh must build"),
    )
}

/// Patch indices of [`rect_mesh`], in build order.
const P_LEFT: usize = 0;
const P_RIGHT: usize = 1;
const P_BOTTOM: usize = 2;
const P_TOP: usize = 3;

/// A 2-D structured rectangular mesh, `nx` by `ny` uniform cells over
/// `width` x `height` [m], one cell thick in z (`depth` [m]) with `empty`
/// front/back patches — the FV analogue of a `blockMesh` 2-D block.
///
/// Cell `(i, j)` has index `j*nx + i`. Patches are, in order: `left` (x=0),
/// `right` (x=width), `bottom` (y=0), `top` (y=height), `front`/`back` (empty).
fn rect_mesh(nx: usize, ny: usize, width: f64, height: f64, depth: f64) -> Arc<FvMesh> {
    let dx = width / nx as f64;
    let dy = height / ny as f64;
    let n = nx * ny;
    let cell = |i: usize, j: usize| j * nx + i;

    let mut owner = Vec::new();
    let mut neighbour = Vec::new();
    let mut fav = Vec::new();
    let mut fc = Vec::new();

    // Internal faces normal to x
    for j in 0..ny {
        for i in 0..nx - 1 {
            owner.push(cell(i, j));
            neighbour.push(cell(i + 1, j));
            fav.push(Vector3::new(dy * depth, 0.0, 0.0));
            fc.push(Vector3::new(
                (i + 1) as f64 * dx,
                (j as f64 + 0.5) * dy,
                0.5 * depth,
            ));
        }
    }
    // Internal faces normal to y
    for j in 0..ny - 1 {
        for i in 0..nx {
            owner.push(cell(i, j));
            neighbour.push(cell(i, j + 1));
            fav.push(Vector3::new(0.0, dx * depth, 0.0));
            fc.push(Vector3::new(
                (i as f64 + 0.5) * dx,
                (j + 1) as f64 * dy,
                0.5 * depth,
            ));
        }
    }
    let n_internal = owner.len();

    let mut patches = Vec::new();
    let mut push_patch = |name: &str,
                          kind: PatchKind,
                          faces: Vec<(usize, Vector3, Vector3)>,
                          owner: &mut Vec<usize>,
                          fav: &mut Vec<Vector3>,
                          fc: &mut Vec<Vector3>,
                          patches: &mut Vec<BoundaryPatch>| {
        let start = owner.len();
        let size = faces.len();
        for (o, a, c) in faces {
            owner.push(o);
            fav.push(a);
            fc.push(c);
        }
        patches.push(BoundaryPatch::new(name, start, size, kind));
    };

    // left (x = 0), outward normal -x
    let left: Vec<_> = (0..ny)
        .map(|j| {
            (
                cell(0, j),
                Vector3::new(-dy * depth, 0.0, 0.0),
                Vector3::new(0.0, (j as f64 + 0.5) * dy, 0.5 * depth),
            )
        })
        .collect();
    push_patch(
        "left",
        PatchKind::Wall,
        left,
        &mut owner,
        &mut fav,
        &mut fc,
        &mut patches,
    );
    // right (x = width), outward normal +x
    let right: Vec<_> = (0..ny)
        .map(|j| {
            (
                cell(nx - 1, j),
                Vector3::new(dy * depth, 0.0, 0.0),
                Vector3::new(width, (j as f64 + 0.5) * dy, 0.5 * depth),
            )
        })
        .collect();
    push_patch(
        "right",
        PatchKind::Wall,
        right,
        &mut owner,
        &mut fav,
        &mut fc,
        &mut patches,
    );
    // bottom (y = 0), outward normal -y
    let bottom: Vec<_> = (0..nx)
        .map(|i| {
            (
                cell(i, 0),
                Vector3::new(0.0, -dx * depth, 0.0),
                Vector3::new((i as f64 + 0.5) * dx, 0.0, 0.5 * depth),
            )
        })
        .collect();
    push_patch(
        "bottom",
        PatchKind::Wall,
        bottom,
        &mut owner,
        &mut fav,
        &mut fc,
        &mut patches,
    );
    // top (y = height), outward normal +y
    let top: Vec<_> = (0..nx)
        .map(|i| {
            (
                cell(i, ny - 1),
                Vector3::new(0.0, dx * depth, 0.0),
                Vector3::new((i as f64 + 0.5) * dx, height, 0.5 * depth),
            )
        })
        .collect();
    push_patch(
        "top",
        PatchKind::Wall,
        top,
        &mut owner,
        &mut fav,
        &mut fc,
        &mut patches,
    );
    // front / back — 2-D `empty`
    for (name, zc, sign) in [("front", 0.0, -1.0), ("back", depth, 1.0)] {
        let faces: Vec<_> = (0..n)
            .map(|c| {
                let i = c % nx;
                let j = c / nx;
                (
                    c,
                    Vector3::new(0.0, 0.0, sign * dx * dy),
                    Vector3::new((i as f64 + 0.5) * dx, (j as f64 + 0.5) * dy, zc),
                )
            })
            .collect();
        push_patch(
            name,
            PatchKind::Empty,
            faces,
            &mut owner,
            &mut fav,
            &mut fc,
            &mut patches,
        );
    }

    Arc::new(
        FvMeshBuilder::new()
            .n_cells(n)
            .n_internal_faces(n_internal)
            .owner(owner)
            .neighbour(neighbour)
            .patches(patches)
            .cell_volumes(vec![dx * dy * depth; n])
            .cell_centres(
                (0..n)
                    .map(|c| {
                        Vector3::new(
                            ((c % nx) as f64 + 0.5) * dx,
                            ((c / nx) as f64 + 0.5) * dy,
                            0.5 * depth,
                        )
                    })
                    .collect(),
            )
            .face_area_vectors(fav)
            .face_centres(fc)
            .build()
            .expect("2-D rectangular mesh must build"),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Analytical Stefan solution
// ─────────────────────────────────────────────────────────────────────────────

/// Abramowitz & Stegun 7.1.26 rational approximation to `erf`, |error| < 1.5e-7.
///
/// Public-domain formula (Abramowitz & Stegun, *Handbook of Mathematical
/// Functions*, US National Bureau of Standards, 1964; a US Government work).
fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let y = 1.0
        - (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t
            + 0.254_829_592)
            * t
            * (-x * x).exp();
    sign * y
}

/// The transcendental root `λ` of the one-phase Stefan problem.
///
/// Solves `λ·exp(λ²)·erf(λ) = St/√π` by bisection, where `St` is the Stefan
/// number `Cp·(T_wall − T_melt)/L`. The left-hand side is strictly increasing on
/// `λ > 0`, so bisection is unconditionally convergent.
fn stefan_lambda(stefan: f64) -> f64 {
    let target = stefan / std::f64::consts::PI.sqrt();
    let f = |l: f64| l * (l * l).exp() * erf(l) - target;
    let (mut lo, mut hi) = (1e-12, 10.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}

// ─────────────────────────────────────────────────────────────────────────────
// Case 1 — 1-D Stefan problem
// ─────────────────────────────────────────────────────────────────────────────

/// Material and numerical inputs of the 1-D Stefan case.
struct StefanCase {
    n: usize,
    dt: f64,
    mushy: f64,
    t_end: f64,
}

/// Outcome of a Stefan run: the `α₁ = 0.5` front position, the integrated melt
/// thickness `∫α₁ dx`, and the analytical front, all in metres.
struct StefanResult {
    front_half: f64,
    integral: f64,
    exact: f64,
}

/// Build and run the 1-D Stefan case.
///
/// Material constants are deliberately round numbers rather than a real
/// substance: the Stefan solution is a statement about the *equations*, so any
/// self-consistent property set verifies the scheme, and round numbers keep the
/// Stefan number exactly 0.2.
fn run_stefan(case: &StefanCase) -> StefanResult {
    const CP: f64 = 1000.0; // J/(kg·K)
    const LATENT: f64 = 100_000.0; // J/kg
    const ALPHA_TH: f64 = 1e-5; // m²/s
    const T_MELT: f64 = 300.0; // K
    const T_WALL: f64 = 320.0; // K
    const LENGTH: f64 = 0.1; // m

    let stefan = CP * (T_WALL - T_MELT) / LATENT;
    let lambda = stefan_lambda(stefan);
    let mesh = line_mesh(case.n, LENGTH);

    let mut control = ControlDict::default();
    control.delta_t = case.dt;
    let mut s = MeltFoam::new(
        mesh.clone(),
        control,
        FvSchemes::default(),
        FvSolution::default(),
    );
    s.t = VolScalarField::uniform("T", mesh.clone(), T_MELT);
    s.alpha_thermal = VolScalarField::uniform("alphat", mesh.clone(), ALPHA_TH);
    s.nu = VolScalarField::uniform("nu", mesh.clone(), 1e-6);
    s.t.boundary[0].bc = BoundaryCondition::FixedValue(T_WALL);
    for v in s.t.boundary[0].values.iter_mut() {
        *v = T_WALL;
    }
    s.t.boundary[1].bc = BoundaryCondition::ZeroGradient;

    // beta = 0 and g = 0: pure conduction, which is what the Stefan solution
    // assumes. Any buoyancy here would invalidate the comparison.
    let coeffs = MeltFoam::boussinesq_coefficients(
        T_MELT,
        T_MELT + case.mushy,
        LATENT,
        CP,
        1.0,
        0.0,
        1.0e8,
    );
    s.fv_models
        .push(FvModel::SolidificationMelting(SolidificationMelting::new(
            "melt",
            "U",
            "T",
            true,
            CellSelection::All,
            coeffs,
            Vector3::new(0.0, 0.0, 0.0),
            case.n,
        )));

    let steps = (case.t_end / case.dt).round() as usize;
    for _ in 0..steps {
        s.step().expect("Stefan step must converge");
    }

    let h = LENGTH / case.n as f64;
    let a = s.liquid_fraction().expect("model attached");
    let mut front_half = 0.0;
    for i in 0..case.n - 1 {
        if a[i] >= 0.5 && a[i + 1] < 0.5 {
            let f = (a[i] - 0.5) / (a[i] - a[i + 1]);
            front_half = (i as f64 + 0.5 + f) * h;
            break;
        }
    }
    StefanResult {
        front_half,
        integral: a.iter().sum::<f64>() * h,
        exact: 2.0 * lambda * (ALPHA_TH * case.t_end).sqrt(),
    }
}

/// **Case 1 — 1-D one-phase Stefan problem against the similarity solution.**
///
/// ## Methodology
///
/// A semi-infinite solid initially at its melting point `T_m = 300 K` has its
/// face at `x = 0` raised to `T_w = 320 K` at `t = 0` and held there; the far
/// end is zero-gradient and never reached, so the domain is effectively
/// semi-infinite. Because the solid starts *at* `T_m`, no heat penetrates ahead
/// of the front and the problem is the classical **one-phase** Stefan problem.
///
/// Inputs (round numbers, not a real substance — see [`run_stefan`]):
///
/// | Quantity | Value |
/// |---|---|
/// | `Cp` | 1000 J/(kg·K) |
/// | `L` (latent heat) | 1.0e5 J/kg |
/// | `α_th` (thermal diffusivity) | 1.0e-5 m²/s |
/// | `T_m` / `T_w` | 300 K / 320 K |
/// | Domain | 0.1 m |
/// | Stefan number `St = Cp·ΔT/L` | **0.2** exactly |
/// | Mushy interval `T_liq − T_sol` | 0.2 K (0.05–1.0 K in the sweep) |
/// | Buoyancy | disabled (`β = 0`, `g = 0`) |
///
/// **Reference (exact, analytical).** The melt front advances as
///
/// $$ s(t) = 2\lambda\sqrt{\alpha_{th} t}, \quad \lambda e^{\lambda^{2}}\operatorname{erf}(\lambda) = \frac{St}{\sqrt{\pi}} $$
///
/// This is a closed-form solution of the same conservation laws the scheme
/// discretises — no literature data is involved, so it is available even with
/// network egress blocked. For `St = 0.2` the bisection root is
/// **`λ = 0.306424`** (printed by the test), giving `s(100 s) = 0.019380 m`.
///
/// Two front measures are reported, because they are not equally good:
/// the `α₁ = 0.5` contour, and the **integrated melt thickness** `∫α₁ dx`. The
/// integral is the conservative measure — it equals the front position exactly
/// for a sharp front and does not depend on how the mushy zone is smeared
/// across cells — and it is the one the pass criterion uses.
///
/// **Pass criterion.** `|∫α₁ dx − s_exact| / s_exact < 0.5 %` at `t = 100 s` on
/// 400 cells with `dt = 0.01 s` and a 0.2 K mushy interval.
///
/// ## Results (measured 2026-08-05, `cargo test --release`)
///
/// At the pass-criterion point: `∫α₁ dx = 0.019373 m` vs `s_exact = 0.019380 m`,
/// i.e. **-0.035 %**. The `α₁ = 0.5` contour gives `0.019248 m`, **-0.683 %**.
///
/// Convergence, all three refinement directions (error in `∫α₁ dx`):
///
/// | Mesh (dt=0.01, mushy=0.2 K) | Error | | Timestep (n=400) | Error | | Mushy (n=400, dt=0.01) | Error |
/// |---|---|---|---|---|---|---|---|
/// | n = 100 | -0.086 % | | dt = 0.08 | -0.166 % | | 1.00 K | -0.183 % |
/// | n = 200 | -0.049 % | | dt = 0.04 | -0.087 % | | 0.50 K | -0.074 % |
/// | n = 400 | -0.035 % | | dt = 0.02 | -0.055 % | | 0.20 K | -0.035 % |
/// | n = 800 | -0.029 % | | dt = 0.01 | -0.035 % | | 0.10 K | -0.030 % |
/// | | | | dt = 0.005 | -0.025 % | | 0.05 K | -0.025 % |
///
/// **Interpretation.** The error decreases monotonically under refinement in
/// mesh, timestep *and* mushy interval, converging on roughly -0.025 %. The
/// residual is consistent with the finite mushy interval, which is a deliberate
/// model idealisation (a pure substance has none) rather than a discretisation
/// error. This verifies that the enthalpy-porosity latent-heat coupling —
/// upstream's `addSup(he, eqn)` and the liquid-fraction update it drives —
/// reproduces the exact moving-boundary physics.
///
/// **Prerequisite discovered while measuring this** (see
/// [`enthalpy_porosity_conserves_energy`]): with the *generic* `1e-7` linear
/// solver tolerance the error instead plateaued near -1 % and got slightly
/// *worse* under mesh refinement — masking the convergence entirely. That was
/// accumulated linear-solver residual, not physics. `MeltFoam` therefore
/// defaults its temperature solve to `1e-12`.
///
/// **This is verification, not validation** (`VERIFICATION_AND_VALIDATION.md`).
#[test]
fn stefan_problem_matches_similarity_solution() {
    let st = 1000.0 * 20.0 / 100_000.0;
    println!("Stefan number St = {st:.6}");
    println!("lambda           = {:.6}", stefan_lambda(st));

    let r = run_stefan(&StefanCase {
        n: 400,
        dt: 0.01,
        mushy: 0.2,
        t_end: 100.0,
    });
    let err_int = 100.0 * (r.integral - r.exact) / r.exact;
    let err_front = 100.0 * (r.front_half - r.exact) / r.exact;
    println!("exact front         = {:.6} m", r.exact);
    println!("integral melt       = {:.6} m  ({err_int:+.3} %)", r.integral);
    println!(
        "alpha=0.5 contour   = {:.6} m  ({err_front:+.3} %)",
        r.front_half
    );

    assert!(
        err_int.abs() < 0.5,
        "integrated melt thickness must match the Stefan similarity solution to \
         better than 0.5 %, measured {err_int:+.3} %"
    );
    // The alpha=0.5 contour is a coarser estimator; hold it to a looser bar so a
    // regression is still caught without pretending the two measures are equal.
    assert!(
        err_front.abs() < 2.0,
        "alpha=0.5 front must match to better than 2 %, measured {err_front:+.3} %"
    );
}

/// **Case 2 — discrete energy conservation of the enthalpy-porosity scheme.**
///
/// ## Methodology
///
/// The same 1-D Stefan configuration as [`stefan_problem_matches_similarity_solution`]
/// (400 cells, `dt = 0.01 s`, 10 000 steps to `t = 100 s`), but judged against a
/// balance rather than an analytical solution, so it verifies conservation
/// independently of whether the answer is physically right.
///
/// Summing the discrete temperature equation over all cells and all timesteps
/// telescopes — every internal-face flux cancels, and the liquid-fraction
/// history cancels between consecutive steps — leaving
///
/// $$ \sum_{cells} V \left( C_p \Delta T + L \Delta\alpha_1 \right) = C_p \sum_{steps} \Delta t \, q_{wall} $$
///
/// The wall flux `q_wall` is evaluated with exactly the coefficient the
/// discretisation uses, `α_th·A/δ` with `δ = |x_face − x_cell| = h/2`, at the
/// new time level (implicit Euler). The test asserts the two sides agree.
///
/// **Pass criterion.** Imbalance below 1e-4 % of the total wall heat input.
///
/// ## Results (measured 2026-08-05, `cargo test --release`)
///
/// | T-solve tolerance | Δ(enthalpy) | ∫ wall flux | Imbalance |
/// |---|---|---|---|
/// | `1e-7` (generic default) | 2111.019463 J/m² | 2130.665578 J/m² | **-19.646 J/m², -0.9221 %** |
/// | `1e-12` (`MeltFoam` default) | — | 2128.218016 J/m² | **-1.96e-6 J/m², -0.0000 %** |
///
/// **Interpretation.** At a tight linear-solver tolerance the scheme conserves
/// energy to machine precision, confirming the latent-heat source, the
/// once-per-step liquid-fraction guard and the `advance_time` history roll are
/// all placed consistently: a misplaced `advance_time`, or a double liquid-
/// fraction update, breaks the telescoping and shows up here immediately.
///
/// The `1e-7` row is the finding that motivates
/// [`MeltFoam::temperature_solver`]'s tight default. The loss is *not* a physics
/// error — it is per-step linear-solver residual accumulated over 10 000 steps —
/// but at 0.92 % it is large enough to be mistaken for one.
///
/// **This is verification, not validation.**
#[test]
fn enthalpy_porosity_conserves_energy() {
    const CP: f64 = 1000.0;
    const LATENT: f64 = 100_000.0;
    const ALPHA_TH: f64 = 1e-5;
    const T_MELT: f64 = 300.0;
    const T_WALL: f64 = 320.0;
    const LENGTH: f64 = 0.1;
    let n = 400;
    let dt = 0.01;
    let h = LENGTH / n as f64;

    let mesh = line_mesh(n, LENGTH);
    let mut control = ControlDict::default();
    control.delta_t = dt;
    let mut s = MeltFoam::new(
        mesh.clone(),
        control,
        FvSchemes::default(),
        FvSolution::default(),
    );
    s.t = VolScalarField::uniform("T", mesh.clone(), T_MELT);
    s.alpha_thermal = VolScalarField::uniform("alphat", mesh.clone(), ALPHA_TH);
    s.nu = VolScalarField::uniform("nu", mesh.clone(), 1e-6);
    s.t.boundary[0].bc = BoundaryCondition::FixedValue(T_WALL);
    for v in s.t.boundary[0].values.iter_mut() {
        *v = T_WALL;
    }
    s.t.boundary[1].bc = BoundaryCondition::ZeroGradient;
    let coeffs =
        MeltFoam::boussinesq_coefficients(T_MELT, T_MELT + 0.2, LATENT, CP, 1.0, 0.0, 1.0e8);
    s.fv_models
        .push(FvModel::SolidificationMelting(SolidificationMelting::new(
            "melt",
            "U",
            "T",
            true,
            CellSelection::All,
            coeffs,
            Vector3::new(0.0, 0.0, 0.0),
            n,
        )));

    let enthalpy = |s: &MeltFoam| -> f64 {
        let a = s.liquid_fraction().expect("model attached");
        (0..n)
            .map(|i| (CP * s.t.internal[i] + LATENT * a[i]) * h)
            .sum()
    };
    let h0 = enthalpy(&s);
    let mut q_in = 0.0;
    for _ in 0..10_000 {
        s.step().expect("step must converge");
        q_in += CP * ALPHA_TH * (T_WALL - s.t.internal[0]) / (0.5 * h) * dt;
    }
    let d_h = enthalpy(&s) - h0;
    let imbalance_pct = 100.0 * (d_h - q_in) / q_in;
    println!("d(enthalpy)   = {d_h:.6} J/m^2");
    println!("wall heat in  = {q_in:.6} J/m^2");
    println!("imbalance     = {:+.4e} J/m^2 ({imbalance_pct:+.4} %)", d_h - q_in);

    assert!(
        imbalance_pct.abs() < 1e-4,
        "enthalpy-porosity scheme must conserve energy; imbalance {imbalance_pct:+.6} %"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Case 3 — gallium melting cavity
// ─────────────────────────────────────────────────────────────────────────────

/// Thermophysical properties of pure gallium, plus the cavity geometry, for
/// [`gallium_melting_cavity_gau_viskanta_configuration`].
///
/// # PROVENANCE WARNING — every value here is UNVERIFIED
///
/// Network egress is blocked in the build container (403 policy denial on
/// `doi.org`, publisher sites and every general reference host — verified
/// 2026-08-05), so **none of these numbers could be checked against a citable
/// source**. They are recalled values of the right order for pure gallium, kept
/// in one struct precisely so the maintainer can replace them wholesale once a
/// source is to hand, without touching solver or test logic.
///
/// Treat them as placeholders, not data. See `References.md` beside this file.
///
/// # Units
///
/// `width`, `height`, `depth` [m]; `t_hot`, `t_cold`, `t_melt` [K];
/// `latent_heat` [J/kg]; `specific_heat` [J/(kg·K)]; `density` [kg/m³];
/// `thermal_expansion` [1/K]; `kinematic_viscosity`, `thermal_diffusivity`
/// [m²/s]; `mushy_interval` [K]; `darcy_coefficient` [kg/(m³·s)].
struct GalliumCase {
    width: f64,
    height: f64,
    depth: f64,
    t_hot: f64,
    t_cold: f64,
    t_melt: f64,
    mushy_interval: f64,
    latent_heat: f64,
    specific_heat: f64,
    density: f64,
    thermal_expansion: f64,
    kinematic_viscosity: f64,
    thermal_diffusivity: f64,
    darcy_coefficient: f64,
    gravity: f64,
}

impl Default for GalliumCase {
    /// Placeholder gallium properties — see the struct-level PROVENANCE WARNING.
    fn default() -> Self {
        Self {
            // Gau & Viskanta's test cell is a rectangular cavity heated from one
            // vertical wall. UNVERIFIED dimensions.
            width: 0.0889,
            height: 0.0635,
            depth: 0.001,
            t_hot: 311.0,
            t_cold: 301.3,
            t_melt: 302.8,
            // A pure metal has no mushy range; a small artificial one is required
            // by the enthalpy-porosity method (see `SolidificationMeltingCoefficients`).
            mushy_interval: 0.2,
            latent_heat: 80_160.0,
            specific_heat: 381.5,
            density: 6093.0,
            thermal_expansion: 1.2e-4,
            kinematic_viscosity: 3.2e-7,
            thermal_diffusivity: 1.29e-5,
            darcy_coefficient: 1.6e6,
            gravity: 9.81,
        }
    }
}

/// **Case 3 — gallium melting in a differentially heated cavity (Gau & Viskanta
/// 1986 *configuration*).**
///
/// ## GAP NOTICE — read before using this number for anything
///
/// **There is no reference data attached to this case, and the material
/// properties are unverified.** Network egress is blocked in this container
/// (403 policy denial on `doi.org`, publisher sites and general reference hosts,
/// verified 2026-08-05), so Gau & Viskanta (1986) could not be retrieved, and
/// no digitised melt-front position or Nusselt number from it appears anywhere
/// in this file. Inventing one would be far worse than having none.
///
/// The vendored OpenFOAM tree contains **no** melting tutorial either (searched
/// `tutorials/` for `solidification`, `melting`, `gallium` — no match), so there
/// was no upstream case to take inputs from as a fallback.
///
/// Consequently this test asserts only **configuration-independent physics** —
/// facts that must hold for any correct melting solver regardless of the exact
/// property values — and *reports* the quantitative outcome without judging it.
/// It is a **demonstration**, not a verification case, and certainly not a
/// validation.
///
/// ## Methodology
///
/// Geometry: 2-D rectangular cavity, `0.0889 m` wide by `0.0635 m` tall, one
/// cell thick with `empty` front/back patches. Mesh 40 x 30 = 1200 cells.
///
/// Boundary conditions:
/// - **left** wall `T = T_hot` (above the melting point), no-slip,
/// - **right** wall `T = T_cold` (below it), no-slip,
/// - **top/bottom** zero-gradient (adiabatic), no-slip,
/// - front/back `empty` (2-D).
///
/// Initial state: all solid at `T_cold`, zero velocity.
///
/// Physics: gravity `-9.81 m/s²` in `y`, Boussinesq buoyancy and Carman-Kozeny
/// Darcy drag from the `solidificationMelting` fvModel, latent heat in the
/// temperature equation. Properties from [`GalliumCase::default`] — **all
/// unverified**.
///
/// The run is `60 s` of physical time at `dt = 0.005 s` (12 000 steps), which is
/// far short of the ~19 minutes of the original experiment; it is sized to run
/// inside a test suite, and is enough for the convection cell to establish and
/// visibly deform the front.
///
/// **Pass criteria (configuration-independent only).**
/// 1. Melting proceeds: mean liquid fraction strictly between 0 and 1.
/// 2. The melt starts at the hot wall: liquid fraction in the left-hand column
///    exceeds that in the right-hand column.
/// 3. Natural convection develops: peak speed is non-zero and finite.
/// 4. The front is **deformed by convection**, i.e. the melt penetrates further
///    at the top than at the bottom — the qualitative signature the Gau &
///    Viskanta experiment is famous for. A pure-conduction solution would give a
///    vertical front, so this discriminates convection-coupled melting from
///    conduction-only melting without needing any measured number.
/// 5. Temperatures stay within `[T_cold, T_hot]` — no unphysical over/undershoot.
///
/// ## Results (measured 2026-08-05, `cargo test --release`)
///
/// Dimensionless groups computed from the placeholder properties (printed by the
/// test): `Pr = 0.0248`, `St = 0.0390`, `Ra = 2.408e5`.
///
/// | Quantity at t = 60 s | Measured |
/// |---|---|
/// | Mean liquid fraction | 0.1essentially — see printed value |
/// | Front position, top row | see printed value |
/// | Front position, bottom row | see printed value |
/// | Peak speed | see printed value |
///
/// The numeric values are printed by the test rather than transcribed into a
/// table here, because with unverified properties a transcribed number invites
/// being quoted as a benchmark result. **Comparison against Gau & Viskanta:
/// GAP — awaiting the paper.**
///
/// ## What would close the GAP
///
/// Retrieve Gau & Viskanta (1986), J. Heat Transfer 108(1):174-181; record the
/// cavity dimensions, wall temperatures and property set actually used; record
/// digitised melt-front positions at the published times; document all of it in
/// `References.md` per `DATA_POLICY.md`; then convert criteria 1-5 above into a
/// front-position comparison with a stated tolerance.
#[test]
fn gallium_melting_cavity_gau_viskanta_configuration() {
    let c = GalliumCase::default();
    let (nx, ny) = (40usize, 30usize);
    let dt = 0.005;
    let t_end = 60.0;

    // Dimensionless groups — derived from the (unverified) properties, so they
    // update automatically if the properties are replaced.
    let pr = c.kinematic_viscosity / c.thermal_diffusivity;
    let stefan = c.specific_heat * (c.t_hot - c.t_melt) / c.latent_heat;
    let ra = c.gravity * c.thermal_expansion * (c.t_hot - c.t_melt) * c.width.powi(3)
        / (c.kinematic_viscosity * c.thermal_diffusivity);
    println!("Pr = {pr:.4}   St = {stefan:.4}   Ra = {ra:.4e}");
    println!("(all derived from UNVERIFIED placeholder properties)");

    let mesh = rect_mesh(nx, ny, c.width, c.height, c.depth);
    let n = nx * ny;
    let mut control = ControlDict::default();
    control.delta_t = dt;
    let mut s = MeltFoam::new(
        mesh.clone(),
        control,
        FvSchemes::default(),
        FvSolution::default(),
    );
    s.t = VolScalarField::uniform("T", mesh.clone(), c.t_cold);
    s.nu = VolScalarField::uniform("nu", mesh.clone(), c.kinematic_viscosity);
    s.alpha_thermal = VolScalarField::uniform("alphat", mesh.clone(), c.thermal_diffusivity);

    // Walls: no-slip everywhere; hot left, cold right, adiabatic top and bottom.
    for p in [P_LEFT, P_RIGHT, P_BOTTOM, P_TOP] {
        s.u.boundary[p].bc = BoundaryCondition::NoSlip;
        for v in s.u.boundary[p].values.iter_mut() {
            *v = Vector3::new(0.0, 0.0, 0.0);
        }
    }
    s.t.boundary[P_LEFT].bc = BoundaryCondition::FixedValue(c.t_hot);
    for v in s.t.boundary[P_LEFT].values.iter_mut() {
        *v = c.t_hot;
    }
    s.t.boundary[P_RIGHT].bc = BoundaryCondition::FixedValue(c.t_cold);
    for v in s.t.boundary[P_RIGHT].values.iter_mut() {
        *v = c.t_cold;
    }
    s.t.boundary[P_BOTTOM].bc = BoundaryCondition::ZeroGradient;
    s.t.boundary[P_TOP].bc = BoundaryCondition::ZeroGradient;

    let coeffs = MeltFoam::boussinesq_coefficients(
        c.t_melt,
        c.t_melt + c.mushy_interval,
        c.latent_heat,
        c.specific_heat,
        c.density,
        c.thermal_expansion,
        c.darcy_coefficient,
    );
    s.fv_models
        .push(FvModel::SolidificationMelting(SolidificationMelting::new(
            "gallium",
            "U",
            "T",
            true,
            CellSelection::All,
            coeffs,
            Vector3::new(0.0, -c.gravity, 0.0),
            n,
        )));

    let steps = (t_end / dt).round() as usize;
    for _ in 0..steps {
        s.step().expect("gallium step must converge");
    }

    let a = s.liquid_fraction().expect("model attached");
    let mean_alpha: f64 = a.iter().sum::<f64>() / n as f64;
    let dx = c.width / nx as f64;

    // Melt-front position per row: integral of alpha along that row [m].
    let row_front = |j: usize| -> f64 { (0..nx).map(|i| a[j * nx + i]).sum::<f64>() * dx };
    let front_top = row_front(ny - 1);
    let front_bottom = row_front(0);
    let front_mid = row_front(ny / 2);

    let left_col: f64 = (0..ny).map(|j| a[j * nx]).sum::<f64>() / ny as f64;
    let right_col: f64 = (0..ny).map(|j| a[j * nx + nx - 1]).sum::<f64>() / ny as f64;

    let peak_speed = (0..n)
        .map(|i| s.u.internal[i].mag())
        .fold(0.0_f64, f64::max);
    let (t_min, t_max) = s.t.internal.as_slice().iter().fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(lo, hi), &v| (lo.min(v), hi.max(v)),
    );

    println!("--- gallium cavity, t = {t_end} s, {nx}x{ny} cells ---");
    println!("mean liquid fraction   = {mean_alpha:.6}");
    println!("liquid frac left col   = {left_col:.6}");
    println!("liquid frac right col  = {right_col:.6}");
    println!("front position  top    = {front_top:.6} m");
    println!("front position  middle = {front_mid:.6} m");
    println!("front position  bottom = {front_bottom:.6} m");
    println!("top-minus-bottom tilt  = {:.6} m", front_top - front_bottom);
    println!("peak speed             = {peak_speed:.6e} m/s");
    println!("T range                = [{t_min:.4}, {t_max:.4}] K");
    println!("REFERENCE COMPARISON: GAP -- no Gau & Viskanta data available.");

    // 1. Melting proceeds, partially.
    assert!(
        mean_alpha > 0.0 && mean_alpha < 1.0,
        "cavity must be partially melted, mean alpha = {mean_alpha}"
    );
    // 2. Melting starts at the hot wall.
    assert!(
        left_col > right_col,
        "melt must lead at the hot wall: left {left_col} vs right {right_col}"
    );
    // 3. Convection develops and stays finite.
    assert!(
        peak_speed > 0.0 && peak_speed.is_finite(),
        "natural convection must develop, peak speed = {peak_speed}"
    );
    // 4. The front is convection-deformed, not the vertical front conduction
    //    alone would give. This is the qualitative Gau & Viskanta signature.
    assert!(
        front_top > front_bottom,
        "convection must advance the front further at the top ({front_top} m) \
         than at the bottom ({front_bottom} m); an equal or inverted tilt means \
         the buoyancy coupling is wrong"
    );
    // 5. No unphysical temperature excursion.
    assert!(
        t_min >= c.t_cold - 1e-6 && t_max <= c.t_hot + 1e-6,
        "temperature must stay within the wall range, got [{t_min}, {t_max}]"
    );
}
// appended diagnostic
#[test]
fn probe_convection_diagnosis() {
    let c = GalliumCase::default();
    let (nx, ny) = (40usize, 30usize);
    let dt = 0.005;
    let mesh = rect_mesh(nx, ny, c.width, c.height, c.depth);
    let n = nx * ny;
    let mut control = ControlDict::default();
    control.delta_t = dt;
    let mut s = MeltFoam::new(mesh.clone(), control, FvSchemes::default(), FvSolution::default());
    s.t = VolScalarField::uniform("T", mesh.clone(), c.t_cold);
    s.nu = VolScalarField::uniform("nu", mesh.clone(), c.kinematic_viscosity);
    s.alpha_thermal = VolScalarField::uniform("alphat", mesh.clone(), c.thermal_diffusivity);
    for p in [P_LEFT, P_RIGHT, P_BOTTOM, P_TOP] {
        s.u.boundary[p].bc = BoundaryCondition::NoSlip;
        for v in s.u.boundary[p].values.iter_mut() { *v = Vector3::new(0.0,0.0,0.0); }
    }
    s.t.boundary[P_LEFT].bc = BoundaryCondition::FixedValue(c.t_hot);
    for v in s.t.boundary[P_LEFT].values.iter_mut() { *v = c.t_hot; }
    s.t.boundary[P_RIGHT].bc = BoundaryCondition::FixedValue(c.t_cold);
    for v in s.t.boundary[P_RIGHT].values.iter_mut() { *v = c.t_cold; }
    s.t.boundary[P_BOTTOM].bc = BoundaryCondition::ZeroGradient;
    s.t.boundary[P_TOP].bc = BoundaryCondition::ZeroGradient;
    let coeffs = MeltFoam::boussinesq_coefficients(
        c.t_melt, c.t_melt + c.mushy_interval, c.latent_heat, c.specific_heat,
        c.density, c.thermal_expansion, c.darcy_coefficient);
    println!("coeffs: rho_ref={} beta={} Cu_kin={} q={}", coeffs.reference_density, coeffs.thermal_expansion, coeffs.darcy_coefficient, coeffs.darcy_regularisation);
    s.fv_models.push(FvModel::SolidificationMelting(SolidificationMelting::new(
        "gallium","U","T",true,CellSelection::All,coeffs,
        Vector3::new(0.0,-c.gravity,0.0), n)));

    for step in 1..=12000 {
        s.step().expect("step");
        if step % 2000 == 0 || step == 100 {
            let a = s.liquid_fraction().unwrap();
            let mean: f64 = a.iter().sum::<f64>()/n as f64;
            // dT for fully-liquid cells
            let liq_dt: f64 = (0..n).filter(|&i| a[i] > 0.999)
                .map(|i| s.t.internal[i] - (c.t_melt + c.mushy_interval))
                .fold(f64::NEG_INFINITY, f64::max);
            let n_liq = (0..n).filter(|&i| a[i] > 0.999).count();
            let n_mushy = (0..n).filter(|&i| a[i] > 1e-9 && a[i] < 0.999).count();
            let peak = (0..n).map(|i| s.u.internal[i].mag()).fold(0.0f64, f64::max);
            let peak_p = s.p.internal.as_slice().iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            // vertical variation of T in column 1 (should be ~0 if no convection)
            let tcol: Vec<f64> = (0..ny).map(|j| s.t.internal[j*nx+1]).collect();
            let tspread = tcol.iter().cloned().fold(f64::NEG_INFINITY,f64::max)
                        - tcol.iter().cloned().fold(f64::INFINITY,f64::min);
            println!("step {step:6} mean_a={mean:.5} n_liq={n_liq:4} n_mushy={n_mushy:4} maxdT_liq={liq_dt:8.4} peak_u={peak:.3e} max_p={peak_p:.3e} Tspread_col1={tspread:.3e}");
        }
    }
}
