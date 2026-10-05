// SPDX-License-Identifier: GPL-3.0-only
//! **The HTR-10 nuclear-data set, in one place: which nuclides, from which
//! tapes, with which thermal laws, at which slots.**
//!
//! # Why this module exists (2026-10-01)
//!
//! Until 2026-10-01 every HTR-10 driver built its own nuclide array:
//! `htr10_rmc_keff::nuclides`, `htr10_endf8_height_sweep::nuclides_endf8`, and
//! a private `const NUC: Htr10Nuclides` in half a dozen other examples. The
//! sweep's own docs said *"the two must not drift: if a tape name or a thermal
//! law changes there, change it here"*, which is the drift this workspace
//! forbids, stated as an instruction. Three maintainer decisions of 2026-10-01
//! changed the nuclide set at once (natural carbon, gh:#425; helium coolant,
//! gh:#426; real nickel, gh:#329), so the set is built here and every driver
//! asks for it.
//!
//! # Two steps, so the composition can be checked without loading anything
//!
//! 1. [`Htr10NuclideLayout::plan`] is **pure**: from an [`Htr10DataConfig`]
//!    it decides every slot (name, tape, thermal law) and the index tables
//!    the materials use ([`Htr10Nuclides`], [`CoolantNuclides`],
//!    [`RodMetalNuclides`]). Unit tests build the material set from a plan and
//!    check compositions with no nuclear data on disk.
//! 2. [`load_htr10_nuclides`] reads the tapes the plan names, binds the
//!    thermal laws, and records every item in a [`RunDiagnostics`].
//!
//! # The default is the correct physics (workspace hard rule)
//!
//! [`Htr10DataConfig::default`] is ENDF/B-VIII.0 with:
//! - **natural carbon**, C-12 / C-13 at 98.93 / 1.07 at.% (gh:#425);
//! - **helium coolant** at 300.15 K and 101.33 kPa (gh:#426);
//! - **real nickel and iron** in the rod steel, Ni-58/60/61/62/64 and
//!   Fe-54/56/57/58 (gh:#329). ~~which cannot be loaded until gh:#339 is
//!   fixed~~ #339 is fixed (2026-10-01; see [`FE57_RECONSTRUCTION_FIXED`]);
//! - every bound thermal law: graphite (30P), C-in-SiC, Si-in-SiC, U-in-UO2,
//!   O-in-UO2.
//!
//! Every other choice is a named ablation on one field of the config. The
//! pin is `tests/htr10_correct_physics_is_default.rs`.

use std::path::PathBuf;
use std::time::Instant;

use njoy_outram_park_fork::leapr::decks::SabMaterial;
use njoy_outram_park_fork::reference_data::{ace_submodule_dir, reference_data_dir};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::htr10::{CarbonSlot, Htr10Nuclides};
use outram_mc_libs::run_diagnostics::{DataSource, RunDiagnostics};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

use super::materials::{GraphiteLaw, RodMetalNuclides, ROD_METAL_TAPES_ENDF8};

/// **Whether ENDF/B-VIII.0 Fe-57 can be reconstructed on this code base.**
///
/// `false` until gh:#339 is fixed: reconstructing `n-026_Fe_057-ENDF8.0.endf`
/// (LRU=1, LRF=7, three particle pairs) exhausted 13.4 GB and was OOM-killed
/// on 2026-09-26, which can take the whole session down on a 15 GB desktop.
/// While it is `false`, [`load_htr10_nuclides`] **refuses** any layout that
/// loads the Fe-57 tape, with [`Htr10DataError::BlockedByGh339`], before it
/// reads a single tape. It does not swap Fe-57 for anything: the simplified
/// treatment that does ([`RodMetalTreatment::Simplified`]) must be asked for
/// by name.
///
/// Whoever fixes gh:#339 flips this to `true` in the same change, after
/// measuring the reconstruction's peak memory.
///
/// **FLIPPED to `true` 2026-10-01 (gh:#339 fixed in `njoy-outram-park-fork`).**
/// The cause was a wrong operand in the ported LINPACK `xdot` (upstream
/// `samm.f90:6189, 6201-6204`), which mis-inverted every spin group with four
/// or more coupled channels, plus the missing non-negativity guard of upstream
/// `reconr.f90:2641-2645`. Fe-57 now reconstructs in 0.10 s at 24 MB peak RSS,
/// word for word NJOY2016's PENDF
/// (`njoy-outram-park-fork/tests/reconr_lrf7_threshold_channels_vs_njoy2016.rs`).
pub const FE57_RECONSTRUCTION_FIXED: bool = true;

/// Which evaluated library the nuclides come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NuclearDataLibrary {
    /// ENDF/B-VIII.0. **Default.**
    #[default]
    EndfB8,
    /// ENDF/B-VII.0, the library Li, Yu & Wei (2014) state for RMC
    /// (`OUTRAM_HTR10_ENDF7`). Carbon is elemental C-nat (MAT 600); there is no
    /// SiC thermal law; the rod metals and helium come from VIII.0 because the
    /// checkout has no VII.0 tapes for them (a mixed-library arm, stated in the
    /// diagnostics).
    EndfB7,
}

/// How natural carbon is represented on the ENDF/B-VIII.0 path.
///
/// The VII.0 arm ignores this: its evaluation is elemental C-nat, which is
/// natural carbon already.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CarbonTreatment {
    /// **Default (maintainer decision 2026-10-01, gh:#425):** natural carbon,
    /// C-12 / C-13 at 98.93 / 1.07 at.% (IUPAC), each kind of carbon two
    /// nuclides with the same thermal law bound to both.
    #[default]
    Natural,
    /// ABLATION (`OUTRAM_HTR10_CARBON_AS_C12`): all carbon loaded as C-12 at
    /// the natural-carbon atom density. This was the VIII.0 model until
    /// 2026-10-01; kept so the C-13 term can be priced.
    AllC12,
}

/// What fills the coolant regions (`mat::HELIUM`: between the pebbles, the
/// core cavity, the discharge tube between balls, the empty control-rod,
/// KLAK, coolant and irradiation channels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Coolant {
    /// **Default (maintainer decision 2026-10-01, gh:#426, "use helium, I
    /// think it is more accurate"):** natural helium, ideal gas at the
    /// material temperature and [`super::materials::HELIUM_PRESSURE_KPA`].
    #[default]
    Helium,
    /// ABLATION (`OUTRAM_HTR10_VACUUM_COOLANT`): an empty material, i.e. exact
    /// vacuum. Kept because Li, Yu & Wei (2014) report *"Calculations are
    /// performed for vacuum and helium"*, and this was the model until
    /// 2026-10-01.
    Vacuum,
}

