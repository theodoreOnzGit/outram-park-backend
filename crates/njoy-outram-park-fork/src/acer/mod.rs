//! ACER — assemble and write a continuous-energy **ACE** library (Type-1 ASCII).
//!
//! Ported from the cross-section core of NJOY2016 `acefc.f90`:
//! - `acelod` — load ENDF data into the ACE arrays (the ESZ block and the
//!   MTR/LQR/TYR/LSIG/SIG cross-section blocks) — see [`build`].
//! - `aceout` + `change` — the Type-1 ASCII file format (header, NXS/JXS arrays,
//!   then the XSS data block written four 20-character fields per line).
//!   **One serialiser**: [`read::RawAceTable::to_type1_string`] and
//!   [`read::RawAceTable::to_type2_bytes`], on the shared edit descriptors in
//!   [`fortran_fmt`]. [`write`] converts an [`AceTable`] into that raw form
//!   and nothing else.
//!
//! ## What an ACE table is
//!
//! An ACE ("A Compact ENDF") table is the form a Monte-Carlo transport code
//! (MCNP, OpenMC) reads. It is three pieces of data:
//!
//! 1. a short **header** (ZAID string, atomic weight ratio, temperature, comment);
//! 2. two small integer arrays, **NXS** (16 dimensions) and **JXS** (32 locators
//!    into the data block, 1-based; `0` means "block absent"); and
//! 3. one big real array, **XSS**, holding every table laid end-to-end. JXS says
//!    where each block starts.
//!
//! ## Scope of this port (first ACER increment)
//!
//! This builds the **cross-section** portion of a neutron continuous-energy ACE
//! table, which is what RECONR + BROADR already give us:
//!
//! | ACE block | JXS slot | Status |
//! |-----------|----------|--------|
//! | ESZ (energy grid, total, disappearance, elastic, heating) | 1 (`esz`) | **built** |
//! | MTR / LQR / TYR / LSIG / SIG (reaction cross sections) | 3–7 | **built** |
//! | LAND / AND (**elastic** angular distribution, MT=2) | 8–9 | **built** (see [`angular`]) |
//! | NU (fission ν̄) | 2 (`nu`) | **built** (see [`nu`]) |
//! | LAND / AND (non-elastic angular distributions) | 8–9 | **built** (see [`angular`]) |
//! | LDLW / DLW (energy distributions) | 10–11 | **built** (see [`energy`]) |
//! | GPD / MTRP / LSIGP / SIGP / LANDP / ANDP / LDLWP / DLWP (photon production) | 12–19 | **built** (see [`photon_blocks`]) |
//! | heating (KERMA) — ESZ column 5 | — | **built** (HEATR H1–H5, via `from_reconr_full`'s `heating` arg) |
//!
//! > ~~The file this writes carries cross sections plus the **elastic**
//! > angular distribution. Secondary energy distributions (DLW) and
//! > non-elastic angular data are still absent, so a transport code cannot yet
//! > follow an inelastic or fission collision.~~ **CORRECTED 2026-09-21** —
//! > that claim was left behind by the increments that added them. `build.rs`
//! > populates JXS slots 2 (NU), 8–9 (LAND/AND for every reaction, not just
//! > elastic), 10–11 (LDLW/DLW) and 12–19 (the photon-production blocks);
//! > `grep -n 'jxs\[' src/acer/build.rs` lists them. The V&V records in
//! > `verification_and_validation/acer_ce_vs_njoy2016_multi_nuclide.md` compare
//! > all of them against NJOY over 55 evaluations.
//!
//! ## Other ACE classes
//!
//! `acer` writes five kinds of table and this crate now builds all five
//! (~~three of them~~, corrected 2026-10-04: the table below already listed
//! photo-atomic and photonuclear as built):
//!
//! | `iopt` | class | upstream | here |
//! |---|---|---|---|
//! | 1 | fast / continuous-energy (`c`) | `acefc.f90` | **built** — this module |
//! | 2 | thermal S(α,β) (`t`) | `aceth.f90` | **built** — [`thermal`], all three `IFENG` forms |
//! | 3 | dosimetry (`y`) | `acedo.f90` | **built** — [`dosimetry`] |
//! | 4 | photo-atomic (`p`) | `acepa.f90` | **built** — [`photoatomic`] |
//! | 5 | photonuclear (`u`) | `acepn.f90` | **built**, byte-exact — [`photonuclear`] (LANL-style path; others refuse by name) |
//! | 7, 8 | read / edit a Type-1 / Type-2 file | `acer.f90` | **read, edited and rewritten** — [`read`]; the `print` half is not implemented |
//!
//! ## Entry point
//!
//! ```no_run
//! use njoy_outram_park_fork::acer::AceTable;
//! use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
//! use njoy_outram_park_fork::endf::tape::Tape;
//! use std::fs::File;
//!
//! let tape = Tape::read(File::open("n-092_U_235-ENDF8.0.endf").unwrap()).unwrap();
//! let cfg  = ReconrConfig { mat: 9228, tolerance: 0.001, temperature: 0.0 };
//! let res  = reconr(&tape, &cfg).unwrap();
//!
//! // kT = 0 (0 K table); suffix .00 → ZAID "92235.00c"
//! let ace = AceTable::from_reconr(&res, 0.0, 0);
//! ace.write_type1("92235.00c.ace").unwrap();
//! ```

