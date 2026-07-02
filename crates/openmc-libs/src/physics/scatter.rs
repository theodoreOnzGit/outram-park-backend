//! Elastic neutron scattering kinematics.
//!
//! C++ source: `src/physics_common.cpp`, `src/physics.cpp`.
//!
//! Only **elastic** scattering (MT=2) is ported so far, in the target-at-rest
//! approximation with an **isotropic centre-of-mass** angular law. That is the
//! right first cut for a fast bare-sphere Keff: fast neutrons scatter nearly
//! isotropically in CM off heavy actinides, and thermal free-gas motion is
//! irrelevant far above the target's thermal energy. Anisotropic elastic (a₁
//! from ENDF MF=4) and inelastic channels (MT=51–90) are future work.

use crate::geometry::position::Direction;
use crate::rng::lcg::prn;
use std::f64::consts::PI;

/// Rotate the unit direction `u` by scattering cosine `mu` and a uniformly
/// sampled azimuth φ ∈ [0, 2π), returning the new unit direction.
///
/// This is the standard OpenMC `rotate_angle`: it builds an orthonormal frame
/// around `u` and tilts by `(mu, φ)`. The near-pole branch (`|w| ≈ 1`) rotates
/// about the x-axis instead to avoid dividing by √(1−w²) ≈ 0.
pub fn rotate_direction(u: Direction, mu: f64, seed: &mut u64) -> Direction {
    let phi = 2.0 * PI * prn(seed);
    let (sinphi, cosphi) = phi.sin_cos();
    let a = (1.0 - mu * mu).max(0.0).sqrt();
    let b = (1.0 - u.w * u.w).max(0.0).sqrt();

    if b > 1.0e-10 {
        Direction::new(
            mu * u.u + a * (u.u * u.w * cosphi - u.v * sinphi) / b,
            mu * u.v + a * (u.v * u.w * cosphi + u.u * sinphi) / b,
            mu * u.w - a * b * cosphi,
        )
    } else {
        // Direction is along ±z; rotate about the x-axis using √(1−v²) instead.
        let b = (1.0 - u.v * u.v).max(0.0).sqrt();
        Direction::new(
            mu * u.u + a * (u.u * u.v * cosphi + u.w * sinphi) / b,
            mu * u.v - a * b * cosphi,
            mu * u.w + a * (u.v * u.w * cosphi - u.u * sinphi) / b,
        )
    }
}

/// Elastic scatter a neutron of energy `e` \[eV\] and direction `u` off a target
/// of atomic weight ratio `awr`, isotropic in the centre-of-mass frame.
///
/// Returns `(e_out, u_out)`. With μ_cm uniform on [−1, 1]:
/// - outgoing energy `E' = E·(A² + 2A·μ_cm + 1)/(A+1)²` (target at rest),
/// - lab scattering cosine `μ_lab = (A·μ_cm + 1)/√(A² + 2A·μ_cm + 1)`,
///
/// and the new direction is `u` rotated by `(μ_lab, φ)` via [`rotate_direction`].
pub fn elastic_scatter(e: f64, u: Direction, awr: f64, seed: &mut u64) -> (f64, Direction) {
    let mu_cm = 2.0 * prn(seed) - 1.0;
    let a = awr;
    let denom = (a + 1.0) * (a + 1.0);
    let g = a * a + 2.0 * a * mu_cm + 1.0; // ∝ E'/E · (A+1)²
    let e_out = e * g / denom;
    let mu_lab = (a * mu_cm + 1.0) / g.sqrt();
    let u_out = rotate_direction(u, mu_lab, seed);
    (e_out, u_out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rotated unit vector stays a unit vector, for both the general and the
    /// near-pole branch.
    #[test]
    fn rotate_preserves_unit_norm() {
        let mut seed = 7u64;
        for u in [
            Direction::from_unnormalised(0.3, -0.4, 0.85), // generic, exactly unit
            Direction::new(0.0, 0.0, 1.0),                 // pole
            Direction::new(0.0, 0.0, -1.0),
        ] {
            for _ in 0..1000 {
                let mu = 2.0 * prn(&mut seed) - 1.0;
                let d = rotate_direction(u, mu, &mut seed);
                let n = (d.u * d.u + d.v * d.v + d.w * d.w).sqrt();
                assert!((n - 1.0).abs() < 1e-12, "‖d‖ = {n}");
            }
        }
    }

    /// Elastic scattering off a heavy target loses little energy and never gains
    /// energy (target at rest): E' ∈ [α·E, E] with α = ((A−1)/(A+1))².
    #[test]
    fn elastic_energy_stays_in_bounds() {
        let mut seed = 99u64;
        let awr = 235.0_f64;
        let alpha = ((awr - 1.0) / (awr + 1.0)).powi(2);
        let e = 2.0e6;
        for _ in 0..10_000 {
            let (e_out, _) = elastic_scatter(e, Direction::new(1.0, 0.0, 0.0), awr, &mut seed);
            assert!(e_out <= e * (1.0 + 1e-9), "gained energy: {e_out} > {e}");
            assert!(e_out >= e * alpha * (1.0 - 1e-9), "below α·E: {e_out}");
        }
    }
}
