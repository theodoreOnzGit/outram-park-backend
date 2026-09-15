// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//
// Interpolation semantics mirror code_aster's DEFI_FONCTION / DEFI_NAPPE
// (https://gitlab.com/codeaster/src, GPL-3.0-or-later, EDF R&D, commit
// b504ea08c2f49575e04644cee2e39a63ea45c16e).

//! Shared driver for verifying ported constitutive laws against code_aster
//! `astest` decks.
//!
//! # Why this exists
//!
//! An `astest` deck is a finite-element problem, and this crate has no FE
//! framework. What makes a *single-element* deck usable anyway is that its
//! stress state is uniform, so the element response *is* the constitutive
//! response and the deck reduces to a material-point drive — provided two
//! things can be reproduced:
//!
//! - **Mixed control.** Real decks rarely prescribe all six stress components.
//!   A relaxation test prescribes one *strain* and leaves the lateral faces
//!   traction-free, so the driver must solve for whichever components are not
//!   given. See [`Control`] and [`solve_mixed_control`].
//! - **Temperature-dependent properties.** code_aster material keywords ending
//!   `_FO` take functions of temperature rather than constants, so the
//!   parameters must be re-evaluated every step. See [`PiecewiseLinear`] and
//!   [`Nappe`], which reproduce `DEFI_FONCTION` and `DEFI_NAPPE`.
//!
//! # Scope
//!
//! Deliberately small. This is a test-support module, not a solver: it does no
//! assembly, holds no mesh, and knows nothing about any particular law. A
//! caller supplies a closure mapping a total strain to a stress, and this
//! module finds the strain satisfying the deck's mixed boundary conditions.

#![allow(dead_code)] // Each test binary includes the whole module, uses part.

use outram_foam_basic_lib::primitives::SymmTensor;

// ── Interpolation, reproducing DEFI_FONCTION / DEFI_NAPPE ────────────────────

/// How a tabulated function behaves outside the range of its abscissae.
///
/// Mirrors code_aster's `PROL_GAUCHE` / `PROL_DROITE` keywords, which are set
/// per end and per function. Getting these wrong is silent: the run simply uses
/// the wrong property near the ends of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extrapolation {
    /// `PROL_* = "CONSTANT"` — hold the end value.
    Constant,
    /// `PROL_* = "LINEAIRE"` — continue the end segment's slope.
    Linear,
}

/// A piecewise-linear tabulated function, code_aster's `DEFI_FONCTION`.
///
/// Points must be supplied in strictly increasing order of abscissa; the
/// constructor panics otherwise, because an out-of-order table produces
/// plausible interpolated garbage rather than an error.
#[derive(Debug, Clone, PartialEq)]
pub struct PiecewiseLinear {
    points: Vec<(f64, f64)>,
    left: Extrapolation,
    right: Extrapolation,
}

impl PiecewiseLinear {
    /// Build from `(abscissa, value)` pairs and the two extrapolation rules.
    ///
    /// # Panics
    ///
    /// If fewer than two points are given, or the abscissae are not strictly
    /// increasing.
    #[must_use]
    pub fn new(points: &[(f64, f64)], left: Extrapolation, right: Extrapolation) -> Self {
        assert!(points.len() >= 2, "a table needs at least two points");
        assert!(
            points.windows(2).all(|w| w[1].0 > w[0].0),
            "table abscissae must be strictly increasing"
        );
        Self {
            points: points.to_vec(),
            left,
            right,
        }
    }

    /// Evaluate at `x`.
    #[must_use]
    pub fn at(&self, x: f64) -> f64 {
        let n = self.points.len();
        let (x0, y0) = self.points[0];
        let (xn, yn) = self.points[n - 1];

        if x <= x0 {
            return match self.left {
                Extrapolation::Constant => y0,
                Extrapolation::Linear => {
                    let (x1, y1) = self.points[1];
                    y0 + (y1 - y0) / (x1 - x0) * (x - x0)
                }
            };
        }
        if x >= xn {
            return match self.right {
                Extrapolation::Constant => yn,
                Extrapolation::Linear => {
                    let (xm, ym) = self.points[n - 2];
                    yn + (yn - ym) / (xn - xm) * (x - xn)
                }
            };
        }
        // Interior: locate the bracketing segment. Tables here are short, so a
        // linear scan is both fastest and obviously correct.
        let i = self
            .points
            .windows(2)
            .position(|w| x >= w[0].0 && x <= w[1].0)
            .expect("interior x must lie in some segment");
        let ((xa, ya), (xb, yb)) = (self.points[i], self.points[i + 1]);
        ya + (yb - ya) / (xb - xa) * (x - xa)
    }
}