pub mod acelcp;
mod acelcp_ext;
pub mod charged;
pub mod charged_coulomb;
/// **A set of ACE files indexed by header** (ZAID, class, kT), for the ACE
/// way into transport (gh #936). See [`ace_set`].
pub mod ace_set;
pub mod acesix;
pub mod acelf5;
pub mod acelf6;
pub mod acensd;
pub mod angular;
pub mod build;
pub mod compare;
pub mod ce_decode;
pub mod ce_laws;
pub mod delayed;
pub mod delayed_blocks;
pub mod dosimetry;
pub mod energy;
pub mod fortran_fmt;
pub mod nu;
pub mod photoatomic;
pub mod photon_angular;
pub mod photon_blocks;
pub mod gamout;
pub mod photonuclear;

/// True when the evaluation supplies **MF=4/5/6 secondary distributions for
/// MT=19** (first-chance fission) — upstream's `mt19` flag, set from the tape
/// dictionary at `acefc.f90:388`.
///
/// This decides which fission representation an ACE table stores: with it set,
/// the partial chances MT=19/20/21/38 are stored and MT=18 is dropped; without
/// it, MT=18 is stored and the partials are dropped. Exactly one set may be
/// stored, because MT=18 is their sum.
///
/// Measured on ENDF/B-VIII.0, 2026-09-20: true for U-234 (MF=4/MT=19), false
/// for U-235 and U-238 — matching which reactions NJOY2016 puts in each of
/// those tables.
/// Exact integral of a **lin-lin** tabulation over `[a, b]`.
///
/// Both ACE tables are lin-lin by construction, so a trapezoid over each
/// panel — with the end panels clipped to `a` and `b` and the integrand
/// interpolated there — is not an approximation, it is the integral of the
/// function the table *defines*.
///
/// This is the instrument that makes a broadened-region comparison
/// possible at all: it depends only on the function each table represents,
/// not on where either code chose to put its grid points. A shared-point
/// comparison cannot reach the broadened region, because below `thnmax`
/// the two adaptive grids essentially never coincide.
pub fn integrate_linlin(e: &[f64], x: &[f64], a: f64, b: f64) -> f64 {
    if b <= a || e.len() < 2 {
        return 0.0;
    }
    let at = |i: usize, t: f64| -> f64 {
        // Linear interpolation inside panel i for energy t.
        let (e0, e1) = (e[i], e[i + 1]);
        if e1 <= e0 {
            return x[i];
        }
        x[i] + (x[i + 1] - x[i]) * (t - e0) / (e1 - e0)
    };
    let mut acc = 0.0;
    // First panel whose upper edge exceeds `a`.
    let mut i = match e.binary_search_by(|v| v.partial_cmp(&a).unwrap()) {
        Ok(k) => k,
        Err(k) => k.saturating_sub(1),
    };
    while i + 1 < e.len() && e[i + 1] <= a {
        i += 1;
    }
    while i + 1 < e.len() && e[i] < b {
        let lo = e[i].max(a);
        let hi = e[i + 1].min(b);
        if hi > lo {
            let (xlo, xhi) = (at(i, lo), at(i, hi));
            acc += 0.5 * (xlo + xhi) * (hi - lo);
        }
        i += 1;
    }
    acc
}

pub fn has_mt19_distributions(tape: &crate::endf::tape::Tape, mat: i32) -> bool {
    (4..=6).any(|mf| tape.section(mat, mf, 19).is_some())
}
/// Thermal scattering **S(α,β)** ACE table writer.
///
/// ~~scaffold only (Phase 4, scheduled after the continuous-energy ACE
/// library)~~ **CORRECTED 2026-09-17** — implemented: `thermal.rs` (387
/// lines) writes the ITIE/ITIX/ITXE inelastic blocks and the ITCE/ITCX
/// coherent-elastic Bragg blocks from [`crate::thermr`]'s
/// `IncoherentInelastic`/`CoherentElastic`, and is exercised by `op-1y4y`'s
/// closing evidence (`thermal_from_mf7` writing ITCE/ITCX from
/// `s_of_e_at(temp_k)`). See [`thermal`] for the table layout.
pub mod read;
pub mod thermal;
pub mod photon_read;
pub mod unr;
pub mod thermal_read;
pub mod write;

/// A continuous-energy ACE table held in memory.
///
/// Produced by [`AceTable::from_reconr`] and serialised by
/// [`write_type1`][AceTable::write_type1]. The three data pieces (header, the
/// integer arrays, and the [`xss`][AceTable::xss] data block) mirror the on-disk
/// layout exactly; see the [module docs](self) for the meaning of each array.
#[derive(Debug, Clone)]
pub struct AceTable {
    /// ZAID string, e.g. `"92235.00c"` — right-justified ZA + suffix + the
    /// incident-particle class letter (`c` = continuous-energy neutron). 10 chars.
    pub zaid: String,
    /// Atomic weight ratio AW0 = nuclide mass / neutron mass (`aw0` in NJOY).
    pub awr: f64,
    /// Temperature expressed as kT \[MeV\] (`tz` in NJOY): `k_B · T` converted to
    /// MeV. `0.0` for a 0 K table.
    pub kt_mev: f64,
    /// Processing-date string (`hd`), ≤ 10 chars. Cosmetic; a placeholder here.
    pub date: String,
    /// Free-text comment (`hk`), ≤ 70 chars.
    pub comment: String,
    /// Material-id string (`hm`), e.g. `"   mat9228"`. 10 chars.
    pub mat_id: String,
    /// The header's 16 IZ entries (their AW partners are written as 0). Zero
    /// for a table built from an incident evaluation; a thermal table carries
    /// ACER card 8a's moderator ZA values (`acer.f90:396-399`).
    pub iz: [i32; 16],
    /// The 16-entry **NXS** array of table dimensions (1-based in ACE docs;
    /// stored 0-based here). See [`nxs`](self::nxs) for the index names.
    pub nxs: [i32; 16],
    /// The 32-entry **JXS** array of 1-based locators into [`xss`](Self::xss).
    /// A zero entry means the block is absent. See [`jxs`](self::jxs) for names.
    pub jxs: [i32; 32],
    /// The **XSS** data block: every ACE block laid end-to-end. JXS locators are
    /// 1-based indices into this vector.
    pub xss: Vec<f64>,
    /// Per-element flag: `true` where the corresponding [`xss`](Self::xss) value
    /// is semantically an **integer** (e.g. an MT number, a point count, a
    /// locator) and must be written with the `i20` integer style rather than the
    /// `1pE20.11` real style. Length equals `xss.len()`.
    pub xss_is_int: Vec<bool>,
}

