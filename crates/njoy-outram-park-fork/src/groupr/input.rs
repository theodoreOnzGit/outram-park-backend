// Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! GROUPR user-input **card deck** model and free-format reader.
//!
//! This module is a *faithful, line-traceable* translation of the input-card
//! reading of NJOY2016's `groupr`/`ruinb` (`groupr.f90:1044-1130`) plus the
//! conditional group-structure, weight, and reaction cards read elsewhere in the
//! `groupr` driver. It models the free-format card sequence that configures a
//! GROUPR run — unit numbers, material, group/weight selectors, temperatures,
//! background cross sections, group grids, weight parameters, and the reaction
//! list — as a documented Rust struct ([`GrouprInput`]) plus named selector
//! enums. Field names keep the upstream Fortran identifiers in their doc
//! comments so a discrepancy localises to a specific `read(nsysi,*)` statement.
//!
//! # Card summary (from `groupr.f90:126-215`)
//!
//! | Card | Fortran vars | Condition |
//! |------|--------------|-----------|
//! | 1 | `nendf npend ngout1 ngout2` | always |
//! | 2 | `matb ign igg iwt lord ntemp nsigz iprint ismooth` | always |
//! | 3 | `title` | always (run label) |
//! | 4 | `temp(ntemp)` | always (kelvin) |
//! | 5 | `sigz(nsigz)` | always (barns; `sigz(1)` forced to infinity) |
//! | 6a/6b | `ngn`, `egn(ngn+1)` | `ign == 1` only |
//! | 7a/7b | `ngg`, `egg(ngg+1)` | `igg == 1` only |
//! | 8a | flux-calc params | `iwt < 0` only |
//! | 8b | `wght` (TAB1) | `iwt == 1` or `-1` only |
//! | 8c | `eb tb ec tc` | `iwt == 4` or `-4` only |
//! | 8d | `ninwt` | `iwt == 0` only |
//! | 9 | `mfd mtd mtname` (repeat; `mfd == 0` ends) | always |
//! | 9a | `file section zaid m` | `mfd == -1` only |
//! | 10 | next `matb` (`0` ends the run) | always |
//!
//! # Neutron group structure
//!
//! The neutron group-structure tables (`gengpn`, selected by `ign`) are **not**
//! duplicated here — they live in [`crate::errorr::groups`]. This module maps
//! `ign` to the shared [`NeutronGroupStructure`] via [`neutron_group_from_ign`],
//! forbidding `ign == -1` exactly as GROUPR does (`groupr.f90:1123-1125`).
//! Photon structures use [`crate::groupr::PhotonGroupStructure`].
//!
//! # What is *not* here
//!
//! The numeric group-averaging engine (feed functions, flux integration, GENDF
//! writer) is not modelled — see [`crate::groupr::run`], which returns
//! [`NjoyError::NotPorted`]. The [`GrouprInput::parse`] reader covers the
//! numeric cards and the reaction list; it documents its handling of the
//! conditional cards 6/7/8 (see that method).

use crate::errorr::groups::NeutronGroupStructure;
use crate::groupr::weights::ThermalFissionParams;
use crate::groupr::PhotonGroupStructure;
use crate::NjoyError;

// ---------------------------------------------------------------------------
// Card 1 — unit numbers (groupr.f90:130-133, read at :1061)
// ---------------------------------------------------------------------------

/// Card 1: logical unit numbers for the ENDF/PENDF/GENDF tapes GROUPR uses.
///
/// Fortran: `read(nsysi,*) nendf,npend,ngout1,ngout2` (`groupr.f90:1061`). A
/// negative unit denotes a binary (unformatted) tape; positive is formatted
/// (BCD/ASCII); zero means "absent". These are transport-layer handles, not
/// physical quantities — no units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitAssignments {
    /// `nendf` — input ENDF tape (evaluation source). Required (nonzero).
    pub nendf: i32,
    /// `npend` — input PENDF tape (pointwise, temperature-broadened cross
    /// sections used for the flux-weighted averaging). Required for a real run.
    pub npend: i32,
    /// `ngout1` — input group (GENDF) tape for add/replace mode. Default 0.
    pub ngout1: i32,
    /// `ngout2` — output group (GENDF) tape. Default 0 (no output written).
    pub ngout2: i32,
}

