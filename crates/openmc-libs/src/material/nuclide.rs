/// Per-nuclide microscopic cross sections — the seam into the nuclear-data crate.
///
/// C++ analogue: `src/nuclide.cpp`, `include/openmc/nuclide.h`.
///
/// **openmc-libs owns no nuclear data.** Following `docs/architecture.md`, every
/// microscopic cross section comes from `njoy-outram-park-fork`. A [`Nuclide`]
/// here is a thin adapter that holds the njoy data objects for one isotope and,
/// on each `xs_at_energy` call, dispatches across the two representations njoy
/// ships:
///
/// - **Windowed multipole (WMP)** below the nuclide's `e_max` — analytic Doppler
///   broadening over the thermal + resonance range.
/// - **Fast multigroup (MGXS)** above `e_max` — a coarse, Watt-collapsed group set
///   that carries the fast range where a bare-sphere fission spectrum lives.
///
/// This "CE particle, MG data above the ceiling" seam is described in
/// `docs/keff-doppler-roadmap.md`. The transport kernel only ever calls
/// [`Nuclide::xs_at_energy`]; it never touches WMP, ENDF, or HDF5.

use crate::rng::distributions::watt;
use crate::rng::lcg::prn;
use njoy_outram_park_fork::ace::angular::ElasticAngular;
use njoy_outram_park_fork::nuclear_data::secondary::{ChiEout, ChiTabular, FissionSpectrum, NuBar};
use njoy_outram_park_fork::nuclear_data::{Mgxs, MgxsLibrary};
use njoy_outram_park_fork::reconr::ReconrResult;
use njoy_outram_park_fork::wmp::{WindowedMultipole, WmpLibrary};
use njoy_outram_park_fork::MtReaction;
use njoy_outram_park_fork::NjoyError;

/// Microscopic cross sections at a given energy (barn = 1e-24 cm²).
///
/// The transport-side currency. `absorption = capture + fission`; `nu_fission`
/// is ν̄·σ_f (the fission-source production channel). `inelastic` is the total
/// inelastic scattering σ (MT=51…91, discrete levels + continuum) and is a
/// *sub-partition* of the scattering channel, not a separate removal — it is
/// already included in `total`. It is non-zero only for the HIGH (`Pointwise`)
/// tier, which carries the resolved inelastic level structure; the LOW tier
/// reports 0 and lumps inelastic into elastic.
///
/// `n2n` is the (n,2n) cross section (ENDF MT=16), likewise a *sub-partition* of
/// scattering already included in `total`. It is carved out so the transport
/// kernel can give it its true neutron **multiplicity** (yield 2) instead of
/// sweeping it into single-neutron elastic — see [`crate::physics::keff`]. HIGH
/// tier only (from the reconstructed MF=3 background); the LOW tier reports 0.
#[derive(Debug, Clone, Copy, Default)]
pub struct MicroXS {
    /// Total σ_t \[barn\].
    pub total: f64,
    /// Elastic scattering σ_s \[barn\].
    pub elastic: f64,
    /// Fission σ_f \[barn\].
    pub fission: f64,
    /// Absorption σ_a = capture + fission \[barn\].
    pub absorption: f64,
    /// Total inelastic scattering σ (MT=51…91) \[barn\]; HIGH tier only, else 0.
    pub inelastic: f64,
    /// (n,2n) scattering σ (MT=16) \[barn\]; HIGH tier only, else 0. Emits 2
    /// neutrons — the multiplicity the transport kernel restores.
    pub n2n: f64,
    /// Fission production ν̄·σ_f \[barn\].
    pub nu_fission: f64,
}

/// A sampled inelastic scattering channel — the outcome of
/// [`Nuclide::sample_inelastic`], telling the transport kernel which kinematics
/// to apply.
///
/// Kept as an enum (not a trait object) per the workspace design rules: the set
/// of inelastic secondary-energy laws is closed and known at compile time.
#[derive(Debug, Clone, Copy)]
pub enum Inelastic {
    /// A discrete inelastic level (MT=51…90): two-body kinematics with the level's
    /// Q-value `q` \[eV\] (negative — the neutron gives up the excitation energy).
    Level {
        /// Reaction Q-value \[eV\] (< 0), i.e. −(level excitation energy).
        q: f64,
    },
    /// The continuum inelastic channel (MT=91): a broad secondary-energy
    /// distribution modelled by a Weisskopf evaporation spectrum.
    Continuum,
}

/// One inelastic scattering channel in the HIGH-tier level structure — the
/// per-MT record used to sample [`Inelastic`] outcomes by cross section.
///
/// Built once at construction from the reconstructed MF=3 sections (MT=51…91), so
/// the hot path only evaluates σ(E) and picks a channel.
#[derive(Debug, Clone, Copy)]
struct InelasticLevel {
    /// The reaction (MT=51…90 discrete, MT=91/4 continuum) to evaluate σ for.
    mt: MtReaction,
    /// Reaction Q-value QI \[eV\] from MF=3 (< 0 for a discrete level; unused for
    /// the continuum, whose energy is sampled from the evaporation model).
    q: f64,
    /// `true` for the continuum channel (MT=91, or a lone lumped MT=4 fallback).
    continuum: bool,
}

