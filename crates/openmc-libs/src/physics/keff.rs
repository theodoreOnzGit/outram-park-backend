//! k-eigenvalue power iteration for a homogeneous bare sphere.
//!
//! This is the minimal criticality driver — the first end-to-end assembly of the
//! transport kernel described in `docs/keff-doppler-roadmap.md` (Priority 1). It
//! deliberately handles only the simplest geometry (one sphere, vacuum outside,
//! one homogeneous material) so the physics can be exercised without the full CSG
//! machinery. The pieces it composes:
//!
//! - **Geometry** — [`crate::geometry::surface::Sphere::distance`] for the one
//!   surface crossing; "inside" is just `|r| < R`.
//! - **Data** — macroscopic cross sections from [`Material`], which pulls
//!   microscopic σ(E,T) from `njoy-outram-park-fork` via [`Nuclide`].
//! - **Physics** — analog collisions: elastic scatter
//!   ([`crate::physics::scatter::elastic_scatter`]), fission banking
//!   ([`crate::physics::fission::sample_num_neutrons`]), and analog capture.
//! - **Source** — Watt fission energy + isotropic direction for banked neutrons.
//!
//! # Algorithm
//!
//! Standard fission-source power iteration. Each *generation* transports
//! `n_particles` histories from the current fission bank; every fission event
//! contributes ν̄ to the generation's production tally and banks ⌊ν̄/k⌋(+1) sites
//! for the next generation. The generation eigenvalue is
//! `k = (Σ ν̄ over fissions) / n_particles`. The first `n_inactive` generations
//! let the source distribution converge and are discarded; the mean over the
//! remaining `n_active` generations is the reported k, with the standard error of
//! that mean.
//!
//! # Fidelity
//!
//! Analog transport (no implicit capture / weight windows), target at rest. Both
//! data tiers now model inelastic down-scatter and forward-peaked elastic; they
//! differ in how finely that physics is resolved:
//!
//! - **HIGH tier** ([`Nuclide::from_endf`]) carries the resolved inelastic level
//!   structure (MT=51…91), so inelastic is a distinct channel with a real
//!   energy-loss law — discrete-level two-body kinematics (each level's Q-value)
//!   and a Weisskopf-evaporation continuum. Elastic uses the full ENDF MF=4
//!   anisotropic angular distribution (per-energy tabulated cosine CDF). `(n,2n)`
//!   (MT=16, from the reconstructed MF=3 background) is a distinct channel that
//!   emits its true **yield-2 multiplicity** — one extra same-generation neutron,
//!   the small positive reactivity a bare fast sphere would otherwise drop.
//! - **LOW tier** ([`Nuclide::from_core`]) has no resolved levels: inelastic is the
//!   group remainder (total − elastic − fission − capture), down-scattered by the
//!   Weisskopf continuum law. Elastic is forward-peaked from a single per-group
//!   mean cosine μ̄ (baked from MF=4) via a maximum-entropy exponential angular law.
//!   Above each nuclide's WMP `e_max` the group data is infinite-dilution
//!   Watt-collapsed with no self-shielding. `(n,2n)` has no group column yet, so
//!   the LOW tier still lumps it into elastic (no multiplication) — a pending bake.
//!
//! For a bare fast sphere, forward-peaked elastic and inelastic down-scatter are
//! the dominant reactivity levers — together they bring **both** tiers' Godiva Keff
//! into agreement with the ICSBEP benchmark (see `docs/development-history.md`).
//!
//! # Example
//!
//! ```no_run
//! use openmc_libs::material::material::{Material, NuclideComponent};
//! use openmc_libs::material::nuclide::Nuclide;
//! use openmc_libs::physics::keff::{run_keff, KeffSettings};
//!
//! // Godiva: bare HEU sphere, r ≈ 8.741 cm.
//! let nuclides = vec![
//!     Nuclide::from_core("U234").unwrap(),
//!     Nuclide::from_core("U235").unwrap(),
//!     Nuclide::from_core("U238").unwrap(),
//! ];
//! let mat = Material {
//!     id: 1,
//!     name: "HEU".into(),
//!     temperature: 293.6,
//!     components: vec![
//!         NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
//!         NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
//!         NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
//!     ],
//! };
//! let result = run_keff(8.7407, &mat, &nuclides, &KeffSettings::default());
//! println!("k = {:.5} ± {:.5}", result.k_mean, result.k_std);
//! ```

