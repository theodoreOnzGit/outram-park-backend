// SPDX-License-Identifier: AGPL-3.0-only
//! Steady heat conduction through the layers, Eqs (2.1)–(2.2) of the BISON
//! Theory Manual with `∂T/∂t = 0`:
//!
//! ```text
//! (1/r²) d/dr (k r² dT/dr) + q''' = 0,     q''' = e_f Ḟ   (W/m³)
//! ```
//!
//! with `T(b) = T_s` on the outer surface and zero flux on the innermost
//! surface (the centre, or an adiabatic inner surface of a shell). Layers
//! are in perfect thermal contact: the manual's gap conductance (Section
//! 13.1) is not implemented.
//!
//! `k` is a constant or an `outram-park-fork-offbeat` correlation `k(T)`
//! (for example PARFUME SiC, `k = 17885/T + 2`). With any `k(T)` the problem
//! is nonlinear and is solved by Picard iteration: `k` evaluated at each
//! Gauss point's temperature from the last iterate, until the largest nodal
//! change is below 1e-10 of the largest temperature (at most 200 iterations).
//!
//! Weak form `∫ k T' δT' r² dr = ∫ q''' δT r² dr`, the same two-node elements
//! and Gauss rule as [`super::mechanics`], assembled in `farrer-park`'s CSR
//! matrix and solved by the same direct tridiagonal solve.
//!
//! [`analytic_steady_temperature`] is the closed form for piecewise-constant
//! `k` and `q'''`, derived here: with `Q(r)` the power enclosed in radius `r`,
//! `dT/dr = −Q(r)/(4π k r²)`, and inside a layer from `r_i` to `r_o` with
//! `Q(r) = Q_i + (4/3)π q'''(r³ − r_i³)`,
//!
//! ```text
//! T(r) = T(r_o) + C_i/(4π k) (1/r − 1/r_o) + q'''/(6k) (r_o² − r²),
//! C_i  = Q_i − (4/3)π q''' r_i³.
//! ```

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use farrer_park::dof::SparsityPattern;
use farrer_park::quadrature::line_rule;
use farrer_park::sparse::CsrMatrix;

use super::geometry::ParticleMesh;
use super::ParticleMechanicsError;

/// A steady thermal load.
#[derive(Debug, Clone, PartialEq)]
pub struct ThermalLoad {
    /// Volumetric heat generation in each layer, W/m³, one per layer
    /// (usually non-zero in the kernel only).
    pub layer_power_density_w_per_m3: Vec<f64>,
    /// Outer-surface temperature, K.
    pub outer_surface_temperature_k: f64,
}

/// Check the load and that every layer has a conductivity; returns each
/// layer's conductivity evaluated at `t_eval` (K), the value used by the
/// closed form (constant-k layers only).
fn conductivities(mesh: &ParticleMesh, load: &ThermalLoad, t_eval: f64) -> Result<Vec<f64>, ParticleMechanicsError> {
    if load.layer_power_density_w_per_m3.len() != mesh.layers().len() {
        return Err(ParticleMechanicsError::InvalidLoad(format!(
            "need one power density per layer ({}), got {}",
            mesh.layers().len(),
            load.layer_power_density_w_per_m3.len()
        )));
    }
    let t_s = load.outer_surface_temperature_k;
    if !(t_s.is_finite() && t_s > 0.0) {
        return Err(ParticleMechanicsError::InvalidLoad(format!(
            "outer temperature must be positive kelvin, got {t_s}"
        )));
    }
    let mut ks = Vec::with_capacity(mesh.layers().len());
    for l in mesh.layers() {
        match l.material.conductivity_at(t_eval) {
            Some(k) if k.is_finite() && k > 0.0 => ks.push(k),
            Some(k) => {
                return Err(ParticleMechanicsError::InvalidMaterial(format!(
                    "layer {}: conductivity must be positive, got {k}",
                    l.label
                )))
            }
            None => {
                return Err(ParticleMechanicsError::InvalidMaterial(format!(
                    "layer {}: no thermal conductivity given",
                    l.label
                )))
            }
        }
    }
    Ok(ks)
}

