//! Neutron scattering kinematics — elastic and inelastic.
//!
//! C++ source: `src/physics_common.cpp`, `src/physics.cpp`.
//!
//! The channels differ in both their outgoing *energy* law and their angular law.
//! **Every channel now samples the evaluation's own angular distribution** where
//! the evaluation carries one: elastic and the discrete inelastic levels from
//! ENDF MF=4 (sampled by the caller and passed as `mu_cm` to
//! [`two_body_scatter_with_mu`]), and the continuum channels from the `f₁ … f_NA`
//! columns of MF=6 LAW=1 (bead `op-og56`, see
//! [`continuum_inelastic_scatter_evaluated`]). Angle is the dominant reactivity
//! lever for a bare fast-metal sphere, where forward-peaked scatter off heavy
//! nuclei sets the transport cross section and hence the leakage.
//!
//! Both MF=6 representations this workspace's evaluations use are sampled:
//! `LANG = 1` (Legendre) and `LANG = 2` (Kalbach-Mann, via the Kalbach-86 slope
//! systematics). A representation that is retained but not sampled reports
//! itself as
//! [`ContinuumAngular::Unported`](njoy_outram_park_fork::nuclear_data::secondary::ContinuumAngular::Unported)
//! rather than being indistinguishable from an evaluation that is genuinely
//! flat; nothing in `reference-data/endf/` currently reaches it. By
//! outgoing-energy law:
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
//! - **Discrete-level inelastic** (MT=51…90) — [`two_body_scatter_with_mu`] with
//!   the level's `Q < 0` and the level's own MF=4 CM cosine: the neutron gives up
//!   the level excitation energy, a *large* per-collision energy loss (tens of
//!   keV to MeV) that softens the fast spectrum. This is the dominant
//!   fast-spectrum energy-loss mechanism for heavy nuclei, and its absence
//!   (inelastic lumped into elastic) was the leading bias in the first Godiva
//!   Keff — see `docs/development-history.md`. Its *angular* law was isotropic
//!   until bead `op-tm9f`; U-238's levels are forward-peaked (`⟨μ_cm⟩` +0.03 at
//!   1 MeV to +0.51 at 14 MeV) and sampling them isotropically suppressed leakage
//!   on Godiva. A level the evaluation leaves isotropic (U-235's MT=51/52/54 are
//!   genuinely so) falls back to [`two_body_scatter`].
//! - **Continuum inelastic** (MT=91) and **(n,2n)** (MT=16) —
//!   [`continuum_inelastic_scatter_evaluated`]: the outgoing energy is a
//!   distribution, not fixed by a single `Q`, and the emission angle is
//!   *correlated with it*. Both halves come from the evaluation's own ENDF MF=6
//!   LAW=1 law. [`continuum_inelastic_scatter`] remains as the fallback for a
//!   nuclide carrying no MF=6 section: a **Weisskopf evaporation** model with
//!   nuclear temperature θ = √(E/a) and level-density parameter a ≈ A/11 MeV⁻¹
//!   (actinide), isotropic in the CM — an approximation, documented as such, and
//!   measurably too hard (U-238 at 2 MeV: `⟨E'/E⟩ = 0.2787` against the
//!   evaluation's 0.2095).
//!
//! The MF=4 paths use the full tabulated cosine distribution (sampled in
//! `material::nuclide`, ported from OpenMC), passed here as a CM cosine via
//! [`two_body_scatter_with_mu`] — elastic from MT=2, each discrete level from its
//! own MT. The MF=6 path costs one variate per collision either way: `LANG = 1`
//! linearises each row's Legendre coefficients once at load time and inverts the
//! resulting cosine CDF, and `LANG = 2` inverts the Kalbach-Mann cumulative in
//! closed form.

use crate::geometry::position::Direction;
use crate::material::nuclide::sample_continuous_tabular_indexed;
use crate::rng::lcg::prn;
use njoy_outram_park_fork::nuclear_data::secondary::ContinuumEmission;
use std::f64::consts::PI;
use crate::mathf::RealMath;

