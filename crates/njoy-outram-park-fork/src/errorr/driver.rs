// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! ERRORR user-input card model and top-level driver skeleton.
//!
//! This module is a *faithful, line-traceable* translation of the **input-card
//! reading and top-level control flow** of NJOY2016's `subroutine errorr`
//! (`errorr.f90:134-1089`). It models the free-format card sequence that
//! configures an ERRORR run — unit numbers, material, group/weight selectors,
//! covariance-file selection, derived-cross-section coefficients, group grid,
//! and weight function — as a documented Rust struct ([`ErrorrInput`]) plus a
//! set of named selector enums.
//!
//! **What is *not* here.** The numerical covariance kernels — union-grid
//! construction (`gridd`), group averaging (`grpav`/`colaps`/`grpav4`),
//! covariance calculation (`covcal`), and covariance output (`covout`) — are
//! ported separately. [`run`] therefore documents the pipeline and returns
//! [`NjoyError::NotPorted`] rather than fabricating a covariance result.
//!
//! **Card grouping** mirrors the Fortran comment blocks (`errorr.f90:172-303`).
//! Field names keep the upstream Fortran identifiers in their doc comments so a
//! discrepancy can be localised to a specific `read(nsysi,*)` statement.
//!
//! ## Card summary (from `errorr.f90:172-303`)
//!
//! | Card | Fortran vars | Condition |
//! |------|--------------|-----------|
//! | 1 | `nendf npend ngout nout nin nstan` | always |
//! | 2 | `matd ign iwt iprint irelco` | always |
//! | 3 | `mprint tempin` | always (REQUIRED since njoy2012) |
//! | 4 | `nek` | ENDF/B-IV (`iverf==4`) only |
//! | 5 | `ek(nek+1)` | IV only, omit if `nek==0` |
//! | 6 | `akxy` | IV only, omit if `nek==0` |
//! | 7 | `iread mfcov irespr legord ifissp efmean dap` | V/VI (`iverf!=4`) only |
//! | 8 | `nmt nek` | `iread==1` only |
//! | 8a | `mts(nmt)` | `iread==1` only |
//! | 8b | `ek(nek+1)` | `iread==1`, omit if `nek==0` |
//! | 9 | `akxy` | `iread==1`, omit if `nek==0` |
//! | 10 | `mat1 mt1` pairs, term. by `0` | `iread==2` only |
//! | 11 | `matb mtb matc mtc` quads, term. by `0` | `nstan!=0` only |
//! | 12a/b | `ngn`, `egn(ngn+1)` | `ign==1` or `ign==-1` only |
//! | 13 | `wght` (TAB1) | `iwt==1` only |
//! | 13b | `eb tb ec tc` | `iwt==4` only |

use crate::NjoyError;

// ---------------------------------------------------------------------------
// Card 1 — unit-number assignments (errorr.f90:174-187, read at :422)
// ---------------------------------------------------------------------------

/// Card 1: logical unit numbers for the ENDF tapes ERRORR reads and writes.
///
/// Fortran: `read(nsysi,*) nendf,npend,ngout,nout,nin,nstan` (`errorr.f90:422`).
/// A negative unit number denotes a binary (unformatted) tape in NJOY; a
/// positive number denotes a formatted (BCD/ASCII) tape; zero means "absent".
/// These are transport-layer handles, not physical quantities — no units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitAssignments {
    /// `nendf` — unit for the input ENDF tape (covariance data source).
    /// Required (nonzero). The sentinel value `999` selects the special
    /// `covadd` merge option instead of a normal run (`errorr.f90:425`).
    pub nendf: i32,
    /// `npend` — unit for the input PENDF tape (pointwise cross sections used
    /// by `grpav` when `ngout==0`).
    pub npend: i32,
    /// `ngout` — unit for the input group cross-section (GENDF) tape. If `0`,
    /// group cross sections are calculated from `npend`. Must be nonzero when
    /// `iread==2` or `mfcov` is 31/35/40 (`errorr.f90:177-183, 543-548`).
    /// Default 0.
    pub ngout: i32,
    /// `nout` — unit for the output covariance tape. Default 0 (no output).
    pub nout: i32,
    /// `nin` — unit for an input covariance tape (for restart/merge). `nin`
    /// and `nout` must be both formatted or both binary. Default 0.
    pub nin: i32,
    /// `nstan` — unit for a ratio-to-standard ENDF tape. Default 0. Must differ
    /// from `nendf` (`errorr.f90:446-447`).
    pub nstan: i32,
}

