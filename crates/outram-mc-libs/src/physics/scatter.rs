//! Neutron scattering kinematics — elastic and inelastic.
//!
//! C++ source: `src/physics_common.cpp`, `src/physics.cpp`.
//!
//! The channels differ in both their outgoing *energy* law and their angular law.
//! Elastic scatter can use an anisotropic centre-of-mass distribution (ENDF MF=4,
//! sampled by the caller and passed as `mu_cm`) — the dominant reactivity lever for
//! a bare fast-metal sphere, where forward-peaked elastic off heavy nuclei sets the
//! transport cross section and hence the leakage. The inelastic channels remain
//! isotropic-CM in angle for now. By outgoing-energy law:
//!
//! - **Elastic** (MT=2) — [`elastic_scatter`]: two-body kinematics with `Q = 0`;
//!   off a heavy actinide the neutron loses almost no energy per collision
//!   (α = ((A−1)/(A+1))² ≈ 0.98 for A ≈ 238).
//! - **Discrete-level inelastic** (MT=51…90) — [`two_body_scatter`] with the
//!   level's `Q < 0`: the neutron gives up the level excitation energy, a *large*
//!   per-collision energy loss (tens of keV to MeV) that softens the fast
//!   spectrum. This is the dominant fast-spectrum energy-loss mechanism for heavy
//!   nuclei, and its absence (inelastic lumped into elastic) was the leading bias
//!   in the first Godiva Keff — see `docs/development-history.md`.
//! - **Continuum inelastic** (MT=91) — [`continuum_inelastic_scatter`]: the
//!   outgoing energy is a distribution, not fixed by a single `Q`. RECONR does not
//!   reconstruct the ENDF MF=5 continuum law, so this uses a **Weisskopf
//!   evaporation** model with a nuclear temperature θ = √(E/a), level-density
//!   parameter a ≈ A/11 MeV⁻¹ (actinide) — an approximation, documented as such.
//!
//! Anisotropic elastic uses the full ENDF MF=4 tabulated cosine distribution
//! (sampled in `material::nuclide`, ported from OpenMC), passed here as a CM cosine
//! via [`two_body_scatter_with_mu`]. Anisotropic *inelastic* angular laws (coupled
//! to the MF=5/MF=6 energy distributions) remain future work.

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

/// Convert a neutron's centre-of-mass outgoing energy and CM scattering cosine to
/// the laboratory frame, for a target of atomic weight ratio `awr` initially at
/// rest.
///
/// `e` is the incident lab energy \[eV\], `e_cm_out` the outgoing neutron energy
/// in the CM frame \[eV\], `mu_cm` the CM scattering cosine. Returns
/// `(e_out_lab, mu_lab)`. The CM frame moves with the incident neutron, carrying a
/// unit-mass "translational" energy `E/(A+1)²`; the lab energy is the vector sum:
///
/// `E' = E_cm + E/(A+1)² + 2·μ_cm·√(E_cm · E/(A+1)²)`,
/// `μ_lab = μ_cm·√(E_cm/E') + √(E/(A+1)²/E')`.
///
/// With `e_cm_out = E·(A/(A+1))²` (elastic) this reduces to the familiar
/// `E' = E·(A² + 2Aμ + 1)/(A+1)²`.
fn cm_to_lab(e: f64, e_cm_out: f64, mu_cm: f64, awr: f64) -> (f64, f64) {
    let ap1 = awr + 1.0;
    let e_trans = e / (ap1 * ap1); // unit-mass energy at the CM velocity
    let cross = 2.0 * mu_cm * (e_cm_out * e_trans).sqrt();
    let e_out = (e_cm_out + e_trans + cross).max(0.0);
    let mu_lab = if e_out > 0.0 {
        (mu_cm * (e_cm_out / e_out).sqrt() + (e_trans / e_out).sqrt()).clamp(-1.0, 1.0)
    } else {
        1.0
    };
    (e_out, mu_lab)
}

