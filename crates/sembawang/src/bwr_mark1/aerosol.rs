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
//! scheme, particles that outgrow the top section stay in it; MELCOR's
//! fallout of particles above the largest size is not built (stated).
//!
//! **Settling** (RN §2.4.2.2) removes each section at `v_grav A/V`, the
//! Stokes velocity with the slip correction over the volume's floor area.
//! Brownian diffusion to walls, thermophoresis and diffusiophoresis are not
//! built yet.

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
}

impl Medium {
    /// Air at `t_k` and `p_pa`, with MELCOR's default aerosol constants.
    pub fn air(t_k: f64, p_pa: f64, mu_pa_s: f64) -> Self {
        Self { t_k, p_pa, molar_mass_g: 28.97, mu_pa_s, rho_p: 1000.0, gamma: 1.0, chi: 1.0, sticking: 1.0, slip: 1.257, dissipation: 0.001 }
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
    fn diffusivity(&self, d: f64) -> f64 {
        K_B * self.t_k / (3.0 * std::f64::consts::PI * d * self.mu_pa_s * self.chi) * self.slip_correction(d)
    }
    /// Stokes terminal velocity with slip, m/s (Eq B-14, and the settling
    /// velocity of RN §2.4.2.2).
    pub fn settling_velocity(&self, d: f64) -> f64 {
        self.rho_p * G * d * d * self.slip_correction(d) / (18.0 * self.mu_pa_s * self.chi)
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
}

/// A sectional size distribution in one well-mixed volume: representative
/// particle volumes `v_k` (fixed ratio), and the aerosol volume
/// concentration in each, m^3 of particle per m^3 of gas.
#[derive(Clone, Debug, PartialEq)]
pub struct Sections {
    pub v: Vec<f64>,
    pub conc: Vec<f64>,
}

impl Sections {
    /// `n` sections from diameter `d_min` with particle-volume ratio
    /// `ratio` (at least 2), empty.
    pub fn new(d_min: f64, n: usize, ratio: f64) -> Self {
        assert!(ratio >= 2.0, "the volume ratio must be at least 2 (Eq 2-37's analogue)");
        let v0 = std::f64::consts::PI / 6.0 * d_min.r_powi(3);
        let v = (0..n).map(|k| v0 * ratio.r_powi(k as i32)).collect();
        Self { v, conc: vec![0.0; n] }
    }
    pub fn diameter(&self, k: usize) -> f64 {
        (6.0 * self.v[k] / std::f64::consts::PI).r_powf(1.0 / 3.0)
    }
    /// Number concentration in section `k`, 1/m^3.
    pub fn number(&self, k: usize) -> f64 {
        self.conc[k] / self.v[k]
    }

    /// Jacobson et al.'s volume fraction of a coagulated particle of volume
    /// `vij` assigned to section `k`.
    fn fraction(&self, vij: f64, k: usize) -> f64 {
        let n = self.v.len();
        let v = &self.v;
        if k + 1 < n && vij >= v[k] && vij < v[k + 1] {
            (v[k + 1] - vij) / (v[k + 1] - v[k]) * v[k] / vij
        } else if k > 0 && vij > v[k - 1] && vij < v[k] {
            1.0 - self.fraction(vij, k - 1)
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
        let n_old: Vec<f64> = (0..n).map(|k| self.number(k)).collect();
        let b: Vec<Vec<f64>> = (0..n).map(|i| (0..n).map(|j| beta(i, j)).collect()).collect();
        let mut new = vec![0.0; n];
        for k in 0..n {
            let mut gain = 0.0;
            for j in 0..=k {
                for i in 0..k {
                    let f = self.fraction(self.v[i] + self.v[j], k);
                    if f > 0.0 {
                        gain += f * b[i][j] * new[i] * n_old[j];
                    }
                }
            }
            let mut loss = 0.0;
            for j in 0..n {
                loss += (1.0 - self.fraction(self.v[k] + self.v[j], k)) * b[k][j] * n_old[j];
            }
            new[k] = (self.conc[k] + dt * gain) / (1.0 + dt * loss);
        }
        self.conc = new;
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
}