/// How the withdrawn control rods' sleeve steel and joint iron are
/// represented (TECDOC-1382 § 4.1.1.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RodMetalTreatment {
    /// **Default, the FULL case (maintainer decision 2026-10-01, gh:#329):**
    /// real Ni-58/60/61/62/64 (tapes from the `reference-data/ace` submodule)
    /// and real Fe-54/56/57/58. **Cannot be loaded until gh:#339 is fixed**
    /// ([`FE57_RECONSTRUCTION_FIXED`]); the loader refuses it rather than
    /// swapping Fe-57.
    #[default]
    Full,
    /// The SIMPLIFIED case: Ni replaced atom for atom by Fe (Ni-58/60 -> Fe-56,
    /// Ni-61 -> Fe-57 -> Fe-56, Ni-62 -> Fe-54, Ni-64 -> Fe-58) and Fe-57 taken
    /// as Fe-56. Both are stated modelling assumptions (maintainer 2026-09-26),
    /// equivalent to `OUTRAM_HTR10_NI_AS_FE=1 OUTRAM_HTR10_FE57_AS_FE56=1`.
    Simplified,
    /// `OUTRAM_HTR10_NI_AS_FE` alone: Ni -> Fe, real Fe-57 (so also blocked
    /// by gh:#339).
    NiAsFeOnly,
    /// `OUTRAM_HTR10_FE57_AS_FE56` alone: real Ni, Fe-57 -> Fe-56.
    Fe57AsFe56Only,
    /// `OUTRAM_HTR10_NO_WITHDRAWN_RODS`: the rod channels are empty helium and
    /// no rod-metal tape is loaded. The rod materials are still built; the
    /// caller strips their unloaded components and proves no cell uses them.
    NotModelled,
}

impl RodMetalTreatment {
    /// Whether Ni is replaced by Fe.
    #[must_use]
    pub fn ni_as_fe(self) -> bool {
        matches!(self, Self::Simplified | Self::NiAsFeOnly)
    }

    /// Whether Fe-57 is replaced by Fe-56.
    #[must_use]
    pub fn fe57_as_fe56(self) -> bool {
        matches!(self, Self::Simplified | Self::Fe57AsFe56Only)
    }

    /// Whether any rod-metal tape is loaded.
    #[must_use]
    pub fn loads_rod_metal(self) -> bool {
        !matches!(self, Self::NotModelled)
    }

    /// The treatment the three legacy environment knobs select.
    #[must_use]
    pub fn from_knobs(ni_as_fe: bool, fe57_as_fe56: bool, no_withdrawn_rods: bool) -> Self {
        match (no_withdrawn_rods, ni_as_fe, fe57_as_fe56) {
            (true, _, _) => Self::NotModelled,
            (false, true, true) => Self::Simplified,
            (false, true, false) => Self::NiAsFeOnly,
            (false, false, true) => Self::Fe57AsFe56Only,
            (false, false, false) => Self::Full,
        }
    }

    /// One line for logs and diagnostics.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Full => "FULL: real Ni-58/60/61/62/64 and Fe-54/56/57/58",
            Self::Simplified => "SIMPLIFIED: Ni -> Fe atom for atom, Fe-57 -> Fe-56",
            Self::NiAsFeOnly => "Ni -> Fe atom for atom, real Fe-57",
            Self::Fe57AsFe56Only => "real Ni, Fe-57 -> Fe-56",
            Self::NotModelled => "withdrawn rods NOT modelled (channels empty)",
        }
    }
}

/// Whether the bound thermal scattering laws are applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThermalScatteringTreatment {
    /// **Default:** every bound law the library provides (graphite, C-in-SiC,
    /// Si-in-SiC) plus the UO2 laws.
    #[default]
    Bound,
    /// ABLATION (`OUTRAM_HTR10_NO_SAB`): every nuclide a free gas. A harness
    /// check: in a graphite-moderated core this must be worth a large,
    /// resolved amount.
    FreeGas,
}

/// Which U-238 evaluation the VIII.0 arm uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum U238Evaluation {
    /// The library's own. **Default.** (VIII.0 tape `n-092_U_238.endf`.)
    #[default]
    Library,
    /// ABLATION (`OUTRAM_HTR10_U238_JENDL`): JENDL-3.3. A different-library
    /// bound on the dominant absorber, never the VII.0 offset.
    Jendl33,
}

/// Where the UO2 thermal laws come from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Uo2Laws {
    /// **Default:** generated in-process from the LEAPR decks committed in
    /// `njoy-outram-park-fork` (the VIII.0 evaluation).
    #[default]
    GeneratedFromLeapr,
    /// Tabulated tapes `tsl-UinUO2.endf` (MAT 76) and `tsl-OinUO2.endf`
    /// (MAT 75) in this directory (`OUTRAM_HTR10_UO2_TAPE_DIR`); how the VII.0
    /// arm gets its own UO2 laws. A tape that fails to load is an error.
    Tapes(PathBuf),
}

/// Everything the nuclide set depends on.
#[derive(Debug, Clone, PartialEq)]
pub struct Htr10DataConfig {
    /// Evaluated library.
    pub library: NuclearDataLibrary,
    /// Graphite S(a,b) on the VIII.0 path. Ignored by VII.0 (one law only).
    pub graphite_law: GraphiteLaw,
    /// Carbon representation on the VIII.0 path.
    pub carbon: CarbonTreatment,
    /// Coolant regions: helium or vacuum.
    pub coolant: Coolant,
    /// Rod steel and joint iron.
    pub rod_metal: RodMetalTreatment,
    /// Bound thermal laws on or off.
    pub thermal: ThermalScatteringTreatment,
    /// U-238 evaluation (VIII.0 only).
    pub u238: U238Evaluation,
    /// Source of the UO2 thermal laws.
    pub uo2_laws: Uo2Laws,
    /// Temperature every nuclide is reconstructed and broadened at. 300.15 K
    /// (27 °C) is the temperature Li, Yu & Wei (2014) and Şeker & Çolak (2003)
    /// state.
    pub temperature: ThermodynamicTemperature,
}

impl Default for Htr10DataConfig {
    /// The correct-physics default: see the module docs.
    fn default() -> Self {
        Self {
            library: NuclearDataLibrary::default(),
            graphite_law: GraphiteLaw::default(),
            carbon: CarbonTreatment::default(),
            coolant: Coolant::default(),
            rod_metal: RodMetalTreatment::default(),
            thermal: ThermalScatteringTreatment::default(),
            u238: U238Evaluation::default(),
            uo2_laws: Uo2Laws::default(),
            temperature: ThermodynamicTemperature::new::<kelvin>(300.15),
        }
    }
}

/// A directory of evaluated tapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataDir {
    /// `reference-data/endf/`, tracked in this repository.
    Endf,
    /// `reference-data/ace/endf/endf-b-viii.0/`, in the `ace_and_other_data`
    /// submodule (the nickel tapes since 2026-10-01; MANIFEST.tsv one level
    /// up). `git submodule update --init reference-data/ace`.
    AceSubmoduleEndfB8,
}

impl DataDir {
    /// Absolute directory, honouring `OUTRAM_PARK_REFERENCE_DATA_DIR`.
    #[must_use]
    pub fn path(self) -> PathBuf {
        match self {
            Self::Endf => reference_data_dir("endf"),
            Self::AceSubmoduleEndfB8 => ace_submodule_dir().join("endf").join("endf-b-viii.0"),
        }
    }
}