/// Named **NXS** indices (0-based into [`AceTable::nxs`]).
///
/// NXS holds the integer dimensions of the table. The ACE specification numbers
/// these 1..16; subtract one for the Rust array.
pub mod nxs {
    /// NXS(1): total length of the XSS data block.
    pub const LEN_XSS: usize = 0;
    /// NXS(2): ZA = 1000·Z + A.
    pub const ZA: usize = 1;
    /// NXS(3): NES — number of energies on the union grid.
    pub const NES: usize = 2;
    /// NXS(4): NTR — number of reactions stored in the MTR block (excludes
    /// elastic MT=2 and the redundant total MT=1).
    pub const NTR: usize = 3;
    /// NXS(5): NR — number of reactions that have angular distributions
    /// (~~deferred; `0`~~ **CORRECTED 2026-10-18** — written by `build.rs`, `nxs_arr[nxs::NR] = nr`).
    pub const NR: usize = 4;
    /// NXS(6): NTRP — number of photon-production reactions (~~deferred; `0`~~
    /// **CORRECTED 2026-10-18** — written by `build.rs` from the photon-production blocks).
    pub const NTRP: usize = 5;
    /// NXS(9): S — excited-state number of the target (`0` = ground state).
    /// NXS(8): **NDNF** — the number of delayed-neutron precursor groups.
    ///
    /// Read by [`super::delayed::decode_delayed`]; upstream's own reader uses
    /// `ace.nxs[8]` (`openmc/data/reaction.py:325`), hence `7` here.
    pub const NDNF: usize = 7;

    pub const S: usize = 8;
    /// NXS(10): Z — atomic number.
    pub const Z: usize = 9;
    /// NXS(11): A — mass number.
    pub const A: usize = 10;
}

/// Named **JXS** indices (0-based into [`AceTable::jxs`]).
///
/// Each entry is a 1-based locator into [`AceTable::xss`] marking where a block
/// begins; `0` means the block is absent. The ACE specification numbers these
/// 1..32; subtract one for the Rust array.
pub mod jxs {
    /// JXS(1): location of the ESZ block (always `1`).
    pub const ESZ: usize = 0;
    /// JXS(2): location of the fission ν̄ (NU) block.
    ///
    /// ~~deferred; `0`~~ **CORRECTED 2026-09-20** — written by [`super::nu`],
    /// and `0` now means only what it should: the material is not fissile.
    pub const NU: usize = 1;
    /// JXS(3): location of the MTR block (reaction MT numbers).
    pub const MTR: usize = 2;
    /// JXS(4): location of the LQR block (reaction Q-values \[MeV\]).
    pub const LQR: usize = 3;
    /// JXS(5): location of the TYR block (neutron yields / frame flags).
    pub const TYR: usize = 4;
    /// JXS(6): location of the LSIG block (per-reaction SIG locators).
    pub const LSIG: usize = 5;
    /// JXS(7): location of the SIG block (the reaction cross sections).
    pub const SIG: usize = 6;
    /// JXS(8): location of the LAND block (angular-distribution locators;
    /// ~~deferred; `0`~~ **CORRECTED 2026-10-18** — set by `build.rs` whenever a reaction is
    /// anisotropic or NR > 0).
    pub const LAND: usize = 7;
    /// JXS(9): location of the AND block (angular distributions;
    /// ~~deferred; `0`~~ **CORRECTED 2026-10-18** — set by `build.rs` with LAND).
    pub const AND: usize = 8;
    /// JXS(10): location of the LDLW block (energy-distribution locators;
    /// ~~deferred; `0`~~ **CORRECTED 2026-10-18** — always set by `build.rs`: the DLW block when
    /// NR > 0, else the next free word, as upstream, GitHub #872).
    pub const LDLW: usize = 9;
    /// JXS(11): location of the DLW block (energy distributions;
    /// ~~deferred; `0`~~ **CORRECTED 2026-10-18** — set by `build.rs` with LDLW).
    pub const DLW: usize = 10;
    /// JXS(22): location of the last word of the table (`END` = XSS length).
    /// JXS(13): location of the photon-production MT list (`MTRP`).
    ///
    /// `0` when the table carries no photon production — a legal ACE table,
    /// not a malformed one.
    pub const MTRP: usize = 12;
    /// JXS(14): locators into [`SIGP`], one per photon-production entry.
    pub const LSIGP: usize = 13;
    /// JXS(15): photon-production yields (MFTYPE=12) or cross sections
    /// (MFTYPE=13).
    pub const SIGP: usize = 14;
    /// JXS(16): locators into [`ANDP`]; `0` means isotropic.
    pub const LANDP: usize = 15;
    /// JXS(17): photon angular distributions. Empty when every photon is
    /// isotropic, in which case it equals [`LDLWP`].
    pub const ANDP: usize = 16;
    /// JXS(18): locators into [`DLWP`], one per photon-production entry.
    pub const LDLWP: usize = 17;
    /// JXS(19): photon energy distributions (ACE Law 2 discrete lines or
    /// Law 4 continua).
    pub const DLWP: usize = 18;
    /// JXS(20): **YP** -- `[NYP, MT(1..NYP)]`, the distinct reactions whose
    /// photon production is a yield (MF=12/16) rather than a cross section
    /// (`acelpp`, `acefc.f90:8993-9008`).
    pub const YP: usize = 19;
    /// JXS(21): **FIS** -- locator of MT=18's own SIG entry when total
    /// fission is a stored reaction (`fis=next`, `acefc.f90:5495`); `0`
    /// otherwise, e.g. when the partial chances are stored instead.
    pub const FIS: usize = 20;