/// The cross-section representation backing a [`Nuclide`] — the LOW/HIGH-fidelity
/// fork.
///
/// Both variants answer the same question ([`Nuclide::xs_at_energy`]) so the
/// transport kernel is agnostic to which one it holds; they differ only in where
/// the numbers come from and how much fidelity they carry. Kept as an enum (not a
/// trait object) per the workspace design rules — the set of data sources is
/// closed and known at compile time.
enum XsSource {
    /// **LOW tier** — the embedded, offline data. Windowed multipole (analytic
    /// Doppler) below `e_max`, coarse Watt-collapsed fast multigroup above it.
    /// No downloads, no HDF5. Infinite-dilution fast data (no self-shielding).
    Core {
        /// Upper energy of the WMP representation \[eV\]; the CE↔MG seam.
        e_max: f64,
        /// Windowed-multipole cross sections (valid on `[e_min, e_max]`).
        wmp: WindowedMultipole,
        /// Fast-range group constants above `e_max` (`None` ⇒ WMP covers all).
        fast: Option<Mgxs>,
    },
    /// **HIGH tier** — resonance-reconstructed, Doppler-broadened *pointwise*
    /// σ(E) spanning the whole energy range, obtained by downloading the raw ENDF
    /// tape and running RECONR + BROADR on device (see [`Nuclide::from_endf`]).
    /// Continuous-energy throughout, so resonance self-shielding is captured
    /// implicitly by sampling σ at the neutron's actual energy — the reference
    /// the LOW tier is judged against. Already broadened to the target
    /// temperature at construction time, hence temperature-independent at lookup.
    // Constructed only via `from_endf` (the `net-fetch` feature); the lookup arm
    // still matches it in every build, so this is dead only without the feature.
    #[cfg_attr(not(feature = "net-fetch"), allow(dead_code))]
    Pointwise {
        /// Full-range reconstructed + broadened MF=3 sections (`eval_mt`).
        recon: ReconrResult,
        /// The inelastic scattering channels (MT=51…91) extracted from `recon`,
        /// precomputed once so [`Nuclide::sample_inelastic`] and the inelastic σ
        /// sum stay cheap. Empty ⇒ no resolved inelastic (falls back to elastic).
        inel: Vec<InelasticLevel>,
        /// Elastic angular distribution (ENDF MF=4/MT=2), CM-frame tabulated
        /// cosine/pdf/cdf per incident energy. Empty / all-isotropic ⇒ the elastic
        /// scatter falls back to isotropic-CM. Drives [`Nuclide::sample_elastic_mu_cm`].
        elastic_angular: ElasticAngular,
    },
}

/// One isotope's cross-section data, pulled from `njoy-outram-park-fork`.
///
/// Two constructors give the two fidelity tiers, both feeding the *same*
/// [`Nuclide::xs_at_energy`] seam so the transport kernel never knows the
/// difference:
///
/// - [`Nuclide::from_core`] — **LOW**: embedded WMP + fast MGXS, offline.
/// - [`Nuclide::from_endf`] — **HIGH**: download raw ENDF, RECONR + BROADR to
///   pointwise σ(E) (behind the `net-fetch` feature).
pub struct Nuclide {
    /// Isotope name, e.g. `"U235"`.
    pub name: String,
    /// Atomic weight ratio (target mass / neutron mass) — sets scatter kinematics.
    pub awr: f64,
    /// Average neutron yield ν̄(E).
    nu: NuBar,
    /// Fission-neutron birth spectrum χ. HIGH tier: the energy-dependent ENDF
    /// MF=5/MT=18 χ(E→E') when the tape carries an LF=1 law; otherwise (and for the
    /// LOW tier) the fixed thermal-Watt stand-in. Drives [`Nuclide::sample_fission_energy`].
    chi: FissionSpectrum,
    /// The cross-section representation (LOW `Core` vs HIGH `Pointwise`).
    xs: XsSource,
}

impl Nuclide {
    /// **LOW fidelity.** Resolve a nuclide from the embedded CORE nuclear-data
    /// libraries.
    ///
    /// Pulls the WMP evaluator from [`WmpLibrary::core`] and the fast MGXS set
    /// from [`MgxsLibrary::core`] (absent for nuclides whose WMP already reaches
    /// 20 MeV). ν̄ is a compact per-nuclide constant table ([`nubar_for`]) — a
    /// stopgap until ACER 4b lands (see `docs/keff-doppler-roadmap.md`).
    ///
    /// # Errors
    /// [`NjoyError`] if `name` is not in the CORE WMP container.
    pub fn from_core(name: &str) -> Result<Self, NjoyError> {
        let wmp = WmpLibrary::core().get(name)?;
        let awr = wmp.awr;
        let e_max = wmp.e_max;
        let fast = MgxsLibrary::core().get(name).cloned();
        let nu = nubar_for(name, wmp.fissionable);
        // LOW tier carries no embedded MF=5; birth from the thermal-Watt stand-in.
        // TODO: bake a per-nuclide MF=5 χ (or a compact energy-dependent form) into
        // the CORE data so the LOW tier gets the same fission-birth fidelity as HIGH
        // (worth ~+500 pcm on Godiva; see docs/development-history.md 2026-07 MF=5).
        let chi = FissionSpectrum::default();
        Ok(Self { name: name.to_string(), awr, nu, chi, xs: XsSource::Core { e_max, wmp, fast } })
    }

