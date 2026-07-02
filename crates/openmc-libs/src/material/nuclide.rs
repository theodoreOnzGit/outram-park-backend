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
use njoy_outram_park_fork::wmp::{WindowedMultipole, WmpLibrary};
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

/// One isotope's cross-section data, pulled from `njoy-outram-park-fork`.
///
/// Construct with [`Nuclide::from_core`], which resolves the isotope out of the
/// embedded CORE libraries (no downloads, no HDF5). The stored objects are:
/// the WMP evaluator (thermal + resonance), an optional fast MGXS set (`None`
/// only when the WMP already spans to 20 MeV, e.g. H-1), and a ν̄(E) table.
pub struct Nuclide {
    /// Isotope name, e.g. `"U235"`.
    pub name: String,
    /// Atomic weight ratio (target mass / neutron mass) — sets scatter kinematics.
    pub awr: f64,
    /// Upper energy of the WMP representation \[eV\]; the CE↔MG seam.
    e_max: f64,
    /// Windowed-multipole cross sections (valid on `[e_min, e_max]`).
    wmp: WindowedMultipole,
    /// Average neutron yield ν̄(E).
    nu: NuBar,
    /// Fast-range group constants above `e_max` (`None` ⇒ WMP covers everything).
    fast: Option<Mgxs>,
}

impl Nuclide {
    /// Resolve a nuclide from the embedded CORE nuclear-data libraries.
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
        Ok(Self { name: name.to_string(), awr, e_max, wmp, nu, fast })
    }

    /// Microscopic cross sections at incident energy `e` \[eV\] and temperature
    /// `temp_k` \[K\].
    ///
    /// Below `e_max` the analytic-Doppler WMP form is used (temperature-dependent);
    /// above `e_max` the fast MGXS constant-per-group lookup takes over
    /// (temperature-independent by construction — the fast range is smooth).
    pub fn xs_at_energy(&self, e: f64, temp_k: f64) -> MicroXS {
        if e <= self.e_max {
            let x = self.wmp.evaluate(e, temp_k);
            MicroXS {
                total: x.total(),
                elastic: x.scatter,
                fission: x.fission,
                absorption: x.absorption,
                nu_fission: x.fission * self.nu.at(e),
            }
        } else if let Some(mg) = &self.fast {
            let m = mg.micro(e);
            MicroXS {
                total: m.total,
                elastic: m.elastic,
                fission: m.fission,
                absorption: m.capture + m.fission,
                nu_fission: m.nu_fission,
            }
        } else {
            // No fast set ⇒ the WMP spans the whole sublibrary; clamp to e_max.
            let x = self.wmp.evaluate(self.e_max, temp_k);
            MicroXS {
                total: x.total(),
                elastic: x.scatter,
                fission: x.fission,
                absorption: x.absorption,
                nu_fission: x.fission * self.nu.at(e),
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
