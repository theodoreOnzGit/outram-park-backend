//! Sectional aerosol dynamics for one well-mixed volume: agglomeration and
//! gravitational settling (gh:#1047, first deliverable; not yet wired into
//! the plant).
//!
//! **Kernels** are MELCOR's MAEROS kernels, from the description in its
//! manual (Vol. 2, SAND2023-10994 O, RN §2.4.2 and Appendix B, which takes
//! them from Powers, Sprung and Leigh; no source used):
//! - Brownian, Fuchs' interpolation (Eqs B-1 to B-12);
//! - gravitational, with the collision efficiency `1.5 d_j^2/(d_i+d_j)^2`
//!   (Eqs B-13 to B-15, 2-38);
//! - turbulent shear and inertial, combined in quadrature (B-16 to B-18);
//! - their sum (Eq 2-40), all with MELCOR's defaults: shape factors 1,
//!   sticking 1, slip 1.257, dissipation 0.001 m^2/s^3.
//!
//! Two lines of the extracted manual text are garbled and are pinned here
//! by their limits instead of being trusted: Fuchs' factor `F` (B-4) is the
//! **inverse** of its bracket, the only reading that gives the continuum
//! limit `8kT/(3 mu)` for equal particles and the free-molecular limit
//! `pi (d_i+d_j)^2 v_ij / 4`; and the mean free path (B-11) is
//! `nu sqrt(1.89e-4 M_w / T)`, whose constant is `4 pi/(8 R 1000)` (about
//! 0.064 um in air at 298 K, the manual's text quoting about 0.069 um).
//! The slip correction (B-3) is the form `changi::flexpart::aerosol::part0`
//! also uses, computed inline there, so not reusable as a function.
//!
//! **Discretisation is not MAEROS's.** MAEROS uses Gelbard and Seinfeld's
//! sectional coefficients, which the manual describes but does not give.
//! This uses the open-literature semi-implicit, volume-conserving sectional
//! scheme of Jacobson, Turco, Jensen and Toon (1994, *Atmos. Environ.* 28,
//! 1327), with representative volumes in a fixed ratio of at least 2 (the
//! analogue of MAEROS's constraint `m_(i+1)/m_i > 2`, Eq 2-37). As in that
//! scheme, ~~particles that outgrow the top section stay in it; MELCOR's
//! fallout of particles above the largest size is not built (stated)~~
//! particles that outgrow the top section fall out, as in MELCOR (RN
//! §2.4.1): the scheme sends them to a virtual section above the top, and
//! that volume is booked as fallout.
//!
//! **Deposition** (RN §2.4.2.2) onto floors, walls and ceilings (a pool
//! surface counts as a floor): gravitational settling (floors positive,
//! ceilings negative), Brownian diffusion across a boundary layer (Eq
//! 2-42), thermophoresis (Eq 2-43) and diffusiophoresis with the Stefan flow
//! (Eqs 2-45, 2-46), summed per surface as MELCOR's table says and floored
//! at zero, then applied as exact exponential decay at `sum A v / V`.
//! Turbulent deposition in pipes and bends is not built.

use petir::mathf::RealMath;

const K_B: f64 = 1.380_649e-23;
const G: f64 = 9.80665;

/// The gas and the aerosol's material, and MELCOR's model constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Medium {
    /// Gas temperature, K, pressure, Pa, molar mass, g/mol, viscosity,
    /// Pa s.
    pub t_k: f64,
    pub p_pa: f64,
    pub molar_mass_g: f64,
    pub mu_pa_s: f64,
    /// Particle material density, kg/m^3 (MELCOR's RHONOM default 1000).
    pub rho_p: f64,
    /// Agglomeration and dynamic shape factors, sticking coefficient,
    /// slip coefficient, turbulent dissipation, m^2/s^3 (MELCOR defaults:
    /// 1, 1, 1, 1.257, 0.001).
    pub gamma: f64,
    pub chi: f64,
    pub sticking: f64,
    pub slip: f64,
    pub dissipation: f64,
    /// Gas-to-particle conductivity ratio, thermal accommodation constant,
    /// diffusion boundary-layer thickness, m (MELCOR's RN1_MS01 defaults:
    /// 0.05, 2.25, 1e-5).
    pub k_ratio: f64,
    pub accommodation: f64,
    pub boundary_layer_m: f64,
}