    /// **HIGH fidelity.** Build a nuclide from a raw ENDF tape downloaded from a
    /// pinned upstream, reconstructed and Doppler-broadened on device.
    ///
    /// The full HIGH-tier path ([`docs/data-acquisition.md`]):
    /// 1. **Download** the ENDF neutron tape for `name` from `library`'s hardcoded
    ///    URL (IAEA NDS), cached after the first call. The nuclide's ENDF MAT is
    ///    looked up from the built-in CORE table.
    /// 2. **RECONR** — reconstruct 0 K pointwise σ(E) from the resonance
    ///    parameters to fractional `tolerance` (NJOY default `1.0e-3`).
    /// 3. **BROADR** — Doppler-broaden every reaction to `temp_k` (free-gas
    ///    SIGMA1 kernel), so the stored data already sits at the material
    ///    temperature.
    /// 4. **ν̄(E)** — parse the real energy-dependent ν̄ from MF=1/MT=452 on the
    ///    same tape (not the constant stopgap), matching the fast spectrum where
    ///    ν̄ rises with energy.
    ///
    /// Continuous-energy throughout: unlike the LOW tier there is no `e_max` seam
    /// and no infinite-dilution group averaging, so resonance self-shielding falls
    /// out of sampling σ at each neutron's actual energy.
    ///
    /// Needs the network and the **`net-fetch`** feature.
    ///
    /// # Errors
    /// [`NjoyError`] if `name` is not in the built-in MAT table, the download or
    /// unzip fails, or the evaluation uses a resonance format RECONR does not yet
    /// reconstruct (e.g. ENDF/B-VIII.0 U's LRF=7 — use ENDF/B-VII.1 for U/Pu).
    #[cfg(feature = "net-fetch")]
    pub fn from_endf(
        library: njoy_outram_park_fork::acquire::EndfLibrary,
        name: &str,
        temp_k: f64,
        tolerance: f64,
    ) -> Result<Self, NjoyError> {
        use njoy_outram_park_fork::acquire::{parse_nuclide, well_known_mat, EndfCache};
        use njoy_outram_park_fork::broadr::doppler_broaden;
        use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};

        let (z, a, sym) = parse_nuclide(name)?;
        let mat = well_known_mat(z, a).ok_or_else(|| {
            NjoyError::Download(format!(
                "MAT for {name} not in the built-in CORE table; only the shipped \
                 nuclides can be fetched by name"
            ))
        })?;

        // 1. Download (cached) + parse the raw ENDF tape.
        let cache = EndfCache::new()?;
        let tape = cache.download_tape(library, mat, z, a, sym)?;

        // 2. RECONR at 0 K.
        let recon0 = reconr(&tape, &ReconrConfig { mat, tolerance, temperature: 0.0 })?;
        let awr = recon0.material.awr;

        // 3. BROADR to the material temperature (in place of the 0 K grid).
        let sections = doppler_broaden(&recon0.sections, awr, temp_k);
        let recon = ReconrResult { material: recon0.material, sections };

        // 4. Real energy-dependent ν̄ from MF=1/452 (falls back to ν̄≡0 for a
        //    non-fissionable nuclide, which has no MF=1/452 section).
        let nu = NuBar::from_endf(&tape, mat)?.unwrap_or_else(|| nubar_for(name, false));

        // 4b. Energy-dependent fission spectrum χ(E→E') from MF=5/MT=18 (LF=1). A
        //     non-fissionable nuclide (no MF=5) or an unsupported LF falls back to
        //     the thermal-Watt stand-in — harmless, since such nuclides never fission.
        let chi = FissionSpectrum::from_endf_mf5(&tape, mat)?.unwrap_or_default();

        // 5. Extract the inelastic level structure (MT=51…91) once, for
        //    energy-loss scattering in transport.
        let inel = build_inelastic_levels(&recon);

        // 6. Elastic angular distribution from MF=4/MT=2 (CM frame). Absent /
        //    unparseable ⇒ isotropic-CM elastic (default empty distribution).
        let elastic_angular = tape
            .section(mat, 4, 2)
            .map(njoy_outram_park_fork::ace::angular::parse_elastic_angular)
            .transpose()?
            .unwrap_or_default();

