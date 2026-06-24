use std::f64::consts::PI;

use openmc_libs::rng::{lcg::prn, distributions::sample_exp};
use uom::si::{f64::Ratio, ratio::ratio};

#[derive(Debug, Clone, Copy)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub fn add(&self, other: Vec3) -> Vec3 {
        Vec3 { x: self.x + other.x, y: self.y + other.y, z: self.z + other.z }
    }

    pub fn scale(&self, s: f64) -> Vec3 {
        Vec3 { x: self.x * s, y: self.y * s, z: self.z * s }
    }

    pub fn norm(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn normalize(&self) -> Vec3 {
        let n = self.norm();
        if n == 0.0 {
            Vec3 { x: 1.0, y: 0.0, z: 0.0 }
        } else {
            self.scale(1.0 / n)
        }
    }
}

/// Sample an isotropic direction on the unit sphere.
/// mu = cos(theta) ~ U[-1, 1], phi ~ U[0, 2π]
#[inline]
fn _sample_isotropic_direction(seed: &mut u64) -> Vec3 {
    let mu: f64 = prn(seed) * 2.0 - 1.0;
    let phi: f64 = prn(seed) * 2.0 * PI;
    let sin_theta: f64 = (1.0_f64 - mu * mu).sqrt();
    Vec3 {
        x: sin_theta * phi.cos(),
        y: sin_theta * phi.sin(),
        z: mu,
    }
}

/// Sample an isotropic direction on the unit sphere, returned as a `[Ratio; 3]`.
#[inline]
pub(crate) fn sample_isotropic_direction_into_array(seed: &mut u64) -> [Ratio; 3] {
    let mu: f64 = prn(seed) * 2.0 - 1.0;
    let phi: f64 = prn(seed) * 2.0 * PI;
    let sin_theta: f64 = (1.0_f64 - mu * mu).sqrt();
    [
        Ratio::new::<ratio>(sin_theta * phi.cos()),
        Ratio::new::<ratio>(sin_theta * phi.sin()),
        Ratio::new::<ratio>(mu),
    ]
}

/// Sample a free path length from Exp(sigma_s).
#[inline]
pub(crate) fn sample_free_path(seed: &mut u64, sigma_s: f64) -> f64 {
    sample_exp(seed, sigma_s)
}
