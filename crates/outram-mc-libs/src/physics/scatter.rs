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
//!   (α = ((A−1)/(A+1))² ≈ 0.98 for A ≈ 238). This form holds the target **at
//!   rest**, which confines the outgoing energy to `[α·E, E]`.
//! - **Free-gas elastic** — [`free_gas_elastic_scatter`]: the same collision with
//!   the target's own thermal motion sampled, below `400·kT`. Transport must use
//!   this one: a target at rest can only take energy away, so without it a
//!   neutron population has no Maxwellian fixed point and cools without bound —
//!   the defect recorded as bead `op-50vu`. A nuclide carrying an S(α,β) table
//!   uses that law instead below its cutoff and this one in the band between the
//!   cutoff and `400·kT`.
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

/// Boltzmann constant \[eV K⁻¹\].
pub const K_BOLTZMANN_EV_PER_K: f64 = 8.617_333_262e-5;

/// OpenMC's `FREE_GAS_THRESHOLD` (`src/constants.h`): above `400·kT` the target
/// nucleus may be treated as stationary, because its thermal speed is negligible
/// beside the neutron's. Below it the target's own motion must be sampled or the
/// neutron can never gain energy — see [`free_gas_elastic_scatter`].
pub const FREE_GAS_THRESHOLD: f64 = 400.0;

/// Elastic scatter off a **thermally moving** target — the free-gas kernel.
///
/// [`two_body_scatter_with_mu`] holds the target at rest, so its outgoing energy
/// is confined to `[α·E, E]`: the neutron can only lose energy. That is correct
/// far above the thermal range and **wrong inside it**, where it removes the
/// up-scatter that gives a neutron population its fixed point. Without up-scatter
/// there is no Maxwellian equilibrium at all: a neutron random-walking in such a
/// medium cools without bound (measured: `⟨E⟩ → 1e-27 eV` and below in FLiBe,
/// graphite kernel carbon, O-16 and SiC after 400 collisions at 600 K, against
/// the correct `1.5·kT = 0.0776 eV` — `examples/epithermal_slowing_down.rs`,
/// bead `op-50vu`). Only a nuclide carrying an S(α,β) table escaped, because that
/// law does model lattice recoil.
///
/// This is a port of OpenMC `elastic_scatter` + `sample_target_velocity`
/// (`src/physics.cpp`) in the **constant cross-section (CXS)** approximation:
/// σ is taken as constant over the target velocity distribution, which is exactly
/// consistent with using a Doppler-broadened σ for the collision *rate* — the rate
/// already carries the target motion, and this supplies the matching kinematics.
/// (OpenMC's DBRC refinement, which resamples σ at the relative energy inside a
/// resonance, is a further correction and is not modelled here.)
///
/// `kt_ev` is the material temperature as `k_B·T` \[eV\]; `mu_cm` is the
/// centre-of-mass cosine from the nuclide's ENDF MF=4 law, sampled by the caller
/// at the incident *lab* energy, as OpenMC does. Above `FREE_GAS_THRESHOLD·kT`
/// (and for `awr > 1`) this delegates to the target-at-rest form, so the fast
/// range is bit-for-bit unchanged.
pub fn free_gas_elastic_scatter(
    e: f64,
    u: Direction,
    awr: f64,
    kt_ev: f64,
    mu_cm: f64,
    seed: &mut u64,
) -> (f64, Direction) {
    // Same gate as OpenMC: heavy target, energy well above thermal ⇒ at rest.
    // A non-positive kT (a caller with no temperature) also falls through here,
    // preserving the previous behaviour rather than sampling a degenerate gas.
    if !(kt_ev > 0.0) || (e >= FREE_GAS_THRESHOLD * kt_ev && awr > 1.0) {
        return two_body_scatter_with_mu(e, u, awr, 0.0, mu_cm, seed);
    }

    // Velocities in units where |v| = √E, so E = v·v throughout.
    let vel = e.sqrt();
    let v_n = [vel * u.u, vel * u.v, vel * u.w];
    let v_t = sample_target_velocity(e, u, awr, kt_ev, seed);

    let ap1 = awr + 1.0;
    let v_cm = [
        (v_n[0] + awr * v_t[0]) / ap1,
        (v_n[1] + awr * v_t[1]) / ap1,
        (v_n[2] + awr * v_t[2]) / ap1,
    ];
    let v_rel = [v_n[0] - v_cm[0], v_n[1] - v_cm[1], v_n[2] - v_cm[2]];
    let speed = (v_rel[0] * v_rel[0] + v_rel[1] * v_rel[1] + v_rel[2] * v_rel[2]).sqrt();
    if !(speed > 0.0) {
        // Neutron and target moving together — no relative motion, no scatter.
        return (e, u);
    }

    // Rotate the relative-velocity direction by the CM cosine, then boost back.
    let dir_rel = Direction::new(v_rel[0] / speed, v_rel[1] / speed, v_rel[2] / speed);
    let dir_out = rotate_direction(dir_rel, mu_cm.clamp(-1.0, 1.0), seed);
    let v_out = [
        speed * dir_out.u + v_cm[0],
        speed * dir_out.v + v_cm[1],
        speed * dir_out.w + v_cm[2],
    ];

    let e_out = v_out[0] * v_out[0] + v_out[1] * v_out[1] + v_out[2] * v_out[2];
    if !(e_out > 0.0) {
        return (e, u);
    }
    let s = e_out.sqrt();
    (
        e_out,
        Direction::new(v_out[0] / s, v_out[1] / s, v_out[2] / s),
    )
}