impl Medium {
    /// Air at `t_k` and `p_pa`, with MELCOR's default aerosol constants.
    pub fn air(t_k: f64, p_pa: f64, mu_pa_s: f64) -> Self {
        Self {
            t_k,
            p_pa,
            molar_mass_g: 28.97,
            mu_pa_s,
            rho_p: 1000.0,
            gamma: 1.0,
            chi: 1.0,
            sticking: 1.0,
            slip: 1.257,
            dissipation: 0.001,
            k_ratio: 0.05,
            accommodation: 2.25,
            boundary_layer_m: 1.0e-5,
        }
    }
    /// Gas density, kg/m^3 (Eq B-12).
    pub fn rho_gas(&self) -> f64 {
        1.21e-4 * self.p_pa * self.molar_mass_g / self.t_k
    }
    /// Mean free path, m (Eq B-11, read as `nu sqrt(1.89e-4 M/T)`).
    pub fn mean_free_path(&self) -> f64 {
        self.mu_pa_s / self.rho_gas() * (1.89e-4 * self.molar_mass_g / self.t_k).sqrt()
    }
    /// Slip correction of a particle of diameter `d` (Eq B-3).
    pub fn slip_correction(&self, d: f64) -> f64 {
        let kn = 2.0 * self.mean_free_path() / d;
        1.0 + kn * (self.slip + 0.4 * (-1.1 / kn).r_exp())
    }
    /// Particle diffusion coefficient, m^2/s (Eq B-2).
    pub(crate) fn diffusivity(&self, d: f64) -> f64 {
        K_B * self.t_k / (3.0 * std::f64::consts::PI * d * self.mu_pa_s * self.chi) * self.slip_correction(d)
    }
    /// Stokes terminal velocity with slip, m/s (Eq B-14, and the settling
    /// velocity of RN §2.4.2.2).
    pub fn settling_velocity(&self, d: f64) -> f64 {
        self.rho_p * G * d * d * self.slip_correction(d) / (18.0 * self.mu_pa_s * self.chi)
    }

    /// Brownian diffusion deposition velocity, m/s (Eq 2-42).
    pub fn diffusion_velocity(&self, d: f64) -> f64 {
        K_B * self.t_k * self.slip_correction(d) / (3.0 * std::f64::consts::PI * self.mu_pa_s * self.chi * d * self.boundary_layer_m)
    }

    /// Thermophoretic velocity toward a surface, m/s, for a gas
    /// temperature gradient `grad_t` toward it (K/m, positive when the gas
    /// cools toward the surface) at wall temperature `t_wall` (Eq 2-43).
    pub fn thermophoretic_velocity(&self, d: f64, grad_t: f64, t_wall: f64) -> f64 {
        let kn = 2.0 * self.mean_free_path() / d;
        let (ct, kr) = (self.accommodation, self.k_ratio);
        3.0 * self.mu_pa_s * self.slip_correction(d) * (ct * kn + kr) * grad_t
            / (2.0 * self.chi * self.rho_gas() * t_wall * (1.0 + 3.0 * self.slip * kn) * (1.0 + 2.0 * ct * kn + kr))
    }

    /// Brownian kernel, m^3/s (Fuchs; Eqs B-1 to B-10).
    pub fn brownian(&self, di: f64, dj: f64) -> f64 {
        let pi = std::f64::consts::PI;
        let (d_i, d_j) = (self.diffusivity(di), self.diffusivity(dj));
        let speed = |d: f64| (8.0 * K_B * self.t_k / (pi * self.rho_p * pi * d * d * d / 6.0)).sqrt();
        let (v_i, v_j) = (speed(di), speed(dj));
        let g = |d: f64, dif: f64, v: f64| {
            let l = 8.0 * dif / (pi * v);
            ((d + l).r_powi(3) - (d * d + l * l).r_powf(1.5)) / (3.0 * d * l) - d
        };
        let g_ij = (g(di, d_i, v_i).r_powi(2) + g(dj, d_j, v_j).r_powi(2)).sqrt();
        let v_ij = (v_i * v_i + v_j * v_j).sqrt();
        let s = di + dj;
        let bracket = s / (s + 2.0 * g_ij) + 8.0 * (d_i + d_j) / (v_ij * s * self.sticking);
        2.0 * pi * (d_i + d_j) * (self.gamma * di + self.gamma * dj) / bracket
    }