/// Nodal temperatures, K, of the steady problem on `mesh`.
///
/// # Errors
///
/// [`ParticleMechanicsError::InvalidMaterial`] if a layer has no positive
/// conductivity; [`ParticleMechanicsError::InvalidLoad`] for a wrong-length
/// power list or a non-positive surface temperature;
/// [`ParticleMechanicsError::LinearSolve`] if the Krylov solve fails.
pub fn solve_steady_conduction(
    mesh: &ParticleMesh,
    load: &ThermalLoad,
) -> Result<Vec<f64>, ParticleMechanicsError> {
    match conductivities(mesh, load, load.outer_surface_temperature_k) {
        Ok(_) => {}
        Err(err) => return Err(err),
    }
    let nonlinear = mesh
        .layers()
        .iter()
        .any(|l| matches!(l.material.conductivity, Some(super::material::Conductivity::Offbeat(_))));
    let n = mesh.n_nodes();
    let mut t = vec![load.outer_surface_temperature_k; n];
    let max_iter = if nonlinear { 200 } else { 1 };
    for _ in 0..max_iter {
        let next = match conduction_pass(mesh, load, &t) {
            Ok(v) => v,
            Err(err) => return Err(err),
        };
        let change = next.iter().zip(&t).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        let scale = next.iter().fold(0.0, |m: f64, v| m.max(v.abs()));
        t = next;
        if !nonlinear || change <= 1e-10 * scale {
            return Ok(t);
        }
    }
    Err(ParticleMechanicsError::LinearSolve(String::from(
        "Picard iteration on k(T) did not converge in 200 iterations",
    )))
}

/// One linear conduction solve with `k` evaluated at the Gauss-point
/// temperatures interpolated from `t_prev` (K, per node).
fn conduction_pass(mesh: &ParticleMesh, load: &ThermalLoad, t_prev: &[f64]) -> Result<Vec<f64>, ParticleMechanicsError> {
    let n = mesh.n_nodes();
    let mut row_ptr = vec![0usize];
    let mut col_idx = Vec::new();
    for i in 0..n {
        if i > 0 {
            col_idx.push(i - 1);
        }
        col_idx.push(i);
        if i + 1 < n {
            col_idx.push(i + 1);
        }
        row_ptr.push(col_idx.len());
    }
    let pattern = SparsityPattern { row_ptr, col_idx };
    let mut kmat = CsrMatrix::from_pattern(&pattern);
    let mut f = vec![0.0; n];
    let rule = line_rule(3);
    for e in 0..mesh.n_elements() {
        let layer = mesh.element_layer(e);
        let mat = mesh.layers()[layer].material;
        let q = load.layer_power_density_w_per_m3[layer];
        let r1 = mesh.node_radius_m(e);
        let r2 = mesh.node_radius_m(e + 1);
        let h = r2 - r1;
        let mut ke = [0.0; 4];
        let mut fe = [0.0; 2];
        for p in &rule {
            let xi = p.xi[0];
            let r = r1 + 0.5 * (xi + 1.0) * h;
            let dv = r * r * 0.5 * h * p.weight;
            let dn = [-1.0 / h, 1.0 / h];
            let nn = [0.5 * (1.0 - xi), 0.5 * (1.0 + xi)];
            let t_gp = nn[0] * t_prev[e] + nn[1] * t_prev[e + 1];
            let k = match mat.conductivity_at(t_gp) {
                Some(k) if k.is_finite() && k > 0.0 => k,
                _ => {
                    return Err(ParticleMechanicsError::InvalidMaterial(format!(
                        "layer {}: conductivity not positive at {t_gp} K",
                        mesh.layers()[layer].label
                    )))
                }
            };
            for a in 0..2 {
                for b in 0..2 {
                    ke[a * 2 + b] += k * dn[a] * dn[b] * dv;
                }
                fe[a] += q * nn[a] * dv;
            }
        }
        match kmat.scatter(&[e, e + 1], &ke) {
            Ok(()) => {}
            Err(err) => return Err(ParticleMechanicsError::LinearSolve(format!("{err}"))),
        }
        f[e] += fe[0];
        f[e + 1] += fe[1];
    }
    // Dirichlet T(b) = T_s, symmetric elimination.
    let last = n - 1;
    let t_s = load.outer_surface_temperature_k;
    if last > 0 {
        let coupling = kmat.get(last - 1, last);
        f[last - 1] -= coupling * t_s;
        let _ = kmat.set(last - 1, last, 0.0);
        let _ = kmat.set(last, last - 1, 0.0);
    }
    let diag = kmat.max_abs_diagonal();
    let _ = kmat.set(last, last, diag);
    f[last] = diag * t_s;
    super::mechanics::direct_solve(&kmat, &f)
}

