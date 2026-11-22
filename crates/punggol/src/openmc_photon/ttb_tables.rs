// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): port of OpenMC.
//   Upstream project: OpenMC, https://github.com/openmc-dev/openmc
//   Upstream files:   src/material.cpp (collision_stopping_power,
//                     init_bremsstrahlung, sternheimer_adjustment,
//                     density_effect), src/photon.cpp (the TTB part of the
//                     PhotonInteraction constructor), src/math_functions.cpp
//                     (spline, spline_integrate)
//   Upstream commit:  8f79d9d0
//   Upstream licence: MIT, Copyright (c) 2011-2025 Massachusetts Institute of
//                     Technology, UChicago Argonne LLC, and OpenMC contributors
//   This port:        AGPL-3.0-only, with the MIT notice preserved here.
//! **Thick-target bremsstrahlung tables**: how OpenMC derives a material's
//! `BremsstrahlungData` (`pdf`, `cdf`, `yield`) from the elements' scaled
//! bremsstrahlung DCS and the collision and radiative stopping powers
//! (gh:#1693). [`super::bremsstrahlung`] samples from these tables; before this
//! module the tables had to be supplied from elsewhere.
//!
//! **Status: first port, differentially tested against a built OpenMC for U
//! and H (`docs/openmc_photon_phase2.md`).**
//!
//! Ported: the element part of the `PhotonInteraction` constructor (truncating
//! the DCS at the photon cutoff and the radiative stopping power,
//! [`prepare_element_ttb`]), `Material::collision_stopping_power`,
//! `sternheimer_adjustment`, `density_effect`, `Material::init_bremsstrahlung`
//! ([`init_bremsstrahlung`]), and the cubic `spline` / `spline_integrate` it
//! integrates with. The material is reduced to what these read:
//! [`TtbComponent`]s (an element and the nuclide's `awr` and the
//! `atom_density_` entry) and the material's `density_` entry.
//!
//! Units follow OpenMC: eV, `atom/b-cm` (positive `atom_density`) or negative
//! weight fractions (`-w`, converted with `awr`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::bremsstrahlung::{Bremsstrahlung, BremsstrahlungData};
use super::{FINE_STRUCTURE, MASS_ELECTRON_EV, Matrix, PLANCK_C, lower_bound_index};
use alloc::vec::Vec;
use core::f64::consts::PI;
use petir::mathf::RealMath;

/// `N_AVOGADRO` (`constants.h`), in 10^24 / mol.
pub const N_AVOGADRO: f64 = 0.602214076;
/// `MASS_NEUTRON` (`atomic_mass.h`), amu.
pub const MASS_NEUTRON_AMU: f64 = 1.00866491595;

/// The raw `bremsstrahlung` group of an element's HDF5 file plus its atomic
/// number.
#[derive(Debug, Clone)]
pub struct ElementTtbInput {
    /// `Z_`.
    pub z: f64,
    /// `I_`: mean excitation energy, eV.
    pub i_ev: f64,
    /// `n_electrons_` (negative: conduction electrons).
    pub n_electrons: Vec<f64>,
    /// `ionization_energy_`, eV.
    pub ionization_energy: Vec<f64>,
    /// `dcs`, `[electron energy][reduced photon energy]`.
    pub dcs: Matrix,
    /// `electron_energy` (eV), before truncation.
    pub electron_energy: Vec<f64>,
}

/// What `PhotonInteraction` keeps for TTB after its constructor.
#[derive(Debug, Clone)]
pub struct ElementTtb {
    /// `Z_`.
    pub z: f64,
    /// `I_`.
    pub i_ev: f64,
    /// `n_electrons_`.
    pub n_electrons: Vec<f64>,
    /// `ionization_energy_`.
    pub ionization_energy: Vec<f64>,
    /// `dcs_` after truncation at the photon cutoff.
    pub dcs: Matrix,
    /// `stopping_power_radiative_` on the global grid.
    pub stopping_power_radiative: Vec<f64>,
}

