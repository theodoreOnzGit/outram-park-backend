//! The settings of [`super`]'s four functions, as builders with NJOY's
//! defaults.

use uom::si::energy::electronvolt;

use crate::acer::thermal::InelasticForm;
use crate::leapr::vintage::PhysicalConstants;
use crate::units::NeutronEnergy;

/// PURR's controls (NJOY's `purr` card 2): probability bands, resonance
/// ladders, and samples per ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PurrSettings {
    /// Probability bands per energy (`nbin`), 20 in NJOY's reference decks.
    pub bands: usize,
    /// Resonance ladders sampled per energy (`nladr`), 64 there.
    pub ladders: usize,
    /// Samples per ladder, 10 000 in this crate's word-for-word gate.
    pub samples: usize,
}

impl Default for PurrSettings {
    fn default() -> Self {
        Self { bands: 20, ladders: 64, samples: 10_000 }
    }
}

/// Settings of [`super::cross_sections`] and [`super::ace`]. The default is
/// NJOY's deck with every physics module on: RECONR and BROADR at 0.001, PURR
/// with [`PurrSettings::default`], HEATR, and the ZAID suffix `.00c`.
///
/// ```
/// use njoy_outram_park_fork::process::CrossSectionSettings;
/// // NJOY2016's reference ACE tables here were made without HEATR:
/// let s = CrossSectionSettings::default().without_heatr();
/// assert!(!s.heatr && s.purr.is_some());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CrossSectionSettings {
    /// RECONR's fractional reconstruction tolerance (card 3 `err`).
    pub reconr_tolerance: f64,
    /// BROADR's fractional thinning tolerance (card 3 `errthn`).
    pub broadr_tolerance: f64,
    /// PURR's controls; `None` runs no PURR, so the ACE table has no
    /// unresolved-resonance probability tables.
    pub purr: Option<PurrSettings>,
    /// Whether HEATR runs; without it the ACE heating column is zero.
    pub heatr: bool,
    /// ACER only: the ZAID suffix, `0` for `.00c`, `80` for `.80c`.
    pub ace_suffix: u32,
}

impl Default for CrossSectionSettings {
    fn default() -> Self {
        Self { reconr_tolerance: 1.0e-3, broadr_tolerance: 1.0e-3, purr: Some(PurrSettings::default()), heatr: true, ace_suffix: 0 }
    }
}

impl CrossSectionSettings {
    /// Ablate PURR: no unresolved-resonance probability tables.
    pub fn without_purr(mut self) -> Self {
        self.purr = None;
        self
    }

    /// PURR with `bands` bands, `ladders` ladders and `samples` samples.
    pub fn with_purr(mut self, bands: usize, ladders: usize, samples: usize) -> Self {
        self.purr = Some(PurrSettings { bands, ladders, samples });
        self
    }

    /// Ablate HEATR: the ACE heating column is written as zero.
    pub fn without_heatr(mut self) -> Self {
        self.heatr = false;
        self
    }

    /// RECONR's tolerance (fractional, default 0.001).
    pub fn with_reconr_tolerance(mut self, tolerance: f64) -> Self {
        self.reconr_tolerance = tolerance;
        self
    }

    /// BROADR's thinning tolerance (fractional, default 0.001).
    pub fn with_broadr_tolerance(mut self, tolerance: f64) -> Self {
        self.broadr_tolerance = tolerance;
        self
    }

    /// The ZAID suffix of the ACE table (`0` gives `.00c`).
    pub fn with_ace_suffix(mut self, suffix: u32) -> Self {
        self.ace_suffix = suffix;
        self
    }
}

/// Settings of [`super::thermal_scattering`] and [`super::thermal_ace`].
/// The default is the NJOY deck OpenMC's `make_ace_thermal` writes, with
/// THERMR's tolerance 0.001 and 16 cosines, `natom` and `emax` from the
/// LEAPR law's own `B(6)` and `B(4)`, and ACER's 16 equally probable outgoing
/// energies.
///
/// ```
/// use njoy_outram_park_fork::process::ThermalSettings;
/// use njoy_outram_park_fork::units::NeutronEnergy;
/// use uom::si::energy::electronvolt;
/// // NJOY's usual hand-written deck: emax 4 eV, natom 2 for H in H2O.
/// let s = ThermalSettings::default()
///     .with_emax(NeutronEnergy::new::<electronvolt>(4.0))
///     .with_natom(2.0)
///     .with_table_name("hh2o");
/// assert_eq!(s.n_cosines, 16);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ThermalSettings {
    /// RECONR's tolerance for the incident nuclide's PENDF.
    pub reconr_tolerance: f64,
    /// BROADR's thinning tolerance for the incident nuclide's PENDF.
    pub broadr_tolerance: f64,
    /// THERMR card 4 `tol`: the tolerance `calcem` and `coh` linearise to.
    pub thermr_tolerance: f64,
    /// THERMR card 2 `nbin`: equally probable cosines per outgoing energy.
    pub n_cosines: usize,
    /// THERMR card 2 `natom`, principal scattering atoms per molecule;
    /// `None` takes the law's `B(6)` (LEAPR's `npr`).
    pub natom: Option<f64>,
    /// THERMR card 4 `emax`, the top of the thermal treatment; `None` takes
    /// the law's `B(4)` (`0.0253 eV × β_max` from LEAPR).
    pub emax: Option<NeutronEnergy>,
    /// LEAPR's physical constants; `None` takes the deck's own vintage
    /// ([`crate::leapr::LeaprDeck::constants`]: the constants of the deck's
    /// `EVAL` date, which reproduce the published tape). Set
    /// [`PhysicalConstants::Codata2018`] to reproduce a current NJOY2016 run.
    pub leapr_constants: Option<PhysicalConstants>,
    /// ACER only: card 9 `nbint`, outgoing energies per incident energy.
    pub n_outgoing: usize,
    /// ACER only: the inelastic form, card 9 `iwt` (1 equally probable, the
    /// default; 0 skewed; 2 continuous).
    pub form: InelasticForm,
    /// ACER only: the table's name in the ZAID (card 8 `tname`, at most 6
    /// characters); `None` gives `tsl` followed by the LEAPR MAT.
    pub table_name: Option<String>,
    /// ACER only: the ZAID suffix, `0` for `.00t`.
    pub ace_suffix: u32,
}