impl Default for UnitAssignments {
    /// GROUPR defaults (`groupr.f90:1058-1059`): `ngout1 = ngout2 = 0`;
    /// `nendf`/`npend` are caller-supplied (here default 0, must be set).
    fn default() -> Self {
        Self {
            nendf: 0,
            npend: 0,
            ngout1: 0,
            ngout2: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Card 2 selectors (groupr.f90:135-149, read at :1070)
// ---------------------------------------------------------------------------

/// Long-print option (`iprint`), card 2 (`groupr.f90:146-147`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintOption {
    /// `0`: minimum print.
    Minimum,
    /// `1`: maximum (long) print. GROUPR default.
    Maximum,
}

impl PrintOption {
    /// The integer `iprint` value.
    pub fn iprint(self) -> i32 {
        match self {
            Self::Minimum => 0,
            Self::Maximum => 1,
        }
    }
    /// Construct from `iprint` (`0`/`1`), else `None`.
    pub fn from_iprint(iprint: i32) -> Option<Self> {
        match iprint {
            0 => Some(Self::Minimum),
            1 => Some(Self::Maximum),
            _ => None,
        }
    }
}

/// Low-energy smoothing switch (`ismooth`), card 2 (`groupr.f90:148-149`).
///
/// When on, GROUPR applies `sqrt(E)` smoothing to MF6 CM emission spectra and
/// histogram delayed-neutron spectra at low energies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothingOption {
    /// `0`: smoothing off.
    Off,
    /// `1`: smoothing on. GROUPR default.
    On,
}

impl SmoothingOption {
    /// The integer `ismooth` value.
    pub fn ismooth(self) -> i32 {
        match self {
            Self::Off => 0,
            Self::On => 1,
        }
    }
    /// Construct from `ismooth` (`0`/`1`), else `None` (NJOY aborts on illegal
    /// `ismooth`, `groupr.f90:1093-1095`).
    pub fn from_ismooth(ismooth: i32) -> Option<Self> {
        match ismooth {
            0 => Some(Self::Off),
            1 => Some(Self::On),
            _ => None,
        }
    }
}

/// Map a GROUPR `ign` index to the shared [`NeutronGroupStructure`].
///
/// GROUPR shares the neutron group-structure tables with ERRORR (`gengpn`), so
/// this adapts `ign` (`1..=36`) to the enum defined in [`crate::errorr::groups`]
/// rather than re-listing the 36 boundary tables. GROUPR explicitly forbids
/// `ign == -1` (that value is ERRORR-only, `groupr.f90:1123-1125`), so `-1` and
/// any other out-of-range index return [`NjoyError::EndfParse`].
///
/// `ign == 1` maps to [`NeutronGroupStructure::Arbitrary`] (boundaries read from
/// card 6). All larger indices name a built-in structure.
pub fn neutron_group_from_ign(ign: i32) -> Result<NeutronGroupStructure, NjoyError> {
    use NeutronGroupStructure as N;
    Ok(match ign {
        1 => N::Arbitrary,
        2 => N::Csewg239,
        3 => N::Lanl30,
        4 => N::Anl27,
        5 => N::Rrd50,
        6 => N::GamI68,
        7 => N::GamII100,
        8 => N::LaserThermos35,
        9 => N::EpriCpm69,
        10 => N::Lanl187,
        11 => N::Lanl70,
        12 => N::SandII620,
        13 => N::Lanl80,
        14 => N::Eurlib100,
        15 => N::SandIIA640,
        16 => N::VitaminE174,
        17 => N::VitaminJ175,
        18 => N::Xmas172,
        19 => N::Ecco33,
        20 => N::Ecco1968,
        21 => N::Tripoli315,
        22 => N::XmasLwpc172,
        23 => N::VitJLwpc175,
        24 => N::ShemCea281,
        25 => N::ShemEpm295,
        26 => N::ShemCeaEpm361,
        27 => N::ShemEpm315,
        28 => N::RahabAecl89,
        29 => N::Ccfe660,
        30 => N::Ukaea1025,
        31 => N::Ukaea1067,
        32 => N::Ukaea1102,
        33 => N::Ukaea142,
        34 => N::Lanl618,
        35 => N::Apollo99,
        36 => N::Ecco1962,
        _ => {
            return Err(NjoyError::EndfParse(format!(
                "illegal neutron group structure ign={ign} \
                 (ign=-1 is errorr-only; gengpn, groupr.f90:1123)"
            )))
        }
    })
}

// ---------------------------------------------------------------------------
// Weight-function selection (iwt; groupr.f90:167-183, genwtf at :4865)
// ---------------------------------------------------------------------------

/// Built-in weight-function option (`|iwt|`), card 2 (`groupr.f90:167-181`).
///
/// This is the *magnitude* of `iwt`; the sign/zero determines the weighting
/// *mode* ([`WeightSelection`]). See [`crate::groupr::weights`] for which of
/// these are analytic vs tabulated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightOption {
    /// `1`: read in a smooth (TAB1) weight function (card 8b).
    ReadIn,
    /// `2`: constant.
    Constant,
    /// `3`: `1/E`.
    OneOverE,
    /// `4`: `1/E` + fission spectrum + thermal Maxwellian (card 8c params).
    ThermalOneOverEFission,
    /// `5`: EPRI-CELL LWR (built-in tabulated).
    EpriCellLwr,
    /// `6`: (thermal) -- (1/E) -- (fission + fusion).
    ThermalOneOverEFissionFusion,
    /// `7`: same as 6, temperature-dependent thermal part.
    ThermalOneOverEFissionFusionTdep,
    /// `8`: thermal -- 1/E -- fast reactor -- fission + fusion (tabulated).
    ThermalOneOverEFastFissionFusion,
    /// `9`: CLAW weight function (built-in tabulated).
    Claw,
    /// `10`: CLAW with temperature-dependent thermal part (hybrid).
    ClawTdep,
    /// `11`: VITAMIN-E weight (ORNL-5505).
    VitaminE,
    /// `12`: VITAMIN-E with temperature-dependent thermal part.
    VitaminETdep,
}