/// The TTB part of the `PhotonInteraction` constructor (`photon.cpp:316-389`):
/// truncate the DCS at `photon_cutoff` (eV) if that is above the lowest
/// electron energy, and compute the radiative stopping power on the global
/// incident-energy grid. `global_e_grid` is `data::ttb_e_grid` as already set
/// by a previous element, or `None` for the first element (its truncated
/// electron-energy grid then becomes the global one, which is returned).
/// `k_grid` is `data::ttb_k_grid`.
pub fn prepare_element_ttb(input: &ElementTtbInput, photon_cutoff: f64, global_e_grid: Option<&[f64]>, k_grid: &[f64]) -> (ElementTtb, Vec<f64>) {
    let n_e = input.dcs.rows();
    let n_k = input.dcs.cols();
    let e = &input.electron_energy;
    let mut dcs = input.dcs.clone();
    let mut electron_energy = e.clone();
    if photon_cutoff > e[0] {
        let i_grid = lower_bound_index(e, photon_cutoff);
        let f = (RealMath::r_ln(photon_cutoff) - RealMath::r_ln(e[i_grid])) / (RealMath::r_ln(e[i_grid + 1]) - RealMath::r_ln(e[i_grid]));
        let mut data = Vec::with_capacity((n_e - i_grid) * n_k);
        for i in 0..n_k {
            let _ = i;
        }
        // row 0: the DCS interpolated (log-log) at the cutoff, then the rows above it
        for i in 0..n_k {
            let y = RealMath::r_exp(RealMath::r_ln(input.dcs.at(i_grid, i)) + f * (RealMath::r_ln(input.dcs.at(i_grid + 1, i)) - RealMath::r_ln(input.dcs.at(i_grid, i))));
            data.push(y);
        }
        for j in (i_grid + 1)..n_e {
            data.extend_from_slice(input.dcs.row(j));
        }
        dcs = Matrix::new(data, n_k).unwrap_or_else(|_| input.dcs.clone());
        let mut grid = Vec::with_capacity(e.len() - i_grid);
        grid.push(photon_cutoff);
        grid.extend_from_slice(&e[i_grid + 1..]);
        electron_energy = grid;
    }
    let grid: Vec<f64> = match global_e_grid {
        Some(g) => g.to_vec(),
        None => electron_energy,
    };
    let mut s_rad = Vec::with_capacity(grid.len());
    for (i, &en) in grid.iter().enumerate() {
        let mut c = 0.0;
        for j in 0..k_grid.len() - 1 {
            c += 0.5 * (dcs.at(i, j + 1) + dcs.at(i, j)) * (k_grid[j + 1] - k_grid[j]);
        }
        let beta_sq = en * (en + 2.0 * MASS_ELECTRON_EV) / ((en + MASS_ELECTRON_EV) * (en + MASS_ELECTRON_EV));
        s_rad.push(input.z * input.z / beta_sq * en * c);
    }
    (
        ElementTtb {
            z: input.z,
            i_ev: input.i_ev,
            n_electrons: input.n_electrons.clone(),
            ionization_energy: input.ionization_energy.clone(),
            dcs,
            stopping_power_radiative: s_rad,
        },
        grid,
    )
}

/// One element of a material as `collision_stopping_power` and
/// `init_bremsstrahlung` read it.
#[derive(Debug, Clone)]
pub struct TtbComponent {
    /// `data::nuclides[nuclide_[i]]->awr_`.
    pub awr: f64,
    /// `atom_density_[i]`: atom/b-cm if positive; negative values are weight
    /// fractions (`-w`).
    pub atom_density: f64,
    /// The element.
    pub element: ElementTtb,
}

/// `atom_density_[0] > 0 ? atom_density_[i] : -atom_density_[i] / awr`.
fn atom_density_of(components: &[TtbComponent], i: usize) -> f64 {
    if components[0].atom_density > 0.0 { components[i].atom_density } else { -components[i].atom_density / components[i].awr }
}

