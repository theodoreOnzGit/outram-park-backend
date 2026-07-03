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
//! Analog transport (no implicit capture / weight windows), isotropic-CM scatter
//! with target at rest. Inelastic scattering depends on the data tier:
//!
//! - **HIGH tier** ([`Nuclide::from_endf`]) carries the resolved inelastic level
//!   structure (MT=51…91), so inelastic is a distinct channel with a real
//!   energy-loss law — discrete-level two-body kinematics (each level's Q-value)
//!   and a Weisskopf-evaporation continuum. This is the dominant fast-spectrum
//!   down-scatter off heavy nuclei and softens the modelled spectrum.
//! - **LOW tier** ([`Nuclide::from_core`]) has no resolved levels, so inelastic and
//!   (n,xn) stay lumped into elastic scatter (the group total minus the explicit
//!   fission and capture columns), and — above each nuclide's WMP `e_max` — the
//!   fast group data is infinite-dilution Watt-collapsed with no self-shielding.
//!
//! Elastic scatter is isotropic-CM in both tiers (anisotropic elastic from ENDF
//! MF=4 remains future work). Good for a fast bare-sphere Keff, not yet a
//! converged benchmark result.
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
use crate::physics::scatter::{continuum_inelastic_scatter, elastic_scatter, two_body_scatter};
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

/// Transport one neutron history to death (absorption or leakage), banking any
/// fission neutrons. Returns the fission production ν̄ summed over this history's
/// fission events (the generation-k numerator contribution).
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
    let mut r = site.r;
    let mut u = site.u;
    let mut e = site.e;
    let mut production = 0.0;

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
        //   fission | capture | inelastic | elastic.
        // `x.inelastic` (MT=51…91) is a sub-band of scattering carved out with its
        // own energy-loss law; it is non-zero only for the HIGH tier, so the LOW
        // tier collapses to the original fission | capture | elastic split. The
        // final elastic bucket (total − absorption − inelastic) still sweeps up any
        // residual (n,xn) as an elastic-like event.
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
            // two-body kinematics (Q-value) or continuum evaporation. This is the
            // dominant fast-spectrum down-scatter off heavy nuclei.
            let (e2, u2) = match nuc.sample_inelastic(e, seed) {
                Inelastic::Level { q } => two_body_scatter(e, u, nuc.awr, q, seed),
                Inelastic::Continuum => continuum_inelastic_scatter(e, u, nuc.awr, seed),
            };
            e = e2;
            u = u2;
        } else {
            let (e2, u2) = elastic_scatter(e, u, nuc.awr, seed);
            e = e2;
            u = u2;
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
    /// Watt-collapsed fast MGXS above). Reference: ICSBEP k_eff = 1.0000 ± 0.0010.
    /// Pass criterion is deliberately a *broad* plausibility band (0.9–1.4), not a
    /// benchmark gate — this guards the full transport chain (data → geometry →
    /// collision → scatter → fission → power iteration), not accuracy.
    ///
    /// **Results (2026-07).** k_eff ≈ 1.129 ± 0.002, i.e. ~+12 900 pcm high. The
    /// result is stationary and low-noise; the large positive bias is expected for
    /// this fidelity (see [`godiva_endf_high_fidelity_is_not_data_limited`] for the
    /// comparison showing the bias is transport-physics-, not data-, limited).
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

    /// **HIGH-fidelity Godiva V&V, and the LOW-vs-HIGH comparison** — behind the
    /// `net-fetch` feature (downloads ENDF; not part of the default offline suite).
    ///
    /// **Methodology.** The same Godiva model and power-iteration settings are run
    /// under both data tiers:
    /// - **LOW** — embedded WMP + infinite-dilution fast MGXS ([`Nuclide::from_core`]);
    ///   inelastic lumped into elastic (no resolved levels in the group data).
    /// - **HIGH** — ENDF/B-VII.1 downloaded and reconstructed on device
    ///   ([`Nuclide::from_endf`]: RECONR 0.1% tol + BROADR to 293.6 K + MF=1/452
    ///   ν̄), continuous-energy pointwise σ(E) with implicit self-shielding *and*
    ///   an explicit inelastic energy-loss law (MT=51…91 two-body + evaporation).
    ///
    /// Reference: ICSBEP HEU-MET-FAST-001, k_eff = 1.0000 ± 0.0010. The test
    /// asserts (a) HIGH converges to a stationary, plausible eigenvalue, and (b)
    /// both tiers still overpredict and agree to within ~5000 pcm — the two
    /// improvements HIGH carries over LOW (continuous-energy data **and** the
    /// inelastic channel) move k_eff, but the residual bias shared by both keeps
    /// them the same side of unity.
    ///
    /// **Results (2026-07, ENDF/B-VII.1).** At the example settings: LOW
    /// k_eff = 1.12852 ± 0.00174 (+12 852 pcm); HIGH (inelastic modelled)
    /// k_eff = 1.09942 ± 0.00169 (+9 942 pcm). Two effects are separable and both
    /// documented in `docs/development-history.md`: the continuous-energy **data**
    /// upgrade alone was worth only ~400 pcm, whereas adding the inelastic
    /// **transport** channel was worth ~2 510 pcm — the fast-spectrum bias was
    /// transport-limited, and inelastic down-scatter was the largest missing lever.
    /// The LOW tier does not yet model inelastic, so the gap between the tiers here
    /// now reflects transport physics as well as data.
    #[cfg(feature = "net-fetch")]
    #[test]
    fn godiva_endf_high_fidelity_is_not_data_limited() {
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
            n_particles: 1500,
            n_inactive: 20,
            n_active: 40,
            ..KeffSettings::default()
        };

        // LOW tier (embedded, offline).
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

        // (a) HIGH converges to a stationary, plausible eigenvalue.
        assert!(
            k_high > 0.9 && k_high < 1.4,
            "HIGH Godiva k_eff {k_high} outside the plausible band [0.9, 1.4]"
        );
        assert!(result.k_std < 0.02, "HIGH k noisy/unconverged: σ = {}", result.k_std);

        // (b) Both stay well above unity. HIGH carries two improvements over LOW —
        //     continuous-energy data (~400 pcm) and the inelastic energy-loss law
        //     (~2500 pcm) — so it sits below LOW but both remain the same side of
        //     unity, within ~5000 pcm of each other.
        assert!(k_low > 1.05 && k_high > 1.05, "both tiers should overpredict (LOW={k_low}, HIGH={k_high})");
        assert!(
            (k_high - k_low).abs() < 0.05,
            "LOW/HIGH should stay within ~5000 pcm, but |HIGH-LOW| = {:.0} pcm \
             (LOW={k_low}, HIGH={k_high})",
            (k_high - k_low).abs() * 1.0e5
        );
    }
}