impl WeightOption {
    /// The integer `|iwt|` value (`1..=12`).
    pub fn iwt(self) -> i32 {
        match self {
            Self::ReadIn => 1,
            Self::Constant => 2,
            Self::OneOverE => 3,
            Self::ThermalOneOverEFission => 4,
            Self::EpriCellLwr => 5,
            Self::ThermalOneOverEFissionFusion => 6,
            Self::ThermalOneOverEFissionFusionTdep => 7,
            Self::ThermalOneOverEFastFissionFusion => 8,
            Self::Claw => 9,
            Self::ClawTdep => 10,
            Self::VitaminE => 11,
            Self::VitaminETdep => 12,
        }
    }
    /// Construct from `|iwt|` (`1..=12`), else `None`.
    pub fn from_iwt(iwt: i32) -> Option<Self> {
        Some(match iwt {
            1 => Self::ReadIn,
            2 => Self::Constant,
            3 => Self::OneOverE,
            4 => Self::ThermalOneOverEFission,
            5 => Self::EpriCellLwr,
            6 => Self::ThermalOneOverEFissionFusion,
            7 => Self::ThermalOneOverEFissionFusionTdep,
            8 => Self::ThermalOneOverEFastFissionFusion,
            9 => Self::Claw,
            10 => Self::ClawTdep,
            11 => Self::VitaminE,
            12 => Self::VitaminETdep,
            _ => return None,
        })
    }

    /// Whether this weight reads a TAB1 record from card 8b (`iwt == 1`).
    pub fn reads_tab1(self) -> bool {
        matches!(self, Self::ReadIn)
    }
    /// Whether this weight reads analytic parameters from card 8c (`iwt == 4`).
    pub fn reads_analytic_params(self) -> bool {
        matches!(self, Self::ThermalOneOverEFission)
    }
    /// Whether an analytic (closed-form) evaluator exists in
    /// [`crate::groupr::weights`] for this option (`2,3,4,6,7,11,12`).
    pub fn is_analytic(self) -> bool {
        matches!(
            self,
            Self::Constant
                | Self::OneOverE
                | Self::ThermalOneOverEFission
                | Self::ThermalOneOverEFissionFusion
                | Self::ThermalOneOverEFissionFusionTdep
                | Self::VitaminE
                | Self::VitaminETdep
        )
    }
}

/// Flux-calculator parameters (card 8a), read only when `iwt < 0`
/// (`groupr.f90:172-187`, read at `:4986`).
///
/// These configure GROUPR's optional infinite-medium flux calculation (heavy
/// absorber + light moderator) that replaces the analytic Bondarenko weight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FluxCalcParams {
    /// `fehi` — break between the computed flux and the Bondarenko flux (eV);
    /// must lie in the resolved-resonance range.
    pub fehi_ev: f64,
    /// `sigpot` — estimate of the potential scattering cross section (barns).
    pub sigpot_barns: f64,
    /// `nflmax` — maximum number of computed flux points.
    pub nflmax: i32,
    /// `ninwt` — tape unit for the new flux parameters (written binary).
    pub ninwt: i32,
    /// `jsigz` — index of the reference sigma-zero in the `sigz` array.
    pub jsigz: i32,
    /// `alpha2` — alpha for the admixed moderator (dimensionless; 0 = none).
    pub alpha2: f64,
    /// `sam` — admixed-moderator cross section, barns per absorber atom.
    pub sam_barns: f64,
    /// `beta` — heterogeneity parameter (dimensionless; 0 = none).
    pub beta: f64,
    /// `alpha3` — alpha for the external moderator (dimensionless; 0 = none).
    pub alpha3: f64,
    /// `gamma` — fraction of admixed-moderator cross section in the external
    /// moderator cross section (dimensionless; 0 = none).
    pub gamma: f64,
}

/// The weighting *mode* implied by the sign/zero of `iwt` (`groupr.f90:181-183`).
#[derive(Debug, Clone, PartialEq)]
pub enum WeightSelection {
    /// `iwt > 0`: use the built-in/read weight directly (Bondarenko weighting).
    Bondarenko(WeightOption),
    /// `iwt < 0`: compute the flux with weight `|iwt|`, using card-8a params.
    ComputeFlux {
        /// The base weight `|iwt|`.
        weight: WeightOption,
        /// Card 8a flux-calculator parameters. `None` until card 8a is read.
        params: Option<FluxCalcParams>,
    },
    /// `iwt == 0`: read the resonance flux from tape unit `ninwt` (card 8d).
    ReadResonanceFlux {
        /// `ninwt` — tape unit holding the binary resonance-flux parameters.
        ninwt: i32,
    },
}