        Ok(Self {
            name: name.to_string(),
            awr,
            nu,
            chi,
            xs: XsSource::Pointwise { recon, inel, elastic_angular },
        })
    }

    /// Microscopic cross sections at incident energy `e` \[eV\] and temperature
    /// `temp_k` \[K\].
    ///
    /// - **LOW (`Core`):** below `e_max` the analytic-Doppler WMP form is used
    ///   (temperature-dependent); above `e_max` the fast MGXS constant-per-group
    ///   lookup takes over (temperature-independent — the fast range is smooth).
    /// - **HIGH (`Pointwise`):** a direct pointwise lookup on the reconstructed
    ///   σ(E). The data was already Doppler-broadened to its target temperature at
    ///   construction, so `temp_k` is ignored here.
    ///
    /// In both tiers the transport kernel partitions on `total`/`fission`/
    /// `absorption`; `elastic` is reported for completeness but the kernel treats
    /// `total − absorption` as the scattering channel (lumping inelastic and
    /// (n,xn) into elastic-like events — see the keff module fidelity note).
    pub fn xs_at_energy(&self, e: f64, temp_k: f64) -> MicroXS {
        match &self.xs {
            XsSource::Core { e_max, wmp, fast } => {
                if e <= *e_max {
                    let x = wmp.evaluate(e, temp_k);
                    MicroXS {
                        total: x.total(),
                        elastic: x.scatter,
                        fission: x.fission,
                        absorption: x.absorption,
                        inelastic: 0.0, // LOW tier lumps inelastic into elastic
                        n2n: 0.0,       // LOW tier lumps (n,2n) into elastic (no group column yet)
                        nu_fission: x.fission * self.nu.at(e),
                    }
                } else if let Some(mg) = fast {
                    let m = mg.micro(e);
                    // The group `total` already contains inelastic, so carve it out
                    // as the non-elastic, non-absorption remainder. This lets the
                    // LOW tier down-scatter inelastically (evaporation law) instead
                    // of lumping that energy loss into elastic. Clamp at 0 to absorb
                    // group-collapse rounding where the channels slightly over-sum.
                    let inelastic = (m.total - m.elastic - m.fission - m.capture).max(0.0);
                    MicroXS {
                        total: m.total,
                        elastic: m.elastic,
                        fission: m.fission,
                        absorption: m.capture + m.fission,
                        inelastic,
                        n2n: 0.0, // LOW tier: (n,2n) still lumped in the group total
                        nu_fission: m.nu_fission,
                    }
                } else {
                    // No fast set ⇒ the WMP spans the whole sublibrary; clamp.
                    let x = wmp.evaluate(*e_max, temp_k);
                    MicroXS {
                        total: x.total(),
                        elastic: x.scatter,
                        fission: x.fission,
                        absorption: x.absorption,
                        inelastic: 0.0,
                        n2n: 0.0,
                        nu_fission: x.fission * self.nu.at(e),
                    }
                }
            }
            XsSource::Pointwise { recon, inel, .. } => {
                let total = recon.eval_mt(MtReaction::Mt1Total, e);
                let elastic = recon.eval_mt(MtReaction::Mt2Elastic, e);
                let fission = recon.eval_mt(MtReaction::Mt18Fission, e);
                let capture = recon.eval_mt(MtReaction::Mt102Capture, e);
                let inelastic: f64 = inel.iter().map(|l| recon.eval_mt(l.mt, e)).sum();
                // (n,2n) from the reconstructed MF=3 background (threshold reaction,
                // no resonance contribution). Carried separately so the transport
                // kernel can give it its yield-2 multiplicity. 0.0 below threshold
                // or if the evaluation has no MT=16 section.
                let n2n = recon.eval_mt(MtReaction::Mt16N2n, e);
                MicroXS {
                    total,
                    elastic,
                    fission,
                    absorption: fission + capture,
                    inelastic,
                    n2n,
                    nu_fission: fission * self.nu.at(e),
                }
            }
        }
    }

    /// Sample which inelastic scattering channel a collision at energy `e` \[eV\]
    /// takes, proportional to each channel's cross section at `e`.
    ///
    /// Returns an [`Inelastic::Level`] carrying the level's Q-value for a discrete
    /// level (MT=51…90), or [`Inelastic::Continuum`] for the MT=91 continuum.
    ///
    /// The HIGH (`Pointwise`) tier resolves individual levels from the
    /// reconstructed MT sections. The LOW (`Core`/group) tier carries no per-level
    /// data — its [`MicroXS::inelastic`] is the group remainder `total − elastic −
    /// fission − capture` — so this returns `Continuum` for it, and the transport
    /// kernel applies the Weisskopf evaporation energy-loss law. An empty-level
    /// call likewise defaults to `Continuum`.
    pub fn sample_inelastic(&self, e: f64, seed: &mut u64) -> Inelastic {
        let XsSource::Pointwise { recon, inel, .. } = &self.xs else {
            return Inelastic::Continuum;
        };
        let total: f64 = inel.iter().map(|l| recon.eval_mt(l.mt, e)).sum();
        if !(total > 0.0) {
            return Inelastic::Continuum;
        }
        let xi = prn(seed) * total;
        let mut cum = 0.0;
        for l in inel {
            cum += recon.eval_mt(l.mt, e);
            if xi < cum {
                return if l.continuum {
                    Inelastic::Continuum
                } else {
                    Inelastic::Level { q: l.q }
                };
            }
        }
        Inelastic::Continuum
    }

    /// Sample an elastic scattering cosine in the **centre-of-mass frame** at
    /// incident energy `e` \[eV\], returning `Some(mu_cm)` for anisotropic elastic
    /// or `None` when the distribution is isotropic (the caller then falls back to
    /// isotropic-CM elastic). Both fidelity tiers scatter forward:
    ///
    /// - **HIGH (`Pointwise`):** the full ENDF MF=4 tabulated distribution. Ported
    ///   from OpenMC `AngleDistribution::sample` (`src/distribution_angle.cpp`):
    ///   locate the incident-energy bin with interpolation factor `r`, pick the
    ///   lower or upper tabulated distribution by `r > ξ` (statistical
    ///   interpolation), then invert its cosine CDF — [`sample_tabular_mu`], itself
    ///   a port of `Tabular::sample_unbiased`.
    /// - **LOW (`Core`/group):** only the per-group **mean cosine** μ̄ is stored
    ///   (baked from MF=4 into the fast MGXS). Above `e_max` the group μ̄ drives a
    ///   maximum-entropy exponential angular law [`sample_exponential_mu`], which
    ///   reproduces μ̄ exactly and stays valid for the strongly forward-peaked
    ///   high-energy groups (where μ̄ ≫ 1/3 breaks the usual P1 law). Below `e_max`
    ///   (WMP resonance range) elastic is treated as isotropic (`None`).
    ///
    /// The cosines are in the CM frame (ENDF LCT=2), matching
    /// [`crate::physics::scatter::two_body_scatter_with_mu`].
    pub fn sample_elastic_mu_cm(&self, e: f64, seed: &mut u64) -> Option<f64> {
        let elastic_angular = match &self.xs {
            XsSource::Pointwise { elastic_angular, .. } => elastic_angular,
            // LOW tier: forward-scatter off the group mean cosine above `e_max`.
            XsSource::Core { e_max, fast, .. } => {
                if e <= *e_max {
                    return None; // resonance range: treat as isotropic-CM
                }
                let mubar = fast.as_ref()?.micro(e).mubar;
                if mubar.abs() < 1.0e-4 {
                    return None; // effectively isotropic
                }
                return Some(sample_exponential_mu(mubar, prn(seed)));
            }
        };
        let dists = &elastic_angular.energies;
        if dists.is_empty() {
            return None;
        }

        // Locate the bracketing incident-energy distributions (energies ascending
        // in MeV) and the interpolation factor, then pick one statistically.
        let e_mev = e * 1.0e-6;
        let n = dists.len();
        let chosen = if e_mev <= dists[0].e_mev {
            &dists[0]
        } else if e_mev >= dists[n - 1].e_mev {
            &dists[n - 1]
        } else {
            let mut i = 0;
            while i + 1 < n && dists[i + 1].e_mev <= e_mev {
                i += 1;
            }
            let (e0, e1) = (dists[i].e_mev, dists[i + 1].e_mev);
            let r = if e1 > e0 { (e_mev - e0) / (e1 - e0) } else { 0.0 };
            if r > prn(seed) {
                &dists[i + 1]
            } else {
                &dists[i]
            }
        };

        // Isotropic at this energy (ENDF locator 0 ⇒ no stored cosines).
        if chosen.cosines.is_empty() {
            return Some(2.0 * prn(seed) - 1.0);
        }
        Some(sample_tabular_mu(&chosen.cosines, &chosen.pdf, &chosen.cdf, prn(seed)))
    }

    /// Sample a fission-neutron birth energy \[eV\] given the incident energy
    /// `e_in` \[eV\] of the neutron that induced the fission.
    ///
    /// - **HIGH tier** with an ENDF MF=5 χ ([`FissionSpectrum::ContinuousTabular`]):
    ///   the real energy-dependent prompt fission spectrum, sampled by
    ///   [`sample_continuous_tabular`] (a port of OpenMC
    ///   `ContinuousTabular::sample`, `src/distribution_energy.cpp`). The outgoing
    ///   spectrum hardens with `e_in`, which the fixed Watt form cannot capture.
    /// - **Otherwise** (LOW tier, or a nuclide whose tape lacked an LF=1 MF=5): the
    ///   thermal-Watt stand-in `χ(E') ∝ e^{−E'/a} sinh √(bE')`, independent of `e_in`.
    ///
    /// This is the fission-source birth spectrum the k-eigenvalue driver banks with.
    pub fn sample_fission_energy(&self, e_in: f64, seed: &mut u64) -> f64 {
        match &self.chi {
            FissionSpectrum::ContinuousTabular(chi) => {
                sample_continuous_tabular(chi, e_in, seed)
            }
            FissionSpectrum::Watt { a, b } => watt(seed, *a, *b),
            FissionSpectrum::Tabulated { e_out, pdf } => {
                sample_tabulated_energy(e_out, pdf, prn(seed))
            }
        }
    }
}

