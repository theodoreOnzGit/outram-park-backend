//! Composition-dependent **reducing functions** `T_r(x)`, `ρ_r(x)` for the
//! multi-fluid mixture model — the GERG-2008 (Kunz & Wagner) form, ported from
//! CoolProp `GERG2008ReducingFunction::{Tr,rhormolar}`.
//!
//! `δ = ρ/ρ_r(x)`, `τ = T_r(x)/T` — the reduced state every component's
//! Helmholtz surface is evaluated at — come from
//!
//! `T_r(x) = Σ_i x_i² T_{c,i} + Σ_i Σ_{j>i} 2 β^T_{ij} γ^T_{ij} T_{c,ij} · f_{ij}(x, β^T_{ij})`
//!
//! `v_r(x) = Σ_i x_i² v_{c,i} + Σ_i Σ_{j>i} 2 β^v_{ij} γ^v_{ij} v_{c,ij} · f_{ij}(x, β^v_{ij})`
//! (`v_r = 1/ρ_r`, `v_{c,i} = 1/ρ_{c,i}`), with the shared mixing kernel
//!
//! `f_{ij}(x, β) = x_i x_j (x_i+x_j) / (β² x_i + x_j)`
//!
//! and the **standard combining rules** for the cross critical parameters
//! (CoolProp `ReducingFunctions.h`: `T_c` / `v_c` matrix members) —
//! `T_{c,ij} = sqrt(T_{c,i} T_{c,j})`, `v_{c,ij} = (1/8)(v_{c,i}^{1/3} + v_{c,j}^{1/3})³`
//! — evaluated directly from each pure component's own critical parameters
//! (already in [`crate::eos::FluidEos`]), not stored per binary pair. Only
//! `β^T_{ij}`, `γ^T_{ij}`, `β^v_{ij}`, `γ^v_{ij}` are pair-specific data (see
//! [`super::binary_pairs`]).

use super::Mixture;

/// The shared GERG-2008 mixing kernel `f_{ij}(x_i, x_j, β) = x_i x_j (x_i+x_j)
/// / (β² x_i + x_j)`.
fn f_ij(xi: f64, xj: f64, beta: f64) -> f64 {
    xi * xj * (xi + xj) / (beta * beta * xi + xj)
}

/// Composition-dependent reducing temperature `T_r(x)` \[K\].
pub fn reducing_temperature(mix: &Mixture) -> f64 {
    let n = mix.components.len();
    let mut tr = 0.0;
    for i in 0..n {
        let xi = mix.mole_fractions[i];
        let tci = mix.components[i].eos().t_critical;
        tr += xi * xi * tci;
        for j in (i + 1)..n {
            let xj = mix.mole_fractions[j];
            let tcj = mix.components[j].eos().t_critical;
            let pair = super::binary_pairs::lookup(mix.components[i], mix.components[j]);
            let (beta_t, gamma_t) = pair.map(|p| p.beta_gamma_t).unwrap_or((1.0, 1.0));
            let tc_ij = (tci * tcj).sqrt();
            tr += 2.0 * beta_t * gamma_t * tc_ij * f_ij(xi, xj, beta_t);
        }
    }
    tr
}

/// Composition-dependent reducing molar density `ρ_r(x)` \[mol/m³\].
pub fn reducing_density(mix: &Mixture) -> f64 {
    let n = mix.components.len();
    let mut vr = 0.0;
    for i in 0..n {
        let xi = mix.mole_fractions[i];
        let vci = 1.0 / mix.components[i].eos().rho_critical;
        vr += xi * xi * vci;
        for j in (i + 1)..n {
            let xj = mix.mole_fractions[j];
            let vcj = 1.0 / mix.components[j].eos().rho_critical;
            let pair = super::binary_pairs::lookup(mix.components[i], mix.components[j]);
            let (beta_v, gamma_v) = pair.map(|p| p.beta_gamma_v).unwrap_or((1.0, 1.0));
            let vc_ij = (vci.cbrt() + vcj.cbrt()).powi(3) / 8.0;
            vr += 2.0 * beta_v * gamma_v * vc_ij * f_ij(xi, xj, beta_v);
        }
    }
    1.0 / vr
}