    pub const END: usize = 21;

    /// JXS(23): **LUNR** — the unresolved-resonance probability tables.
    ///
    /// Zero when the evaluation has no unresolved range, which is the usual
    /// case for a light nuclide. Read by
    /// [`crate::purr::UrrProbabilityTables::from_ace`]; upstream's own reader
    /// is `openmc/data/urr.py::ProbabilityTables.from_ace`, which indexes
    /// `ace.jxs[23]` (1-based) — hence `22` here.
    pub const LUNR: usize = 22;

    /// JXS(24): **DNU** — delayed ν̄_d. Zero ⇒ the table carries no delayed
    /// data, which upstream uses as the test for exactly that
    /// (`openmc/data/reaction.py:319`).
    pub const DNU: usize = 23;

    /// JXS(25): **BDD** — basic delayed data: per precursor group, the decay
    /// constant (in inverse **shakes**) followed by the group's probability
    /// TAB1.
    pub const BDD: usize = 24;

    /// JXS(26): **DNEDL** — one locator per group into [`DNED`].
    pub const DNEDL: usize = 25;

    /// JXS(27): **DNED** — the delayed neutrons' energy distributions.
    ///
    /// ~~Not decoded, and not a gap in the delayed-neutron path: `DelayedData`
    /// carries no outgoing spectrum on **either** route — the ENDF route's
    /// `DelayedData::from_tape` keeps `DelayedChi`'s `fraction` and drops its
    /// `spectrum` too. Whoever adds a delayed emission spectrum needs this
    /// block and MF=5/MT=455 together.~~ **CORRECTED 2026-09-29** — decoded by
    /// `acer::delayed::decode_delayed` (one law per group via DNEDL) and
    /// sampled for delayed neutrons, with MF=5/MT=455 on the ENDF route
    /// (GitHub #365 audit).
    pub const DNED: usize = 26;
}

/// Run the ACER card-input driver (NJOY module entry point).
///
/// **Status:** this module's processing physics is ported (see its `README.md`
/// and the typed API above); the NJOY *card-input driver* itself is not yet
/// ported, so this returns [`crate::NjoyError::NotPorted`]. Use the module's
/// typed API directly rather than this driver.
pub fn run() -> Result<(), crate::NjoyError> {
    Err(crate::NjoyError::NotPorted(
        "acer driver (physics ported — use the module API)",
    ))
}

/// Assemble a complete continuous-energy [`AceTable`] from a tape and its
/// reconstructed, broadened cross sections.
///
/// # Why this exists
///
/// [`AceTable::from_reconr_full`] takes nine arguments, seven of which are
/// themselves built by a fixed sequence of calls — elastic angular from MF=4,
/// `build_emissions` over the partial Q-values, nu-bar, chi, emission spectra,
/// photon production, KERMA with the energy balance applied, and the NU block.
/// That sequence was written out **identically** in
/// `njoy-outram-park-fork/examples/write_ace.rs` and in
/// `outram-mc-libs/examples/lct008_ace_roundtrip.rs`, and a third copy was
/// about to be added for the 2026-09-24 LCT-008 timing sweep.
///
/// Three hand-copies of an assembly order is how two of them end up subtly
/// different and nobody notices: the ACE they produce still writes, still
/// reads, and transports to a slightly different answer. Having one caller
/// path means a change to the order changes every table this workspace
/// builds, or none.
///
/// `kt_mev` is the ACE temperature in MeV — `8.617333262e-5 * T[K] * 1e-6`.
/// `suffix` is the ZAID suffix digit (`0` for the usual `.00c`).
///
/// # This is the deck WITHOUT PURR, so the table carries no URR
///
/// `build_full` mirrors NJOY's `RECONR+ACER` / `RECONR+BROADR+ACER` decks and
/// writes **no UNR block**, so a nuclide read back from its output has no
/// unresolved-resonance self-shielding. It stays that way because it is what
/// several byte-parity gates compare against NJOY tables made without PURR.
/// A table meant for **transport** should come from [`build_full_with_purr`],
/// which is the `…+PURR+ACER` deck (GitHub #325).
pub fn build_full(
    tape: &crate::endf::tape::Tape,
    mat: i32,
    recon: &crate::reconr::ReconrResult,
    kt_mev: f64,
    suffix: u32,
) -> Result<AceTable, crate::NjoyError> {
    build_deck(tape, mat, recon, kt_mev, suffix, &AceDeck::default())
}