    /// Gravitational kernel, m^3/s (Eqs B-13 to B-15).
    pub fn gravitational(&self, di: f64, dj: f64) -> f64 {
        let eps = 1.5 * di.min(dj).r_powi(2) / (di + dj).r_powi(2);
        eps * std::f64::consts::PI / 4.0 * self.sticking * (self.gamma * (di + dj)).r_powi(2) * (self.settling_velocity(di) - self.settling_velocity(dj)).abs()
    }

    /// Turbulent kernel, shear and inertial in quadrature, m^3/s (Eqs B-16
    /// to B-18).
    pub fn turbulent(&self, di: f64, dj: f64) -> f64 {
        let rho_g = self.rho_gas();
        let e = self.dissipation;
        let sum3 = (self.gamma * di + self.gamma * dj).r_powi(3);
        let b1 = (std::f64::consts::PI * e * rho_g / (120.0 * self.mu_pa_s)).sqrt() * sum3;
        let a = |d: f64| self.rho_p * self.slip_correction(d) * d * d / self.chi;
        let b2 = 0.04029 * rho_g.r_powf(0.25) * e.r_powf(0.75) / self.mu_pa_s.r_powf(1.25) * (self.gamma * di + self.gamma * dj).r_powi(2) * (a(di) - a(dj)).abs();
        self.sticking * (b1 * b1 + b2 * b2).sqrt()
    }

    /// The total kernel (Eq 2-40).
    pub fn kernel(&self, di: f64, dj: f64) -> f64 {
        self.brownian(di, dj) + self.gravitational(di, dj) + self.turbulent(di, dj)
    }

    /// The total kernel for every pair of the diameters `d`, m^3/s: the
    /// same equations as [`Medium::kernel`], with each particle's own
    /// quantities (slip, diffusivity, speed, Fuchs' `g`, terminal velocity)
    /// computed once instead of once per pair.
    pub fn kernel_table(&self, d: &[f64]) -> Vec<Vec<f64>> {
        let pi = std::f64::consts::PI;
        let lambda = self.mean_free_path();
        let rho_g = self.rho_gas();
        struct P {
            dif: f64,
            v: f64,
            g: f64,
            vt: f64,
            a: f64,
        }
        let props: Vec<P> = d
            .iter()
            .map(|&di| {
                let kn = 2.0 * lambda / di;
                let c = 1.0 + kn * (self.slip + 0.4 * (-1.1 / kn).r_exp());
                let dif = K_B * self.t_k / (3.0 * pi * di * self.mu_pa_s * self.chi) * c;
                let v = (8.0 * K_B * self.t_k / (pi * self.rho_p * pi * di * di * di / 6.0)).sqrt();
                let l = 8.0 * dif / (pi * v);
                let g = ((di + l).r_powi(3) - (di * di + l * l).r_powf(1.5)) / (3.0 * di * l) - di;
                let vt = self.rho_p * G * di * di * c / (18.0 * self.mu_pa_s * self.chi);
                let a = self.rho_p * c * di * di / self.chi;
                P { dif, v, g, vt, a }
            })
            .collect();
        let e = self.dissipation;
        let t1 = (pi * e * rho_g / (120.0 * self.mu_pa_s)).sqrt();
        let t2 = 0.04029 * rho_g.r_powf(0.25) * e.r_powf(0.75) / self.mu_pa_s.r_powf(1.25);
        (0..d.len())
            .map(|i| {
                (0..d.len())
                    .map(|j| {
                        let (pi_, pj) = (&props[i], &props[j]);
                        let s = d[i] + d[j];
                        let gs = self.gamma * d[i] + self.gamma * d[j];
                        let g_ij = (pi_.g * pi_.g + pj.g * pj.g).sqrt();
                        let v_ij = (pi_.v * pi_.v + pj.v * pj.v).sqrt();
                        let bracket = s / (s + 2.0 * g_ij) + 8.0 * (pi_.dif + pj.dif) / (v_ij * s * self.sticking);
                        let brown = 2.0 * pi * (pi_.dif + pj.dif) * gs / bracket;
                        let eps = 1.5 * d[i].min(d[j]).r_powi(2) / s.r_powi(2);
                        let grav = eps * pi / 4.0 * self.sticking * (self.gamma * s).r_powi(2) * (pi_.vt - pj.vt).abs();
                        let b1 = t1 * gs.r_powi(3);
                        let b2 = t2 * gs.r_powi(2) * (pi_.a - pj.a).abs();
                        brown + grav + self.sticking * (b1 * b1 + b2 * b2).sqrt()
                    })
                    .collect()
            })
            .collect()
    }
}