impl Default for UnitAssignments {
    /// NJOY defaults (`errorr.f90:417-421`): only `nendf`/`npend` are normally
    /// user-set; the rest default to 0. Here `nendf`/`npend` default to 0 too
    /// and must be provided by the caller before a real run.
    fn default() -> Self {
        Self {
            nendf: 0,
            npend: 0,
            ngout: 0,
            nout: 0,
            nin: 0,
            nstan: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Card 2 selector enums (errorr.f90:189-199, read at :477)
// ---------------------------------------------------------------------------

/// Neutron group-structure option (`ign`), card 2 (`errorr.f90:191-196`,
/// `:306-344`).
///
/// Same meaning as GROUPR's `ign`, with the ERRORR-specific addition of `-1`.
/// Selects a predefined multigroup boundary set, or a read-in one. Default
/// [`GroupStructure::ArbitraryRead`] (`ign==1`).
///
/// For `ign==1` and `ign==-1` the boundaries are read from card 12
/// ([`GroupGrid`]); all other variants name a built-in structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupStructure {
    /// `-1`: arbitrary structure (read in), supplemented with the ENDF
    /// covariance grid within the user energy range.
    ArbitraryReadSupplemented,
    /// `1`: arbitrary structure (read in). Default.
    ArbitraryRead,
    /// `2`: CSEWG 239-group structure.
    Csewg239,
    /// `3`: LANL 30-group structure.
    Lanl30,
    /// `4`: ANL 27-group structure.
    Anl27,
    /// `5`: RRD 50-group structure.
    Rrd50,
    /// `6`: GAM-I 68-group structure.
    GamI68,
    /// `7`: GAM-II 100-group structure.
    GamII100,
    /// `8`: LASER-THERMOS 35-group structure.
    LaserThermos35,
    /// `9`: EPRI-CPM 69-group structure.
    EpriCpm69,
    /// `10`: LANL 187-group structure.
    Lanl187,
    /// `11`: LANL 70-group structure.
    Lanl70,
    /// `12`: SAND-II 620-group structure.
    SandII620,
    /// `13`: LANL 80-group structure.
    Lanl80,
    /// `14`: EURLIB 100-group structure.
    Eurlib100,
    /// `15`: SAND-IIA 640-group structure.
    SandIIa640,
    /// `16`: VITAMIN-E 174-group structure.
    VitaminE174,
    /// `17`: VITAMIN-J 175-group structure.
    VitaminJ175,
    /// `18`: XMAS NEA-LANL structure.
    XmasNeaLanl,
    /// `19`: ECCO 33-group structure. (Upstream warns option 19 changed; use
    /// `-1` instead — `errorr.f90:483-487`.)
    Ecco33,
    /// `20`: ECCO 1968-group structure.
    Ecco1968,
    /// `21`: TRIPOLI 315-group structure.
    Tripoli315,
    /// `22`: XMAS LWPC 172-group structure.
    XmasLwpc172,
    /// `23`: VIT-J LWPC 175-group structure.
    VitJLwpc175,
    /// `24`: SHEM CEA 281-group structure.
    ShemCea281,
    /// `25`: SHEM EPM 295-group structure.
    ShemEpm295,
    /// `26`: SHEM CEA/EPM 361-group structure.
    ShemCeaEpm361,
    /// `27`: SHEM EPM 315-group structure.
    ShemEpm315,
    /// `28`: RAHAB AECL 89-group structure.
    RahabAecl89,
    /// `29`: CCFE 660-group structure (30 MeV).
    Ccfe660,
    /// `30`: UKAEA 1025-group structure (30 MeV).
    Ukaea1025,
    /// `31`: UKAEA 1067-group structure (200 MeV).
    Ukaea1067,
    /// `32`: UKAEA 1102-group structure (1 GeV).
    Ukaea1102,
    /// `33`: UKAEA 142-group structure (200 MeV).
    Ukaea142,
    /// `34`: LANL 618-group structure.
    Lanl618,
}

impl GroupStructure {
    /// Return the Fortran `ign` integer for this structure (`errorr.f90:306-344`).
    pub fn to_ign(self) -> i32 {
        match self {
            Self::ArbitraryReadSupplemented => -1,
            Self::ArbitraryRead => 1,
            Self::Csewg239 => 2,
            Self::Lanl30 => 3,
            Self::Anl27 => 4,
            Self::Rrd50 => 5,
            Self::GamI68 => 6,
            Self::GamII100 => 7,
            Self::LaserThermos35 => 8,
            Self::EpriCpm69 => 9,
            Self::Lanl187 => 10,
            Self::Lanl70 => 11,
            Self::SandII620 => 12,
            Self::Lanl80 => 13,
            Self::Eurlib100 => 14,
            Self::SandIIa640 => 15,
            Self::VitaminE174 => 16,
            Self::VitaminJ175 => 17,
            Self::XmasNeaLanl => 18,
            Self::Ecco33 => 19,
            Self::Ecco1968 => 20,
            Self::Tripoli315 => 21,
            Self::XmasLwpc172 => 22,
            Self::VitJLwpc175 => 23,
            Self::ShemCea281 => 24,
            Self::ShemEpm295 => 25,
            Self::ShemCeaEpm361 => 26,
            Self::ShemEpm315 => 27,
            Self::RahabAecl89 => 28,
            Self::Ccfe660 => 29,
            Self::Ukaea1025 => 30,
            Self::Ukaea1067 => 31,
            Self::Ukaea1102 => 32,
            Self::Ukaea142 => 33,
            Self::Lanl618 => 34,
        }
    }

    /// Parse a Fortran `ign` integer. Returns `None` for out-of-range values
    /// (valid: `-1`, `1..=34`).
    pub fn from_ign(ign: i32) -> Option<Self> {
        Some(match ign {
            -1 => Self::ArbitraryReadSupplemented,
            1 => Self::ArbitraryRead,
            2 => Self::Csewg239,
            3 => Self::Lanl30,
            4 => Self::Anl27,
            5 => Self::Rrd50,
            6 => Self::GamI68,
            7 => Self::GamII100,
            8 => Self::LaserThermos35,
            9 => Self::EpriCpm69,
            10 => Self::Lanl187,
            11 => Self::Lanl70,
            12 => Self::SandII620,
            13 => Self::Lanl80,
            14 => Self::Eurlib100,
            15 => Self::SandIIa640,
            16 => Self::VitaminE174,
            17 => Self::VitaminJ175,
            18 => Self::XmasNeaLanl,
            19 => Self::Ecco33,
            20 => Self::Ecco1968,
            21 => Self::Tripoli315,
            22 => Self::XmasLwpc172,
            23 => Self::VitJLwpc175,
            24 => Self::ShemCea281,
            25 => Self::ShemEpm295,
            26 => Self::ShemCeaEpm361,
            27 => Self::ShemEpm315,
            28 => Self::RahabAecl89,
            29 => Self::Ccfe660,
            30 => Self::Ukaea1025,
            31 => Self::Ukaea1067,
            32 => Self::Ukaea1102,
            33 => Self::Ukaea142,
            34 => Self::Lanl618,
            _ => return None,
        })
    }

    /// `true` when the group boundaries must be read from card 12
    /// ([`GroupGrid`]) — i.e. `ign == 1` or `ign == -1` (`errorr.f90:292-295`,
    /// `:945`).
    pub fn reads_boundaries(self) -> bool {
        matches!(self, Self::ArbitraryRead | Self::ArbitraryReadSupplemented)
    }
}

/// Weight-function option (`iwt`), card 2 (`errorr.f90:197`, `:345-358`).
///
/// Selects the flux weighting used to average cross sections into groups. Same
/// meaning as GROUPR's `iwt`. Default [`WeightOption::ThermalOneOverEFissionFusion`]
/// (`iwt==6`).
///
/// Note: ERRORR rejects `iwt <= 0` and forces `iwt = 6` with a message
/// (`errorr.f90:478-482`), so only the positive options below are valid input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightOption {
    /// `1`: read in a smooth (TAB1) weight function — card 13 ([`WeightData::Tab1`]).
    ReadIn,
    /// `2`: constant (flat) weight.
    Constant,
    /// `3`: 1/E weight.
    OneOverE,
    /// `4`: 1/E + fission spectrum + thermal Maxwellian — card 13b
    /// ([`WeightData::Analytic`]).
    OneOverEPlusFissionPlusThermal,
    /// `5`: EPRI-CELL LWR weight.
    EpriCellLwr,
    /// `6`: (thermal) -- (1/E) -- (fission + fusion). Default.
    ThermalOneOverEFissionFusion,
    /// `7`: same as 6 with a temperature-dependent thermal part.
    ThermalOneOverEFissionFusionTDep,
    /// `8`: thermal -- 1/E -- fast reactor -- (fission + fusion).
    ThermalOneOverEFastFissionFusion,
    /// `9`: CLAW weight function.
    Claw,
    /// `10`: CLAW with a temperature-dependent thermal part.
    ClawTDep,
    /// `11`: VITAMIN-E weight function (ORNL-5505).
    VitaminE,
    /// `12`: VIT-E with a temperature-dependent thermal part.
    VitaminETDep,
}

impl WeightOption {
    /// Return the Fortran `iwt` integer.
    pub fn to_iwt(self) -> i32 {
        match self {
            Self::ReadIn => 1,
            Self::Constant => 2,
            Self::OneOverE => 3,
            Self::OneOverEPlusFissionPlusThermal => 4,
            Self::EpriCellLwr => 5,
            Self::ThermalOneOverEFissionFusion => 6,
            Self::ThermalOneOverEFissionFusionTDep => 7,
            Self::ThermalOneOverEFastFissionFusion => 8,
            Self::Claw => 9,
            Self::ClawTDep => 10,
            Self::VitaminE => 11,
            Self::VitaminETDep => 12,
        }
    }