/// Sample the target nucleus velocity for a free-gas elastic collision, in the
/// same `|v| = √E` units as the neutron.
///
/// Port of OpenMC `sample_target_velocity` (`src/physics.cpp`). The target speed
/// is drawn from the Maxwellian weighted by the relative speed `|v_n − v_t|`
/// (which is what makes fast targets more likely to be hit), via the standard
/// two-branch sampling plus rejection: with probability `α` from `p(y) ∝ y e^{−y²}`,
/// otherwise from `p(y) ∝ y² e^{−y²}`, each accepted with probability
/// `|v_n − v_t| / (v_n + v_t)`.
///
/// The returned velocity is isotropic in azimuth about `u` and makes cosine `mu`
/// with it, `mu` being sampled jointly with the speed by the same rejection.
fn sample_target_velocity(e: f64, u: Direction, awr: f64, kt_ev: f64, seed: &mut u64) -> [f64; 3] {
    // β·v_n = √(A·E / kT) — the neutron speed in units of the target's thermal one.
    let beta_vn = (awr * e / kt_ev).sqrt();
    let alpha = 1.0 / (1.0 + PI.sqrt() * beta_vn / 2.0);

    let (mut beta_vt_sq, mut mu) = (0.0_f64, 0.0_f64);
    // The rejection accepts with probability ≥ (v_n − v_t)/(v_n + v_t) and in
    // practice within a handful of trials; the bound keeps a pathological RNG
    // from hanging transport, and falling out with the last draw is unbiased to
    // the accuracy of that bound.
    for _ in 0..1024 {
        beta_vt_sq = if prn(seed) < alpha {
            // p(y) ∝ y·e^{−y²}: y² = −ln(r₁r₂).
            -(safe_prn(seed) * safe_prn(seed)).ln()
        } else {
            // p(y) ∝ y²·e^{−y²}.
            let c = (PI / 2.0 * prn(seed)).cos();
            -safe_prn(seed).ln() - safe_prn(seed).ln() * c * c
        };
        let beta_vt = beta_vt_sq.sqrt();
        mu = 2.0 * prn(seed) - 1.0;
        let denom = beta_vn + beta_vt;
        let accept = if denom > 0.0 {
            (beta_vn * beta_vn + beta_vt_sq - 2.0 * beta_vn * beta_vt * mu)
                .max(0.0)
                .sqrt()
                / denom
        } else {
            1.0
        };
        if prn(seed) < accept {
            break;
        }
    }

    // Back to |v| = √E units: E_t = β²v_t² · kT / A.
    let vt = (beta_vt_sq * kt_ev / awr).sqrt();
    let u_t = rotate_direction(u, mu.clamp(-1.0, 1.0), seed);
    [vt * u_t.u, vt * u_t.v, vt * u_t.w]
}