/// Diffusiophoretic velocity toward a surface, m/s, with the Stefan flow
/// (Eqs 2-45, 2-46): `w_cond` the steam condensation flux onto it,
/// kg/(m^2 s) (negative evaporating), `x_steam` the bulk steam mole
/// fraction, `m_steam`, `m_nc` molar masses (any one unit), `rho_bulk` the
/// bulk gas density and `rho_sat` the saturated vapour density, kg/m^3.
pub fn diffusiophoretic_velocity(w_cond: f64, x_steam: f64, m_steam: f64, m_nc: f64, rho_bulk: f64, rho_sat: f64) -> f64 {
    if w_cond >= 0.0 {
        m_steam / (x_steam * m_steam + (1.0 - x_steam) * m_nc) * w_cond / rho_bulk
    } else {
        w_cond / rho_sat
    }
}

/// How a deposition surface faces (RN §2.4.2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    /// Upward-facing: floors and pool surfaces.
    Floor,
    Wall,
    /// Downward-facing.
    Ceiling,
}

/// A deposition surface: area, m^2, facing, and the phoretic velocities it
/// drives, m/s, given per section by the caller (thermophoresis from its
/// heat flux, diffusiophoresis from its condensation; zero for none).
#[derive(Clone, Debug, PartialEq)]
pub struct Surface {
    pub area: f64,
    pub facing: Facing,
    pub phoretic: Vec<f64>,
}

/// A sectional size distribution in one well-mixed volume: representative
/// particle volumes `v_k` (fixed ratio), and the aerosol volume
/// concentration in each, m^3 of particle per m^3 of gas.
#[derive(Clone, Debug, PartialEq)]
pub struct Sections {
    pub v: Vec<f64>,
    pub conc: Vec<f64>,
    /// Particle volume that outgrew the top section, m^3 per m^3 of gas,
    /// accumulated.
    pub fallout: f64,
}

impl Sections {
    /// `n` sections from diameter `d_min` with particle-volume ratio
    /// `ratio` (at least 2), empty.
    pub fn new(d_min: f64, n: usize, ratio: f64) -> Self {
        assert!(ratio >= 2.0, "the volume ratio must be at least 2 (Eq 2-37's analogue)");
        let v0 = std::f64::consts::PI / 6.0 * d_min.r_powi(3);
        let v = (0..n).map(|k| v0 * ratio.r_powi(k as i32)).collect();
        Self { v, conc: vec![0.0; n], fallout: 0.0 }
    }
    pub fn diameter(&self, k: usize) -> f64 {
        (6.0 * self.v[k] / std::f64::consts::PI).r_powf(1.0 / 3.0)
    }
    /// Number concentration in section `k`, 1/m^3.
    pub fn number(&self, k: usize) -> f64 {
        self.conc[k] / self.v[k]
    }

    /// Jacobson et al.'s volume fraction of a coagulated particle of volume
    /// `vij` assigned to section `k` of `v`, the sections extended by one
    /// virtual section above the top (the fallout).
    fn fraction(v: &[f64], vij: f64, k: usize) -> f64 {
        let n = v.len();
        if k + 1 < n && vij >= v[k] && vij < v[k + 1] {
            (v[k + 1] - vij) / (v[k + 1] - v[k]) * v[k] / vij
        } else if k > 0 && vij > v[k - 1] && vij < v[k] {
            1.0 - Self::fraction(v, vij, k - 1)
        } else if k + 1 == n && vij >= v[k] {
            1.0
        } else {
            0.0
        }
    }