/// Which NJOY modules a deck runs ahead of ACER, for [`build_deck`].
///
/// HEATR is **on** by default: heating is physics the evaluation supplies,
/// and the workspace rule is that such physics is default-on with ablation an
/// explicit act. [`AceDeck::without_heatr`] is that act -- it reproduces a
/// deck with no HEATR module, whose PENDF carries no MT=301, so `acelod`
/// leaves the ESZ heating column at zero (`acefc.f90:5641-5645`). The
/// reference tables in `reference-data/ace/reference-njoy/` were made that
/// way, which is the reason this exists.
#[derive(Debug, Clone, PartialEq)]
pub struct AceDeck {
    heatr: bool,
    purr: Option<(usize, usize, usize)>,
    temperature_k: Option<f64>,
    /// HEATR's card 3 asked for the partial kermas `302`, `318`, `402` (each
    /// flag in that order): PURR's `rdheat` then finds them (`ihave = 2`).
    heatr_partials: [bool; 3],
    /// PURR's output unit was blocked binary (`nout < 0`).
    purr_binary: bool,
    /// HEATR's output unit was blocked binary (`nout < 0`).
    heatr_binary: bool,
    /// ACER's PENDF unit is blocked binary (negative `npend`): its energies and
    /// cross sections stay at full double precision instead of going through
    /// ENDF text (gh:#1449).
    pendf_binary: bool,
    /// ACER's ENDF unit is blocked binary (negative `nendf`): `convr`'s scratch
    /// tape is then binary and photon production keeps full doubles (gh:#1492).
    endf_binary: bool,
    /// ACER's `newfor`: `1` (default) the new angular format, `0` the old
    /// equally probable bins (gh:#1449).
    newfor: i32,
    /// The old-style 30 x 20 equally probable photon matrix `gamout` makes from
    /// ACER's `ngend` GENDF; stored after GPD (gh:#1481).
    photon_matrix: Option<Vec<f64>>,
    /// MT=301 as HEATR's tape carries it, `(E, H)` pairs; when set, ESZ's
    /// heating column reads these (ACER reads the tape, not HEATR's memory).
    esz_kerma: Option<Vec<(f64, f64)>>,
    /// MT=444 (damage energy) as HEATR's tape carries it, `(E [eV], sigma [eV-b])`.
    mt444: Option<Vec<(f64, f64)>>,
    /// GASPR's MT=203-207 as the PENDF tape carries them: `(MT, (E, sigma) pairs)`.
    gas: Vec<(i32, Vec<(f64, f64)>)>,
    /// HEATR's card-2 `ed` (OpenMC #4022): `Some(0.0)` asks for the damage
    /// energy (partial `444`) with NJOY's default displacement table,
    /// `Some(ed)` overrides it with `ed` eV, `None` (default) asks for neither.
    displacement_energy: Option<f64>,
}

impl Default for AceDeck {
    fn default() -> Self {
        AceDeck {
            heatr: true,
            purr: None,
            temperature_k: None,
            heatr_partials: [false; 3],
            purr_binary: false,
            heatr_binary: false,
            pendf_binary: false,
            endf_binary: false,
            newfor: 1,
            photon_matrix: None,
            esz_kerma: None,
            mt444: None,
            gas: Vec::new(),
            displacement_energy: None,
        }
    }
}

impl AceDeck {
    /// The deck without HEATR: the ESZ heating column is written as zero.
    pub fn without_heatr(mut self) -> Self {
        self.heatr = false;
        self
    }

    /// HEATR's card 3 asked for the partial kermas MT=302, 318, 402 (each flag
    /// in that order). PURR then self-shields the heating column
    /// (`purr.f90:533-558`) instead of dividing MT=301 by the total.
    pub fn with_heatr_partials(mut self, partials: [bool; 3]) -> Self {
        self.heatr_partials = partials;
        self
    }

    /// Ask HEATR for the damage-energy production (MT=444) with the threshold
    /// displacement energy `ed` \[eV\] on its card 2 (OpenMC #4022,
    /// `make_ace(displacement_energy=...)`): `0.0` is NJOY's built-in table.
    /// The result is [`DeckModules`]' `damage_energy`; MT=301 is unchanged.
    pub fn with_displacement_energy(mut self, ed: Option<f64>) -> Self {
        self.displacement_energy = ed;
        self
    }

    /// The `30 x 20` photon matrix of `gamout` ([`gamout::photon_spectra`]), from
    /// ACER's `ngend` GENDF; it follows GPD (`acelod`, `acefc.f90:6276-6300`).
    pub fn with_photon_matrix(mut self, m: Option<Vec<f64>>) -> Self {
        self.photon_matrix = m;
        self
    }

    /// ACER's `newfor` (`0` the old angular format, `1` the new, the default).
    pub fn with_newfor(mut self, newfor: i32) -> Self {
        self.newfor = newfor;
        self
    }

    /// ACER's ENDF unit is blocked binary (`nendf < 0`): `convr`'s scratch
    /// tape is binary and photon production keeps full doubles (gh:#1492).
    pub fn with_endf_binary(mut self, binary: bool) -> Self {
        self.endf_binary = binary;
        self
    }

    /// ACER's PENDF unit is blocked binary (`npend < 0`): its values stay full
    /// doubles and `gamsum`'s scratch tape is binary (gh:#1449, #1492).
    pub fn with_pendf_binary(mut self, binary: bool) -> Self {
        self.pendf_binary = binary;
        self
    }

    /// See the `heatr_binary` field.
    pub fn with_heatr_binary(mut self, binary: bool) -> Self {
        self.heatr_binary = binary;
        self
    }

    /// MT=301 as the HEATR tape holds it: ESZ's heating column is built from
    /// these `(E, H)` pairs, as upstream's ACER reads the tape (gh:#1449: the
    /// in-memory kerma differs from the printed value by up to half a unit in
    /// the 7th figure, which moved one ESZ word of test 07).
    pub fn with_esz_kerma_pairs(mut self, pairs: Vec<(f64, f64)>) -> Self {
        self.esz_kerma = Some(pairs);
        self
    }