/// Sample an outgoing fission-neutron energy \[eV\] from an energy-dependent ENDF
/// MF=5 χ(E→E') at incident energy `e_in` \[eV\] — a direct port of OpenMC
/// `ContinuousTabular::sample` (`src/distribution_energy.cpp`), specialized to the
/// no-discrete-lines case (`n_discrete = 0`) that a fission spectrum always is, and
/// to a lin-lin incident-energy grid (ENDF INT=2, as the ENDF/B-VII.1 U fission
/// TAB2 uses):
///
/// 1. locate the incident-energy bin `i` and interpolation factor `r`;
/// 2. statistically pick the lower/upper table `l` (`r > ξ ? i+1 : i`);
/// 3. invert the chosen table's outgoing-energy CDF ([`sample_ct_table`]); then
/// 4. scale the sampled E' between the `i` and `i+1` tables' \[E₁, E_K\] envelopes so
///    the outgoing energy tracks the incident-energy interpolation.
fn sample_continuous_tabular(chi: &ChiTabular, e_in: f64, seed: &mut u64) -> f64 {
    let energy = &chi.incident;
    let n = energy.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return sample_ct_table(&chi.tables[0], prn(seed));
    }

    // (1) incident-energy bin + interpolation factor (clamp outside the grid).
    let (i, r) = if e_in < energy[0] {
        (0usize, 0.0)
    } else if e_in > energy[n - 1] {
        (n - 2, 1.0)
    } else {
        let mut i = 0;
        while i + 1 < n && energy[i + 1] <= e_in {
            i += 1;
        }
        let i = i.min(n - 2);
        (i, (e_in - energy[i]) / (energy[i + 1] - energy[i]))
    };

    // (2) statistically pick the lower or upper table.
    let l = if r > prn(seed) { i + 1 } else { i };

    // (3) invert table l's outgoing-energy CDF.
    let e_out = sample_ct_table(&chi.tables[l], prn(seed));

    // (4) interpolate the outgoing energy between the i and i+1 table envelopes.
    let ti = &chi.tables[i];
    let ti1 = &chi.tables[i + 1];
    let (e_i_1, e_i_k) = (ti.e_out[0], ti.e_out[ti.e_out.len() - 1]);
    let (e_i1_1, e_i1_k) = (ti1.e_out[0], ti1.e_out[ti1.e_out.len() - 1]);
    let e_1 = e_i_1 + r * (e_i1_1 - e_i_1);
    let e_k = e_i_k + r * (e_i1_k - e_i_k);
    if l == i {
        if e_i_k > e_i_1 {
            e_1 + (e_out - e_i_1) * (e_k - e_1) / (e_i_k - e_i_1)
        } else {
            e_out
        }
    } else if e_i1_k > e_i1_1 {
        e_1 + (e_out - e_i1_1) * (e_k - e_1) / (e_i1_k - e_i1_1)
    } else {
        e_out
    }
}