    /// Parse a Fortran `iwt` integer. Returns `None` outside `1..=12`.
    /// (Callers should note upstream coerces `iwt <= 0` to `6`.)
    pub fn from_iwt(iwt: i32) -> Option<Self> {
        Some(match iwt {
            1 => Self::ReadIn,
            2 => Self::Constant,
            3 => Self::OneOverE,
            4 => Self::OneOverEPlusFissionPlusThermal,
            5 => Self::EpriCellLwr,
            6 => Self::ThermalOneOverEFissionFusion,
            7 => Self::ThermalOneOverEFissionFusionTDep,
            8 => Self::ThermalOneOverEFastFissionFusion,
            9 => Self::Claw,
            10 => Self::ClawTDep,
            11 => Self::VitaminE,
            12 => Self::VitaminETDep,
            _ => return None,
        })
    }

    /// `true` when a TAB1 weight function must be read from card 13
    /// (`iwt == 1`, `errorr.f90:296-297`, `:951`).
    pub fn reads_tab1(self) -> bool {
        matches!(self, Self::ReadIn)
    }

    /// `true` when the analytic flux parameters must be read from card 13b
    /// (`iwt == 4`, `errorr.f90:298-302`, `:959`).
    pub fn reads_analytic_params(self) -> bool {
        matches!(self, Self::OneOverEPlusFissionPlusThermal)
    }
}

/// Covariance print option (`iprint`), card 2 (`errorr.f90:198`).
/// Controls the verbosity of the covariance-matrix printout. Default
/// [`CovariancePrint::Maximum`] (`iprint==1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CovariancePrint {
    /// `0`: minimum printout.
    Minimum,
    /// `1`: maximum printout. Default.
    Maximum,
}

impl CovariancePrint {
    /// Return the Fortran `iprint` integer (0/1).
    pub fn to_iprint(self) -> i32 {
        match self {
            Self::Minimum => 0,
            Self::Maximum => 1,
        }
    }
    /// Parse `iprint` (0/1). Returns `None` otherwise.
    pub fn from_iprint(iprint: i32) -> Option<Self> {
        match iprint {
            0 => Some(Self::Minimum),
            1 => Some(Self::Maximum),
            _ => None,
        }
    }
}

/// Covariance form (`irelco`), card 2 (`errorr.f90:199`).
/// Whether output covariances are absolute or relative. Default
/// [`CovarianceForm::Relative`] (`irelco==1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CovarianceForm {
    /// `0`: absolute covariances.
    Absolute,
    /// `1`: relative covariances. Default.
    Relative,
}

impl CovarianceForm {
    /// Return the Fortran `irelco` integer (0/1).
    pub fn to_irelco(self) -> i32 {
        match self {
            Self::Absolute => 0,
            Self::Relative => 1,
        }
    }
    /// Parse `irelco` (0/1). Returns `None` otherwise.
    pub fn from_irelco(irelco: i32) -> Option<Self> {
        match irelco {
            0 => Some(Self::Absolute),
            1 => Some(Self::Relative),
            _ => None,
        }
    }
}

/// Group-averaging print option (`mprint`), card 3 (`errorr.f90:202`).
/// Controls the verbosity of the group cross-section averaging step. Default
/// [`GroupAvgPrint::Minimum`] (`mprint==0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupAvgPrint {
    /// `0`: minimum printout. Default.
    Minimum,
    /// `1`: maximum printout.
    Maximum,
}