/// A two-dimensional tabulated function, code_aster's `DEFI_NAPPE`.
///
/// One inner [`PiecewiseLinear`] per value of the outer parameter — for the
/// decks here, one curve per temperature. Evaluation interpolates *within* each
/// bracketing curve first and then *between* the two results, which is what
/// upstream does and is not the same as interpolating the tables' points.
#[derive(Debug, Clone, PartialEq)]
pub struct Nappe {
    curves: Vec<(f64, PiecewiseLinear)>,
    left: Extrapolation,
    right: Extrapolation,
}

impl Nappe {
    /// Build from `(outer parameter, curve)` pairs, in strictly increasing
    /// order of the outer parameter.
    ///
    /// # Panics
    ///
    /// If fewer than two curves are given or the parameters are not strictly
    /// increasing.
    #[must_use]
    pub fn new(
        curves: Vec<(f64, PiecewiseLinear)>,
        left: Extrapolation,
        right: Extrapolation,
    ) -> Self {
        assert!(curves.len() >= 2, "a nappe needs at least two curves");
        assert!(
            curves.windows(2).all(|w| w[1].0 > w[0].0),
            "nappe parameters must be strictly increasing"
        );
        Self {
            curves,
            left,
            right,
        }
    }

    /// Evaluate at outer parameter `p` and inner abscissa `x`.
    #[must_use]
    pub fn at(&self, p: f64, x: f64) -> f64 {
        let n = self.curves.len();
        let (p0, ref c0) = self.curves[0];
        let (pn, ref cn) = self.curves[n - 1];

        if p <= p0 {
            return match self.left {
                Extrapolation::Constant => c0.at(x),
                Extrapolation::Linear => {
                    let (p1, ref c1) = self.curves[1];
                    let (y0, y1) = (c0.at(x), c1.at(x));
                    y0 + (y1 - y0) / (p1 - p0) * (p - p0)
                }
            };
        }
        if p >= pn {
            return match self.right {
                Extrapolation::Constant => cn.at(x),
                Extrapolation::Linear => {
                    let (pm, ref cm) = self.curves[n - 2];
                    let (ym, yn) = (cm.at(x), cn.at(x));
                    yn + (yn - ym) / (pn - pm) * (p - pn)
                }
            };
        }
        let i = self
            .curves
            .windows(2)
            .position(|w| p >= w[0].0 && p <= w[1].0)
            .expect("interior p must lie between two curves");
        let (pa, ref ca) = self.curves[i];
        let (pb, ref cb) = self.curves[i + 1];
        let (ya, yb) = (ca.at(x), cb.at(x));
        ya + (yb - ya) / (pb - pa) * (p - pa)
    }
}

// ── Mixed strain/stress control ──────────────────────────────────────────────

/// What a deck prescribes for one tensor component.
///
/// A single-element deck almost never prescribes all six stresses. A uniaxial
/// relaxation test, for instance, imposes an axial *strain* through a boundary
/// displacement and leaves the lateral faces traction-free, so five components
/// are stress-prescribed at zero and one is strain-prescribed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Control {
    /// This strain component is imposed; the stress follows.
    Strain(f64),
    /// This stress component is imposed; the strain is solved for.
    Stress(f64),
}

/// The six components in `SymmTensor` order, as this module indexes them.
///
/// Fixed here so the driver and its callers cannot disagree about ordering —
/// a class of bug that produces a plausible wrong answer rather than an error.
fn components(t: SymmTensor) -> [f64; 6] {
    [t.xx, t.yy, t.zz, t.xy, t.xz, t.yz]
}

fn from_components(c: [f64; 6]) -> SymmTensor {
    SymmTensor::new(c[0], c[3], c[4], c[1], c[5], c[2])
}