/// Invert one outgoing-energy table's CDF at a uniform draw `r1 ∈ [0, 1)`,
/// returning the sampled E' \[eV\]. The inner CDF search + interpolation of OpenMC
/// `ContinuousTabular::sample`: walk the CDF to the bin `k` with `c[k] ≤ r1 <
/// c[k+1]`, then invert the density over that bin — the quadratic lin-lin inverse
/// `E' = E_k + (√(p_k² + 2 f (r1 − c_k)) − p_k)/f` (with `f` the density slope), or
/// the linear histogram inverse `E' = E_k + (r1 − c_k)/p_k`.
fn sample_ct_table(t: &ChiEout, r1: f64) -> f64 {
    let n = t.e_out.len();
    if n == 1 {
        return t.e_out[0];
    }
    // Continuous-portion CDF search (n_discrete = 0), mirroring the C++ loop:
    // leaves k as the lower edge with c[k] ≤ r1 < c[k+1] (k clamped to n−2).
    let mut c_k = t.cdf[0];
    let mut k = 0usize;
    let end = n.saturating_sub(2); // n_energy_out − 2
    for j in 0..end {
        k = j;
        let c_k1 = t.cdf[k + 1];
        if r1 < c_k1 {
            break;
        }
        k = j + 1;
        c_k = c_k1;
    }

    let e_l_k = t.e_out[k];
    let p_l_k = t.pdf[k];
    if t.linlin {
        let e_l_k1 = t.e_out[k + 1];
        let p_l_k1 = t.pdf[k + 1];
        if e_l_k == e_l_k1 {
            return e_l_k;
        }
        let frac = (p_l_k1 - p_l_k) / (e_l_k1 - e_l_k);
        if frac == 0.0 {
            if p_l_k > 0.0 {
                e_l_k + (r1 - c_k) / p_l_k
            } else {
                e_l_k
            }
        } else {
            e_l_k + ((p_l_k * p_l_k + 2.0 * frac * (r1 - c_k)).max(0.0).sqrt() - p_l_k) / frac
        }
    } else {
        // Histogram interpolation.
        if p_l_k > 0.0 {
            e_l_k + (r1 - c_k) / p_l_k
        } else {
            e_l_k
        }
    }
}

/// Sample an energy \[eV\] from a static (energy-independent) tabulated χ pdf by
/// CDF inversion — the [`FissionSpectrum::Tabulated`] arm. Builds the lin-lin CDF
/// on the fly (this variant carries no precomputed CDF) and inverts it with the
/// same quadratic form as [`sample_ct_table`].
fn sample_tabulated_energy(e_out: &[f64], pdf: &[f64], r1: f64) -> f64 {
    let n = e_out.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return e_out[0];
    }
    // Cumulative trapezoidal integral of the pdf, normalized to 1.
    let mut cdf = vec![0.0; n];
    for k in 1..n {
        cdf[k] = cdf[k - 1] + 0.5 * (pdf[k] + pdf[k - 1]) * (e_out[k] - e_out[k - 1]);
    }
    let total = cdf[n - 1];
    if !(total > 0.0) {
        return e_out[0];
    }
    let mut k = 0usize;
    while k + 1 < n - 1 && cdf[k + 1] / total <= r1 {
        k += 1;
    }
    let c_k = cdf[k] / total;
    let p_k = pdf[k] / total;
    let p_k1 = pdf[k + 1] / total;
    let (x_k, x_k1) = (e_out[k], e_out[k + 1]);
    if x_k == x_k1 {
        return x_k;
    }
    let frac = (p_k1 - p_k) / (x_k1 - x_k);
    if frac == 0.0 {
        if p_k > 0.0 {
            x_k + (r1 - c_k) / p_k
        } else {
            x_k
        }
    } else {
        x_k + ((p_k * p_k + 2.0 * frac * (r1 - c_k)).max(0.0).sqrt() - p_k) / frac
    }
}

/// Invert a tabulated cosine CDF to sample a value, assuming lin-lin interpolation
/// between grid points — a port of OpenMC `Tabular::sample_unbiased`
/// (`src/distribution.cpp`, lin-lin branch).
///
/// `cosines`/`pdf`/`cdf` are the ascending cosine grid, its (non-negative,
/// unit-integral) density, and cumulative distribution (`cdf[0]=0`, `cdf[last]=1`);
/// `xi ∈ [0, 1)` is the uniform draw. Within the selected bin the density is linear,
/// so the CDF is quadratic and the inverse is the standard
/// `x_i + (√(p_i² + 2m(ξ − c_i)) − p_i) / m` (falling back to `x_i + (ξ−c_i)/p_i`
/// when the slope `m` vanishes).
fn sample_tabular_mu(cosines: &[f64], pdf: &[f64], cdf: &[f64], xi: f64) -> f64 {
    let n = cosines.len();
    if n == 1 {
        return cosines[0].clamp(-1.0, 1.0);
    }
    // First CDF bin whose upper edge is above the draw (clamped so `k+1` is valid).
    let mut k = 0;
    while k + 1 < n && cdf[k + 1] <= xi {
        k += 1;
    }
    let k = k.min(n - 2);

    let (x_i, x_i1) = (cosines[k], cosines[k + 1]);
    let (p_i, p_i1) = (pdf[k], pdf[k + 1]);
    let c_i = cdf[k];
    let dx = x_i1 - x_i;
    if dx <= 0.0 {
        return x_i.clamp(-1.0, 1.0);
    }
    let m = (p_i1 - p_i) / dx;
    let mu = if m == 0.0 {
        if p_i > 0.0 {
            x_i + (xi - c_i) / p_i
        } else {
            x_i
        }
    } else {
        x_i + ((p_i * p_i + 2.0 * m * (xi - c_i)).max(0.0).sqrt() - p_i) / m
    };
    mu.clamp(-1.0, 1.0)
}