/// One evaluated tape: directory and file name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tape {
    /// Directory.
    pub dir: DataDir,
    /// File name.
    pub file: &'static str,
}

impl Tape {
    /// A tape in `reference-data/endf/`.
    #[must_use]
    pub const fn endf(file: &'static str) -> Self {
        Self {
            dir: DataDir::Endf,
            file,
        }
    }

    /// Absolute path.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.dir.path().join(self.file)
    }
}

/// A bound thermal scattering law a slot is to carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalLaw {
    /// Graphite S(a,b) from a tape: VIII.0 `GraphiteLaw` (MAT 30/31/32) or the
    /// VII.0 `tsl-graphite-ENDF7.0.endf` (MAT 31).
    Graphite {
        /// Tape file in `reference-data/endf/`.
        file: &'static str,
        /// MAT in the tape's control columns.
        mat: i32,
    },
    /// C-in-SiC, VIII.0 `tsl-CinSiC.endf`, MAT 44.
    CInSiC,
    /// Si-in-SiC, VIII.0 `tsl-SiinSiC.endf`, MAT 43.
    SiInSiC,
    /// U-in-UO2 (LEAPR deck or tape, see [`Uo2Laws`]).
    UInUO2,
    /// O-in-UO2 (LEAPR deck or tape, see [`Uo2Laws`]).
    OInUO2,
}

impl ThermalLaw {
    /// Short label for slot names.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Graphite { .. } => "graphite S(a,b)",
            Self::CInSiC => "C-in-SiC S(a,b)",
            Self::SiInSiC => "Si-in-SiC S(a,b)",
            Self::UInUO2 => "U-in-UO2 S(a,b)",
            Self::OInUO2 => "O-in-UO2 S(a,b)",
        }
    }
}

/// One slot of the nuclide array.
#[derive(Debug, Clone, PartialEq)]
pub struct NuclideSlot {
    /// Nuclide name passed to the reconstruction (e.g. `"C13"`).
    pub name: &'static str,
    /// Tape it is read from.
    pub tape: Tape,
    /// Thermal law bound to it, if any (already `None` under the free-gas
    /// ablation and for VII.0 SiC).
    pub thermal: Option<ThermalLaw>,
}

impl NuclideSlot {
    /// Human-readable label: name and thermal treatment.
    #[must_use]
    pub fn label(&self) -> String {
        match self.thermal {
            Some(l) => format!("{} ({})", self.name, l.label()),
            None => format!("{} (free gas)", self.name),
        }
    }
}

/// Where the coolant's nuclides sit, or that there are none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoolantNuclides {
    /// Natural helium: He-3 and He-4 slots.
    Helium {
        /// He-3 slot.
        he3: usize,
        /// He-4 slot.
        he4: usize,
    },
    /// The vacuum ablation: no coolant nuclides.
    Vacuum,
}

/// The complete nuclide plan: every slot in order, and the index tables the
/// material set reads.
#[derive(Debug, Clone)]
pub struct Htr10NuclideLayout {
    /// Pebble, reflector and B4C slots (carbon as [`CarbonSlot`]s).
    pub pebble: Htr10Nuclides,
    /// Coolant slots.
    pub coolant: CoolantNuclides,
    /// Rod-metal slots. Under [`RodMetalTreatment::Simplified`] the Ni (and
    /// Fe-57) fields point at Fe slots; under
    /// [`RodMetalTreatment::NotModelled`] they point past [`Self::len`].
    pub metal: RodMetalNuclides,
    /// The rod-metal treatment this layout realises.
    pub rod_metal: RodMetalTreatment,
    /// The slots, in array order.
    pub slots: Vec<NuclideSlot>,
    /// Modelling statements a run must record (mixed-library arms,
    /// substitutions, ablations), one line each.
    pub notes: Vec<String>,
}

/// Why the nuclide set could not be built.
#[derive(Debug, thiserror::Error)]
pub enum Htr10DataError {
    /// The layout loads Fe-57 and gh:#339 is not fixed.
    #[error(
        "BLOCKED on gh:#339: this layout loads ENDF/B-VIII.0 Fe-57 \
         (n-026_Fe_057-ENDF8.0.endf), whose reconstruction exhausts ~13 GB and \
         was OOM-killed. Not loading it, and not substituting it silently. Use \
         the SIMPLIFIED rod-metal case (Ni -> Fe, Fe-57 -> Fe-56) until #339 is \
         fixed, then flip `htr10_rmc::data::FE57_RECONSTRUCTION_FIXED`."
    )]
    BlockedByGh339,
    /// A configuration that does not exist (e.g. a VIII.0-only ablation on
    /// the VII.0 arm).
    #[error("invalid HTR-10 data configuration: {0}")]
    InvalidConfig(String),
    /// A tape is not on disk.
    #[error("{name}: tape {path} is not in this checkout (for the Ni tapes: git submodule update --init reference-data/ace)")]
    MissingTape {
        /// Slot or law name.
        name: String,
        /// Path tried.
        path: PathBuf,
    },
    /// A tape is present but did not load.
    #[error("{name}: loading {path} failed")]
    LoadFailed {
        /// Slot or law name.
        name: String,
        /// Path tried.
        path: PathBuf,
    },
}

/// The C-12 and C-13 tapes (VIII.0) and the elemental C-nat tape (VII.0).
const C12: Tape = Tape::endf("n-006_C_012-ENDF8.0.endf");
const C13: Tape = Tape::endf("n-006_C_013-ENDF8.0.endf");
const CNAT_ENDF7: Tape = Tape::endf("n-006_C_000-ENDF7.0.endf");
/// Neutron He-3 and He-4, ENDF/B-VIII.0 (added 2026-10-01, `ebbcb62b96`).
/// NOT `a-002_He_004`, which is the incident-ALPHA sublibrary.
const HE3: Tape = Tape::endf("n-002_He_003-ENDF8.0.endf");
const HE4: Tape = Tape::endf("n-002_He_004-ENDF8.0.endf");