/// Outcome of a mixed-control solve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixedSolution {
    /// The total strain satisfying the prescribed components.
    pub strain: SymmTensor,
    /// The stress it produced.
    pub stress: SymmTensor,
    /// Iterations used.
    pub iterations: usize,
    /// Worst residual at exit, in the units of whichever component was worst —
    /// stress \[MPa\] for stress-controlled components, strain \[-\] for
    /// strain-controlled ones.
    pub residual: f64,
}

/// Solve for the total strain satisfying a per-component mix of prescribed
/// strain and stress.
///
/// `response` maps a trial total strain to the stress the law returns for it.
/// It is called repeatedly, so it must be a *pure* function of the strain — any
/// internal state it advances must be restored, or the iteration converges to
/// the wrong answer while looking healthy.
///
/// # The iteration
///
/// Strain-controlled components are simply set and never moved. Stress-controlled
/// components are corrected by the elastic compliance applied to the stress
/// residual:
///
/// `Δε ← Δε + C⁻¹ : (σ_target - σ)`, restricted to those components.
///
/// Using the *elastic* compliance under-corrects whenever the material has
/// softened, which is what keeps a plain fixed point stable without a line
/// search. It converges linearly, and slowly when the elastoplastic tangent is
/// far below the elastic one — so `tolerance` should be set from what the
/// problem needs, not from what looks tidy.
///
/// # Panics
///
/// If the iteration does not converge within `max_iterations`. That is a test
/// failure, not a recoverable condition, and silently returning an
/// unconverged strain would corrupt every comparison downstream.
pub fn solve_mixed_control<F>(
    control: [Control; 6],
    young: f64,
    poisson: f64,
    initial_strain: SymmTensor,
    mut response: F,
    tolerance: f64,
    max_iterations: usize,
) -> MixedSolution
where
    F: FnMut(SymmTensor) -> SymmTensor,
{
    let mut strain = components(initial_strain);
    for (i, c) in control.iter().enumerate() {
        if let Control::Strain(value) = *c {
            strain[i] = value;
        }
    }

    for iteration in 1..=max_iterations {
        let stress = response(from_components(strain));
        let s = components(stress);

        // Residual only on the stress-controlled components; the
        // strain-controlled ones are exact by construction.
        let mut residual: f64 = 0.0;
        let mut wanted = [0.0_f64; 6];
        for (i, c) in control.iter().enumerate() {
            wanted[i] = match *c {
                Control::Stress(target) => {
                    residual = residual.max((target - s[i]).abs());
                    target - s[i]
                }
                Control::Strain(_) => 0.0,
            };
        }

        if residual < tolerance {
            return MixedSolution {
                strain: from_components(strain),
                stress,
                iterations: iteration,
                residual,
            };
        }

        // Elastic compliance applied to the stress residual. Written out per
        // component rather than as a tensor op so the shear factor is explicit:
        // for the off-diagonal terms the compliance is (1+nu)/E with no trace
        // subtraction.
        let trace = wanted[0] + wanted[1] + wanted[2];
        let a = (1.0 + poisson) / young;
        let b = poisson * trace / young;
        let correction = [
            a * wanted[0] - b,
            a * wanted[1] - b,
            a * wanted[2] - b,
            a * wanted[3],
            a * wanted[4],
            a * wanted[5],
        ];
        for (i, c) in control.iter().enumerate() {
            if matches!(*c, Control::Stress(_)) {
                strain[i] += correction[i];
            }
        }
    }

    panic!("mixed-control solve did not converge in {max_iterations} iterations");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Methodology.** A `DEFI_FONCTION` must interpolate linearly between
    /// tabulated points and honour `PROL_GAUCHE`/`PROL_DROITE` at each end
    /// independently. Build the table `(900, 12.2) (1000, 10.8) (1025, 10.45)`
    /// — `N_T` from `ssnv126a` — and check the node values, an interior
    /// midpoint against the hand-computed segment average, and both
    /// extrapolation modes. Tolerance 1e-12 relative.
    ///
    /// **Results, measured 2026-08-05.** Node values returned exactly:
    /// `at(900) = 12.2`, `at(1000) = 10.8`, `at(1025) = 10.45`. The midpoint
    /// `at(950) = 11.5`, which is `(12.2 + 10.8)/2` exactly. Constant
    /// extrapolation held the ends: `at(800) = 12.2` and `at(1100) = 10.45`.
    /// Linear extrapolation continued the end slopes:
    /// `at(800) = 13.599999999999998` and `at(1100) = 9.399999999999995` —
    /// both a few ulp off the exact `13.6` and `9.4`, which is ordinary
    /// floating-point error in the slope product, not a table defect. The
    /// assertions are written to 1e-12 for that reason.
    #[test]
    fn piecewise_linear_reproduces_defi_fonction() {
        let table = [(900.0, 12.2), (1000.0, 10.8), (1025.0, 10.45)];
        let held = PiecewiseLinear::new(&table, Extrapolation::Constant, Extrapolation::Constant);
        let sloped = PiecewiseLinear::new(&table, Extrapolation::Linear, Extrapolation::Linear);

        println!(
            "nodes: {} {} {}",
            held.at(900.0),
            held.at(1000.0),
            held.at(1025.0)
        );
        println!("midpoint at(950) = {}", held.at(950.0));
        println!(
            "constant: at(800) = {}, at(1100) = {}",
            held.at(800.0),
            held.at(1100.0)
        );
        println!(
            "linear:   at(800) = {}, at(1100) = {}",
            sloped.at(800.0),
            sloped.at(1100.0)
        );

        assert!((held.at(900.0) - 12.2).abs() < 1e-12);
        assert!((held.at(1000.0) - 10.8).abs() < 1e-12);
        assert!((held.at(1025.0) - 10.45).abs() < 1e-12);
        assert!((held.at(950.0) - 11.5).abs() < 1e-12);
        assert!((held.at(800.0) - 12.2).abs() < 1e-12);
        assert!((held.at(1100.0) - 10.45).abs() < 1e-12);
        assert!((sloped.at(800.0) - 13.6).abs() < 1e-12);
        assert!((sloped.at(1100.0) - 9.4).abs() < 1e-12);
    }

    /// **Methodology.** A `DEFI_NAPPE` must interpolate *within* each bracketing
    /// curve and then *between* the two results. Build the `KD_T` nappe from
    /// `ssnv126a` — temperatures `(900, 1000, 1025, 1050)`, each carrying a
    /// curve in `X` — and check that evaluating on a tabulated temperature
    /// reproduces that curve exactly, and that a temperature midway between two
    /// curves gives the average of their values at the same `X`. Tolerance
    /// 1e-12 relative.
    ///
    /// **Results, measured 2026-08-05.** On the tabulated temperature 1000 K at
    /// `X = 100`, the nappe returned `15` — exactly that curve's own value.
    /// Midway at 1012.5 K it returned `15.01815`, against the hand average
    /// `(15 + 15.0363)/2 = 15.01815`, agreeing to 1e-12. Evaluating off the
    /// curves' `X` range exercised the inner `LINEAIRE` extrapolation:
    /// `at(1000, 300) = 16`.
    ///
    /// That last figure was **predicted wrong** on first writing, as `15.5`,
    /// by reading off the curve's value at its last tabulated point `X = 200`
    /// instead of extrapolating past it. The last segment rises `0.5` over
    /// `100` units of `X`, so continuing that slope another `100` units gives
    /// `15.5 + 0.5 = 16`. The code was right and the prediction was not; the
    /// figure here is what the test printed.
    #[test]
    fn nappe_reproduces_defi_nappe() {
        let curve = |a: f64| {
            PiecewiseLinear::new(
                &[(0.01, a), (100.0, a + 0.5), (200.0, a + 1.0)],
                Extrapolation::Linear,
                Extrapolation::Linear,
            )
        };
        let nappe = Nappe::new(
            vec![
                (900.0, curve(14.355)),
                (1000.0, curve(14.5)),
                (1025.0, curve(14.5363)),
                (1050.0, curve(14.5725)),
            ],
            Extrapolation::Linear,
            Extrapolation::Linear,
        );

        let on_curve = nappe.at(1000.0, 100.0);
        let midway = nappe.at(1012.5, 100.0);
        let expected_mid = (15.0 + 15.0363) / 2.0;
        let beyond_x = nappe.at(1000.0, 300.0);
        println!("at(1000, 100) = {on_curve}");
        println!("at(1012.5, 100) = {midway}, hand average = {expected_mid}");
        println!("at(1000, 300) = {beyond_x}");

        assert!((on_curve - 15.0).abs() < 1e-12);
        assert!((midway - expected_mid).abs() < 1e-12);
        assert!((beyond_x - 16.0).abs() < 1e-12);
    }

    /// **Methodology.** The mixed-control solve must be exact on a case with a
    /// known closed form. Drive a purely **elastic** response
    /// `σ = λ tr(ε) I + 2μ ε` with `E = 150000` MPa, `ν = 0.3`, prescribing
    /// `ε_zz = 3.3333e-3` and requiring every other stress component to vanish
    /// — the uniaxial relaxation configuration of `ssnv126a`. The closed form
    /// is then `σ_zz = E ε_zz` and `ε_xx = ε_yy = -ν ε_zz`. Pass criterion:
    /// agreement to 1e-9 relative, and every stress-controlled component below
    /// the driver tolerance of 1e-10 MPa.
    ///
    /// **Results, measured 2026-08-05.** Converged in **29 iterations** to a
    /// residual of `3.629452294262592e-11` MPa. Measured
    /// `σ_zz = 500.000000000022` MPa against the closed form
    /// `E ε_zz = 500.00000000000006`, and `ε_xx = ε_yy = -0.001000000000`
    /// against `-ν ε_zz = -0.001`. The traction-free components came out at
    /// `3.63e-11` MPa on `xx` and `yy` and exactly zero on the three shears —
    /// the shears never move because nothing in this loading couples them.
    ///
    /// The iteration count is worth recording: **29 iterations for a purely
    /// linear problem**, because the fixed point corrects with the compliance
    /// rather than solving the coupled system. Each lateral correction changes
    /// the trace, which changes the other lateral stress, so the two chase each
    /// other down at a rate set by `ν`. It is cheap here, but it is why the
    /// tolerance for a softening material must be chosen from the problem
    /// rather than set arbitrarily tight.
    #[test]
    fn mixed_control_recovers_uniaxial_elasticity() {
        let young = 150_000.0;
        let poisson = 0.3;
        let axial = 0.1 / 30.0;

        let lame = young * poisson / ((1.0 + poisson) * (1.0 - 2.0 * poisson));
        let two_mu = young / (1.0 + poisson);
        let elastic = |e: SymmTensor| {
            let trace = e.xx + e.yy + e.zz;
            SymmTensor::new(
                lame * trace + two_mu * e.xx,
                two_mu * e.xy,
                two_mu * e.xz,
                lame * trace + two_mu * e.yy,
                two_mu * e.yz,
                lame * trace + two_mu * e.zz,
            )
        };

        let control = [
            Control::Stress(0.0),
            Control::Stress(0.0),
            Control::Strain(axial),
            Control::Stress(0.0),
            Control::Stress(0.0),
            Control::Stress(0.0),
        ];
        let solution = solve_mixed_control(
            control,
            young,
            poisson,
            SymmTensor::ZERO,
            elastic,
            1.0e-10,
            10_000,
        );

        let expected_axial_stress = young * axial;
        let expected_lateral = -poisson * axial;
        println!(
            "iterations = {}, residual = {:e} MPa",
            solution.iterations, solution.residual
        );
        println!(
            "sigma_zz = {:.12} (closed form {expected_axial_stress})",
            solution.stress.zz
        );
        println!(
            "eps_xx = {:.12}, eps_yy = {:.12} (closed form {expected_lateral})",
            solution.strain.xx, solution.strain.yy
        );
        println!(
            "traction-free components: {:e} {:e} {:e} {:e} {:e}",
            solution.stress.xx,
            solution.stress.yy,
            solution.stress.xy,
            solution.stress.xz,
            solution.stress.yz
        );

        assert!(
            ((solution.stress.zz - expected_axial_stress) / expected_axial_stress).abs() < 1e-9
        );
        assert!(((solution.strain.xx - expected_lateral) / expected_lateral).abs() < 1e-9);
        assert!(((solution.strain.yy - expected_lateral) / expected_lateral).abs() < 1e-9);
        assert!(
            solution.strain.zz == axial,
            "prescribed component must be exact"
        );
    }
}