/// Sample an elastic scattering cosine (CM frame) from the **maximum-entropy
/// exponential angular law** with a prescribed mean cosine `mubar` — the LOW-tier
/// forward-scatter model.
///
/// When only the mean cosine μ̄ of a group is known, the least-committal angular
/// density on `[−1, 1]` with that mean is `p(μ) ∝ exp(λμ)` (maximum entropy). Its
/// mean is the Langevin function `L(λ) = coth λ − 1/λ`, so `λ = L⁻¹(μ̄)`
/// ([`langevin_inverse`]). Unlike the linearly-anisotropic (P1) law `½(1 + 3μ̄μ)`,
/// which goes negative once `|μ̄| > 1/3`, this stays a valid density for every
/// `μ̄ ∈ (−1, 1)` — essential here because the fast groups off heavy nuclei are
/// strongly forward-peaked (μ̄ up to ~0.9).
///
/// Sampling is by CDF inversion. With `F(μ) = (e^{λ(μ+1)} − 1)/(e^{2λ} − 1)`,
/// solving `F(μ) = ξ` gives the numerically stable form
/// `μ = 1 + ln(ξ + (1 − ξ) e^{−2λ}) / λ`. `ξ ∈ [0, 1)` is the uniform draw. A
/// near-zero `μ̄` degenerates to isotropic (`μ = 2ξ − 1`).
fn sample_exponential_mu(mubar: f64, xi: f64) -> f64 {
    let mu_bar = mubar.clamp(-0.99, 0.99);
    if mu_bar.abs() < 1.0e-4 {
        return (2.0 * xi - 1.0).clamp(-1.0, 1.0);
    }
    let lambda = langevin_inverse(mu_bar);
    if lambda.abs() < 1.0e-6 {
        return (2.0 * xi - 1.0).clamp(-1.0, 1.0);
    }
    let mu = 1.0 + (xi + (1.0 - xi) * (-2.0 * lambda).exp()).ln() / lambda;
    mu.clamp(-1.0, 1.0)
}

/// Invert the Langevin function `L(λ) = coth λ − 1/λ = μ̄` for `λ`, by Newton's
/// method on `f(λ) = coth λ − 1/λ − μ̄`.
///
/// `L` is odd, monotonic, and maps `λ ∈ (−∞, ∞)` onto `μ̄ ∈ (−1, 1)`. The initial
/// guess uses the small-μ̄ slope `L(λ) ≈ λ/3` (so `λ₀ ≈ 3μ̄`) blended with the
/// `μ̄ → ±1` pole `λ ≈ 1/(1 − |μ̄|)`; a few Newton steps then converge. Used only
/// at bake-consuming time (once per elastic collision in the LOW tier), so a
/// handful of iterations is negligible.
fn langevin_inverse(mu_bar: f64) -> f64 {
    let x = mu_bar.abs();
    // Seed between the small-angle slope and the near-unity pole.
    let mut lambda = if x < 0.3 {
        3.0 * x
    } else {
        (1.0 / (1.0 - x)).min(50.0)
    };
    for _ in 0..30 {
        // coth λ − 1/λ − x, with a stable coth via 1/tanh.
        let coth = 1.0 / lambda.tanh();
        let f = coth - 1.0 / lambda - x;
        // L'(λ) = 1/λ² − csch²λ = 1/λ² − (coth²λ − 1).
        let d = 1.0 / (lambda * lambda) - (coth * coth - 1.0);
        if d.abs() < 1.0e-12 {
            break;
        }
        let step = f / d;
        lambda -= step;
        if lambda <= 0.0 {
            lambda = 1.0e-3; // keep the root finder on the positive branch
        }
        if step.abs() < 1.0e-10 {
            break;
        }
    }
    if mu_bar < 0.0 {
        -lambda
    } else {
        lambda
    }
}

/// Extract the inelastic scattering channels (MT=51…91) from a reconstructed
/// evaluation, for energy-loss scattering in transport.
///
/// MT=51…90 are discrete levels (each carrying its QI Q-value for two-body
/// kinematics); MT=91 is the continuum. If only the *lumped* total inelastic
/// (MT=4) is present with no resolved levels, it is used as a single continuum
/// channel so the down-scatter is still modelled.
#[cfg(feature = "net-fetch")]
fn build_inelastic_levels(recon: &ReconrResult) -> Vec<InelasticLevel> {
    let mut levels: Vec<InelasticLevel> = recon
        .sections
        .iter()
        .filter_map(|s| {
            let n = s.mt.number();
            if (51..=90).contains(&n) {
                Some(InelasticLevel { mt: s.mt, q: s.qi, continuum: false })
            } else if n == 91 {
                Some(InelasticLevel { mt: s.mt, q: s.qi, continuum: true })
            } else {
                None
            }
        })
        .collect();

    // Fallback: only the lumped MT=4 total inelastic is present (no resolved
    // levels) — treat it as one continuum channel rather than dropping it.
    if levels.is_empty() {
        if let Some(s) = recon.sections.iter().find(|s| s.mt.number() == 4) {
            levels.push(InelasticLevel { mt: s.mt, q: s.qi, continuum: true });
        }
    }
    levels
}