/// A uniform strictly inside `(0, 1)`, so `ln` of it is always finite.
#[inline]
fn safe_prn(seed: &mut u64) -> f64 {
    let r = prn(seed);
    if r > 0.0 {
        r
    } else {
        f64::MIN_POSITIVE
    }
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

    // ── Free-gas target motion (bead op-50vu) ────────────────────────────────

    /// 600 K, the FHR pebble temperature: `kT` and the two reference means the
    /// tests below check against.
    const KT_600: f64 = K_BOLTZMANN_EV_PER_K * 600.0;

    fn z_dir() -> Direction {
        Direction::new(0.0, 0.0, 1.0)
    }

    /// Above `400·kT` the free-gas kernel must be the *same function* as the
    /// target-at-rest one, not merely close: the fast range is unchanged by this
    /// work, and that claim is worth pinning rather than asserting.
    #[test]
    fn free_gas_is_target_at_rest_above_the_threshold() {
        let (awr, mu_cm) = (11.893_65, 0.37);
        for &e in &[FREE_GAS_THRESHOLD * KT_600, 1.0e3, 1.0e6] {
            let mut s1 = 4_242_424_242_u64;
            let mut s2 = 4_242_424_242_u64;
            let (e_fg, u_fg) = free_gas_elastic_scatter(e, z_dir(), awr, KT_600, mu_cm, &mut s1);
            let (e_ar, u_ar) = two_body_scatter_with_mu(e, z_dir(), awr, 0.0, mu_cm, &mut s2);
            assert_eq!(e_fg, e_ar, "E = {e}: outgoing energy must be bit-identical");
            assert_eq!((u_fg.u, u_fg.v, u_fg.w), (u_ar.u, u_ar.v, u_ar.w));
        }
    }

    /// A non-positive `kT` means the caller has no temperature to offer; the
    /// kernel must then behave exactly as it did before rather than sampling a
    /// degenerate gas (which would divide by zero in `beta_vn`).
    #[test]
    fn free_gas_falls_back_when_no_temperature_is_given() {
        let mut s1 = 99_u64;
        let mut s2 = 99_u64;
        let a = free_gas_elastic_scatter(0.0253, z_dir(), 11.9, 0.0, 0.1, &mut s1);
        let b = two_body_scatter_with_mu(0.0253, z_dir(), 11.9, 0.0, 0.1, &mut s2);
        assert_eq!(a.0, b.0);
    }

    /// The defect this kernel fixes: a target held at rest can only take energy
    /// away, so a thermal neutron never gains any. A real free gas up-scatters a
    /// large fraction of the time at `E ≈ kT/2`.
    #[test]
    fn free_gas_permits_up_scatter_at_thermal_energies() {
        let mut seed = 20_260_911_u64;
        let (awr, n) = (8.934_78, 40_000); // Be-9, a FLiBe scatterer
        let mut up_fg = 0usize;
        let mut up_rest = 0usize;
        for _ in 0..n {
            let mu = 2.0 * prn(&mut seed) - 1.0;
            if free_gas_elastic_scatter(0.0253, z_dir(), awr, KT_600, mu, &mut seed).0 > 0.0253 {
                up_fg += 1;
            }
            if two_body_scatter_with_mu(0.0253, z_dir(), awr, 0.0, mu, &mut seed).0 > 0.0253 {
                up_rest += 1;
            }
        }
        assert_eq!(up_rest, 0, "target-at-rest can never up-scatter");
        let frac = up_fg as f64 / n as f64;
        assert!(
            frac > 0.25,
            "free gas at 0.0253 eV (= 0.49 kT at 600 K) must up-scatter often, got {frac}"
        );
    }

    /// The property the per-collision checks cannot see: the kernel must have a
    /// **fixed point**.
    ///
    /// Iterating the scatter law is exactly a neutron walking collision to
    /// collision in an infinite non-absorbing medium, so the stationary
    /// distribution is the *collision* density — `n(E)·v·σ ∝ E·e^{−E/kT}` for a
    /// Maxwellian `n` and a constant σ (which is the CXS approximation this
    /// kernel is built on). That is a Gamma(2, kT), whose mean is `2·kT`, **not**
    /// the density's `1.5·kT`.
    #[test]
    fn free_gas_random_walk_settles_on_the_maxwellian_collision_density() {
        let mut seed = 8_675_309_u64;
        // Light and heavy: xi differs by 25x, so both the fast-relaxing and the
        // slow-relaxing end of the pebble's nuclide list are covered.
        for (label, awr, scatters) in [("Be9", 8.934_78_f64, 300), ("U238", 236.005_8, 20_000)] {
            let walkers = 4_000;
            let mut sum = 0.0;
            for _ in 0..walkers {
                let mut e = 1.0_f64; // start well above thermal
                for _ in 0..scatters {
                    let mu = 2.0 * prn(&mut seed) - 1.0;
                    e = free_gas_elastic_scatter(e, z_dir(), awr, KT_600, mu, &mut seed).0;
                    assert!(e > 0.0 && e.is_finite(), "{label}: energy left the domain");
                }
                sum += e;
            }
            let mean = sum / walkers as f64;
            let ratio = mean / (2.0 * KT_600);
            assert!(
                (0.9..1.1).contains(&ratio),
                "{label}: <E> = {mean:.5} eV is {ratio:.3} x the 2kT = {:.5} eV fixed point",
                2.0 * KT_600
            );
        }
    }

    /// The asymptotic first moment, where it *is* exact.
    ///
    /// Well above the thermal range the target's motion is a small perturbation
    /// and the mean energy loss collapses to the target-at-rest value,
    /// `⟨E′⟩ − E → −2A·E/(1+A)² = −E(1−α)/2`. Checked just *below* the `400·kT`
    /// gate, so the free-gas sampler really runs: at `300·kT` the residual target
    /// motion is O(kT/E) ≈ 0.3 %.
    ///
    /// (Note the same expression with a `(2kT − E)` factor, sometimes quoted as
    /// "the" free-gas first moment, is **not** exact: it gets both limits right —
    /// this one, and the `2kT` fixed point below — but the collision-rate `|v_r|`
    /// weighting between them makes the true moment a different interpolation.
    /// It is not used as an oracle here for that reason.)
    #[test]
    fn free_gas_first_moment_is_asymptotically_target_at_rest() {
        let n = 400_000;
        for &awr in &[1.996_8_f64, 8.934_78, 11.893_65, 236.005_8] {
            let e = 300.0 * KT_600;
            let mut seed = 271_828_182_u64;
            let mut sum = 0.0;
            for _ in 0..n {
                let mu = 2.0 * prn(&mut seed) - 1.0; // isotropic CM
                sum += free_gas_elastic_scatter(e, z_dir(), awr, KT_600, mu, &mut seed).0;
            }
            let measured = sum / n as f64 - e;
            let expected = -2.0 * awr * e / ((1.0 + awr) * (1.0 + awr));
            let err = (measured - expected) / expected;
            assert!(
                err.abs() < 0.02,
                "A = {awr}: <dE> at 300 kT = {measured:.6e} eV, target-at-rest \
                 {expected:.6e} eV ({:.1} % off)",
                100.0 * err
            );
        }
    }

    /// Detailed balance, against the one distribution that is exactly known: the
    /// equilibrium neutron **density** must be Maxwellian.
    ///
    /// Two earlier oracles were tried here and both were wrong, so the reasoning
    /// is spelled out rather than asserted.
    ///
    /// Iterating the kernel walks a neutron from collision to collision, so the
    /// stationary law of that chain is the *collision* density, not the neutron
    /// density. It is tempting to write the collision density as `n(E)·v·σ`, which
    /// for a Maxwellian `n` and constant σ would be a Gamma(2, kT) with mean `2kT`
    /// — and that is what the random-walk test above lands on, to ~2 %. But it is
    /// not exact: a collision rate in a *moving* gas goes as the mean **relative**
    /// speed, not the neutron speed,
    ///
    /// ```text
    /// v̄_rel(E) = √E · [ (1 + 1/(2a²))·erf(a) + e^{−a²}/(a√π) ],   a = √(A·E/kT)
    /// ```
    ///
    /// which exceeds `√E` once `E` approaches `kT`, and by more for a *light*
    /// target (whose own thermal speed is larger). Tested against Gamma(2, kT) the
    /// sampler misses by 3.9 % at `A ≈ 2` and by far less at `A = 238` — the
    /// signature of exactly this term, not of a defect.
    ///
    /// So convert: `n(E) ∝ c(E)/v̄_rel(E)`. Scoring each collision with weight
    /// `1/v̄_rel(E)` turns the chain's own samples into the neutron density, whose
    /// moments are known with nothing fitted — `⟨E⟩ = 1.5·kT`, `⟨E²⟩ = 3.75·(kT)²`.
    /// Checking the second moment as well as the first tests the *shape* the
    /// sampler produces: a kernel that up-scattered too rarely but too far would
    /// hold the mean and miss the variance.
    ///
    /// This also confirms the scheme is self-consistent with how transport uses
    /// it. The collision rate there comes from the Doppler-**broadened** σ_t,
    /// which already carries the `v̄_rel/v` factor; the CXS kinematics sampled here
    /// are its matching partner.
    #[test]
    fn free_gas_equilibrium_density_is_maxwellian() {
        const KT: f64 = KT_600;
        // Light through heavy: D, Be-9, C-12, O-16, U-238.
        for &awr in &[1.996_8_f64, 8.934_78, 11.893_65, 15.857_51, 236.005_8] {
            let mut seed = 13_579_u64;
            // Burn-in scales with 1/xi: a heavy target needs many more collisions
            // to forget where it started.
            let alpha = ((awr - 1.0) / (awr + 1.0)).powi(2);
            let xi = 1.0 + alpha * alpha.ln() / (1.0 - alpha);
            let burn = (40.0 / xi) as usize;
            let samples = 400_000;

            let mut e = 2.0 * KT;
            for _ in 0..burn {
                let mu = 2.0 * prn(&mut seed) - 1.0;
                e = free_gas_elastic_scatter(e, z_dir(), awr, KT, mu, &mut seed).0;
            }

            let (mut w_sum, mut w_e, mut w_e2) = (0.0, 0.0, 0.0);
            for _ in 0..samples {
                let mu = 2.0 * prn(&mut seed) - 1.0; // isotropic CM
                e = free_gas_elastic_scatter(e, z_dir(), awr, KT, mu, &mut seed).0;
                assert!(
                    e > 0.0 && e.is_finite(),
                    "A = {awr}: energy left the domain"
                );
                let w = 1.0 / mean_relative_speed(e, awr, KT);
                w_sum += w;
                w_e += w * e;
                w_e2 += w * e * e;
            }
            let m1 = w_e / w_sum;
            let m2 = w_e2 / w_sum;
            let d1 = (m1 - 1.5 * KT) / (1.5 * KT);
            let d2 = (m2 - 3.75 * KT * KT) / (3.75 * KT * KT);
            assert!(
                d1.abs() < 0.02 && d2.abs() < 0.05,
                "A = {awr}: equilibrium density moments are off by {:.2} % (mean, \
                 {m1:.6} vs {:.6} eV) and {:.2} % (second)",
                100.0 * d1,
                1.5 * KT,
                100.0 * d2
            );
        }
    }

    /// `v̄_rel(E)` — the mean speed of a neutron of energy `E` relative to a
    /// Maxwellian gas of mass ratio `awr` at `kT`, in the same `|v| = √E` units the
    /// kernel uses. The standard closed form; `a = √(A·E/kT)` is the neutron speed
    /// in units of the target's most probable one (the sampler's `beta_vn`).
    fn mean_relative_speed(e: f64, awr: f64, kt: f64) -> f64 {
        let a = (awr * e / kt).sqrt();
        e.sqrt() * ((1.0 + 1.0 / (2.0 * a * a)) * erf(a) + (-a * a).exp() / (a * PI.sqrt()))
    }

    /// Abramowitz & Stegun 7.1.26 — `|error| < 1.5e-7`, ample for a 2 % assertion.
    fn erf(x: f64) -> f64 {
        const P: f64 = 0.327_591_1;
        const A: [f64; 5] = [
            0.254_829_592,
            -0.284_496_736,
            1.421_413_741,
            -1.453_152_027,
            1.061_405_429,
        ];
        let sign = if x < 0.0 { -1.0 } else { 1.0 };
        let x = x.abs();
        let t = 1.0 / (1.0 + P * x);
        let poly = A.iter().rev().fold(0.0, |acc, &c| (acc + c) * t);
        sign * (1.0 - poly * (-x * x).exp())
    }

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