use crate::geometry::position::{stream, Direction, Position};
use crate::geometry::surface::{BoundaryType, Sphere, Surface};
use crate::material::material::Material;
use crate::material::nuclide::{Inelastic, Nuclide};
use crate::physics::fission::sample_num_neutrons;
use crate::physics::scatter::{
    continuum_inelastic_scatter, elastic_scatter, two_body_scatter, two_body_scatter_with_mu,
};
use crate::rng::distributions::{isotropic_direction, watt};
use crate::rng::lcg::prn;

/// Settings for a [`run_keff`] power iteration.
#[derive(Debug, Clone, Copy)]
pub struct KeffSettings {
    /// Neutron histories per generation. More ⇒ lower per-generation noise.
    pub n_particles: usize,
    /// Inactive (source-convergence) generations, discarded from the k tally.
    pub n_inactive: usize,
    /// Active generations averaged into the reported eigenvalue.
    pub n_active: usize,
    /// Material/data temperature \[K\] used for Doppler-broadened lookups.
    pub temperature_k: f64,
    /// Master RNG seed. Fixed seed ⇒ bit-reproducible run.
    pub seed: u64,
    /// Watt fission-spectrum parameter `a` \[eV\] for banked neutron energies.
    pub watt_a: f64,
    /// Watt fission-spectrum parameter `b` \[eV⁻¹\].
    pub watt_b: f64,
}

impl Default for KeffSettings {
    /// A modest run (2000 histories × [30 inactive + 70 active]) with the
    /// U-235 thermal Watt spectrum. Enough for a first-look Keff in seconds.
    fn default() -> Self {
        Self {
            n_particles: 2000,
            n_inactive: 30,
            n_active: 70,
            temperature_k: 293.6,
            seed: 1,
            watt_a: 0.988e6,
            watt_b: 2.249e-6,
        }
    }
}

/// Result of a [`run_keff`] power iteration.
#[derive(Debug, Clone)]
pub struct KeffResult {
    /// Mean eigenvalue over the active generations.
    pub k_mean: f64,
    /// Standard error of the mean (1σ) over the active generations.
    pub k_std: f64,
    /// Per-generation eigenvalue estimates, all generations (inactive first).
    pub k_by_generation: Vec<f64>,
}

/// A fission-source neutron awaiting transport in the next generation.
#[derive(Clone, Copy)]
struct Site {
    r: Position,
    u: Direction,
    e: f64,
}

/// Run fission-source power iteration on a bare sphere of radius `radius_cm`
/// (centred at the origin, vacuum outside) filled with `material`.
///
/// `nuclides` is the global nuclide array the material's components index into.
/// Returns the mean eigenvalue and its standard error over the active
/// generations. See the module docs for the algorithm and fidelity caveats.
pub fn run_keff(
    radius_cm: f64,
    material: &Material,
    nuclides: &[Nuclide],
    settings: &KeffSettings,
) -> KeffResult {
    let sphere = Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: radius_cm, bc: BoundaryType::Vacuum };
    let mut seed = settings.seed;
    let temp = settings.temperature_k;

    // Initial source: uniform in the sphere volume, isotropic, Watt energy.
    let mut source: Vec<Site> = (0..settings.n_particles)
        .map(|_| {
            let (dx, dy, dz) = isotropic_direction(&mut seed);
            let rr = radius_cm * prn(&mut seed).cbrt(); // uniform-in-volume radius
            Site {
                r: Position::new(rr * dx, rr * dy, rr * dz),
                u: Direction::new(dx, dy, dz),
                e: watt(&mut seed, settings.watt_a, settings.watt_b),
            }
        })
        .collect();

    let n_gen = settings.n_inactive + settings.n_active;
    let mut k_by_generation = Vec::with_capacity(n_gen);
    let mut k_running = 1.0; // guess feeding the site-count normalisation
    let mut active_k = Vec::with_capacity(settings.n_active);

    for gen in 0..n_gen {
        let mut next_bank: Vec<Site> = Vec::with_capacity(settings.n_particles);
        let mut production = 0.0_f64;

        for site in &source {
            production += transport_history(
                *site, &sphere, material, nuclides, temp, k_running, settings, &mut next_bank,
                &mut seed,
            );
        }

        let k_gen = production / settings.n_particles as f64;
        k_by_generation.push(k_gen);
        k_running = k_gen;
        if gen >= settings.n_inactive {
            active_k.push(k_gen);
        }

        // Resample the next generation's source to exactly n_particles sites.
        if next_bank.is_empty() {
            // Sub-critical to extinction (or no data): nothing left to iterate.
            break;
        }
        source = resample(&next_bank, settings.n_particles, &mut seed);
    }

    let (k_mean, k_std) = mean_and_stderr(&active_k);
    KeffResult { k_mean, k_std, k_by_generation }
}