    /// One semi-implicit coagulation step of `dt` s with the kernel
    /// `beta(i, j)`, m^3/s (Jacobson et al. 1994, their Eq 8): volume is
    /// conserved to round-off and the step is stable for any `dt`.
    pub fn coagulate<F: Fn(usize, usize) -> f64>(&mut self, dt: f64, beta: F) {
        let n = self.v.len();
        let mut ext = self.v.clone();
        ext.push(self.v[n - 1] * self.v[n - 1] / self.v[n - 2]);
        let old: f64 = self.conc.iter().sum();
        let n_old: Vec<f64> = (0..n).map(|k| self.number(k)).collect();
        let b: Vec<Vec<f64>> = (0..n).map(|i| (0..n).map(|j| beta(i, j)).collect()).collect();
        // A coagulated pair (i, j) lands only in the two sections around
        // v_i + v_j (the fractions vanish elsewhere), so bucket each pair's
        // gain by its target: O(n^2) instead of looping over every (i, j,
        // k). The terms and their order of use are Jacobson et al.'s.
        let mut gains: Vec<Vec<(usize, usize, f64)>> = vec![Vec::new(); n];
        for i in 0..n {
            for j in 0..n {
                let vij = self.v[i] + self.v[j];
                let lo = ext.partition_point(|&x| x <= vij).saturating_sub(1);
                for k in [lo, lo + 1] {
                    // Jacobson's sums run over j <= k and i < k.
                    if k < n && j <= k && i < k {
                        let f = Self::fraction(&ext, vij, k);
                        if f > 0.0 {
                            gains[k].push((i, j, f));
                        }
                    }
                }
            }
        }
        let mut new = vec![0.0; n];
        for k in 0..n {
            let gain: f64 = gains[k].iter().map(|&(i, j, f)| f * b[i][j] * new[i] * n_old[j]).sum();
            let mut loss = 0.0;
            for j in 0..n {
                loss += (1.0 - Self::fraction(&ext, self.v[k] + self.v[j], k)) * b[k][j] * n_old[j];
            }
            new[k] = (self.conc[k] + dt * gain) / (1.0 + dt * loss);
        }
        // What left the real sections went to the virtual one: fallout.
        self.fallout += old - new.iter().sum::<f64>();
        self.conc = new;
    }

    /// Deposition for `dt` s onto `surfaces` of a volume of `volume` m^3:
    /// per section, the sum over surfaces of area times the net velocity
    /// (gravitational by facing, Brownian diffusion, the surface's
    /// phoretic velocity), each surface's net floored at zero, applied
    /// exactly. Returns the particle volume deposited on each surface, m^3
    /// per m^3 of gas.
    pub fn deposit(&mut self, medium: &Medium, surfaces: &[Surface], volume: f64, dt: f64) -> Vec<f64> {
        let mut out = vec![0.0; surfaces.len()];
        for k in 0..self.v.len() {
            let d = self.diameter(k);
            let (vg, vd) = (medium.settling_velocity(d), medium.diffusion_velocity(d));
            let rates: Vec<f64> = surfaces
                .iter()
                .map(|s| {
                    let grav = match s.facing {
                        Facing::Floor => vg,
                        Facing::Wall => 0.0,
                        Facing::Ceiling => -vg,
                    };
                    s.area * (grav + vd + s.phoretic[k]).max(0.0) / volume
                })
                .collect();
            let total: f64 = rates.iter().sum();
            if total <= 0.0 {
                continue;
            }
            let gone = self.conc[k] * (1.0 - (-total * dt).r_exp());
            for (o, r) in out.iter_mut().zip(&rates) {
                *o += gone * r / total;
            }
            self.conc[k] -= gone;
        }
        out
    }