/// `sternheimer_adjustment` (`material.cpp:1197-1242`).
pub fn sternheimer_adjustment(f: &[f64], e_b_sq: &[f64], e_p_sq: f64, n_conduction: f64, log_i: f64, tol: f64, max_iter: i32) -> f64 {
    let n = f.len();
    let mut rho = 2.0_f64;
    let mut iter = 0;
    while iter < max_iter {
        let rho_0 = rho;
        let mut g = 0.0;
        let mut gp = 0.0;
        for i in 0..n {
            let e_r_sq = e_b_sq[i] * rho * rho + 2.0 / 3.0 * f[i] * e_p_sq;
            g += f[i] * RealMath::r_ln(e_r_sq);
            gp += e_b_sq[i] * f[i] * rho / e_r_sq;
        }
        if n_conduction > 0.0 {
            g += n_conduction * RealMath::r_ln(n_conduction * e_p_sq);
        }
        rho -= (g - 2.0 * log_i) / (2.0 * gp);
        if rho < 0.0 {
            rho = rho_0 / 2.0;
        }
        if (rho - rho_0).abs() / rho_0 < tol {
            break;
        }
        iter += 1;
    }
    if iter >= max_iter {
        // upstream warns
        rho = 1.0e-6;
    }
    rho
}

/// `density_effect` (`material.cpp:1244-1318`).
pub fn density_effect(f: &[f64], e_b_sq: &[f64], e_p_sq: f64, n_conduction: f64, rho: f64, e: f64, tol: f64, max_iter: i32) -> f64 {
    let n = f.len();
    let beta_sq = e * (e + 2.0 * MASS_ELECTRON_EV) / ((e + MASS_ELECTRON_EV) * (e + MASS_ELECTRON_EV));
    let mut beta_0_sq = 0.0;
    if n_conduction == 0.0 {
        for i in 0..n {
            beta_0_sq += f[i] * e_p_sq / (e_b_sq[i] * rho * rho);
        }
        beta_0_sq = 1.0 / (1.0 + beta_0_sq);
    }
    let mut delta = 0.0;
    if beta_sq < beta_0_sq {
        return delta;
    }
    let mut w_sq = e / MASS_ELECTRON_EV * (e / MASS_ELECTRON_EV + 2.0);
    let mut iter = 0;
    while iter < max_iter {
        let w_sq_0 = w_sq;
        let mut g = 0.0;
        let mut gp = 0.0;
        for i in 0..n {
            let c = e_b_sq[i] * rho * rho / e_p_sq + w_sq;
            g += f[i] / c;
            gp -= f[i] / (c * c);
        }
        g += n_conduction / w_sq;
        gp -= n_conduction / (w_sq * w_sq);
        w_sq -= (g + 1.0 - 1.0 / beta_sq) / gp;
        if w_sq < 0.0 {
            w_sq = w_sq_0 / 2.0;
        }
        if (w_sq - w_sq_0).abs() / w_sq_0 < tol {
            break;
        }
        iter += 1;
    }
    if iter >= max_iter {
        return delta;
    }
    for i in 0..n {
        let l_sq = e_b_sq[i] * rho * rho / e_p_sq + 2.0 / 3.0 * f[i];
        delta += f[i] * RealMath::r_ln((l_sq + w_sq) / l_sq);
    }
    if n_conduction > 0.0 {
        delta += n_conduction * RealMath::r_ln((n_conduction + w_sq) / n_conduction);
    }
    delta - w_sq * (1.0 - beta_sq)
}