/// Transport one source neutron — plus any same-generation `(n,2n)` secondaries
/// it spawns — to death (absorption or leakage), banking any fission neutrons.
/// Returns the fission production ν̄ summed over every fission event in the
/// history (the generation-k numerator contribution).
///
/// `(n,2n)` neutrons are tracked to completion **within this generation** via a
/// local work stack, mirroring OpenMC's `create_secondary` bank (`src/physics.cpp`
/// `inelastic_scatter`): only *fission* neutrons are banked to the next
/// generation (`next_bank`); `(n,xn)` multiplicity is realized in-generation.
#[allow(clippy::too_many_arguments)]
fn transport_history(
    site: Site,
    sphere: &Sphere,
    material: &Material,
    nuclides: &[Nuclide],
    temp: f64,
    k_running: f64,
    settings: &KeffSettings,
    next_bank: &mut Vec<Site>,
    seed: &mut u64,
) -> f64 {
    let mut production = 0.0;
    // Same-generation work stack: the source neutron plus any (n,2n) secondaries.
    let mut stack: Vec<Site> = vec![site];

    while let Some(start) = stack.pop() {
        let mut r = start.r;
        let mut u = start.u;
        let mut e = start.e;

        loop {
            let sigma_t = material.macro_xs_total(e, nuclides);
            if !(sigma_t > 0.0) {
                break; // no interaction possible; treat as escape
            }
            let d_col = -prn(seed).ln() / sigma_t;
            let d_bound = sphere.distance(r, u, false);

            if d_col >= d_bound {
                break; // reaches the vacuum boundary first → leaks
            }

            // Collide: advance to the collision site and pick the target nuclide.
            r = stream(r, u, d_col);
            let ci = material.sample_nuclide(e, seed, nuclides);
            let nuc = &nuclides[material.components[ci].nuclide_idx];
            let x = nuc.xs_at_energy(e, temp);

            // Reaction partition on the *total*:
            //   fission | capture | inelastic | (n,2n) | elastic.
            // `x.inelastic` (MT=51…91) and `x.n2n` (MT=16) are sub-bands of
            // scattering carved out with their own laws; both are non-zero only for
            // the HIGH tier, so the LOW tier collapses to the fission | capture |
            // elastic split. The final elastic bucket (total − absorption −
            // inelastic − n2n) sweeps up any remaining scattering as elastic-like.
            let xi = prn(seed) * x.total;
            if xi < x.fission {
                let nu_bar = if x.fission > 0.0 { x.nu_fission / x.fission } else { 0.0 };
                production += nu_bar;
                let n = sample_num_neutrons(nu_bar, k_running, seed);
                for _ in 0..n {
                    let (dx, dy, dz) = isotropic_direction(seed);
                    next_bank.push(Site {
                        r,
                        u: Direction::new(dx, dy, dz),
                        e: watt(seed, settings.watt_a, settings.watt_b),
                    });
                }
                break; // fission is a terminal absorption for the incident neutron
            } else if xi < x.absorption {
                break; // radiative capture → dead
            } else if xi < x.absorption + x.inelastic {
                // Inelastic scatter with a real energy-loss law: a discrete level's
                // two-body kinematics (Q-value) or continuum evaporation. This is
                // the dominant fast-spectrum down-scatter off heavy nuclei.
                let (e2, u2) = match nuc.sample_inelastic(e, seed) {
                    Inelastic::Level { q } => two_body_scatter(e, u, nuc.awr, q, seed),
                    Inelastic::Continuum => continuum_inelastic_scatter(e, u, nuc.awr, seed),
                };
                e = e2;
                u = u2;
            } else if xi < x.absorption + x.inelastic + x.n2n {
                // (n,2n): the incident neutron down-scatters and one extra neutron
                // is emitted — the yield-2 multiplicity that restores the neutron
                // a bare fast sphere would otherwise lose. Ported from OpenMC
                // `inelastic_scatter` (src/physics.cpp:1167-1177): for an integral
                // yield Y it calls `create_secondary` Y−1 times with the *primary's*
                // post-scatter energy and direction, so the second neutron shares
                // the sampled outgoing state. We lack a parsed MF=6 (n,2n) emission
                // law, so the outgoing energy uses the same Weisskopf-evaporation
                // continuum as MT=91 inelastic — faithful to the multiplicity, a
                // stand-in for the emission spectrum.
                let (e2, u2) = continuum_inelastic_scatter(e, u, nuc.awr, seed);
                stack.push(Site { r, u: u2, e: e2 }); // yield − 1 = 1 secondary
                e = e2;
                u = u2;
            } else {
                // Elastic. Use the ENDF MF=4 angular distribution when the nuclide
                // carries one (HIGH tier) — fast neutrons scatter forward off heavy
                // nuclei, which raises bare-sphere leakage — else isotropic-CM.
                let (e2, u2) = match nuc.sample_elastic_mu_cm(e, seed) {
                    Some(mu_cm) => two_body_scatter_with_mu(e, u, nuc.awr, 0.0, mu_cm, seed),
                    None => elastic_scatter(e, u, nuc.awr, seed),
                };
                e = e2;
                u = u2;
            }
        }
    }
    production
}