    /// MT=444 from HEATR's tape: stored by ACER as a reaction (`unionx` sets
    /// its threshold to `elow`; `acelod` stores `sigfig(s/emev,7,0)`,
    /// `acefc.f90:1767, 5664`) and its energies join the union grid.
    pub fn with_mt444_pairs(mut self, pairs: Vec<(f64, f64)>) -> Self {
        self.mt444 = Some(pairs);
        self
    }

    /// GASPR's MT=203-207 from the PENDF tape: ACER stores them as reactions
    /// (`unionx` skips only MT > 207 up to 221, `acefc.f90:1735-1745`), so a deck
    /// that ran GASPR gets them in MTR/LSIG/SIG (gh:#1369, test 13).
    pub fn with_gas_pairs(mut self, gas: Vec<(i32, Vec<(f64, f64)>)>) -> Self {
        self.gas = gas;
        self
    }

    /// PURR's output unit was blocked binary (`nout < 0`): its MT=153 reaches
    /// ACER without a text round trip.
    pub fn with_purr_binary(mut self, binary: bool) -> Self {
        self.purr_binary = binary;
        self
    }

    /// Add PURR with `nbin` bands, `nladr` ladders and `nsamp` samples.
    pub fn with_purr(mut self, nbin: usize, nladr: usize, nsamp: usize) -> Self {
        self.purr = Some((nbin, nladr, nsamp));
        self
    }

    /// The deck's card temperature \[K\] (BROADR's and PURR's `293.6/`).
    /// PURR samples its ladders at this temperature. Without it, PURR uses
    /// `kt_mev / k_B`. That is exact only when `kt_mev` was computed from the
    /// temperature: a kT read back from a table header is printed to five
    /// figures, and `2.5301e-8 MeV / k_B` is not 293.6 K. That offset moved
    /// U-234's probability bands in the 5th figure against NJOY2016's
    /// (measured 2026-09-26).
    pub fn with_temperature(mut self, temperature_k: f64) -> Self {
        self.temperature_k = Some(temperature_k);
        self
    }
}

/// Assemble a continuous-energy [`AceTable`] for the given [`AceDeck`]. The
/// single assembly path behind [`build_full`] and [`build_full_with_purr`].
pub fn build_deck(
    tape: &crate::endf::tape::Tape,
    mat: i32,
    recon: &crate::reconr::ReconrResult,
    kt_mev: f64,
    suffix: u32,
    deck: &AceDeck,
) -> Result<AceTable, crate::NjoyError> {
    // ACER reads its cross sections from a PENDF tape, not from memory.
    let pendf = if deck.pendf_binary { recon.clone() } else { recon.through_pendf_text() };
    let modules = deck_modules(tape, mat, &pendf, kt_mev, deck)?;
    assemble(tape, mat, &pendf, kt_mev, suffix, &modules)
}

/// What the modules a deck runs between BROADR and ACER leave on the PENDF:
/// PURR's probability tables and HEATR's MT=301. [`build_deck`] computes them
/// and hands them to [`assemble`]; [`crate::process::cross_sections`] keeps
/// them in its result so the ACE file is assembled from the same numbers.
#[derive(Debug, Clone, Default)]
pub(crate) struct DeckModules {
    /// PURR's tables; `None` without PURR or without an unresolved range.
    pub(crate) urr: Option<crate::purr::UrrProbabilityTables>,
    /// HEATR's KERMA; `None` without HEATR.
    pub(crate) kerma: Option<crate::heatr::Kerma>,
    /// HEATR's damage-energy production MT=444 \[eV b\] when a displacement
    /// energy was asked for (OpenMC #4022); `None` otherwise.
    pub(crate) damage_energy: Option<crate::heatr::Kerma>,
    /// ACER's `newfor` (gh:#1449).
    pub newfor: i32,
    /// The `gamout` photon matrix after GPD (gh:#1481).
    pub(crate) photon_matrix: Option<Vec<f64>>,
    /// HEATR's MT=444 `(E, sigma)` pairs, when the deck's HEATR wrote it.
    pub(crate) mt444: Option<Vec<(f64, f64)>>,
    /// GASPR's MT=203-207 pairs from the PENDF tape.
    pub(crate) gas: Vec<(i32, Vec<(f64, f64)>)>,
    /// ACER's PENDF unit is blocked binary, so `gamsum`'s scratch tape is too
    /// (gh:#1492).
    pub(crate) pendf_binary: bool,
    /// ACER's ENDF unit is blocked binary, so `convr`'s scratch tape is too.
    pub(crate) endf_binary: bool,
}