/// `Material::collision_stopping_power` (`material.cpp:526-621`): the collision
/// stopping power on `e_grid` (eV, linear). `density_entry` is the material's
/// `density_` (negative: g/cm^3, otherwise `atom/b-cm` and the mass density is
/// summed from the components).
pub fn collision_stopping_power(components: &[TtbComponent], density_entry: f64, e_grid: &[f64], positron: bool) -> Vec<f64> {
    let mut electron_density = 0.0;
    let mut mass_density = 0.0;
    let mut log_i = 0.0;
    let mut n_conduction = 0.0;
    let mut f: Vec<f64> = Vec::new();
    let mut e_b_sq: Vec<f64> = Vec::new();
    for (i, c) in components.iter().enumerate() {
        let elm = &c.element;
        let atom_density = atom_density_of(components, i);
        electron_density += atom_density * elm.z;
        mass_density += atom_density * c.awr * MASS_NEUTRON_AMU;
        log_i += atom_density * elm.z * RealMath::r_ln(elm.i_ev);
        for j in 0..elm.n_electrons.len() {
            if elm.n_electrons[j] < 0.0 {
                n_conduction -= elm.n_electrons[j] * atom_density;
                continue;
            }
            e_b_sq.push(elm.ionization_energy[j] * elm.ionization_energy[j]);
            f.push(elm.n_electrons[j] * atom_density);
        }
    }
    log_i /= electron_density;
    n_conduction /= electron_density;
    for f_i in f.iter_mut() {
        *f_i /= electron_density;
    }
    let density = if density_entry < 0.0 { -density_entry } else { mass_density / N_AVOGADRO };
    let e_p_sq = PLANCK_C * PLANCK_C * PLANCK_C * N_AVOGADRO * electron_density * density / (2.0 * PI * PI * FINE_STRUCTURE * MASS_ELECTRON_EV * mass_density);
    let rho = sternheimer_adjustment(&f, &e_b_sq, e_p_sq, n_conduction, log_i, 1.0e-6, 100);
    const CM_PER_ANGSTROM: f64 = 1.0e-8;
    let r_e = CM_PER_ANGSTROM * PLANCK_C / (2.0 * PI * FINE_STRUCTURE * MASS_ELECTRON_EV);
    const BARN_PER_CM_SQ: f64 = 1.0e24;
    let c = BARN_PER_CM_SQ * 2.0 * PI * r_e * r_e * MASS_ELECTRON_EV * electron_density;
    let mut s_col = Vec::with_capacity(e_grid.len());
    for &e in e_grid {
        let delta = density_effect(&f, &e_b_sq, e_p_sq, n_conduction, rho, e, 1.0e-6, 100);
        let beta_sq = e * (e + 2.0 * MASS_ELECTRON_EV) / ((e + MASS_ELECTRON_EV) * (e + MASS_ELECTRON_EV));
        let tau = e / MASS_ELECTRON_EV;
        let ff = if positron {
            let t = tau + 2.0;
            RealMath::r_ln(4.0) - (beta_sq / 12.0) * (23.0 + 14.0 / t + 10.0 / (t * t) + 4.0 / (t * t * t))
        } else {
            (1.0 - beta_sq) * (1.0 + tau * tau / 8.0 - (2.0 * tau + 1.0) * RealMath::r_ln(2.0))
        };
        s_col.push(c / beta_sq * (2.0 * (RealMath::r_ln(e) - log_i) + RealMath::r_ln(1.0 + tau / 2.0) + ff - delta));
    }
    s_col
}

/// `spline` (`math_functions.cpp:803-824`): natural cubic spline second
/// derivatives `z` for the `n = x.len()` points.
pub fn spline(x: &[f64], y: &[f64], z: &mut [f64]) {
    let n = x.len();
    let mut c_new = alloc::vec![0.0_f64; n - 1];
    c_new[0] = 0.0;
    z[0] = 0.0;
    z[n - 1] = 0.0;
    for i in 1..n - 1 {
        let a = x[i] - x[i - 1];
        let c = x[i + 1] - x[i];
        let b = 2.0 * (a + c);
        let d = 6.0 * ((y[i + 1] - y[i]) / c - (y[i] - y[i - 1]) / a);
        c_new[i] = c / (b - a * c_new[i - 1]);
        z[i] = (d - a * z[i - 1]) / (b - a * c_new[i - 1]);
    }
    for i in (0..=n - 2).rev() {
        z[i] -= c_new[i] * z[i + 1];
    }
}