impl Htr10NuclideLayout {
    /// Decide every slot for `cfg`. Pure: reads nothing from disk.
    ///
    /// Slot order: the pebble layout ([`Htr10Nuclides::NATURAL_CARBON`], 14
    /// slots, or [`Htr10Nuclides::ELEMENTAL_CARBON`], 11), then He-3 and He-4
    /// (unless vacuum), then the rod metals (last, so the no-rods ablation can
    /// strip them by index).
    ///
    /// # Errors
    ///
    /// [`Htr10DataError::InvalidConfig`] for a VIII.0-only ablation asked of
    /// the VII.0 arm (`AllC12`, `Jendl33`, a non-default graphite law).
    pub fn plan(cfg: &Htr10DataConfig) -> Result<Self, Htr10DataError> {
        let endf7 = cfg.library == NuclearDataLibrary::EndfB7;
        let free = cfg.thermal == ThermalScatteringTreatment::FreeGas;
        let mut notes = Vec::new();
        if endf7 {
            if cfg.carbon != CarbonTreatment::Natural {
                return Err(Htr10DataError::InvalidConfig(
                    "the C-12-only carbon ablation applies to ENDF/B-VIII.0 only; \
                     VII.0 carbon is elemental C-nat"
                        .into(),
                ));
            }
            if cfg.u238 != U238Evaluation::Library {
                return Err(Htr10DataError::InvalidConfig(
                    "the JENDL-3.3 U-238 ablation applies to ENDF/B-VIII.0 only".into(),
                ));
            }
            if cfg.graphite_law != GraphiteLaw::default() {
                return Err(Htr10DataError::InvalidConfig(
                    "the graphite-law choice applies to ENDF/B-VIII.0 only".into(),
                ));
            }
        }

        // Thermal laws, or none under the free-gas ablation.
        let graphite = (!free).then_some(if endf7 {
            ThermalLaw::Graphite {
                file: "tsl-graphite-ENDF7.0.endf",
                mat: 31,
            }
        } else {
            ThermalLaw::Graphite {
                file: cfg.graphite_law.tape(),
                mat: cfg.graphite_law.mat(),
            }
        });
        // VII.0 ships no SiC thermal evaluation: a real library difference.
        let sic = !free && !endf7;
        let c_in_sic = sic.then_some(ThermalLaw::CInSiC);
        let si_in_sic = sic.then_some(ThermalLaw::SiInSiC);
        let u_uo2 = (!free).then_some(ThermalLaw::UInUO2);
        let o_uo2 = (!free).then_some(ThermalLaw::OInUO2);
        if free {
            notes.push("ABLATION: every S(a,b) disabled -- all nuclides free gas".into());
        } else if endf7 {
            notes.push("SiC S(a,b) not applied: ENDF/B-VII.0 has no SiC thermal evaluation".into());
        }

        let lib = |b8: &'static str, b7: &'static str| Tape::endf(if endf7 { b7 } else { b8 });
        let u238 = if endf7 {
            Tape::endf("n-092_U_238-ENDF7.0.endf")
        } else if cfg.u238 == U238Evaluation::Jendl33 {
            notes.push(
                "ABLATION: U-238 from JENDL-3.3 (a library bound, NOT the VII.0 offset)".into(),
            );
            Tape::endf("n-092_U_238-JENDL3.3.endf")
        } else {
            Tape::endf("n-092_U_238.endf")
        };

        // Carbon: elemental on VII.0 (C-nat) and under the C-12 ablation;
        // natural C-12/C-13 otherwise.
        let (pebble, c_tape, c_name, split) = if endf7 {
            notes.push("carbon: ENDF/B-VII.0 elemental natural carbon (MAT 600)".into());
            (Htr10Nuclides::ELEMENTAL_CARBON, CNAT_ENDF7, "Cnat", false)
        } else if cfg.carbon == CarbonTreatment::AllC12 {
            notes.push("ABLATION: all carbon as C-12 (no C-13)".into());
            (Htr10Nuclides::ELEMENTAL_CARBON, C12, "C12", false)
        } else {
            notes.push("carbon: natural, C-12 / C-13 at 98.93 / 1.07 at.% (IUPAC)".into());
            (Htr10Nuclides::NATURAL_CARBON, C12, "C12", true)
        };
        let slot = |name, tape, thermal| NuclideSlot {
            name,
            tape,
            thermal,
        };
        let mut slots = vec![
            slot(
                "U235",
                lib("n-092_U_235-ENDF8.0.endf", "n-092_U_235-ENDF7.0.endf"),
                u_uo2,
            ),
            slot("U238", u238, u_uo2),
            slot(
                "O16",
                lib("n-008_O_016-ENDF8.0.endf", "n-008_O_016-ENDF7.0.endf"),
                o_uo2,
            ),
            slot(c_name, c_tape, None), // free carbon (rod B4C and steel)
            slot(c_name, c_tape, graphite),
            slot(
                "Si28",
                lib("n-014_Si_028-ENDF8.0.endf", "n-014_Si_028-ENDF7.0.endf"),
                si_in_sic,
            ),
            slot(
                "B10",
                lib("n-005_B_010-ENDF8.0.endf", "n-005_B_010-ENDF7.0.endf"),
                None,
            ),
            slot(c_name, c_tape, c_in_sic),
            slot(
                "Si29",
                lib("n-014_Si_029-ENDF8.0.endf", "n-014_Si_029-ENDF7.0.endf"),
                si_in_sic,
            ),
            slot(
                "Si30",
                lib("n-014_Si_030-ENDF8.0.endf", "n-014_Si_030-ENDF7.0.endf"),
                si_in_sic,
            ),
            slot(
                "B11",
                lib("n-005_B_011-ENDF8.0.endf", "n-005_B_011-ENDF7.0.endf"),
                None,
            ),
        ];
        if split {
            // 11, 12, 13: C-13 for the free, graphite and SiC carbon, with the
            // same thermal law as its C-12 partner (as OpenMC binds
            // c_Graphite to C12 and C13).
            slots.push(slot("C13", C13, None));
            slots.push(slot("C13", C13, graphite));
            slots.push(slot("C13", C13, c_in_sic));
        }
        assert_eq!(
            slots.len(),
            pebble.slot_count(),
            "pebble slots match the index table"
        );
        assert!(matches!(pebble.c_graphite, CarbonSlot::Natural { .. }) == split);

        let coolant = match cfg.coolant {
            Coolant::Helium => {
                let he3 = slots.len();
                slots.push(slot("He3", HE3, None));
                slots.push(slot("He4", HE4, None));
                notes.push(format!(
                    "coolant: natural helium (He-3 {:.3e} at.), ideal gas at {:.2} K and {} kPa (pressure ASSUMED atmospheric, after Seker & Colak 2003 p.267)",
                    super::materials::HE3_ATOM_FRACTION_OF_NATURAL_HE,
                    cfg.temperature.get::<kelvin>(),
                    super::materials::HELIUM_PRESSURE_KPA,
                ));
                if endf7 {
                    notes.push("helium is ENDF/B-VIII.0 in this ENDF/B-VII.0 arm: no VII.0 helium tape in reference-data/endf".into());
                }
                CoolantNuclides::Helium { he3, he4: he3 + 1 }
            }
            Coolant::Vacuum => {
                notes.push("ABLATION: coolant regions are exact vacuum (no helium)".into());
                CoolantNuclides::Vacuum
            }
        };

        let first_metal = slots.len();
        let (metal_tapes, metal) = rod_metal_plan(first_metal, cfg.rod_metal);
        notes.push(format!("rod metal: {}", cfg.rod_metal.label()));
        if cfg.rod_metal.loads_rod_metal() && endf7 {
            notes.push(
                "rod-metal nuclides (Fe, Cr, Ni, Mn, Ti, free Si) are ENDF/B-VIII.0 in this \
                 ENDF/B-VII.0 arm: no VII.0 tapes for them"
                    .into(),
            );
        }
        for (name, tape) in metal_tapes {
            slots.push(slot(name, tape, None));
        }

        Ok(Self {
            pebble,
            coolant,
            metal,
            rod_metal: cfg.rod_metal,
            slots,
            notes,
        })
    }