impl GroupAvgPrint {
    /// Return the Fortran `mprint` integer (0/1).
    pub fn to_mprint(self) -> i32 {
        match self {
            Self::Minimum => 0,
            Self::Maximum => 1,
        }
    }
    /// Parse `mprint` (0/1). Returns `None` otherwise.
    pub fn from_mprint(mprint: i32) -> Option<Self> {
        match mprint {
            0 => Some(Self::Minimum),
            1 => Some(Self::Maximum),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Card 7 selectors (ENDF/B V/VI, errorr.f90:217-254, read at :521)
// ---------------------------------------------------------------------------

/// MT-list read option (`iread`), card 7 (`errorr.f90:218`).
///
/// Selects how the reaction (MT) list to be processed is obtained. Default
/// [`ReadOption::ProgramCalculated`] (`iread==0`). Valid range 0..=2
/// (`errorr.f90:536-539`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOption {
    /// `0`: program-calculated MTs (from the ENDF dictionary). Default.
    ProgramCalculated,
    /// `1`: user-input MTs and derived-xsec energies (cards 8/8a/8b/9).
    InputMtsAndEks,
    /// `2`: calculated MTs plus extra `mat1-mt1` pairs from card 10.
    CalculatedPlusExtraPairs,
}

impl ReadOption {
    /// Return the Fortran `iread` integer (0/1/2).
    pub fn to_iread(self) -> i32 {
        match self {
            Self::ProgramCalculated => 0,
            Self::InputMtsAndEks => 1,
            Self::CalculatedPlusExtraPairs => 2,
        }
    }
    /// Parse `iread` (0/1/2). Returns `None` for illegal values
    /// (`errorr.f90:536`).
    pub fn from_iread(iread: i32) -> Option<Self> {
        match iread {
            0 => Some(Self::ProgramCalculated),
            1 => Some(Self::InputMtsAndEks),
            2 => Some(Self::CalculatedPlusExtraPairs),
            _ => None,
        }
    }
}

/// ENDF covariance file to process (`mfcov`), card 7 (`errorr.f90:221-227`).
///
/// Selects which ENDF covariance file drives the run. Default
/// [`CovarianceFile::Mf33`]. Note the MF=32 (resonance-parameter) contribution
/// is folded in automatically when `mfcov==33` (`errorr.f90:223-225`). The
/// test-only high-speed encodings `-33` and `333` are modelled separately by
/// [`SpeedupOption`], since upstream rewrites both to `mfcov=33`
/// (`errorr.f90:668-679`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CovarianceFile {
    /// `31`: ν̄ (nu-bar) covariances. Requires a nonzero `ngout`.
    Mf31,
    /// `33`: cross-section covariances (folds in MF=32). Default.
    Mf33,
    /// `34`: angular-distribution covariances.
    Mf34,
    /// `35`: secondary-energy (spectrum) covariances. Requires nonzero `ngout`.
    Mf35,
    /// `40`: production-cross-section covariances. Requires nonzero `ngout`.
    Mf40,
}

impl CovarianceFile {
    /// Return the Fortran `mfcov` integer (31/33/34/35/40).
    pub fn to_mfcov(self) -> i32 {
        match self {
            Self::Mf31 => 31,
            Self::Mf33 => 33,
            Self::Mf34 => 34,
            Self::Mf35 => 35,
            Self::Mf40 => 40,
        }
    }
    /// Parse `mfcov`. Returns `None` for values ERRORR is not coded for
    /// (`errorr.f90:705-709`). The `-33`/`333` test encodings are *not*
    /// accepted here — model them via [`SpeedupOption`] with `Mf33`.
    pub fn from_mfcov(mfcov: i32) -> Option<Self> {
        match mfcov {
            31 => Some(Self::Mf31),
            33 => Some(Self::Mf33),
            34 => Some(Self::Mf34),
            35 => Some(Self::Mf35),
            40 => Some(Self::Mf40),
            _ => None,
        }
    }

    /// `true` when a nonzero `ngout` is mandatory for this file
    /// (`errorr.f90:543-548`: `mfcov` 31/35/40).
    pub fn requires_ngout(self) -> bool {
        matches!(self, Self::Mf31 | Self::Mf35 | Self::Mf40)
    }
}

/// High-speed test encoding of `mfcov` (`errorr.f90:663-679`).
///
/// The `mfcov` inputs `-33` and `333` are *not* distinct covariance files —
/// upstream rewrites both to `mfcov=33` and tightens the `eskip1..3`
/// energy-thinning factors for a faster (lower-fidelity) test run. Modelled as
/// a separate flag so [`CovarianceFile`] stays a clean 5-way enum.
///
/// These are for test cases only; production runs use [`SpeedupOption::None`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedupOption {
    /// Normal fidelity: `eskip1=1.00002, eskip2=1.0003, eskip3=1.005`
    /// (`errorr.f90:664-667`).
    None,
    /// `mfcov=-33` fast test: `eskip1=1.0002, eskip2=1.0003, eskip3=1.005`
    /// (`errorr.f90:668-673`).
    Fast,
    /// `mfcov=333` faster test: `eskip1=1.002, eskip2=1.003, eskip3=1.005`
    /// (`errorr.f90:674-679`).
    Faster,
}

/// Resonance-parameter-covariance (MF=32) processing method (`irespr`),
/// card 7 (`errorr.f90:228-231`). Only meaningful when `mfcov==33`. Default
/// [`ResonanceProcessing::OnePercentSensitivity`] (`irespr==1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResonanceProcessing {
    /// `0`: area-sensitivity method.
    AreaSensitivity,
    /// `1`: 1%-sensitivity method. Default.
    OnePercentSensitivity,
}

impl ResonanceProcessing {
    /// Return the Fortran `irespr` integer (0/1).
    pub fn to_irespr(self) -> i32 {
        match self {
            Self::AreaSensitivity => 0,
            Self::OnePercentSensitivity => 1,
        }
    }
    /// Parse `irespr` (0/1). Returns `None` otherwise.
    pub fn from_irespr(irespr: i32) -> Option<Self> {
        match irespr {
            0 => Some(Self::AreaSensitivity),
            1 => Some(Self::OnePercentSensitivity),
            _ => None,
        }
    }
}