/// `spline_integrate` (`math_functions.cpp:850-895`).
pub fn spline_integrate(x: &[f64], y: &[f64], z: &[f64], xa: f64, xb: f64) -> f64 {
    let n = x.len();
    let mut ia = n - 1;
    loop {
        ia -= 1;
        if ia == 0 || xa >= x[ia] {
            break;
        }
    }
    let mut ib = n - 1;
    loop {
        ib -= 1;
        if ib == 0 || xb >= x[ib] {
            break;
        }
    }
    let mut s = 0.0;
    for i in ia..=ib {
        let mut h = x[i + 1] - x[i];
        let b = (y[i + 1] - y[i]) / h - (h / 6.0) * (z[i + 1] + 2.0 * z[i]);
        let c = z[i] / 2.0;
        let d = (z[i + 1] - z[i]) / (h * 6.0);
        if i == ia {
            let r = xa - x[ia];
            s -= y[i] * r + b / 2.0 * r * r + c / 3.0 * r * r * r + d / 4.0 * r * r * r * r;
        }
        if i == ib {
            h = xb - x[ib];
        }
        s += y[i] * h + b / 2.0 * h * h + c / 3.0 * h * h * h + d / 4.0 * h * h * h * h;
    }
    s
}

/// `Material::init_bremsstrahlung` (`material.cpp:623-802`): the electron and
/// positron tables of a material on `e_grid` (eV, **linear**: upstream takes
/// the logarithm of `data::ttb_e_grid` only after every material is built) and
/// `k_grid` (reduced photon energies).
pub fn init_bremsstrahlung(components: &[TtbComponent], density_entry: f64, e_grid: &[f64], k_grid: &[f64]) -> Bremsstrahlung {
    let n_e = e_grid.len();
    let n_k = k_grid.len();
    let mut tables: Vec<BremsstrahlungData> = Vec::with_capacity(2);
    for particle in 0..2 {
        let positron = particle == 1;
        let mut pdf = alloc::vec![0.0_f64; n_e * n_e];
        let mut cdf = alloc::vec![0.0_f64; n_e * n_e];
        let mut yield_ = alloc::vec![0.0_f64; n_e];
        let s_col = collision_stopping_power(components, density_entry, e_grid, positron);
        let mut s_rad = alloc::vec![0.0_f64; n_e];
        let mut dcs = alloc::vec![0.0_f64; n_e * n_k];
        let mut z_eq_sq = 0.0;
        let mut sum_density = 0.0;
        for (i, c) in components.iter().enumerate() {
            let elm = &c.element;
            let atom_density = atom_density_of(components, i);
            z_eq_sq += atom_density * elm.z * elm.z;
            sum_density += atom_density;
            let w = atom_density * elm.z * elm.z;
            for j in 0..n_e {
                for k in 0..n_k {
                    dcs[j * n_k + k] += w * elm.dcs.at(j, k);
                }
                s_rad[j] += atom_density * elm.stopping_power_radiative[j];
            }
        }
        z_eq_sq /= sum_density;
        if positron {
            for i in 0..n_e {
                let t = RealMath::r_ln(1.0 + 1.0e6 * e_grid[i] / (z_eq_sq * MASS_ELECTRON_EV));
                let r = 1.0
                    - RealMath::r_exp(
                        -1.2359e-1 * t + 6.1274e-2 * RealMath::r_powf(t, 2.0) - 3.1516e-2 * RealMath::r_powf(t, 3.0) + 7.7446e-3 * RealMath::r_powf(t, 4.0)
                            - 1.0595e-3 * RealMath::r_powf(t, 5.0)
                            + 7.0568e-5 * RealMath::r_powf(t, 6.0)
                            - 1.808e-6 * RealMath::r_powf(t, 7.0),
                    );
                s_rad[i] *= r;
                for k in 0..n_k {
                    dcs[i * n_k + k] *= r;
                }
            }
        }
        let stopping_power: Vec<f64> = s_col.iter().zip(&s_rad).map(|(a, b)| a + b).collect();
        let mut f = alloc::vec![0.0_f64; n_e];
        let mut z = alloc::vec![0.0_f64; n_e];
        for i in 0..n_e - 1 {
            let w = e_grid[i];
            for j in i..n_e {
                let e = e_grid[j];
                let k = w / e;
                let i_k = lower_bound_index(k_grid, k);
                let k_l = k_grid[i_k];
                let k_r = k_grid[i_k + 1];
                let x_l = dcs[j * n_k + i_k];
                let x_r = dcs[j * n_k + i_k + 1];
                let x = x_l + (k - k_l) * (x_r - x_l) / (k_r - k_l);
                let beta_sq = e * (e + 2.0 * MASS_ELECTRON_EV) / ((e + MASS_ELECTRON_EV) * (e + MASS_ELECTRON_EV));
                f[j] = x / (beta_sq * stopping_power[j] * w);
            }
            let n = n_e - i;
            if n > 2 {
                spline(&e_grid[i..], &f[i..], &mut z[i..]);
                let mut c = 0.0;
                for j in i..n_e - 1 {
                    c += spline_integrate(&e_grid[i..], &f[i..], &z[i..], e_grid[j], e_grid[j + 1]);
                    pdf[(j + 1) * n_e + i] = c;
                }
            } else {
                let e_l = RealMath::r_ln(e_grid[i]);
                let e_r = RealMath::r_ln(e_grid[i + 1]);
                let x_l = RealMath::r_ln(f[i]);
                let x_r = RealMath::r_ln(f[i + 1]);
                pdf[(i + 1) * n_e + i] = 0.5 * (e_r - e_l) * (RealMath::r_exp(e_l + x_l) + RealMath::r_exp(e_r + x_r));
            }
        }
        for j in 1..n_e {
            pdf[j * n_e + j] = RealMath::r_exp(-500.0);
            let mut c = 0.0;
            for i in 0..j {
                let w_l = RealMath::r_ln(e_grid[i]);
                let w_r = RealMath::r_ln(e_grid[i + 1]);
                let x_l = RealMath::r_ln(pdf[j * n_e + i]);
                let x_r = RealMath::r_ln(pdf[j * n_e + i + 1]);
                let beta = (x_r - x_l) / (w_r - w_l);
                let a = beta + 1.0;
                c += RealMath::r_exp(w_l + x_l) / a * RealMath::r_exp_m1(a * (w_r - w_l));
                cdf[j * n_e + i + 1] = c;
            }
            yield_[j] = c;
        }
        for y in yield_.iter_mut() {
            *y = if *y > 0.0 { RealMath::r_ln(*y) } else { -500.0 };
        }
        tables.push(BremsstrahlungData { pdf: Matrix::new(pdf, n_e).unwrap_or_default(), cdf: Matrix::new(cdf, n_e).unwrap_or_default(), yield_ });
    }
    let positron = tables.pop().unwrap_or_default();
    let electron = tables.pop().unwrap_or_default();
    Bremsstrahlung { electron, positron }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The spline reproduces a cubic exactly in the interior and integrates
    /// a smooth function to its analytic value.
    #[test]
    fn spline_integrates_a_smooth_function() {
        let x: Vec<f64> = (0..21).map(|i| i as f64 * 0.1).collect();
        let y: Vec<f64> = x.iter().map(|v| v.sin()).collect();
        let mut z = vec![0.0; x.len()];
        spline(&x, &y, &mut z);
        let s = spline_integrate(&x, &y, &z, 0.3, 1.7);
        assert!((s - (0.3f64.cos() - 1.7f64.cos())).abs() < 1e-4, "{s}");
    }

    /// Newton iterations converge for a single oscillator and the density
    /// effect vanishes below beta_0 for a nonmetal.
    #[test]
    fn sternheimer_and_density_effect_basic_behaviour() {
        let f = [1.0];
        let e_b_sq = [100.0 * 100.0];
        let rho = sternheimer_adjustment(&f, &e_b_sq, 20.0 * 20.0, 0.0, RealMath::r_ln(150.0), 1.0e-6, 100);
        assert!(rho > 0.0 && rho.is_finite());
        assert_eq!(density_effect(&f, &e_b_sq, 20.0 * 20.0, 0.0, rho, 1.0e3, 1.0e-6, 100), 0.0);
        assert!(density_effect(&f, &e_b_sq, 20.0 * 20.0, 0.0, rho, 1.0e9, 1.0e-6, 100) > 0.0);
    }

    fn toy_element(z: f64, n_e: usize, n_k: usize) -> ElementTtb {
        let dcs: Vec<f64> = (0..n_e * n_k).map(|i| 1.0 + 0.01 * (i % 7) as f64).collect();
        ElementTtb {
            z,
            i_ev: 12.0 * z + 7.0,
            n_electrons: vec![z - 1.0, 1.0],
            ionization_energy: vec![50.0 * z, 13.6],
            dcs: Matrix::new(dcs, n_k).unwrap(),
            stopping_power_radiative: (0..n_e).map(|i| 1.0e-3 * (1.0 + i as f64)).collect(),
        }
    }

    /// A toy material gives tables of the right shape (the toy numbers are not
    /// physical, so their values are not checked; the differential test against
    /// a built OpenMC is the check on the values).
    #[test]
    fn init_bremsstrahlung_gives_tables_of_the_right_shape() {
        let n_e = 8;
        let e_grid: Vec<f64> = (0..n_e).map(|i| 1.0e3 * 10f64.powf(i as f64 * 0.7)).collect();
        let k_grid: Vec<f64> = (0..5).map(|i| i as f64 * 0.25).collect();
        let comps = [TtbComponent { awr: 28.0, atom_density: 0.05, element: toy_element(14.0, n_e, k_grid.len()) }];
        let t = init_bremsstrahlung(&comps, 2.3, &e_grid, &k_grid);
        for p in [&t.electron, &t.positron] {
            assert_eq!((p.pdf.rows(), p.pdf.cols(), p.cdf.rows(), p.yield_.len()), (n_e, n_e, n_e, n_e));
            assert_eq!(p.yield_[0], -500.0);
        }
        // weight-fraction input (negative atom_density) takes the other branch
        let w = [TtbComponent { awr: 28.0, atom_density: -1.0, element: toy_element(14.0, n_e, k_grid.len()) }];
        let tw = init_bremsstrahlung(&w, -2.3, &e_grid, &k_grid);
        assert_eq!(tw.electron.yield_.len(), n_e);
    }

    /// Truncation at a cutoff above the lowest energy starts the grid at the cutoff.
    #[test]
    fn prepare_element_truncates_at_the_cutoff() {
        let e = vec![1.0e3, 2.0e3, 4.0e3, 8.0e3];
        let dcs = Matrix::new(vec![1.0; 4 * 3], 3).unwrap();
        let inp = ElementTtbInput { z: 6.0, i_ev: 78.0, n_electrons: vec![6.0], ionization_energy: vec![40.0], dcs, electron_energy: e };
        let (el, grid) = prepare_element_ttb(&inp, 3.0e3, None, &[0.0, 0.5, 1.0]);
        assert_eq!(grid, vec![3.0e3, 4.0e3, 8.0e3]);
        assert_eq!(el.dcs.rows(), 3);
        assert_eq!(el.stopping_power_radiative.len(), 3);
        let (_, g2) = prepare_element_ttb(&inp, 500.0, None, &[0.0, 0.5, 1.0]);
        assert_eq!(g2.len(), 4);
    }
}