/// Two-body scatter a neutron of energy `e` \[eV\] and direction `u` off a target
/// of atomic weight ratio `awr` with reaction Q-value `q` \[eV\], isotropic in the
/// centre-of-mass frame.
///
/// Returns `(e_out, u_out)`. Two-body kinematics fix the outgoing neutron CM
/// energy from the Q-value (target at rest):
///
/// `E_cm = E·(A/(A+1))² + Q·A/(A+1)`,
///
/// which is then transformed to the lab via [`cm_to_lab`]. `Q = 0` is elastic;
/// `Q < 0` is endothermic (a discrete inelastic level of excitation energy |Q|,
/// with threshold `E = |Q|·(A+1)/A`). Below threshold `E_cm` would be negative;
/// it is clamped to zero, but the caller should not select a channel below its
/// (zero) cross section there.
///
/// For an anisotropic CM angular law use [`two_body_scatter_with_mu`].
pub fn two_body_scatter(
    e: f64,
    u: Direction,
    awr: f64,
    q: f64,
    seed: &mut u64,
) -> (f64, Direction) {
    let mu_cm = 2.0 * prn(seed) - 1.0; // isotropic in CM
    two_body_scatter_with_mu(e, u, awr, q, mu_cm, seed)
}

/// Two-body scatter with a **caller-supplied** centre-of-mass scattering cosine
/// `mu_cm` — the anisotropic form of [`two_body_scatter`].
///
/// Identical to [`two_body_scatter`] except the CM cosine is provided rather than
/// sampled isotropically: the outgoing CM energy is fixed by the Q-value and
/// [`cm_to_lab`] maps it to the lab. Use this when an angular distribution (ENDF
/// MF=4, sampled elsewhere) supplies `mu_cm`; only the azimuth is sampled here.
///
/// `mu_cm` is the cosine in the **CM frame** — the frame ENDF elastic angular
/// distributions are given in (LCT=2) — and is clamped to `[−1, 1]`.
pub fn two_body_scatter_with_mu(
    e: f64,
    u: Direction,
    awr: f64,
    q: f64,
    mu_cm: f64,
    seed: &mut u64,
) -> (f64, Direction) {
    let a = awr;
    let ap1 = a + 1.0;
    let e_cm_out = (e * (a / ap1).powi(2) + q * a / ap1).max(0.0);
    let (e_out, mu_lab) = cm_to_lab(e, e_cm_out, mu_cm.clamp(-1.0, 1.0), a);
    (e_out, rotate_direction(u, mu_lab, seed))
}

/// Elastic scatter a neutron of energy `e` \[eV\] and direction `u` off a target
/// of atomic weight ratio `awr`, isotropic in the centre-of-mass frame.
///
/// The `Q = 0` special case of [`two_body_scatter`]. Outgoing energy stays in
/// `[α·E, E]` with `α = ((A−1)/(A+1))²`; off heavy actinides that is a per-collision
/// loss of at most a couple of percent.
pub fn elastic_scatter(e: f64, u: Direction, awr: f64, seed: &mut u64) -> (f64, Direction) {
    two_body_scatter(e, u, awr, 0.0, seed)
}