    /// Gravitational settling for `dt` s onto a floor of `area` m^2 under a
    /// volume of `volume` m^3: each section decays at `v_grav A / V`, done
    /// exactly. Returns the particle volume deposited, m^3 per m^3 of gas.
    pub fn settle(&mut self, medium: &Medium, area: f64, volume: f64, dt: f64) -> f64 {
        let mut out = 0.0;
        for k in 0..self.v.len() {
            let keep = (-medium.settling_velocity(self.diameter(k)) * area / volume * dt).r_exp();
            out += self.conc[k] * (1.0 - keep);
            self.conc[k] *= keep;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn air() -> Medium {
        Medium::air(298.15, 1.013e5, 1.84e-5)
    }

    /// The kernels against their limits and hand values:
    /// - the mean free path in air at 298 K is about 0.066 um;
    /// - Brownian, equal 10 um particles (continuum): `8 k T / (3 mu)`
    ///   times the slip correction, within 3 %;
    /// - Brownian, equal 2 nm particles (free molecular):
    ///   `pi (2d)^2 sqrt(2) v / 4` with `v = sqrt(8kT/(pi m))`, within 5 %;
    /// - gravitational vanishes for equal sizes and is symmetric;
    /// - the settling velocity of a 10 um, 1000 kg/m^3 sphere is about
    ///   3.0e-3 m/s.
    ///
    /// Results (2026-10-29): mean free path 0.0662 um; continuum ratio
    /// 0.9935; free-molecular ratio 0.9997; settling 3.010e-3 m/s. At 1 and
    /// 10 um the three kernels are of one order (Brownian 2.1e-15,
    /// gravitational 3.5e-15, turbulent 3.0e-15 m^3/s). Tolerances were
    /// set before the first run (3 %, 5 %) and tightened after it.
    #[test]
    fn the_maeros_kernels_reach_their_limits() {
        let m = air();
        let lambda = m.mean_free_path();
        assert!((lambda - 0.066e-6).abs() < 0.004e-6, "{lambda}");
        let d = 10e-6;
        let cont = 8.0 * K_B * m.t_k / (3.0 * m.mu_pa_s) * m.slip_correction(d);
        assert!((m.brownian(d, d) / cont - 1.0).abs() < 0.01, "{} vs {cont}", m.brownian(d, d));
        let d = 2e-9;
        let pi = std::f64::consts::PI;
        let v = (8.0 * K_B * m.t_k / (pi * m.rho_p * pi * d * d * d / 6.0)).sqrt();
        let fm = pi * (2.0 * d) * (2.0 * d) * 2f64.sqrt() * v / 4.0;
        assert!((m.brownian(d, d) / fm - 1.0).abs() < 0.005, "{} vs {fm}", m.brownian(d, d));
        assert_eq!(m.gravitational(3e-6, 3e-6), 0.0);
        assert!((m.gravitational(1e-6, 5e-6) - m.gravitational(5e-6, 1e-6)).abs() < 1e-30);
        let ds = [0.1e-6, 1e-6, 7e-6];
        let table = m.kernel_table(&ds);
        for i in 0..3 {
            for j in 0..3 {
                assert!((table[i][j] / m.kernel(ds[i], ds[j]) - 1.0).abs() < 1e-12, "{i} {j}");
            }
        }
        let vs = m.settling_velocity(10e-6);
        assert!((vs - 3.0e-3).abs() < 0.15e-3, "{vs}");
    }

    /// The sectional scheme against Smoluchowski's analytic result for a
    /// constant kernel K: the number falls as `N0 / (1 + K N0 t / 2)`. From
    /// a monodisperse start in the first of 40 sections (volume ratio 2),
    /// to a quarter of the initial number, within 3 %; volume conserved to
    /// 1e-12. And settling decays each section exactly exponentially.
    ///
    /// Result (2026-10-29): the number at `t = 6/(K N0)` is 1.00055 times
    /// the analytic value (tolerance set at 3 % before the run, tightened
    /// to 0.5 % after it).
    #[test]
    fn the_sectional_scheme_matches_smoluchowski_and_settles_exactly() {
        let mut s = Sections::new(0.1e-6, 40, 2.0);
        let n0 = 1.0e12;
        s.conc[0] = n0 * s.v[0];
        let k = 1.0e-15;
        let v_total: f64 = s.conc.iter().sum();
        let t_end = 6.0 / (k * n0); // N/N0 = 1/(1+3) = 0.25
        let steps = 600;
        for _ in 0..steps {
            s.coagulate(t_end / steps as f64, |_, _| k);
        }
        let n: f64 = (0..40).map(|i| s.number(i)).sum();
        let exact = n0 / (1.0 + k * n0 * t_end / 2.0);
        assert!((n / exact - 1.0).abs() < 0.005, "{} vs {exact}", n);
        assert!((s.conc.iter().sum::<f64>() / v_total - 1.0).abs() < 1e-12);
        let m = air();
        let mut t = Sections::new(1e-6, 3, 8.0);
        t.conc = vec![1.0, 1.0, 1.0];
        let out = t.settle(&m, 10.0, 100.0, 50.0);
        for i in 0..3 {
            let exact = (-m.settling_velocity(t.diameter(i)) * 0.1 * 50.0).r_exp();
            assert!((t.conc[i] - exact).abs() < 1e-12);
        }
        assert!((out + t.conc.iter().sum::<f64>() - 3.0).abs() < 1e-12);
    }

    /// Fallout and the deposition terms:
    /// - with three sections and a large constant kernel, volume in the
    ///   sections plus fallout is conserved to 1e-12, and fallout is
    ///   positive;
    /// - Brownian diffusion velocity for 0.1 um in air with a 10 um layer:
    ///   `kT C/(3 pi mu d Delta)` = 4.116e-21 x 2.895 / 1.734e-16 = 6.87e-5
    ///   m/s by hand (first written as 2.0e-5, an arithmetic slip caught by
    ///   this test);
    /// - thermophoresis is zero with no gradient and scales with it;
    /// - diffusiophoresis in pure steam condensing at 1e-3 kg/(m^2 s) into
    ///   0.6 kg/m^3 gas is `w/rho` = 1.67e-3 m/s;
    /// - a ceiling takes nothing from large particles (gravity beats
    ///   diffusion), a floor takes more than a wall, and the volume
    ///   deposited plus what remains is conserved.
    #[test]
    fn fallout_and_deposition_follow_melcor() {
        let mut s = Sections::new(1e-6, 3, 2.0);
        s.conc = vec![1e-6, 1e-6, 1e-6];
        for _ in 0..50 {
            s.coagulate(1.0, |_, _| 1e-6);
        }
        assert!(s.fallout > 0.0 && (s.conc.iter().sum::<f64>() + s.fallout - 3e-6).abs() < 1e-18, "{s:?}");
        let m = air();
        let vd = m.diffusion_velocity(0.1e-6);
        let hand = K_B * m.t_k * m.slip_correction(0.1e-6) / (3.0 * std::f64::consts::PI * m.mu_pa_s * 0.1e-6 * 1e-5);
        assert!((vd - hand).abs() < 1e-15 && (vd - 6.87e-5).abs() < 0.02e-5, "{vd} {}", m.slip_correction(0.1e-6));
        assert_eq!(m.thermophoretic_velocity(1e-6, 0.0, 400.0), 0.0);
        let t1 = m.thermophoretic_velocity(1e-6, 100.0, 400.0);
        assert!(t1 > 0.0 && (m.thermophoretic_velocity(1e-6, 200.0, 400.0) / t1 - 2.0).abs() < 1e-12);
        assert!((diffusiophoretic_velocity(1e-3, 1.0, 18.0, 28.0, 0.6, 0.5) - 1e-3 / 0.6).abs() < 1e-15);
        let mut big = Sections::new(10e-6, 1, 2.0);
        big.conc = vec![1.0];
        let surfaces = [
            Surface { area: 10.0, facing: Facing::Floor, phoretic: vec![0.0] },
            Surface { area: 10.0, facing: Facing::Wall, phoretic: vec![0.0] },
            Surface { area: 10.0, facing: Facing::Ceiling, phoretic: vec![0.0] },
        ];
        let out = big.deposit(&m, &surfaces, 100.0, 60.0);
        assert!(out[2] == 0.0 && out[0] > out[1] && out[1] > 0.0, "{out:?}");
        assert!((out.iter().sum::<f64>() + big.conc[0] - 1.0).abs() < 1e-12);
    }
}