    /// Number of slots, i.e. the length of the nuclide array.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether the layout has no slots (never, for a planned layout).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Whether the plan loads the Fe-57 tape (blocked by gh:#339 while
    /// [`FE57_RECONSTRUCTION_FIXED`] is `false`).
    #[must_use]
    pub fn loads_fe57(&self) -> bool {
        self.slots
            .iter()
            .any(|s| s.tape.file == "n-026_Fe_057-ENDF8.0.endf")
    }

    /// Label of slot `idx`, or `"unknown"`.
    #[must_use]
    pub fn nuclide_name(&self, idx: usize) -> String {
        self.slots
            .get(idx)
            .map_or_else(|| "unknown".to_string(), NuclideSlot::label)
    }
}

/// Which rod-metal tapes to load, and the slot table pointing into them,
/// for slots starting at `first`.
///
/// Full: every tape of [`ROD_METAL_TAPES_ENDF8`]. Ni -> Fe points each Ni
/// isotope at an Fe isotope (Ni-58/60 -> Fe-56, Ni-61 -> Fe-57, Ni-62 ->
/// Fe-54, Ni-64 -> Fe-58, which puts the replaced atoms close to natural Fe's
/// isotopics) and skips the Ni tapes; Fe-57 -> Fe-56 skips the Fe-57 tape and
/// points everything that pointed at Fe-57 at Fe-56. Slots of the tapes still
/// loaded are assigned in table order, so no index is guessed. Not modelled:
/// no tapes, and the contiguous table past the end (stripped by the caller).
///
/// Moved here from `examples/htr10_rmc_keff.rs::rod_metal_plan` on 2026-10-01.
fn rod_metal_plan(
    first: usize,
    t: RodMetalTreatment,
) -> (Vec<(&'static str, Tape)>, RodMetalNuclides) {
    if !t.loads_rod_metal() {
        return (Vec::new(), RodMetalNuclides::contiguous(first));
    }
    let (ni_as_fe, fe57_as_fe56) = (t.ni_as_fe(), t.fe57_as_fe56());
    let resolve = |name: &'static str| -> &'static str {
        let n = match name {
            "Ni58" | "Ni60" if ni_as_fe => "Fe56",
            "Ni61" if ni_as_fe => "Fe57",
            "Ni62" if ni_as_fe => "Fe54",
            "Ni64" if ni_as_fe => "Fe58",
            other => other,
        };
        if fe57_as_fe56 && n == "Fe57" {
            "Fe56"
        } else {
            n
        }
    };
    let loaded: Vec<(&'static str, Tape)> = ROD_METAL_TAPES_ENDF8
        .iter()
        .copied()
        .filter(|(n, _)| resolve(n) == *n)
        .collect();
    let slot = |name: &'static str| -> usize {
        let target = resolve(name);
        first
            + loaded
                .iter()
                .position(|(n, _)| *n == target)
                .expect("replacement tape is loaded")
    };
    let table = RodMetalNuclides {
        fe54: slot("Fe54"),
        fe56: slot("Fe56"),
        fe57: slot("Fe57"),
        fe58: slot("Fe58"),
        cr50: slot("Cr50"),
        cr52: slot("Cr52"),
        cr53: slot("Cr53"),
        cr54: slot("Cr54"),
        ni58: slot("Ni58"),
        ni60: slot("Ni60"),
        ni61: slot("Ni61"),
        ni62: slot("Ni62"),
        ni64: slot("Ni64"),
        mn55: slot("Mn55"),
        ti46: slot("Ti46"),
        ti47: slot("Ti47"),
        ti48: slot("Ti48"),
        ti49: slot("Ti49"),
        ti50: slot("Ti50"),
        si28: slot("Si28"),
        si29: slot("Si29"),
        si30: slot("Si30"),
    };
    if t == RodMetalTreatment::Full {
        assert_eq!(
            table,
            RodMetalNuclides::contiguous(first),
            "the full plan is contiguous"
        );
    }
    (loaded, table)
}

/// Read every tape `layout` names at `cfg.temperature`, bind the thermal
/// laws, and record each item (path, timing, success) in `diag`, together with
/// the layout's modelling notes.
///
/// Reconstruction tolerance is NJOY's 1e-3. Each slot is reconstructed
/// separately, as before (a nuclide carrying a thermal law is a distinct
/// object).
///
/// # Errors
///
/// - [`Htr10DataError::BlockedByGh339`] before anything is read, if the layout
///   loads Fe-57 and [`FE57_RECONSTRUCTION_FIXED`] is `false`;
/// - [`Htr10DataError::MissingTape`] / [`Htr10DataError::LoadFailed`] for a
///   nuclide or graphite law that is absent or fails. A missing SiC law falls
///   back to free gas with a note, as it always has; a UO2 tape that fails is
///   an error.
pub fn load_htr10_nuclides(
    cfg: &Htr10DataConfig,
    layout: &Htr10NuclideLayout,
    diag: &mut RunDiagnostics,
) -> Result<Vec<Nuclide>, Htr10DataError> {
    load_htr10_nuclides_with_progress(cfg, layout, diag, |_| {})
}

/// One step of [`load_htr10_nuclides_with_progress`], for a caller that shows
/// progress (Dhoby Ghaut's workbench, gh:#568).
#[derive(Debug, Clone, PartialEq)]
pub enum LoadProgress {
    /// A thermal-scattering law or a nuclide is about to be processed.
    Started {
        /// What is being processed (`"graphite S(a,b)"`, `"U235"`).
        item: String,
    },
    /// It finished, after `seconds` of wall time.
    Finished {
        /// Same text as the matching [`LoadProgress::Started`].
        item: String,
        /// Wall-clock seconds.
        seconds: f64,
    },
}

/// [`load_htr10_nuclides`], calling `progress` before and after every
/// thermal law and every nuclide slot. The processing, its order and its
/// result are exactly those of [`load_htr10_nuclides`], which is this with a
/// no-op `progress`.
///
/// # Errors
///
/// As [`load_htr10_nuclides`].
pub fn load_htr10_nuclides_with_progress<F: FnMut(LoadProgress)>(
    cfg: &Htr10DataConfig,
    layout: &Htr10NuclideLayout,
    diag: &mut RunDiagnostics,
    mut progress: F,
) -> Result<Vec<Nuclide>, Htr10DataError> {
    if layout.loads_fe57() && !FE57_RECONSTRUCTION_FIXED {
        return Err(Htr10DataError::BlockedByGh339);
    }
    for n in &layout.notes {
        diag.note(n.clone());
        eprintln!("  {n}");
    }
    let t_k = cfg.temperature.get::<kelvin>();

    // Each distinct thermal law, loaded once.
    let mut laws: Vec<(ThermalLaw, ThermalScattering)> = Vec::new();
    for law in layout.slots.iter().filter_map(|s| s.thermal) {
        if laws.iter().any(|(l, _)| *l == law) {
            continue;
        }
        let item = law.label().to_string();
        progress(LoadProgress::Started { item: item.clone() });
        let t = Instant::now();
        let loaded = load_law(law, cfg, diag)?;
        progress(LoadProgress::Finished {
            item,
            seconds: t.elapsed().as_secs_f64(),
        });
        if let Some(sab) = loaded {
            laws.push((law, sab));
        }
    }

    let mut out = Vec::with_capacity(layout.len());
    for s in &layout.slots {
        let p = s.tape.path();
        if !p.exists() {
            return Err(Htr10DataError::MissingTape {
                name: s.name.into(),
                path: p,
            });
        }
        eprint!("  {:<6} ", s.name);
        progress(LoadProgress::Started {
            item: s.name.to_string(),
        });
        let t = Instant::now();
        let nuc = diag.time_data(
            format!("{} cross sections", s.name),
            DataSource::File(p.clone()),
            format!("{t_k:.2} K, tol 1.0e-3"),
            || Nuclide::from_endf_file(&p, s.name, t_k, 1.0e-3).ok(),
        );
        eprintln!("{:.1?}", t.elapsed());
        progress(LoadProgress::Finished {
            item: s.name.to_string(),
            seconds: t.elapsed().as_secs_f64(),
        });
        let nuc = nuc.ok_or_else(|| Htr10DataError::LoadFailed {
            name: s.name.into(),
            path: p,
        })?;
        let bound = s
            .thermal
            .and_then(|law| laws.iter().find(|(l, _)| *l == law));
        out.push(match bound {
            Some((_, sab)) => nuc.with_thermal_scattering(sab.clone()),
            None => nuc,
        });
    }
    Ok(out)
}

/// Load one thermal law. `Ok(None)` is the documented free-gas fallback for a
/// missing SiC tape.
fn load_law(
    law: ThermalLaw,
    cfg: &Htr10DataConfig,
    diag: &mut RunDiagnostics,
) -> Result<Option<ThermalScattering>, Htr10DataError> {
    let t_k = cfg.temperature.get::<kelvin>();
    let from_tape = |diag: &mut RunDiagnostics, path: PathBuf, mat: i32, name: &'static str| {
        diag.time_data(
            format!("{name} S(a,b)"),
            DataSource::File(path.clone()),
            format!("MAT {mat}, {t_k:.2} K"),
            || {
                ThermalScattering::from_endf_file(path.to_str()?, mat, t_k, name)
                    .map_err(|e| eprintln!("  {name} S(a,b) load FAILED (MAT {mat}): {e}"))
                    .ok()
            },
        )
    };
    match law {
        ThermalLaw::Graphite { file, mat } => {
            let p = DataDir::Endf.path().join(file);
            if !p.exists() {
                return Err(Htr10DataError::MissingTape {
                    name: "graphite S(a,b)".into(),
                    path: p,
                });
            }
            diag.note(format!("graphite S(a,b): {file} (MAT {mat})"));
            eprintln!("  graphite S(a,b): {file} (MAT {mat})");
            from_tape(diag, p.clone(), mat, "c_Graphite")
                .map(Some)
                .ok_or(Htr10DataError::LoadFailed {
                    name: "graphite S(a,b)".into(),
                    path: p,
                })
        }
        ThermalLaw::CInSiC | ThermalLaw::SiInSiC => {
            let (file, mat, name) = if law == ThermalLaw::CInSiC {
                ("tsl-CinSiC.endf", 44, "c_SiC")
            } else {
                ("tsl-SiinSiC.endf", 43, "Si_SiC")
            };
            let p = DataDir::Endf.path().join(file);
            if !p.exists() {
                eprintln!("  {name}: {file} not in this checkout -- falling back to free gas");
                diag.note(format!("{name} S(a,b): {file} absent -- FREE GAS fallback"));
                return Ok(None);
            }
            Ok(from_tape(diag, p, mat, name))
        }
        ThermalLaw::UInUO2 | ThermalLaw::OInUO2 => {
            let (material, name, file, mat) = if law == ThermalLaw::UInUO2 {
                (SabMaterial::UInUO2, "U_UO2", "tsl-UinUO2.endf", 76)
            } else {
                (SabMaterial::OInUO2, "O_UO2", "tsl-OinUO2.endf", 75)
            };
            match &cfg.uo2_laws {
                Uo2Laws::Tapes(dir) => {
                    let p = dir.join(file);
                    eprint!("  {name:<8} TAPE  ");
                    let t = Instant::now();
                    let r = from_tape(diag, p.clone(), mat, name);
                    eprintln!("{:.1?}", t.elapsed());
                    r.map(Some).ok_or(Htr10DataError::LoadFailed {
                        name: name.into(),
                        path: p,
                    })
                }
                Uo2Laws::GeneratedFromLeapr => {
                    eprint!("  {name:<8} LEAPR ");
                    let t = Instant::now();
                    let r = diag.time_data(
                        format!("{name} S(a,b)"),
                        DataSource::GeneratedFromLeaprDeck(material.base().to_string()),
                        format!("MAT {}, {t_k:.2} K, generated in-process", material.mat()),
                        || {
                            ThermalScattering::from_leapr(material, t_k, name)
                                .map_err(|e| eprintln!("  {name} LEAPR generation FAILED: {e}"))
                                .ok()
                        },
                    );
                    eprintln!("{:.1?}", t.elapsed());
                    r.map(Some).ok_or(Htr10DataError::LoadFailed {
                        name: name.into(),
                        path: PathBuf::from(material.base()),
                    })
                }
            }
        }
    }
}