/// Card 7 covariance-processing controls (ENDF/B V/VI, `iverf != 4`).
///
/// Fortran: `read(nsysi,*) iread,mfcov,irespr,legord,ifissp,efmean,dap`
/// (`errorr.f90:521`). Read only for `iverf != 4`; for `iverf == 4` these keep
/// their defaults and the ENDF/B-IV derived-xsec path (cards 4-6) is used
/// instead.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CovarianceControl {
    /// `iread` — MT-list read option. Default [`ReadOption::ProgramCalculated`].
    pub iread: ReadOption,
    /// `mfcov` — covariance file to process. Default [`CovarianceFile::Mf33`].
    pub mfcov: CovarianceFile,
    /// High-speed test flag derived from `mfcov = -33 / 333`. Default
    /// [`SpeedupOption::None`].
    pub speedup: SpeedupOption,
    /// `irespr` — MF=32 processing method (used only for `mfcov==33`). Default
    /// [`ResonanceProcessing::OnePercentSensitivity`].
    pub irespr: ResonanceProcessing,
    /// `legord` (Fortran `legord`, card 7 field `l`) — Legendre order for
    /// reaction MT. Ignored unless `mfcov==34`. Must be `>= 0` for MF=34
    /// (`errorr.f90:526-529`). Default 1.
    pub legord: i32,
    /// `ifissp` (Fortran `ifissp`, card 7 field `l1/ifissp`) — dual-purpose:
    /// for `mfcov==34` it is the Legendre order for MT1 (default 1); for
    /// `mfcov==35` it selects the spectrum subsection to process (default -1 =
    /// the subsection containing `efmean`). The sentinel `-1000` marks "unset"
    /// on input and is resolved to `-1` (MF=35) or `1` (MF=34) at
    /// `errorr.f90:522-523`. Ignored for other `mfcov`. Default -1000.
    pub ifissp: i32,
    /// `efmean` — incident neutron energy in **eV** selecting the MF=35
    /// spectrum-covariance subsection whose energy interval includes it.
    /// Ignored unless `mfcov==35`. Default 2.0e6 eV (`errorr.f90:407`).
    pub efmean: f64,
    /// `dap` — user scattering-radius uncertainty as a fraction (0.1 = 10%).
    /// Overrides any value on the ENDF tape. Only defined for `mfcov==33`;
    /// a nonzero value sets the internal `isru` flag (`errorr.f90:541`).
    /// Default 0.0.
    pub dap: f64,
}

impl Default for CovarianceControl {
    /// NJOY defaults (`errorr.f90:514-520`).
    fn default() -> Self {
        Self {
            iread: ReadOption::ProgramCalculated,
            mfcov: CovarianceFile::Mf33,
            speedup: SpeedupOption::None,
            irespr: ResonanceProcessing::OnePercentSensitivity,
            legord: 1,
            ifissp: -1000,
            efmean: 2.0e6,
            dap: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Derived cross sections, standards, group grid, weight data
// ---------------------------------------------------------------------------

/// Derived cross-section definition (cards 4-6 for ENDF/B-IV; cards 8b/9 for
/// `iread==1`).
///
/// A "derived" cross section is a linear combination of the base reactions,
/// defined per energy range by a coefficient matrix `akxy`. When there are no
/// energy ranges (`nek==0`) every reaction is treated as independent
/// (`errorr.f90:829-837`, `:885-891`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DerivedXsec {
    /// `ek` — the `nek+1` derived-xsec energy bounds, in **eV**, monotonically
    /// increasing (`errorr.f90:211-213`, `:840`). Empty means `nek==0` (all
    /// reactions independent).
    pub ek: Vec<f64>,
    /// `akxy` — derived-xsec coefficient cube indexed `[range][row_mt][col_mt]`,
    /// one row per line in the card image (`errorr.f90:213`, `:844-855`). Outer
    /// length is `nek` (number of energy ranges), inner two are `nmt x nmt`.
    pub akxy: Vec<Vec<Vec<f64>>>,
}

/// A `(mat, mt)` reaction identifier pair (card 10 extra pairs, `iread==2`).
/// Fortran `mat1`/`mt1` (`errorr.f90:270-273`, read at `:911`). Terminated in
/// the card stream by `mat1==0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatMtPair {
    /// `mat1` — cross-material MAT whose reaction is added to the covariance list.
    pub mat1: i32,
    /// `mt1` — the reaction MT within `mat1`.
    pub mt1: i32,
}

/// Standard-reaction redefinition (card 11, only when `nstan != 0`).
///
/// Fortran `matb`/`mtb`/`matc`/`mtc` (`errorr.f90:277-290`, read at `:926`).
/// Redefines the standard reaction referenced in `matd`: reaction `(matb,mtb)`
/// is replaced by `(matc,mtc)`. Terminated in the card stream by `matb==0`.
/// If `matb`/`mtb` are **negative**, `(matc,mtc)` instead names a third
/// reaction correlated with `matd` through the shared standard
/// (`errorr.f90:283-290`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandardRedefinition {
    /// `matb` — standards reaction MAT referenced in `matd` (negative = special
    /// correlated-third-reaction mode).
    pub matb: i32,
    /// `mtb` — standards reaction MT referenced in `matd`.
    pub mtb: i32,
    /// `matc` — replacement standards reaction MAT.
    pub matc: i32,
    /// `mtc` — replacement standards reaction MT.
    pub mtc: i32,
}

/// Card 12: an arbitrary (read-in) group structure (only for `ign == 1` or
/// `ign == -1`).
///
/// Fortran `ngn` (card 12a) then `egn(ngn+1)` (card 12b) (`errorr.f90:292-295`,
/// read at `:946-948`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GroupGrid {
    /// `egn` — the `ngn+1` group boundaries in **eV**, monotonically
    /// increasing. `ngn` (number of groups) is `egn.len() - 1`.
    pub egn: Vec<f64>,
}