/// The closed-form steady temperature at radius `r_m` (m), K, for the same
/// problem with CONSTANT conductivities (module docs); a `k(T)` layer is
/// refused, since the closed form does not cover it.
///
/// # Errors
///
/// As [`solve_steady_conduction`]'s input checks, and
/// [`ParticleMechanicsError::InvalidLoad`] for `r_m` outside the mesh.
pub fn analytic_steady_temperature(
    mesh: &ParticleMesh,
    load: &ThermalLoad,
    r_m: f64,
) -> Result<f64, ParticleMechanicsError> {
    if mesh
        .layers()
        .iter()
        .any(|l| matches!(l.material.conductivity, Some(super::material::Conductivity::Offbeat(_))))
    {
        return Err(ParticleMechanicsError::InvalidMaterial(String::from(
            "the layered closed form needs constant conductivities",
        )));
    }
    let ks = match conductivities(mesh, load, load.outer_surface_temperature_k) {
        Ok(ks) => ks,
        Err(err) => return Err(err),
    };
    if !(r_m >= mesh.inner_radius_m() && r_m <= mesh.outer_radius_m()) {
        return Err(ParticleMechanicsError::InvalidLoad(format!("radius {r_m} m outside the particle")));
    }
    let pi = core::f64::consts::PI;
    let layers = mesh.layers();
    // Enclosed power at each layer's inner radius.
    let mut q_in = Vec::with_capacity(layers.len());
    let mut acc = 0.0;
    for (i, l) in layers.iter().enumerate() {
        q_in.push(acc);
        let q = load.layer_power_density_w_per_m3[i];
        acc += 4.0 / 3.0 * pi * q * (l.outer_radius_m * l.outer_radius_m * l.outer_radius_m - l.inner_radius_m * l.inner_radius_m * l.inner_radius_m);
    }
    // Temperature at each layer's outer radius, outside in.
    let mut t_out = vec![0.0; layers.len()];
    let mut t = load.outer_surface_temperature_k;
    for i in (0..layers.len()).rev() {
        t_out[i] = t;
        let l = &layers[i];
        t = layer_temperature(l.inner_radius_m, l.outer_radius_m, ks[i], load.layer_power_density_w_per_m3[i], q_in[i], t_out[i], l.inner_radius_m);
    }
    let i = match layers.iter().position(|l| r_m <= l.outer_radius_m) {
        Some(i) => i,
        None => layers.len() - 1,
    };
    let l = &layers[i];
    Ok(layer_temperature(l.inner_radius_m, l.outer_radius_m, ks[i], load.layer_power_density_w_per_m3[i], q_in[i], t_out[i], r_m))
}

fn layer_temperature(ri: f64, ro: f64, k: f64, q: f64, q_enclosed: f64, t_ro: f64, r: f64) -> f64 {
    let pi = core::f64::consts::PI;
    let c = q_enclosed - 4.0 / 3.0 * pi * q * ri * ri * ri;
    let shell = if c == 0.0 { 0.0 } else { c / (4.0 * pi * k) * (1.0 / r - 1.0 / ro) };
    t_ro + shell + q / (6.0 * k) * (ro * ro - r * r)
}