/// Continuum inelastic scatter (MT=91) — the outgoing neutron energy is sampled
/// from a **Weisskopf evaporation spectrum** rather than fixed by a single level.
///
/// `f(E'_cm) ∝ E'_cm · exp(−E'_cm/θ)` with nuclear temperature `θ = √(E/a)` and
/// level-density parameter `a ≈ A/11 MeV⁻¹` (a standard actinide value). The
/// sampled CM energy is capped below the elastic CM energy `E·(A/(A+1))²` so the
/// collision always loses energy, then transformed to the lab isotropically in CM.
///
/// This is an **approximation**: RECONR reconstructs cross sections (MF=3) but not
/// the ENDF MF=5 secondary-energy law, so the true continuum distribution is not
/// available here. The evaporation model captures the essential physics — a large,
/// broadly distributed down-scatter — which is what softens the fast spectrum.
pub fn continuum_inelastic_scatter(
    e: f64,
    u: Direction,
    awr: f64,
    seed: &mut u64,
) -> (f64, Direction) {
    let a = awr;
    let ap1 = a + 1.0;
    let e_cm_elastic = e * (a / ap1).powi(2); // max neutron CM energy (no loss)

    // Weisskopf temperature θ = √(E/a_ld), a_ld ≈ A/11 MeV⁻¹.
    let a_ld = (a / 11.0).max(1.0); // MeV⁻¹
    let theta = ((e * 1.0e-6 / a_ld).sqrt() * 1.0e6).max(1.0); // eV

    // Sample E'_cm ~ Gamma(shape 2, scale θ) = −θ·ln(r1·r2), rejecting any draw
    // that would not lose energy. Fall back to a uniform down-scatter if the
    // (rare) rejection loop is exhausted.
    let mut e_cm_out = e_cm_elastic * prn(seed);
    for _ in 0..64 {
        let cand = -theta * (prn(seed) * prn(seed)).ln();
        if cand <= e_cm_elastic {
            e_cm_out = cand;
            break;
        }
    }

    let mu_cm = 2.0 * prn(seed) - 1.0; // isotropic in CM
    let (e_out, mu_lab) = cm_to_lab(e, e_cm_out, mu_cm, a);
    (e_out, rotate_direction(u, mu_lab, seed))
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

    /// Discrete-level inelastic (Q < 0) removes at least the excitation energy from
    /// the available energy: the outgoing energy is well below the incident, and
    /// never exceeds it. Uses a U-238-like first level (Q ≈ −45 keV).
    #[test]
    fn inelastic_level_loses_at_least_the_excitation() {
        let mut seed = 12345u64;
        let awr = 236.0_f64;
        let q = -45.0e3; // 45 keV level
        let e = 2.0e6;
        // In CM the neutron loses exactly |Q|·A/(A+1); in lab the mean loss is
        // larger still. Check no energy gain and a strict loss on average.
        let mut sum_loss = 0.0;
        for _ in 0..10_000 {
            let (e_out, u) = two_body_scatter(e, Direction::new(0.0, 0.0, 1.0), awr, q, &mut seed);
            assert!(
                e_out <= e * (1.0 + 1e-9),
                "inelastic gained energy: {e_out}"
            );
            let n = (u.u * u.u + u.v * u.v + u.w * u.w).sqrt();
            assert!((n - 1.0).abs() < 1e-12, "direction not unit: {n}");
            sum_loss += e - e_out;
        }
        let mean_loss = sum_loss / 10_000.0;
        assert!(
            mean_loss > 40.0e3,
            "mean inelastic loss {mean_loss} < excitation"
        );
    }

    /// The continuum evaporation channel always loses energy and stays physical
    /// (0 < E' < E), with a mean loss far larger than an elastic collision — the
    /// spectrum-softening effect we are after.
    #[test]
    fn continuum_inelastic_softens_and_is_bounded() {
        let mut seed = 2024u64;
        let awr = 236.0_f64;
        let e = 2.0e6;
        let mut sum_out = 0.0;
        for _ in 0..10_000 {
            let (e_out, _) =
                continuum_inelastic_scatter(e, Direction::new(0.0, 0.0, 1.0), awr, &mut seed);
            assert!(
                e_out > 0.0 && e_out < e * (1.0 + 1e-9),
                "continuum E' out of (0,E]: {e_out}"
            );
            sum_out += e_out;
        }
        let mean_out = sum_out / 10_000.0;
        // Elastic would keep ~99% of E; evaporation must remove much more.
        assert!(
            mean_out < 0.9 * e,
            "continuum too hard: mean E' = {mean_out}"
        );
    }
}