/// Resample `n` sites uniformly with replacement from `bank` — the crude
/// population control that renormalises the fission bank back to a fixed source
/// size each generation.
fn resample(bank: &[Site], n: usize, seed: &mut u64) -> Vec<Site> {
    let len = bank.len();
    (0..n)
        .map(|_| {
            let idx = ((prn(seed) * len as f64) as usize).min(len - 1);
            bank[idx]
        })
        .collect()
}

/// Mean and standard error of the mean (1σ) of the active-generation eigenvalues.
fn mean_and_stderr(k: &[f64]) -> (f64, f64) {
    let n = k.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mean = k.iter().sum::<f64>() / n as f64;
    if n < 2 {
        return (mean, 0.0);
    }
    let var = k.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
    (mean, (var / n as f64).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::material::NuclideComponent;

    /// **LOW-fidelity Godiva V&V** (HEU-MET-FAST-001).
    ///
    /// **Methodology.** Bare HEU sphere, r = 8.7407 cm, ICSBEP atom densities
    /// (U-234/235/238), 1500 histories × [20 inactive + 40 active], cross sections
    /// from the embedded LOW tier (WMP below `e_max` + infinite-dilution
    /// Watt-collapsed fast MGXS above — now with per-group μ̄ for forward elastic and
    /// inelastic carved from the group total). Reference: ICSBEP k_eff =
    /// 1.0000 ± 0.0010. Pass criterion is deliberately a *broad* plausibility band
    /// (0.9–1.4), not a benchmark gate — this guards the full transport chain (data
    /// → geometry → collision → scatter → fission → power iteration), not accuracy.
    ///
    /// **Results (2026-07).** k_eff ≈ 1.010 ± 0.002, i.e. ~+1 000 pcm high (down
    /// from ~1.129 / +12 900 pcm before the LOW tier gained inelastic + forward
    /// elastic scatter — see `docs/development-history.md`). The result is
    /// stationary and low-noise; the small residual bias is expected for this
    /// fidelity (no self-shielding; one mean cosine; evaporation for inelastic).
    #[test]
    fn godiva_converges_to_sane_keff() {
        let nuclides = vec![
            Nuclide::from_core("U234").unwrap(),
            Nuclide::from_core("U235").unwrap(),
            Nuclide::from_core("U238").unwrap(),
        ];
        let material = Material {
            id: 1,
            name: "Godiva".into(),
            temperature: 293.6,
            components: vec![
                NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
                NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
                NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
            ],
        };
        let settings = KeffSettings {
            n_particles: 1500,
            n_inactive: 20,
            n_active: 40,
            ..KeffSettings::default()
        };
        let result = run_keff(8.7407, &material, &nuclides, &settings);

        assert_eq!(result.k_by_generation.len(), 60, "ran all generations");
        assert!(
            result.k_mean > 0.9 && result.k_mean < 1.4,
            "Godiva k_eff {} outside the plausible first-cut band [0.9, 1.4]",
            result.k_mean
        );
        assert!(result.k_std < 0.02, "k noisy/unconverged: σ = {}", result.k_std);
    }

    /// A far-subcritical configuration (tiny sphere ⇒ leakage-dominated) must
    /// come out well below the critical Godiva sphere — a sign check that the
    /// geometry/leakage coupling actually bites.
    #[test]
    fn small_sphere_is_less_reactive_than_godiva() {
        let nuclides = vec![Nuclide::from_core("U235").unwrap()];
        let material = Material {
            id: 1,
            name: "U235".into(),
            temperature: 293.6,
            components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 4.8e-2 }],
        };
        let settings = KeffSettings { n_particles: 1000, n_inactive: 15, n_active: 25, ..KeffSettings::default() };

        let k_big = run_keff(9.0, &material, &nuclides, &settings).k_mean;
        let k_small = run_keff(3.0, &material, &nuclides, &settings).k_mean;
        assert!(
            k_small < k_big,
            "3 cm sphere (k={k_small}) should leak more than 9 cm (k={k_big})"
        );
    }

    /// **HIGH-fidelity Godiva V&V — the benchmark result** — behind the
    /// `net-fetch` feature (downloads ENDF; not part of the default offline suite).
    ///
    /// **Methodology.** The same Godiva model and power-iteration settings are run
    /// under both data tiers, judged against ICSBEP HEU-MET-FAST-001
    /// (k_eff = 1.0000 ± 0.0010):
    /// - **LOW** ([`Nuclide::from_core`]) — embedded WMP + infinite-dilution fast
    ///   MGXS. Now carries the same two transport-physics levers as HIGH, reduced to
    ///   group data: inelastic as the group remainder (Weisskopf evaporation) and
    ///   forward-peaked elastic from a per-group mean cosine μ̄ (max-entropy
    ///   exponential law). No self-shielding, one μ̄ instead of the full shape.
    /// - **HIGH** ([`Nuclide::from_endf`]) — ENDF/B-VII.1 downloaded and
    ///   reconstructed on device (RECONR 0.1% tol + BROADR to 293.6 K + MF=1/452
    ///   ν̄), continuous-energy σ(E), an explicit inelastic energy-loss law
    ///   (MT=51…91 two-body + evaporation), **anisotropic (full ENDF MF=4)
    ///   elastic scatter** (ported from OpenMC `AngleDistribution`/`Tabular`), and
    ///   **(n,2n) with its true yield-2 multiplicity** (MT=16 from the MF=3
    ///   background; one extra same-generation neutron per event, ported from
    ///   OpenMC `inelastic_scatter`, `src/physics.cpp:1167`).
    ///
    /// The test asserts that **both** tiers converge to a stationary eigenvalue
    /// near unity — HIGH from continuous-energy data and the full MF=4 shape, LOW
    /// from coarse group data plus a single per-group μ̄ — confirming the two levers
    /// (energy transfer + forward peaking) are what close the Godiva gap and that
    /// they survive the reduction to group data.
    ///
    /// **Results (2026-07-03; HIGH = ENDF/B-VII.1, LOW = embedded VIII.0 group;
    /// 5000 particles, 40 inactive + 120 active generations, default seed).**
    /// LOW k_eff = **1.01024 (+1 024 pcm)**; HIGH k_eff = **0.99872 ± 0.00173
    /// (−128 pcm)** — both in agreement with the benchmark. The ranked HIGH-tier
    /// lever contributions (anisotropic elastic ~10 300 pcm ≫ inelastic ~2 510 pcm
    /// ≫ continuous-energy data ~400 pcm) are in `docs/development-history.md`.
    ///
    /// **(n,2n) multiplicity — measured worth.** A same-settings A/B (n2n on vs
    /// forced off) gives HIGH = 0.99872 ± 0.00173 (on) vs 0.99701 ± 0.00168 (off),
    /// a shift of **+171 ± 241 pcm** — the physically-correct sign (an extra
    /// neutron raises k) but only ~0.7σ, i.e. **not statistically resolved from
    /// zero** at this statistics. That is expected: U (n,2n) has a ~5–6 MeV
    /// threshold and sees only the thin high-energy tail of the fission spectrum,
    /// so its Godiva worth is genuinely tens of pcm. The change is a *fidelity /
    /// correctness* fix (mirrors OpenMC; matters for (n,xn)-sensitive spectra and
    /// Be/D-reflected systems), not a measurable mover of Godiva's k.
    ///
    /// Near-perfect landings likely involve some cancellation of residual
    /// approximations (no fast self-shielding; Weisskopf stand-in for the MF=5
    /// continuum law; fixed thermal-Watt χ instead of energy-dependent MF=5), so
    /// the bands below are deliberately generous rather than a tight accuracy gate.
    #[cfg(feature = "net-fetch")]
    #[test]
    fn godiva_high_fidelity_reaches_benchmark() {
        use njoy_outram_park_fork::acquire::EndfLibrary;

        let material = Material {
            id: 1,
            name: "Godiva".into(),
            temperature: 293.6,
            components: vec![
                NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
                NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
                NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
            ],
        };
        let settings = KeffSettings {
            n_particles: 5000,
            n_inactive: 40,
            n_active: 120,
            ..KeffSettings::default()
        };

        // LOW tier (embedded, offline) — the first-cut reference.
        let low = vec![
            Nuclide::from_core("U234").unwrap(),
            Nuclide::from_core("U235").unwrap(),
            Nuclide::from_core("U238").unwrap(),
        ];
        let k_low = run_keff(8.7407, &material, &low, &settings).k_mean;

        // HIGH tier (download + reconstruct ENDF/B-VII.1). U is Reich-Moore (LRF=3)
        // in VII.1, which the RECONR port reconstructs (VIII.0 U is LRF=7).
        let high: Vec<Nuclide> = ["U234", "U235", "U238"]
            .iter()
            .map(|n| Nuclide::from_endf(EndfLibrary::EndfBVII1, n, 293.6, 1.0e-3).unwrap())
            .collect();
        let result = run_keff(8.7407, &material, &high, &settings);
        let k_high = result.k_mean;

        // (a) HIGH converges near unity (generous band — small run, residual
        //     approximations), and is stationary.
        assert!(
            k_high > 0.95 && k_high < 1.05,
            "HIGH Godiva k_eff {k_high} not within ~5000 pcm of the benchmark"
        );
        assert!(result.k_std < 0.02, "HIGH k noisy/unconverged: σ = {}", result.k_std);

        // (b) Both levers now live in the LOW tier too, so the embedded/offline run
        //     also lands near unity — from group data plus a single per-group μ̄.
        //     (Before the LOW port it sat at ~1.13 / +12 800 pcm.)
        assert!(
            k_low > 0.95 && k_low < 1.06,
            "LOW tier should also reach the benchmark band now (LOW={k_low})"
        );
    }
}