/// Card 13 / 13b: the weight function, when it must be supplied on input.
///
/// Which variant is required is dictated by [`WeightOption`]: `iwt==1` reads a
/// TAB1 record (card 13), `iwt==4` reads the analytic break/temperature
/// parameters (card 13b). For all other `iwt` values a built-in weight is used
/// and no weight data is read (this field is [`WeightData::None`]).
#[derive(Debug, Clone, PartialEq)]
pub enum WeightData {
    /// No weight data read from input (built-in weight; `iwt` not 1 or 4).
    None,
    /// Card 13 (`iwt==1`): a smooth weight function as a raw ENDF TAB1 record
    /// (`errorr.f90:296-297`, read at `:954`). Stored as the flat `wght(...)`
    /// array exactly as ERRORR reads it; interpretation (NR/NP interpolation
    /// ranges) is left to the group-averaging kernel.
    Tab1 {
        /// The raw `wght` TAB1 words (control words followed by the
        /// interpolation table and `(E, weight)` pairs).
        wght: Vec<f64>,
    },
    /// Card 13b (`iwt==4`): analytic 1/E + fission + thermal-Maxwellian
    /// parameters (`errorr.f90:298-302`, read at `:960`). All four are in **eV**.
    Analytic {
        /// `eb` — thermal break energy (eV).
        eb: f64,
        /// `tb` — thermal temperature (eV).
        tb: f64,
        /// `ec` — fission break energy (eV).
        ec: f64,
        /// `tc` — fission temperature (eV).
        tc: f64,
    },
}

impl Default for WeightData {
    fn default() -> Self {
        Self::None
    }
}

/// Special `nendf == 999` covariance-merge option (`errorr.f90:425-444`).
///
/// When card 1's `nendf` is `999`, ERRORR does *not* run a normal covariance
/// calculation. Instead it reads card `nitape notape` then a list of MT numbers
/// (terminated by `0`) and calls `covadd` to merge covariance reactions between
/// two existing tapes, then returns. Modelled here so the driver can dispatch
/// to `covadd`; the merge itself is a separate (not-yet-ported) kernel.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CovAddOption {
    /// `nitape` — input tape unit for the `covadd` merge.
    pub nitape: i32,
    /// `notape` — output tape unit for the `covadd` merge.
    pub notape: i32,
    /// The MT numbers to add, read one per card and terminated by `0`. At most
    /// 5 are allowed (`errorr.f90:432-434`).
    pub add_mts: Vec<i32>,
}

// ---------------------------------------------------------------------------
// Top-level input aggregate
// ---------------------------------------------------------------------------

/// Complete ERRORR user-input deck (`errorr.f90:172-303`).
///
/// Aggregates every card ERRORR may read. Version- and option-conditional cards
/// are modelled as optional/enum fields whose presence mirrors the Fortran
/// `read` guards; each field's doc comment states the card, the Fortran name,
/// and the condition under which upstream reads it.
///
/// Construct via [`ErrorrInput::default`] and override fields, or build the
/// sub-structs directly. Defaults reproduce NJOY's initialised values
/// (`errorr.f90:400-421`, `:472-476`, `:488-520`).
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorrInput {
    /// Card 1 — unit assignments.
    pub units: UnitAssignments,

    /// Card 2 — `matd`: the ENDF material (MAT) number to process
    /// (`errorr.f90:190`). No default; must be set by the caller.
    pub matd: i32,
    /// Card 2 — `ign`: neutron group-structure option.
    pub group_structure: GroupStructure,
    /// Card 2 — `iwt`: weight-function option.
    pub weight: WeightOption,
    /// Card 2 — `iprint`: covariance print option.
    pub iprint: CovariancePrint,
    /// Card 2 — `irelco`: covariance form (absolute/relative).
    pub irelco: CovarianceForm,

    /// Card 3 — `mprint`: group-averaging print option.
    pub mprint: GroupAvgPrint,
    /// Card 3 — `tempin`: temperature in **kelvin** (`errorr.f90:203`). Must
    /// match a temperature present on the input GENDF tape when `ngout != 0`.
    /// Default 300 K.
    pub tempin: f64,

    /// Card 7 covariance controls (ENDF/B V/VI). Read only for `iverf != 4`;
    /// for ENDF/B-IV tapes these fields keep their defaults and the card 4-6
    /// derived-xsec path is used instead. The ENDF format version (`iverf`) is
    /// determined from the tape at run time (`errorr.f90:459-469`), not from
    /// input, so both paths are represented and the driver selects between them.
    pub covariance: CovarianceControl,

    /// Cards 8/8a (`iread==1`): the user MT list to process (Fortran `mts`,
    /// `errorr.f90:262`, `:878`). Empty unless `iread==1`.
    pub user_mts: Vec<i32>,

    /// Derived cross-section coefficients: cards 4-6 (ENDF/B-IV) or cards 8b/9
    /// (`iread==1`). See [`DerivedXsec`].
    pub derived: DerivedXsec,

    /// Card 10 (`iread==2`): extra `mat1-mt1` pairs to add to the covariance
    /// reaction list (`errorr.f90:268-273`). Empty unless `iread==2`.
    pub extra_pairs: Vec<MatMtPair>,

    /// Card 11 (`nstan != 0`): standard-reaction redefinitions
    /// (`errorr.f90:275-290`). Empty unless `nstan != 0`.
    pub standards: Vec<StandardRedefinition>,

    /// Card 12 (`ign==1` or `ign==-1`): read-in group boundaries. `None` for the
    /// built-in group structures.
    pub group_grid: Option<GroupGrid>,

    /// Cards 13/13b: input weight-function data, selected by `iwt`.
    pub weight_data: WeightData,

    /// The special `nendf==999` `covadd` merge option. `Some` only when card 1
    /// sets `nendf==999` (`errorr.f90:425`); a normal run leaves this `None`.
    pub covadd: Option<CovAddOption>,
}