/// Run the deck's PURR and HEATR on `pendf` (a result already through
/// [`crate::reconr::ReconrResult::through_pendf_text`]).
pub(crate) fn deck_modules(
    tape: &crate::endf::tape::Tape,
    mat: i32,
    pendf: &crate::reconr::ReconrResult,
    kt_mev: f64,
    deck: &AceDeck,
) -> Result<DeckModules, crate::NjoyError> {
    const K_BOLTZMANN_MEV: f64 = 8.617_333_262e-11;
    // HEATR, as the deck runs it: the full translation of `heatr.f90` on this
    // PENDF, photons transported (`local = 0`); ACER takes MT=301 from its
    // tape as upstream's ACER takes it from HEATR's (`acefc.f90:5641-5645`).
    // Before 2026-10-05 this was `Kerma::from_endf(..).with_energy_balance`,
    // an approximation of `nheat`/`gheat` on its own grid. It runs BEFORE
    // PURR, as in the deck (`... broadr -> heatr -> purr -> acer`), because
    // PURR's `rdheat` reads its MT=301 for the bands' heating column.
    let mut wanted: Vec<i32> = [302, 318, 402]
        .iter()
        .zip(deck.heatr_partials)
        .filter_map(|(&mt, on)| if on { Some(mt) } else { None })
        .collect();
    // OpenMC #4022: `make_ace(displacement_energy=ed)` puts `ed` on HEATR's
    // card 2 and asks for the damage-energy partial 444.
    if deck.displacement_energy.is_some() && deck.heatr {
        wanted.push(444);
    }
    let (kerma, partial_kermas) = if deck.heatr {
        let (total, parts) = crate::heatr::heatr_kerma_with_partials_ed(
            tape,
            mat,
            pendf,
            &wanted,
            !deck.heatr_binary,
            deck.displacement_energy.unwrap_or(0.0),
        )?;
        (Some(total), parts)
    } else {
        (None, Vec::new())
    };
    let mut damage_energy = None;
    // `rdheat`: MT=302, 318, 402 by position of the three, `None` when absent.
    let mut partial: [Option<crate::heatr::Kerma>; 3] = [None, None, None];
    for (mt, k) in wanted.iter().zip(partial_kermas) {
        let slot = match mt {
            302 => 0,
            318 => 1,
            402 => 2,
            _ => {
                damage_energy = k;
                continue;
            }
        };
        partial[slot] = k;
    }
    let urr = match deck.purr {
        Some((nbin, nladr, nsamp)) => crate::purr::UrrProbabilityTables::from_endf_with_heating(
            tape,
            mat,
            deck.temperature_k.unwrap_or(kt_mev / K_BOLTZMANN_MEV),
            nbin,
            nladr,
            nsamp,
            &crate::purr::PurrHeating { total: kerma.clone(), partial, binary: deck.purr_binary },
        )?,
        None => None,
    };
    // ACER's ESZ reads MT=301 from the PENDF as HEATR left it: every value
    // went through `hout`'s `sigfig(v, 7, 0)` (6 figures above `big = 1e10`,
    // `heatr.f90` hout) and then, on the scratch tape `unionx` writes, through
    // `a11` text. `format_endf_float` alone rounds half-up on the binary
    // double, which differs from `sigfig` on a value like 3204091.4999...:
    // test 07's ESZ heating at 81.99999 eV (gh:#1449, 75.45917 against NJOY's
    // 75.45914) was that one word. PURR's `rdheat` was given the unrounded
    // doubles above.
    let kerma = if deck.heatr_binary {
        kerma.map(|k| {
            use crate::endf::parse::{format_endf_float, parse_endf_float};
            use crate::mixr::mix::sigfig;
            let rt = |x: f64| parse_endf_float(&format_endf_float(x)).unwrap_or(x);
            let rth = |x: f64| rt(sigfig(x, if x > 1.0e10 { 6 } else { 7 }, 0));
            crate::heatr::Kerma { energy: k.energy.iter().map(|&x| rt(x)).collect(), h: k.h.iter().map(|&x| rth(x)).collect() }
        })
    } else {
        kerma
    };
    let kerma = match &deck.esz_kerma {
        Some(p) if !p.is_empty() => Some(crate::heatr::Kerma {
            energy: p.iter().map(|x| x.0).collect(),
            h: p.iter().map(|x| x.1).collect(),
        }),
        _ => kerma,
    };
    Ok(DeckModules { urr, kerma, damage_energy, newfor: deck.newfor, photon_matrix: deck.photon_matrix.clone(), mt444: deck.mt444.clone(), gas: deck.gas.clone(), pendf_binary: deck.pendf_binary, endf_binary: deck.endf_binary })
}

/// ACER proper: the continuous-energy table from the PENDF and what
/// [`deck_modules`] left on it. `pendf` must already be through
/// [`crate::reconr::ReconrResult::through_pendf_text`].
pub(crate) fn assemble(
    tape: &crate::endf::tape::Tape,
    mat: i32,
    pendf: &crate::reconr::ReconrResult,
    kt_mev: f64,
    suffix: u32,
    modules: &DeckModules,
) -> Result<AceTable, crate::NjoyError> {
    // MT=444 joins the reactions as `unionx` includes it (`acefc.f90:1743-1782`).
    let with_gas;
    let pendf = if modules.gas.is_empty() {
        pendf
    } else {
        let mut r = pendf.clone();
        for (mt, pairs) in &modules.gas {
            let sec = crate::reconr::ReconrSection { lr: 0, mt: crate::endf::mt::MtReaction::from_any(*mt), qi: 0.0, pairs: pairs.clone() };
            let at = r.sections.iter().position(|s| i32::from(s.mt) > *mt).unwrap_or(r.sections.len());
            r.sections.insert(at, sec);
        }
        with_gas = r;
        &with_gas
    };
    let with444;
    let recon = match &modules.mt444 {
        Some(pairs) if !pairs.is_empty() => {
            let mut r = pendf.clone();
            let sec = crate::reconr::ReconrSection {
                lr: 0,
                mt: crate::endf::mt::MtReaction::from_any(444),
                qi: 0.0,
                pairs: pairs.clone(),
            };
            let at = r.sections.iter().position(|s| i32::from(s.mt) > 444).unwrap_or(r.sections.len());
            r.sections.insert(at, sec);
            with444 = r;
            &with444
        }
        _ => pendf,
    };
    let urr = modules.urr.as_ref();
    let kerma = modules.kerma.as_ref();
    let angular = match tape.section(mat, 4, 2) {
        Some(s) => Some(crate::acer::angular::parse_elastic_angular_newfor(s, modules.newfor)?),
        None => None,
    };
    let partials: Vec<(i32, f64)> = recon
        .sections
        .iter()
        .map(|s| (i32::from(s.mt), s.qi))
        .collect();
    let emissions = crate::acer::energy::build_emissions_mode(tape, mat, recon.material.awr, &partials, modules.newfor, !modules.pendf_binary);
    let nu_block = crate::acer::nu::build(tape, mat)?;
    // Delayed-neutron blocks whenever the evaluation has them, as NJOY writes
    // them regardless of the rest of the deck (`acefc.f90:6003`). ismooth = 1
    // is upstream's default (`acer.f90:326`).
    let delayed = crate::acer::delayed_blocks::build(tape, mat, true)?;
    let mut table = AceTable::from_reconr_full_with_extras_disc(
        recon,
        kt_mev,
        suffix,
        angular.as_ref(),
        &emissions,
        kerma,
        nu_block.as_deref(),
        crate::acer::has_mt19_distributions(tape, mat),
        crate::acer::photon_blocks::build_with_pendf_units(tape, mat, recon, !modules.endf_binary, !modules.pendf_binary).as_deref(),
        urr,
        delayed.as_ref(),
        &crate::acer::photon_blocks::gamma_discontinuities(tape, mat),
    );
    if let Some(mut g) = crate::acer::photon_blocks::gpd_with_units(tape, mat, recon, !modules.endf_binary, !modules.pendf_binary) {
        // `acelod`: the 30 x 20 photon matrix follows GPD when ACER had an `ngend` GENDF.
        if let Some(m) = &modules.photon_matrix {
            g.extend_from_slice(m);
        }
        table.insert_gpd(&g);
    }
    // NJOY's `hm`: the ENDF MAT, `'   mat',i4` (`acefc.f90:4949`). Found
    // 2026-10-14 (gh:#864) by a browser-built U-234 table whose every other
    // byte matched NJOY2016's: it read `mat92234` (the ZA) for `mat9225`.
    table.mat_id = format!("   mat{mat:4}");
    // Charged-particle production, after END (`acelcp`, `acefc.f90:6323`).
    crate::acer::acelcp::append_mode(&mut table, tape, mat, recon, modules.newfor, !modules.pendf_binary);
    Ok(table)
}