/// A constant-in-energy ν̄ table for one nuclide — the stopgap secondary-data
/// source until ACER 4b (ENDF MF=1/452) is wired through njoy.
///
/// The fast MGXS already folds an energy-dependent ν̄ into its `nu_fission`
/// column, so this table only governs the sub-`e_max` resonance range, where a
/// bare fast sphere fissions rarely; a single representative value is adequate.
/// Non-fissionable nuclides get ν̄ ≡ 0.
///
/// Values are total (prompt + delayed) ν̄ near threshold, from ENDF/B evaluations.
fn nubar_for(name: &str, fissionable: bool) -> NuBar {
    if !fissionable {
        return NuBar { energy: vec![1.0e-3, 2.0e7], nu_total: vec![0.0, 0.0] };
    }
    let nu = match name {
        "U233" => 2.49,
        "U235" => 2.44,
        "U238" => 2.49, // fast-threshold fission; negligible below e_max
        "Pu239" => 2.88,
        "Pu241" => 2.94,
        "Th232" => 2.45,
        _ => 2.50,
    };
    NuBar { energy: vec![1.0e-3, 2.0e7], nu_total: vec![nu, nu] }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exponential angular sampler must reproduce its target mean cosine μ̄ —
    /// the whole point of a maximum-entropy model with a prescribed first moment.
    /// Averaging many CDF-inverted draws over uniform ξ recovers μ̄ for values that
    /// span the P1-valid range and well beyond it (μ̄ = 0.6, 0.85), where a linear
    /// law would be invalid.
    #[test]
    fn exponential_sampler_reproduces_mean_cosine() {
        let n = 200_000usize;
        for &target in &[0.05_f64, 0.2, 0.333, 0.6, 0.85] {
            let mut sum = 0.0;
            for i in 0..n {
                // Deterministic low-discrepancy sweep of ξ ∈ (0,1) for a stable mean.
                let xi = (i as f64 + 0.5) / n as f64;
                let mu = sample_exponential_mu(target, xi);
                assert!((-1.0..=1.0).contains(&mu), "μ in range");
                sum += mu;
            }
            let mean = sum / n as f64;
            assert!(
                (mean - target).abs() < 5.0e-3,
                "μ̄ target {target}, sampled {mean}"
            );
        }
    }

    /// A negative mean cosine (backscatter) is handled symmetrically, and a
    /// near-zero μ̄ degenerates to isotropic (mean ≈ 0).
    #[test]
    fn exponential_sampler_handles_backward_and_isotropic() {
        let n = 100_000usize;
        let mean = |target: f64| -> f64 {
            (0..n)
                .map(|i| sample_exponential_mu(target, (i as f64 + 0.5) / n as f64))
                .sum::<f64>()
                / n as f64
        };
        assert!((mean(-0.4) + 0.4).abs() < 5.0e-3, "backward μ̄ reproduced");
        assert!(mean(0.0).abs() < 5.0e-3, "isotropic mean ≈ 0");
    }

    /// The MF=5 outgoing-energy CDF inversion ([`sample_ct_table`]) reproduces the
    /// distribution it was built from. A uniform outgoing spectrum on [0, 2 MeV]
    /// (histogram pdf, CDF = [0, ½, 1]) must sample uniformly, so a deterministic
    /// low-discrepancy sweep of ξ recovers the analytic mean of 1 MeV and every
    /// draw lands inside the tabulated support.
    #[test]
    fn ct_table_uniform_reproduces_mean() {
        let t = ChiEout {
            e_out: vec![0.0, 1.0e6, 2.0e6],
            pdf: vec![5.0e-7, 5.0e-7, 5.0e-7], // ∫ = 1 over [0, 2 MeV]
            cdf: vec![0.0, 0.5, 1.0],
            linlin: false,
        };
        let n = 100_000usize;
        let mut sum = 0.0;
        for i in 0..n {
            let r1 = (i as f64 + 0.5) / n as f64;
            let e = sample_ct_table(&t, r1);
            assert!((0.0..=2.0e6).contains(&e), "E' {e} outside tabulated support");
            sum += e;
        }
        let mean = sum / n as f64;
        assert!((mean - 1.0e6).abs() < 1.0e4, "uniform mean {mean} ≈ 1 MeV");
    }

    /// The energy-dependent χ hardens with incident energy — the whole point of the
    /// MF=5 law over a fixed Watt spectrum. With a low-energy table on [0, 2 MeV]
    /// and a high-energy table on [0, 4 MeV], the incident-energy envelope
    /// interpolation ([`sample_continuous_tabular`]) must lift the mean outgoing
    /// energy as the incident energy sweeps from the bottom to the top of the grid.
    #[test]
    fn continuous_tabular_hardens_with_incident_energy() {
        let low = ChiEout {
            e_out: vec![0.0, 1.0e6, 2.0e6],
            pdf: vec![5.0e-7, 5.0e-7, 5.0e-7],
            cdf: vec![0.0, 0.5, 1.0],
            linlin: false,
        };
        let high = ChiEout {
            e_out: vec![0.0, 2.0e6, 4.0e6],
            pdf: vec![2.5e-7, 2.5e-7, 2.5e-7],
            cdf: vec![0.0, 0.5, 1.0],
            linlin: false,
        };
        let chi = ChiTabular { incident: vec![1.0e5, 2.0e7], tables: vec![low, high] };
        let mean_at = |e_in: f64| -> f64 {
            let mut seed = 12345u64;
            let n = 200_000usize;
            (0..n).map(|_| sample_continuous_tabular(&chi, e_in, &mut seed)).sum::<f64>()
                / n as f64
        };
        let m_lo = mean_at(1.0e5);
        let m_hi = mean_at(2.0e7);
        assert!((m_lo - 1.0e6).abs() < 2.0e4, "low-incident mean {m_lo} ≈ 1 MeV");
        assert!((m_hi - 2.0e6).abs() < 2.0e4, "high-incident mean {m_hi} ≈ 2 MeV");
        assert!(m_hi > 1.5 * m_lo, "χ must harden: {m_lo} → {m_hi}");
    }

    /// The Langevin inverse round-trips: L(L⁻¹(μ̄)) = μ̄, where L(λ)=coth λ − 1/λ.
    #[test]
    fn langevin_inverse_round_trips() {
        for &mu in &[-0.85_f64, -0.3, 0.05, 0.333, 0.6, 0.9] {
            let lambda = langevin_inverse(mu);
            let l = 1.0 / lambda.tanh() - 1.0 / lambda;
            assert!((l - mu).abs() < 1.0e-6, "L(L⁻¹({mu})) = {l}");
        }
    }
}