impl Default for ErrorrInput {
    /// NJOY-faithful defaults (`errorr.f90:400-421`, `:472-476`, `:488-520`):
    /// `ign=1, iwt=6, iprint=1, irelco=1, mprint=0, tempin=300`, covariance
    /// control per [`CovarianceControl::default`]. `matd` and the tape units
    /// default to 0 and must be set for a real run.
    fn default() -> Self {
        Self {
            units: UnitAssignments::default(),
            matd: 0,
            group_structure: GroupStructure::ArbitraryRead,
            weight: WeightOption::ThermalOneOverEFissionFusion,
            iprint: CovariancePrint::Maximum,
            irelco: CovarianceForm::Relative,
            mprint: GroupAvgPrint::Minimum,
            tempin: 300.0,
            covariance: CovarianceControl::default(),
            user_mts: Vec::new(),
            derived: DerivedXsec::default(),
            extra_pairs: Vec::new(),
            standards: Vec::new(),
            group_grid: None,
            weight_data: WeightData::None,
            covadd: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level driver skeleton
// ---------------------------------------------------------------------------

/// Run the ERRORR module (`subroutine errorr`, `errorr.f90:134-1089`).
///
/// This is the **orchestration skeleton only**. The card-reading structure is
/// fully modelled by [`ErrorrInput`]; the numerical covariance kernels
/// (`gridd`, `grpav`/`colaps`/`grpav4`, `covcal`, `covout`, and the
/// `covadd` merge) are ported separately, so this function documents the
/// pipeline stages in-line and returns [`NjoyError::NotPorted`] rather than
/// fabricating a covariance matrix.
///
/// # Errors
/// Always returns [`NjoyError::NotPorted`] with `"errorr::run"` (or
/// `"errorr::covadd"` for the `nendf==999` path) until the kernels land.
pub fn run(input: &ErrorrInput) -> Result<(), NjoyError> {
    // --- Stage 0: input validation & header (errorr.f90:399-423) ---------
    //   set defaults, timer, banner, then card 1:
    //     read(nsysi,*) nendf,npend,ngout,nout,nin,nstan
    //   Modelled by `input.units`.

    // --- Special nendf==999 covadd merge (errorr.f90:425-444) ------------
    //   read nitape/notape + MT list, call covadd(...), close tapes, return.
    if input.covadd.is_some() {
        return Err(NjoyError::NotPorted("errorr::covadd"));
    }

    // --- Stage 1: open tapes, read TPID/CONT, detect ENDF version --------
    //   errorr.f90:446-469
    //     openz/repoz/tpidio/contio; n1h/n2h -> iverf = 4/5/6; emaxx.
    //   card 2 read at :477  (matd,ign,iwt,iprint,irelco) -> input.matd,
    //     input.group_structure, input.weight, input.iprint, input.irelco
    //   card 3 read at :490  (mprint,tempin) -> input.mprint, input.tempin

    // --- Stage 2: card 7 / covariance controls (errorr.f90:513-711) ------
    //   iverf != 4:  read(nsysi,*) iread,mfcov,irespr,legord,ifissp,efmean,dap
    //     -> input.covariance; MF=35 spectrum checks (ngchk, findf, listio);
    //     high-speed test rewrites of mfcov=-33/333 (input.covariance.speedup).

    // --- Stage 3: dictionary scan & covariance reaction list -------------
    //   errorr.f90:713-792
    //     read MF1/MT451 dictionary, set mf32/mf33 flags, collect reaction
    //     MTs (iga / lump / mats / mts) for the requested mfcov.

    // --- Stage 4: SAMMY resonance method check (errorr.f90:795-813) ------
    //   s2sammy(nendf,...) probes MF2; sets nmtres / mmtres or desammy().

    // --- Stage 5: derived-xsec coefficient setup -------------------------
    //   ENDF/B-IV (iverf==4): cards 4-6 at errorr.f90:819-857 -> input.derived
    //   ENDF/B-V/VI:          errorr.f90:861-919
    //     iread==1: cards 8/8a/8b/9 -> input.user_mts + input.derived
    //     iread==2: card 10 mat1-mt1 pairs -> input.extra_pairs
    //   card 11 (nstan != 0): standards redefinition -> input.standards
    //     (errorr.f90:921-938)

    // --- Stage 6: union energy grid + lumped reactions -------------------
    //   errorr.f90:965-976
    //     ngchk (condense GENDF), gridd(neki) builds the union grid and akxy,
    //     lumpmt() collapses lumped reactions.

    // --- Stage 7: user group grid (egngpn) ------------------------------
    //   errorr.f90:1004-1009
    //     card 12 (ign==+/-1) egn read here -> input.group_grid;
    //     card 13/13b weight data (iwt 1/4) -> input.weight_data;
    //     egngpn(ign,ngn,egn) materialises egn(1:ngn+1).

    // --- Stage 8: group constants on union grid -------------------------
    //   errorr.f90:1024-1040
    //     ngout==0 -> grpav(mprint,tempin) from pointwise (npend);
    //     else      -> colaps() from multigroup (ngout);
    //     mfcov==34 -> grpav4(mprint).

    // --- Stage 9: covariance calculation (covcal) -----------------------
    //   errorr.f90:1051-1054  -> the MF33 covariance matrices.

    // --- Stage 10: covariance output (covout) ---------------------------
    //   errorr.f90:1056-1060  -> fold in MF32, write output tape, atend.

    // --- Stage 11: cleanup (errorr.f90:1061-1088) -----------------------
    //   desammy, deallocate, repoz/closz all tapes, timer, footer.

    // Numeric kernels not yet ported — never fabricate a covariance result.
    Err(NjoyError::NotPorted("errorr::run"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify the [`ErrorrInput::default`] deck reproduces NJOY's initialised
    /// card values.
    ///
    /// **Methodology.** ERRORR sets its defaults before the free-format reads
    /// at `errorr.f90:400-421` (units), `:472-476` (card 2), `:488-520`
    /// (card 3 + card 7). This test constructs the Rust default and asserts each
    /// field maps back to the documented Fortran integer/real default.
    ///
    /// **Result (2026-07-15, commit ac5adf5).** All defaults match:
    /// `ign=1, iwt=6, iprint=1, irelco=1, mprint=0, tempin=300, iread=0,
    /// mfcov=33, irespr=1, legord=1, ifissp=-1000, efmean=2e6, dap=0`. Units and
    /// `matd` default to 0 (caller-supplied). Confirms the input model agrees
    /// with the upstream default block.
    #[test]
    fn defaults_match_njoy() {
        let inp = ErrorrInput::default();

        // card 1
        assert_eq!(inp.units, UnitAssignments::default());
        assert_eq!(inp.units.ngout, 0);
        assert_eq!(inp.units.nout, 0);

        // card 2 (errorr.f90:472-475)
        assert_eq!(inp.group_structure.to_ign(), 1);
        assert_eq!(inp.weight.to_iwt(), 6);
        assert_eq!(inp.iprint.to_iprint(), 1);
        assert_eq!(inp.irelco.to_irelco(), 1);

        // card 3 (errorr.f90:488-489)
        assert_eq!(inp.mprint.to_mprint(), 0);
        assert_eq!(inp.tempin, 300.0);

        // card 7 (errorr.f90:514-520)
        assert_eq!(inp.covariance.iread.to_iread(), 0);
        assert_eq!(inp.covariance.mfcov.to_mfcov(), 33);
        assert_eq!(inp.covariance.irespr.to_irespr(), 1);
        assert_eq!(inp.covariance.legord, 1);
        assert_eq!(inp.covariance.ifissp, -1000);
        assert_eq!(inp.covariance.efmean, 2.0e6);
        assert_eq!(inp.covariance.dap, 0.0);
        assert_eq!(inp.covariance.speedup, SpeedupOption::None);

        // optional cards absent by default
        assert!(inp.group_grid.is_none());
        assert!(inp.covadd.is_none());
        assert_eq!(inp.weight_data, WeightData::None);
        assert!(inp.user_mts.is_empty());
        assert!(inp.extra_pairs.is_empty());
        assert!(inp.standards.is_empty());
    }

    /// Round-trip every selector enum through its Fortran integer, and check
    /// the option-gating predicates.
    ///
    /// **Methodology.** For each selector enum, assert `from_*(to_*(x)) == x`
    /// over the full valid range and that out-of-range integers reject with
    /// `None`. Also assert the card-gating helpers (`reads_boundaries`,
    /// `reads_tab1`, `reads_analytic_params`, `requires_ngout`) match the
    /// Fortran `if` guards.
    ///
    /// **Result (2026-07-15).** All 34 `ign` values (`-1`, `1..=34`), 12 `iwt`
    /// values, 3 `iread` values, 5 `mfcov` values, and the 0/1 selectors
    /// round-trip; illegal integers return `None`. Gating predicates agree with
    /// `errorr.f90:945` (ign +/-1), `:951` (iwt 1), `:959` (iwt 4), and
    /// `:543-548` (mfcov 31/35/40).
    #[test]
    fn selector_enums_round_trip() {
        // ign: -1 and 1..=34
        for ign in std::iter::once(-1).chain(1..=34) {
            let g = GroupStructure::from_ign(ign).expect("valid ign");
            assert_eq!(g.to_ign(), ign);
        }
        assert!(GroupStructure::from_ign(0).is_none());
        assert!(GroupStructure::from_ign(35).is_none());
        assert!(GroupStructure::ArbitraryRead.reads_boundaries());
        assert!(GroupStructure::ArbitraryReadSupplemented.reads_boundaries());
        assert!(!GroupStructure::VitaminJ175.reads_boundaries());

        // iwt: 1..=12
        for iwt in 1..=12 {
            let w = WeightOption::from_iwt(iwt).expect("valid iwt");
            assert_eq!(w.to_iwt(), iwt);
        }
        assert!(WeightOption::from_iwt(0).is_none());
        assert!(WeightOption::from_iwt(13).is_none());
        assert!(WeightOption::ReadIn.reads_tab1());
        assert!(WeightOption::OneOverEPlusFissionPlusThermal.reads_analytic_params());
        assert!(!WeightOption::Constant.reads_tab1());
        assert!(!WeightOption::Constant.reads_analytic_params());

        // iread: 0..=2
        for iread in 0..=2 {
            let r = ReadOption::from_iread(iread).expect("valid iread");
            assert_eq!(r.to_iread(), iread);
        }
        assert!(ReadOption::from_iread(3).is_none());

        // mfcov: 31/33/34/35/40
        for mfcov in [31, 33, 34, 35, 40] {
            let f = CovarianceFile::from_mfcov(mfcov).expect("valid mfcov");
            assert_eq!(f.to_mfcov(), mfcov);
        }
        assert!(CovarianceFile::from_mfcov(32).is_none());
        assert!(CovarianceFile::from_mfcov(-33).is_none());
        assert!(CovarianceFile::Mf31.requires_ngout());
        assert!(CovarianceFile::Mf35.requires_ngout());
        assert!(CovarianceFile::Mf40.requires_ngout());
        assert!(!CovarianceFile::Mf33.requires_ngout());
        assert!(!CovarianceFile::Mf34.requires_ngout());

        // 0/1 selectors
        for i in 0..=1 {
            assert_eq!(CovariancePrint::from_iprint(i).unwrap().to_iprint(), i);
            assert_eq!(CovarianceForm::from_irelco(i).unwrap().to_irelco(), i);
            assert_eq!(GroupAvgPrint::from_mprint(i).unwrap().to_mprint(), i);
            assert_eq!(
                ResonanceProcessing::from_irespr(i).unwrap().to_irespr(),
                i
            );
        }
        assert!(CovariancePrint::from_iprint(2).is_none());
    }

    /// The driver skeleton must report `NotPorted`, never a fabricated result.
    ///
    /// **Methodology.** Call [`run`] with a default deck and with a `covadd`
    /// deck; assert both return [`NjoyError::NotPorted`] with the documented
    /// tag. Guards the "no fabrication" rule until the kernels are ported.
    ///
    /// **Result (2026-07-15).** Default deck -> `NotPorted("errorr::run")`;
    /// `covadd` deck -> `NotPorted("errorr::covadd")`.
    #[test]
    fn run_is_not_ported() {
        let inp = ErrorrInput::default();
        match run(&inp) {
            Err(NjoyError::NotPorted(tag)) => assert_eq!(tag, "errorr::run"),
            other => panic!("expected NotPorted(\"errorr::run\"), got {other:?}"),
        }

        let mut covadd_deck = ErrorrInput::default();
        covadd_deck.units.nendf = 999;
        covadd_deck.covadd = Some(CovAddOption {
            nitape: 20,
            notape: 21,
            add_mts: vec![18, 102],
        });
        match run(&covadd_deck) {
            Err(NjoyError::NotPorted(tag)) => assert_eq!(tag, "errorr::covadd"),
            other => panic!("expected NotPorted(\"errorr::covadd\"), got {other:?}"),
        }
    }
}