/// # Verification: the material set built from each layout (no nuclear data)
///
/// **Methodology.** Plan a layout ([`Htr10NuclideLayout::plan`], pure), build
/// the full `mat::COUNT` material set from it with
/// [`super::materials::htr10_material_set`], and check the compositions the
/// 2026-10-01 decisions require against hand values:
/// - carbon: the natural-carbon layout carries, in every material, exactly the
///   carbon of the C-12-only ablation, split 0.9893 / 0.0107 (IUPAC);
/// - helium: `p / (k_B T)` at 300.15 K and 101.33 kPa = 2.4452e-5
///   atoms/(b cm), He-3 fraction 1.343e-6 (IUPAC); vacuum under the ablation;
/// - rod steel: the full case places each element's atoms on its own isotopes;
///   the simplified case moves Ni (and Fe-57) atoms onto Fe slots atom for atom,
///   conserving the total; neither the Ni nor the Fe-57 tape is loaded.
///
/// **Results (2026-10-01).** All pass; see each test.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::htr10_rmc::core_model::mat;
    use crate::htr10_rmc::materials::{
        abundance, atomic_weight, htr10_material_set, Htr10MaterialConfig,
        HE3_ATOM_FRACTION_OF_NATURAL_HE, ROD_JOINT_IRON_DENSITY, ROD_STEEL_DENSITY, ROD_STEEL_WT,
    };
    use outram_mc_libs::material::material::Material;
    use outram_mc_libs::pebble_beds::htr10::{
        C12_ATOM_FRACTION_OF_NATURAL_C, C13_ATOM_FRACTION_OF_NATURAL_C,
    };

    fn build(cfg: &Htr10DataConfig) -> (Htr10NuclideLayout, Vec<Material>) {
        let layout = Htr10NuclideLayout::plan(cfg).expect("valid configuration");
        let mats = htr10_material_set(&layout, Htr10MaterialConfig::benchmark_default(300.15));
        (layout, mats)
    }

    fn in_slot(m: &Material, idx: usize) -> f64 {
        m.components
            .iter()
            .filter(|c| c.nuclide_idx == idx)
            .map(|c| c.atom_density)
            .sum()
    }

    fn total(m: &Material) -> f64 {
        m.components.iter().map(|c| c.atom_density).sum()
    }

    fn close(got: f64, want: f64, rel: f64, what: &str) {
        assert!(
            (got - want).abs() <= rel * want.abs(),
            "{what}: got {got:.8e}, want {want:.8e}"
        );
    }

    /// Every component of every material names a slot of the layout (the
    /// no-rods ablation excepted, whose rod components are stripped by the
    /// driver).
    #[test]
    fn every_component_names_a_planned_slot() {
        for rod in [RodMetalTreatment::Full, RodMetalTreatment::Simplified] {
            for coolant in [Coolant::Helium, Coolant::Vacuum] {
                let (layout, mats) = build(&Htr10DataConfig {
                    rod_metal: rod,
                    coolant,
                    ..Default::default()
                });
                for m in &mats {
                    for c in &m.components {
                        assert!(
                            c.nuclide_idx < layout.len(),
                            "{}: slot {}",
                            m.name,
                            c.nuclide_idx
                        );
                    }
                }
            }
        }
    }

    /// **Carbon is conserved and split 0.9893 / 0.0107 in every material**
    /// (gh:#425). Result: 35 carbon-bearing materials (pebble layers,
    /// reflector zones, bricks, dummies, B4C, steel), each equal to the C-12-only
    /// ablation's carbon to 1e-12 relative, with C-13 at exactly 1.07 at.%.
    #[test]
    fn natural_carbon_is_conserved_and_split_in_every_material() {
        let (nat, nat_mats) = build(&Htr10DataConfig::default());
        let (c12, c12_mats) = build(&Htr10DataConfig {
            carbon: CarbonTreatment::AllC12,
            ..Default::default()
        });
        let kinds = |l: &Htr10NuclideLayout| [l.pebble.c_free, l.pebble.c_graphite, l.pebble.c_sic];
        let mut carbon_bearing = 0;
        for (m, m12) in nat_mats.iter().zip(&c12_mats) {
            let mut any = false;
            for (k, k12) in kinds(&nat).into_iter().zip(kinds(&c12)) {
                let want = k12.total_in(m12);
                let got = k.total_in(m);
                if want == 0.0 {
                    assert_eq!(got, 0.0, "{}", m.name);
                    continue;
                }
                any = true;
                close(got, want, 1e-12, &format!("{} carbon", m.name));
                let CarbonSlot::Natural { c12: i12, c13: i13 } = k else {
                    panic!("default VIII.0 layout must split carbon");
                };
                close(
                    in_slot(m, i12) / got,
                    C12_ATOM_FRACTION_OF_NATURAL_C,
                    1e-12,
                    "C-12 fraction",
                );
                close(
                    in_slot(m, i13) / got,
                    C13_ATOM_FRACTION_OF_NATURAL_C,
                    1e-12,
                    "C-13 fraction",
                );
            }
            carbon_bearing += usize::from(any);
        }
        println!("{carbon_bearing} carbon-bearing materials");
        // Every material but the kernel, the coolant and the joint iron (38 - 3):
        // 5 pebble layers, reflector, brick, bored band, dummies, 24 zones, B4C, steel.
        assert_eq!(carbon_bearing, 35, "carbon-bearing materials");
        // The rod steel's carbon is free gas, and is the TECDOC 0.1 wt.%.
        let steel = &nat_mats[mat::ROD_STEEL];
        let want = ROD_STEEL_DENSITY * 0.001 * 6.022_140_76e23 / atomic_weight::C * 1e-24;
        close(
            nat.pebble.c_free.total_in(steel),
            want,
            1e-12,
            "steel carbon",
        );
    }

    /// The VII.0 arm keeps ELEMENTAL carbon: one C-nat slot per kind, no
    /// C-13 slot and no placeholder, 11 pebble slots.
    #[test]
    fn the_endf7_arm_keeps_elemental_natural_carbon() {
        let (layout, _) = build(&Htr10DataConfig {
            library: NuclearDataLibrary::EndfB7,
            ..Default::default()
        });
        assert!(matches!(layout.pebble.c_graphite, CarbonSlot::Elemental(_)));
        assert_eq!(layout.pebble.slot_count(), 11);
        assert!(layout
            .slots
            .iter()
            .all(|s| s.tape.file != "n-006_C_013-ENDF8.0.endf"));
        let CarbonSlot::Elemental(i) = layout.pebble.c_graphite else {
            unreachable!()
        };
        assert_eq!(layout.slots[i].tape.file, "n-006_C_000-ENDF7.0.endf");
        // No SiC law on VII.0; helium taken from VIII.0, and said so.
        assert!(layout
            .slots
            .iter()
            .all(|s| s.thermal != Some(ThermalLaw::CInSiC)));
        assert!(layout
            .notes
            .iter()
            .any(|n| n.contains("helium is ENDF/B-VIII.0")));
        // The VIII.0-only ablations are refused on VII.0.
        assert!(Htr10NuclideLayout::plan(&Htr10DataConfig {
            library: NuclearDataLibrary::EndfB7,
            carbon: CarbonTreatment::AllC12,
            ..Default::default()
        })
        .is_err());
    }

    /// **Helium: 2.4452e-5 atoms/(b cm) at 300.15 K and 101.33 kPa, He-3 at
    /// 1.343e-6** (gh:#426). Result: 2.44521e-5 (hand: 101330 / (1.380649e-23
    /// x 300.15) x 1e-30), He-3 fraction exact. Vacuum ablation: empty.
    #[test]
    fn helium_coolant_is_natural_helium_at_one_atmosphere() {
        let (layout, mats) = build(&Htr10DataConfig::default());
        let CoolantNuclides::Helium { he3, he4 } = layout.coolant else {
            panic!("helium is the default coolant");
        };
        let he = &mats[mat::HELIUM];
        let n = total(he);
        println!("helium {n:.6e} atoms/b-cm, He-3 {:.4e}", in_slot(he, he3));
        close(n, 2.445_21e-5, 1e-5, "helium atom density");
        close(
            in_slot(he, he3) / n,
            HE3_ATOM_FRACTION_OF_NATURAL_HE,
            1e-12,
            "He-3 fraction",
        );
        close(
            in_slot(he, he4) / n,
            1.0 - HE3_ATOM_FRACTION_OF_NATURAL_HE,
            1e-12,
            "He-4 fraction",
        );
        assert_eq!(layout.slots[he3].tape.file, "n-002_He_003-ENDF8.0.endf");
        assert_eq!(layout.slots[he4].tape.file, "n-002_He_004-ENDF8.0.endf");
        assert!(
            layout
                .slots
                .iter()
                .all(|s| !s.tape.file.starts_with("a-002")),
            "alpha sublibrary"
        );

        let (vac, vmats) = build(&Htr10DataConfig {
            coolant: Coolant::Vacuum,
            ..Default::default()
        });
        assert_eq!(vac.coolant, CoolantNuclides::Vacuum);
        assert!(vmats[mat::HELIUM].components.is_empty());
        assert!(vac.slots.iter().all(|s| !s.name.starts_with("He")));
    }

    /// **Rod steel, FULL case:** each element's atoms on its own natural
    /// isotopes, Ni from the ACE submodule. Result: Fe, Cr, Ni, Ti, Si, Mn
    /// atoms equal `rho w N_A / M` to 1e-12; the joint iron is 0.04 /b-cm.
    #[test]
    fn full_rod_metal_places_real_nickel_and_iron() {
        let (layout, mats) = build(&Htr10DataConfig::default());
        let m = layout.metal;
        let steel = &mats[mat::ROD_STEEL];
        let wt = |e: &str| ROD_STEEL_WT.iter().find(|(s, _)| *s == e).unwrap().1;
        let n_elem = |e: &str, a: f64| ROD_STEEL_DENSITY * wt(e) * 6.022_140_76e23 / a * 1e-24;
        let fe = [m.fe54, m.fe56, m.fe57, m.fe58];
        let ni = [m.ni58, m.ni60, m.ni61, m.ni62, m.ni64];
        let sum = |idx: &[usize]| idx.iter().map(|&i| in_slot(steel, i)).sum::<f64>();
        close(sum(&fe), n_elem("Fe", atomic_weight::FE), 1e-12, "steel Fe");
        close(sum(&ni), n_elem("Ni", atomic_weight::NI), 1e-12, "steel Ni");
        for (&i, f) in ni.iter().zip(abundance::NI) {
            close(
                in_slot(steel, i),
                n_elem("Ni", atomic_weight::NI) * f,
                1e-12,
                "Ni isotope",
            );
        }
        // Every Ni slot is a Ni tape in the ACE submodule; Fe-57 is real.
        for &i in &ni {
            assert!(layout.slots[i].name.starts_with("Ni"));
            assert_eq!(layout.slots[i].tape.dir, DataDir::AceSubmoduleEndfB8);
        }
        assert_eq!(layout.slots[m.fe57].name, "Fe57");
        assert!(layout.loads_fe57());
        let iron = &mats[mat::ROD_IRON];
        close(total(iron), ROD_JOINT_IRON_DENSITY, 1e-12, "joint iron");
        close(
            in_slot(iron, m.fe57),
            ROD_JOINT_IRON_DENSITY * abundance::FE[2],
            1e-12,
            "joint Fe-57",
        );
    }

    /// **Rod steel, SIMPLIFIED case:** Ni -> Fe atom for atom and Fe-57 ->
    /// Fe-56. Result: steel and joint-iron atom totals equal the full case to
    /// 1e-12; the Fe-56 slot carries Fe-56 + Fe-57 + Ni-58 + Ni-60 + Ni-61;
    /// neither Ni nor Fe-57 is loaded.
    #[test]
    fn simplified_rod_metal_maps_atom_for_atom() {
        let (full, fmats) = build(&Htr10DataConfig::default());
        let (simp, smats) = build(&Htr10DataConfig {
            rod_metal: RodMetalTreatment::Simplified,
            ..Default::default()
        });
        for k in [mat::ROD_STEEL, mat::ROD_IRON] {
            close(total(&smats[k]), total(&fmats[k]), 1e-12, &smats[k].name);
        }
        assert!(!simp.loads_fe57());
        assert!(simp.slots.iter().all(|s| !s.name.starts_with("Ni")));
        let (f, s) = (full.metal, simp.metal);
        assert_eq!(s.ni58, s.fe56);
        assert_eq!(s.ni60, s.fe56);
        assert_eq!(s.ni61, s.fe56); // Ni-61 -> Fe-57 -> Fe-56
        assert_eq!(s.ni62, s.fe54);
        assert_eq!(s.ni64, s.fe58);
        assert_eq!(s.fe57, s.fe56);
        let (fs, ss) = (&fmats[mat::ROD_STEEL], &smats[mat::ROD_STEEL]);
        let moved = [f.fe56, f.fe57, f.ni58, f.ni60, f.ni61]
            .iter()
            .map(|&i| in_slot(fs, i))
            .sum::<f64>();
        close(in_slot(ss, s.fe56), moved, 1e-12, "simplified Fe-56 slot");
        close(
            in_slot(ss, s.fe54),
            in_slot(fs, f.fe54) + in_slot(fs, f.ni62),
            1e-12,
            "simplified Fe-54 slot",
        );
        // Five Ni tapes and Fe-57 dropped: 6 fewer slots.
        assert_eq!(full.len() - simp.len(), 6);
    }

    /// The thermal laws bind to BOTH carbon isotopes, as OpenMC does; free gas
    /// only under the NO_SAB ablation.
    #[test]
    fn thermal_laws_bind_to_both_carbon_isotopes() {
        let (l, _) = build(&Htr10DataConfig::default());
        let law = ThermalLaw::Graphite {
            file: GraphiteLaw::default().tape(),
            mat: GraphiteLaw::default().mat(),
        };
        for i in l.pebble.c_graphite.slots() {
            assert_eq!(l.slots[i].thermal, Some(law), "{}", l.slots[i].label());
        }
        for i in l.pebble.c_sic.slots() {
            assert_eq!(l.slots[i].thermal, Some(ThermalLaw::CInSiC));
        }
        for i in l.pebble.c_free.slots() {
            assert_eq!(l.slots[i].thermal, None);
        }
        let (f, _) = build(&Htr10DataConfig {
            thermal: ThermalScatteringTreatment::FreeGas,
            ..Default::default()
        });
        assert!(f.slots.iter().all(|s| s.thermal.is_none()));
    }

    /// The full case is REFUSED, before any tape is read, while gh:#339 is
    /// open; it is never silently swapped for Fe-56.
    #[test]
    fn the_full_case_is_refused_until_gh339_is_fixed() {
        if FE57_RECONSTRUCTION_FIXED {
            return;
        }
        let cfg = Htr10DataConfig::default();
        let layout = Htr10NuclideLayout::plan(&cfg).unwrap();
        let mut diag = RunDiagnostics::new("htr10-data-gate-test");
        let r = load_htr10_nuclides(&cfg, &layout, &mut diag);
        assert!(matches!(r, Err(Htr10DataError::BlockedByGh339)));
        assert_eq!(
            diag.data_item_count(),
            0,
            "nothing may be read before refusing"
        );
    }
}
