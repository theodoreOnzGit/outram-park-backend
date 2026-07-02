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

use njoy_outram_park_fork::nuclear_data::secondary::NuBar;
use njoy_outram_park_fork::nuclear_data::{Mgxs, MgxsLibrary};
use njoy_outram_park_fork::reconr::ReconrResult;
use njoy_outram_park_fork::wmp::{WindowedMultipole, WmpLibrary};
use njoy_outram_park_fork::MtReaction;
use njoy_outram_park_fork::NjoyError;

/// Microscopic cross sections at a given energy (barn = 1e-24 cm²).
///
/// The transport-side currency. `absorption = capture + fission`; `nu_fission`
/// is ν̄·σ_f (the fission-source production channel).
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
    /// Fission production ν̄·σ_f \[barn\].
    pub nu_fission: f64,
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
        Ok(Self { name: name.to_string(), awr, nu, xs: XsSource::Core { e_max, wmp, fast } })
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

        Ok(Self { name: name.to_string(), awr, nu, xs: XsSource::Pointwise { recon } })
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
                        nu_fission: x.fission * self.nu.at(e),
                    }
                } else if let Some(mg) = fast {
                    let m = mg.micro(e);
                    MicroXS {
                        total: m.total,
                        elastic: m.elastic,
                        fission: m.fission,
                        absorption: m.capture + m.fission,
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
                        nu_fission: x.fission * self.nu.at(e),
                    }
                }
            }
            XsSource::Pointwise { recon } => {
                let total = recon.eval_mt(MtReaction::Mt1Total, e);
                let elastic = recon.eval_mt(MtReaction::Mt2Elastic, e);
                let fission = recon.eval_mt(MtReaction::Mt18Fission, e);
                let capture = recon.eval_mt(MtReaction::Mt102Capture, e);
                MicroXS {
                    total,
                    elastic,
                    fission,
                    absorption: fission + capture,
                    nu_fission: fission * self.nu.at(e),
                }
            }
        }
    }
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