impl Default for ThermalSettings {
    fn default() -> Self {
        Self {
            reconr_tolerance: 1.0e-3,
            broadr_tolerance: 1.0e-3,
            thermr_tolerance: 1.0e-3,
            n_cosines: 16,
            natom: None,
            emax: None,
            leapr_constants: None,
            n_outgoing: 16,
            form: InelasticForm::Equiprobable,
            table_name: None,
            ace_suffix: 0,
        }
    }
}

impl ThermalSettings {
    /// THERMR's tolerance (fractional, default 0.001).
    pub fn with_thermr_tolerance(mut self, tolerance: f64) -> Self {
        self.thermr_tolerance = tolerance;
        self
    }

    /// THERMR's cosines per outgoing energy (default 16).
    pub fn with_cosines(mut self, n: usize) -> Self {
        self.n_cosines = n;
        self
    }

    /// Principal scattering atoms per molecule, instead of the law's `B(6)`.
    pub fn with_natom(mut self, natom: f64) -> Self {
        self.natom = Some(natom);
        self
    }

    /// The top of the thermal treatment, instead of the law's `B(4)`.
    pub fn with_emax(mut self, emax: NeutronEnergy) -> Self {
        self.emax = Some(emax);
        self
    }

    /// LEAPR's physical constants, instead of the deck's own vintage.
    pub fn with_leapr_constants(mut self, constants: PhysicalConstants) -> Self {
        self.leapr_constants = Some(constants);
        self
    }

    /// ACER's outgoing energies per incident energy (default 16).
    pub fn with_outgoing_energies(mut self, n: usize) -> Self {
        self.n_outgoing = n;
        self
    }

    /// ACER's inelastic form (default [`InelasticForm::Equiprobable`]).
    pub fn with_form(mut self, form: InelasticForm) -> Self {
        self.form = form;
        self
    }

    /// The table's ZAID name (at most 6 characters, e.g. `"hh2o"`).
    pub fn with_table_name(mut self, name: &str) -> Self {
        self.table_name = Some(name.to_string());
        self
    }

    /// The ZAID suffix (`0` gives `.00t`).
    pub fn with_ace_suffix(mut self, suffix: u32) -> Self {
        self.ace_suffix = suffix;
        self
    }

    /// The incident PENDF's settings: RECONR and BROADR only. THERMR reads
    /// the elastic cross section, MAT, AWR and ZA from it, so PURR and HEATR
    /// could not change the thermal table and are not run.
    pub(crate) fn incident(&self) -> super::CrossSectionSettings {
        super::CrossSectionSettings {
            reconr_tolerance: self.reconr_tolerance,
            broadr_tolerance: self.broadr_tolerance,
            purr: None,
            heatr: false,
            ace_suffix: 0,
        }
    }

    /// `emax` in eV, when set.
    pub(crate) fn emax_ev(&self) -> Option<f64> {
        self.emax.map(|e| e.get::<electronvolt>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every builder sets the field it names and nothing else; the defaults
    /// are NJOY's, with the physics on.
    #[test]
    fn builders_set_what_they_name() {
        let d = CrossSectionSettings::default();
        assert_eq!((d.reconr_tolerance, d.broadr_tolerance, d.heatr, d.ace_suffix), (1e-3, 1e-3, true, 0));
        assert_eq!(d.purr, Some(PurrSettings { bands: 20, ladders: 64, samples: 10_000 }));
        let s = d
            .clone()
            .with_purr(10, 32, 100)
            .with_reconr_tolerance(0.01)
            .with_broadr_tolerance(0.02)
            .with_ace_suffix(80);
        assert_eq!(s.purr, Some(PurrSettings { bands: 10, ladders: 32, samples: 100 }));
        assert_eq!((s.reconr_tolerance, s.broadr_tolerance, s.ace_suffix, s.heatr), (0.01, 0.02, 80, true));
        assert_eq!(s.without_purr().purr, None);

        let t = ThermalSettings::default();
        assert_eq!((t.thermr_tolerance, t.n_cosines, t.n_outgoing, t.natom, t.emax, t.form), (1e-3, 16, 16, None, None, InelasticForm::Equiprobable));
        let t = t
            .with_thermr_tolerance(0.05)
            .with_cosines(8)
            .with_outgoing_energies(32)
            .with_natom(2.0)
            .with_emax(NeutronEnergy::new::<electronvolt>(4.0))
            .with_leapr_constants(PhysicalConstants::Codata2018)
            .with_form(InelasticForm::Continuous)
            .with_table_name("hh2o")
            .with_ace_suffix(1);
        assert_eq!((t.thermr_tolerance, t.n_cosines, t.n_outgoing, t.natom, t.emax_ev()), (0.05, 8, 32, Some(2.0), Some(4.0)));
        assert_eq!((t.form, t.table_name.as_deref(), t.ace_suffix), (InelasticForm::Continuous, Some("hh2o"), 1));
        assert_eq!(t.leapr_constants, Some(PhysicalConstants::Codata2018));
        let inc = t.incident();
        assert_eq!((inc.purr, inc.heatr, inc.reconr_tolerance), (None, false, 1e-3));
    }
}