impl AceTable {
    /// Insert the GPD block (`JXS(12)`) immediately before MTRP, where
    /// `acelod` stores it (`acefc.f90:6255-6271`), and shift every locator
    /// that follows. The photon blocks' internal locators are relative, so
    /// only the absolute `JXS` entries move.
    ///
    /// Does nothing without photon-production blocks, or when `gpd` is not
    /// one word per ESZ energy.
    pub fn insert_gpd(&mut self, gpd: &[f64]) {
        let nes = self.nxs[nxs::NES] as usize;
        let at = self.jxs[jxs::MTRP];
        // GPD is `nes` words, plus the `30 x 20` photon matrix when there is one.
        if at <= 0 || (gpd.len() != nes && gpd.len() != nes + 600) || self.jxs[11] != 0 {
            return;
        }
        let nes = gpd.len();
        let pos = (at - 1) as usize;
        self.xss.splice(pos..pos, gpd.iter().copied());
        self.xss_is_int.splice(pos..pos, std::iter::repeat_n(false, nes));
        for k in [
            jxs::MTRP,
            jxs::LSIGP,
            jxs::SIGP,
            jxs::LANDP,
            jxs::ANDP,
            jxs::LDLWP,
            jxs::DLWP,
            jxs::YP,
        ] {
            if self.jxs[k] >= at {
                self.jxs[k] += nes as i32;
            }
        }
        self.jxs[11] = at;
        self.jxs[jxs::END] += nes as i32;
        self.nxs[nxs::LEN_XSS] += nes as i32;
    }
}

/// [`build_full`] **with PURR**: the same table plus the UNR probability-table
/// block, i.e. NJOY's `RECONR -> BROADR -> PURR -> ACER` deck — GitHub #325.
///
/// Until this existed, no ACE table this workspace wrote could carry URR
/// self-shielding, even though `Nuclide::from_ace` has decoded the block since
/// GitHub #307. So the round trip `ENDF -> ACER -> read` lost physics that both
/// ends could handle, and every ACE-route parity study compared different
/// physics on its two arms.
///
/// The tables are generated by
/// [`UrrProbabilityTables::from_endf`](crate::purr::UrrProbabilityTables::from_endf)
/// at the table's own temperature (`kt_mev` converted back to kelvin) and
/// written by [`unr::unr_words`], a port of `acefc.f90:5958-5990`. `nbin`,
/// `nladr` and `nsamp` are PURR's controls; the reference library in
/// `reference-data/ace` was made with `20 / 64` bins/ladders (its deck is
/// `purr / 20 22 23 / MAT 1 1 20 64 /`), and this crate's own verified
/// comparison uses `20 / 64 / 10000`.
///
/// **Cost:** PURR is a Monte Carlo over resonance ladders — ~45 s for U-238's
/// 83 energies at `20 / 64 / 10000` on one core. That is the price of the
/// physics, stated rather than used as a reason to default it off.
///
/// A nuclide without an unresolved range (`from_endf` returns `None`) gets
/// exactly the [`build_full`] table: no UNR block and `JXS(23) = 0`, which is
/// ACE's "no unresolved range" and not an omission.
pub fn build_full_with_purr(
    tape: &crate::endf::tape::Tape,
    mat: i32,
    recon: &crate::reconr::ReconrResult,
    kt_mev: f64,
    suffix: u32,
    nbin: usize,
    nladr: usize,
    nsamp: usize,
) -> Result<AceTable, crate::NjoyError> {
    build_deck(tape, mat, recon, kt_mev, suffix, &AceDeck::default().with_purr(nbin, nladr, nsamp))
}