impl WeightSelection {
    /// The signed integer `iwt` this selection corresponds to.
    pub fn iwt(&self) -> i32 {
        match self {
            Self::Bondarenko(w) => w.iwt(),
            Self::ComputeFlux { weight, .. } => -weight.iwt(),
            Self::ReadResonanceFlux { .. } => 0,
        }
    }
    /// Construct from a signed `iwt`. Returns `None` for a nonzero magnitude
    /// outside `1..=12`.
    pub fn from_iwt(iwt: i32) -> Option<Self> {
        if iwt == 0 {
            return Some(Self::ReadResonanceFlux { ninwt: 0 });
        }
        let w = WeightOption::from_iwt(iwt.abs())?;
        Some(if iwt > 0 {
            Self::Bondarenko(w)
        } else {
            Self::ComputeFlux {
                weight: w,
                params: None,
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Card 9 — reaction requests (groupr.f90:200-215, read at :628)
// ---------------------------------------------------------------------------

/// One card-9 reaction request: `read(nsysi,*) mfd, mtd, mtname`
/// (`groupr.f90:628`). Terminated in the deck by a request with `mfd == 0`.
#[derive(Debug, Clone, PartialEq)]
pub struct ReactionRequest {
    /// `mfd` — the "file" (reaction *class*) to process (e.g. 3 = vector cross
    /// section/yield, 6/8 = neutron-neutron matrix, 16/17/18 = neutron-gamma
    /// matrix, 21-26/31-36 = charged-particle/residual production). Special
    /// large values (`1zzzaaam`..`4zzzaaam`) request nuclide production. Not a
    /// physical unit — an ENDF file selector. See `groupr.f90:283-343`.
    pub mfd: i32,
    /// `mtd` — the section (reaction MT) to process. `-n` means "all MTs from
    /// the previous entry through `n` inclusive"; 221-250 are thermal
    /// scattering; 257/258/259 are average energy/lethargy/inverse velocity
    /// (`groupr.f90:345-357`).
    pub mtd: i32,
    /// `mtname` — free-text description of the section (up to 60 chars).
    pub name: String,
}

// ---------------------------------------------------------------------------
// The full input deck for one GROUPR material job.
// ---------------------------------------------------------------------------

/// Named alias for a temperature in **kelvin** (card 4 `temp` value).
pub type TemperatureKelvin = f64;
/// Named alias for a background (sigma-zero) cross section in **barns**
/// (card 5 `sigz` value).
pub type SigmaZeroBarns = f64;
/// Named alias for a group-boundary energy in **electron-volts (eV)**.
pub type BoundaryEnergyEv = f64;

/// A complete GROUPR input deck for one material-processing job (cards 1-9)
/// plus the pointer to the next material (card 10).
///
/// This mirrors the state `ruinb` and the `groupr` driver build from the free-
/// format card reads. Construct it by hand or via [`GrouprInput::parse`].
#[derive(Debug, Clone, PartialEq)]
pub struct GrouprInput {
    /// Card 1 — tape unit numbers.
    pub units: UnitAssignments,
    /// Card 2 `matb` — material (MAT) to process. `< 0` triggers add/replace or
    /// auto-process-all modes (`groupr.f90:136-140, 1071-1073`).
    pub matb: i32,
    /// Card 2 `ign` — neutron group-structure selector (`1..=36`; `1` = read
    /// card 6). Mapped to [`NeutronGroupStructure`] via
    /// [`neutron_group_from_ign`].
    pub neutron_groups: NeutronGroupStructure,
    /// Card 2 `igg` — photon group-structure selector (`0..=10`; `1` = read
    /// card 7).
    pub photon_groups: PhotonGroupStructure,
    /// Card 2 `iwt` — weighting-function selection (mode + option).
    pub weight: WeightSelection,
    /// Card 2 `lord` — Legendre order of the scattering matrices.
    pub lord: i32,
    /// Card 2 `iprint` — long-print option.
    pub iprint: PrintOption,
    /// Card 2 `ismooth` — low-energy smoothing switch.
    pub ismooth: SmoothingOption,
    /// Card 3 `title` — run label.
    pub title: String,
    /// Card 4 `temp(ntemp)` — temperatures in kelvin (`ntemp = temps.len()`).
    pub temperatures: Vec<TemperatureKelvin>,
    /// Card 5 `sigz(nsigz)` — background cross sections in barns. NJOY forces
    /// `sigz(1)` to infinity (`sigzmx = 1e10`); this vector stores the values
    /// as read (the first is conventionally the largest / "infinity").
    pub sigma_zeros: Vec<SigmaZeroBarns>,
    /// Card 6 `egn` — read-in neutron group boundaries (eV), present only when
    /// `ign == 1` ([`NeutronGroupStructure::Arbitrary`]).
    pub neutron_grid: Option<Vec<BoundaryEnergyEv>>,
    /// Card 7 `egg` — read-in photon group boundaries (eV), present only when
    /// `igg == 1` ([`PhotonGroupStructure::Arbitrary`]).
    pub photon_grid: Option<Vec<BoundaryEnergyEv>>,
    /// Card 8c `eb,tb,ec,tc` — analytic thermal/fission weight parameters,
    /// present only when `|iwt| == 4`.
    pub thermal_fission_params: Option<ThermalFissionParams>,
    /// Card 9 — the reaction requests (the terminating `mfd == 0` is not
    /// stored).
    pub reactions: Vec<ReactionRequest>,
    /// Card 10 — the next material to process (`0` ends the GROUPR run). `None`
    /// if the deck did not include a card-10 line.
    pub next_material: Option<i32>,
}

impl Default for GrouprInput {
    /// A minimal deck mirroring GROUPR's initialised card values
    /// (`groupr.f90:1064-1067`): `ign` unset (defaults to the read-in arbitrary
    /// structure), `igg = 0` (no photon groups), `iwt = 6`
    /// ((thermal)--(1/E)--(fission+fusion)), `lord = 3`, one temperature 300 K,
    /// one sigma-zero (infinity), maximum print, smoothing on.
    fn default() -> Self {
        Self {
            units: UnitAssignments::default(),
            matb: 0,
            neutron_groups: NeutronGroupStructure::Arbitrary,
            photon_groups: PhotonGroupStructure::None,
            weight: WeightSelection::Bondarenko(WeightOption::ThermalOneOverEFissionFusion),
            lord: 3,
            iprint: PrintOption::Maximum,
            ismooth: SmoothingOption::On,
            title: String::new(),
            temperatures: vec![300.0],
            sigma_zeros: vec![1.0e10],
            neutron_grid: None,
            photon_grid: None,
            thermal_fission_params: None,
            reactions: Vec::new(),
            next_material: None,
        }
    }
}

impl GrouprInput {
    /// The number of temperatures (`ntemp`).
    pub fn ntemp(&self) -> usize {
        self.temperatures.len()
    }
    /// The number of background cross sections (`nsigz`).
    pub fn nsigz(&self) -> usize {
        self.sigma_zeros.len()
    }

    /// Parse a GROUPR free-format input deck (numeric cards + reaction list).
    ///
    /// # Scope and simplifications
    ///
    /// NJOY's reader is a token stream that ignores line breaks; real decks,
    /// however, place one card per line. This parser therefore reads
    /// **one card per line** in the card order of `groupr.f90:126-215`:
    ///
    /// - card 1 (4 units), card 2 (up to 9 selectors, trailing ones defaulted),
    ///   card 3 (title line, surrounding quotes and a trailing `/` stripped),
    ///   card 4 (`ntemp` temperatures), card 5 (`nsigz` sigma-zeros);
    /// - card 6 (`ngn` then boundaries) iff `ign == 1`;
    /// - card 7 (`ngg` then boundaries) iff `igg == 1`;
    /// - card 8c (`eb tb ec tc`) iff `|iwt| == 4`;
    /// - card 9 reaction lines (`mfd mtd 'name'`) until `mfd == 0`;
    /// - card 10 next material (optional).
    ///
    /// **Not handled** (returns [`NjoyError::NotPorted`]): the flux-calculator
    /// card 8a (`iwt < 0`), the TAB1 weight card 8b (`iwt == 1`), the
    /// resonance-flux card 8d (`iwt == 0`), and the card-9a extended residual
    /// format (`mfd == -1`). These need the flux/TAB1 machinery that is not
    /// ported. Blank lines and `//`-prefixed comment lines are skipped.
    ///
    /// Card 2 selectors are validated: an illegal `ign`, `igg`, `iwt`,
    /// `iprint`, or `ismooth` yields the same rejection GROUPR would abort on.
    pub fn parse(input: &str) -> Result<Self, NjoyError> {
        let mut lines = input
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with("//"));

        // --- card 1 ---
        let c1 = next_line(&mut lines, "card 1 (units)")?;
        let u = parse_ints(c1);
        let units = UnitAssignments {
            nendf: *u.first().ok_or_else(|| card_err("card 1: nendf"))?,
            npend: *u.get(1).ok_or_else(|| card_err("card 1: npend"))?,
            ngout1: u.get(2).copied().unwrap_or(0),
            ngout2: u.get(3).copied().unwrap_or(0),
        };

        // --- card 2 --- matb ign igg iwt lord ntemp nsigz iprint ismooth
        let c2 = next_line(&mut lines, "card 2 (selectors)")?;
        let s = parse_ints(c2);
        let matb = *s.first().ok_or_else(|| card_err("card 2: matb"))?;
        let ign = *s.get(1).ok_or_else(|| card_err("card 2: ign"))?;
        let igg = s.get(2).copied().unwrap_or(0);
        let iwt = s.get(3).copied().unwrap_or(6);
        let lord = s.get(4).copied().unwrap_or(3);
        let ntemp = s.get(5).copied().unwrap_or(1).max(1) as usize;
        let nsigz = s.get(6).copied().unwrap_or(1).max(1) as usize;
        let iprint = s.get(7).copied().unwrap_or(1);
        let ismooth = s.get(8).copied().unwrap_or(1);

        let neutron_groups = neutron_group_from_ign(ign)?;
        let photon_groups =
            PhotonGroupStructure::from_igg(igg).ok_or_else(|| card_err("card 2: illegal igg"))?;
        let weight =
            WeightSelection::from_iwt(iwt).ok_or_else(|| card_err("card 2: illegal iwt"))?;
        let iprint =
            PrintOption::from_iprint(iprint).ok_or_else(|| card_err("card 2: illegal iprint"))?;
        let ismooth = SmoothingOption::from_ismooth(ismooth)
            .ok_or_else(|| card_err("card 2: illegal ismooth"))?;

        // --- card 3: title ---
        let title = strip_title(next_line(&mut lines, "card 3 (title)")?);

        // --- card 4: temperatures ---
        let c4 = next_line(&mut lines, "card 4 (temperatures)")?;
        let temperatures: Vec<f64> = parse_floats(c4).into_iter().take(ntemp).collect();
        if temperatures.len() != ntemp {
            return Err(card_err("card 4: fewer temperatures than ntemp"));
        }

        // --- card 5: sigma zeros ---
        let c5 = next_line(&mut lines, "card 5 (sigma zeros)")?;
        let sigma_zeros: Vec<f64> = parse_floats(c5).into_iter().take(nsigz).collect();
        if sigma_zeros.len() != nsigz {
            return Err(card_err("card 5: fewer sigma-zeros than nsigz"));
        }

        // --- card 6: neutron grid (ign == 1) ---
        let neutron_grid = if ign == 1 {
            let ngn = *parse_ints(next_line(&mut lines, "card 6a (ngn)")?)
                .first()
                .ok_or_else(|| card_err("card 6a: ngn"))? as usize;
            let egn: Vec<f64> = parse_floats(next_line(&mut lines, "card 6b (egn)")?)
                .into_iter()
                .take(ngn + 1)
                .collect();
            if egn.len() != ngn + 1 {
                return Err(card_err("card 6b: fewer boundaries than ngn+1"));
            }
            Some(egn)
        } else {
            None
        };

        // --- card 7: photon grid (igg == 1) ---
        let photon_grid = if igg == 1 {
            let ngg = *parse_ints(next_line(&mut lines, "card 7a (ngg)")?)
                .first()
                .ok_or_else(|| card_err("card 7a: ngg"))? as usize;
            let egg: Vec<f64> = parse_floats(next_line(&mut lines, "card 7b (egg)")?)
                .into_iter()
                .take(ngg + 1)
                .collect();
            if egg.len() != ngg + 1 {
                return Err(card_err("card 7b: fewer boundaries than ngg+1"));
            }
            Some(egg)
        } else {
            None
        };

        // --- card 8: weight data ---
        let thermal_fission_params = match iwt.abs() {
            4 => {
                let v = parse_floats(next_line(&mut lines, "card 8c (eb tb ec tc)")?);
                if v.len() < 4 {
                    return Err(card_err("card 8c: need eb tb ec tc"));
                }
                Some(ThermalFissionParams {
                    thermal_break_ev: v[0],
                    thermal_temp_ev: v[1],
                    fission_break_ev: v[2],
                    fission_temp_ev: v[3],
                })
            }
            0 => return Err(NjoyError::NotPorted("groupr::parse::card8d_resonance_flux")),
            1 => return Err(NjoyError::NotPorted("groupr::parse::card8b_tab1_weight")),
            _ => None,
        };
        if iwt < 0 {
            return Err(NjoyError::NotPorted("groupr::parse::card8a_flux_calc"));
        }

        // --- card 9: reactions (until mfd == 0) ---
        let mut reactions = Vec::new();
        for line in lines.by_ref() {
            let (nums, name) = split_reaction_line(line);
            let mfd = *nums.first().ok_or_else(|| card_err("card 9: mfd"))?;
            if mfd == 0 {
                break;
            }
            if mfd == -1 {
                return Err(NjoyError::NotPorted(
                    "groupr::parse::card9a_extended_residual",
                ));
            }
            let mtd = *nums.get(1).ok_or_else(|| card_err("card 9: mtd"))?;
            reactions.push(ReactionRequest { mfd, mtd, name });
        }

        // --- card 10: next material (optional) ---
        let next_material = lines.next().and_then(|l| parse_ints(l).first().copied());

        Ok(Self {
            units,
            matb,
            neutron_groups,
            photon_groups,
            weight,
            lord,
            iprint,
            ismooth,
            title,
            temperatures,
            sigma_zeros,
            neutron_grid,
            photon_grid,
            thermal_fission_params,
            reactions,
            next_material,
        })
    }
}

// ---------------------------------------------------------------------------
// Free-format helpers.
// ---------------------------------------------------------------------------

/// Next non-skipped line, or a "missing card" parse error.
fn next_line<'a, I: Iterator<Item = &'a str>>(
    lines: &mut I,
    what: &str,
) -> Result<&'a str, NjoyError> {
    lines
        .next()
        .ok_or_else(|| NjoyError::EndfParse(format!("groupr deck ended early: missing {what}")))
}

/// A card-level parse error with a fixed message prefix.
fn card_err(msg: &str) -> NjoyError {
    NjoyError::EndfParse(format!("groupr deck: {msg}"))
}

/// Split a line into whitespace/comma tokens, stopping at a trailing `/`
/// terminator (NJOY free-format decks end a card with `/`).
fn tokens(line: &str) -> impl Iterator<Item = &str> {
    let body = line.split('/').next().unwrap_or(line);
    body.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|t| !t.is_empty())
}

/// Parse the leading integer tokens of a line (Fortran-style; a real value like
/// `3.0` truncates toward zero).
fn parse_ints(line: &str) -> Vec<i32> {
    tokens(line)
        .map_while(|t| {
            t.parse::<i32>()
                .ok()
                .or_else(|| t.parse::<f64>().ok().map(|f| f as i32))
        })
        .collect()
}

/// Parse the leading float tokens of a line, accepting Fortran `d`/`D`
/// exponents.
fn parse_floats(line: &str) -> Vec<f64> {
    tokens(line)
        .map_while(|t| {
            let norm = t.replace(['d', 'D'], "e");
            norm.parse::<f64>().ok()
        })
        .collect()
}

/// Strip a card-3 title: remove surrounding single/double quotes and a trailing
/// `/`, then trim. Matches the `read(text,'(16a4,a2)')` handling in `ruinb`
/// (`groupr.f90:1078-1080`) closely enough for round-tripping.
fn strip_title(line: &str) -> String {
    let mut t = line.trim();
    if let Some(stripped) = t.strip_suffix('/') {
        t = stripped.trim();
    }
    let t = t.trim_matches(|c| c == '\'' || c == '"').trim();
    t.to_string()
}

/// Split a card-9 reaction line into its leading integers (`mfd`, `mtd`) and the
/// trailing free-text name (which may be quoted).
fn split_reaction_line(line: &str) -> (Vec<i32>, String) {
    // Numbers up to the first quote or the '/'.
    let body = line.split('/').next().unwrap_or(line);
    // Name: text inside quotes if present, else empty.
    let name = if let Some(start) = body.find(['\'', '"']) {
        let rest = &body[start + 1..];
        let end = rest.find(['\'', '"']).unwrap_or(rest.len());
        rest[..end].to_string()
    } else {
        String::new()
    };
    let num_part = body.split(['\'', '"']).next().unwrap_or(body);
    let nums = parse_ints(num_part);
    (nums, name)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The `ign` adapter agrees (both directions) with the shared ERRORR
    /// [`NeutronGroupStructure::ign`] and forbids `ign == -1`.
    ///
    /// **Methodology.** For `ign = 1..=36`, assert
    /// `neutron_group_from_ign(ign).ign() == ign` (round-trip against the
    /// authoritative ERRORR mapping, so the two cannot drift). Assert `-1`, `0`,
    /// and `37` are rejected, matching GROUPR's `ign == -1` guard
    /// (`groupr.f90:1123`) and the illegal-structure abort.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** All 36 indices round-trip
    /// against `errorr::groups::NeutronGroupStructure::ign`; `-1/0/37` reject.
    #[test]
    fn ign_adapter_matches_errorr() {
        for ign in 1..=36 {
            let s = neutron_group_from_ign(ign).expect("valid ign");
            assert_eq!(s.ign(), ign, "ign={ign} round-trip");
        }
        assert!(neutron_group_from_ign(-1).is_err());
        assert!(neutron_group_from_ign(0).is_err());
        assert!(neutron_group_from_ign(37).is_err());
    }

    /// Every card-2 selector enum round-trips through its Fortran integer.
    ///
    /// **Methodology.** Round-trip `iprint` (0/1), `ismooth` (0/1), `iwt`
    /// magnitude (1..=12), and the signed `iwt` weighting mode (`-12..=12`).
    /// Check `WeightSelection::iwt()` reproduces the sign convention
    /// (`groupr.f90:181-183`) and that gating predicates match genwtf.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** All 12 `iwt` magnitudes and the
    /// 25 signed values (`-12..=-1, 0, 1..=12`) round-trip; iprint/ismooth
    /// round-trip; `reads_tab1`/`reads_analytic_params`/`is_analytic` agree.
    #[test]
    fn card2_selectors_round_trip() {
        for i in 0..=1 {
            assert_eq!(PrintOption::from_iprint(i).unwrap().iprint(), i);
            assert_eq!(SmoothingOption::from_ismooth(i).unwrap().ismooth(), i);
        }
        assert!(PrintOption::from_iprint(2).is_none());
        assert!(SmoothingOption::from_ismooth(-1).is_none());

        for iwt in 1..=12 {
            assert_eq!(WeightOption::from_iwt(iwt).unwrap().iwt(), iwt);
        }
        assert!(WeightOption::from_iwt(0).is_none());
        assert!(WeightOption::from_iwt(13).is_none());

        for iwt in -12..=12 {
            let sel = WeightSelection::from_iwt(iwt).expect("valid iwt");
            assert_eq!(sel.iwt(), iwt, "signed iwt={iwt}");
        }
        assert!(WeightSelection::from_iwt(13).is_none());
        assert!(WeightSelection::from_iwt(-13).is_none());

        assert!(WeightOption::ReadIn.reads_tab1());
        assert!(WeightOption::ThermalOneOverEFission.reads_analytic_params());
        assert!(WeightOption::Constant.is_analytic());
        assert!(!WeightOption::EpriCellLwr.is_analytic());
    }

    /// The default deck reproduces GROUPR's initialised card values.
    ///
    /// **Methodology.** Compare [`GrouprInput::default`] to the initialisation
    /// block at `groupr.f90:1064-1067` (`ntemp=1, nsigz=1, iprint=1,
    /// ismooth=1`) plus the documented `iwt=6`, `lord=3` example defaults.
    ///
    /// **Result (2026-07-15).** ntemp=1 (300 K), nsigz=1 (1e10 barns),
    /// iprint=Maximum, ismooth=On, igg=None, iwt=+6, lord=3.
    #[test]
    fn default_deck_matches_njoy_init() {
        let d = GrouprInput::default();
        assert_eq!(d.ntemp(), 1);
        assert_eq!(d.temperatures, vec![300.0]);
        assert_eq!(d.nsigz(), 1);
        assert_eq!(d.sigma_zeros, vec![1.0e10]);
        assert_eq!(d.iprint, PrintOption::Maximum);
        assert_eq!(d.ismooth, SmoothingOption::On);
        assert_eq!(d.photon_groups, PhotonGroupStructure::None);
        assert_eq!(d.weight.iwt(), 6);
        assert_eq!(d.lord, 3);
    }

    /// A representative deck parses to the expected structured fields.
    ///
    /// **Methodology.** Parse a deck for U-235 (`matb=9228`) on the VITAMIN-J
    /// 175-group neutron structure (`ign=17`), no photon groups (`igg=0`),
    /// constant weight (`iwt=2`), `lord=1`, two temperatures, two sigma-zeros,
    /// and three reactions (total, fission, capture) terminated by `0/`, then a
    /// next-material `0/`. Assert every parsed field. Card order per
    /// `groupr.f90:126-215`.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** Units (20,21,0,22); matb=9228;
    /// neutron=VitaminJ175 (ign=17); photon=None; iwt=+2; lord=1; temps
    /// [293.6, 900]; sigz [1e10, 100]; three reactions {(3,1),(3,18),(3,102)};
    /// next_material=0. All fields match.
    #[test]
    fn parse_representative_deck() {
        let deck = "\
20 21 0 22/
9228 17 0 2 1 2 2/
'u-235 vitamin-j'/
293.6 900.0/
1.0e10 100.0/
3 1 'total'/
3 18 'fission'/
3 102 'capture'/
0/
0/
";
        let inp = GrouprInput::parse(deck).expect("deck parses");
        assert_eq!(
            inp.units,
            UnitAssignments {
                nendf: 20,
                npend: 21,
                ngout1: 0,
                ngout2: 22
            }
        );
        assert_eq!(inp.matb, 9228);
        assert_eq!(inp.neutron_groups, NeutronGroupStructure::VitaminJ175);
        assert_eq!(inp.neutron_groups.ign(), 17);
        assert_eq!(inp.photon_groups, PhotonGroupStructure::None);
        assert_eq!(inp.weight.iwt(), 2);
        assert_eq!(inp.lord, 1);
        assert_eq!(inp.title, "u-235 vitamin-j");
        assert_eq!(inp.temperatures, vec![293.6, 900.0]);
        assert_eq!(inp.sigma_zeros, vec![1.0e10, 100.0]);
        assert_eq!(inp.reactions.len(), 3);
        assert_eq!(
            inp.reactions[0],
            ReactionRequest {
                mfd: 3,
                mtd: 1,
                name: "total".into()
            }
        );
        assert_eq!(
            inp.reactions[1],
            ReactionRequest {
                mfd: 3,
                mtd: 18,
                name: "fission".into()
            }
        );
        assert_eq!(
            inp.reactions[2],
            ReactionRequest {
                mfd: 3,
                mtd: 102,
                name: "capture".into()
            }
        );
        assert_eq!(inp.next_material, Some(0));
    }

    /// Parsing reads card 6 (read-in neutron grid) when `ign == 1` and card 8c
    /// (thermal/fission params) when `iwt == 4`.
    ///
    /// **Methodology.** Parse a deck with `ign=1` (arbitrary neutron structure:
    /// `ngn=3` then 4 boundaries) and `iwt=4` (card 8c `eb tb ec tc`). Assert
    /// the neutron grid and the parsed [`ThermalFissionParams`]. Card 6 read at
    /// `groupr.f90:4157-4161`; card 8c at `:5038`.
    ///
    /// **Result (2026-07-15).** neutron_grid = [1e-5, 1.0, 1e3, 2e7];
    /// thermal_fission_params = {eb=0.1, tb=0.025, ec=8.2e5, tc=1.35e6}.
    #[test]
    fn parse_reads_conditional_cards() {
        let deck = "\
20 21 0 22/
9228 1 0 4 1/
'arbitrary + thermal weight'/
300.0/
1.0e10/
3/
1.0e-5 1.0 1.0e3 2.0e7/
0.1 0.025 8.2e5 1.35e6/
3 1 'total'/
0/
0/
";
        let inp = GrouprInput::parse(deck).expect("deck parses");
        assert_eq!(inp.neutron_groups, NeutronGroupStructure::Arbitrary);
        assert_eq!(inp.neutron_grid, Some(vec![1.0e-5, 1.0, 1.0e3, 2.0e7]));
        let p = inp.thermal_fission_params.expect("card 8c present");
        assert_eq!(p.thermal_break_ev, 0.1);
        assert_eq!(p.thermal_temp_ev, 0.025);
        assert_eq!(p.fission_break_ev, 8.2e5);
        assert_eq!(p.fission_temp_ev, 1.35e6);
        assert_eq!(inp.reactions.len(), 1);
    }

    /// Not-ported weight cards and the extended-residual reaction card are
    /// reported honestly rather than mis-parsed.
    ///
    /// **Methodology.** A deck with `iwt=-3` (flux calc, card 8a), `iwt=1`
    /// (TAB1, card 8b), and `iwt=0` (resonance flux, card 8d) must each yield
    /// [`NjoyError::NotPorted`]. Guards against fabricating flux/TAB1 handling.
    ///
    /// **Result (2026-07-15).** iwt=-3 -> card8a NotPorted; iwt=1 -> card8b
    /// NotPorted; iwt=0 -> card8d NotPorted.
    #[test]
    fn parse_reports_notported_weight_cards() {
        let base = |iwt: i32| {
            format!("20 21 0 22/\n9228 3 0 {iwt} 1/\n'x'/\n300.0/\n1.0e10/\n3 1 'total'/\n0/\n0/\n")
        };
        assert!(matches!(
            GrouprInput::parse(&base(-3)),
            Err(NjoyError::NotPorted(_))
        ));
        assert!(matches!(
            GrouprInput::parse(&base(1)),
            Err(NjoyError::NotPorted(_))
        ));
        assert!(matches!(
            GrouprInput::parse(&base(0)),
            Err(NjoyError::NotPorted(_))
        ));
    }
}