/// Rotate the unit direction `u` by scattering cosine `mu` and a uniformly
/// sampled azimuth φ ∈ [0, 2π), returning the new unit direction.
///
/// This is the standard OpenMC `rotate_angle`: it builds an orthonormal frame
/// around `u` and tilts by `(mu, φ)`. The near-pole branch (`|w| ≈ 1`) rotates
/// about the x-axis instead to avoid dividing by √(1−w²) ≈ 0.
pub fn rotate_direction(u: Direction, mu: f64, seed: &mut u64) -> Direction {
    let phi = 2.0 * PI * prn(seed);
    let (sinphi, cosphi) = phi.r_sin_cos();
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
///
/// # Why this is public
///
/// It is one of the two unmeasured leads left on `op-os8x` — the Godiva
/// spectral residual against OpenMC — because it is the step that couples the
/// sampled `μ_cm` to `E'`, so an error here moves the spectrum while leaving
/// every tabulated law identical, which is the signature that remains once
/// cross sections, angular laws, `k`, and the MT=91 transfer table have all been
/// excluded. Exposing it lets
/// `outram-mc-libs`'s `tests/cm_to_lab_vs_kinematics.rs` check it against
/// first-principles velocity addition rather than only through a sampler.
// ANCHOR: cm_to_lab (mdBook include, Monte Carlo tutorial rung 2, gh:#524)
pub fn cm_to_lab(e: f64, e_cm_out: f64, mu_cm: f64, awr: f64) -> (f64, f64) {
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
// ANCHOR_END: cm_to_lab

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
    let e_cm_out = (e * (a / ap1).r_powi(2) + q * a / ap1).max(0.0);
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
/// medium cools without bound (measured: ~~`⟨E⟩ → 1e-27 eV` and below in FLiBe,
/// graphite kernel carbon, O-16 and SiC~~ after 400 collisions at 600 K, against
/// the correct `1.5·kT = 0.0776 eV` — `examples/epithermal_slowing_down.rs`,
/// bead `op-50vu`). **CORRECTED 2026-10-05** (re-measured with that example's
/// `TARGET_AT_REST=1` ablation, GitHub #524): free-gas C-12 `1.5e-27 eV`, Be-9
/// `2e-35`, Li-7 `1e-43`, but F-19 `3e-18`, O-16 `3e-21` and Si-28 `8e-13 eV`:
/// all tens of decades below `kT`, not all below `1e-27`. Only a nuclide carrying
/// an S(α,β) table escaped, because that law does model lattice recoil.
///
/// This is a port of OpenMC `elastic_scatter` + `sample_target_velocity`
/// (`src/physics.cpp`) in the **constant cross-section (CXS)** approximation:
/// σ is taken as constant over the target velocity distribution, which is exactly
/// consistent with using a Doppler-broadened σ for the collision *rate* — the rate
/// already carries the target motion, and this supplies the matching kinematics.
/// (~~OpenMC's DBRC refinement, which resamples σ at the relative energy inside a
/// resonance, is a further correction and is not modelled here.~~ **CORRECTED
/// 2026-09-29 (GitHub #407):** DBRC is modelled, in
/// [`free_gas_elastic_scatter_dbrc`], and is on by default for every nuclide
/// that carries a 0 K elastic grid.)
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
    free_gas_elastic_scatter_dbrc(e, u, awr, kt_ev, mu_cm, seed, None)
}

/// [`free_gas_elastic_scatter`] with an optional **DBRC** correction.
///
/// With `dbrc = None` this is bit-identical to `free_gas_elastic_scatter`,
/// including its RNG draw count — so a nuclide without a table is unaffected,
/// and every result recorded before DBRC existed still reproduces.
///
/// With `Some(table)` and an incident energy inside the table's window, the
/// sampled target velocity is additionally accepted with probability
/// `σ_s^{0K}(E_rel) / σ_max`, which is the weighting the constant-cross-section
/// approximation drops. See [`DbrcTable`] for why that matters and where.
pub fn free_gas_elastic_scatter_dbrc(
    e: f64,
    u: Direction,
    awr: f64,
    kt_ev: f64,
    mu_cm: f64,
    seed: &mut u64,
    dbrc: Option<&DbrcTable>,
) -> (f64, Direction) {
    // OpenMC's gate (`sample_target_velocity`, `src/physics.cpp`): a
    // **resonant** nuclide (one with a DBRC table, OpenMC's `nuc.resonant_`)
    // is at rest only above the resonance-scattering upper bound
    // (`res_scat_energy_max`, here the table's `e_max`). Every other nuclide is
    // at rest at and above `400 kT` when heavier than a neutron.
    //
    // ~~The 400 kT test ran first for every nuclide~~ until 2026-09-29 (GitHub
    // #407), so DBRC never acted above 400 kT (10.1 eV at 293.6 K), and U-238's
    // resonances above that were scattered off a target at rest.
    //
    // A non-positive kT (a caller with no temperature) is at rest, preserving
    // the previous behaviour rather than sampling a degenerate gas.
    let at_rest = match dbrc {
        _ if !(kt_ev > 0.0) => true,
        Some(t) => !t.applies(e),
        None => e >= FREE_GAS_THRESHOLD * kt_ev && awr > 1.0,
    };
    if at_rest {
        return two_body_scatter_with_mu(e, u, awr, 0.0, mu_cm, seed);
    }

    // Velocities in units where |v| = √E, so E = v·v throughout.
    let vel = e.sqrt();
    let v_n = [vel * u.u, vel * u.v, vel * u.w];
    let v_t = sample_target_velocity_dbrc(e, u, awr, kt_ev, seed, dbrc);

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

/// A nuclide's **0 K elastic scattering cross section** over the energy window
/// where resonance structure makes the constant-cross-section approximation
/// wrong — the data DBRC needs.
///
/// # What DBRC corrects
///
/// [`free_gas_elastic_scatter`] samples the target velocity from a Maxwellian
/// weighted only by the relative speed. That is the **constant cross-section
/// (CXS)** approximation: it assumes `σ_s` does not vary over the range of
/// relative energies a thermal target can reach. Near a resolved resonance in a
/// heavy nuclide that is badly wrong — U-238's 6.67 eV resonance changes `σ_s`
/// by orders of magnitude across a window the target's own motion spans — and
/// the correct kernel weights the target velocity by `σ_s(E_rel)` as well.
///
/// **Doppler Broadening Rejection Correction** does that by rejection: sample a
/// target velocity as usual, compute the relative energy, and accept with
/// probability `σ_s^{0K}(E_rel) / σ_max` over the neighbourhood. The 0 K cross
/// section is the right one because the target motion is being modelled
/// explicitly — using a broadened `σ` here would count Doppler broadening twice.
///
/// # Why it matters here specifically
///
/// It is an **epithermal** effect on resolved resonances, so it is invisible on
/// a bare fast metal sphere and material in a thermal or epithermal lattice —
/// exactly the reactors this project targets (HTR-10, MSRE, the FHR pebble),
/// and exactly where this crate has the least validation evidence. It raises
/// U-238's effective resonance absorption and is worth of order 100-200 pcm in
/// an LWR pin cell in the published literature; it has **not** been measured
/// here (see `Nuclide::with_dbrc`).
///
/// Ported from OpenMC `sample_target_velocity` / `ResScatMethod::DBRC`
/// (`src/physics.cpp`, `src/nuclide.cpp`).
#[derive(Debug, Clone)]
pub struct DbrcTable {
    /// Ascending energy grid \[eV\].
    energy: Vec<f64>,
    /// 0 K elastic cross section \[b\] aligned with `energy`.
    xs: Vec<f64>,
    /// Upper energy \[eV\] above which DBRC is not applied.
    e_max: f64,
    /// `∫ √E σ₀K(E) dE` from the first grid point, trapezoidal, negative
    /// σ zeroed: OpenMC's `xs_cdf_` (`src/nuclide.cpp`, resonant nuclides),
    /// what RVS samples the relative energy from.
    cdf: Vec<f64>,
    /// Which resonance-scattering sampler the window uses.
    method: ResonanceScatteringMethod,
}

/// How a resonant nuclide's target velocity is sampled inside the window
/// `E_red ± 4` (OpenMC's `ResScatMethod`, `src/physics.cpp`
/// `sample_target_velocity`). Both sample the same 0 K-corrected
/// distribution; they differ in the random-number path.
///
/// **RVS is the default**, as upstream's (`settings.cpp`:
/// `res_scat_method {ResScatMethod::rvs}`). Maintainer decision 2026-11-09
/// (gh:#1311): resonance scattering stays **on** by default (the root
/// CLAUDE.md correct-physics rule) and uses upstream's default method. DBRC
/// is the named alternative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResonanceScatteringMethod {
    /// Relative velocity sampling: the relative energy from the √E σ₀K CDF,
    /// the target energy from the Maxwellian, accepted on `|μ| < 1`.
    #[default]
    Rvs,
    /// Doppler-broadening rejection correction: constant-cross-section
    /// candidates, accepted with `σ₀K(E_rel) / σ_max`.
    Dbrc,
}

impl DbrcTable {
    /// Build from an ascending 0 K `(energy [eV], σ_elastic [b])` grid, applied
    /// below `e_max_ev`.
    ///
    /// ~~Restricted to `e <= e_max_ev`~~ **CORRECTED 2026-09-29 (GitHub
    /// #407):** the whole supplied grid is kept. The rejection window of a
    /// neutron just below `e_max_ev` reaches above it, and OpenMC reads the 0 K
    /// cross section there (`nuclide.energy_0K_` spans the whole table).
    ///
    /// Returns `None` when fewer than two points lie at or below `e_max_ev` —
    /// a nuclide with no resolved resonance structure in the window has
    /// nothing for DBRC to correct.
    pub fn from_pairs(pairs: &[(f64, f64)], e_max_ev: f64) -> Option<Self> {
        let energy: Vec<f64> = pairs.iter().map(|p| p.0).collect();
        let xs: Vec<f64> = pairs.iter().map(|p| p.1).collect();
        if energy.iter().filter(|&&e| e <= e_max_ev).count() < 2 {
            return None;
        }
        // OpenMC `src/nuclide.cpp`: negative cross sections set to zero, then
        // xs_cdf[i+1] = xs_cdf[i] + (√E_i σ_i + √E_{i+1} σ_{i+1}) / 2 · (E_{i+1} − E_i).
        let pos: Vec<f64> = xs.iter().map(|&x| if x < 0.0 { 0.0 } else { x }).collect();
        let mut cdf = vec![0.0; energy.len()];
        let mut sum = 0.0;
        for i in 0..energy.len().saturating_sub(1) {
            sum += (energy[i].sqrt() * pos[i] + energy[i + 1].sqrt() * pos[i + 1]) / 2.0 * (energy[i + 1] - energy[i]);
            cdf[i + 1] = sum;
        }
        Some(DbrcTable {
            energy,
            xs,
            e_max: e_max_ev,
            cdf,
            method: ResonanceScatteringMethod::default(),
        })
    }

    /// The same table with `method` as its sampler.
    pub fn with_method(mut self, method: ResonanceScatteringMethod) -> Self {
        self.method = method;
        self
    }

    /// The sampler this table uses.
    pub fn method(&self) -> ResonanceScatteringMethod {
        self.method
    }

    /// OpenMC's index rule for a window bound (`physics.cpp`): 0 below the
    /// grid, `n − 2` above it, otherwise `lower_bound_index` (the last `i`
    /// with `energy[i] <= e`).
    fn bound_index(&self, e: f64) -> usize {
        let n = self.energy.len();
        if e < self.energy[0] {
            0
        } else if e > self.energy[n - 1] {
            n - 2
        } else {
            self.energy.partition_point(|&x| x <= e).saturating_sub(1).min(n - 2)
        }
    }

    /// RVS: one target velocity, `(E_t / A)` speed squared in √E units and
    /// cosine with the neutron, by OpenMC's `ResScatMethod::rvs` branch
    /// (`src/physics.cpp`), for a neutron of `e` eV, `awr`, `kt_ev`. Returns
    /// `None` in the degenerate case (both bounds in one interval), where
    /// upstream uses the constant-cross-section sampler.
    fn sample_rvs(&self, e: f64, awr: f64, kt_ev: f64, seed: &mut u64) -> Option<(f64, f64)> {
        let e_red = (awr * e / kt_ev).sqrt();
        let e_low = (e_red - 4.0).max(0.0).r_powi(2) * kt_ev / awr;
        let e_up = (e_red + 4.0) * (e_red + 4.0) * kt_ev / awr;
        let i_low = self.bound_index(e_low);
        let i_up = self.bound_index(e_up);
        if i_up == i_low {
            return None;
        }
        let en = &self.energy;
        let cdf = &self.cdf;
        let mut cdf_low = 0.0;
        if e_low > en[0] {
            let m = (cdf[i_low + 1] - cdf[i_low]) / (en[i_low + 1] - en[i_low]);
            cdf_low = cdf[i_low] + m * (e_low - en[i_low]);
        }
        let m = (cdf[i_up + 1] - cdf[i_up]) / (en[i_up + 1] - en[i_up]);
        let cdf_up = cdf[i_up] + m * (e_up - en[i_up]);
        loop {
            // Directly sample the Maxwellian target energy.
            let e_t = -kt_ev * prn(seed).r_ln();
            // A relative energy from the cross-section CDF.
            let cdf_rel = cdf_low + prn(seed) * (cdf_up - cdf_low);
            // lower_bound_index over cdf[i_low ..= i_up + 1].
            let slice = &cdf[i_low..=i_up + 1];
            let k = slice.partition_point(|&c| c <= cdf_rel).saturating_sub(1).min(slice.len() - 2);
            let j = i_low + k;
            let m = (cdf[j + 1] - cdf[j]) / (en[j + 1] - en[j]);
            let e_rel = en[j] + (cdf_rel - cdf[j]) / m;
            // Rejection on the cosine between neutron and target velocities.
            let mu = (e_t + awr * (e - e_rel)) / (2.0 * (awr * e * e_t).sqrt());
            if mu.abs() < 1.0 {
                return Some((e_t / awr, mu));
            }
        }
    }

    /// Upper energy \[eV\] DBRC is applied below.
    pub fn e_max_ev(&self) -> f64 {
        self.e_max
    }

    /// Number of tabulated points.
    pub fn len(&self) -> usize {
        self.energy.len()
    }

    /// Whether the table is empty (never true for a [`Self::from_pairs`] result).
    pub fn is_empty(&self) -> bool {
        self.energy.is_empty()
    }

    /// 0 K elastic cross section \[b\] at `e` \[eV\], lin-lin interpolated and
    /// clamped at the ends.
    pub fn xs_at(&self, e: f64) -> f64 {
        let n = self.energy.len();
        if e <= self.energy[0] {
            return self.xs[0];
        }
        if e >= self.energy[n - 1] {
            return self.xs[n - 1];
        }
        let hi = self.energy.partition_point(|&x| x < e).max(1).min(n - 1);
        let (x0, x1) = (self.energy[hi - 1], self.energy[hi]);
        let (y0, y1) = (self.xs[hi - 1], self.xs[hi]);
        if x1 > x0 {
            y0 + (y1 - y0) * (e - x0) / (x1 - x0)
        } else {
            y0
        }
    }

    /// The maximum 0 K elastic cross section \[b\] over `[lo, hi]` eV — the
    /// rejection envelope.
    ///
    /// Scans the tabulated points in the window as well as its endpoints, so a
    /// resonance peak between grid nodes cannot be missed in a way that would
    /// make the acceptance probability exceed 1 (which would silently bias the
    /// sampling rather than error).
    pub fn xs_max_over(&self, lo: f64, hi: f64) -> f64 {
        let mut m = self.xs_at(lo).max(self.xs_at(hi));
        let start = self.energy.partition_point(|&x| x < lo);
        for k in start..self.energy.len() {
            if self.energy[k] > hi {
                break;
            }
            m = m.max(self.xs[k]);
        }
        m
    }

    /// Whether DBRC (target motion with the 0 K rejection) applies to a
    /// neutron of energy `e` \[eV\]: at or below `e_max`, OpenMC's
    /// `res_scat_energy_max`. Above it a resonant nuclide's target is at rest.
    pub fn applies(&self, e: f64) -> bool {
        e <= self.e_max
    }

    /// Whether any tabulated point lies strictly inside `(lo, hi)`. When none
    /// does, both ends fall in one grid interval, where the 0 K cross section
    /// is linear. OpenMC then uses the constant-cross-section sampler
    /// (`i_E_up == i_E_low`, "the degenerate case").
    fn has_point_inside(&self, lo: f64, hi: f64) -> bool {
        let a = self.energy.partition_point(|&x| x <= lo);
        a < self.energy.len() && self.energy[a] < hi
    }
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
///
/// With `dbrc = Some(..)` the optional DBRC rejection is layered on top.
/// (A `dbrc = None` wrapper, `sample_target_velocity`, had no caller and was
/// removed 2026-09-30.)
///
/// # The two rejections are separate, and the order matters
///
/// Upstream's own rejection (accept with `|v_n − v_t| / (v_n + v_t)`) makes the
/// sampled speed proportional to the relative speed. DBRC adds a **second**
/// rejection on `σ_s^{0K}(E_rel) / σ_max`. Applying them as two independent
/// accept tests on the same candidate is what makes the product of the two
/// weights come out right; folding them into one acceptance probability would
/// need the envelope of the product and is not what OpenMC does.
///
/// With `dbrc = None` the added loop never runs and the draw sequence is
/// identical to the pre-DBRC sampler — the property that keeps every existing
/// result reproducible.
fn sample_target_velocity_dbrc(
    e: f64,
    u: Direction,
    awr: f64,
    kt_ev: f64,
    seed: &mut u64,
    dbrc: Option<&DbrcTable>,
) -> [f64; 3] {
    // β·v_n = √(A·E / kT) — the neutron speed in units of the target's thermal one.
    let beta_vn = (awr * e / kt_ev).sqrt();
    let alpha = 1.0 / (1.0 + PI.sqrt() * beta_vn / 2.0);

    // OpenMC's resonance-scattering window (`physics.cpp`,
    // `sample_target_velocity`, `ResScatMethod::dbrc`):
    //   E_low = (max(0, √(AE/kT) − 4))² kT/A,  E_up = (√(AE/kT) + 4)² kT/A.
    // ~~`E ± 4√(kT E/A)`~~ until 2026-09-29 (GitHub #407): about half that
    // width, so the envelope `σ_max` could miss a peak the target reaches.
    // DBRC applies at and above OpenMC's `res_scat_energy_min` (0.01 eV); below
    // it, and in the degenerate case where the window holds no grid point,
    // the constant-cross-section sampler is used, as upstream.
    const RES_SCAT_ENERGY_MIN: f64 = 0.01;
    let window = dbrc.filter(|t| t.method == ResonanceScatteringMethod::Dbrc && t.applies(e) && e >= RES_SCAT_ENERGY_MIN).and_then(|t| {
        let e_low = (beta_vn - 4.0).max(0.0).r_powi(2) * kt_ev / awr;
        let e_up = (beta_vn + 4.0).r_powi(2) * kt_ev / awr;
        t.has_point_inside(e_low, e_up).then(|| (t, e_up, t.xs_max_over(e_low, e_up)))
    });

    // RVS (the default method): its own window arithmetic and degenerate
    // test, as upstream's `ResScatMethod::rvs` branch. The result is already
    // in √E units, so it returns here.
    if let Some(t) = dbrc.filter(|t| t.method == ResonanceScatteringMethod::Rvs && t.applies(e) && e >= RES_SCAT_ENERGY_MIN) {
        if let Some((vt_sq, mu)) = t.sample_rvs(e, awr, kt_ev, seed) {
            let vt = vt_sq.sqrt();
            let u_t = rotate_direction(u, mu, seed);
            return [vt * u_t.u, vt * u_t.v, vt * u_t.w];
        }
        let (beta_vt_sq, mu) = sample_cxs_target(beta_vn, alpha, seed);
        let vt = (beta_vt_sq * kt_ev / awr).sqrt();
        let u_t = rotate_direction(u, mu.clamp(-1.0, 1.0), seed);
        return [vt * u_t.u, vt * u_t.v, vt * u_t.w];
    }
    let (beta_vt_sq, mu) = match window {
        None => sample_cxs_target(beta_vn, alpha, seed),
        Some((t, e_up, s_max)) => {
            // OpenMC: draw CXS candidates until E_rel < E_up, then accept with
            // σ⁰ᴷ(E_rel)/σ_max; on rejection start again from a new candidate.
            //
            // **Unbounded, as upstream (`while (true)`).** ~~Bounded at 4096
            // trials, returning the last, unaccepted candidate~~ until
            // 2026-09-30 (GitHub #407). Next to a resonance the envelope
            // σ_max is the 0 K peak, so the acceptance σ̄/σ_peak is small
            // (U-238 at 20.5 eV: the window reaches the 20.87 eV peak). The
            // cap was then hit often, and each hit returned a plain CXS
            // target. That
            // silently undid most of the correction, and it was measured:
            // U-238's single-scatter spectrum at 20.5 eV was chi²/ν =
            // 2625/59 against OpenMC (`tests/dbrc_single_scatter_vs_openmc.rs`).
            // The loop terminates with probability 1 whenever σ⁰ᴷ > 0 inside
            // the window. `s_max > 0` is checked before entering.
            if !(s_max > 0.0) {
                sample_cxs_target(beta_vn, alpha, seed)
            } else {
                loop {
                    let (cand, e_rel) = loop {
                        let cand = sample_cxs_target(beta_vn, alpha, seed);
                        let beta_vt = cand.0.sqrt();
                        let e_rel = (beta_vn * beta_vn + cand.0 - 2.0 * beta_vn * beta_vt * cand.1)
                            .max(0.0)
                            * kt_ev
                            / awr;
                        if e_rel < e_up {
                            break (cand, e_rel);
                        }
                    };
                    if prn(seed) < t.xs_at(e_rel) / s_max {
                        break cand;
                    }
                }
            }
        }
    };

    // Back to |v| = √E units: E_t = β²v_t² · kT / A.
    let vt = (beta_vt_sq * kt_ev / awr).sqrt();
    let u_t = rotate_direction(u, mu.clamp(-1.0, 1.0), seed);
    [vt * u_t.u, vt * u_t.v, vt * u_t.w]
}

/// One target velocity in the **constant cross-section** approximation, as
/// `(β²v_t², μ)`: OpenMC's `sample_cxs_target_velocity` (`src/physics.cpp`).
/// The speed comes from the two-branch Maxwellian mixture and the pair is
/// accepted with `|v_n − v_t| / (v_n + v_t)`. The draw sequence is exactly the
/// one this crate always used, so the no-DBRC path is bit-identical.
fn sample_cxs_target(beta_vn: f64, alpha: f64, seed: &mut u64) -> (f64, f64) {
    let (mut beta_vt_sq, mut mu) = (0.0_f64, 0.0_f64);
    // The rejection accepts with probability ≥ (v_n − v_t)/(v_n + v_t) and in
    // practice within a handful of trials; the bound keeps a pathological RNG
    // from hanging transport, and falling out with the last draw is unbiased to
    // the accuracy of that bound.
    for _ in 0..1024 {
        beta_vt_sq = if prn(seed) < alpha {
            // p(y) ∝ y·e^{−y²}: y² = −ln(r₁r₂).
            -(safe_prn(seed) * safe_prn(seed)).r_ln()
        } else {
            // p(y) ∝ y²·e^{−y²}.
            let c = (PI / 2.0 * prn(seed)).r_cos();
            -safe_prn(seed).r_ln() - safe_prn(seed).r_ln() * c * c
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
    (beta_vt_sq, mu)
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
/// level-density parameter `a ≈ A/11 MeV⁻¹` (a standard actinide value), sampled
/// then transformed to the lab isotropically in CM.
///
/// `q` is the channel's ENDF Q-value \[eV\] (negative — MT=91's `QI`, i.e. minus
/// the energy of the lowest continuum state). It **caps** the outgoing CM energy
/// at what two-body energy balance allows for that minimum excitation,
///
/// `E'_cm ≤ E·(A/(A+1))² + Q·A/(A+1)`,
///
/// which is the same bound [`two_body_scatter`] enforces for a discrete level.
/// Pass `q = 0` for a channel whose Q is genuinely zero or unknown; that reduces
/// the cap to the elastic CM energy, which is the behaviour this function had
/// before the Q-value was threaded through.
///
/// # Why the cap is not optional
///
/// Without it the sampler caps only at the *elastic* CM energy, so a neutron can
/// leave a continuum-inelastic collision carrying energy the reaction cannot
/// have left it — the excitation of the residual nucleus is simply not paid for.
/// Measured on ENDF/B-VIII.0 before this was threaded through: **22 % of draws**
/// above the kinematic bound for U-238 and U-235 at 1 MeV (just above the MT=91
/// threshold at `|QI| = 0.434 MeV`), 2.5 % at 2 MeV, falling to zero by 14 MeV
/// where the cap is loose. The violation is largest exactly where the channel
/// opens, which is where it matters most.
///
/// This remains an **approximation** in its *shape*: it is the fallback for a
/// nuclide whose evaluation carries no MF=6 LAW=1 law this port can read. Where
/// one is available, call
/// [`continuum_inelastic_scatter_evaluated`] instead and pass it — the evaluated
/// law is the evaluation's own spectrum and this evaporation model is measurably
/// too hard (U-238 at 2 MeV: `⟨E'/E⟩ = 0.2787` here against the evaluation's
/// **0.2095**, i.e. 33 % harder, at the energy where MT=91 is opening). See
/// GitHub #192.
pub fn continuum_inelastic_scatter(
    e: f64,
    u: Direction,
    awr: f64,
    q: f64,
    seed: &mut u64,
) -> (f64, Direction) {
    let a = awr;
    let ap1 = a + 1.0;
    // Max neutron CM energy allowed by two-body energy balance for this channel.
    let e_cm_elastic = (e * (a / ap1).r_powi(2) + q * a / ap1).max(0.0);

    // Weisskopf temperature θ = √(E/a_ld), a_ld ≈ A/11 MeV⁻¹.
    let a_ld = (a / 11.0).max(1.0); // MeV⁻¹
    let theta = ((e * 1.0e-6 / a_ld).sqrt() * 1.0e6).max(1.0); // eV

    // Sample E'_cm ~ Gamma(shape 2, scale θ) = −θ·ln(r1·r2), rejecting any draw
    // that would not lose energy. Fall back to a uniform down-scatter if the
    // (rare) rejection loop is exhausted.
    let mut e_cm_out = e_cm_elastic * prn(seed);
    for _ in 0..64 {
        let cand = -theta * (prn(seed) * prn(seed)).r_ln();
        if cand <= e_cm_elastic {
            e_cm_out = cand;
            break;
        }
    }

    let mu_cm = 2.0 * prn(seed) - 1.0; // isotropic in CM
    let (e_out, mu_lab) = cm_to_lab(e, e_cm_out, mu_cm, a);
    (e_out, rotate_direction(u, mu_lab, seed))
}

/// Continuum inelastic (MT=91) or (n,2n) (MT=16) scatter using the
/// **evaluated** ENDF MF=6 LAW=1 emission law when the nuclide carries one,
/// falling back to [`continuum_inelastic_scatter`]'s Weisskopf evaporation
/// stand-in when it does not.
///
/// # Arguments
///
/// `e` incident lab energy \[eV\], `u` incident direction, `awr` the target's
/// atomic weight ratio, `q` the channel `QI` \[eV\] (used only by the fallback
/// and by the kinematic cap), `law` the evaluated emission law
/// ([`Nuclide::continuum_law`](crate::material::nuclide::Nuclide::continuum_law)),
/// and `seed` the particle's RNG stream.
///
/// # What the evaluated path does
///
/// 1. Picks an emission **branch** in proportion to the branches' yields. Most
///    evaluations have one; F-19's MF=6/MT=16 has two, of yield 1 each, carrying
///    different spectra for the first and second emitted neutron.
/// 2. Samples `E'` from that branch's tabulated `f₀(E→E')` with the same
///    `ContinuousTabular` sampler the MF=5 fission spectrum uses (locate the
///    incident bin, statistically pick a table, invert its CDF, scale between the
///    neighbouring tables' envelopes) — ported from OpenMC
///    `src/distribution_energy.cpp`.
/// 3. Transforms to the laboratory frame **only if the evaluation says to**:
///    ENDF `LCT = 2` (both uranium isotopes' MT=91) means the law is tabulated in
///    the centre of mass, so the sampled `E'` is a CM energy and goes through
///    [`cm_to_lab`] with an isotropic CM cosine. `LCT = 1` (F-19's MT=91 and
///    MT=16) means it is already a laboratory energy, and the emission is taken
///    isotropic in the lab. Assuming one frame for both is wrong by the full
///    CM-motion term, which is why the flag is carried rather than inferred.
///
/// # The angular correlation
///
/// MF=6 LAW=1 is a **correlated** energy-angle law: each `[E', f₀, f₁ … f_NA]`
/// row carries an emission cosine distribution *conditional on that outgoing
/// energy*. Those `f₁ … f_NA` are now read and sampled for `LANG = 1`
/// (Legendre), which is what ENDF/B-VIII.0's U-234/U-235/U-238, F-19 and Si-28
/// use — see [`ContinuumAngular`](njoy_outram_park_fork::nuclear_data::secondary::ContinuumAngular). Until bead `op-og56` they were discarded at
/// parse time and every continuum neutron left isotropically; the evaluations
/// say otherwise on essentially every row (U-238's MT=91: 8652 of 8654 rows
/// anisotropic, `⟨μ⟩` reaching 0.56).
///
/// Three cases, and they are deliberately distinguishable rather than all
/// arriving as "isotropic":
///
/// - [`ContinuumAngular::Legendre`](njoy_outram_park_fork::nuclear_data::secondary::ContinuumAngular::Legendre) — the row's tabulated cosine CDF is
///   inverted. This is the evaluated law.
/// - [`ContinuumAngular::EvaluatedIsotropic`](njoy_outram_park_fork::nuclear_data::secondary::ContinuumAngular::EvaluatedIsotropic) — the evaluation declares
///   `NA = 0` throughout (F-19's MT=91), so `μ = 2ξ − 1` is **correct**.
/// - `ContinuumAngular::KalbachMann` — `LANG = 2`, which ENDF/B-VIII.0's O-16
///   and Al-27 use on MT=16 and MT=91. The slope `a` comes from the Kalbach-86
///   systematics where the evaluation stores only `r`.
/// - [`ContinuumAngular::Unported`] — a representation this port retains but
///   does not sample. Emission falls back to isotropic and says so. **No
///   evaluation in `reference-data/endf/` currently reaches it.**
///
/// # Ablation
///
/// Setting `OUTRAM_MC_ISOTROPIC_CONTINUUM=1` forces the isotropic draw on every
/// path, so the law's worth can be **priced** by running the same case twice
/// rather than argued from the size of `⟨μ⟩`. It consumes the identical RNG
/// variate either way — one draw, whether it indexes a CDF or is mapped
/// linearly — so a paired run differs by the physics and not by a re-randomised
/// stream. See `tests/continuum_angular_ablation_control.rs`, which asserts the
/// switch actually changes the sampled cosines: a control that silently fails to
/// ablate reports "no difference" and reads as "this physics does not matter".
pub fn continuum_inelastic_scatter_evaluated(
    e: f64,
    u: Direction,
    awr: f64,
    q: f64,
    law: Option<&ContinuumEmission>,
    seed: &mut u64,
) -> (f64, Direction) {
    continuum_inelastic_scatter_evaluated_with(
        e,
        u,
        awr,
        q,
        law,
        continuum_angular_mode_from_env(),
        seed,
    )
}

/// Whether the evaluated continuum angular law is sampled, or ablated away.
///
/// The ablation arm of a paired worth measurement. Separating it from the
/// environment lookup is deliberate: a test can drive both arms in one process
/// with no global state and no ordering hazard, which a cached `static` flag
/// cannot support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuumAngularMode {
    /// Sample the evaluation's correlated cosine law wherever it carries one.
    /// The physical arm.
    Evaluated,
    /// Force isotropic emission in the law's own frame, whatever the evaluation
    /// says. The ablation arm — this is how the law's reactivity worth gets
    /// *priced* rather than argued from the size of `⟨μ⟩`.
    IsotropicAblation,
}

/// [`continuum_inelastic_scatter_evaluated`] with the angular treatment chosen
/// explicitly rather than read from the environment.
///
/// Both modes consume the **same RNG variates in the same order**, and the
/// outgoing *energy* is identical between them — one draw is spent either
/// inverting the row's cosine CDF or mapping linearly to `2ξ − 1`. A paired run
/// therefore differs by the angular physics alone and not by a re-randomised
/// stream, which is what makes the difference attributable. That property is
/// asserted, not assumed: see
/// `tests/continuum_angular_ablation_control.rs`.
pub fn continuum_inelastic_scatter_evaluated_with(
    e: f64,
    u: Direction,
    awr: f64,
    q: f64,
    law: Option<&ContinuumEmission>,
    mode: ContinuumAngularMode,
    seed: &mut u64,
) -> (f64, Direction) {
    let Some(law) = law else {
        return continuum_inelastic_scatter(e, u, awr, q, seed);
    };

    let branch = law.branch_for(e, prn(seed));
    let (sampled, mu) = sample_continuum_branch(branch, e, mode, seed);

    if law.cm_frame {
        // The evaluation's grids already stop at the two-body bound, but the
        // envelope scaling between neighbouring incident tables can land a
        // fraction above it; clamp so no draw can exceed what energy balance
        // allows, exactly as the stand-in path does.
        let ap1 = awr + 1.0;
        let cap = (e * (awr / ap1).r_powi(2) + q * awr / ap1).max(0.0);
        let e_cm_out = sampled.min(cap);
        // `mu` is a CM cosine here (ENDF LCT=2), so it goes through the same
        // frame transform as the energy.
        let (e_out, mu_lab) = cm_to_lab(e, e_cm_out, mu, awr);
        (e_out, rotate_direction(u, mu_lab, seed))
    } else {
        // Laboratory-frame law (LCT=1): the sampled energy is already the
        // outgoing lab energy and `mu` is already a lab cosine.
        (sampled, rotate_direction(u, mu, seed))
    }
}

/// Sample one **correlated energy-angle** draw from a continuum branch at
/// incident energy `e` \[eV\]: `(E', mu)` in the law's own frame, before any
/// CM→lab transform. Public so the verification tests exercise exactly the
/// transport path.
///
/// The energy is OpenMC's `ContinuousTabular` scheme (two variates); the
/// cosine takes one more variate, from the row OpenMC's
/// `CorrelatedAngleEnergy::sample_dist` picks — the **closer** bin edge for a
/// lin-lin table — or, for Kalbach-Mann, from `r`/`a` interpolated to the
/// sampled `E'` as `KalbachMann::sample_params` does (GitHub #365 audit;
/// before it the lower row was always used and `r`/`a` were not
/// interpolated). The ablation spends the same one variate on `2ξ − 1`.
pub fn sample_continuum_branch(
    branch: &njoy_outram_park_fork::nuclear_data::secondary::ContinuumBranch,
    e: f64,
    mode: ContinuumAngularMode,
    seed: &mut u64,
) -> (f64, f64) {
    let (sampled, table, pick) = sample_continuous_tabular_indexed(&branch.spectrum, e, seed);
    // One variate, spent either on inverting the row's cosine CDF or on the flat
    // map — so the ablation does not shift the RNG stream.
    let xi = prn(seed);
    let kalbach = matches!(
        branch.angular,
        njoy_outram_park_fork::nuclear_data::secondary::ContinuumAngular::KalbachMann(_)
    );
    let mu = match mode {
        ContinuumAngularMode::IsotropicAblation => {
            // Kalbach-Mann spends a SECOND variate in OpenMC's sampler (below);
            // the ablation spends it too, so the two arms stay on one stream.
            if kalbach {
                let _ = prn(seed);
            }
            2.0 * xi - 1.0
        }
        ContinuumAngularMode::Evaluated => {
            if let Some((r, a)) = kalbach_row_at(&branch.angular, table, pick) {
                kalbach_mann_mu_openmc(r, a, xi, seed)
            } else {
                // `sample_mu_at` dispatches over the representation and returns `None`
                // only where no angular law exists, where isotropic is right anyway.
                branch
                    .angular
                    .sample_mu_at(table, pick, xi)
                    .unwrap_or(2.0 * xi - 1.0)
            }
        }
    };
    (sampled, mu)
}

/// The Kalbach-Mann `(r, a)` of the row `pick` selects, interpolated in outgoing
/// energy as OpenMC's `KalbachMann::sample_params` does
/// (`r[k] + frac * (r[k+1] - r[k])`, `frac = 0` for histogram tables and
/// lines), or `None` for any other angular representation.
fn kalbach_row_at(
    angular: &njoy_outram_park_fork::nuclear_data::secondary::ContinuumAngular,
    table: usize,
    pick: njoy_outram_park_fork::nuclear_data::secondary::AnglePick,
) -> Option<(f64, f64)> {
    use njoy_outram_park_fork::nuclear_data::secondary::ContinuumAngular;
    let ContinuumAngular::KalbachMann(tables) = angular else {
        return None;
    };
    let rows = &tables.get(table)?.rows;
    let a = rows.get(pick.row)?;
    Some(match rows.get(pick.row + 1) {
        Some(b) if pick.frac > 0.0 => (a.r + pick.frac * (b.r - a.r), a.a + pick.frac * (b.a - a.a)),
        _ => (a.r, a.a),
    })
}

/// OpenMC's Kalbach-Mann emission cosine (`KalbachMann::sample`,
/// `src/secondary_kalbach.cpp`, commit `8f79d9d0`): a **mixture** sampled with
/// two variates, not the closed-form inverse of the density. `xi` is the first
/// (`prn(seed) > km_r` selects the branch), the second is drawn here:
///
/// - `xi > r`: `T = U(-1, 1) sinh(a)`, `mu = ln(T + sqrt(T^2 + 1)) / a`;
/// - else: `r1 = prn`, `mu = ln(r1 e^a + (1 - r1) e^-a) / a`.
///
/// The same distribution as [`ContinuumKalbachRow::sample_mu`]'s inverse, with a
/// different map from variates to `mu`; this is the one that reproduces OpenMC's
/// `mu` for a given seed (`tests/secondary_samplers_vs_openmc_code_to_code.rs`).
/// `a` below `1e-12` (the isotropic limit, which upstream would divide by zero
/// on) gives `2 u - 1`.
///
/// [`ContinuumKalbachRow::sample_mu`]: njoy_outram_park_fork::nuclear_data::secondary::ContinuumKalbachRow::sample_mu
pub fn kalbach_mann_mu_openmc(r: f64, a: f64, xi: f64, seed: &mut u64) -> f64 {
    if !(a.abs() > 1.0e-12) || !a.is_finite() {
        return 2.0 * prn(seed) - 1.0;
    }
    if xi > r {
        let t = (-1.0 + 2.0 * prn(seed)) * a.r_sinh();
        (t + (t * t + 1.0).sqrt()).r_ln() / a
    } else {
        let r1 = prn(seed);
        (r1 * a.r_exp() + (1.0 - r1) * (-a).r_exp()).r_ln() / a
    }
}

/// The continuum angular mode for this process, from
/// `OUTRAM_MC_ISOTROPIC_CONTINUUM`.
///
/// Set the variable to `1` (or anything other than `0`/empty) to run the
/// ablation arm. It is read once and cached, so the inner transport loop pays an
/// atomic load rather than an environment lookup per collision — which also
/// means changing it mid-process has no effect. A test that needs both arms
/// calls [`continuum_inelastic_scatter_evaluated_with`] directly instead of
/// fighting this cache.
///
/// The switch exists so the law's reactivity worth is **measured**, not
/// asserted — the standing lesson of GitHub #193, where pricing a mechanism by
/// switching it off found in one run what days of accuracy comparisons had
/// missed. The same shape as the `OUTRAM_RINGRPT_*` ablations.
pub fn continuum_angular_mode_from_env() -> ContinuumAngularMode {
    use std::sync::atomic::{AtomicU8, Ordering};
    static FLAG: AtomicU8 = AtomicU8::new(u8::MAX);
    let cached = FLAG.load(Ordering::Relaxed);
    let on = if cached != u8::MAX {
        cached == 1
    } else {
        let on = match std::env::var("OUTRAM_MC_ISOTROPIC_CONTINUUM") {
            Ok(v) => !v.is_empty() && v != "0",
            Err(_) => false,
        };
        FLAG.store(on as u8, Ordering::Relaxed);
        on
    };
    if on {
        ContinuumAngularMode::IsotropicAblation
    } else {
        ContinuumAngularMode::Evaluated
    }
}

// ---------------------------------------------------------------------------
// OpenMC-faithful collision kinematics (gh:#1444, #1386 prio-1 row 4).
//
// Translation of routines from OpenMC `8f79d9d02483c90e4134f03a9df4b75ce6b062bf`
// (MIT; Copyright 2011-2025 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors):
//   src/math_functions.cpp  rotate_angle
//   src/physics.cpp         elastic_scatter, sample_target_velocity,
//                           sample_cxs_target_velocity, inelastic_scatter (CM -> lab block)
//   src/nuclide.cpp         Nuclide::elastic_xs_0K, the 0 K CDF build
//   openmc/data/reaction.py LevelInelastic parameters (threshold, mass ratio)
//
// These sit BESIDE the older kernels above (`two_body_scatter_with_mu`,
// `free_gas_elastic_scatter_dbrc`, `rotate_direction`), which stay callable and
// keep every recorded pin: they are the same physics with different operation
// order, a different order of the CM-cosine draw against the target-velocity
// draws, and (for `rotate_direction`) a different pole branch. The functions
// below follow upstream line by line so a track differential can be bit-identical
// (`tests/collision_kinematics_vs_openmc_code_to_code.rs`).
// ---------------------------------------------------------------------------

/// `rotate_angle(u, mu, nullptr, seed)` (`src/math_functions.cpp:772`): rotate the
/// unit direction `u` by `mu` with an azimuth sampled uniformly (one draw, or
/// `phi` when given). Differs from [`rotate_direction`] only in the near-pole
/// branch (`1 - w^2 <= 1e-20`), where upstream expands about the `v` component
/// with `sin`/`cos` swapped, and in using `fmax` for the `a`, `b` radicands.
pub fn rotate_angle_openmc(u: Direction, mu: f64, phi: Option<f64>, seed: &mut u64) -> Direction {
    // Sample azimuthal angle in [0,2pi) if none provided
    let phi_ = match phi {
        Some(p) => p,
        None => 2.0 * PI * prn(seed),
    };
    let (sinphi, cosphi) = phi_.r_sin_cos();
    let a = (1.0 - mu * mu).max(0.0).sqrt();
    let mut b = (1.0 - u.w * u.w).max(0.0).sqrt();
    if b > 1e-10 {
        Direction::new(
            mu * u.u + a * (u.u * u.w * cosphi - u.v * sinphi) / b,
            mu * u.v + a * (u.v * u.w * cosphi + u.u * sinphi) / b,
            mu * u.w - a * b * cosphi,
        )
    } else {
        b = (1.0 - u.v * u.v).sqrt();
        Direction::new(
            mu * u.u + a * (-u.u * u.v * sinphi + u.w * cosphi) / b,
            mu * u.v + a * b * sinphi,
            mu * u.w - a * (u.v * u.w * sinphi + u.u * cosphi) / b,
        )
    }
}

/// The CM -> lab block of `inelastic_scatter` (`src/physics.cpp:1169-1179`) plus
/// its clamp of `|mu| > 1` to `+-1`: `(E_lab, mu_lab)` from the incident lab energy
/// `e_in`, the outgoing CM energy `e_cm`, the CM cosine `mu` and the target mass
/// ratio `awr`. [`cm_to_lab`] is the same transform in another operation order.
pub fn inelastic_cm_to_lab_openmc(e_in: f64, e_cm: f64, mu: f64, awr: f64) -> (f64, f64) {
    let a = awr;
    let e = e_cm + (e_in + 2.0 * mu * (a + 1.0) * (e_in * e_cm).sqrt()) / ((a + 1.0) * (a + 1.0));
    let mu_lab = mu * (e_cm / e).sqrt() + 1.0 / (a + 1.0) * (e_in / e).sqrt();
    let mu_lab = if mu_lab.abs() > 1.0 { 1.0_f64.copysign(mu_lab) } else { mu_lab };
    (e, mu_lab)
}

/// `LevelInelastic::sample` with the parameters `openmc.data` derives
/// (`reaction.py:1274-1275`): `threshold = (A + 1)/A * |Q|`, `mass_ratio =
/// (A/(A + 1))^2`, `E_cm = mass_ratio * (E - threshold)`.
pub fn level_inelastic_e_cm_openmc(e: f64, awr: f64, q: f64) -> f64 {
    let threshold = (awr + 1.0) / awr * q.abs();
    let mass_ratio = (awr / (awr + 1.0)) * (awr / (awr + 1.0));
    mass_ratio * (e - threshold)
}

/// A discrete-level inelastic scatter as OpenMC does it: [`level_inelastic_e_cm_openmc`],
/// [`inelastic_cm_to_lab_openmc`], then [`rotate_angle_openmc`] by the lab cosine
/// (`inelastic_scatter`, `src/physics.cpp:1157`). `mu_cm` is drawn by the caller
/// BEFORE this is called, as upstream draws the cosine inside `sample` before
/// `rotate_angle` draws the azimuth. Returns `(E_lab, u_out)`.
pub fn level_inelastic_scatter_openmc(
    e: f64,
    u: Direction,
    awr: f64,
    q: f64,
    mu_cm: f64,
    seed: &mut u64,
) -> (f64, Direction) {
    let e_cm = level_inelastic_e_cm_openmc(e, awr, q);
    let (e_lab, mu_lab) = inelastic_cm_to_lab_openmc(e, e_cm, mu_cm, awr);
    (e_lab, rotate_angle_openmc(u, mu_lab, None, seed))
}

/// Which target-velocity sampler `sample_target_velocity` ends up using
/// (OpenMC `ResScatMethod`, `constants.h:274`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetMethod {
    Cxs,
    Rvs,
    Dbrc,
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `lower_bound_index` (`include/openmc/search.h`): `0` when the first element
/// equals `value`, else `lower_bound(value) - first - 1` (so a value equal to a
/// grid point `k > 0` gives `k - 1`, unlike an upper-bound search). Signed, as upstream's
/// `difference_type` (`-1` when `value` is below the first element).
fn lower_bound_index_openmc(slice: &[f64], value: f64) -> isize {
    if slice[0] == value {
        return 0;
    }
    slice.partition_point(|&x| x < value) as isize - 1
}

impl DbrcTable {
    /// The 0 K cross section as OpenMC holds it: negative values zeroed in place
    /// for every point but the last (`nuclide.cpp:489-491` loops `i < size - 1`).
    fn xs0(&self, i: usize) -> f64 {
        let v = self.xs[i];
        if i + 1 < self.xs.len() && v < 0.0 {
            0.0
        } else {
            v
        }
    }

    /// `Nuclide::elastic_xs_0K` (`src/nuclide.cpp:589`).
    fn elastic_xs_0k_openmc(&self, e: f64) -> f64 {
        let n = self.energy.len();
        let mut i_grid = if e < self.energy[0] {
            0
        } else if e > self.energy[n - 1] {
            n - 2
        } else {
            lower_bound_index_openmc(&self.energy, e) as usize
        };
        // check for rare case where two energy points are the same
        if self.energy[i_grid] == self.energy[i_grid + 1] {
            i_grid += 1;
        }
        let f = (e - self.energy[i_grid]) / (self.energy[i_grid + 1] - self.energy[i_grid]);
        (1.0 - f) * self.xs0(i_grid) + f * self.xs0(i_grid + 1)
    }

    /// Window index of `sample_target_velocity` (`physics.cpp:900-925`).
    fn window_index_openmc(&self, e: f64) -> usize {
        let n = self.energy.len();
        if e < self.energy[0] {
            0
        } else if e > self.energy[n - 1] {
            n - 2
        } else {
            lower_bound_index_openmc(&self.energy, e) as usize
        }
    }
}

/// `sample_cxs_target_velocity` (`src/physics.cpp:1028`): the target velocity in
/// the constant-cross-section approximation, in `|v| = sqrt(E)` units.
fn sample_cxs_target_velocity_openmc(awr: f64, e: f64, u: Direction, kt: f64, seed: &mut u64) -> [f64; 3] {
    let beta_vn = (awr * e / kt).sqrt();
    let alpha = 1.0 / (1.0 + PI.sqrt() * beta_vn / 2.0);
    let (mut beta_vt_sq, mut mu);
    loop {
        // Sample two random numbers
        let r1 = prn(seed);
        let r2 = prn(seed);
        if prn(seed) < alpha {
            // sampling scheme C45 from the Monte Carlo sampler
            beta_vt_sq = -(r1 * r2).r_ln();
        } else {
            // sampling scheme C61
            let c = (PI / 2.0 * prn(seed)).r_cos();
            beta_vt_sq = -r1.r_ln() - r2.r_ln() * c * c;
        }
        // Determine beta * vt
        let beta_vt = beta_vt_sq.sqrt();
        // Sample cosine of angle between neutron and target velocity
        mu = -1.0 + (1.0 - (-1.0)) * prn(seed);
        // Determine rejection probability
        let accept_prob = (beta_vn * beta_vn + beta_vt_sq - 2.0 * beta_vn * beta_vt * mu).sqrt()
            / (beta_vn + beta_vt);
        // Perform rejection sampling on vt and mu
        if prn(seed) < accept_prob {
            break;
        }
    }
    // Determine speed of target nucleus
    let vt = (beta_vt_sq * kt / awr).sqrt();
    // Determine velocity vector of target nucleus based on neutron's velocity
    // and the sampled angle between them
    let d = rotate_angle_openmc(u, mu, None, seed);
    [vt * d.u, vt * d.v, vt * d.w]
}

/// `sample_target_velocity` (`src/physics.cpp:874`): the target nucleus velocity
/// for a collision of a neutron of lab energy `e` and velocity `v_neut`.
/// `resonant` is the nuclide's 0 K table (`nuc.resonant_` is `resonant.is_some()`;
/// its `e_max_ev` is `res_scat_energy_max`, `res_scat_energy_min` is upstream's
/// 0.01 eV, and the method is the table's).
fn sample_target_velocity_openmc(
    awr: f64,
    resonant: Option<&DbrcTable>,
    e: f64,
    u: Direction,
    v_neut: [f64; 3],
    kt: f64,
    seed: &mut u64,
) -> [f64; 3] {
    const RES_SCAT_ENERGY_MIN: f64 = 0.01;
    let mut method;
    match resonant {
        Some(t) => {
            method = match t.method {
                ResonanceScatteringMethod::Rvs => TargetMethod::Rvs,
                ResonanceScatteringMethod::Dbrc => TargetMethod::Dbrc,
            };
            // upper resonance scattering energy bound (target is at rest above this E)
            if e > t.e_max {
                return [0.0; 3];
                // lower resonance scattering energy bound (should be no resonances below)
            } else if e < RES_SCAT_ENERGY_MIN {
                method = TargetMethod::Cxs;
            }
        }
        // otherwise, use free gas model
        None => {
            if e >= FREE_GAS_THRESHOLD * kt && awr > 1.0 {
                return [0.0; 3];
            }
            method = TargetMethod::Cxs;
        }
    }

    match (method, resonant) {
        (TargetMethod::Rvs | TargetMethod::Dbrc, Some(t)) => {
            let e_red = (awr * e / kt).sqrt();
            let e_low = (e_red - 4.0).max(0.0).powi(2) * kt / awr;
            let e_up = (e_red + 4.0) * (e_red + 4.0) * kt / awr;
            let i_e_low = t.window_index_openmc(e_low);
            let i_e_up = t.window_index_openmc(e_up);
            if i_e_up == i_e_low {
                // Handle degenerate case -- the bounds fall in one interval
                return sample_cxs_target_velocity_openmc(awr, e, u, kt, seed);
            }
            let en = &t.energy;
            if method == TargetMethod::Dbrc {
                // interpolate xs since we're not exactly at the energy indices
                let mut xs_low = t.xs0(i_e_low);
                let mut m = (t.xs0(i_e_low + 1) - xs_low) / (en[i_e_low + 1] - en[i_e_low]);
                xs_low += m * (e_low - en[i_e_low]);
                let mut xs_up = t.xs0(i_e_up);
                m = (t.xs0(i_e_up + 1) - xs_up) / (en[i_e_up + 1] - en[i_e_up]);
                xs_up += m * (e_up - en[i_e_up]);
                // get max 0K xs value over range of practical relative energies
                // (`std::max_element` over [i_E_low + 1, i_E_up + 1))
                let mut xs_max = t.xs0(i_e_low + 1);
                for k in (i_e_low + 2)..(i_e_up + 1) {
                    if t.xs0(k) > xs_max {
                        xs_max = t.xs0(k);
                    }
                }
                xs_max = xs_low.max(xs_max).max(xs_up);
                loop {
                    let (mut e_rel, mut v_target);
                    loop {
                        // sample target velocity with the cxs approximation
                        v_target = sample_cxs_target_velocity_openmc(awr, e, u, kt, seed);
                        let v_rel = [v_neut[0] - v_target[0], v_neut[1] - v_target[1], v_neut[2] - v_target[2]];
                        e_rel = dot3(v_rel, v_rel);
                        if e_rel < e_up {
                            break;
                        }
                    }
                    // perform Doppler broadening rejection correction (dbrc)
                    let xs_0k = t.elastic_xs_0k_openmc(e_rel);
                    let r = xs_0k / xs_max;
                    if prn(seed) < r {
                        return v_target;
                    }
                }
            } else {
                // rvs: interpolate xs CDF since we're not exactly at the energy indices
                let cdf = &t.cdf;
                let mut cdf_low = 0.0;
                if e_low > en[0] {
                    let m = (cdf[i_e_low + 1] - cdf[i_e_low]) / (en[i_e_low + 1] - en[i_e_low]);
                    cdf_low = cdf[i_e_low] + m * (e_low - en[i_e_low]);
                }
                let m = (cdf[i_e_up + 1] - cdf[i_e_up]) / (en[i_e_up + 1] - en[i_e_up]);
                let cdf_up = cdf[i_e_up] + m * (e_up - en[i_e_up]);
                loop {
                    // directly sample Maxwellian
                    let mut e_t = -kt * prn(seed).r_ln();
                    // sample a relative energy using the xs cdf
                    let cdf_rel = cdf_low + prn(seed) * (cdf_up - cdf_low);
                    let i_e_rel = lower_bound_index_openmc(&cdf[i_e_low..i_e_up + 2], cdf_rel) as usize;
                    let mut e_rel = en[i_e_low + i_e_rel];
                    let m = (cdf[i_e_low + i_e_rel + 1] - cdf[i_e_low + i_e_rel])
                        / (en[i_e_low + i_e_rel + 1] - en[i_e_low + i_e_rel]);
                    e_rel += (cdf_rel - cdf[i_e_low + i_e_rel]) / m;
                    // rejection sampling on cosine between neutron and target velocities
                    let mu = (e_t + awr * (e - e_rel)) / (2.0 * (awr * e * e_t).sqrt());
                    if mu.abs() < 1.0 {
                        // set and accept target velocity
                        e_t /= awr;
                        let d = rotate_angle_openmc(u, mu, None, seed);
                        let s = e_t.sqrt();
                        return [s * d.u, s * d.v, s * d.w];
                    }
                }
            }
        }
        _ => sample_cxs_target_velocity_openmc(awr, e, u, kt, seed),
    }
}

/// `elastic_scatter` (`src/physics.cpp:792`): an elastic collision of a neutron
/// of lab energy `e` and direction `u` with a target of mass ratio `awr` at
/// `kt` \[eV\], with the target velocity from `sample_target_velocity` (target at
/// rest above `400 kT`, or above the resonance window of a resonant nuclide).
///
/// `use_ptable` is `p.neutron_xs(i).use_ptable` (target at rest when the URR
/// probability table is in use). **`sample_mu_cm` is called AFTER the target
/// velocity has been sampled**, as upstream draws the CM cosine there: with
/// `Some` angular law it is `angle().sample(E, seed)`, with none
/// `uniform_distribution(-1, 1, seed)`. (The older
/// [`free_gas_elastic_scatter_dbrc`] takes the cosine drawn BEFORE the target
/// velocity, so for a free-gas collision the two consume the stream in a
/// different order: the same distribution, a different path.)
///
/// Returns `(E_out, u_out, mu_lab)`.
#[allow(clippy::too_many_arguments)]
pub fn elastic_scatter_openmc<F: FnOnce(&mut u64) -> f64>(
    e: f64,
    u: Direction,
    awr: f64,
    kt: f64,
    resonant: Option<&DbrcTable>,
    use_ptable: bool,
    sample_mu_cm: F,
    seed: &mut u64,
) -> (f64, Direction, f64) {
    let mut vel = e.sqrt();
    // Neutron velocity in LAB
    let mut v_n = [vel * u.u, vel * u.v, vel * u.w];
    // Sample velocity of target nucleus
    let v_t = if !use_ptable {
        sample_target_velocity_openmc(awr, resonant, e, u, v_n, kt, seed)
    } else {
        [0.0; 3]
    };
    // Velocity of center-of-mass
    let v_cm = [
        (v_n[0] + awr * v_t[0]) / (awr + 1.0),
        (v_n[1] + awr * v_t[1]) / (awr + 1.0),
        (v_n[2] + awr * v_t[2]) / (awr + 1.0),
    ];
    // Transform to CM frame
    v_n = [v_n[0] - v_cm[0], v_n[1] - v_cm[1], v_n[2] - v_cm[2]];
    // Find speed of neutron in CM
    vel = dot3(v_n, v_n).sqrt();
    // Sample scattering angle
    let mu_cm = sample_mu_cm(seed);
    // Determine direction cosines in CM
    let u_cm = Direction::new(v_n[0] / vel, v_n[1] / vel, v_n[2] / vel);
    // Rotate neutron velocity vector to new angle
    let d = rotate_angle_openmc(u_cm, mu_cm, None, seed);
    v_n = [vel * d.u, vel * d.v, vel * d.w];
    // Transform back to LAB frame
    v_n = [v_n[0] + v_cm[0], v_n[1] + v_cm[1], v_n[2] + v_cm[2]];
    let e_out = dot3(v_n, v_n);
    vel = e_out.sqrt();
    // cosine of scattering angle in LAB frame by the dot product of the
    // pre- and post-collision directions
    let mut mu = (u.u * v_n[0] + u.v * v_n[1] + u.w * v_n[2]) / vel;
    let u_out = Direction::new(v_n[0] / vel, v_n[1] / vel, v_n[2] / vel);
    // Because of floating-point roundoff, mu_lab may leave [-1,1)
    if mu.abs() > 1.0 {
        mu = 1.0_f64.copysign(mu);
    }
    (e_out, u_out, mu)
}


#[cfg(test)]
mod tests {

    /// **DBRC samples the target as OpenMC's kernel does (GitHub #407).**
    ///
    /// The DBRC target-velocity density is the constant-cross-section (CXS)
    /// density times `σ⁰ᴷ(E_rel)` over OpenMC's window. So a histogram of
    /// `E_rel` from the DBRC sampler must equal the CXS sampler's histogram
    /// **reweighted by `σ⁰ᴷ(E_rel)`**. That is an importance-weighting
    /// identity, independent of how the rejection is coded.
    ///
    /// Synthetic 0 K elastic: 10 b background plus a Breit-Wigner peak of
    /// 5000 b at 20.0 eV, `Γ = 0.02 eV`, on a 1 meV grid. AWR 236, kT 0.0253 eV,
    /// incident 20.3 eV. Every defect the #407 survey named fails here:
    /// - 20.3 eV is above 400 kT, where the old gate held the target at rest;
    /// - the peak sits outside the old half-width window, so its envelope
    ///   missed it and the acceptance clamped;
    /// - candidates above `E_up` were not resampled.
    ///
    /// Pass: every one of 30 bins over the window within 5 sigma.
    #[test]
    fn dbrc_target_density_is_cxs_times_sigma_0k() {
        let (a, kt, e) = (236.0_f64, 0.0253_f64, 20.3_f64);
        let sig = |x: f64| 10.0 + 5000.0 / (1.0 + ((x - 20.0) / 0.01).r_powi(2));
        let pairs: Vec<(f64, f64)> =
            (0..20_000).map(|i| 12.0 + 0.001 * i as f64).map(|x| (x, sig(x))).collect();
        let t = DbrcTable::from_pairs(&pairs, 1.0e3).unwrap();
        let u = Direction::new(0.0, 0.0, 1.0);
        let vn = e.sqrt();
        let e_rel = |v_t: [f64; 3]| {
            let d = [-v_t[0], -v_t[1], vn - v_t[2]];
            d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
        };
        let beta_vn = (a * e / kt).sqrt();
        let (lo, hi) = (
            (beta_vn - 4.0).max(0.0).r_powi(2) * kt / a,
            (beta_vn + 4.0).r_powi(2) * kt / a,
        );
        const NB: usize = 30;
        const N: usize = 400_000;
        let bin = |x: f64| (((x - lo) / (hi - lo)) * NB as f64).floor() as isize;
        let mut h_dbrc = [0.0_f64; NB];
        let mut seed = 0xDB_0001_u64;
        let mut outside = 0usize;
        for _ in 0..N {
            let v = sample_target_velocity_dbrc(e, u, a, kt, &mut seed, Some(&t));
            let b = bin(e_rel(v));
            if (0..NB as isize).contains(&b) {
                h_dbrc[b as usize] += 1.0;
            } else {
                outside += 1;
            }
        }
        assert_eq!(outside, 0, "DBRC must never return E_rel outside OpenMC's window");
        let (mut h_w, mut h_w2) = ([0.0_f64; NB], [0.0_f64; NB]);
        let mut seed2 = 0xDB_0002_u64;
        const M: usize = 2_000_000;
        for _ in 0..M {
            let v = sample_target_velocity_dbrc(e, u, a, kt, &mut seed2, None);
            let x = e_rel(v);
            let b = bin(x);
            if (0..NB as isize).contains(&b) {
                let w = t.xs_at(x);
                h_w[b as usize] += w;
                h_w2[b as usize] += w * w;
            }
        }
        let (sd, sw): (f64, f64) = (h_dbrc.iter().sum(), h_w.iter().sum());
        let mut worst: f64 = 0.0;
        for k in 0..NB {
            let (p, q) = (h_dbrc[k] / sd, h_w[k] / sw);
            let var_p = p * (1.0 - p) / sd;
            let var_q = h_w2[k] / (sw * sw);
            let z = (p - q) / (var_p + var_q).sqrt().max(1e-12);
            worst = worst.max(z.abs());
            assert!(z.abs() < 5.0, "bin {k}: DBRC {p:.5} vs CXS x sigma {q:.5}, z {z:.2}");
        }
        println!("DBRC vs CXS x sigma_0K: worst |z| over {NB} bins = {worst:.2}");

        // The gate: a resonant nuclide's target moves up to the table's
        // `e_max`, so the neutron can gain energy at 20.3 eV (target at rest
        // caps E' at E). It was held at rest above 400 kT before GitHub #407.
        let mut s3 = 0xDB_0003_u64;
        let up = (0..20_000)
            .filter(|_| {
                let mu = 2.0 * prn(&mut s3) - 1.0;
                free_gas_elastic_scatter_dbrc(e, u, a, kt, mu, &mut s3, Some(&t)).0 > e
            })
            .count();
        assert!(up > 1000, "only {up} of 20000 up-scattered: target held at rest");
    }

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
            let alpha = ((awr - 1.0) / (awr + 1.0)).r_powi(2);
            let xi = 1.0 + alpha * alpha.r_ln() / (1.0 - alpha);
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
        e.sqrt() * ((1.0 + 1.0 / (2.0 * a * a)) * erf(a) + (-a * a).r_exp() / (a * PI.sqrt()))
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
        sign * (1.0 - poly * (-x * x).r_exp())
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
        let alpha = ((awr - 1.0) / (awr + 1.0)).r_powi(2);
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
                continuum_inelastic_scatter(e, Direction::new(0.0, 0.0, 1.0), awr, 0.0, &mut seed);
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
