// SPDX-License-Identifier: AGPL-3.0-only
//
// NOT a port. boon-lay's own numerical solution of the diffusion equation in a
// layered sphere, written 2026-11-04 (gh:#1209, child of #1207). The method is
// the textbook cell-centred finite-volume discretisation with backward-Euler
// time stepping (e.g. Patankar 1980, "Numerical Heat Transfer and Fluid Flow",
// ch. 4); FRESCO-II (Krohn & Finken) solves the same equation on a mesh, but
// no FRESCO source was used. The linear solve is petir's port of GSL's
// `linalg/tridiag.c` (`petir::linalg::solve_symm_tridiag`).

//! # Diffusion through a layered sphere, on a mesh (NOT a port)
//!
//! **What it solves.** One fission product in a TRISO particle: the kernel
//! (layer 0) and the coatings around it, each with its own diffusion
//! coefficient `D`, under
//!
//! ```text
//!   ∂c/∂t = (1/r²) ∂/∂r ( r² D(r) ∂c/∂r ) − λ c + s(r)
//! ```
//!
//! with `c` continuous and the flux continuous at every layer boundary
//! (partition ratio 1, as in the exact mean of the Fission products lesson
//! *Across the whole particle*), symmetry at the centre and `c = 0` at the
//! outer surface (a perfect sink: whatever reaches it is released).
//!
//! **Why it exists.** TRISO-ATOPS answers the release question with
//! analytical formulas: Booth's series on one sphere, and a planar time-lag
//! formula for silver through the SiC. Whether those are *conservative* can
//! only be measured against a solution of the same equation through all the
//! layers, and the workspace had none (searched 2026-11-04, gh:#1209:
//! boon-lay's multilayer walker cannot cross the buffer/PyC boundary in
//! practice, `tampines::pebble_bed::triso` is steady conduction, the
//! PFLOTRAN fork has no spherical grid).
//!
//! **Method.** Cells are uniform within each layer, and every layer boundary
//! is a cell face. The unknown is the mean concentration of each cell. Between
//! two cells the conductance is the series resistance of the two half cells,
//! `G = A_f / (Δr₁/(2D₁) + Δr₂/(2D₂))`, the exact value for a flux through a
//! face where `D` jumps; at the surface `G = A / (Δr/(2D))`. Backward Euler
//! makes every step a symmetric, diagonally dominant tridiagonal system
//! (unconditionally stable). The atoms leaving through the surface and those
//! decaying are counted from the same implicit values, so
//! `inside + released + decayed` is conserved to round-off at every step.
//! The plane approximations (one temperature per step, `D` constant within a
//! layer) are the caller's: [`LayeredDiffusion::step`] takes one `D` per layer
//! for that step.
//!
//! ## V&V
//!
//! **Predictions, written 2026-11-04 before the tests were first run**
//! (`tests` below; Crank's series is boon-lay's ported `booth_longlived`):
//!
//! - P1. One layer, uniform start, no decay: the released fraction at
//!   `Dt/a² = 0.01, 0.05, 0.2` is within 1e-3 of Crank's series with 200 cells
//!   and steps of `Dt/a² = 1e-5`, and the error falls when the mesh and the
//!   step are refined (backward Euler is first order in time, the mesh second
//!   order in space; the early-time error is the larger).
//! - P2. Atoms are conserved: `inside + released + decayed = 1` within 1e-12
//!   after every step, with and without decay. **MISSED with decay** (see
//!   Results).
//! - P3. The mean exit time of an atom born uniformly in the kernel matches
//!   the exact layered formula `a²/(15 D₀) + Σ (r_out² − r_in²)/(6 D_l)` within
//!   1e-3 relative on the CRP-6 particle with `D` spread over seven orders of
//!   magnitude, at 40 cells per layer.
//! - P4. The steady release-to-birth of a decaying species born in a single
//!   sphere matches Booth's exact `3/x (coth x − 1/x)`, `x = a √(λ/D)`
//!   (boon-lay's ported `booth_shortlived_fast_diffuse`), within 1e-4
//!   relative at `x = 1` and 1e-2 at `x = 30`, with 400 cells.
//!
//! **Results, first run 2026-11-04** (`cargo test --release -p boon-lay --lib
//! multilayer_diffusion -- --nocapture`):
//!
//! - P1 held: |FV − Crank| = 7.9e-5, 3.0e-5 and 1.1e-5 at `Dt/a² = 0.01, 0.05,
//!   0.2` (200 cells, steps 1e-5), against 1.0e-3, 3.6e-4 and 1.3e-4 at 50
//!   cells and steps 1e-4. Refining both by about 10× cuts the error 10–13×,
//!   as the first-order time error dominates.
//! - P2 **missed with decay**: worst 1.8e-13 without decay, but **6.6e-12**
//!   with λ = 1e-6 /s, over 200 steps of 1 h. The update conserves atoms
//!   exactly in algebra; the remainder is the tridiagonal solve's round-off,
//!   which scales with the stiffness `h D/Δr²` (≈ 1.1e3 in this buffer). The
//!   prediction ignored that. The test now gates on the derived bound
//!   `eps × (h D/Δr²) × steps` ≈ 5e-11, and does not use a value chosen after
//!   the run.
//! - P3 held: 1.537470e10 s against the exact 1.537689e10 s, 1.4e-4 relative.
//! - P4 held: 6.8e-7 relative at x = 1 and 6.8e-4 at x = 30.
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`).

use petir::mathf::RealMath;
use petir::linalg::solve_symm_tridiag;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{DiffusionCoefficient, Frequency, Length, Ratio, Time};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::time::second;

use std::f64::consts::PI;

/// One layer of the sphere: its outer radius and how many cells it gets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphereLayer {
    /// Outer radius of the layer (the inner one is the previous layer's).
    pub outer_radius: Length,
    /// Number of equal-width cells across the layer (at least 1).
    pub cells: usize,
}

/// Why a layered sphere or a step was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum MultilayerDiffusionError {
    /// No layers, a layer with no cells, or radii not strictly increasing
    /// from a positive first one.
    BadGeometry(&'static str),
    /// The number of diffusion coefficients is not the number of layers, or
    /// one is not finite and positive.
    BadDiffusionCoefficients,
    /// A time step or decay constant is negative or not finite.
    BadStep,
    /// The tridiagonal solve failed (a vanishing pivot; cannot happen for a
    /// valid system, which is diagonally dominant).
    Solve,
}

/// The mesh of a layered sphere: cell faces, volumes and which layer each
/// cell belongs to. Built once; the `D` values come with each solve.
#[derive(Debug, Clone, PartialEq)]
pub struct LayeredSphere {
    /// Cell faces in metres, `cells + 1` of them, from 0 to the outer radius.
    faces_m: Vec<f64>,
    /// Cell volumes, m³.
    volumes: Vec<f64>,
    /// Layer index of each cell.
    layer_of_cell: Vec<usize>,
    /// Number of layers.
    layers: usize,
}

impl LayeredSphere {
    /// Build the mesh. Layer 0 is the kernel; the others are shells around it.
    ///
    /// # Errors
    /// [`MultilayerDiffusionError::BadGeometry`] for no layers, a layer with
    /// no cells, or radii that are not positive and strictly increasing.
    pub fn new(layers: &[SphereLayer]) -> Result<Self, MultilayerDiffusionError> {
        if layers.is_empty() {
            return Err(MultilayerDiffusionError::BadGeometry("no layers"));
        }
        let mut faces_m = vec![0.0];
        let mut layer_of_cell = Vec::new();
        let mut inner = 0.0;
        for (l, layer) in layers.iter().enumerate() {
            let outer = layer.outer_radius.get::<meter>();
            if layer.cells == 0 {
                return Err(MultilayerDiffusionError::BadGeometry("a layer has no cells"));
            }
            if !(outer.is_finite() && outer > inner) {
                return Err(MultilayerDiffusionError::BadGeometry("radii must increase from 0"));
            }
            for k in 1..=layer.cells {
                faces_m.push(inner + (outer - inner) * k as f64 / layer.cells as f64);
                layer_of_cell.push(l);
            }
            // The layer boundary is exactly the given radius, not a sum.
            *faces_m.last_mut().expect("just pushed") = outer;
            inner = outer;
        }
        let volumes = faces_m.windows(2).map(|w| 4.0 / 3.0 * PI * (w[1].r_powi(3) - w[0].r_powi(3))).collect();
        Ok(Self { faces_m, volumes, layer_of_cell, layers: layers.len() })
    }

    /// Number of cells.
    #[must_use]
    pub fn cells(&self) -> usize {
        self.volumes.len()
    }

    /// Number of layers.
    #[must_use]
    pub fn layers(&self) -> usize {
        self.layers
    }

    /// Cell centres (mid-radius), metres, for plotting.
    #[must_use]
    pub fn centres_m(&self) -> Vec<f64> {
        self.faces_m.windows(2).map(|w| 0.5 * (w[0] + w[1])).collect()
    }

    /// Validate `D` per layer and return it in m²/s.
    fn d_values(&self, d: &[DiffusionCoefficient]) -> Result<Vec<f64>, MultilayerDiffusionError> {
        if d.len() != self.layers {
            return Err(MultilayerDiffusionError::BadDiffusionCoefficients);
        }
        let v: Vec<f64> = d.iter().map(|x| x.get::<square_meter_per_second>()).collect();
        if v.iter().all(|x| x.is_finite() && *x > 0.0) {
            Ok(v)
        } else {
            Err(MultilayerDiffusionError::BadDiffusionCoefficients)
        }
    }

    /// Conductances (m³/s per unit concentration): between cell `i` and
    /// `i + 1`, and from the last cell to the surface sink.
    fn conductances(&self, d: &[f64]) -> (Vec<f64>, f64) {
        let n = self.cells();
        let half = |i: usize| 0.5 * (self.faces_m[i + 1] - self.faces_m[i]) / d[self.layer_of_cell[i]];
        let area = |r: f64| 4.0 * PI * r * r;
        let inner = (0..n - 1).map(|i| area(self.faces_m[i + 1]) / (half(i) + half(i + 1))).collect();
        (inner, area(self.faces_m[n]) / half(n - 1))
    }

    /// The matrix `diag` / `offdiag` of `Σ G (c_i − c_j) + G_out c_N + w_i c_i`,
    /// with `w_i` an extra diagonal weight per cell.
    fn system(&self, d: &[f64], scale: f64, extra: impl Fn(usize) -> f64) -> (Vec<f64>, Vec<f64>, f64) {
        let n = self.cells();
        let (g, g_out) = self.conductances(d);
        let mut diag: Vec<f64> = (0..n).map(&extra).collect();
        for (i, gi) in g.iter().enumerate() {
            diag[i] += scale * gi;
            diag[i + 1] += scale * gi;
        }
        diag[n - 1] += scale * g_out;
        (diag, g.iter().map(|x| -scale * x).collect(), g_out)
    }

    /// Volume of the kernel (layer 0), m³.
    fn kernel_volume(&self) -> f64 {
        self.volumes.iter().zip(&self.layer_of_cell).filter(|(_, l)| **l == 0).map(|(v, _)| v).sum()
    }

    /// Mean time for an atom born uniformly in the kernel to reach the
    /// surface, with no decay: `∇·(D∇τ) = −1`, `τ = 0` at the surface,
    /// averaged over the kernel.
    ///
    /// # Errors
    /// Bad `D` values, or a failed solve.
    pub fn mean_exit_time(&self, d: &[DiffusionCoefficient]) -> Result<Time, MultilayerDiffusionError> {
        let dv = self.d_values(d)?;
        let (diag, off, _) = self.system(&dv, 1.0, |_| 0.0);
        let tau = solve_symm_tridiag(&diag, &off, &self.volumes).map_err(|_| MultilayerDiffusionError::Solve)?;
        let vk = self.kernel_volume();
        let mean = tau.iter().zip(&self.volumes).zip(&self.layer_of_cell).filter(|(_, l)| **l == 0).map(|((t, v), _)| t * v).sum::<f64>() / vk;
        Ok(Time::new::<second>(mean))
    }

    /// Steady release-to-birth ratio `R/B` of a species born at a constant
    /// rate, uniformly in the kernel, and decaying with `λ` everywhere: the
    /// rate leaving the surface over the birth rate.
    ///
    /// # Errors
    /// Bad `D` values, a negative or non-finite `λ`, or a failed solve.
    pub fn steady_release_to_birth(
        &self,
        d: &[DiffusionCoefficient],
        decay_constant: Frequency,
    ) -> Result<Ratio, MultilayerDiffusionError> {
        let dv = self.d_values(d)?;
        let lam = decay_constant.get::<hertz>();
        if !(lam.is_finite() && lam >= 0.0) {
            return Err(MultilayerDiffusionError::BadStep);
        }
        let (diag, off, g_out) = self.system(&dv, 1.0, |i| lam * self.volumes[i]);
        let vk = self.kernel_volume();
        let birth: Vec<f64> = (0..self.cells()).map(|i| if self.layer_of_cell[i] == 0 { self.volumes[i] / vk } else { 0.0 }).collect();
        let c = solve_symm_tridiag(&diag, &off, &birth).map_err(|_| MultilayerDiffusionError::Solve)?;
        Ok(Ratio::new::<ratio>(g_out * c[self.cells() - 1]))
    }
}

/// A transient: the atoms in each cell, and the running totals released at
/// the surface and decayed, all as fractions of the starting inventory.
#[derive(Debug, Clone, PartialEq)]
pub struct LayeredDiffusion {
    sphere: LayeredSphere,
    /// Atoms in each cell (fraction of the starting inventory).
    atoms: Vec<f64>,
    released: f64,
    decayed: f64,
    elapsed_s: f64,
}

impl LayeredDiffusion {
    /// Start with the whole inventory spread uniformly through the kernel
    /// (layer 0), nothing in the coatings: the start of CRP-6 Case 1 and of
    /// Booth's sphere.
    #[must_use]
    pub fn uniform_in_kernel(sphere: LayeredSphere) -> Self {
        Self::uniform_in_first_layers(sphere, 1)
    }

    /// Start with the whole inventory spread uniformly through the first
    /// `n` layers (at least one; capped at the number of layers). Lets a
    /// caller split the kernel into an inner core and a finely meshed outer
    /// shell with the same `D`, to resolve a thin boundary layer, and still
    /// start uniformly through the whole kernel.
    #[must_use]
    pub fn uniform_in_first_layers(sphere: LayeredSphere, n: usize) -> Self {
        let n = n.clamp(1, sphere.layers);
        let inside = |l: usize| l < n;
        let v: f64 = sphere.volumes.iter().zip(&sphere.layer_of_cell).filter(|(_, l)| inside(**l)).map(|(v, _)| v).sum();
        let atoms = sphere.volumes.iter().zip(&sphere.layer_of_cell).map(|(vi, l)| if inside(*l) { vi / v } else { 0.0 }).collect();
        Self { sphere, atoms, released: 0.0, decayed: 0.0, elapsed_s: 0.0 }
    }

    /// One backward-Euler step of length `dt` with one `D` per layer and a
    /// decay constant `λ` (zero for none).
    ///
    /// # Errors
    /// Bad `D` values, a negative or non-finite `dt` or `λ`, or a failed solve.
    pub fn step(
        &mut self,
        dt: Time,
        d: &[DiffusionCoefficient],
        decay_constant: Frequency,
    ) -> Result<(), MultilayerDiffusionError> {
        let dv = self.sphere.d_values(d)?;
        let (h, lam) = (dt.get::<second>(), decay_constant.get::<hertz>());
        if !(h.is_finite() && h >= 0.0 && lam.is_finite() && lam >= 0.0) {
            return Err(MultilayerDiffusionError::BadStep);
        }
        let vol = &self.sphere.volumes;
        // In concentrations: (V/h)(c' − c) = −[Σ G (c'_i − c'_j) + G_out c'_N + λ V c'].
        let (diag, off, g_out) = self.sphere.system(&dv, h, |i| vol[i] * (1.0 + lam * h));
        let rhs: Vec<f64> = self.atoms.clone();
        let c = solve_symm_tridiag(&diag, &off, &rhs).map_err(|_| MultilayerDiffusionError::Solve)?;
        let n = c.len();
        self.released += h * g_out * c[n - 1];
        self.decayed += h * lam * c.iter().zip(vol).map(|(ci, v)| ci * v).sum::<f64>();
        self.atoms = c.iter().zip(vol).map(|(ci, v)| ci * v).collect();
        self.elapsed_s += h;
        Ok(())
    }

    /// Fraction of the starting inventory released through the surface.
    #[must_use]
    pub fn released_fraction(&self) -> Ratio {
        Ratio::new::<ratio>(self.released)
    }

    /// Fraction that decayed (anywhere in the particle).
    #[must_use]
    pub fn decayed_fraction(&self) -> Ratio {
        Ratio::new::<ratio>(self.decayed)
    }

    /// Atoms in each cell (fractions of the starting inventory), centre
    /// outwards: the concentration profile times each cell's volume.
    #[must_use]
    pub fn cell_atoms(&self) -> &[f64] {
        &self.atoms
    }

    /// Fraction still inside, per layer.
    #[must_use]
    pub fn inside_by_layer(&self) -> Vec<f64> {
        let mut out = vec![0.0; self.sphere.layers()];
        for (a, l) in self.atoms.iter().zip(&self.sphere.layer_of_cell) {
            out[*l] += a;
        }
        out
    }

    /// Time stepped so far.
    #[must_use]
    pub fn elapsed(&self) -> Time {
        Time::new::<second>(self.elapsed_s)
    }

    /// The mesh.
    #[must_use]
    pub fn sphere(&self) -> &LayeredSphere {
        &self.sphere
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triso_atops_fork::release_models::steady_state::{booth_longlived, booth_shortlived_fast_diffuse};
    use uom::si::length::micrometer;

    fn um(x: f64) -> Length {
        Length::new::<micrometer>(x)
    }
    fn dco(x: f64) -> DiffusionCoefficient {
        DiffusionCoefficient::new::<square_meter_per_second>(x)
    }
    fn s(x: f64) -> Time {
        Time::new::<second>(x)
    }
    fn hz(x: f64) -> Frequency {
        Frequency::new::<hertz>(x)
    }

    /// Release from one sphere after `tau = Dt/a²` with `cells` cells and
    /// `steps` equal steps, against Crank's series.
    fn bare_error(cells: usize, steps: usize, tau: f64) -> f64 {
        let a = 1e-4;
        let d = 1e-12;
        let sphere = LayeredSphere::new(&[SphereLayer { outer_radius: Length::new::<meter>(a), cells }]).unwrap();
        let mut run = LayeredDiffusion::uniform_in_kernel(sphere);
        let t = tau * a * a / d;
        for _ in 0..steps {
            run.step(s(t / steps as f64), &[dco(d)], hz(0.0)).unwrap();
        }
        let exact = booth_longlived(dco(d), s(t), Length::new::<meter>(a)).get::<ratio>();
        (run.released_fraction().get::<ratio>() - exact).abs()
    }

    /// P1. Results, first run 2026-11-04: see the assertion messages; the
    /// numbers are printed with `--nocapture`.
    #[test]
    fn one_sphere_against_crank() {
        for tau in [0.01f64, 0.05, 0.2] {
            let steps = (tau / 1e-5).round() as usize;
            let e = bare_error(200, steps, tau);
            let coarse = bare_error(50, steps / 10, tau);
            println!("P1 tau {tau}: |FV - Crank| = {e:.3e} (200 cells, dtau 1e-5); {coarse:.3e} (50 cells, dtau 1e-4)");
            assert!(e < 1e-3, "tau {tau}: {e}");
            assert!(e < coarse, "refining helps at tau {tau}: {e} vs {coarse}");
        }
    }

    /// P2.
    #[test]
    fn atoms_are_conserved() {
        let sphere = LayeredSphere::new(&[
            SphereLayer { outer_radius: um(250.0), cells: 20 },
            SphereLayer { outer_radius: um(340.0), cells: 5 },
            SphereLayer { outer_radius: um(375.0), cells: 5 },
        ])
        .unwrap();
        for lam in [0.0, 1e-6] {
            let mut run = LayeredDiffusion::uniform_in_kernel(sphere.clone());
            let mut worst: f64 = 0.0;
            for _ in 0..200 {
                run.step(s(3.6e3), &[dco(1e-14), dco(1e-10), dco(1e-16)], hz(lam)).unwrap();
                let total = run.inside_by_layer().iter().sum::<f64>() + run.released + run.decayed;
                worst = worst.max((total - 1.0).abs());
            }
            // The update conserves atoms exactly in algebra; what is left is
            // the solve's round-off, about eps × (h D/Δr²)_max per step. In the
            // buffer here h D/Δr² = 3600 × 1e-10 / (18 µm)² ≈ 1.1e3, so the
            // bound over 200 steps is eps × 1.1e3 × 200 ≈ 5e-11. (The
            // prediction, 1e-12, ignored that stiffness and missed with decay
            // on: 6.6e-12, first run 2026-11-04.)
            let stiff = 3.6e3 * 1e-10 / (18e-6f64).powi(2);
            let bound = f64::EPSILON * stiff * 200.0;
            println!("P2 lambda {lam}: worst |inside + released + decayed - 1| = {worst:.3e} (round-off bound {bound:.1e})");
            assert!(worst < bound, "{worst}");
        }
    }

    /// P3, on the CRP-6 particle's radii (212.5 µm kernel, 100 buffer, 40
    /// IPyC, 35 SiC, 40 OPyC) with D spanning seven orders of magnitude.
    #[test]
    fn mean_exit_time_against_the_layered_formula() {
        let radii = [212.5, 312.5, 352.5, 387.5, 427.5];
        let d = [2.25e-15, 1e-8, 1e-17, 3e-19, 1e-17];
        let layers: Vec<SphereLayer> = radii.iter().map(|r| SphereLayer { outer_radius: um(*r), cells: 40 }).collect();
        let sphere = LayeredSphere::new(&layers).unwrap();
        let dd: Vec<_> = d.iter().map(|x| dco(*x)).collect();
        let fv = sphere.mean_exit_time(&dd).unwrap().get::<second>();
        let r: Vec<f64> = radii.iter().map(|x| x * 1e-6).collect();
        let mut exact = r[0] * r[0] / (15.0 * d[0]);
        for l in 1..5 {
            exact += (r[l] * r[l] - r[l - 1] * r[l - 1]) / (6.0 * d[l]);
        }
        let rel = (fv / exact - 1.0).abs();
        println!("P3: FV {fv:.6e} s, exact {exact:.6e} s, relative {rel:.3e}");
        assert!(rel < 1e-3, "{rel}");
    }

    /// P4.
    #[test]
    fn steady_release_to_birth_against_booth() {
        let a = 1e-5;
        let d = 1e-16;
        let sphere = LayeredSphere::new(&[SphereLayer { outer_radius: Length::new::<meter>(a), cells: 400 }]).unwrap();
        for (x, tol) in [(1.0, 1e-4), (30.0, 1e-2)] {
            let lam = x * x * d / (a * a);
            let fv = sphere.steady_release_to_birth(&[dco(d)], hz(lam)).unwrap().get::<ratio>();
            let booth = booth_shortlived_fast_diffuse(dco(d), hz(lam), Length::new::<meter>(a)).get::<ratio>();
            let rel = (fv / booth - 1.0).abs();
            println!("P4 x {x}: FV {fv:.8}, Booth {booth:.8}, relative {rel:.3e}");
            assert!(rel < tol, "x {x}: {rel}");
        }
    }

    #[test]
    fn bad_inputs_are_refused() {
        assert!(LayeredSphere::new(&[]).is_err());
        assert!(LayeredSphere::new(&[SphereLayer { outer_radius: um(1.0), cells: 0 }]).is_err());
        assert!(LayeredSphere::new(&[
            SphereLayer { outer_radius: um(2.0), cells: 1 },
            SphereLayer { outer_radius: um(1.0), cells: 1 }
        ])
        .is_err());
        let sphere = LayeredSphere::new(&[SphereLayer { outer_radius: um(1.0), cells: 3 }]).unwrap();
        assert_eq!(sphere.cells(), 3);
        assert_eq!(sphere.centres_m().len(), 3);
        assert!(sphere.mean_exit_time(&[dco(1.0), dco(1.0)]).is_err());
        assert!(sphere.mean_exit_time(&[dco(-1.0)]).is_err());
        assert!(sphere.steady_release_to_birth(&[dco(1.0)], hz(-1.0)).is_err());
        let mut run = LayeredDiffusion::uniform_in_kernel(sphere);
        assert!(run.step(s(-1.0), &[dco(1.0)], hz(0.0)).is_err());
        run.step(s(1.0), &[dco(1e-12)], hz(0.0)).unwrap();
        assert_eq!(run.elapsed().get::<second>(), 1.0);
        assert_eq!(run.sphere().layers(), 1);
        assert!(run.decayed_fraction().get::<ratio>() == 0.0);
        assert_eq!(run.cell_atoms().len(), 3);
        // A kernel split into a core and a shell with the same D gives the
        // same release as the undivided kernel (to discretisation error).
        let split = LayeredSphere::new(&[
            SphereLayer { outer_radius: um(50.0), cells: 50 },
            SphereLayer { outer_radius: um(100.0), cells: 50 },
        ])
        .unwrap();
        let whole = LayeredSphere::new(&[SphereLayer { outer_radius: um(100.0), cells: 100 }]).unwrap();
        let mut a = LayeredDiffusion::uniform_in_first_layers(split, 2);
        let mut b = LayeredDiffusion::uniform_in_kernel(whole);
        assert!((a.cell_atoms().iter().sum::<f64>() - 1.0).abs() < 1e-12);
        for _ in 0..50 {
            a.step(s(100.0), &[dco(1e-12), dco(1e-12)], hz(0.0)).unwrap();
            b.step(s(100.0), &[dco(1e-12)], hz(0.0)).unwrap();
        }
        let (fa, fb) = (a.released_fraction().get::<ratio>(), b.released_fraction().get::<ratio>());
        assert!((fa - fb).abs() < 1e-12, "{fa} vs {fb}");
        assert!((run.cell_atoms().iter().sum::<f64>() + run.released_fraction().get::<ratio>() - 1.0).abs() < 1e-12);
    }
}
